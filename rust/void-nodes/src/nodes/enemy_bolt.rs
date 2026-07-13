use godot::prelude::*;
use godot::classes::{
    Area3D, IArea3D, CollisionShape3D, SphereShape3D, MeshInstance3D, SphereMesh,
    StandardMaterial3D, node,
};

use super::constants::{groups, methods};
use super::godot_util;
use super::live_handle::{LiveOpt, LiveRef};
use void_logic::armament::{cluster, homing, tracer, Faction};

/// A bolt slot's special payload, resolved when the bolt spends itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoltPayload {
    #[default]
    None,
    /// A cluster shell: bursts into player-faction fragments on impact or expiry.
    ClusterBurst,
}

/// Everything about a bolt's life beyond its ballistics: whose it is, what
/// it steers at, and what it does when spent. One argument at every fire
/// site — the ballistic triple (position, velocity, damage) stays loose
/// because every caller computes those fresh.
#[derive(Clone, Copy)]
pub struct BoltProfile {
    pub faction: Faction,
    pub homing_target: Option<InstanceId>,
    pub turn_rad: f32,
    pub payload: BoltPayload,
}

impl BoltProfile {
    /// A plain ballistic bolt for the given side: no lock, no payload.
    pub fn ballistic(faction: Faction) -> Self {
        Self { faction, homing_target: None, turn_rad: 0.0, payload: BoltPayload::None }
    }
}

/// A bolt: a small Area3D projectile, one slot of the [`BoltPool`] ring
/// (Faucet Principle, tier 2), serving BOTH factions — the slot's faction
/// decides whose group it passes through and whose it damages. It travels at
/// constant velocity (set on fire), optionally steering toward a homing
/// target, and detonates on first contact. Detonation and expiry return the
/// slot to *dormant* rather than freeing it, so the pool reuses it.
// Bolts are an Area3D moved by teleporting each physics frame, so a small,
// fast bolt tunnels past the player between frames. A larger radius makes hits
// land reliably (the dominant cause of "damage feels low").
const BOLT_RADIUS: f32 = 0.35;
const BOLT_LIFETIME_S: f64 = 3.0;
/// How far back along the bolt's travel the hit sound is placed, so the cue
/// points at the shooter (the ship-side companion knob is ship_controller's
/// HIT_SFX_OFFSET — tune the two together or ram and bolt hits localize
/// inconsistently).
const HIT_SFX_OFFSET: f32 = 5.0;
/// Cluster fragments inherit this fraction of the shell's speed, faster than
/// the lobbed shell so the burst reads as an explosion.
const FRAGMENT_SPEED_FACTOR: f32 = 1.6;
/// Each fragment carries this fraction of the shell's damage.
const FRAGMENT_DAMAGE_FACTOR: f32 = 0.4;
/// Cone half-angle for the burst.
const FRAGMENT_SPREAD_RAD: f32 = 0.7;

#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct EnemyBolt {
    base: Base<Area3D>,
    velocity: Vector3,
    damage: f32,
    age: f64,
    /// Per-slot generation counter, bumped on every [`fire`](EnemyBolt::fire).
    /// A deferred hit callback carries the generation it was armed with and
    /// no-ops if the slot has since re-fired — the stale-signal guard the
    /// faucet design calls for (a slot re-fired in the same frame its previous
    /// occupant's `body_entered` was still queued must not register the old hit).
    generation: u64,
    /// Logical liveness, flipped immediately on fire/expiry/hit. The engine-side
    /// dormancy flags follow on the deferred boundary — Godot blocks monitoring
    /// and collision toggles inside physics callbacks and signal flushes — so
    /// this field, not `is_monitoring`, is the truth the pool and flight logic
    /// read.
    live: bool,
    /// Whose bolt this life is — pass through your own side, damage the other.
    faction: Faction,
    /// Steer toward this node while it lives; a freed or hidden target drops
    /// the lock and the bolt flies on ballistic (instance-id validated per
    /// tick, the same guard discipline as the generation counter).
    homing_target: Option<InstanceId>,
    /// Max steering rate (rad/s) for this life's homing lock — the player's
    /// tracking laser arms `homing::TURN_RATE`, an enemy arms its def's
    /// `bolt_turn_deg`. 0 = ballistic even with a target.
    turn_rad: f32,
    /// The visual mesh, held weakly for the tracer stretch (the collider
    /// stays a speed-agnostic sphere).
    mesh: Option<LiveRef<MeshInstance3D>>,
    /// What the bolt does when it spends itself.
    payload: BoltPayload,
}

