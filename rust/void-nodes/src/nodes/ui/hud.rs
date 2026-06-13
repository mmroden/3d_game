use godot::prelude::*;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, ColorRect, HBoxContainer, VBoxContainer, Control,
    Engine,
    control::LayoutPreset,
};

use crate::nodes::constants::theme;
use void_logic::ui_style;

/// In-game HUD: health bar, credits, laser level, level number.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
#[allow(clippy::upper_case_acronyms)]
pub struct HUD {
    base: Base<CanvasLayer>,
    health_fill: Option<Gd<ColorRect>>,
    health_label: Option<Gd<Label>>,
    shield_fill: Option<Gd<ColorRect>>,
    shield_label: Option<Gd<Label>>,
    power_mode_label: Option<Gd<Label>>,
    credits_label: Option<Gd<Label>>,
    laser_label: Option<Gd<Label>>,
    level_label: Option<Gd<Label>>,
    laser_indicator: Option<Gd<ColorRect>>,
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
            credits_label: None,
            laser_label: None,
            level_label: None,
            laser_indicator: None,
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

        if let Some(fill) = &mut self.health_fill {
            if fill.is_instance_valid() {
                fill.set_size(Vector2::new(200.0 * fraction, 20.0));
                let color = if fraction > 0.5 {
                    Color::from_rgb(0.2, 0.9, 0.2)
                } else if fraction > 0.25 {
                    Color::from_rgb(0.9, 0.9, 0.2)
                } else {
                    Color::from_rgb(0.9, 0.2, 0.2)
                };
                fill.set_color(color);
            }
        }

