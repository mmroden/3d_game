use godot::prelude::*;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, ColorRect,
    Engine,
};

/// In-game HUD: health bar, credits, laser level, level number.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct HUD {
    base: Base<CanvasLayer>,
    health_fill: Option<Gd<ColorRect>>,
    health_label: Option<Gd<Label>>,
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
                label.add_theme_color_override("font_color", color);
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
        // Top-left: Health
        let mut health_bg = ColorRect::new_alloc();
        health_bg.set_position(Vector2::new(20.0, 20.0));
        health_bg.set_size(Vector2::new(200.0, 20.0));
        health_bg.set_color(Color::from_rgba(0.2, 0.2, 0.2, 0.7));
        self.base_mut().add_child(&health_bg);

        let mut health_fill = ColorRect::new_alloc();
        health_fill.set_position(Vector2::new(20.0, 20.0));
        health_fill.set_size(Vector2::new(200.0, 20.0));
        health_fill.set_color(Color::from_rgb(0.2, 0.9, 0.2));
        self.base_mut().add_child(&health_fill);
        self.health_fill = Some(health_fill);

        let mut health_label = Label::new_alloc();
        health_label.set_position(Vector2::new(230.0, 16.0));
        health_label.set_text("100/100");
        health_label.add_theme_font_size_override("font_size", 18);
        health_label.add_theme_color_override("font_color", Color::from_rgb(0.9, 0.9, 0.9));
        self.base_mut().add_child(&health_label);
        self.health_label = Some(health_label);

        // Top-left below health: Credits
        let mut credits_label = Label::new_alloc();
        credits_label.set_position(Vector2::new(20.0, 48.0));
        credits_label.set_text("Credits: 0");
        credits_label.add_theme_font_size_override("font_size", 18);
        credits_label.add_theme_color_override("font_color", Color::from_rgb(1.0, 0.85, 0.2));
        self.base_mut().add_child(&credits_label);
        self.credits_label = Some(credits_label);

        // Top-right: Laser info
        let mut laser_indicator = ColorRect::new_alloc();
        laser_indicator.set_position(Vector2::new(1720.0, 20.0));
        laser_indicator.set_size(Vector2::new(16.0, 16.0));
        laser_indicator.set_color(Color::from_rgb(1.0, 0.2, 0.2));
        self.base_mut().add_child(&laser_indicator);
        self.laser_indicator = Some(laser_indicator);

        let mut laser_label = Label::new_alloc();
        laser_label.set_position(Vector2::new(1740.0, 16.0));
        laser_label.set_text("Laser: Red");
        laser_label.add_theme_font_size_override("font_size", 18);
        laser_label.add_theme_color_override("font_color", Color::from_rgb(1.0, 0.2, 0.2));
        self.base_mut().add_child(&laser_label);
        self.laser_label = Some(laser_label);

        // Top-right: Level
        let mut level_label = Label::new_alloc();
        level_label.set_position(Vector2::new(1800.0, 48.0));
        level_label.set_text("Level 1");
        level_label.add_theme_font_size_override("font_size", 18);
        level_label.add_theme_color_override("font_color", Color::from_rgb(0.7, 0.7, 0.8));
        self.base_mut().add_child(&level_label);
        self.level_label = Some(level_label);

        // Bottom center: Controls reminder (fades eventually)
        let mut controls = Label::new_alloc();
        controls.set_position(Vector2::new(650.0, 1040.0));
        controls.set_text("WASD: Move | Arrows: Look | Space: Fire | R/F: Up/Down");
        controls.add_theme_font_size_override("font_size", 16);
        controls.add_theme_color_override("font_color", Color::from_rgba(0.5, 0.5, 0.6, 0.7));
        self.base_mut().add_child(&controls);
    }
}
