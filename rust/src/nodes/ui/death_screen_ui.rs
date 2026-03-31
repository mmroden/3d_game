use godot::prelude::*;
use godot::global::Key;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, Control,
    Engine, InputEvent, InputEventKey,
};

use super::menu_panel;
use crate::systems::ui_style;

/// Death screen: shows stats and penalty, press key to return to menu.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct DeathScreenUI {
    base: Base<CanvasLayer>,
}

#[godot_api]
impl ICanvasLayer for DeathScreenUI {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self { base }
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
        if key_event.get_keycode() == Key::ENTER || key_event.get_keycode() == Key::SPACE {
            self.base_mut().emit_signal("return_pressed", &[]);
        }
    }
}

#[godot_api]
impl DeathScreenUI {
    #[signal]
    fn return_pressed();

    #[func]
    pub fn show_death(&mut self, laser_name: GString, downgraded_to: GString, level_reached: i32) {
        for mut child in self.base().get_children().iter_shared() {
            child.queue_free();
        }

        // Semi-transparent overlay for ship showcase visibility (dark red tint)
        let mut overlay = godot::classes::ColorRect::new_alloc();
        overlay.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        overlay.set_color(Color::from_rgba(0.08, 0.02, 0.02, crate::systems::ui_style::SHOWCASE_BG_ALPHA));
        self.base_mut().add_child(&overlay);

        let (panel, mut vbox) = menu_panel::create_menu_panel();

        let mut title = Label::new_alloc();
        title.set_text("MISSION FAILED");
        title.add_theme_font_size_override("font_size", 56);
        title.add_theme_color_override("font_color", Color::from_rgb(1.0, 0.2, 0.2));
        vbox.add_child(&title);

        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 30.0));
        vbox.add_child(&spacer);

        let mut level_label = Label::new_alloc();
        level_label.set_text(&format!("Reached Level {}", level_reached));
        level_label.add_theme_font_size_override("font_size", 28);
        level_label.add_theme_color_override("font_color", super::rgb(ui_style::TEXT_SECONDARY));
        vbox.add_child(&level_label);

        let mut penalty_label = Label::new_alloc();
        penalty_label.set_text(&format!(
            "Laser downgraded: {} -> {}",
            laser_name, downgraded_to
        ));
        penalty_label.add_theme_font_size_override("font_size", 28);
        penalty_label.add_theme_color_override("font_color", Color::from_rgb(1.0, 0.6, 0.2));
        vbox.add_child(&penalty_label);

        let mut spacer2 = Control::new_alloc();
        spacer2.set_custom_minimum_size(Vector2::new(0.0, 40.0));
        vbox.add_child(&spacer2);

        let mut prompt = Label::new_alloc();
        prompt.set_text("Press ENTER to return to base");
        prompt.add_theme_font_size_override("font_size", 22);
        prompt.add_theme_color_override("font_color", super::rgb(ui_style::TEXT_UNSELECTED));
        vbox.add_child(&prompt);

        self.base_mut().add_child(&panel);
        self.base_mut().set_visible(true);
    }
}