        if let Some(label) = &mut self.health_label {
            if label.is_instance_valid() {
                label.set_text(&format!("{}/{}", current as i32, max as i32));
            }
        }
    }

    #[func]
    pub fn update_shield(&mut self, current: f32, max: f32) {
        let fraction = if max > 0.0 { (current / max).clamp(0.0, 1.0) } else { 0.0 };

        if let Some(fill) = &mut self.shield_fill {
            if fill.is_instance_valid() {
                fill.set_size(Vector2::new(200.0 * fraction, 14.0));
                // Blue to dark blue as shield depletes
                let brightness = 0.3 + fraction * 0.7;
                fill.set_color(Color::from_rgb(0.2 * brightness, 0.4 * brightness, brightness));
            }
        }

        if let Some(label) = &mut self.shield_label {
            if label.is_instance_valid() {
                label.set_text(&format!("{}/{}", current as i32, max as i32));
            }
        }
    }

    /// Update power routing mode display. 0=Balanced, 1=ShieldBoost, 2=WeaponBoost.
    #[func]
    pub fn update_power_mode(&mut self, mode: i32) {
        if let Some(label) = &mut self.power_mode_label {
            if label.is_instance_valid() {
                let (text, color) = match mode {
                    1 => ("SHIELDS", Color::from_rgb(0.3, 0.6, 1.0)),
                    2 => ("WEAPONS", Color::from_rgb(1.0, 0.4, 0.2)),
                    _ => ("", Color::from_rgba(0.5, 0.5, 0.5, 0.5)),
                };
                label.set_text(text);
                label.add_theme_color_override(theme::FONT_COLOR, color);
            }
        }
    }

    #[func]
    pub fn update_credits(&mut self, credits: i64) {
        if let Some(label) = &mut self.credits_label {
            if label.is_instance_valid() {
                label.set_text(&format!("Credits: {}", credits));
            }
        }
    }

    #[func]
    pub fn update_laser(&mut self, name: GString, color: Color) {
        if let Some(label) = &mut self.laser_label {
            if label.is_instance_valid() {
                label.set_text(&format!("Laser: {}", name));
                label.add_theme_color_override(theme::FONT_COLOR, color);
            }
        }
        if let Some(indicator) = &mut self.laser_indicator {
            if indicator.is_instance_valid() {
                indicator.set_color(color);
            }
        }
    }

    #[func]
    pub fn update_level(&mut self, level: i32) {
        if let Some(label) = &mut self.level_label {
            if label.is_instance_valid() {
                label.set_text(&format!("Level {}", level));
            }
        }
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
        health_bg.set_custom_minimum_size(Vector2::new(200.0, 20.0));
        health_bg.set_color(Color::from_rgba(0.2, 0.2, 0.2, 0.7));
        health_row.add_child(&health_bg);

        // Health fill overlaid on top of bg (we'll position it absolutely)
        // For simplicity, use a separate ColorRect that gets resized
        let mut health_fill = ColorRect::new_alloc();
        health_fill.set_custom_minimum_size(Vector2::new(200.0, 20.0));
        health_fill.set_color(Color::from_rgb(0.2, 0.9, 0.2));
        // Place fill at same position as bg (overlapping)
        health_fill.set_position(Vector2::new(0.0, 0.0));

        let mut health_label = Label::new_alloc();
        health_label.set_text("100/100");
        health_label.add_theme_font_size_override(theme::FONT_SIZE, 18);
        health_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.9, 0.9, 0.9));
        health_row.add_child(&health_label);

        top_left.add_child(&health_row);

        // We need health_fill overlapping the bg — add it to the CanvasLayer directly
        // and position it relative to the top_left container
        self.health_fill = Some(health_fill.clone());
        self.health_label = Some(health_label);

        // Shield bar row (below health)
        let mut shield_row = HBoxContainer::new_alloc();

        let mut shield_bg = ColorRect::new_alloc();
        shield_bg.set_custom_minimum_size(Vector2::new(200.0, 14.0));
        shield_bg.set_color(Color::from_rgba(0.1, 0.1, 0.3, 0.7));
        shield_row.add_child(&shield_bg);

        let mut shield_fill = ColorRect::new_alloc();
        shield_fill.set_custom_minimum_size(Vector2::new(200.0, 14.0));
        shield_fill.set_color(Color::from_rgb(0.2, 0.4, 1.0));
        shield_fill.set_position(Vector2::new(0.0, 0.0));

        let mut shield_label = Label::new_alloc();
        shield_label.set_text("50/50");
        shield_label.add_theme_font_size_override(theme::FONT_SIZE, 14);
        shield_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.5, 0.7, 1.0));
        shield_row.add_child(&shield_label);

        top_left.add_child(&shield_row);

        self.shield_fill = Some(shield_fill.clone());
        self.shield_label = Some(shield_label);

        // Power mode indicator (below shield bar)
        let mut power_mode_label = Label::new_alloc();
        power_mode_label.set_text("");
        power_mode_label.add_theme_font_size_override(theme::FONT_SIZE, 16);
        power_mode_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgba(0.5, 0.5, 0.5, 0.5));
        top_left.add_child(&power_mode_label);
        self.power_mode_label = Some(power_mode_label);

        // Credits
        let mut credits_label = Label::new_alloc();
        credits_label.set_text("Credits: 0");
        credits_label.add_theme_font_size_override(theme::FONT_SIZE, 18);
        credits_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_CREDITS));
        top_left.add_child(&credits_label);
        self.credits_label = Some(credits_label);

        self.base_mut().add_child(&top_left);
        // Add health fill as overlay on CanvasLayer, positioned at top-left
        health_fill.set_position(Vector2::new(20.0, 20.0));
        self.base_mut().add_child(&health_fill);
        // Shield fill overlay below health
        shield_fill.set_position(Vector2::new(20.0, 44.0));
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
        self.laser_indicator = Some(laser_indicator);

        let mut laser_label = Label::new_alloc();
        laser_label.set_text("Laser: Red");
        laser_label.add_theme_font_size_override(theme::FONT_SIZE, 18);
        laser_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(1.0, 0.2, 0.2));
        laser_row.add_child(&laser_label);
        self.laser_label = Some(laser_label);

        top_right.add_child(&laser_row);

        // Level
        let mut level_label = Label::new_alloc();
        level_label.set_text("Level 1");
        level_label.add_theme_font_size_override(theme::FONT_SIZE, 18);
        level_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_SECONDARY));
        top_right.add_child(&level_label);
        self.level_label = Some(level_label);

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
    }
}
