use godot::prelude::*;
use godot::global::Key;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, Control,
    Engine, InputEvent, InputEventKey,
};

use super::menu_panel;

/// Post-level kill summary: shows enemy types killed and credits earned.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct KillSummaryUI {
    base: Base<CanvasLayer>,
}

#[godot_api]
impl ICanvasLayer for KillSummaryUI {
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
            self.base_mut().emit_signal("continue_pressed", &[]);
        }
    }
}

#[godot_api]
impl KillSummaryUI {
    #[signal]
    fn continue_pressed();

    /// Populate and show the summary screen.
    #[func]
    pub fn show_summary(&mut self, kill_data: Dictionary, total_credits: i64, level: i32) {
        for mut child in self.base().get_children().iter_shared() {
            child.queue_free();
        }

        // Semi-transparent overlay for ship showcase visibility
        let overlay = menu_panel::create_showcase_overlay();
        self.base_mut().add_child(&overlay);

        let (panel, mut vbox) = menu_panel::create_menu_panel();

        // Title
        let mut title = Label::new_alloc();
        title.set_text(&format!("LEVEL {} COMPLETE", level));
        title.add_theme_font_size_override("font_size", 48);
        title.add_theme_color_override("font_color", Color::from_rgb(0.3, 1.0, 0.3));
        vbox.add_child(&title);

        // Spacer
        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 20.0));
        vbox.add_child(&spacer);

        // Kill list header
        let mut header = Label::new_alloc();
        header.set_text("ENEMIES DEFEATED");
        header.add_theme_font_size_override("font_size", 28);
        header.add_theme_color_override("font_color", Color::from_rgb(0.7, 0.7, 0.8));
        vbox.add_child(&header);

        // Each enemy type
        for key in kill_data.keys_array().iter_shared() {
            let name = key.to::<GString>();
            let count = kill_data.get_or_nil(key.clone()).to::<i32>();
            let mut row = Label::new_alloc();
            row.set_text(&format!("  {} x {}", name, count));
            row.add_theme_font_size_override("font_size", 24);
            row.add_theme_color_override("font_color", Color::from_rgb(0.8, 0.8, 0.9));
            vbox.add_child(&row);
        }

        // Spacer
        let mut spacer2 = Control::new_alloc();
        spacer2.set_custom_minimum_size(Vector2::new(0.0, 20.0));
        vbox.add_child(&spacer2);

        // Credits
        let mut credits_label = Label::new_alloc();
        credits_label.set_text(&format!("TOTAL CREDITS: {}", total_credits));
        credits_label.add_theme_font_size_override("font_size", 32);
        credits_label.add_theme_color_override("font_color", Color::from_rgb(1.0, 0.85, 0.2));
        vbox.add_child(&credits_label);

        // Spacer
        let mut spacer3 = Control::new_alloc();
        spacer3.set_custom_minimum_size(Vector2::new(0.0, 30.0));
        vbox.add_child(&spacer3);

        // Continue prompt
        let mut prompt = Label::new_alloc();
        prompt.set_text("Press ENTER to continue");
        prompt.add_theme_font_size_override("font_size", 22);
        prompt.add_theme_color_override("font_color", Color::from_rgb(0.5, 0.5, 0.6));
        vbox.add_child(&prompt);

        self.base_mut().add_child(&panel);
        self.base_mut().set_visible(true);
    }
}
