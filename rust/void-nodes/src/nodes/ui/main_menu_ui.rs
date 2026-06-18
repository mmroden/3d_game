use godot::prelude::*;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, Control, Engine, Input, Node,
};

use super::menu_panel;
use crate::nodes::constants::{actions, methods, nodes, signals, theme};
use crate::nodes::live_handle::LiveVec;
use void_logic::game_options::GameOptions;
use void_logic::menu_cursor::MenuCursor;
use void_logic::ui_style;

/// FF-style main menu: Continue / New Game / Options / Exit.
/// Pure view — emits signals for all actions, never reaches into the scene tree.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct MainMenuUI {
    base: Base<CanvasLayer>,
    cursor: MenuCursor,
    menu_items: Vec<String>,
    labels: LiveVec<Label>,
    in_options: bool,
    option_cursor: MenuCursor,
    option_labels: LiveVec<Label>,
    /// Cached copy of the authoritative `GameOptions`, seeded from
    /// GameManager at startup and updated on `options_changed`. One
    /// type, one default — no second literal to drift out of sync.
    options: GameOptions,
}

#[godot_api]
impl ICanvasLayer for MainMenuUI {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            base,
            cursor: MenuCursor::new_at(1, 4), // Default to New Game
            menu_items: vec![
                "Continue".to_string(),
                "New Game".to_string(),
                "Options".to_string(),
                "Exit".to_string(),
            ],
            labels: LiveVec::new(),
            in_options: false,
            option_cursor: MenuCursor::new(3),
            option_labels: LiveVec::new(),
            options: GameOptions::default(),
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        self.build_ui();
        self.connect_to_game_manager();
    }

    fn process(&mut self, _delta: f64) {
        if !self.base().is_visible() {
            return;
        }

        let input = Input::singleton();

        if self.in_options {
            self.handle_options_actions(&input);
        } else {
            self.handle_menu_actions(&input);
        }
    }
}

#[godot_api]
impl MainMenuUI {
    #[signal]
    fn new_game_selected();

    #[signal]
    fn continue_selected();

    #[signal]
    fn exit_selected();

    #[signal]
    fn sbs_toggled();

    #[signal]
    fn msaa_toggled();

    /// Called by GameManager to update displayed option states.
    #[func]
    pub fn set_option_states(&mut self, sbs_on: bool, msaa_on: bool) {
        self.options.sbs_enabled = sbs_on;
        self.options.msaa_enabled = msaa_on;
        if self.in_options {
            self.refresh_options();
        }
    }

    /// Test/inspection seam: the MSAA state this menu would display,
    /// which must always equal GameManager's authoritative option.
    #[func]
    pub fn displayed_msaa(&self) -> bool {
        self.options.msaa_enabled
    }

    /// Called when GameManager emits options_changed signal.
    #[func]
    pub fn on_options_changed(&mut self, sbs_enabled: bool, msaa_enabled: bool) {
        self.set_option_states(sbs_enabled, msaa_enabled);
    }
}

impl MainMenuUI {
    fn connect_to_game_manager(&mut self) {
        // MainMenuUI is a child of Main, GameManager is also a child of Main
        let Some(parent) = self.base().get_parent() else {
            godot_warn!("MainMenuUI: could not find parent");
            return;
        };
        if let Some(game_mgr) = parent.try_get_node_as::<Node>(nodes::GAME_MANAGER) {
            let callable = self.base().callable(methods::ON_OPTIONS_CHANGED);
            if !game_mgr.is_connected(signals::OPTIONS_CHANGED, &callable) {
                let mut gm = game_mgr;
                gm.connect(signals::OPTIONS_CHANGED, &callable);
            }
        } else {
            godot_warn!("MainMenuUI: GameManager not found");
        }
    }

