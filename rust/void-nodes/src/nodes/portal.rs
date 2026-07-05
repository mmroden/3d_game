use godot::prelude::*;
use godot::classes::{
    Area3D, IArea3D, CollisionShape3D, SphereShape3D,
    StandardMaterial3D, GpuParticles3D, SphereMesh, Node3D,
};

use super::constants::{groups, methods, scenes, signals};
use super::godot_util;
use void_logic::audio_catalog::SfxEvent;

/// Radius (m) of the trigger sphere the player must touch to take the portal.
const PORTAL_TRIGGER_RADIUS: f32 = 2.0;

/// Diameter (m) the imported gate model is fit-scaled to. Derived from the
/// trigger radius so the ring the player flies through and the volume that
/// fires can never drift apart.
const GATE_SIZE: f32 = 2.0 * PORTAL_TRIGGER_RADIUS;

/// End-of-level portal. Player touches it to complete the level.
#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct Portal {
    base: Base<Area3D>,
    time: f32,
}

#[godot_api]
impl IArea3D for Portal {
    fn init(base: Base<Area3D>) -> Self {
        Self { base, time: 0.0 }
    }

    fn ready(&mut self) {
        // Collision shape
        let mut shape = CollisionShape3D::new_alloc();
        let mut sphere = SphereShape3D::new_gd();
        sphere.set_radius(PORTAL_TRIGGER_RADIUS);
        shape.set_shape(&sphere);
        self.base_mut().add_child(&shape);

        // Visual: the jump-gate model (installed by `make assets`; see
        // scenes::JUMP_GATE_MODEL), fit to the trigger diameter. Loaded through
        // the shared model helper like the ship and enemies — no procedural mesh.
        let mut gate_parent: Gd<Node3D> = self.base().clone().upcast();
        godot_util::spawn_model_fitted(&mut gate_parent, scenes::JUMP_GATE_MODEL, GATE_SIZE);

        // Particle effect: orbiting sparkles
        let mut particles = GpuParticles3D::new_alloc();
        particles.set_amount(20);
        particles.set_lifetime(2.0);
        particles.set_explosiveness_ratio(0.0);

        let pmat = godot_util::particle_burst_material(
            180.0,
            Color::from_rgba(0.5, 0.9, 1.0, 0.8),
            (1.0, 3.0),
            None,
        );
        particles.set_process_material(&pmat);

        let mut sphere_mesh = SphereMesh::new_gd();
        sphere_mesh.set_radius(0.03);
        sphere_mesh.set_height(0.06);
        let mut spark_mat = StandardMaterial3D::new_gd();
        spark_mat.set_albedo(Color::from_rgba(0.6, 0.9, 1.0, 1.0));
        spark_mat.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
        spark_mat.set_emission(Color::from_rgba(0.4, 0.8, 1.0, 1.0));
        spark_mat.set_emission_energy_multiplier(6.0);
        sphere_mesh.set_material(&spark_mat);
        particles.set_draw_pass_mesh(0, &sphere_mesh);

        particles.set_emitting(true);
        self.base_mut().add_child(&particles);

        // Monitor for player (collision layer 1)
        self.base_mut().set_monitoring(true);
        self.base_mut().set_collision_mask(1);
        self.base_mut().set_collision_layer(0);

        // Connect body_entered signal
        let callable = self.base().callable(methods::ON_BODY_ENTERED);
        self.base_mut().connect(signals::BODY_ENTERED, &callable);
    }

    fn process(&mut self, delta: f64) {
        self.time += delta as f32;
        // Slow rotation
        let rotation = Vector3::new(0.0, self.time * 0.5, 0.0);
        self.base_mut().set_rotation(rotation);
    }
}

#[godot_api]
impl Portal {
    #[signal]
    fn portal_entered();

    /// Dormancy flip (Faucet Principle): on boss levels the portal is
    /// pre-built with the level but stays dark and intangible until the
    /// fight resolves. Flips directly — from a physics callback, invoke it
    /// via `call_deferred` (the house dormancy pattern).
    #[func]
    pub fn set_dormant(&mut self, dormant: bool) {
        self.base_mut().set_visible(!dormant);
        self.base_mut().set_monitoring(!dormant);
    }

    #[func]
    fn on_body_entered(&mut self, body: Gd<Node3D>) {
        // Check if it's the player (in "player" group)
        if body.is_in_group(groups::PLAYER) {
            // Portal enter SFX
            let pos = self.base().get_global_position();
            if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
                audio.bind_mut().play_event_at(SfxEvent::PortalEnter, pos);
            }
            self.base_mut().emit_signal(signals::PORTAL_ENTERED, &[]);
            // Disable further collisions — call_deferred of the setter (the
            // house dormancy pattern; a property set_deferred("monitoring")
            // silently never lands).
            self.base_mut()
                .call_deferred(methods::SET_MONITORING, &[Variant::from(false)]);
        }
    }
}
