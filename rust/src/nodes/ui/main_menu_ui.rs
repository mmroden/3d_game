use godot::prelude::*;
use godot::global::Key;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, Control, Engine, InputEvent, InputEventKey, Node,
};

use super::menu_panel;

/// FF-style main menu: Continue / New Game / Options / Exit.
/// Pure view — emits signals for all actions, never reaches into the scene tree.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct MainMenuUI {
    base: Base<CanvasLayer>,
    cursor_index: usize,
    menu_items: Vec<String>,
    labels: Vec<Gd<Label>>,
    in_options: bool,
    option_cursor: usize,
    option_labels: Vec<Gd<Label>>,
    sbs_enabled: bool,
    msaa_enabled: bool,
}

#[godot_api]
impl ICanvasLayer for MainMenuUI {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            base,
            cursor_index: 1, // Default to New Game
            menu_items: vec![
                "Continue".to_string(),
                "New Game".to_string(),
                "Options".to_string(),
                "Exit".to_string(),
            ],
            labels: Vec::new(),
            in_options: false,
            option_cursor: 0,
            option_labels: Vec::new(),
            sbs_enabled: false,
            msaa_enabled: true,
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        self.build_ui();
        self.connect_to_game_manager();
    }

    fn input(&mut self, event: Gd<InputEvent>) {
        if !self.base().is_visible() {
            return;
        }

        let Ok(key_event) = event.try_cast::<InputEventKey>() else { return };
        if !key_event.is_pressed() || key_event.is_echo() {
            return;
        }

        let keycode = key_event.get_keycode();

        if self.in_options {
            self.handle_options_input(keycode);
        } else {
            self.handle_menu_input(keycode);
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
        self.sbs_enabled = sbs_on;
        self.msaa_enabled = msaa_on;
        if self.in_options {
            self.refresh_options();
        }
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
        if let Some(game_mgr) = parent.try_get_node_as::<Node>("GameManager") {
            let callable = self.base().callable("on_options_changed");
            if !game_mgr.is_connected("options_changed", &callable) {
                let mut gm = game_mgr;
                gm.connect("options_changed", &callable);
            }
        } else {
            godot_warn!("MainMenuUI: GameManager not found");
        }
    }

    fn build_ui(&mut self) {
        let (panel, mut vbox) = menu_panel::create_menu_panel();

        // Title
        let mut title = Label::new_alloc();
        title.set_text("VOID SCAVENGER");
        title.add_theme_font_size_override("font_size", 64);
        title.add_theme_color_override("font_color", Color::from_rgb(0.6, 0.8, 1.0));
        vbox.add_child(&title);

        // Subtitle
        let mut subtitle = Label::new_alloc();
        subtitle.set_text("6DOF Roguelike Space Shooter");
        subtitle.add_theme_font_size_override("font_size", 20);
        subtitle.add_theme_color_override("font_color", Color::from_rgb(0.4, 0.5, 0.7));
        vbox.add_child(&subtitle);

        // Spacer
        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 40.0));
        vbox.add_child(&spacer);

        // Menu items
        self.labels.clear();
        for (i, item) in self.menu_items.iter().enumerate() {
            let mut label = Label::new_alloc();
            let text = if i == self.cursor_index {
                format!("> {}", item)
            } else {
                format!("  {}", item)
            };
            label.set_text(&text);
            label.add_theme_font_size_override("font_size", 32);
            let color = if i == self.cursor_index {
                Color::from_rgb(1.0, 1.0, 1.0)
            } else {
                Color::from_rgb(0.5, 0.5, 0.6)
            };
            label.add_theme_color_override("font_color", color);
            vbox.add_child(&label);
            self.labels.push(label);
        }

        self.base_mut().add_child(&panel);
    }

    fn handle_menu_input(&mut self, keycode: Key) {
        match keycode {
            Key::UP | Key::W => {
                if self.cursor_index > 0 {
                    self.cursor_index -= 1;
                } else {
                    self.cursor_index = self.menu_items.len() - 1;
                }
                self.update_cursor();
            }
            Key::DOWN | Key::S => {
                self.cursor_index = (self.cursor_index + 1) % self.menu_items.len();
                self.update_cursor();
            }
            Key::ENTER | Key::SPACE => {
                self.select_item();
            }
            _ => {}
        }
    }

    fn select_item(&mut self) {
        match self.cursor_index {
            0 => {
                self.base_mut().emit_signal("continue_selected", &[]);
            }
            1 => {
                self.base_mut().emit_signal("new_game_selected", &[]);
            }
            2 => {
                self.in_options = true;
                self.option_cursor = 0;
                self.show_options();
            }
            3 => {
                self.base_mut().emit_signal("exit_selected", &[]);
                self.base().get_tree().quit();
            }
            _ => {}
        }
    }

    fn update_cursor(&mut self) {
        for (i, label) in self.labels.iter_mut().enumerate() {
            if !label.is_instance_valid() {
                continue;
            }
            let color = if i == self.cursor_index {
                Color::from_rgb(1.0, 1.0, 1.0)
            } else {
                Color::from_rgb(0.5, 0.5, 0.6)
            };
            label.add_theme_color_override("font_color", color);

            let base_text = self.menu_items[i].clone();
            if i == self.cursor_index {
                label.set_text(&format!("> {}", base_text));
            } else {
                label.set_text(&format!("  {}", base_text));
            }
        }
    }

    fn show_options(&mut self) {
        for label in &mut self.labels {
            if label.is_instance_valid() {
                label.set_visible(false);
            }
        }

        self.option_labels.clear();
        let parent = self.labels[0].get_parent().unwrap();
        let mut parent: Gd<Node> = parent;

        let options = vec![
            format!("  SBS Stereo: {}", if self.sbs_enabled { "ON" } else { "OFF" }),
            format!("  MSAA: {}", if self.msaa_enabled { "ON" } else { "OFF" }),
            "  Back".to_string(),
        ];

        for (i, text) in options.iter().enumerate() {
            let mut label = Label::new_alloc();
            let display = if i == self.option_cursor {
                format!("> {}", text.trim())
            } else {
                text.clone()
            };
            label.set_text(&display);
            label.add_theme_font_size_override("font_size", 32);
            let color = if i == self.option_cursor {
                Color::from_rgb(1.0, 1.0, 1.0)
            } else {
                Color::from_rgb(0.5, 0.5, 0.6)
            };
            label.add_theme_color_override("font_color", color);
            parent.add_child(&label);
            self.option_labels.push(label);
        }
    }

    fn handle_options_input(&mut self, keycode: Key) {
        match keycode {
            Key::UP | Key::W => {
                if self.option_cursor > 0 {
                    self.option_cursor -= 1;
                } else {
                    self.option_cursor = 2;
                }
                self.refresh_options();
            }
            Key::DOWN | Key::S => {
                self.option_cursor = (self.option_cursor + 1) % 3;
                self.refresh_options();
            }
            Key::ENTER | Key::SPACE => {
                match self.option_cursor {
                    0 => {
                        self.base_mut().emit_signal("sbs_toggled", &[]);
                    }
                    1 => {
                        self.base_mut().emit_signal("msaa_toggled", &[]);
                    }
                    2 => {
                        self.close_options();
                    }
                    _ => {}
                }
            }
            Key::ESCAPE => {
                self.close_options();
            }
            _ => {}
        }
    }

    fn refresh_options(&mut self) {
        let texts = vec![
            format!("SBS Stereo: {}", if self.sbs_enabled { "ON" } else { "OFF" }),
            format!("MSAA: {}", if self.msaa_enabled { "ON" } else { "OFF" }),
            "Back".to_string(),
        ];

        for (i, label) in self.option_labels.iter_mut().enumerate() {
            if !label.is_instance_valid() {
                continue;
            }
            let display = if i == self.option_cursor {
                format!("> {}", texts[i])
            } else {
                format!("  {}", texts[i])
            };
            label.set_text(&display);
            let color = if i == self.option_cursor {
                Color::from_rgb(1.0, 1.0, 1.0)
            } else {
                Color::from_rgb(0.5, 0.5, 0.6)
            };
            label.add_theme_color_override("font_color", color);
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
