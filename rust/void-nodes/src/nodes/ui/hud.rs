use godot::prelude::*;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, ColorRect, HBoxContainer, VBoxContainer, Control,
    Engine,
    control::LayoutPreset,
};

use crate::nodes::constants::theme;
use crate::nodes::live_handle::{LiveOpt, LiveRef};
use void_logic::ui_style;

/// Health/shield bar dimensions. Shared by `build_hud` (background + fill) and
/// the `update_*` resizers so the two can't drift out of sync.
const BAR_WIDTH: f32 = 360.0;
const HEALTH_BAR_HEIGHT: f32 = 30.0;
const SHIELD_BAR_HEIGHT: f32 = 22.0;
/// Top-left inset for the absolutely-positioned bar fills.
const BAR_INSET: f32 = 20.0;

/// In-game HUD: health bar, credits, laser level, level number.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
#[allow(clippy::upper_case_acronyms)]
pub struct HUD {
    base: Base<CanvasLayer>,
    health_fill: Option<LiveRef<ColorRect>>,
    health_label: Option<LiveRef<Label>>,
    shield_fill: Option<LiveRef<ColorRect>>,
    shield_label: Option<LiveRef<Label>>,
    power_mode_label: Option<LiveRef<Label>>,
    components_label: Option<LiveRef<Label>>,
    organics_label: Option<LiveRef<Label>>,
    laser_label: Option<LiveRef<Label>>,
    level_label: Option<LiveRef<Label>>,
    laser_indicator: Option<LiveRef<ColorRect>>,
    slow_overlay: Option<LiveRef<ColorRect>>,
    slow_label: Option<LiveRef<Label>>,
}

#[godot_api]
impl ICanvasLayer for HUD {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            base,
            health_fill: None,
            health_label: None,
            shield_fill: None,
            shield_label: None,
            power_mode_label: None,
            components_label: None,
            organics_label: None,
            laser_label: None,
            level_label: None,
            laser_indicator: None,
            slow_overlay: None,
            slow_label: None,
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        self.build_hud();
        self.base_mut().set_visible(false);
    }
}

#[godot_api]
impl HUD {
    #[func]
    pub fn update_health(&mut self, current: f32, max: f32) {
        let fraction = (current / max).clamp(0.0, 1.0);

        self.health_fill.with(|fill| {
            fill.set_size(Vector2::new(BAR_WIDTH * fraction, HEALTH_BAR_HEIGHT));
            let color = if fraction > 0.5 {
                Color::from_rgb(0.2, 0.9, 0.2)
            } else if fraction > 0.25 {
                Color::from_rgb(0.9, 0.9, 0.2)
            } else {
                Color::from_rgb(0.9, 0.2, 0.2)
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
    fn build_hud(&mut self) {
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
        health_fill.set_custom_minimum_size(Vector2::new(BAR_WIDTH, HEALTH_BAR_HEIGHT));
        health_fill.set_color(Color::from_rgb(0.2, 0.9, 0.2));
        // Place fill at same position as bg (overlapping)
        health_fill.set_position(Vector2::new(0.0, 0.0));

        let mut health_label = Label::new_alloc();
        health_label.set_text("100/100");
        health_label.add_theme_font_size_override(theme::FONT_SIZE, 22);
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
        shield_fill.set_custom_minimum_size(Vector2::new(BAR_WIDTH, SHIELD_BAR_HEIGHT));
        shield_fill.set_color(Color::from_rgb(0.2, 0.4, 1.0));
        shield_fill.set_position(Vector2::new(0.0, 0.0));

        let mut shield_label = Label::new_alloc();
        shield_label.set_text("50/50");
        shield_label.add_theme_font_size_override(theme::FONT_SIZE, 18);
        shield_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.5, 0.7, 1.0));
        shield_row.add_child(&shield_label);

        top_left.add_child(&shield_row);

        self.shield_fill = Some(LiveRef::new(&shield_fill));
        self.shield_label = Some(LiveRef::new(&shield_label));

        // Power mode indicator (below shield bar)
        let mut power_mode_label = Label::new_alloc();
        power_mode_label.set_text("");
        power_mode_label.add_theme_font_size_override(theme::FONT_SIZE, 16);
        power_mode_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgba(0.5, 0.5, 0.5, 0.5));
        top_left.add_child(&power_mode_label);
        self.power_mode_label = Some(LiveRef::new(&power_mode_label));

        // Components (in-run currency)
        let mut components_label = Label::new_alloc();
        components_label.set_text("Components: 0");
        components_label.add_theme_font_size_override(theme::FONT_SIZE, 18);
        components_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_COMPONENTS));
        top_left.add_child(&components_label);
        self.components_label = Some(LiveRef::new(&components_label));

