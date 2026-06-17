use godot::prelude::*;
use godot::classes::{
    Node3D, INode3D, MeshInstance3D, Engine, OmniLight3D,
};

use super::constants::scenes;
use super::godot_util;
use super::live_handle::{LiveRef, LiveVec};
use void_logic::ship::{self, ShipColor};

/// Target length of the showcase ship (auto-scaled to fit the view).
const SHOWCASE_SHIP_LENGTH: f32 = 2.5;

/// Cosmetic rotating ship with laser beams for the non-gameplay screens.
/// Shown by GameManager on MainMenu, ShipSelect, KillSummary, Shop, and Death.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct ShipShowcase {
    base: Base<Node3D>,
    rotation_speed: f32,
    beam_timer: f32,
    beam_interval: f32,
    accent_color: [f32; 4],
    beams: LiveVec<MeshInstance3D>,
    /// Color accent light on the showcase model.
    model_glow: Option<LiveRef<OmniLight3D>>,
}

#[godot_api]
impl INode3D for ShipShowcase {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            rotation_speed: 0.5,
            beam_timer: 0.0,
            beam_interval: 1.5,
            accent_color: [1.0, 0.2, 0.2, 1.0],
            beams: LiveVec::new(),
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

        // Track the player camera every frame so the ship is always in view —
        // placing it once raced the camera's teleport (parking it in a backdrop
        // room) and left the ship off-screen, which read as a black screen.
        if let Some(main) = self.base().get_parent() {
            if let Some(pos) = godot_util::camera_front_position(&main, 6.0) {
                self.base_mut().set_global_position(pos);
            }
        }

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
    pub fn show_showcase(&mut self, color_id: i32) {
        let sc = ShipColor::from_id(color_id).unwrap_or_default();
        self.accent_color = sc.color();
        godot_util::recolor_glow(&self.model_glow, godot_util::to_color(sc.color()));
        // Re-skin the painted hull to the variant's body style. apply_body_style
        // walks our subtree, so the model child is found without a stored handle.
        let style = sc.body_style();
        let idx = ship::style_texture_index(ship::STYLED_BODY_PART, style);
        let root: Gd<Node3D> = self.base().clone();
        godot_util::apply_body_style(&root, style, idx);
        self.beam_timer = 0.0;
        self.base_mut().set_visible(true);
    }

    #[func]
    pub fn hide_showcase(&mut self) {
        self.base_mut().set_visible(false);
        // Clean up beams
        self.beams.for_each_live(|_, beam, _| beam.queue_free());
        self.beams.clear();
    }
}

impl ShipShowcase {
    fn build_ship_model(&mut self) {
        // The real player ship model (same asset the player flies, via the same
        // shared helper), so the showcase matches what you take into the level.
        let mut base: Gd<Node3D> = self.base().clone();
        let Some(mut model) =
            godot_util::spawn_fitted_model(&mut base, scenes::SHIP_MODEL, SHOWCASE_SHIP_LENGTH)
        else {
            return;
        };

        // Neutral key light so the showcase ship and the room around it are lit
        // (a freshly-generated abandoned base is deliberately dim). Shared with
        // the bestiary briefing so both screens read the same.
        let mut base_for_key: Gd<Node3D> = self.base().clone();
        godot_util::attach_key_light(&mut base_for_key, 4.0, 30.0);

        // Color accent light — recolored by show_showcase to the ship color.
        self.model_glow = Some(godot_util::attach_glow_light(&mut model, &self.accent_color, 3.0, 12.0));
        // Default painted body style until show_showcase pushes the choice.
        let style = ShipColor::default().body_style();
        godot_util::apply_body_style(&model, style, ship::style_texture_index(ship::STYLED_BODY_PART, style));
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
        if let Some(mesh_instance) = godot_util::create_beam_mesh(from, to, &self.accent_color) {
            if let Some(root) = godot_util::scene_root(self.base().get_tree()) {
                root.clone().add_child(&mesh_instance);
                self.beams.push(&mesh_instance, ());
            }
        }
    }

    fn age_beams(&mut self, delta: f32) {
        const BEAM_LIFETIME: f32 = 0.3;
        godot_util::age_beams(&mut self.beams, delta, BEAM_LIFETIME, &self.accent_color);
    }
}
