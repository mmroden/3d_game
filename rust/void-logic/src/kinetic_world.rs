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
    add, bounce, cross, dot, scale, sub, ControlInput, Impulse, KineticState, Restitution,
    Retention, SpeedLimits,
};
use crate::room_assembler::CollisionBox;

/// Speed caps for ballistic bodies. The modest linear cap also keeps a
/// body from crossing a wall collider in a single tick.
const BALLISTIC_LIMITS: SpeedLimits = SpeedLimits {
    linear: 20.0,
    angular: 5.0,
};

/// Below this speed, a moving body is captured at exact rest —
/// micro-settling, so disturbed rooms return to equilibrium and the
/// at-rest fast path re-arms.
const REST_SPEED: f32 = 0.075;

/// How much of a glancing strike's tangential motion becomes tumble.
const TUMBLE_FACTOR: f32 = 0.4;

/// A powered mover mirrored into the world for one tick: the world
/// reads its motion to shove ballistic bodies aside; the mover itself
/// is never simulated here (the engine owns its collision).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mover {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub radius: f32,
}

/// Handle to a body in the world. The index is stable for the lifetime
/// of the world (bodies are never removed mid-level).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyId(usize);

impl BodyId {
    pub fn index(self) -> usize {
        self.0
    }
}

/// A mover with no propulsion: a drifting bookshelf, a chunk of
/// wreckage, a physical projectile. There is deliberately no way to
/// thrust one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BallisticBody {
    position: [f32; 3],
    radius: f32,
    restitution: Restitution,
    kinetics: KineticState,
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
            kinetics: KineticState::new(),
        }
    }

    pub fn position(&self) -> [f32; 3] {
        self.position
    }

    pub fn radius(&self) -> f32 {
        self.radius
    }

    pub fn linear_velocity(&self) -> [f32; 3] {
        self.kinetics.linear_velocity()
    }

    /// Cosmetic tumble for the view; ballistic colliders are spheres,
    /// so orientation never affects physics.
    pub fn angular_velocity(&self) -> [f32; 3] {
        self.kinetics.angular_velocity()
    }

    pub fn is_at_rest(&self) -> bool {
        self.kinetics.linear_velocity() == [0.0; 3] && self.kinetics.angular_velocity() == [0.0; 3]
    }
}

/// Owns all ballistic bodies and the static collision geometry they
/// bounce around in. Statics come from the same
/// `level_assembly::collision_boxes` output that builds the Godot
/// colliders — one source of truth for where the walls are.
#[derive(Debug, Default)]
pub struct KineticWorld {
    bodies: Vec<BallisticBody>,
    statics: Vec<CollisionBox>,
}

impl KineticWorld {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_statics(&mut self, boxes: impl IntoIterator<Item = CollisionBox>) {
        self.statics.extend(boxes);
    }

    pub fn add_body(&mut self, body: BallisticBody) -> BodyId {
        self.bodies.push(body);
        BodyId(self.bodies.len() - 1)
    }

    pub fn body(&self, id: BodyId) -> Option<&BallisticBody> {
        self.bodies.get(id.0)
    }

    pub fn bodies(&self) -> impl Iterator<Item = (BodyId, &BallisticBody)> {
        self.bodies.iter().enumerate().map(|(i, b)| (BodyId(i), b))
    }

    /// One-shot impulse: level-setup disturbance ("the occupants
    /// knocked it loose") or collision response from a powered mover.
    pub fn disturb(&mut self, id: BodyId, impulse: Impulse) {
        if let Some(body) = self.bodies.get_mut(id.0) {
            body.kinetics.apply_impulse(impulse);
        }
    }

    /// Advance every body one tick: integrate, collide against the
    /// statics, bounce, exchange momentum between touching bodies,
    /// capture near-rest bodies at exact rest.
    pub fn step(&mut self, delta: f32) {
        for body in self.bodies.iter_mut() {
            if body.is_at_rest() {
                continue;
            }
            body.kinetics
                .step(ControlInput::NONE, Retention::FULL, BALLISTIC_LIMITS, delta);
            body.position = add(body.position, scale(body.kinetics.linear_velocity(), delta));

            for slab in self.statics.iter() {
                if let Some(contact) = sphere_box_penetration(body.position, body.radius, slab) {
                    body.position = add(body.position, contact.push);
                    let reflected =
                        bounce(body.kinetics.linear_velocity(), contact.normal, body.restitution);
                    body.kinetics.accept_resolved_linear(reflected);
                }
            }

            let v = body.kinetics.linear_velocity();
            if dot(v, v).sqrt() < REST_SPEED {
                body.kinetics.accept_resolved_linear([0.0; 3]);
                body.kinetics.halt_rotation();
            }
        }

        self.exchange_momentum();
    }

