use godot::prelude::*;
use godot::classes::{
    Area3D, IArea3D, CollisionShape3D, SphereShape3D, MeshInstance3D, SphereMesh,
    StandardMaterial3D, node,
};

use super::constants::{groups, methods};
use super::godot_util;

/// Enemy bolt: a small Area3D projectile, one slot of the [`BoltPool`] ring
/// (Faucet Principle, tier 2). Godot moves nothing for us here — it travels at
/// constant velocity (set on fire) and detonates on first contact: damage to
/// the player, or just vanish on a wall. Detonation and expiry return the slot
/// to *dormant* rather than freeing it, so the pool reuses it for the next shot.
// Bolts are an Area3D moved by teleporting each physics frame, so a small,
// fast bolt tunnels past the player between frames. A larger radius makes hits
// land reliably (the dominant cause of "damage feels low").
const BOLT_RADIUS: f32 = 0.35;
const BOLT_LIFETIME_S: f64 = 3.0;

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
        let step = self.velocity * delta as f32;
        let next = self.base().get_global_position() + step;
        self.base_mut().set_global_position(next);

        self.age += delta;
        if self.age >= BOLT_LIFETIME_S {
            // A spent bolt returns to dormant for reuse — never freed.
            self.deactivate();
        }
    }
}

#[godot_api]
impl EnemyBolt {
    /// Fire this slot: reset-in-place and activate. The whole "reset" for an
    /// Area3D bolt is exactly this routine (transform, velocity, damage, age,
    /// generation, monitoring on) — there is no separate recycle path. Bumping
    /// the generation invalidates any hit still queued from a previous life.
    #[func]
    pub fn fire(&mut self, position: Vector3, velocity: Vector3, damage: f32) {
        self.velocity = velocity;
        self.damage = damage;
        self.age = 0.0;
        self.generation = self.generation.wrapping_add(1);
        self.base_mut().set_global_position(position);
        self.activate();
        // Re-placed across the map, not flown there — don't interpolate from the
        // slot's previous resting position (same as spawn-time placement).
        self.base_mut().reset_physics_interpolation();
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
        // Pass harmlessly through enemies. The bolt and enemies share a
        // collision layer, and a bolt spawns at the firer's muzzle inside its
        // own collision sphere — without this it would detonate on the firing
        // enemy the instant it appears (no visible shot, no damage).
        if body.is_in_group(groups::ENEMIES) {
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
        if body.is_in_group(groups::PLAYER) && body.has_method(methods::TAKE_DAMAGE) {
            // The bolt is on top of the ship at impact, so its own position gives
            // no direction. Point back along its travel — that's where the
            // shooter is — so the hit sound localizes toward the threat.
            let source = self.base().get_global_position() - self.velocity.normalized() * 5.0;
            body.call(
                methods::TAKE_DAMAGE,
                &[Variant::from(self.damage), Variant::from(source)],
            );
        }
        // Detonate on the player or solid world geometry — back to dormant.
        self.deactivate();
    }
}
