use godot::prelude::*;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, Control, Engine, Input,
};

use super::menu_panel;
use crate::nodes::constants::{actions, methods, nodes, signals, theme};
use void_logic::game_options::GameOptions;
use void_logic::menu_cursor::MenuCursor;
use void_logic::ui_style;

/// In-game pause menu: Resume / Options / New Game / Quit to Main Menu.
/// Uses Godot input actions so both keyboard and controller work seamlessly.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct PauseMenuUI {
    base: Base<CanvasLayer>,
    cursor: MenuCursor,
    menu_items: Vec<String>,
    labels: Vec<Gd<Label>>,
    in_options: bool,
    option_cursor: MenuCursor,
    option_labels: Vec<Gd<Label>>,
    /// Cached copy of the authoritative `GameOptions` (see MainMenuUI).
    options: GameOptions,
}

#[godot_api]
impl ICanvasLayer for PauseMenuUI {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            base,
            cursor: MenuCursor::new(4),
            menu_items: vec![
                "Resume".to_string(),
                "Options".to_string(),
                "New Game".to_string(),
                "Quit to Main Menu".to_string(),
            ],
            labels: Vec::new(),
            in_options: false,
            option_cursor: MenuCursor::new(3),
            option_labels: Vec::new(),
            options: GameOptions::default(),
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        self.base_mut().set_visible(false);
        self.base_mut().set_process_mode(godot::classes::node::ProcessMode::ALWAYS);
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
impl PauseMenuUI {
    #[signal]
    fn resume_selected();

    #[signal]
    fn new_game_selected();

    #[signal]
    fn quit_selected();

    #[signal]
    fn sbs_toggled();

    #[signal]
    fn msaa_toggled();

    #[func]
    pub fn on_options_changed(&mut self, sbs_enabled: bool, msaa_enabled: bool) {
        self.options.sbs_enabled = sbs_enabled;
        self.options.msaa_enabled = msaa_enabled;
        if self.in_options {
            self.refresh_options();
        }
    }
}

impl PauseMenuUI {
    fn connect_to_game_manager(&mut self) {
        let Some(parent) = self.base().get_parent() else {
            godot_warn!("PauseMenuUI: could not find parent");
            return;
        };
        if let Some(game_mgr) = parent.try_get_node_as::<godot::classes::Node>(nodes::GAME_MANAGER) {
            let callable = self.base().callable(methods::ON_OPTIONS_CHANGED);
            if !game_mgr.is_connected(signals::OPTIONS_CHANGED, &callable) {
                let mut gm = game_mgr;
                gm.connect(signals::OPTIONS_CHANGED, &callable);
            }
        }
    }

    fn build_ui(&mut self) {
        let (panel, mut vbox) = menu_panel::create_menu_panel();

        let mut title = Label::new_alloc();
        title.set_text("PAUSED");
        title.add_theme_font_size_override(theme::FONT_SIZE, 56);
        title.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.6, 0.8, 1.0));
        vbox.add_child(&title);

        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 40.0));
        vbox.add_child(&spacer);

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
            self.labels.push(label);
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
        } else if input.is_action_just_pressed(actions::OPEN_MENU) || input.is_action_just_pressed(actions::MENU_BACK) {
            // ESC / Circle / Menu button while on pause menu = resume
            self.base_mut().emit_signal(signals::RESUME_SELECTED, &[]);
        }
    }

    fn select_item(&mut self) {
        match self.cursor.index() {
            0 => {
                self.base_mut().emit_signal(signals::RESUME_SELECTED, &[]);
            }
            1 => {
                self.in_options = true;
                self.option_cursor.reset();
                self.show_options();
            }
            2 => {
                self.base_mut().emit_signal(signals::NEW_GAME_SELECTED, &[]);
            }
            3 => {
                self.base_mut().emit_signal(signals::QUIT_SELECTED, &[]);
            }
            _ => {}
        }
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
            label.add_theme_color_override(theme::FONT_COLOR, color);

            let base_text = self.menu_items[i].clone();
            if i == self.cursor.index() {
                label.set_text(&format!("> {}", base_text));
            } else {
                label.set_text(&format!("  {}", base_text));
            }
        }
    }

    fn show_options(&mut self) {
        for mut label in self.option_labels.drain(..) {
            if label.is_instance_valid() {
                label.queue_free();
            }
        }

        for label in &mut self.labels {
            if label.is_instance_valid() {
                label.set_visible(false);
            }
        }
        let Some(parent) = self.labels[0].get_parent() else { return };
        let mut parent: Gd<godot::classes::Node> = parent;

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
            self.option_labels.push(label);
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

        for (i, label) in self.option_labels.iter_mut().enumerate() {
            if !label.is_instance_valid() {
                continue;
            }
            let display = if i == self.option_cursor.index() {
                format!("> {}", texts[i])
            } else {
                format!("  {}", texts[i])
            };
            label.set_text(&display);
            let color = if i == self.option_cursor.index() {
                super::rgb(ui_style::TEXT_SELECTED)
            } else {
                super::rgb(ui_style::TEXT_UNSELECTED)
            };
            label.add_theme_color_override(theme::FONT_COLOR, color);
        }
    }

    fn close_options(&mut self) {
        for mut label in self.option_labels.drain(..) {
            if label.is_instance_valid() {
                label.queue_free();
            }
        }
        for label in &mut self.labels {
            if label.is_instance_valid() {
                label.set_visible(true);
            }
        }
        self.in_options = false;
        self.update_cursor();
    }
}
