use godot::prelude::*;
use godot::classes::{Node3D, INode3D, Engine};

use void_logic::enemy_type::EnemyType;

use super::constants::scenes;
use super::godot_util;
use super::live_handle::LiveRef;

/// Kind ids shared with the bestiary UI (mirror of `void_logic::bestiary::
/// BestiaryKind`): 0 = organic barrel, 1 = component cache, 2 = enemy.
const KIND_ORGANIC_BARREL: i32 = 0;
const KIND_COMPONENT_CACHE: i32 = 1;
const KIND_ENEMY: i32 = 2;

/// Target length the spun model is fitted to (world units), so a tiny drone and
/// a big crate read at the same scale on the briefing turntable.
const DISPLAY_LENGTH: f32 = 2.0;
const ROTATION_SPEED: f32 = 0.6;

/// A slow turntable for the bestiary briefing: shows the bare model for the
/// current entry (a pickup or an enemy) without any of its gameplay behavior.
/// GameManager parks it in the loadout room in front of the camera, like the
/// ship showcase, and drives it one entry at a time.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct BestiaryDisplay {
    base: Base<Node3D>,
    model: Option<LiveRef<Node3D>>,
}

#[godot_api]
impl INode3D for BestiaryDisplay {
    fn init(base: Base<Node3D>) -> Self {
        Self { base, model: None }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        // Same neutral key light the ship showcase uses, so briefing subjects
        // read against the dark backdrop instead of sitting near-black (the
        // per-entry colored accent alone isn't enough). On the turntable's Y
        // axis, so it stays put as the subject spins.
        let mut base: Gd<Node3D> = self.base().clone();
        godot_util::attach_key_light(&mut base, 4.0, 30.0);
        self.base_mut().set_visible(false);
    }

    fn process(&mut self, delta: f64) {
        if !self.base().is_visible() {
            return;
        }
        // Track the player camera each frame so the subject is always in view,
        // regardless of where the parked camera ends up (same fix as the ship
        // showcase — placing it once left it off-screen, reading as black).
        if let Some(main) = self.base().get_parent() {
            if let Some(pos) = godot_util::camera_front_position(&main, 6.0) {
                self.base_mut().set_global_position(pos);
            }
        }
        let angle = ROTATION_SPEED * delta as f32;
        self.base_mut().rotate_y(angle);
    }
}

#[godot_api]
impl BestiaryDisplay {
    /// Show the model for one bestiary entry. `kind` selects pickup vs enemy;
    /// `enemy_type_id` is the `EnemyType` id when `kind` is an enemy, ignored
    /// otherwise. Replaces whatever was on the turntable.
    #[func]
    pub fn show_entry(&mut self, kind: i32, enemy_type_id: i32) {
        self.clear_model();
        // Each entry starts square to the camera, then spins from there.
        self.base_mut().set_rotation(Vector3::ZERO);

        let (path, glow): (Option<&str>, Option<[f32; 4]>) = match kind {
            KIND_ORGANIC_BARREL => (Some(scenes::BARREL_MODEL), Some([0.2, 0.9, 0.2, 1.0])),
            KIND_COMPONENT_CACHE => (Some(scenes::CRATE_MODEL), Some([0.2, 0.5, 1.0, 1.0])),
            KIND_ENEMY => (
                EnemyType::from_id(enemy_type_id).map(|t| t.model_path()),
                // Neutral key light so the unlit enemy reads in the dark room.
                Some([1.0, 1.0, 0.95, 1.0]),
            ),
            _ => (None, None),
        };

        if let Some(path) = path {
            let mut parent: Gd<Node3D> = self.base().clone();
            if let Some(mut model) = godot_util::spawn_fitted_model(&mut parent, path, DISPLAY_LENGTH) {
                if let Some(color) = glow {
                    godot_util::attach_glow_light(&mut model, &color, 3.0, 8.0);
                }
                self.model = Some(LiveRef::new(&model));
            }
        }
        self.base_mut().set_visible(true);
    }

    /// Hide the turntable and drop its model.
    #[func]
    pub fn hide_display(&mut self) {
        self.clear_model();
        self.base_mut().set_visible(false);
    }
}

impl BestiaryDisplay {
    fn clear_model(&mut self) {
        if let Some(model) = self.model.take() {
            model.with(|m| m.queue_free());
        }
    }
}
