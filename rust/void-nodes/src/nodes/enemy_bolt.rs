use godot::prelude::*;
use godot::classes::{
    Area3D, IArea3D, CollisionShape3D, SphereShape3D, MeshInstance3D, SphereMesh,
    StandardMaterial3D,
};

use super::constants::{groups, methods};

/// Enemy bolt: a small Area3D projectile. Godot moves nothing for us
/// here — it travels at constant velocity (set on spawn) and detonates
/// on first contact: damage to the player, or just vanish on a wall.
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
}

#[godot_api]
impl IArea3D for EnemyBolt {
    fn init(base: Base<Area3D>) -> Self {
        Self {
            base,
            velocity: Vector3::ZERO,
            damage: 0.0,
            age: 0.0,
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

        let callable = self.base().callable("on_body_entered");
        self.base_mut().connect("body_entered", &callable);
    }

    fn physics_process(&mut self, delta: f64) {
        let step = self.velocity * delta as f32;
        let next = self.base().get_global_position() + step;
        self.base_mut().set_global_position(next);

        self.age += delta;
        if self.age >= BOLT_LIFETIME_S {
            self.base_mut().queue_free();
        }
    }
}

#[godot_api]
impl EnemyBolt {
    /// Configure a freshly-spawned bolt. Call after adding it to the tree.
    pub fn launch(&mut self, position: Vector3, velocity: Vector3, damage: f32) {
        self.velocity = velocity;
        self.damage = damage;
        let mut base = self.base_mut();
        base.set_global_position(position);
        // Spawned at the muzzle, not flown there — don't interpolate from origin.
        base.reset_physics_interpolation();
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
        let mut body = body;
        if body.is_in_group(groups::PLAYER) && body.has_method(methods::TAKE_DAMAGE) {
            body.call(methods::TAKE_DAMAGE, &[Variant::from(self.damage)]);
        }
        // Detonate on the player or solid world geometry.
        self.base_mut().queue_free();
    }
}