    /// Shove ballistic bodies out of the way of powered movers
    /// (player, enemies), exchanging momentum in the mover's frame.
    /// Movers are treated as infinite mass: props never push back here
    /// — the engine resolves the mover's side of the contact.
    pub fn collide_movers(&mut self, movers: &[Mover]) {
        for mover in movers {
            for body in self.bodies.iter_mut() {
                let between = sub(body.position, mover.position);
                let dist_sq = dot(between, between);
                let reach = body.radius + mover.radius;
                if dist_sq >= reach * reach || dist_sq == 0.0 {
                    continue;
                }
                let dist = dist_sq.sqrt();
                let normal = scale(between, 1.0 / dist);
                body.position = add(body.position, scale(normal, reach - dist));

                // Momentum exchange in the mover's frame: a separating
                // contact (bounce returns its input) injects nothing.
                let relative = sub(body.linear_velocity(), mover.velocity);
                let reflected = bounce(relative, normal, body.restitution);
                if reflected != relative {
                    body.kinetics
                        .accept_resolved_linear(add(reflected, mover.velocity));
                    body.kinetics.apply_impulse(Impulse {
                        linear: [0.0; 3],
                        angular: scale(cross(normal, relative), TUMBLE_FACTOR),
                    });
                }
            }
        }
    }

    /// Equal-mass impulse exchange between overlapping bodies, plus
    /// positional separation. Uses the lesser restitution of the pair.
    fn exchange_momentum(&mut self) {
        let n = self.bodies.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let (left, right) = self.bodies.split_at_mut(j);
                let a = &mut left[i];
                let b = &mut right[0];
                if a.is_at_rest() && b.is_at_rest() {
                    continue;
                }
                let between = sub(b.position, a.position);
                let dist_sq = dot(between, between);
                let reach = a.radius + b.radius;
                if dist_sq >= reach * reach || dist_sq == 0.0 {
                    continue;
                }
                let dist = dist_sq.sqrt();
                let normal = scale(between, 1.0 / dist);
                let relative = sub(b.linear_velocity(), a.linear_velocity());
                let approach = dot(relative, normal);
                // Separate overlap regardless; exchange only if closing.
                let push = scale(normal, (reach - dist) * 0.5);
                a.position = sub(a.position, push);
                b.position = add(b.position, push);
                if approach >= 0.0 {
                    continue;
                }
                let e = a
                    .restitution
                    .coefficient()
                    .min(b.restitution.coefficient());
                let magnitude = -(1.0 + e) * approach / 2.0;
                let impulse = scale(normal, magnitude);
                a.kinetics.apply_impulse(Impulse {
                    linear: scale(impulse, -1.0),
                    angular: [0.0; 3],
                });
                b.kinetics.apply_impulse(Impulse {
                    linear: impulse,
                    angular: [0.0; 3],
                });
            }
        }
    }
}

struct Contact {
    /// Displacement that expels the sphere from the box.
    push: [f32; 3],
    /// World-space surface normal at the contact.
    normal: [f32; 3],
}

/// Rotate `v` about the Y axis by `angle` (right-handed, Y up).
fn rotate_y(v: [f32; 3], angle: f32) -> [f32; 3] {
    let (s, c) = angle.sin_cos();
    [c * v[0] + s * v[2], v[1], -s * v[0] + c * v[2]]
}

