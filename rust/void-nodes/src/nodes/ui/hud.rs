use godot::prelude::*;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, ColorRect, HBoxContainer, VBoxContainer, Control, Node,
    Camera3D, Engine, Polygon2D,
    control::LayoutPreset,
};

use super::map_panel::MapPanel;
use crate::nodes::ship_controller::ShipController;
use crate::nodes::constants::{groups, theme, signals, methods, nodes};
use crate::nodes::live_handle::{LiveOpt, LiveRef, LiveVec};
use void_logic::radar::{self, BandRect};
use void_logic::ui_style;
use void_logic::damage_flash::DamageFlash;
use void_logic::run_state::HealthZone;

/// Health/shield bar dimensions. Shared by `build_hud` (background + fill) and
/// the `update_*` resizers so the two can't drift out of sync.
const BAR_WIDTH: f32 = 500.0;
const HEALTH_BAR_HEIGHT: f32 = 42.0;
const SHIELD_BAR_HEIGHT: f32 = 32.0;
/// Top-left inset for the absolutely-positioned bar fills.
const BAR_INSET: f32 = 20.0;

/// In SBS, the fraction of the full window to inset the HUD on each side. Each
/// eye sees the central ~50%, but its extreme edges are where an element is at
/// very different eccentricity per eye (one eye "reaching" across the nose),
/// which fights fusion and reads as blur. Insetting further than 0.25 pulls the
/// HUD into the comfortable central fusion zone. Tune toward 0.25 for more
/// spread, toward 0.4 for more central.
const SBS_SAFE_AREA_MARGIN: f32 = 0.33;

/// Fixed radar-arrow pool size (Faucet-style: allocated once at build, only
/// visibility flips per frame). More simultaneous off-screen enemies than
/// this just go unmarked — the first contacts in group order win, which is
/// stable frame to frame (the group holds the level's fixed roster).
const MAX_RADAR_ARROWS: usize = 16;

/// Valkyrie charge-row slot pool (Faucet-style like the arrows): bars past
/// this render no slot — the row is display, the ChargeState is the truth.
const MAX_VALKYRIE_SLOTS: usize = 8;
const VALK_SLOT_W: f32 = 34.0;
const VALK_SLOT_H: f32 = 10.0;
const VALK_SLOT_GAP: f32 = 4.0;

/// In-game HUD: health bar, credits, laser level, level number.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
#[allow(clippy::upper_case_acronyms)]
pub struct HUD {
    base: Base<CanvasLayer>,
    /// All HUD content lives under this. In SBS it shrinks to the central band
    /// each eye sees (controls anchor to the full 2x-wide window, so corner
    /// content would otherwise fall outside each eye); in mono it's full-bleed.
    safe_area: Option<LiveRef<Control>>,
    health_fill: Option<LiveRef<ColorRect>>,
    health_label: Option<LiveRef<Label>>,
    shield_fill: Option<LiveRef<ColorRect>>,
    shield_label: Option<LiveRef<Label>>,
    /// Shield Surge charges — blank until the item is owned and stocked.
    charges_label: Option<LiveRef<Label>>,
    power_mode_label: Option<LiveRef<Label>>,
    components_label: Option<LiveRef<Label>>,
    lives_label: Option<LiveRef<Label>>,
    organics_label: Option<LiveRef<Label>>,
    laser_label: Option<LiveRef<Label>>,
    level_label: Option<LiveRef<Label>>,
    laser_indicator: Option<LiveRef<ColorRect>>,
    slow_overlay: Option<LiveRef<ColorRect>>,
    slow_label: Option<LiveRef<Label>>,
    /// Permanent-unlock flags, pushed by GameManager (never self-invented):
    /// the radar arrows and the recon map render only while these are set.
    radar_unlocked: bool,
    map_unlocked: bool,
    /// The radar-arrow pool, children of `safe_area` so the SBS band confines
    /// them structurally.
    radar_arrows: LiveVec<Polygon2D>,
    /// Enemy instance ids in radar scope — the lit neighborhood, computed by
    /// LevelManager (RADAR_ROOM_DEPTH) and pushed by GameManager on room
    /// changes. The HUD draws ONLY these; empty means silent (playtest
    /// 2026-07-04: level-wide arrows are noise).
    radar_contacts: std::collections::HashSet<i64>,
    /// The recon-map corner widget (FogMap unlock), child of `safe_area`.
    map_panel: Option<LiveRef<MapPanel>>,
    /// Decaying screen damage tint (playtest 2026-07-06): amber on a held
    /// shield, red on a hull breach. The pure decay lives in void-logic;
    /// this overlay renders its current color+alpha each frame.
    damage_flash: DamageFlash,
    damage_tint: Option<LiveRef<ColorRect>>,
    /// The hull fraction the bar currently DISPLAYS (cached from the one
    /// `update_health` push) — the tint derives its zone from this same
    /// truth, so bar and tint can never disagree and no second health
    /// crossing exists. GameManager pushes before it flashes.
    health_fraction: f32,
    /// The Valkyrie charge row (playtest 2026-07-06): fixed slot pool of
    /// backgrounds + fills, children of `safe_area`; hidden until the
    /// cannon is owned, lighting left-to-right as the charge crosses.
    valkyrie_row: Option<LiveRef<Control>>,
    valkyrie_slots: LiveVec<ColorRect>,
    valkyrie_fills: LiveVec<ColorRect>,
}

