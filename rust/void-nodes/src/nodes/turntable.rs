use godot::prelude::*;
use godot::classes::{Node3D, INode3D, MeshInstance3D, Engine, OmniLight3D};

use void_logic::ship::{self, ShipColor};
use void_logic::enemy_type::EnemyType;

use super::constants::scenes;
use super::godot_util;
use super::live_handle::{LiveRef, LiveVec};

/// Kind ids shared with the bestiary UI (mirror of `void_logic::bestiary::
/// BestiaryKind`): 0 = organic barrel, 1 = component cache, 2 = enemy.
const KIND_ORGANIC_BARREL: i32 = 0;
const KIND_COMPONENT_CACHE: i32 = 1;
const KIND_ENEMY: i32 = 2;

/// Fit lengths (world units) the spun subject is scaled to, so a tiny drone and
/// a big crate read at the same size. The hero ship gets a touch more room.
const SHIP_LENGTH: f32 = 2.5;
const ENTRY_LENGTH: f32 = 2.0;

/// One model on a slow turntable, parked in front of the camera in the backdrop
/// room. This is the single display mechanism for the non-gameplay screens:
/// camera-front tracking, the neutral key light that lifts the subject (and the
/// dim room) out of the dark, the spin, and model fitting all live here once.
/// Two content modes ride on top of that mechanism:
///   * ship mode (`show_ship`) — the player's hull, with body-style paint,
///     an accent glow, and cosmetic beams (the menu/loadout "hero" shot);
///   * entry mode (`show_entry`) — a single bestiary catalog subject (pickup
///     or enemy), no beams.
///
/// Replaces the former `ShipShowcase` + `BestiaryDisplay`, which had drifted
/// apart as two copies of this same mechanism.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct Turntable {
    base: Base<Node3D>,
    rotation_speed: f32,
    beam_timer: f32,
    beam_interval: f32,
    /// Cosmetic beams fire only in ship mode; the bestiary subjects are inert.
    beams_enabled: bool,
    accent_color: [f32; 4],
    beams: LiveVec<MeshInstance3D>,
    model: Option<LiveRef<Node3D>>,
    model_glow: Option<LiveRef<OmniLight3D>>,
}

#[godot_api]
impl INode3D for Turntable {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            rotation_speed: 0.5,
            beam_timer: 0.0,
            beam_interval: 1.5,
            beams_enabled: false,
            accent_color: [1.0, 0.2, 0.2, 1.0],
            beams: LiveVec::new(),
            model: None,
            model_glow: None,
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        // Neutral key light so the subject AND the dim room around it read
        // against the dark backdrop a freshly-generated abandoned base spawns
        // with. On the turntable's own axis so it stays put as the subject
        // spins. Built once here; both modes share it.
        let mut base: Gd<Node3D> = self.base().clone();
        godot_util::attach_key_light(&mut base, 4.0, 30.0);
        self.base_mut().set_visible(false);
    }

    fn process(&mut self, delta: f64) {
        if !self.base().is_visible() {
            return;
        }
        let delta = delta as f32;
        // Track the player camera every frame so the subject is always in view,
        // regardless of where the parked camera ends up — placing it once raced
        // the camera teleport and left it off-screen, reading as black.
        if let Some(main) = self.base().get_parent() {
            if let Some(pos) = godot_util::camera_front_position(&main, 6.0) {
                self.base_mut().set_global_position(pos);
            }
        }
        let angle = self.rotation_speed * delta;
        self.base_mut().rotate_y(angle);

        if self.beams_enabled {
            self.beam_timer += delta;
            if self.beam_timer >= self.beam_interval {
                self.beam_timer = 0.0;
                self.fire_cosmetic_beams();
            }
            self.age_beams(delta);
        }
    }
}

#[godot_api]
impl Turntable {
    /// Ship mode: the player's hull in the chosen color, with body-style paint,
    /// an accent glow, and cosmetic beams. Shown on the menu and loadout screens.
    #[func]
    pub fn show_ship(&mut self, color_id: i32) {
        let sc = ShipColor::from_id(color_id).unwrap_or_default();
        self.accent_color = sc.color();
        self.beams_enabled = true;
        self.beam_timer = 0.0;
        self.set_model(scenes::SHIP_MODEL, sc.color(), 12.0, SHIP_LENGTH);
        // Paint the hull to the variant's body style. apply_body_style walks our
        // subtree, so the freshly-spawned model child is found without a handle.
        let style = sc.body_style();
        let idx = ship::style_texture_index(ship::STYLED_BODY_PART, style);
        let root: Gd<Node3D> = self.base().clone();
        godot_util::apply_body_style(&root, style, idx);
    }

    /// Entry mode: one bestiary catalog subject. `kind` selects pickup vs enemy;
    /// `enemy_type_id` is the `EnemyType` id when `kind` is an enemy, else ignored.
    #[func]
    pub fn show_entry(&mut self, kind: i32, enemy_type_id: i32) {
        self.beams_enabled = false;
        let (path, glow): (Option<&str>, [f32; 4]) = match kind {
            KIND_ORGANIC_BARREL => (Some(scenes::BARREL_MODEL), [0.2, 0.9, 0.2, 1.0]),
            KIND_COMPONENT_CACHE => (Some(scenes::CRATE_MODEL), [0.2, 0.5, 1.0, 1.0]),
            KIND_ENEMY => (
                EnemyType::from_id(enemy_type_id).map(|t| t.model_path()),
                // Neutral glow so the unlit enemy reads in the dark room.
                [1.0, 1.0, 0.95, 1.0],
            ),
            _ => (None, [1.0, 1.0, 1.0, 1.0]),
        };
        if let Some(path) = path {
            self.set_model(path, glow, 8.0, ENTRY_LENGTH);
        } else {
            self.hide_turntable();
        }
    }

    /// Drop the current subject and hide the turntable.
    #[func]
    pub fn hide_turntable(&mut self) {
        self.clear_model();
        self.beams.for_each_live(|_, beam, _| beam.queue_free());
        self.beams.clear();
        self.base_mut().set_visible(false);
    }
}

impl Turntable {
    /// The shared spawn path: clear the old subject, square the turntable, spawn
    /// the fitted model with its accent glow, and show. Every mode funnels here.
    fn set_model(&mut self, path: &str, glow: [f32; 4], glow_range: f32, length: f32) {
        self.clear_model();
        self.base_mut().set_rotation(Vector3::ZERO);
        let mut parent: Gd<Node3D> = self.base().clone();
        if let Some(mut model) = godot_util::spawn_fitted_model(&mut parent, path, length) {
            self.model_glow = Some(godot_util::attach_glow_light(&mut model, &glow, 3.0, glow_range));
            self.model = Some(LiveRef::new(&model));
        }
        self.base_mut().set_visible(true);
    }

    fn clear_model(&mut self) {
        if let Some(model) = self.model.take() {
            model.with(|m| m.queue_free());
        }
        self.model_glow = None;
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
