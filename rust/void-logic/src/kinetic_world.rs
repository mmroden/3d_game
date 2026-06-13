//! KineticWorld: the representation of every ballistic mover.
//!
//! Ballistic bodies have no propulsion — the type exposes no thrust
//! API — and their momentum changes only through collisions (plus the
//! one-shot `disturb` impulse used at level setup and by collision
//! response). The world steps every body centrally whether or not
//! anything renders it: trajectories are representation; rendering is
//! a view concern.
//!
//! Rest is the default and the equilibrium: a body at exactly zero
//! velocity is skipped entirely (free, exact sleeping — a theorem of
//! the regime, not a tuned heuristic), and rest capture snaps slow
//! bodies back to zero after bounces so disturbed rooms settle again.

use crate::kinetics::{
    add, sanitize, scale, sub, dot, ControlInput, Impulse, Mass, Restitution, Retention,
    SpeedLimits,
};
use crate::room_assembler::CollisionBox;
use rapier3d::prelude::*;

/// Speed caps for ballistic bodies, asserted at the facade after every
/// step (defense in depth alongside rapier's CCD).
const BALLISTIC_MAX_SPEED: f32 = 20.0;
const BALLISTIC_MAX_SPIN: f32 = 5.0;

/// Below this speed, a moving body is captured at exact rest —
/// micro-settling, so disturbed rooms return to equilibrium. Rapier's
/// own sleeping is threshold-based; the facade reasserts exact zeros
/// so `is_at_rest` stays a theorem, not an approximation.
const REST_SPEED: f32 = 0.075;

/// Collider user-data tag for static level geometry.
const STATIC_TAG: u128 = u128::MAX;

/// Handle to a body in the world. Ids are stable for the level's
/// lifetime: removed bodies tombstone their slot and the id is never
/// reused, so consumers may hold ids across ticks (and must correlate
/// snapshots by id, never by index — snapshots compact).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyId(usize);

impl BodyId {
    pub fn index(self) -> usize {
        self.0
    }

    /// Test-only constructor; real ids come only from the world's
    /// register pathways.
    #[cfg(test)]
    pub(crate) fn from_index(index: usize) -> Self {
        Self(index)
    }
}

/// A mover with no propulsion: a drifting bookshelf, a chunk of
/// wreckage, a physical projectile. There is deliberately no way to
/// thrust one. This struct is the read-mirror of the simulated body:
/// reads come from here; every mutation flows through the facade into
/// the physics engine and is synced back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BallisticBody {
    position: [f32; 3],
    radius: f32,
    restitution: Restitution,
    mass: Option<Mass>,
    linear_velocity: [f32; 3],
    angular_velocity: [f32; 3],
}

/// The landing zone: the only artifact that crosses from physics to
/// rendering. Immutable once published; the view keeps the previous
/// snapshot it consumed, interpolates toward the newest, and can never
/// name `KineticWorld`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WorldSnapshot {
    /// Monotonic tick counter; one publication per `step`.
    pub tick: u64,
    pub bodies: Vec<BodySnapshot>,
}

/// One body's render-relevant state at a tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodySnapshot {
    pub id: BodyId,
    pub position: [f32; 3],
    /// Cosmetic tumble rate for the view to integrate.
    pub angular_velocity: [f32; 3],
    pub at_rest: bool,
}

/// What a ballistic body collided with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContactWith {
    Body(BodyId),
    Static,
}

/// A contact that began this tick (onset only — a continuous contact
/// emits exactly one). The shell maps these to consequences (ram
/// damage, SFX); the world never interprets them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContactEvent {
    pub body: BodyId,
    pub with: ContactWith,
    /// World-space contact normal, pointing away from `body`.
    pub normal: [f32; 3],
    /// Closing speed along the normal at impact.
    pub impact_speed: f32,
    pub position: [f32; 3],
}

impl BallisticBody {
    /// Every ballistic body starts at rest: stillness is the 65My
    /// equilibrium of an abandoned base. Motion arrives only by
    /// collision or an explicit disturbance.
    pub fn at_rest(position: [f32; 3], radius: f32, restitution: Restitution) -> Self {
        Self {
            position,
            radius,
            restitution,
            mass: None,
            linear_velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
        }
    }

    /// Explicit inertial mass. Bodies without one get a density-1
    /// sphere mass, so equal-size props exchange momentum evenly.
    pub fn with_mass(mut self, mass: Mass) -> Self {
        self.mass = Some(mass);
        self
    }

    pub fn position(&self) -> [f32; 3] {
        self.position
    }

    pub fn radius(&self) -> f32 {
        self.radius
    }

    pub fn linear_velocity(&self) -> [f32; 3] {
        self.linear_velocity
    }

    /// Cosmetic tumble for the view; ballistic colliders are spheres,
    /// so orientation never affects physics.
    pub fn angular_velocity(&self) -> [f32; 3] {
        self.angular_velocity
    }

    pub fn is_at_rest(&self) -> bool {
        self.linear_velocity == [0.0; 3] && self.angular_velocity == [0.0; 3]
    }
}

/// Facade-side state of a powered body: its control slot and motion
/// envelope. Rapier owns linear motion; angular velocity is facade-
/// owned (rotations are locked in the engine — orientation is the
/// view's concern for spheres).
#[derive(Debug, Clone, Copy)]
struct PoweredState {
    control: ControlInput,
    retention: Retention,
    limits: SpeedLimits,
    angular: [f32; 3],
}

/// Placement, hull, and motion envelope for a powered body — one
/// named-field argument object, so registration sites read as what
/// they configure.
#[derive(Debug, Clone, Copy)]
pub struct PoweredBodySpec {
    pub position: [f32; 3],
    pub radius: f32,
    pub mass: Mass,
    pub restitution: Restitution,
    pub retention: Retention,
    pub limits: SpeedLimits,
}

