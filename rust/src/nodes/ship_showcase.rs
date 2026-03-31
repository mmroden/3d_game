use godot::prelude::*;
use godot::classes::{
    Node3D, INode3D, MeshInstance3D, BoxMesh, StandardMaterial3D, Engine,
};

/// Cosmetic rotating ship with laser beams for end-of-level screens.
/// Spawned by GameManager during KillSummary/Shop/Death phases.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct ShipShowcase {
    base: Base<Node3D>,
    rotation_speed: f32,
    beam_timer: f32,
    beam_interval: f32,
    laser_color: [f32; 4],
    beams: Vec<Gd<MeshInstance3D>>,
}

#[godot_api]
impl INode3D for ShipShowcase {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            rotation_speed: 0.5,
            beam_timer: 0.0,
            beam_interval: 1.5,
            laser_color: [1.0, 0.2, 0.2, 1.0],
            beams: Vec::new(),
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        self.base_mut().set_visible(false);
        self.build_ship_model();
    }

    fn process(&mut self, delta: f64) {
        if !self.base().is_visible() {
            return;
        }
        let delta = delta as f32;

        // Slow rotation
        let angle = self.rotation_speed * delta;
        self.base_mut().rotate_y(angle);

        // Fire cosmetic beams periodically
        self.beam_timer += delta;
        if self.beam_timer >= self.beam_interval {
            self.beam_timer = 0.0;
            self.fire_cosmetic_beams();
        }

        // Age and remove old beams
        self.age_beams(delta);
    }
}

#[godot_api]
impl ShipShowcase {
    #[func]
    pub fn show_showcase(&mut self, color: Color) {
        self.laser_color = [color.r, color.g, color.b, color.a];
        self.beam_timer = 0.0;
        self.base_mut().set_visible(true);
    }

    #[func]
    pub fn hide_showcase(&mut self) {
        self.base_mut().set_visible(false);
        // Clean up beams
        for mut beam in self.beams.drain(..) {
            if beam.is_instance_valid() {
                beam.queue_free();
            }
        }
    }
}

impl ShipShowcase {
    fn build_ship_model(&mut self) {
        // Simple placeholder ship body — a sleek box
        let mut body = MeshInstance3D::new_alloc();
        let mut body_mesh = BoxMesh::new_gd();
        body_mesh.set_size(Vector3::new(0.6, 0.2, 1.2));
        body.set_mesh(&body_mesh);

        let mut body_mat = StandardMaterial3D::new_gd();
        body_mat.set_albedo(Color::from_rgb(0.3, 0.35, 0.4));
        body_mat.set_metallic(0.8);
        body_mat.set_roughness(0.3);
        body.set_surface_override_material(0, &body_mat);
        self.base_mut().add_child(&body);

        // Left wing
        let mut left_wing = MeshInstance3D::new_alloc();
        let mut wing_mesh = BoxMesh::new_gd();
        wing_mesh.set_size(Vector3::new(0.8, 0.05, 0.4));
        left_wing.set_mesh(&wing_mesh);
        left_wing.set_position(Vector3::new(-0.5, 0.0, 0.2));

        let mut wing_mat = StandardMaterial3D::new_gd();
        wing_mat.set_albedo(Color::from_rgb(0.25, 0.3, 0.35));
        wing_mat.set_metallic(0.7);
        left_wing.set_surface_override_material(0, &wing_mat);
        self.base_mut().add_child(&left_wing);

        // Right wing
        let mut right_wing = MeshInstance3D::new_alloc();
        right_wing.set_mesh(&wing_mesh);
        right_wing.set_position(Vector3::new(0.5, 0.0, 0.2));
        right_wing.set_surface_override_material(0, &wing_mat);
        self.base_mut().add_child(&right_wing);

        // Engine glow
        let mut engine = MeshInstance3D::new_alloc();
        let mut engine_mesh = BoxMesh::new_gd();
        engine_mesh.set_size(Vector3::new(0.15, 0.15, 0.1));
        engine.set_mesh(&engine_mesh);
        engine.set_position(Vector3::new(0.0, 0.0, 0.65));

        let mut engine_mat = StandardMaterial3D::new_gd();
        engine_mat.set_albedo(Color::from_rgb(0.2, 0.5, 1.0));
        engine_mat.set_emission(Color::from_rgb(0.3, 0.6, 1.0));
        engine_mat.set_emission_energy_multiplier(5.0);
        engine.set_surface_override_material(0, &engine_mat);
        self.base_mut().add_child(&engine);
    }

    fn fire_cosmetic_beams(&mut self) {
        let basis = self.base().get_global_transform().basis;
        let center = self.base().get_global_position();
        let forward = -basis.col_c();
        let right = basis.col_a();

        let left_origin = center - right * 0.3 + forward * 0.3;
        let right_origin = center + right * 0.3 + forward * 0.3;
        let beam_length = 8.0;

        self.spawn_beam(left_origin, left_origin + forward * beam_length);
        self.spawn_beam(right_origin, right_origin + forward * beam_length);
    }

    fn spawn_beam(&mut self, from: Vector3, to: Vector3) {
        let midpoint = (from + to) * 0.5;
        let length = from.distance_to(to);
        if length < 0.01 {
            return;
        }

        let mut mesh_instance = MeshInstance3D::new_alloc();
        let mut box_mesh = BoxMesh::new_gd();
        box_mesh.set_size(Vector3::new(0.02, 0.02, length));
        mesh_instance.set_mesh(&box_mesh);

        let mut material = StandardMaterial3D::new_gd();
        let c = self.laser_color;
        material.set_albedo(Color::from_rgba(c[0], c[1], c[2], 1.0));
        material.set_emission(Color::from_rgba(c[0], c[1], c[2], 1.0));
        material.set_emission_energy_multiplier(5.0);
        mesh_instance.set_surface_override_material(0, &material);

        mesh_instance.set_meta("beam_age", &Variant::from(0.0_f32));

        let dir = (to - from).normalized();
        let up = if dir.cross(Vector3::UP).length() > 0.001 {
            Vector3::UP
        } else {
            Vector3::RIGHT
        };
        let z_axis = -dir;
        let x_axis = up.cross(z_axis).normalized();
        let y_axis = z_axis.cross(x_axis);
        let beam_basis = Basis::from_cols(x_axis, y_axis, z_axis);
        let transform = Transform3D { basis: beam_basis, origin: midpoint };
        mesh_instance.set_transform(transform);

        let node = mesh_instance.clone();
        self.base_mut().get_tree().get_root().unwrap().add_child(&mesh_instance);
        self.beams.push(node);
    }

    fn age_beams(&mut self, delta: f32) {
        const BEAM_LIFETIME: f32 = 0.3;

        self.beams.retain_mut(|beam| {
            if !beam.is_instance_valid() {
                return false;
            }
            let age = beam.get_meta("beam_age").to::<f32>() + delta;
            if age >= BEAM_LIFETIME {
                beam.queue_free();
                false
            } else {
                beam.set_meta("beam_age", &Variant::from(age));
                let alpha = 1.0 - (age / BEAM_LIFETIME);
                if let Some(mat) = beam.get_surface_override_material(0) {
                    let mut std_mat = mat.cast::<StandardMaterial3D>();
                    let c = [1.0, 0.2, 0.2]; // default fallback
                    std_mat.set_albedo(Color::from_rgba(c[0], c[1], c[2], alpha));
                }
                true
            }
        });
    }
}