#[godot_api]
impl ICanvasLayer for HUD {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            base,
            safe_area: None,
            health_fill: None,
            health_label: None,
            shield_fill: None,
            shield_label: None,
            charges_label: None,
            power_mode_label: None,
            components_label: None,
            lives_label: None,
            organics_label: None,
            laser_label: None,
            level_label: None,
            laser_indicator: None,
            slow_overlay: None,
            slow_label: None,
            radar_unlocked: false,
            map_unlocked: false,
            radar_arrows: LiveVec::new(),
            radar_contacts: std::collections::HashSet::new(),
            map_panel: None,
            damage_flash: DamageFlash::new(),
            damage_tint: None,
            // Full until the first push: a pathological pre-push flash
            // reads green and raises no alarm.
            health_fraction: 1.0,
            valkyrie_row: None,
            valkyrie_slots: LiveVec::new(),
            valkyrie_fills: LiveVec::new(),
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        self.build_hud();
        self.build_radar_arrows();
        self.build_map_panel();
        self.connect_options();
        self.base_mut().set_visible(false);
    }

    fn process(&mut self, delta: f64) {
        self.update_radar();
        self.update_valkyrie_row();
        self.update_damage_tint(delta as f32);
        // The map renders only while its unlock flag is pushed.
        let show_map = self.base().is_visible() && self.map_unlocked;
        self.map_panel.with(|panel| panel.set_visible(show_map));
    }
}

#[godot_api]
impl HUD {
    /// React to GameManager's options broadcast: confine the HUD to the central
    /// band each eye sees in SBS, or full-bleed in mono.
    #[func]
    fn on_options_changed(&mut self, sbs_enabled: bool, _msaa_enabled: bool) {
        self.apply_safe_area(sbs_enabled);
    }

    #[func]
    pub fn update_health(&mut self, current: f32, max: f32) {
        let fraction = (current / max).clamp(0.0, 1.0);
        self.health_fraction = fraction;

        self.health_fill.with(|fill| {
            fill.set_size(Vector2::new(BAR_WIDTH * fraction, HEALTH_BAR_HEIGHT));
            // The bar's color comes from the ONE zone scale the damage tint
            // also speaks (owner 2026-07-09) — they can never disagree.
            let color = match HealthZone::of(fraction) {
                HealthZone::Green => Color::from_rgb(0.2, 0.9, 0.2),
                HealthZone::Yellow => Color::from_rgb(0.9, 0.9, 0.2),
                HealthZone::Red => Color::from_rgb(0.9, 0.2, 0.2),
            };
            fill.set_color(color);
        });

        self.health_label
            .with(|label| label.set_text(&format!("{}/{}", current as i32, max as i32)));
    }

    #[func]
    pub fn update_shield(&mut self, current: f32, max: f32) {
        let fraction = if max > 0.0 { (current / max).clamp(0.0, 1.0) } else { 0.0 };

        self.shield_fill.with(|fill| {
            fill.set_size(Vector2::new(BAR_WIDTH * fraction, SHIELD_BAR_HEIGHT));
            // Blue to dark blue as shield depletes
            let brightness = 0.3 + fraction * 0.7;
            fill.set_color(Color::from_rgb(0.2 * brightness, 0.4 * brightness, brightness));
        });

        self.shield_label
            .with(|label| label.set_text(&format!("{}/{}", current as i32, max as i32)));
    }

