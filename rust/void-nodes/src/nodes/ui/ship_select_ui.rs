use godot::prelude::*;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, Control,
    Engine, Input,
    control::{LayoutPreset, GrowDirection},
};

use super::menu_panel;
use crate::nodes::constants::{actions, signals, theme};
use crate::nodes::live_handle::LiveVec;
use void_logic::menu_cursor::MenuCursor;
use void_logic::ship::ShipColor;
use void_logic::ui_style;

/// Between-level loadout screen: pick a ship color, each a real stat tradeoff.
/// Colors apply live (the showcase recolors); Continue starts the level.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct ShipSelectUI {
    base: Base<CanvasLayer>,
    cursor: MenuCursor,
    labels: LiveVec<Label>,
    /// Currently applied ship color id (for the selection marker).
    selected_id: i32,
}

#[godot_api]
impl ICanvasLayer for ShipSelectUI {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            base,
            // One row per color, plus Continue.
            cursor: MenuCursor::new(ShipColor::ALL.len() + 1),
            labels: LiveVec::new(),
            selected_id: 0,
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        self.base_mut().set_visible(false);
    }

    fn process(&mut self, _delta: f64) {
        if !self.base().is_visible() {
            return;
        }
        let input = Input::singleton();

        if input.is_action_just_pressed(actions::MENU_UP) {
            self.cursor.move_up();
            self.update_cursor();
        } else if input.is_action_just_pressed(actions::MENU_DOWN) {
            self.cursor.move_down();
            self.update_cursor();
        } else if input.is_action_just_pressed(actions::MENU_SELECT) {
            let index = self.cursor.index();
            if index < ShipColor::ALL.len() {
                // Apply this color (game_manager recolors ship + showcase).
                self.selected_id = index as i32;
                self.base_mut()
                    .emit_signal(signals::SHIP_COLOR_SELECTED, &[Variant::from(index as i32)]);
                self.build_options();
            } else {
                self.base_mut().emit_signal(signals::CONTINUE_PRESSED, &[]);
            }
        }
    }
}

#[godot_api]
impl ShipSelectUI {
    #[signal]
    fn ship_color_selected(id: i32);

    #[signal]
    fn continue_pressed();

    /// Show the screen, marking `current_id` as the active color.
    #[func]
    pub fn show_ship_select(&mut self, current_id: i32) {
        self.selected_id = current_id;
        self.build_options();
        self.base_mut().set_visible(true);
    }

    fn build_options(&mut self) {
        for mut child in self.base().get_children().iter_shared() {
            child.queue_free();
        }
        self.labels.clear();

        let overlay = menu_panel::create_showcase_overlay();
        self.base_mut().add_child(&overlay);
        let (mut panel, mut vbox) = menu_panel::create_menu_panel();
        // Sit the loadout panel low so the rotating ship stays clear in the
        // middle of the screen, rather than centred behind the panel.
        panel.set_anchors_preset(LayoutPreset::CENTER_BOTTOM);
        panel.set_v_grow_direction(GrowDirection::BEGIN);
        panel.set_offset(godot::builtin::Side::BOTTOM, -50.0);

        let mut title = Label::new_alloc();
        title.set_text("SHIP LOADOUT");
        title.add_theme_font_size_override(theme::FONT_SIZE, 48);
        title.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.6, 0.9, 1.0));
        vbox.add_child(&title);

        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 24.0));
        vbox.add_child(&spacer);

        // One row per color.
        for variant in ShipColor::ALL {
            let c = variant.color();
            let chosen = variant.id() == self.selected_id;
            let mark = if chosen { "● " } else { "  " };
            let mut label = Label::new_alloc();
            label.set_text(&format!("{}{} — {}", mark, variant.display_name(), variant.blurb()));
            label.add_theme_font_size_override(theme::FONT_SIZE, 28);
            label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgba(c[0], c[1], c[2], c[3]));
            vbox.add_child(&label);
            self.labels.push(&label, ());
        }

        let mut spacer2 = Control::new_alloc();
        spacer2.set_custom_minimum_size(Vector2::new(0.0, 24.0));
        vbox.add_child(&spacer2);

        let mut continue_label = Label::new_alloc();
        continue_label.set_text("  Continue");
        continue_label.add_theme_font_size_override(theme::FONT_SIZE, 28);
        continue_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_UNSELECTED));
        vbox.add_child(&continue_label);
        self.labels.push(&continue_label, ());

        self.base_mut().add_child(&panel);
        self.update_cursor();
    }

    fn update_cursor(&mut self) {
        let selected = self.cursor.index();
        self.labels.for_each_live(|i, label, _| {
            if i == selected {
                label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_SELECTED));
            } else if i < ShipColor::ALL.len() {
                // Restore the color's own hue when not under the cursor.
                let c = ShipColor::from_id(i as i32).unwrap_or_default().color();
                label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgba(c[0], c[1], c[2], c[3]));
            } else {
                label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_UNSELECTED));
            }
        });
    }
}