        // Organics (permanent currency)
        let mut organics_label = Label::new_alloc();
        organics_label.set_text("Organics: 0");
        organics_label.add_theme_font_size_override(theme::FONT_SIZE, 18);
        organics_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_ORGANICS));
        top_left.add_child(&organics_label);
        self.organics_label = Some(LiveRef::new(&organics_label));

        self.base_mut().add_child(&top_left);
        // Add health fill as overlay on CanvasLayer, positioned at top-left
        health_fill.set_position(Vector2::new(BAR_INSET, BAR_INSET));
        self.base_mut().add_child(&health_fill);
        // Shield fill overlay just below the (now taller) health bar.
        shield_fill.set_position(Vector2::new(BAR_INSET, BAR_INSET + HEALTH_BAR_HEIGHT + 4.0));
        self.base_mut().add_child(&shield_fill);

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
        laser_label.add_theme_font_size_override(theme::FONT_SIZE, 18);
        laser_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(1.0, 0.2, 0.2));
        laser_row.add_child(&laser_label);
        self.laser_label = Some(LiveRef::new(&laser_label));

        top_right.add_child(&laser_row);

        // Level
        let mut level_label = Label::new_alloc();
        level_label.set_text("Level 1");
        level_label.add_theme_font_size_override(theme::FONT_SIZE, 18);
        level_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_SECONDARY));
        top_right.add_child(&level_label);
        self.level_label = Some(LiveRef::new(&level_label));

        self.base_mut().add_child(&top_right);

        // === Bottom center: Controls reminder ===
        let mut bottom_center = Control::new_alloc();
        bottom_center.set_anchors_preset(LayoutPreset::CENTER_BOTTOM);
        bottom_center.set_offset(godot::builtin::Side::TOP, -40.0);
        bottom_center.set_offset(godot::builtin::Side::LEFT, -300.0);
        bottom_center.set_offset(godot::builtin::Side::RIGHT, 300.0);

        let mut controls = Label::new_alloc();
        controls.set_text("WASD: Move | Arrows: Look | Space: Fire | R/F: Up/Down");
        controls.add_theme_font_size_override(theme::FONT_SIZE, 16);
        controls.add_theme_color_override(theme::FONT_COLOR, Color::from_rgba(
            ui_style::TEXT_UNSELECTED[0], ui_style::TEXT_UNSELECTED[1], ui_style::TEXT_UNSELECTED[2], 0.7,
        ));
        bottom_center.add_child(&controls);

        self.base_mut().add_child(&bottom_center);

        // === Center targeting reticle (dot + crosshair) ===
        let reticle_color = Color::from_rgba(0.5, 1.0, 0.6, 0.85);
        let mut reticle = Control::new_alloc();
        reticle.set_anchors_preset(LayoutPreset::CENTER);

        let mut dot = ColorRect::new_alloc();
        dot.set_color(reticle_color);
        dot.set_size(Vector2::new(4.0, 4.0));
        dot.set_position(Vector2::new(-2.0, -2.0));
        reticle.add_child(&dot);

        // Four ticks around a center gap: (size, position) relative to center.
        let ticks = [
            (Vector2::new(9.0, 2.0), Vector2::new(-18.0, -1.0)), // left
            (Vector2::new(9.0, 2.0), Vector2::new(9.0, -1.0)),   // right
            (Vector2::new(2.0, 9.0), Vector2::new(-1.0, -18.0)), // up
            (Vector2::new(2.0, 9.0), Vector2::new(-1.0, 9.0)),   // down
        ];
        for (size, posn) in ticks {
            let mut tick = ColorRect::new_alloc();
            tick.set_color(reticle_color);
            tick.set_size(size);
            tick.set_position(posn);
            reticle.add_child(&tick);
        }
        self.base_mut().add_child(&reticle);

        // === Slow debuff indicator (hidden until a swarmer slows the player) ===
        let mut slow_overlay = ColorRect::new_alloc();
        slow_overlay.set_anchors_preset(LayoutPreset::FULL_RECT);
        slow_overlay.set_color(Color::from_rgba(0.7, 0.1, 0.1, 0.16));
        slow_overlay.set_visible(false);
        self.base_mut().add_child(&slow_overlay);
        self.slow_overlay = Some(LiveRef::new(&slow_overlay));

        let mut slow_label = Label::new_alloc();
        slow_label.set_anchors_preset(LayoutPreset::CENTER_TOP);
        slow_label.set_offset(godot::builtin::Side::TOP, 80.0);
        slow_label.set_text("SLOWED");
        slow_label.add_theme_font_size_override(theme::FONT_SIZE, 30);
        slow_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(1.0, 0.4, 0.4));
        slow_label.set_visible(false);
        self.base_mut().add_child(&slow_label);
        self.slow_label = Some(LiveRef::new(&slow_label));
    }
}