/// A body's complete world-side record. Tombstoned (`alive: false`)
/// on removal so other BodyIds stay stable; tombstoned slots vanish
/// from queries, snapshots, and events.
struct BodySlot {
    mirror: BallisticBody,
    handle: RigidBodyHandle,
    /// `Some` for powered bodies, `None` for ballistic.
    powered: Option<PoweredState>,
    alive: bool,
}

/// Owns all ballistic bodies and the static collision geometry they
/// bounce around in, simulated by rapier3d behind this facade — rapier
/// types never escape this module. Statics come from the same
/// `level_assembly::collision_boxes` output that builds the Godot
/// colliders — one source of truth for where the walls are.
pub struct KineticWorld {
    /// One slot per BodyId, in creation order: a body's engine handle,
    /// read-mirror, control state, and liveness can never disagree.
    slots: Vec<BodySlot>,
    gravity: Vector,
    params: IntegrationParameters,
    pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    rigid_bodies: RigidBodySet,
    colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd: CCDSolver,
    tick: u64,
}

impl Default for KineticWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl KineticWorld {
    pub fn new() -> Self {
        // Tighter contact slop than rapier's default: bodies settle
        // within a tenth of a millimeter of surfaces instead of a full
        // millimeter inside them.
        let params = IntegrationParameters {
            normalized_allowed_linear_error: 1.0e-4,
            ..IntegrationParameters::default()
        };
        Self {
            slots: Vec::new(),
            gravity: Vector::new(0.0, 0.0, 0.0),
            params,
            pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            rigid_bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd: CCDSolver::new(),
            tick: 0,
        }
    }

    pub fn add_statics(&mut self, boxes: impl IntoIterator<Item = CollisionBox>) {
        for slab in boxes {
            let collider = ColliderBuilder::cuboid(
                slab.half_extents[0],
                slab.half_extents[1],
                slab.half_extents[2],
            )
            .translation(Vector::new(slab.position[0], slab.position[1], slab.position[2]))
            .rotation(Vector::new(0.0, slab.rotation_y, 0.0))
            // Neutral surface: with combine-rule Max on bodies, the
            // body's own restitution governs the bounce, exactly as
            // the bespoke solver behaved.
            .restitution(0.0)
            // Modest surface grip: tangential coupling is what makes
            // struck props tumble naturally off walls and hulls.
            .friction(0.3)
            .user_data(STATIC_TAG)
            .build();
            self.colliders.insert(collider);
        }
    }

    pub fn add_body(&mut self, body: BallisticBody) -> BodyId {
        let index = self.slots.len();
        let mut rigid_builder = RigidBodyBuilder::dynamic()
            .translation(Vector::new(
                body.position[0],
                body.position[1],
                body.position[2],
            ))
            .can_sleep(true)
            .ccd_enabled(true);
        if let Some(mass) = body.mass {
            rigid_builder = rigid_builder.additional_mass(mass.as_f32());
        }
        let handle = self.rigid_bodies.insert(rigid_builder.build());
        let collider = ColliderBuilder::ball(body.radius)
            .restitution(body.restitution.coefficient())
            .restitution_combine_rule(CoefficientCombineRule::Max)
            // Bodies grip surfaces: glancing contacts spin props, and
            // hulls scraping along walls bleed speed (wall-grinding
            // isn't free).
            .friction(0.4)
            // Explicit mass wins; otherwise a density-1 sphere.
            .density(if body.mass.is_some() { 0.0 } else { 1.0 })
            .active_events(ActiveEvents::COLLISION_EVENTS)
            .user_data(index as u128)
            .build();
        self.colliders
            .insert_with_parent(collider, handle, &mut self.rigid_bodies);
        self.slots.push(BodySlot {
            mirror: body,
            handle,
            powered: None,
            alive: true,
        });
        BodyId(index)
    }

    /// Add a powered body: a mover with a control slot the shell
    /// writes each tick. Its hull slides on geometry (restitution 0
    /// unless given); its motion envelope (retention, speed caps) is
    /// enforced by the same kinetic core as every other mover.
    pub fn add_powered(&mut self, spec: PoweredBodySpec) -> BodyId {
        let id = self.add_body(
            BallisticBody::at_rest(spec.position, spec.radius, spec.restitution)
                .with_mass(spec.mass),
        );
        self.slots[id.index()].powered = Some(PoweredState {
            control: ControlInput::NONE,
            retention: spec.retention,
            limits: spec.limits,
            angular: [0.0; 3],
        });
        // Rotations are facade-owned for powered bodies; the engine
        // simulates linear motion only. Retention maps to the engine's
        // exponential damping (d = -ln r), so the solver owns velocity
        // end to end — overwriting it per tick would erase contact
        // corrections and grind hulls into walls.
        if let Some(rigid) = self.rigid_bodies.get_mut(self.slots[id.index()].handle) {
            rigid.lock_rotations(true, false);
            rigid.set_linear_damping(-spec.retention.factor().ln());
        }
        id
    }

    /// Write a powered body's control for the coming tick. Writing to
    /// a ballistic or removed body is a no-op by construction.
    pub fn set_control(&mut self, id: BodyId, control: ControlInput) {
        if let Some(state) = self
            .slots
            .get_mut(id.index())
            .and_then(|slot| slot.powered.as_mut())
        {
            state.control = control;
        }
    }

    /// Update a powered body's motion envelope (the ship's retention
    /// changes with Stability upgrades mid-run).
    pub fn set_envelope(&mut self, id: BodyId, retention: Retention, limits: SpeedLimits) {
        let Some(slot) = self.slots.get_mut(id.index()) else {
            return;
        };
        if let Some(state) = slot.powered.as_mut() {
            state.retention = retention;
            state.limits = limits;
            if let Some(rigid) = self.rigid_bodies.get_mut(slot.handle) {
                rigid.set_linear_damping(-retention.factor().ln());
            }
        }
    }