    /// Update power routing mode display. 0=Balanced, 1=ShieldBoost, 2=WeaponBoost.
    #[func]
    pub fn update_power_mode(&mut self, mode: i32) {
        self.power_mode_label.with(|label| {
            let (text, color) = match mode {
                1 => ("SHIELDS", Color::from_rgb(0.3, 0.6, 1.0)),
                2 => ("WEAPONS", Color::from_rgb(1.0, 0.4, 0.2)),
                _ => ("", Color::from_rgba(0.5, 0.5, 0.5, 0.5)),
            };
            label.set_text(text);
            label.add_theme_color_override(theme::FONT_COLOR, color);
        });
    }

    #[func]
    pub fn update_components(&mut self, components: i64) {
        self.components_label
            .with(|label| label.set_text(&format!("Components: {}", components)));
    }

    /// Pushed by GameManager with the authoritative unlock state. Cached so
    /// the radar/map draw paths gate on it without asking anyone per frame.
    #[func]
    pub fn set_unlock_flags(&mut self, radar: bool, map: bool, tracker: bool) {
        self.radar_unlocked = radar;
        self.map_unlocked = map;
        self.map_panel.with(|panel| panel.bind_mut().set_tracker(tracker));
    }

    /// The radar's scope, pushed by GameManager on every room change: the
    /// instance ids of enemies in the lit neighborhood (LevelManager's
    /// `radar_contacts`, RADAR_ROOM_DEPTH). The HUD never widens this.
    #[func]
    pub fn set_radar_contacts(&mut self, ids: PackedInt64Array) {
        self.radar_contacts = ids.as_slice().iter().copied().collect();
        // The Threat Tracker draws the same neighborhood on the recon map.
        self.map_panel.with(|panel| panel.bind_mut().set_threats(ids.clone()));
    }

    /// Apply the HUD type ladder: an `ui_style` size plus the dark outline
    /// every HUD label wears so it reads against the scene behind it.
    fn style_hud_text(label: &mut Gd<Label>, size: i32) {
        label.add_theme_font_size_override(theme::FONT_SIZE, size);
        label.add_theme_constant_override(theme::OUTLINE_SIZE, ui_style::HUD_OUTLINE);
        let c = ui_style::HUD_OUTLINE_COLOR;
        label.add_theme_color_override(
            theme::FONT_OUTLINE_COLOR,
            Color::from_rgb(c[0], c[1], c[2]),
        );
    }

    /// Pre-build the fixed arrow pool, dormant, under the safe band: the SBS
    /// confinement is structural (band-local coordinates), and play only
    /// flips visibility.
    fn build_radar_arrows(&mut self) {
        let Some(safe_area) = &self.safe_area else { return };
        let mut arrows: Vec<Gd<Polygon2D>> = Vec::new();
        safe_area.with(|sa| {
            for _ in 0..MAX_RADAR_ARROWS {
                let mut arrow = Polygon2D::new_alloc();
                let mut points = PackedVector2Array::new();
                // Resting size — arrow_scale looms this up as contacts close.
                points.push(Vector2::new(22.0, 0.0));
                points.push(Vector2::new(-16.0, -14.0));
                points.push(Vector2::new(-16.0, 14.0));
                arrow.set_polygon(&points);
                arrow.set_visible(false);
                sa.add_child(&arrow);
                arrows.push(arrow);
            }
        });
        for arrow in &arrows {
            self.radar_arrows.push(arrow, ());
        }
    }

    /// Build the recon-map widget in the band's bottom-left corner: inside
    /// `safe_area` so SBS confinement is structural, sized as a fixed corner
    /// panel, hidden until the FogMap unlock flag arrives.
    fn build_map_panel(&mut self) {
        use godot::builtin::Side;
        let Some(safe_area) = &self.safe_area else { return };
        let mut panel = MapPanel::new_alloc();
        panel.set_anchor(Side::LEFT, 0.0);
        panel.set_anchor(Side::RIGHT, 0.0);
        panel.set_anchor(Side::TOP, 1.0);
        panel.set_anchor(Side::BOTTOM, 1.0);
        // Sized for legibility (playtest 2026-07-04: 200px was useless).
        panel.set_offset(Side::LEFT, 16.0);
        panel.set_offset(Side::RIGHT, 356.0);
        panel.set_offset(Side::TOP, -356.0);
        panel.set_offset(Side::BOTTOM, -16.0);
        panel.set_visible(false);
        safe_area.with(|sa| sa.add_child(&panel));
        self.map_panel = Some(LiveRef::new(&panel));
    }

