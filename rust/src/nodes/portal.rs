use godot::prelude::*;
use godot::classes::{
    Area3D, IArea3D, CollisionShape3D, SphereShape3D,
    MeshInstance3D, TorusMesh, StandardMaterial3D,
    GpuParticles3D, ParticleProcessMaterial, SphereMesh,
};

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
        sphere.set_radius(2.0);
        shape.set_shape(&sphere);
        self.base_mut().add_child(&shape);

        // Visual: glowing torus
        let mut mesh_instance = MeshInstance3D::new_alloc();
        let mut torus = TorusMesh::new_gd();
        torus.set_inner_radius(0.8);
        torus.set_outer_radius(1.5);
        mesh_instance.set_mesh(&torus);

        let mut mat = StandardMaterial3D::new_gd();
        mat.set_albedo(Color::from_rgba(0.2, 0.8, 1.0, 0.8));
        mat.set_emission(Color::from_rgba(0.3, 0.7, 1.0, 1.0));
        mat.set_emission_energy_multiplier(8.0);
        mat.set_transparency(godot::classes::base_material_3d::Transparency::ALPHA);
        mesh_instance.set_surface_override_material(0, &mat);
        self.base_mut().add_child(&mesh_instance);

        // Particle effect: orbiting sparkles
        let mut particles = GpuParticles3D::new_alloc();
        particles.set_amount(20);
        particles.set_lifetime(2.0);
        particles.set_explosiveness_ratio(0.0);

        let mut pmat = ParticleProcessMaterial::new_gd();
        pmat.set_spread(180.0);
        pmat.set_color(Color::from_rgba(0.5, 0.9, 1.0, 0.8));
        pmat.set_gravity(Vector3::ZERO);
        pmat.set_param_min(
            godot::classes::particle_process_material::Parameter::INITIAL_LINEAR_VELOCITY,
            1.0,
        );
        pmat.set_param_max(
            godot::classes::particle_process_material::Parameter::INITIAL_LINEAR_VELOCITY,
            3.0,
        );
        particles.set_process_material(&pmat);

        let mut sphere_mesh = SphereMesh::new_gd();
        sphere_mesh.set_radius(0.03);
        sphere_mesh.set_height(0.06);
        let mut spark_mat = StandardMaterial3D::new_gd();
        spark_mat.set_albedo(Color::from_rgba(0.6, 0.9, 1.0, 1.0));
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
        let callable = self.base().callable("on_body_entered");
        self.base_mut().connect("body_entered", &callable);
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

    #[func]
    fn on_body_entered(&mut self, body: Gd<Node3D>) {
        // Check if it's the player (in "player" group)
        if body.is_in_group("player") {
            self.base_mut().emit_signal("portal_entered", &[]);
            // Disable further collisions
            self.base_mut().set_monitoring(false);
        }
    }
}
