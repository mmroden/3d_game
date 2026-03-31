use godot::prelude::*;
use godot::global::Key;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, Control,
    Engine, InputEvent, InputEventKey,
};

use super::menu_panel;
use void_logic::menu_cursor::MenuCursor;
use void_logic::ui_style;

/// Upgrade shop between levels: buy laser upgrades with credits.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct ShopUI {
    base: Base<CanvasLayer>,
    cursor: MenuCursor,
    labels: Vec<Gd<Label>>,
}

#[godot_api]
impl ICanvasLayer for ShopUI {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            base,
            cursor: MenuCursor::new(2),
            labels: Vec::new(),
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        self.base_mut().set_visible(false);
    }

    fn input(&mut self, event: Gd<InputEvent>) {
        if !self.base().is_visible() {
            return;
        }
        let Ok(key_event) = event.try_cast::<InputEventKey>() else { return };
        if !key_event.is_pressed() || key_event.is_echo() {
            return;
        }

        match key_event.get_keycode() {
            Key::UP | Key::W => {
                self.cursor.move_up();
                self.update_cursor();
            }
            Key::DOWN | Key::S => {
                self.cursor.move_down();
                self.update_cursor();
            }
            Key::ENTER | Key::SPACE => {
                match self.cursor.index() {
                    0 => {
                        self.base_mut().emit_signal("buy_pressed", &[]);
                    }
                    1 => {
                        self.base_mut().emit_signal("continue_pressed", &[]);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

#[godot_api]
impl ShopUI {
    #[signal]
    fn buy_pressed();

    #[signal]
    fn continue_pressed();

    /// Populate and show the shop screen.
    #[func]
    #[allow(clippy::too_many_arguments)]
    pub fn show_shop(
        &mut self,
        credits: i64,
        laser_name: GString,
        laser_color: Color,
        laser_damage: f32,
        next_cost: i64,
        can_afford: bool,
        is_max: bool,
    ) {
        for mut child in self.base().get_children().iter_shared() {
            child.queue_free();
        }

        self.labels.clear();
        self.cursor.reset();

        // Semi-transparent overlay for ship showcase visibility
        let overlay = menu_panel::create_showcase_overlay();
        self.base_mut().add_child(&overlay);

        let (panel, mut vbox) = menu_panel::create_menu_panel();

        // Title
        let mut title = Label::new_alloc();
        title.set_text("UPGRADE STATION");
        title.add_theme_font_size_override("font_size", 48);
        title.add_theme_color_override("font_color", Color::from_rgb(0.8, 0.6, 1.0));
        vbox.add_child(&title);

        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 20.0));
        vbox.add_child(&spacer);

        // Current laser info
        let mut current = Label::new_alloc();
        current.set_text(&format!(
            "Current Laser: {} (Damage: {})",
            laser_name, laser_damage as i32
        ));
        current.add_theme_font_size_override("font_size", 28);
        current.add_theme_color_override("font_color", laser_color);
        vbox.add_child(&current);

        // Credits
        let mut credits_label = Label::new_alloc();
        credits_label.set_text(&format!("Credits: {}", credits));
        credits_label.add_theme_font_size_override("font_size", 28);
        credits_label.add_theme_color_override("font_color", super::rgb(ui_style::TEXT_CREDITS));
        vbox.add_child(&credits_label);

        let mut spacer2 = Control::new_alloc();
        spacer2.set_custom_minimum_size(Vector2::new(0.0, 30.0));
        vbox.add_child(&spacer2);

        // Upgrade option
        let upgrade_text = if is_max {
            "  LASER MAXED OUT".to_string()
        } else if can_afford {
            format!("> Upgrade Laser ({} credits)", next_cost)
        } else {
            format!("  Upgrade Laser ({} credits) [NOT ENOUGH]", next_cost)
        };
        let mut upgrade_label = Label::new_alloc();
        upgrade_label.set_text(&upgrade_text);
        upgrade_label.add_theme_font_size_override("font_size", 28);
        let upgrade_color = if is_max {
            Color::from_rgb(0.4, 0.4, 0.5)
        } else if can_afford {
            super::rgb(ui_style::TEXT_SELECTED)
        } else {
            Color::from_rgb(0.6, 0.3, 0.3)
        };
        upgrade_label.add_theme_color_override("font_color", upgrade_color);
        vbox.add_child(&upgrade_label);
        self.labels.push(upgrade_label);

        // Continue
        let mut continue_label = Label::new_alloc();
        continue_label.set_text("  Continue to Next Level");
        continue_label.add_theme_font_size_override("font_size", 28);
        continue_label.add_theme_color_override("font_color", super::rgb(ui_style::TEXT_UNSELECTED));
        vbox.add_child(&continue_label);
        self.labels.push(continue_label);

        self.base_mut().add_child(&panel);
        self.base_mut().set_visible(true);
        self.update_cursor();
    }

    fn update_cursor(&mut self) {
        for (i, label) in self.labels.iter_mut().enumerate() {
            if !label.is_instance_valid() {
                continue;
            }
            let color = if i == self.cursor.index() {
                super::rgb(ui_style::TEXT_SELECTED)
            } else {
                super::rgb(ui_style::TEXT_UNSELECTED)
            };
            label.add_theme_color_override("font_color", color);
        }
    }
}