    /// Relay a freshly derived map view (GameManager pushes one per newly
    /// visited room) to the corner widget.
    #[func]
    pub fn update_map(
        &mut self,
        rects: PackedFloat32Array,
        flags: PackedByteArray,
        projection: PackedFloat32Array,
    ) {
        self.map_panel.with(|panel| {
            panel.bind_mut().update_map(rects.clone(), flags.clone(), projection.clone());
        });
    }

    /// A hit landed — flash the screen tint at the struck layer's color
    /// (owner 2026-07-09): the tint is the HULL-severity alarm. A held
    /// shield raises none (the hull did not diminish); a breach tints at
    /// the zone the bar DISPLAYS — yellow when the bar is-or-goes yellow,
    /// red when it is-or-goes red, nothing while it stays green.
    /// GameManager pushes `update_health` before flashing, so the displayed
    /// fraction is the POST-hit one. The decay runs in `process`.
    #[func]
    pub fn flash_damage(&mut self, hull: bool) {
        if hull {
            self.damage_flash.strike_hull(HealthZone::of(self.health_fraction));
        }
    }

    /// Fade and render the damage tint. Alpha and color come from the pure
    /// `DamageFlash`; the overlay hides once it has faded to nothing.
    fn update_damage_tint(&mut self, delta: f32) {
        self.damage_flash.tick(delta);
        let visible = self.damage_flash.is_visible();
        let [r, g, b] = self.damage_flash.color();
        let a = self.damage_flash.alpha();
        self.damage_tint.with(|tint| {
            tint.set_visible(visible);
            tint.set_color(Color::from_rgba(r, g, b, a));
        });
    }

    /// Per-frame Valkyrie charge readout, pulled from the ship (the node
    /// owns the live charge; RunState owns the upgrades). The row hides
    /// with the HUD and while the cannon is unowned.
    fn update_valkyrie_row(&mut self) {
        let visible_base = self.base().is_visible();
        let player = self
            .base()
            .get_parent()
            .and_then(|p| p.try_get_node_as::<ShipController>(nodes::PLAYER));
        let (bars, charge) = match &player {
            Some(p) => {
                let p = p.bind();
                (
                    (p.valkyrie_bars().max(0) as usize).min(MAX_VALKYRIE_SLOTS),
                    p.valkyrie_charge(),
                )
            }
            None => (0, 0.0),
        };
        let show = visible_base && bars > 0;
        self.valkyrie_row.with(|row| {
            row.set_visible(show);
            // Re-center the visible span of the row under the reticle.
            let total_w =
                bars as f32 * VALK_SLOT_W + bars.saturating_sub(1) as f32 * VALK_SLOT_GAP;
            row.set_offset(godot::builtin::Side::LEFT, -total_w * 0.5);
        });
        self.valkyrie_slots.for_each_live(|i, slot, _| {
            slot.set_visible(i < bars);
        });
        self.valkyrie_fills.for_each_live(|i, fill, _| {
            fill.set_visible(i < bars);
            let frac = (charge - i as f32).clamp(0.0, 1.0);
            fill.set_size(Vector2::new(VALK_SLOT_W * frac, VALK_SLOT_H));
        });
    }