#[godot_api]
impl IArea3D for EnemyBolt {
    fn init(base: Base<Area3D>) -> Self {
        Self {
            base,
            velocity: Vector3::ZERO,
            damage: 0.0,
            age: 0.0,
            generation: 0,
            live: false,
            faction: Faction::Enemy,
            homing_target: None,
            turn_rad: 0.0,
            mesh: None,
            payload: BoltPayload::None,
        }
    }

    fn ready(&mut self) {
        let mut shape = SphereShape3D::new_gd();
        shape.set_radius(BOLT_RADIUS);
        let mut col = CollisionShape3D::new_alloc();
        col.set_shape(&shape);
        self.base_mut().add_child(&col);

        let mut sphere = SphereMesh::new_gd();
        sphere.set_radius(BOLT_RADIUS);
        sphere.set_height(BOLT_RADIUS * 2.0);
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_albedo(Color::from_rgb(1.0, 0.3, 0.1));
        mat.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
        mat.set_emission(Color::from_rgb(1.0, 0.4, 0.05));
        let mut mesh = MeshInstance3D::new_alloc();
        mesh.set_mesh(&sphere);
        mesh.set_surface_override_material(0, &mat);
        self.base_mut().add_child(&mesh);
        self.mesh = Some(LiveRef::new(&mesh));

        // The bolt is its own light source — it lights the dark room as it flies.
        let mut node: Gd<Node3D> = self.base().clone().upcast();
        godot_util::attach_glow_light(&mut node, &[1.0, 0.45, 0.12], 4.0, 6.0);

        let callable = self.base().callable("on_body_entered");
        self.base_mut().connect("body_entered", &callable);

        // Preallocated slots enter the tree dormant: they cost only memory until
        // fired. The pool builds the whole ring during the loading phase.
        self.deactivate();
    }

    fn physics_process(&mut self, delta: f64) {
        // The engine flags lag liveness by one deferred flush; don't fly or age
        // during that gap.
        if !self.live {
            return;
        }
        self.steer_toward_target(delta as f32);
        // A steering bolt bends its path — keep the streak on the velocity.
        // Ballistic bolts hold their arm-time orientation for free.
        if self.turn_rad > 0.0 {
            self.shape_tracer();
        }
        let step = self.velocity * delta as f32;
        let next = self.base().get_global_position() + step;
        self.base_mut().set_global_position(next);

        self.age += delta;
        if self.age >= BOLT_LIFETIME_S {
            // A spent bolt returns to dormant for reuse — never freed. A
            // cluster shell that outlives its flight still bursts.
            self.spend();
        }
    }
}

#[godot_api]
impl EnemyBolt {
    /// Fire this slot as an enemy ballistic bolt — the classic path every
    /// enemy fire site calls.
    #[func]
    pub fn fire(&mut self, position: Vector3, velocity: Vector3, damage: f32) {
        self.arm(position, velocity, damage, BoltProfile::ballistic(Faction::Enemy));
    }

    /// Arm this slot: reset-in-place and activate. The whole "reset" for an
    /// Area3D bolt is exactly this routine (transform, velocity, damage, age,
    /// generation, faction, homing lock, payload, monitoring on) — there is no
    /// separate recycle path. Bumping the generation invalidates any hit still
    /// queued from a previous life.
    pub fn arm(
        &mut self,
        position: Vector3,
        velocity: Vector3,
        damage: f32,
        profile: BoltProfile,
    ) {
        self.velocity = velocity;
        self.damage = damage;
        self.age = 0.0;
        self.faction = profile.faction;
        self.homing_target = profile.homing_target;
        self.turn_rad = profile.turn_rad;
        self.payload = profile.payload;
        self.generation = self.generation.wrapping_add(1);
        self.base_mut().set_global_position(position);
        // Every life re-shapes the tracer: a pooled slot may go from a fast
        // oblong streak to a slow round blob between lives.
        self.shape_tracer();
        self.activate();
        // Re-placed across the map, not flown there — don't interpolate from the
        // slot's previous resting position (same as spawn-time placement).
        self.base_mut().reset_physics_interpolation();
    }

