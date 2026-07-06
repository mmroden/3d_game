use godot::prelude::*;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, Control,
    Engine, Input,
};

use super::menu_panel;
use crate::nodes::constants::{actions, signals, theme};
use void_logic::ui_style;

/// Which death this screen is showing: the run ending (back to the menu) or
/// a spare life spent (on to the shop, then back into the level).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeathMode {
    RunOver,
    LifeLost,
}

/// Death screen: run over shows the penalty and returns to the menu; a life
/// lost shows the lives left and re-arms at the shop.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct DeathScreenUI {
    base: Base<CanvasLayer>,
    mode: DeathMode,
}

#[godot_api]
impl ICanvasLayer for DeathScreenUI {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self { base, mode: DeathMode::RunOver }
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
        if input.is_action_just_pressed(actions::MENU_SELECT) {
            let signal = match self.mode {
                DeathMode::RunOver => signals::RETURN_PRESSED,
                DeathMode::LifeLost => signals::RESPAWN_PRESSED,
            };
            self.base_mut().emit_signal(signal, &[]);
        }
    }
}

#[godot_api]
impl DeathScreenUI {
    #[signal]
    fn return_pressed();

    #[signal]
    fn respawn_pressed();

    /// Show the life-lost variant: the run continues — through the shop and
    /// back into the same level.
    #[func]
    pub fn show_life_lost(&mut self, lives_left: i32, level: i32) {
        self.mode = DeathMode::LifeLost;
        for mut child in self.base().get_children().iter_shared() {
            child.queue_free();
        }

        let mut overlay = godot::classes::ColorRect::new_alloc();
        overlay.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        overlay.set_color(Color::from_rgba(0.08, 0.02, 0.02, void_logic::ui_style::SHOWCASE_BG_ALPHA));
        self.base_mut().add_child(&overlay);

        let (panel, mut vbox) = menu_panel::create_menu_panel();

        let mut title = Label::new_alloc();
        title.set_text("LIFE LOST");
        title.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_TITLE);
        title.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(1.0, 0.5, 0.2));
        vbox.add_child(&title);

        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 30.0));
        vbox.add_child(&spacer);

        let mut lives_label = Label::new_alloc();
        let plural = if lives_left == 1 { "life" } else { "lives" };
        lives_label.set_text(&format!("{} {} remaining — level {} restarts", lives_left, plural, level));
        lives_label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_BODY);
        lives_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_SECONDARY));
        vbox.add_child(&lives_label);

        let mut kept_label = Label::new_alloc();
        kept_label.set_text("Salvage and upgrades kept");
        kept_label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_BODY);
        kept_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_UNSELECTED));
        vbox.add_child(&kept_label);

        let mut spacer2 = Control::new_alloc();
        spacer2.set_custom_minimum_size(Vector2::new(0.0, 40.0));
        vbox.add_child(&spacer2);

        let mut prompt = Label::new_alloc();
        prompt.set_text("Press ENTER to re-arm at the shop");
        prompt.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_DETAIL);
        prompt.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_UNSELECTED));
        vbox.add_child(&prompt);

        self.base_mut().add_child(&panel);
        self.base_mut().set_visible(true);
    }

    /// Show the run-over variant: penalties applied, back to the menu.
    #[func]
    pub fn show_death(&mut self, laser_name: GString, downgraded_to: GString, level_reached: i32) {
        self.mode = DeathMode::RunOver;
        for mut child in self.base().get_children().iter_shared() {
            child.queue_free();
        }

        // Semi-transparent overlay for ship showcase visibility (dark red tint)
        let mut overlay = godot::classes::ColorRect::new_alloc();
        overlay.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        overlay.set_color(Color::from_rgba(0.08, 0.02, 0.02, void_logic::ui_style::SHOWCASE_BG_ALPHA));
        self.base_mut().add_child(&overlay);

        let (panel, mut vbox) = menu_panel::create_menu_panel();

        let mut title = Label::new_alloc();
        title.set_text("MISSION FAILED");
        title.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_TITLE);
        title.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(1.0, 0.2, 0.2));
        vbox.add_child(&title);

        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 30.0));
        vbox.add_child(&spacer);

        let mut level_label = Label::new_alloc();
        level_label.set_text(&format!("Reached Level {}", level_reached));
        level_label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_BODY);
        level_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_SECONDARY));
        vbox.add_child(&level_label);

        let mut penalty_label = Label::new_alloc();
        penalty_label.set_text(&format!(
            "Laser downgraded: {} -> {}",
            laser_name, downgraded_to
        ));
        penalty_label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_BODY);
        penalty_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(1.0, 0.6, 0.2));
        vbox.add_child(&penalty_label);

        // Blue purchases die with the run — say so, so it reads as design.
        let mut salvage_label = Label::new_alloc();
        salvage_label.set_text("Salvage and bought upgrades lost");
        salvage_label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_BODY);
        salvage_label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(1.0, 0.6, 0.2));
        vbox.add_child(&salvage_label);

        let mut spacer2 = Control::new_alloc();
        spacer2.set_custom_minimum_size(Vector2::new(0.0, 40.0));
        vbox.add_child(&spacer2);

        let mut prompt = Label::new_alloc();
        prompt.set_text("Press ENTER to return to base");
        prompt.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_DETAIL);
        prompt.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_UNSELECTED));
        vbox.add_child(&prompt);

        self.base_mut().add_child(&panel);
        self.base_mut().set_visible(true);
    }
}