    /// Per-frame radar pass: project every live enemy through the player
    /// camera and pin an edge arrow (band-local math in `void_logic::radar`)
    /// for each one not visibly on screen. Dormant and room-culled enemies
    /// are skipped via `is_visible_in_tree` — the same authority that hides
    /// them. Gated on the Radar unlock flag GameManager pushes.
    fn update_radar(&mut self) {
        if !self.base().is_visible() || !self.radar_unlocked {
            self.radar_arrows.for_each_live(|_, arrow, _| arrow.set_visible(false));
            return;
        }
        let Some(parent) = self.base().get_parent() else { return };
        let Some(camera) = parent.try_get_node_as::<Camera3D>(nodes::PLAYER_CAMERA) else {
            return;
        };
        let Some(safe_area) = &self.safe_area else { return };

        // Band geometry from the safe-area control (already SBS-anchored).
        let mut band_origin = Vector2::ZERO;
        let mut band = BandRect { width: 0.0, height: 0.0 };
        safe_area.with(|sa| {
            let rect = sa.get_global_rect();
            band_origin = rect.position;
            band = BandRect { width: rect.size.x, height: rect.size.y };
        });
        if band.width <= 0.0 || band.height <= 0.0 {
            return;
        }

        let tree = self.base().get_tree();
        let camera_pos = camera.get_global_position();
        let mut placements: Vec<(radar::ArrowPlacement, f32)> = Vec::new();
        for node in tree.get_nodes_in_group(groups::ENEMIES).iter_shared() {
            let Ok(enemy) = node.try_cast::<Node3D>() else { continue };
            if !self.radar_contacts.contains(&enemy.instance_id().to_i64()) {
                continue; // outside the lit neighborhood — out of radar scope
            }
            if !enemy.is_visible_in_tree() {
                continue; // dormant minion or room-culled — not on the radar
            }
            let pos = enemy.get_global_position();
            let projected = camera.unproject_position(pos);
            let local = [projected.x - band_origin.x, projected.y - band_origin.y];
            let behind = camera.is_position_behind(pos);
            if let Some(placement) = radar::edge_arrow(local, behind, band) {
                placements.push((placement, camera_pos.distance_to(pos)));
                if placements.len() >= MAX_RADAR_ARROWS {
                    break;
                }
            }
        }

        self.radar_arrows.for_each_live(|i, arrow, _| {
            match placements.get(i) {
                Some((placement, distance)) => {
                    arrow.set_visible(true);
                    arrow.set_position(Vector2::new(placement.pos[0], placement.pos[1]));
                    arrow.set_rotation(placement.angle_rad);
                    // Near threats loom larger and burn hot; far ones rest
                    // small and fade — but never below legibility.
                    let scale = radar::arrow_scale(*distance);
                    arrow.set_scale(Vector2::new(scale, scale));
                    let fade = (1.0 - (distance - 10.0) / 80.0).clamp(0.35, 1.0);
                    arrow.set_color(Color::from_rgba(1.0, 0.35, 0.25, fade));
                }
                None => arrow.set_visible(false),
            }
        });
    }

    /// Shield Surge rack size. Zero blanks the line (unbought or spent —
    /// either way there is nothing to press).
    #[func]
    pub fn update_charges(&mut self, charges: i32) {
        self.charges_label.with(|label| {
            if charges > 0 {
                label.set_text(&format!("Surge ×{charges}"));
            } else {
                label.set_text("");
            }
        });
    }

    #[func]
    pub fn update_lives(&mut self, lives: i32) {
        self.lives_label
            .with(|label| label.set_text(&format!("Lives: {}", lives)));
    }

    #[func]
    pub fn update_organics(&mut self, organics: i64) {
        self.organics_label
            .with(|label| label.set_text(&format!("Organics: {}", organics)));
    }

    #[func]
    pub fn update_laser(&mut self, name: GString, color: Color) {
        self.laser_label.with(|label| {
            label.set_text(&format!("Laser: {}", name));
            label.add_theme_color_override(theme::FONT_COLOR, color);
        });
        self.laser_indicator.with(|indicator| indicator.set_color(color));
    }

    #[func]
    pub fn update_level(&mut self, level: i32) {
        self.level_label
            .with(|label| label.set_text(&format!("Level {}", level)));
    }

    /// Show/hide the "SLOWED" debuff indicator (red screen tint + label).
    #[func]
    pub fn update_slow(&mut self, active: bool) {
        self.slow_overlay.with(|overlay| overlay.set_visible(active));
        self.slow_label.with(|label| label.set_visible(active));
    }
}