    /// Point the bolt along its velocity and stretch the mesh into a tracer
    /// streak spanning `tracer::TRAIL_SECONDS` of travel — fast bolts read
    /// as oblong tracers, slow ones stay round blobs. The collider is a
    /// sphere, so rotating the body is free; the mesh alone stretches.
    fn shape_tracer(&mut self) {
        let speed = self.velocity.length();
        if speed <= f32::EPSILON {
            return; // a parked slot keeps its last shape — it is invisible anyway
        }
        let dir = self.velocity / speed;
        // Any up not colinear with the travel axis serves the frame.
        let up = if dir.y.abs() > 0.99 { Vector3::RIGHT } else { Vector3::UP };
        let target = self.base().get_global_position() + dir;
        self.base_mut().look_at_ex(target).up(up).done();
        let stretch = tracer::stretch(speed, BOLT_RADIUS * 2.0);
        // -Z is the look axis: the streak lies along the travel.
        self.mesh.with(|m| m.set_scale(Vector3::new(1.0, 1.0, stretch)));
    }

    /// Bend the velocity toward the homing target, if the lock still resolves
    /// to a live, visible node; otherwise drop it and fly on ballistic.
    fn steer_toward_target(&mut self, dt: f32) {
        if self.turn_rad <= 0.0 {
            return; // armed ballistic — a target without a turn rate never bends
        }
        let Some(target_id) = self.homing_target else { return };
        let target = Gd::<Node3D>::try_from_instance_id(target_id).ok()
            .filter(|t| t.is_inside_tree() && t.is_visible_in_tree());
        let Some(target) = target else {
            self.homing_target = None; // target died or went dormant: ballistic
            return;
        };
        let to = target.get_global_position() - self.base().get_global_position();
        let steered = homing::steer(
            [self.velocity.x, self.velocity.y, self.velocity.z],
            [to.x, to.y, to.z],
            self.turn_rad,
            dt,
        );
        self.velocity = Vector3::new(steered[0], steered[1], steered[2]);
    }

    /// This slot's current generation. Exposed for the stale-hit guard test.
    #[func]
    fn generation(&self) -> i64 {
        self.generation as i64
    }

    /// Whether this slot is currently live (activated by a fire, not yet spent).
    /// The pool sums this across the ring for its live count. Reads the logical
    /// flag, which leads the engine flags by up to one deferred flush.
    #[func]
    pub fn is_live(&self) -> bool {
        self.live
    }

    /// Go live: logical flag now, engine flags on the deferred boundary.
    fn activate(&mut self) {
        self.live = true;
        self.base_mut().call_deferred(methods::APPLY_DORMANCY, &[true.to_variant()]);
    }

    /// Return to dormant: logical flag now, engine flags on the deferred
    /// boundary. `physics_process` and `resolve_hit` gate on the logical flag,
    /// so nothing observable happens during the one-flush lag.
    fn deactivate(&mut self) {
        self.live = false;
        self.base_mut().call_deferred(methods::APPLY_DORMANCY, &[false.to_variant()]);
    }

    /// Flip the engine-side dormancy flags, together: visibility, processing,
    /// and collision (monitoring + monitorable for an Area3D). Always invoked
    /// via `call_deferred` — expiry fires inside `physics_process` and hit
    /// chains inside signal flushes, where Godot blocks these setters ("Function
    /// blocked during in/out signal"); a blocked set would strand the slot
    /// half-dormant: an invisible, frozen bolt still colliding, i.e. a damage
    /// mine. The deferred boundary is always legal.
    #[func]
    fn apply_dormancy(&mut self, live: bool) {
        let mode = if live { node::ProcessMode::INHERIT } else { node::ProcessMode::DISABLED };
        let mut base = self.base_mut();
        base.set_visible(live);
        base.set_process_mode(mode);
        base.set_monitoring(live);
        base.set_monitorable(live);
    }

    /// Force this slot back to dormant regardless of its flight state — for the
    /// pool to clear the ring when a level rebuilds. Bumps the generation so any
    /// hit still queued from the cleared bolt is dropped as stale.
    pub fn deactivate_for_pool(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.deactivate();
    }

