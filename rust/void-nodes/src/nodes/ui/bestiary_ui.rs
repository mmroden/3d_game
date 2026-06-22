use godot::prelude::*;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, Control,
    Engine, Input,
    text_server::AutowrapMode,
};

use super::menu_panel;
use crate::nodes::constants::{actions, signals, theme};
use void_logic::ui_style;

/// Pre-level bestiary briefing: a quiet loadout room with one subject — a
/// currency pickup or a catalogued enemy — spinning in the middle (driven by
/// GameManager via the shared Turntable), and a low panel with its name and
/// lore. The player taps to step through the catalog; the final tap drops them
/// into the level. GameManager owns the paging; this is just the panel + input.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct BestiaryUI {
    base: Base<CanvasLayer>,
    /// Brief input lockout after the screen appears, so the same button press
    /// that opened the briefing (ship-select's Continue) can't bleed through and
    /// instantly start the mission.
    input_cooldown: f32,
}

#[godot_api]
impl ICanvasLayer for BestiaryUI {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self { base, input_cooldown: 0.0 }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        self.base_mut().set_visible(false);
    }

    fn process(&mut self, delta: f64) {
        if !self.base().is_visible() {
            return;
        }
        if self.input_cooldown > 0.0 {
            self.input_cooldown = (self.input_cooldown - delta as f32).max(0.0);
            return;
        }
        let input = Input::singleton();
        // The menu buttons (leftmost d-pad / arrows) step between catalogued
        // subjects — the same navigation every other menu uses, never the left
        // stick; Select/Fire begins the mission. GameManager owns the index and
        // clamps the ends.
        if input.is_action_just_pressed(actions::MENU_UP) {
            self.base_mut().emit_signal(signals::BESTIARY_PAGED, &[Variant::from(-1_i32)]);
        } else if input.is_action_just_pressed(actions::MENU_DOWN) {
            self.base_mut().emit_signal(signals::BESTIARY_PAGED, &[Variant::from(1_i32)]);
        } else if input.is_action_just_pressed(actions::MENU_SELECT)
            || input.is_action_just_pressed(actions::FIRE)
        {
            self.base_mut().emit_signal(signals::CONTINUE_PRESSED, &[]);
        }
    }
}

#[godot_api]
impl BestiaryUI {
    #[signal]
    fn continue_pressed();

    #[signal]
    fn bestiary_paged(delta: i32);

    /// Populate the panel for one entry and show the screen. `position` reads
    /// like "1 / 3"; `hint` is the call to action ("▲ next ▼" / "Begin mission").
    /// Lock input for a beat. Called by GameManager when the briefing is first
    /// entered (NOT on page refresh), so the ship-select press that opened this
    /// screen can't bleed through and instantly start the mission. Decoupled
    /// from visibility because `show_phase` makes the layer visible before the
    /// first `show_bestiary` runs, which would defeat a self-check.
    #[func]
    pub fn begin_briefing(&mut self) {
        self.input_cooldown = 0.25;
    }

    /// Whether input is currently locked out (entry debounce in effect).
    #[func]
    pub fn input_locked(&self) -> bool {
        self.input_cooldown > 0.0
    }

    #[func]
    pub fn show_bestiary(&mut self, title: GString, blurb: GString, position: GString, hint: GString) {
        for mut child in self.base().get_children().iter_shared() {
            child.queue_free();
        }

        let overlay = menu_panel::create_showcase_overlay();
        self.base_mut().add_child(&overlay);
        let (mut panel, mut vbox) = menu_panel::create_menu_panel();
        // Sit the panel low so the spinning subject stays clear in the middle,
        // matching the loadout screen's framing.
        menu_panel::seat_panel_low(&mut panel);

        let mut position_label = Label::new_alloc();
        position_label.set_text(&position.to_string());
        position_label.add_theme_font_size_override(theme::FONT_SIZE, 20);
        position_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_UNSELECTED));
        vbox.add_child(&position_label);

        let mut title_label = Label::new_alloc();
        title_label.set_text(&title.to_string());
        title_label.add_theme_font_size_override(theme::FONT_SIZE, 44);
        title_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.6, 0.9, 1.0));
        vbox.add_child(&title_label);

        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 16.0));
        vbox.add_child(&spacer);

        let mut blurb_label = Label::new_alloc();
        blurb_label.set_text(&blurb.to_string());
        blurb_label.add_theme_font_size_override(theme::FONT_SIZE, 24);
        blurb_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_UNSELECTED));
        // Lore is a paragraph — wrap it and cap the width so it stays readable.
        blurb_label.set_autowrap_mode(AutowrapMode::WORD_SMART);
        blurb_label.set_custom_minimum_size(Vector2::new(720.0, 0.0));
        vbox.add_child(&blurb_label);

        let mut spacer2 = Control::new_alloc();
        spacer2.set_custom_minimum_size(Vector2::new(0.0, 20.0));
        vbox.add_child(&spacer2);

        let mut hint_label = Label::new_alloc();
        hint_label.set_text(&hint.to_string());
        hint_label.add_theme_font_size_override(theme::FONT_SIZE, 26);
        hint_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_SELECTED));
        vbox.add_child(&hint_label);

        self.base_mut().add_child(&panel);
        self.base_mut().set_visible(true);
    }

    #[func]
    pub fn hide_bestiary(&mut self) {
        self.base_mut().set_visible(false);
    }
}