    /// Remove a body (death, despawn). The slot is tombstoned so other
    /// BodyIds stay stable; the body vanishes from queries, snapshots,
    /// and events.
    pub fn remove_body(&mut self, id: BodyId) {
        let Some(slot) = self.slots.get_mut(id.index()) else {
            return;
        };
        if !slot.alive {
            return;
        }
        slot.alive = false;
        slot.powered = None;
        let handle = slot.handle;
        self.rigid_bodies.remove(
            handle,
            &mut self.islands,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            true,
        );
    }

    pub fn body(&self, id: BodyId) -> Option<&BallisticBody> {
        self.slots
            .get(id.0)
            .filter(|slot| slot.alive)
            .map(|slot| &slot.mirror)
    }

    pub fn bodies(&self) -> impl Iterator<Item = (BodyId, &BallisticBody)> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.alive)
            .map(|(i, slot)| (BodyId(i), &slot.mirror))
    }

    /// One-shot impulse: level-setup disturbance ("the occupants
    /// knocked it loose") or collision response from a powered mover.
    /// Velocity-delta semantics (mass-independent), matching the
    /// kinetic core's `Impulse` contract.
    pub fn disturb(&mut self, id: BodyId, impulse: Impulse) {
        let Some(slot) = self.slots.get(id.0).filter(|slot| slot.alive) else {
            return;
        };
        if let Some(rigid) = self.rigid_bodies.get_mut(slot.handle) {
            let lv = rigid.linvel()
                + Vector::new(impulse.linear[0], impulse.linear[1], impulse.linear[2]);
            let av = rigid.angvel()
                + Vector::new(impulse.angular[0], impulse.angular[1], impulse.angular[2]);
            rigid.set_linvel(lv, true);
            rigid.set_angvel(av, true);
        }
        self.sync_mirror(id.0);
    }

    /// Publish the landing-zone snapshot of the current tick.
    pub fn snapshot(&self) -> WorldSnapshot {
        WorldSnapshot {
            tick: self.tick,
            bodies: self
                .slots
                .iter()
                .enumerate()
                .filter(|(_, slot)| slot.alive)
                .map(|(i, slot)| BodySnapshot {
                    id: BodyId(i),
                    position: slot.mirror.position,
                    angular_velocity: slot.mirror.angular_velocity,
                    at_rest: slot.mirror.is_at_rest(),
                })
                .collect(),
        }
    }

    /// True when the segment from `from` to `to` hits any collider —
    /// the world's own line-of-sight answer (walls and drifting props
    /// both block sight). `exclude` removes bodies from consideration
    /// (a looker's ray starts inside its own collider). The spatial
    /// index builds during `step`, so queries reflect the world as of
    /// the most recent tick.
    pub fn ray_blocked(&self, from: [f32; 3], to: [f32; 3], exclude: &[BodyId]) -> bool {
        let dir = sub(to, from);
        let length = dot(dir, dir).sqrt();
        if length <= f32::EPSILON {
            return false;
        }
        let ray = Ray::new(
            Vector::new(from[0], from[1], from[2]),
            Vector::new(dir[0] / length, dir[1] / length, dir[2] / length),
        );
        // QueryFilter::exclude_rigid_body holds a single handle, so
        // multi-exclusion (looker AND target) needs the predicate.
        let excluded: Vec<RigidBodyHandle> = exclude
            .iter()
            .filter_map(|id| self.slots.get(id.index()).map(|slot| slot.handle))
            .collect();
        let keep = |_collider: ColliderHandle, collider: &Collider| -> bool {
            collider
                .parent()
                .is_none_or(|parent| !excluded.contains(&parent))
        };
        let filter = QueryFilter::default().predicate(&keep);
        self.broad_phase
            .as_query_pipeline(
                self.narrow_phase.query_dispatcher(),
                &self.rigid_bodies,
                &self.colliders,
                filter,
            )
            .cast_ray(&ray, length, true)
            .is_some()
    }

    /// Advance every body one tick through the physics engine, then
    /// reassert the facade's invariants: speed caps, NaN recovery, and
    /// exact-rest capture. Returns the contacts that began this tick.
    pub fn step(&mut self, delta: f32) -> Vec<ContactEvent> {
        self.params.dt = delta;

        // Apply each powered body's control: thrust as a solver-
        // friendly impulse (the engine owns linear velocity end to
        // end; damping carries retention), torque integrated facade-
        // side with the exact per-second math (rotation feel).
        for slot in &mut self.slots {
            if !slot.alive {
                continue;
            }
            let Some(state) = slot.powered.as_mut() else {
                continue;
            };
            let Some(rigid) = self.rigid_bodies.get_mut(slot.handle) else {
                continue;
            };
            if state.control.thrust != [0.0; 3] {
                let momentum = scale(state.control.thrust, delta * rigid.mass());
                rigid.apply_impulse(Vector::new(momentum[0], momentum[1], momentum[2]), true);
            }
            state.angular = sanitize(
                scale(
                    add(state.angular, scale(state.control.torque, delta)),
                    state.retention.factor_for(delta),
                ),
                state.limits.angular,
            );
        }

        let collector = ContactCollector::default();
        self.pipeline.step(
            self.gravity,
            &self.params,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd,
            &(),
            &collector,
        );

        for index in 0..self.slots.len() {
            if !self.slots[index].alive {
                continue;
            }
            self.assert_invariants(index);
            self.sync_mirror(index);
        }
        self.tick += 1;

        collector.events.into_inner().unwrap_or_default()
    }

    /// Facade invariants applied after the engine step: NaN recovery,
    /// speed caps, and exact-rest capture. Bodies already at exact rest
    /// are left untouched (rapier's own sleeping owns them); forcing
    /// `sleep()` by hand would bypass the island manager.
    fn assert_invariants(&mut self, index: usize) {
        let limits = self.slots[index]
            .powered
            .as_ref()
            .map(|state| state.limits)
            .unwrap_or(SpeedLimits {
                linear: BALLISTIC_MAX_SPEED,
                angular: BALLISTIC_MAX_SPIN,
            });
        let handle = self.slots[index].handle;
        let Some(rigid) = self.rigid_bodies.get_mut(handle) else {
            return;
        };
        let lv = rigid.linvel();
        let av = rigid.angvel();
        if lv == Vector::ZERO && av == Vector::ZERO {
            return;
        }
        let linear = sanitize([lv.x, lv.y, lv.z], limits.linear);
        let angular = sanitize([av.x, av.y, av.z], limits.angular);
        if dot(linear, linear).sqrt() < REST_SPEED {
            rigid.set_linvel(Vector::ZERO, false);
            rigid.set_angvel(Vector::ZERO, false);
            if let Some(state) = self.slots[index].powered.as_mut() {
                state.angular = [0.0; 3];
            }
        } else {
            rigid.set_linvel(Vector::new(linear[0], linear[1], linear[2]), false);
            rigid.set_angvel(Vector::new(angular[0], angular[1], angular[2]), false);
        }
    }

    /// Copy engine state into the read-mirror so callers can keep
    /// borrowing `&BallisticBody`.
    fn sync_mirror(&mut self, index: usize) {
        let Some(rigid) = self.rigid_bodies.get(self.slots[index].handle) else {
            return;
        };
        let t = rigid.translation();
        let lv = rigid.linvel();
        let av = rigid.angvel();
        let slot = &mut self.slots[index];
        slot.mirror.position = [t.x, t.y, t.z];
        slot.mirror.linear_velocity = [lv.x, lv.y, lv.z];
        // Powered bodies have engine rotations locked; their angular
        // velocity is facade-owned.
        slot.mirror.angular_velocity = match slot.powered.as_ref() {
            Some(state) => state.angular,
            None => [av.x, av.y, av.z],
        };
    }
}