    fn build_ui(&mut self) {
        // Dark overlay + a low panel so the showcase ship (and, later, live
        // action) shows above the menu — the same framing as the ship-select
        // and bestiary screens.
        let overlay = menu_panel::create_showcase_overlay();
        self.base_mut().add_child(&overlay);
        let (mut panel, mut vbox) = menu_panel::create_menu_panel();
        menu_panel::seat_panel_low(&mut panel);

        // Title
        let mut title = Label::new_alloc();
        title.set_text("VOID SCAVENGER");
        title.add_theme_font_size_override(theme::FONT_SIZE, 64);
        title.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.6, 0.8, 1.0));
        vbox.add_child(&title);

        // Subtitle
        let mut subtitle = Label::new_alloc();
        subtitle.set_text("6DOF Roguelike Space Shooter");
        subtitle.add_theme_font_size_override(theme::FONT_SIZE, 20);
        subtitle.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.4, 0.5, 0.7));
        vbox.add_child(&subtitle);

        // Spacer
        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 40.0));
        vbox.add_child(&spacer);

        // Menu items
        self.labels.clear();
        for (i, item) in self.menu_items.iter().enumerate() {
            let mut label = Label::new_alloc();
            let text = if i == self.cursor.index() {
                format!("> {}", item)
            } else {
                format!("  {}", item)
            };
            label.set_text(&text);
            label.add_theme_font_size_override(theme::FONT_SIZE, 32);
            let color = if i == self.cursor.index() {
                super::rgb(ui_style::TEXT_SELECTED)
            } else {
                super::rgb(ui_style::TEXT_UNSELECTED)
            };
            label.add_theme_color_override(theme::FONT_COLOR, color);
            vbox.add_child(&label);
            self.labels.push(&label, ());
        }

        self.base_mut().add_child(&panel);
    }

    fn handle_menu_actions(&mut self, input: &Gd<Input>) {
        if input.is_action_just_pressed(actions::MENU_UP) {
            self.cursor.move_up();
            self.update_cursor();
        } else if input.is_action_just_pressed(actions::MENU_DOWN) {
            self.cursor.move_down();
            self.update_cursor();
        } else if input.is_action_just_pressed(actions::MENU_SELECT) {
            self.select_item();
        }
    }

    fn select_item(&mut self) {
        match self.cursor.index() {
            0 => {
                self.base_mut().emit_signal(signals::CONTINUE_SELECTED, &[]);
            }
            1 => {
                self.base_mut().emit_signal(signals::NEW_GAME_SELECTED, &[]);
            }
            2 => {
                self.in_options = true;
                self.option_cursor.reset();
                self.show_options();
            }
            3 => {
                self.base_mut().emit_signal(signals::EXIT_SELECTED, &[]);
                self.base().get_tree().quit();
            }
            _ => {}
        }
    }

    fn update_cursor(&mut self) {
        let selected = self.cursor.index();
        let items = &self.menu_items;
        self.labels.for_each_live(|i, label, _| {
            let color = if i == selected {
                super::rgb(ui_style::TEXT_SELECTED)
            } else {
                super::rgb(ui_style::TEXT_UNSELECTED)
            };
            label.add_theme_color_override(theme::FONT_COLOR, color);

            if i == selected {
                label.set_text(&format!("> {}", items[i]));
            } else {
                label.set_text(&format!("  {}", items[i]));
            }
        });
    }

    fn show_options(&mut self) {
        // Free any lingering option labels from a prior entry.
        // queue_free() is deferred, so guard against rapid re-entry.
        self.option_labels.for_each_live(|_, label, _| label.queue_free());
        self.option_labels.clear();

        self.labels.for_each_live(|_, label, _| label.set_visible(false));
        let Some(mut parent) = self.labels.get_live(0).and_then(|l| l.get_parent()) else {
            return;
        };

        let options = [
            format!("  SBS Stereo: {}", if self.options.sbs_enabled { "ON" } else { "OFF" }),
            format!("  MSAA: {}", if self.options.msaa_enabled { "ON" } else { "OFF" }),
            "  Back".to_string(),
        ];

        for (i, text) in options.iter().enumerate() {
            let mut label = Label::new_alloc();
            let display = if i == self.option_cursor.index() {
                format!("> {}", text.trim())
            } else {
                text.clone()
            };
            label.set_text(&display);
            label.add_theme_font_size_override(theme::FONT_SIZE, 32);
            let color = if i == self.option_cursor.index() {
                super::rgb(ui_style::TEXT_SELECTED)
            } else {
                super::rgb(ui_style::TEXT_UNSELECTED)
            };
            label.add_theme_color_override(theme::FONT_COLOR, color);
            parent.add_child(&label);
            self.option_labels.push(&label, ());
        }
    }

    fn handle_options_actions(&mut self, input: &Gd<Input>) {
        if input.is_action_just_pressed(actions::MENU_UP) {
            self.option_cursor.move_up();
            self.refresh_options();
        } else if input.is_action_just_pressed(actions::MENU_DOWN) {
            self.option_cursor.move_down();
            self.refresh_options();
        } else if input.is_action_just_pressed(actions::MENU_SELECT) {
            match self.option_cursor.index() {
                0 => {
                    self.base_mut().emit_signal(signals::SBS_TOGGLED, &[]);
                }
                1 => {
                    self.base_mut().emit_signal(signals::MSAA_TOGGLED, &[]);
                }
                2 => {
                    self.close_options();
                }
                _ => {}
            }
        } else if input.is_action_just_pressed(actions::MENU_BACK) {
            self.close_options();
        }
    }

    fn refresh_options(&mut self) {
        let texts = [
            format!("SBS Stereo: {}", if self.options.sbs_enabled { "ON" } else { "OFF" }),
            format!("MSAA: {}", if self.options.msaa_enabled { "ON" } else { "OFF" }),
            "Back".to_string(),
        ];

        let selected = self.option_cursor.index();
        self.option_labels.for_each_live(|i, label, _| {
            let display = if i == selected {
                format!("> {}", texts[i])
            } else {
                format!("  {}", texts[i])
            };
            label.set_text(&display);
            let color = if i == selected {
                super::rgb(ui_style::TEXT_SELECTED)
            } else {
                super::rgb(ui_style::TEXT_UNSELECTED)
            };
            label.add_theme_color_override(theme::FONT_COLOR, color);
        });
    }

    fn close_options(&mut self) {
        self.option_labels.for_each_live(|_, label, _| label.queue_free());
        self.option_labels.clear();
        self.labels.for_each_live(|_, label, _| label.set_visible(true));
        self.in_options = false;
        self.update_cursor();
    }

}