impl HUD {
    /// Listen to GameManager's options broadcast so the HUD learns when SBS is
    /// on. GameManager is a sibling under Main; its deferred startup broadcast
    /// seeds the initial state after every node is ready.
    fn connect_options(&mut self) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut gm) = parent.try_get_node_as::<Node>(nodes::GAME_MANAGER) {
            let callable = self.base().callable(methods::ON_OPTIONS_CHANGED);
            if !gm.is_connected(signals::OPTIONS_CHANGED, &callable) {
                gm.connect(signals::OPTIONS_CHANGED, &callable);
            }
        }
    }

    /// SBS: shrink the HUD wrapper to the central 50% of the full (2x-wide)
    /// window — the band each eye actually sees — so corner bars land in view
    /// instead of off the side, and each eye's HUD matches the mono screen.
    /// Mono: full-bleed. Controls anchor to the full window regardless
    /// (custom_viewport redirects rendering, not layout), so this re-anchors the
    /// single wrapper everything hangs off rather than each element.
    fn apply_safe_area(&self, sbs: bool) {
        use godot::builtin::Side;
        self.safe_area.with(|sa| {
            if sbs {
                sa.set_anchor(Side::LEFT, SBS_SAFE_AREA_MARGIN);
                sa.set_anchor(Side::TOP, 0.0);
                sa.set_anchor(Side::RIGHT, 1.0 - SBS_SAFE_AREA_MARGIN);
                sa.set_anchor(Side::BOTTOM, 1.0);
                for side in [Side::LEFT, Side::TOP, Side::RIGHT, Side::BOTTOM] {
                    sa.set_offset(side, 0.0);
                }
            } else {
                sa.set_anchors_preset(LayoutPreset::FULL_RECT);
            }
        });
    }

    fn build_hud(&mut self) {
        // Everything hangs off this so SBS can pull the whole HUD into the
        // central band each eye sees (see `apply_safe_area`). Full-bleed here;
        // the OPTIONS_CHANGED broadcast narrows it when SBS is on.
        let mut safe_area = Control::new_alloc();
        safe_area.set_anchors_preset(LayoutPreset::FULL_RECT);
        safe_area.set_mouse_filter(godot::classes::control::MouseFilter::IGNORE);

        // === Top-left: Health + Credits ===
        let mut top_left = VBoxContainer::new_alloc();
        top_left.set_anchors_preset(LayoutPreset::TOP_LEFT);
        top_left.set_offset(godot::builtin::Side::LEFT, 20.0);
        top_left.set_offset(godot::builtin::Side::TOP, 20.0);

        // Health bar row
        let mut health_row = HBoxContainer::new_alloc();

        let mut health_bg = ColorRect::new_alloc();
        health_bg.set_custom_minimum_size(Vector2::new(BAR_WIDTH, HEALTH_BAR_HEIGHT));
        health_bg.set_color(Color::from_rgba(0.2, 0.2, 0.2, 0.7));
        health_row.add_child(&health_bg);

        // Health fill overlaid on top of bg (we'll position it absolutely)
        // For simplicity, use a separate ColorRect that gets resized
        let mut health_fill = ColorRect::new_alloc();
        // set_size, NOT custom_minimum_size: the fill is a free (non-container)
        // child, and a minimum size would floor it so update_health's
        // set_size(width * fraction) could never shrink it — the bar would
        // recolor on damage but never shorten.
        health_fill.set_size(Vector2::new(BAR_WIDTH, HEALTH_BAR_HEIGHT));
        health_fill.set_color(Color::from_rgb(0.2, 0.9, 0.2));
        // Place fill at same position as bg (overlapping)
        health_fill.set_position(Vector2::new(0.0, 0.0));

        let mut health_label = Label::new_alloc();
        health_label.set_text("100/100");
        Self::style_hud_text(&mut health_label, ui_style::FONT_HUD_PRIMARY);
        health_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.9, 0.9, 0.9));
        health_row.add_child(&health_label);

        top_left.add_child(&health_row);

        // We need health_fill overlapping the bg — add it to the CanvasLayer directly
        // and position it relative to the top_left container
        self.health_fill = Some(LiveRef::new(&health_fill));
        self.health_label = Some(LiveRef::new(&health_label));

        // Shield bar row (below health)
        let mut shield_row = HBoxContainer::new_alloc();

        let mut shield_bg = ColorRect::new_alloc();
        shield_bg.set_custom_minimum_size(Vector2::new(BAR_WIDTH, SHIELD_BAR_HEIGHT));
        shield_bg.set_color(Color::from_rgba(0.1, 0.1, 0.3, 0.7));
        shield_row.add_child(&shield_bg);

        let mut shield_fill = ColorRect::new_alloc();
        // set_size, not custom_minimum_size — see health_fill above.
        shield_fill.set_size(Vector2::new(BAR_WIDTH, SHIELD_BAR_HEIGHT));
        shield_fill.set_color(Color::from_rgb(0.2, 0.4, 1.0));
        shield_fill.set_position(Vector2::new(0.0, 0.0));

        let mut shield_label = Label::new_alloc();
        shield_label.set_text("50/50");
        Self::style_hud_text(&mut shield_label, ui_style::FONT_HUD_LABEL);
        shield_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.5, 0.7, 1.0));
        shield_row.add_child(&shield_label);

        top_left.add_child(&shield_row);

        self.shield_fill = Some(LiveRef::new(&shield_fill));
        self.shield_label = Some(LiveRef::new(&shield_label));

        // Shield Surge charges (below the shield it refills).
        let mut charges_label = Label::new_alloc();
        charges_label.set_text("");
        Self::style_hud_text(&mut charges_label, ui_style::FONT_HUD_LABEL);
        charges_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.5, 0.7, 1.0));
        top_left.add_child(&charges_label);
        self.charges_label = Some(LiveRef::new(&charges_label));

        // Power mode indicator (below shield bar)
        let mut power_mode_label = Label::new_alloc();
        power_mode_label.set_text("");
        Self::style_hud_text(&mut power_mode_label, ui_style::FONT_HUD_FINE);
        power_mode_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgba(0.5, 0.5, 0.5, 0.5));
        top_left.add_child(&power_mode_label);
        self.power_mode_label = Some(LiveRef::new(&power_mode_label));

        // Components (in-run currency)
        let mut components_label = Label::new_alloc();
        components_label.set_text("Components: 0");
        Self::style_hud_text(&mut components_label, ui_style::FONT_HUD_LABEL);
        components_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_COMPONENTS));
        top_left.add_child(&components_label);
        self.components_label = Some(LiveRef::new(&components_label));

        // Lives, alongside the salvage they'll be spent protecting.
        let mut lives_label = Label::new_alloc();
        lives_label.set_text("Lives: 1");
        Self::style_hud_text(&mut lives_label, ui_style::FONT_HUD_LABEL);
        lives_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_SECONDARY));
        top_left.add_child(&lives_label);
        self.lives_label = Some(LiveRef::new(&lives_label));

        // Organics (permanent currency)
        let mut organics_label = Label::new_alloc();
        organics_label.set_text("Organics: 0");
        Self::style_hud_text(&mut organics_label, ui_style::FONT_HUD_LABEL);
        organics_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_ORGANICS));
        top_left.add_child(&organics_label);
        self.organics_label = Some(LiveRef::new(&organics_label));

        safe_area.add_child(&top_left);
        // Add health fill as overlay on CanvasLayer, positioned at top-left
        health_fill.set_position(Vector2::new(BAR_INSET, BAR_INSET));
        safe_area.add_child(&health_fill);
        // Shield fill overlay just below the (now taller) health bar.
        shield_fill.set_position(Vector2::new(BAR_INSET, BAR_INSET + HEALTH_BAR_HEIGHT + 4.0));
        safe_area.add_child(&shield_fill);

        // === Top-right: Laser info + Level ===
        let mut top_right = VBoxContainer::new_alloc();
        top_right.set_anchors_preset(LayoutPreset::TOP_RIGHT);
        top_right.set_offset(godot::builtin::Side::RIGHT, -20.0);
        top_right.set_offset(godot::builtin::Side::TOP, 20.0);
        top_right.set_offset(godot::builtin::Side::LEFT, -200.0);

        // Laser row
        let mut laser_row = HBoxContainer::new_alloc();

        let mut laser_indicator = ColorRect::new_alloc();
        laser_indicator.set_custom_minimum_size(Vector2::new(16.0, 16.0));
        laser_indicator.set_color(Color::from_rgb(1.0, 0.2, 0.2));
        laser_row.add_child(&laser_indicator);
        self.laser_indicator = Some(LiveRef::new(&laser_indicator));

        let mut laser_label = Label::new_alloc();
        laser_label.set_text("Laser: Red");
        Self::style_hud_text(&mut laser_label, ui_style::FONT_HUD_LABEL);
        laser_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(1.0, 0.2, 0.2));
        laser_row.add_child(&laser_label);
        self.laser_label = Some(LiveRef::new(&laser_label));

        top_right.add_child(&laser_row);

        // Level
        let mut level_label = Label::new_alloc();
        level_label.set_text("Level 1");
        Self::style_hud_text(&mut level_label, ui_style::FONT_HUD_LABEL);
        level_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_SECONDARY));
        top_right.add_child(&level_label);
        self.level_label = Some(LiveRef::new(&level_label));

        safe_area.add_child(&top_right);

        // === Bottom center: the Valkyrie charge row (playtest 2026-07-06) ===
        // Fixed slot pool (Faucet-style like the radar arrows): only
        // visibility and fill widths change per frame. Hidden until the
        // cannon is owned; slots light left-to-right as the one charge
        // crosses the row.
        let mut valk_row = Control::new_alloc();
        valk_row.set_name("ValkyrieChargeRow");
        valk_row.set_anchors_preset(LayoutPreset::CENTER_BOTTOM);
        valk_row.set_offset(godot::builtin::Side::TOP, -40.0);
        valk_row.set_visible(false);
        for i in 0..MAX_VALKYRIE_SLOTS {
            let x = i as f32 * (VALK_SLOT_W + VALK_SLOT_GAP);
            let mut bg = ColorRect::new_alloc();
            bg.set_name(&format!("ValkyrieSlot{i}"));
            bg.set_color(Color::from_rgba(0.10, 0.12, 0.16, 0.85));
            bg.set_size(Vector2::new(VALK_SLOT_W, VALK_SLOT_H));
            bg.set_position(Vector2::new(x, 0.0));
            bg.set_visible(false);
            valk_row.add_child(&bg);
            self.valkyrie_slots.push(&bg, ());
            let mut fill = ColorRect::new_alloc();
            fill.set_name(&format!("ValkyrieFill{i}"));
            fill.set_color(Color::from_rgb(1.0, 0.72, 0.25));
            fill.set_size(Vector2::new(0.0, VALK_SLOT_H));
            fill.set_position(Vector2::new(x, 0.0));
            fill.set_visible(false);
            valk_row.add_child(&fill);
            self.valkyrie_fills.push(&fill, ());
        }
        safe_area.add_child(&valk_row);
        self.valkyrie_row = Some(LiveRef::new(&valk_row));

        // The center targeting reticle no longer lives in the HUD: a
        // fixed-depth sight on the UI plane doubles against nearer
        // geometry in stereo (occlusion-vergence rivalry). It is
        // world-space now, on the camera's aim axis at the aim ray's
        // depth — see ShipController::spawn_reticle.

        // === Damage tint (hidden until a hit lands; amber shield / red hull) ===
        // A deep full-screen tinge that fades over ~5s (playtest 2026-07-06).
        // Sits under the slow overlay so a swarmer slow reads over a fresh hit.
        // FULL-SCREEN WASHES PARENT TO THE WINDOW, NOT THE BAND (playtest
        // 2026-07-09: inside the SBS safe area they render as a center
        // stripe) — the band exists to pull positioned chrome into each
        // eye's view; a wash must tint everything the eye sees. Added to the
        // layer BEFORE the band, so the chrome stays readable over a flash.
        let mut damage_tint = ColorRect::new_alloc();
        damage_tint.set_name("DamageTint");
        damage_tint.set_anchors_preset(LayoutPreset::FULL_RECT);
        damage_tint.set_color(Color::from_rgba(0.0, 0.0, 0.0, 0.0));
        damage_tint.set_mouse_filter(godot::classes::control::MouseFilter::IGNORE);
        damage_tint.set_visible(false);
        self.base_mut().add_child(&damage_tint);
        self.damage_tint = Some(LiveRef::new(&damage_tint));

        // === Slow debuff indicator (hidden until a swarmer slows the player) ===
        let mut slow_overlay = ColorRect::new_alloc();
        slow_overlay.set_name("SlowOverlay");
        slow_overlay.set_anchors_preset(LayoutPreset::FULL_RECT);
        slow_overlay.set_color(Color::from_rgba(0.7, 0.1, 0.1, 0.16));
        slow_overlay.set_mouse_filter(godot::classes::control::MouseFilter::IGNORE);
        slow_overlay.set_visible(false);
        self.base_mut().add_child(&slow_overlay);
        self.slow_overlay = Some(LiveRef::new(&slow_overlay));

        let mut slow_label = Label::new_alloc();
        slow_label.set_anchors_preset(LayoutPreset::CENTER_TOP);
        slow_label.set_offset(godot::builtin::Side::TOP, 80.0);
        slow_label.set_text("SLOWED");
        Self::style_hud_text(&mut slow_label, ui_style::FONT_HUD_PRIMARY);
        slow_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(1.0, 0.4, 0.4));
        slow_label.set_visible(false);
        safe_area.add_child(&slow_label);
        self.slow_label = Some(LiveRef::new(&slow_label));

        self.base_mut().add_child(&safe_area);
        self.safe_area = Some(LiveRef::new(&safe_area));
    }
}