/// Sphere vs. Y-rotated box: `Some(contact)` when penetrating.
fn sphere_box_penetration(center: [f32; 3], radius: f32, slab: &CollisionBox) -> Option<Contact> {
    let local = rotate_y(sub(center, slab.position), -slab.rotation_y);
    let closest = [
        local[0].clamp(-slab.half_extents[0], slab.half_extents[0]),
        local[1].clamp(-slab.half_extents[1], slab.half_extents[1]),
        local[2].clamp(-slab.half_extents[2], slab.half_extents[2]),
    ];
    let offset = sub(local, closest);
    let dist_sq = dot(offset, offset);
    if dist_sq > 0.0 {
        // Center outside the box: surface contact when within radius.
        if dist_sq >= radius * radius {
            return None;
        }
        let dist = dist_sq.sqrt();
        let normal_local = scale(offset, 1.0 / dist);
        let push_local = scale(normal_local, radius - dist);
        return Some(Contact {
            push: rotate_y(push_local, slab.rotation_y),
            normal: rotate_y(normal_local, slab.rotation_y),
        });
    }
    // Center inside the box: expel along the shallowest axis.
    let mut axis = 0;
    let mut depth = f32::MAX;
    for (candidate, (half, coord)) in slab.half_extents.iter().zip(local.iter()).enumerate() {
        let pen = half - coord.abs();
        if pen < depth {
            depth = pen;
            axis = candidate;
        }
    }
    let mut normal_local = [0.0; 3];
    normal_local[axis] = if local[axis] >= 0.0 { 1.0 } else { -1.0 };
    let push_local = scale(normal_local, depth + radius);
    Some(Contact {
        push: rotate_y(push_local, slab.rotation_y),
        normal: rotate_y(normal_local, slab.rotation_y),
    })
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
    fn a_moving_mover_launches_a_resting_prop() {
        let mut world = KineticWorld::new();
        let id = world.add_body(BallisticBody::at_rest(
            [0.1, 0.0, 0.0],
            0.5,
            Restitution::clamped(0.6),
        ));
        let mover = Mover {
            position: [-1.0, 0.0, 0.0],
            velocity: [3.0, 0.0, 0.0],
            radius: 0.7,
        };
        world.collide_movers(&[mover]);
        let body = world.body(id).unwrap();
        assert!(
            body.linear_velocity()[0] > 0.0,
            "a prop struck by a moving mover must carry momentum away, got {:?}",
            body.linear_velocity()
        );
        assert!(!body.is_at_rest());
        let gap = body.position()[0] - mover.position[0];
        assert!(
            gap >= 1.2 - 1e-3,
            "prop must be expelled to the mover's surface, gap {gap}"
        );
    }

    #[test]
    fn a_stationary_mover_displaces_without_launching() {
        let mut world = KineticWorld::new();
        let id = world.add_body(BallisticBody::at_rest(
            [0.1, 0.0, 0.0],
            0.5,
            Restitution::clamped(0.6),
        ));
        let mover = Mover {
            position: [-1.0, 0.0, 0.0],
            velocity: [0.0; 3],
            radius: 0.7,
        };
        world.collide_movers(&[mover]);
        let body = world.body(id).unwrap();
        assert!(
            body.is_at_rest(),
            "a still mover nudges position but imparts no momentum, got {:?}",
            body.linear_velocity()
        );
        assert!(body.position()[0] - mover.position[0] >= 1.2 - 1e-3);
    }

    #[test]
    fn a_glancing_mover_strike_imparts_tumble() {
        let mut world = KineticWorld::new();
        let id = world.add_body(BallisticBody::at_rest(
            [0.6, 0.8, 0.0],
            0.5,
            Restitution::clamped(0.6),
        ));
        let mover = Mover {
            position: [0.0; 3],
            velocity: [3.0, 0.0, 0.0],
            radius: 0.7,
        };
        world.collide_movers(&[mover]);
        let spin = world.body(id).unwrap().angular_velocity();
        assert!(
            spin != [0.0; 3],
            "an off-center strike must set the prop tumbling"
        );
    }

    #[test]
    fn a_mover_moving_away_does_not_launch() {
        let mut world = KineticWorld::new();
        let id = world.add_body(BallisticBody::at_rest(
            [0.1, 0.0, 0.0],
            0.5,
            Restitution::clamped(0.6),
        ));
        let mover = Mover {
            position: [-1.0, 0.0, 0.0],
            velocity: [-3.0, 0.0, 0.0],
            radius: 0.7,
        };
        world.collide_movers(&[mover]);
        assert!(
            world.body(id).unwrap().is_at_rest(),
            "a separating mover displaces but never injects momentum"
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