/// EventHandler that translates rapier collision-start events into
/// facade `ContactEvent`s. Identity flows through collider user-data
/// (body index, or `STATIC_TAG` for level geometry).
#[derive(Default)]
struct ContactCollector {
    events: std::sync::Mutex<Vec<ContactEvent>>,
}

impl EventHandler for ContactCollector {
    fn handle_collision_event(
        &self,
        bodies: &RigidBodySet,
        colliders: &ColliderSet,
        event: CollisionEvent,
        contact_pair: Option<&ContactPair>,
    ) {
        let CollisionEvent::Started(h1, h2, _) = event else {
            return;
        };
        let (Some(c1), Some(c2)) = (colliders.get(h1), colliders.get(h2)) else {
            return;
        };
        let u1 = c1.user_data;
        let u2 = c2.user_data;
        // The reported `body` is always ballistic; when both are, the
        // lower index reports (one event per pair).
        let (body_index, with, body_is_first) = match (u1 != STATIC_TAG, u2 != STATIC_TAG) {
            (true, true) => {
                if u1 <= u2 {
                    (u1 as usize, ContactWith::Body(BodyId(u2 as usize)), true)
                } else {
                    (u2 as usize, ContactWith::Body(BodyId(u1 as usize)), false)
                }
            }
            (true, false) => (u1 as usize, ContactWith::Static, true),
            (false, true) => (u2 as usize, ContactWith::Static, false),
            (false, false) => return,
        };

        let velocity_of = |collider: &Collider| -> [f32; 3] {
            collider
                .parent()
                .and_then(|h| bodies.get(h))
                .map(|rb| {
                    let v = rb.linvel();
                    [v.x, v.y, v.z]
                })
                .unwrap_or([0.0; 3])
        };

        // Normal and contact point from the manifold; rapier's manifold
        // normal points from the first collider toward the second.
        let mut normal = [0.0_f32; 3];
        let mut position = {
            let t = if body_is_first {
                c1.translation()
            } else {
                c2.translation()
            };
            [t.x, t.y, t.z]
        };
        if let Some(pair) = contact_pair {
            if let Some(manifold) = pair.manifolds.first() {
                let n = manifold.data.normal;
                normal = if body_is_first {
                    [n.x, n.y, n.z]
                } else {
                    [-n.x, -n.y, -n.z]
                };
                if let Some(point) = manifold.points.first() {
                    let world = c1.position().transform_point(point.local_p1);
                    position = [world.x, world.y, world.z];
                }
            }
        }

        let relative = sub(velocity_of(c1), velocity_of(c2));
        let impact_speed = dot(relative, normal).abs();

        if let Ok(mut events) = self.events.lock() {
            events.push(ContactEvent {
                body: BodyId(body_index),
                with,
                normal,
                impact_speed,
                position,
            });
        }
    }

