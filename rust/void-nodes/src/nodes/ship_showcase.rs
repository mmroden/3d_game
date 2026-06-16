use godot::prelude::*;
use godot::classes::{
    Node3D, INode3D, MeshInstance3D, Engine, OmniLight3D,
    PackedScene, ResourceLoader, light_3d,
};

use super::constants::scenes;
use super::godot_util;

/// Target length of the showcase ship (auto-scaled to fit the view).
const SHOWCASE_SHIP_LENGTH: f32 = 2.5;

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
    /// Coloured accent light on the showcase model.
    model_glow: Option<Gd<OmniLight3D>>,
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
            model_glow: None,
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
        if let Some(glow) = &mut self.model_glow {
            if glow.is_instance_valid() {
                glow.set_color(Color::from_rgb(color.r, color.g, color.b));
            }
        }
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
        // The real player ship model (same asset the player flies), so the
        // menu/between-level showcase matches what you take into the level.
        let Some(scene) = ResourceLoader::singleton().load(scenes::SHIP_MODEL) else { return };
        let Ok(packed) = scene.try_cast::<PackedScene>() else { return };
        let Some(instance) = packed.instantiate() else { return };
        let mut model: Gd<Node3D> = instance.cast();
        self.base_mut().add_child(&model);
        godot_util::fit_model_to_length(&mut model, SHOWCASE_SHIP_LENGTH);
        // The model is built facing backward; face its nose forward so the
        // cosmetic beams come from the nose, not the tail (matches the player).
        model.rotate_y(std::f32::consts::PI);

        // Neutral key light so the showcase ship and the room around it are lit
        // (a freshly-generated abandoned base is deliberately dim). On the
        // y-axis at (0, h, 0) it stays put as the showcase spins.
        let mut key = OmniLight3D::new_alloc();
        key.set_color(Color::from_rgb(1.0, 1.0, 0.97));
        key.set_param(light_3d::Param::ENERGY, 4.0);
        key.set_param(light_3d::Param::RANGE, 30.0);
        key.set_position(Vector3::new(0.0, 2.5, 0.0));
        self.base_mut().add_child(&key);

        // Colour accent light — recoloured by show_showcase to the ship colour.
        let mut light = OmniLight3D::new_alloc();
        light.set_color(Color::from_rgb(self.laser_color[0], self.laser_color[1], self.laser_color[2]));
        light.set_param(light_3d::Param::ENERGY, 3.0);
        light.set_param(light_3d::Param::RANGE, 12.0);
        model.add_child(&light);
        self.model_glow = Some(light);
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
        if let Some(mesh_instance) = godot_util::create_beam_mesh(from, to, &self.laser_color) {
            if let Some(root) = godot_util::scene_root(self.base().get_tree()) {
                let node = mesh_instance.clone();
                root.clone().add_child(&mesh_instance);
                self.beams.push(node);
            }
        }
    }

    fn age_beams(&mut self, delta: f32) {
        const BEAM_LIFETIME: f32 = 0.3;
        godot_util::age_beams(&mut self.beams, delta, BEAM_LIFETIME, &self.laser_color);
    }
}