    #[func]
    fn on_body_entered(&mut self, body: Gd<Node3D>) {
        // Pass harmlessly through the firing side. Everything shares one
        // collision layer, and a bolt spawns at the firer's muzzle inside its
        // own collision sphere — without this it would detonate on the firer
        // the instant it appears (no visible shot, no damage).
        let own_side = match self.faction {
            Faction::Enemy => groups::ENEMIES,
            Faction::Player => groups::PLAYER,
        };
        if body.is_in_group(own_side) {
            return;
        }
        // Player bolts also pass through the player's own drones.
        if self.faction == Faction::Player && body.is_in_group(groups::PLAYER_DRONES) {
            return;
        }
        // Resolve the hit on a deferred boundary, tagged with the generation it
        // was armed at. If the slot re-fires before the deferred call runs, the
        // generation will have advanced and `resolve_hit` drops the stale hit.
        let armed_generation = self.generation as i64;
        let this = self.to_gd();
        this.callable(methods::RESOLVE_HIT).call_deferred(&[
            body.to_variant(),
            armed_generation.to_variant(),
        ]);
    }

    /// Apply a body's hit, guarded by the generation it was armed with. A hit
    /// queued for a previous life (the slot re-fired in between) is dropped.
    #[func]
    fn resolve_hit(&mut self, body: Gd<Node3D>, armed_generation: i64) {
        if armed_generation != self.generation as i64 {
            return; // stale: this slot has re-fired since the hit was queued
        }
        if !self.live {
            return; // the bolt expired in the gap before its engine flags flushed
        }
        let mut body = body;
        match self.faction {
            Faction::Enemy => {
                if body.is_in_group(groups::PLAYER) && body.has_method(methods::TAKE_DAMAGE) {
                    // The bolt is on top of the ship at impact, so its own position
                    // gives no direction. Point back along its travel — that's where
                    // the shooter is — so the hit sound localizes toward the threat.
                    let source =
                        self.base().get_global_position() - self.velocity.normalized() * HIT_SFX_OFFSET;
                    body.call(
                        methods::TAKE_DAMAGE,
                        &[Variant::from(self.damage), Variant::from(source)],
                    );
                }
            }
            Faction::Player => {
                if body.is_in_group(groups::ENEMIES) && body.has_method(methods::TAKE_DAMAGE) {
                    // EnemyDrone::take_damage takes the raw amount.
                    body.call(methods::TAKE_DAMAGE, &[Variant::from(self.damage)]);
                }
            }
        }
        // Detonate on the target or solid world geometry — back to dormant
        // (bursting first, if this life carried a cluster payload).
        self.spend();
    }

    /// Spend the bolt: resolve any payload, then return the slot to dormant.
    fn spend(&mut self) {
        if self.payload == BoltPayload::ClusterBurst {
            self.burst();
        }
        self.deactivate();
    }

    /// The cluster shell's burst: a deterministic cone of player-faction
    /// fragments fired through the shared pool (found by group, like the
    /// audio manager). The seed mixes the slot and its generation so every
    /// shell bursts uniquely but reproducibly.
    fn burst(&mut self) {
        let dir = self.velocity.normalized();
        if !dir.is_finite() {
            return;
        }
        let position = self.base().get_global_position();
        let speed = self.velocity.length() * FRAGMENT_SPEED_FACTOR;
        let seed = (self.base().instance_id().to_i64() as u64).wrapping_add(self.generation);
        let directions = cluster::fragment_directions(
            [dir.x, dir.y, dir.z],
            cluster::FRAGMENT_COUNT,
            FRAGMENT_SPREAD_RAD,
            seed,
        );
        let pool = self
            .base()
            .get_tree()
            .get_first_node_in_group(groups::BOLT_POOL)
            .and_then(|n| n.try_cast::<super::bolt_pool::BoltPool>().ok());
        let Some(mut pool) = pool else { return };
        let damage = self.damage * FRAGMENT_DAMAGE_FACTOR;
        for d in directions {
            pool.bind_mut().fire_player(
                position,
                Vector3::new(d[0], d[1], d[2]) * speed,
                damage,
            );
        }
    }
}