    fn handle_contact_force_event(
        &self,
        _dt: Real,
        _bodies: &RigidBodySet,
        _colliders: &ColliderSet,
        _contact_pair: &ContactPair,
        _total_force_magnitude: Real,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinetics::dot;

    fn speed(v: [f32; 3]) -> f32 {
        dot(v, v).sqrt()
    }

    const DT: f32 = 1.0 / 60.0;

    fn linear(v: [f32; 3]) -> Impulse {
        Impulse {
            linear: v,
            angular: [0.0; 3],
        }
    }

    fn wall(position: [f32; 3], half_extents: [f32; 3]) -> CollisionBox {
        CollisionBox {
            position,
            half_extents,
            rotation_y: 0.0,
        }
    }

    /// Six walls enclosing an axis-aligned room centered on the origin.
    fn sealed_room(half: f32) -> Vec<CollisionBox> {
        let h = half;
        let t = 0.5; // wall thickness (half)
        vec![
            wall([h + t, 0.0, 0.0], [t, h + 1.0, h + 1.0]),
            wall([-h - t, 0.0, 0.0], [t, h + 1.0, h + 1.0]),
            wall([0.0, h + t, 0.0], [h + 1.0, t, h + 1.0]),
            wall([0.0, -h - t, 0.0], [h + 1.0, t, h + 1.0]),
            wall([0.0, 0.0, h + t], [h + 1.0, h + 1.0, t]),
            wall([0.0, 0.0, -h - t], [h + 1.0, h + 1.0, t]),
        ]
    }

    #[test]
    fn a_body_at_rest_stays_exactly_at_rest() {
        let mut world = KineticWorld::new();
        world.add_statics(sealed_room(4.0));
        let id = world.add_body(BallisticBody::at_rest(
            [1.0, 2.0, 3.0],
            0.5,
            Restitution::clamped(0.5),
        ));
        for _ in 0..600 {
            world.step(DT);
        }
        let body = world.body(id).unwrap();
        assert_eq!(body.position(), [1.0, 2.0, 3.0], "rest is the fixed point");
        assert!(body.is_at_rest());
    }

    #[test]
    fn a_disturbed_body_drifts() {
        let mut world = KineticWorld::new();
        let id = world.add_body(BallisticBody::at_rest(
            [0.0; 3],
            0.5,
            Restitution::clamped(0.5),
        ));
        world.disturb(id, linear([1.0, 0.0, 0.0]));
        world.step(DT);
        assert!(world.body(id).unwrap().position()[0] > 0.0);
    }

    #[test]
    fn a_body_bounces_off_a_wall_instead_of_crossing_it() {
        let mut world = KineticWorld::new();
        // Wall slab: front face at z = -1.5.
        world.add_statics([wall([0.0, 0.0, -2.0], [5.0, 5.0, 0.5])]);
        let id = world.add_body(BallisticBody::at_rest(
            [0.0; 3],
            0.5,
            Restitution::clamped(0.8),
        ));
        world.disturb(id, linear([0.0, 0.0, -5.0]));
        for _ in 0..120 {
            world.step(DT);
            let z = world.body(id).unwrap().position()[2];
            assert!(
                z > -1.6,
                "body crossed into the wall: center z = {z} (face at -1.5)"
            );
        }
        let v = world.body(id).unwrap().linear_velocity();
        assert!(v[2] > 0.0, "velocity must reflect off the wall, got {v:?}");
    }

    #[test]
    fn a_body_overlapping_a_wall_is_pushed_out() {
        let mut world = KineticWorld::new();
        world.add_statics([wall([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])]);
        let id = world.add_body(BallisticBody::at_rest(
            [0.0, 0.0, 0.9],
            0.5,
            Restitution::clamped(0.5),
        ));
        // Nudge it so it isn't skipped as at-rest.
        world.disturb(id, linear([0.0, 0.0, -0.1]));
        for _ in 0..60 {
            world.step(DT);
        }
        let p = world.body(id).unwrap().position();
        assert!(
            p[2] >= 1.5 - 1e-3,
            "body must be expelled to the surface (z >= 1.5), got {p:?}"
        );
        assert!(p[0].is_finite() && p[1].is_finite() && p[2].is_finite());
    }

    #[test]
    fn a_disturbed_body_in_a_sealed_room_settles_back_to_rest() {
        let mut world = KineticWorld::new();
        world.add_statics(sealed_room(2.0));
        let id = world.add_body(BallisticBody::at_rest(
            [0.0; 3],
            0.5,
            Restitution::clamped(0.2),
        ));
        world.disturb(id, linear([0.8, 0.6, 0.7]));
        // 60 simulated seconds: lossy bounces + rest capture must
        // return the room to equilibrium.
        for _ in 0..3600 {
            world.step(DT);
            let p = world.body(id).unwrap().position();
            for axis in p {
                assert!(
                    axis.abs() < 2.6,
                    "body escaped the sealed room: {p:?}"
                );
            }
        }
        assert!(
            world.body(id).unwrap().is_at_rest(),
            "lossy bounces must eventually capture the body at rest, velocity {:?}",
            world.body(id).unwrap().linear_velocity()
        );
    }

    #[test]
    fn touching_bodies_exchange_momentum() {
        let mut world = KineticWorld::new();
        let a = world.add_body(BallisticBody::at_rest(
            [-1.0, 0.0, 0.0],
            0.5,
            Restitution::clamped(0.9),
        ));
        let b = world.add_body(BallisticBody::at_rest(
            [1.0, 0.0, 0.0],
            0.5,
            Restitution::clamped(0.9),
        ));
        world.disturb(a, linear([2.0, 0.0, 0.0]));
        for _ in 0..120 {
            world.step(DT);
        }
        let va = world.body(a).unwrap().linear_velocity();
        let vb = world.body(b).unwrap().linear_velocity();
        assert!(
            vb[0] > 0.5,
            "struck body must carry momentum away, got {vb:?}"
        );
        assert!(
            va[0] < 1.0,
            "striking body must lose momentum, got {va:?}"
        );
        // Equal masses: momentum is conserved up to rest-capture.
        let total = va[0] + vb[0];
        assert!(
            (total - 2.0).abs() < 0.3,
            "momentum must be approximately conserved, got {total}"
        );
    }

    #[test]
    fn rotated_walls_collide_in_world_space() {
        let mut world = KineticWorld::new();
        // A slab that only blocks x ≈ ±3 once rotated 90° about Y:
        // local half extents [0.5, 2, 3] → world extents [3, 2, 0.5].
        world.add_statics([CollisionBox {
            position: [0.0; 3],
            half_extents: [0.5, 2.0, 3.0],
            rotation_y: std::f32::consts::FRAC_PI_2,
        }]);
        let id = world.add_body(BallisticBody::at_rest(
            [5.0, 0.0, 0.0],
            0.5,
            Restitution::clamped(0.8),
        ));
        world.disturb(id, linear([-4.0, 0.0, 0.0]));
        for _ in 0..120 {
            world.step(DT);
            let x = world.body(id).unwrap().position()[0];
            assert!(
                x > 2.9,
                "body penetrated the rotated slab: center x = {x} (face at 3.0)"
            );
        }
        assert!(
            world.body(id).unwrap().linear_velocity()[0] > 0.0,
            "velocity must reflect off the rotated face"
        );
    }

    #[test]
    fn glancing_strikes_tumble_props_via_friction() {
        use crate::kinetics::Mass;
        let (retention, limits) = test_envelope();
        let mut world = KineticWorld::new();
        let mover = world.add_powered(PoweredBodySpec {
            position: [-2.0, 0.3, 0.0],
            radius: 0.5,
            mass: Mass::kilograms(60.0),
            restitution: Restitution::clamped(0.0),
            retention,
            limits,
        });
        let prop = world.add_body(BallisticBody::at_rest(
            [1.0, 0.0, 0.0],
            0.5,
            Restitution::clamped(0.6),
        ));
        // Coast into the prop off-center: surface friction converts
        // the tangential slip at the contact into spin.
        world.disturb(mover, linear([20.0, 0.0, 0.0]));
        for _ in 0..120 {
            world.step(DT);
        }
        let body = world.body(prop).unwrap();
        assert!(
            !body.is_at_rest(),
            "the glancing strike must launch the prop"
        );
        let spin = body.angular_velocity();
        assert!(
            dot(spin, spin).sqrt() > 0.1,
            "friction at the contact must set the prop tumbling, got {spin:?}"
        );
    }

    #[test]
    fn a_heavy_body_shrugs_off_a_light_striker() {
        use crate::kinetics::Mass;
        let mut world = KineticWorld::new();
        let light = world.add_body(BallisticBody::at_rest(
            [-1.5, 0.0, 0.0],
            0.5,
            Restitution::clamped(0.9),
        ));
        // ~20x the striker's density-1 sphere mass (~0.52 kg). Much
        // heavier and the post-impact drift falls below REST_SPEED and
        // is (correctly) captured at rest by micro-settling.
        let heavy = world.add_body(
            BallisticBody::at_rest([0.5, 0.0, 0.0], 0.5, Restitution::clamped(0.9))
                .with_mass(Mass::kilograms(10.0)),
        );
        world.disturb(light, linear([3.0, 0.0, 0.0]));
        for _ in 0..120 {
            world.step(DT);
        }
        let heavy_v = world.body(heavy).unwrap().linear_velocity()[0];
        let light_v = world.body(light).unwrap().linear_velocity()[0];
        assert!(
            heavy_v > 0.01,
            "the heavy body must still be moved a little, got {heavy_v}"
        );
        assert!(
            heavy_v < 0.6,
            "momentum-weighted exchange: a much heavier body must barely \
             deflect, got {heavy_v}"
        );
        assert!(
            light_v < 0.0,
            "the light striker must rebound off the heavy body, got {light_v}"
        );
    }

    #[test]
    fn collisions_emit_one_onset_event_per_contact() {
        let mut world = KineticWorld::new();
        let a = world.add_body(BallisticBody::at_rest(
            [-1.5, 0.0, 0.0],
            0.5,
            Restitution::clamped(0.9),
        ));
        let _b = world.add_body(BallisticBody::at_rest(
            [0.5, 0.0, 0.0],
            0.5,
            Restitution::clamped(0.9),
        ));
        world.disturb(a, linear([3.0, 0.0, 0.0]));
        let mut contacts: Vec<ContactEvent> = Vec::new();
        for _ in 0..120 {
            contacts.extend(world.step(DT));
        }
        let body_contacts: Vec<_> = contacts
            .iter()
            .filter(|c| matches!(c.with, ContactWith::Body(_)))
            .collect();
        assert_eq!(
            body_contacts.len(),
            1,
            "one continuous contact must emit exactly one onset event, got {body_contacts:?}"
        );
        let hit = body_contacts[0];
        assert!(
            hit.impact_speed > 1.0,
            "closing speed ~3 m/s must be reported, got {}",
            hit.impact_speed
        );
        assert!(
            hit.normal[0].abs() > 0.9,
            "head-on x impact must have an x-dominant normal, got {:?}",
            hit.normal
        );
    }

    #[test]
    fn wall_impacts_emit_static_contact_events() {
        let mut world = KineticWorld::new();
        world.add_statics([wall([0.0, 0.0, -2.0], [5.0, 5.0, 0.5])]);
        let id = world.add_body(BallisticBody::at_rest(
            [0.0; 3],
            0.5,
            Restitution::clamped(0.8),
        ));
        world.disturb(id, linear([0.0, 0.0, -4.0]));
        let mut static_hits = 0;
        for _ in 0..60 {
            for contact in world.step(DT) {
                if contact.with == ContactWith::Static {
                    static_hits += 1;
                    assert_eq!(contact.body, id);
                }
            }
        }
        assert_eq!(
            static_hits, 1,
            "one wall bounce must emit exactly one static onset event"
        );
    }

    #[test]
    fn resting_bodies_emit_no_events() {
        let mut world = KineticWorld::new();
        world.add_statics(sealed_room(4.0));
        world.add_body(BallisticBody::at_rest(
            [0.0; 3],
            0.5,
            Restitution::clamped(0.5),
        ));
        let mut total = 0;
        for _ in 0..120 {
            total += world.step(DT).len();
        }
        assert_eq!(total, 0, "an undisturbed room is silent");
    }

    fn test_envelope() -> (Retention, SpeedLimits) {
        (
            Retention::decaying(0.046),
            SpeedLimits {
                linear: 50.0,
                angular: 5.0,
            },
        )
    }

    #[test]
    fn cruise_thrust_sustains_the_target_speed() {
        use crate::kinetics::Mass;
        let (retention, limits) = test_envelope();
        let mut world = KineticWorld::new();
        let id = world.add_powered(PoweredBodySpec {
            position: [0.0; 3],
            radius: 0.5,
            mass: Mass::kilograms(20.0),
            restitution: Restitution::clamped(0.2),
            retention,
            limits,
        });
        world.set_control(
            id,
            ControlInput {
                thrust: [retention.cruise_thrust(4.0), 0.0, 0.0],
                torque: [0.0; 3],
            },
        );
        for _ in 0..360 {
            world.step(DT);
        }
        let v = world.body(id).unwrap().linear_velocity()[0];
        assert!(
            (v - 4.0).abs() < 0.3,
            "cruise_thrust(4.0) must settle near 4 m/s, got {v}"
        );
    }

    #[test]
    fn powered_bodies_accelerate_under_control_and_dissipate_without_it() {
        use crate::kinetics::Mass;
        let (retention, limits) = test_envelope();
        let mut world = KineticWorld::new();
        let id = world.add_powered(PoweredBodySpec {
            position: [0.0; 3],
            radius: 0.5,
            mass: Mass::kilograms(60.0),
            restitution: Restitution::clamped(0.0),
            retention,
            limits,
        });
        world.set_control(
            id,
            ControlInput {
                thrust: [40.0, 0.0, 0.0],
                torque: [0.0; 3],
            },
        );
        for _ in 0..120 {
            world.step(DT);
        }
        let body = world.body(id).unwrap();
        assert!(
            body.linear_velocity()[0] > 5.0,
            "thrust must accelerate a powered body, got {:?}",
            body.linear_velocity()
        );
        assert!(body.position()[0] > 1.0, "it must actually travel");

        world.set_control(id, ControlInput::NONE);
        for _ in 0..600 {
            world.step(DT);
        }
        assert!(
            world.body(id).unwrap().is_at_rest(),
            "without control, the envelope's retention must bring it to rest, got {:?}",
            world.body(id).unwrap().linear_velocity()
        );
    }

    #[test]
    fn powered_hulls_slide_along_walls_without_bouncing() {
        use crate::kinetics::Mass;
        let (retention, limits) = test_envelope();
        let mut world = KineticWorld::new();
        // Wall face at z = -1.5, long enough in x that two seconds of
        // sliding never reaches its edge; a restitution-0 hull
        // thrusting diagonally into it must press and slide, never
        // bounce.
        world.add_statics([wall([0.0, 0.0, -2.0], [50.0, 5.0, 0.5])]);
        let id = world.add_powered(PoweredBodySpec {
            position: [0.0; 3],
            radius: 0.5,
            mass: Mass::kilograms(60.0),
            restitution: Restitution::clamped(0.0),
            retention,
            limits,
        });
        world.set_control(
            id,
            ControlInput {
                thrust: [10.0, 0.0, -20.0],
                torque: [0.0; 3],
            },
        );
        for _ in 0..240 {
            world.step(DT);
            let p = world.body(id).unwrap().position();
            // Impact-tick transients press a few cm before the solver's
            // first correction; tunneling is what must never happen.
            assert!(
                p[2] > -1.2,
                "hull tunneled into the wall, center z = {}",
                p[2]
            );
        }
        let body = world.body(id).unwrap();
        let p = body.position();
        let v = body.linear_velocity();
        assert!(
            p[2] > -1.02,
            "the hull must settle non-penetrating, z = {}",
            p[2]
        );
        assert!(
            p[2] < -0.9,
            "the hull must stay pressed against the wall (slide, not bounce), z = {}",
            p[2]
        );
        assert!(
            v[0] > 0.5,
            "the tangential component must keep flowing (reduced by \
             honest friction drag while pressing), got {v:?}"
        );
        assert!(
            v[2].abs() < 0.2,
            "no rebound off a restitution-0 contact, got {v:?}"
        );
    }

    #[test]
    fn props_push_powered_movers_back() {
        use crate::kinetics::Mass;
        let (retention, limits) = test_envelope();
        let mut world = KineticWorld::new();
        let mover = world.add_powered(PoweredBodySpec {
            position: [-2.0, 0.0, 0.0],
            radius: 0.5,
            mass: Mass::kilograms(60.0),
            restitution: Restitution::clamped(0.0),
            retention,
            limits,
        });
        let prop = world.add_body(
            BallisticBody::at_rest([2.0, 0.0, 0.0], 0.5, Restitution::clamped(0.6))
                .with_mass(Mass::kilograms(60.0)),
        );
        // Coast into the prop (no thrust), so the exchange is
        // observable instead of forming a thrust-driven convoy. 20 m/s
        // outruns the retention decay over the 3 m gap.
        world.disturb(mover, linear([20.0, 0.0, 0.0]));
        let mut hit_each_other = false;
        for _ in 0..120 {
            for contact in world.step(DT) {
                if contact.body == mover && contact.with == ContactWith::Body(prop) {
                    hit_each_other = true;
                }
            }
        }
        assert!(hit_each_other, "the contact event must name the pair");
        let prop_v = world.body(prop).unwrap().linear_velocity()[0];
        let mover_v = world.body(mover).unwrap().linear_velocity()[0];
        assert!(
            prop_v > 2.0,
            "an equal-mass prop must carry momentum away, got {prop_v}"
        );
        assert!(
            mover_v < prop_v,
            "third law: the striking mover must be slowed by the prop \
             (mover {mover_v} vs prop {prop_v})"
        );
    }

    #[test]
    fn gentle_thrust_from_rest_still_moves() {
        // Rest capture is a ballistic settling rule; a powered body
        // holding ANY thrust must never be frozen by it (attacking
        // slimes, gentle analog input).
        use crate::kinetics::Mass;
        let (retention, limits) = test_envelope();
        let mut world = KineticWorld::new();
        let id = world.add_powered(PoweredBodySpec {
            position: [0.0; 3],
            radius: 0.5,
            mass: Mass::kilograms(60.0),
            restitution: Restitution::clamped(0.0),
            retention,
            limits,
        });
        world.set_control(
            id,
            ControlInput {
                thrust: [5.0, 0.0, 0.0],
                torque: [0.0; 3],
            },
        );
        for _ in 0..240 {
            world.step(DT);
        }
        let body = world.body(id).unwrap();
        assert!(
            body.position()[0] > 0.5,
            "5 m/s² of held thrust must move a body from rest, got {:?} at {:?}",
            body.linear_velocity(),
            body.position()
        );
    }

    #[test]
    fn rays_to_an_excluded_target_body_are_not_blocked() {
        // The LOS calling convention: a looker raycasts to a target's
        // CENTER — both bodies must be excluded or the target's own
        // collider blocks every sight line.
        let mut world = KineticWorld::new();
        let looker = world.add_body(BallisticBody::at_rest(
            [0.0; 3],
            0.5,
            Restitution::clamped(0.5),
        ));
        let target = world.add_body(BallisticBody::at_rest(
            [4.0, 0.0, 0.0],
            0.5,
            Restitution::clamped(0.5),
        ));
        world.step(DT);
        assert!(
            world.ray_blocked([0.0; 3], [4.0, 0.0, 0.0], &[looker]),
            "excluding only the looker: the target's own collider blocks"
        );
        assert!(
            !world.ray_blocked([0.0; 3], [4.0, 0.0, 0.0], &[looker, target]),
            "excluding both endpoints: clear space must be clear"
        );
    }

    #[test]
    fn survivor_snapshots_are_unaffected_by_removals() {
        // Snapshots compact on removal; survivors keep their ids and
        // state, and consumers must correlate by id, never by index.
        let mut world = KineticWorld::new();
        let doomed = world.add_body(BallisticBody::at_rest(
            [0.0; 3],
            0.5,
            Restitution::clamped(0.5),
        ));
        let survivor = world.add_body(BallisticBody::at_rest(
            [5.0, 0.0, 0.0],
            0.5,
            Restitution::clamped(0.5),
        ));
        world.disturb(survivor, linear([1.0, 0.0, 0.0]));
        world.step(DT);
        world.remove_body(doomed);
        let snapshot = world.snapshot();
        assert_eq!(snapshot.bodies.len(), 1);
        let entry = snapshot.bodies[0];
        assert_eq!(entry.id, survivor, "the survivor keeps its id");
        assert!(
            entry.position[0] > 5.0,
            "the survivor keeps its own state, got {:?}",
            entry.position
        );
        assert_ne!(
            entry.id.index(),
            0,
            "compaction means index-into-bodies != id — consumers must key by id"
        );
    }

    #[test]
    fn removed_bodies_vanish_from_snapshots_and_queries() {
        let mut world = KineticWorld::new();
        let id = world.add_body(BallisticBody::at_rest(
            [0.0; 3],
            0.5,
            Restitution::clamped(0.5),
        ));
        world.disturb(id, linear([1.0, 0.0, 0.0]));
        world.remove_body(id);
        assert!(world.body(id).is_none(), "a removed body answers no queries");
        let contacts = world.step(DT);
        assert!(contacts.is_empty());
        assert!(
            world.snapshot().bodies.is_empty(),
            "a removed body never appears in the landing zone"
        );
    }

    #[test]
    fn snapshots_publish_the_world_each_tick() {
        let mut world = KineticWorld::new();
        let id = world.add_body(BallisticBody::at_rest(
            [1.0, 2.0, 3.0],
            0.5,
            Restitution::clamped(0.5),
        ));
        world.disturb(id, linear([3.0, 0.0, 0.0]));

        let before = world.snapshot();
        world.step(DT);
        let after = world.snapshot();

        assert_eq!(
            after.tick,
            before.tick + 1,
            "each step publishes the next tick"
        );
        assert_eq!(after.bodies.len(), 1);
        let body = after.bodies[0];
        assert_eq!(body.id, id);
        assert!(
            body.position[0] > before.bodies[0].position[0],
            "the snapshot must reflect the moved body: {:?} -> {:?}",
            before.bodies[0].position,
            body.position
        );
        assert!(!body.at_rest);
    }

    #[test]
    fn rays_are_blocked_by_walls_and_clear_in_space() {
        let mut world = KineticWorld::new();
        world.add_statics([wall([0.0, 0.0, -2.0], [5.0, 5.0, 0.5])]);
        // The spatial index builds during step; the game always ticks
        // before querying.
        world.step(DT);
        assert!(
            world.ray_blocked([0.0, 0.0, 0.0], [0.0, 0.0, -6.0], &[]),
            "a ray through a wall must be blocked"
        );
        assert!(
            !world.ray_blocked([0.0, 0.0, 0.0], [0.0, 6.0, 0.0], &[]),
            "a ray through empty space must be clear"
        );
    }

    #[test]
    fn slow_bodies_are_captured_at_exact_rest() {
        let mut world = KineticWorld::new();
        world.add_statics([wall([0.0, 0.0, -2.0], [5.0, 5.0, 0.5])]);
        let id = world.add_body(BallisticBody::at_rest(
            [0.0, 0.0, -0.9],
            0.5,
            Restitution::clamped(0.1),
        ));
        world.disturb(id, linear([0.0, 0.0, -0.5]));
        for _ in 0..600 {
            world.step(DT);
        }
        assert!(
            world.body(id).unwrap().is_at_rest(),
            "a nearly-dead bounce must capture at exact rest, velocity {:?}",
            world.body(id).unwrap().linear_velocity()
        );
        assert!(speed(world.body(id).unwrap().linear_velocity()) == 0.0);
    }
}
