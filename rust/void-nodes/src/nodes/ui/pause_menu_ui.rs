use godot::prelude::*;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, Control, Engine, Input, PanelContainer,
};

use super::controls_panel::{ControlsOutcome, ControlsPanel};
use super::menu_panel;
use super::options_panel::{OptionsOutcome, OptionsPanel};
use super::options_wire;
use crate::nodes::constants::{actions, methods, nodes, signals, theme};
use crate::nodes::live_handle::{LiveOpt, LiveRef, LiveVec};
use crate::nodes::views::view_manager::ViewManager;
use void_logic::menu_cursor::MenuCursor;
use void_logic::ui_style;

/// What the layer is showing: the rows, the options rows in their
/// place, or the controls screen over the whole layer. One typed state
/// (see MainMenuUI), never a second bool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PauseView {
    Root,
    Options,
    Controls,
}

/// In-game pause menu: Resume / Options / Controls / New Game / Quit to
/// Main Menu. Uses Godot input actions so both keyboard and controller
/// work seamlessly.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct PauseMenuUI {
    base: Base<CanvasLayer>,
    cursor: MenuCursor,
    menu_items: Vec<String>,
    labels: LiveVec<Label>,
    view: PauseView,
    /// The menu reads input from its second visible frame on. GameManager
    /// pauses on the open_menu press and shows this menu in the same
    /// frame; Escape is also menu_back, so without the arm the very press
    /// that opened the menu resumed the game (the pause never held —
    /// test_controls_screen.gd pins it).
    armed: bool,
    /// The options rows, in the panel where the rows sit (see MainMenuUI).
    options_panel: Option<LiveRef<OptionsPanel>>,
    /// The rows' panel, hidden under the controls screen.
    panel: Option<LiveRef<PanelContainer>>,
    /// The controls screen, built once with the menu.
    controls: Option<LiveRef<ControlsPanel>>,
}

#[godot_api]
impl ICanvasLayer for PauseMenuUI {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            base,
            cursor: MenuCursor::new(5),
            menu_items: vec![
                "Resume".to_string(),
                "Options".to_string(),
                "Controls".to_string(),
                "New Game".to_string(),
                "Quit to Main Menu".to_string(),
            ],
            labels: LiveVec::new(),
            view: PauseView::Root,
            armed: false,
            options_panel: None,
            panel: None,
            controls: None,
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
            self.armed = false;
            return;
        }
        if !self.armed {
            // The frame that showed us carries the press that did.
            self.armed = true;
            return;
        }

        let input = Input::singleton();

        match self.view {
            PauseView::Root => self.handle_menu_actions(&input),
            PauseView::Options => self.handle_options_actions(&input),
            PauseView::Controls => self.handle_controls_actions(&input),
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

    /// The options rows asked for a change: (row name, delta) — see
    /// MainMenuUI.
    #[signal]
    fn option_adjusted(row: GString, delta: i32);

    /// GameManager's options broadcast: the rows show it.
    #[func]
    pub fn on_options_changed(&mut self, options: options_wire::OptionsDictionary) {
        let options = options_wire::from_dictionary(&options);
        self.options_panel.with(|p| p.bind_mut().set_options(options));
    }

    /// ViewManager's word on the glasses: the rows' footer shows it.
    #[func]
    pub fn on_display_status_changed(&mut self, status: GString) {
        self.options_panel.with(|p| p.bind_mut().set_note(&status.to_string()));
    }

    /// Open the controls screen over the rows — the Controls row's own
    /// door, and the test seam.
    #[func]
    pub fn open_controls(&mut self) {
        self.view = PauseView::Controls;
        self.panel.with(|p| p.set_visible(false));
        self.controls.with(|c| c.bind_mut().open_default());
    }

    /// Test/inspection seam: whether the controls screen is showing.
    #[func]
    pub fn controls_visible(&self) -> bool {
        self.view == PauseView::Controls
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
        // The display's word on the glasses, for the options footer —
        // seeded with what was said before this menu listened.
        if let Some(mut view_mgr) = parent.try_get_node_as::<ViewManager>(nodes::VIEW_MANAGER) {
            let callable = self.base().callable(methods::ON_DISPLAY_STATUS_CHANGED);
            if !view_mgr.is_connected(signals::DISPLAY_STATUS_CHANGED, &callable) {
                view_mgr.connect(signals::DISPLAY_STATUS_CHANGED, &callable);
            }
            let status = view_mgr.bind().display_status();
            self.on_display_status_changed(status);
        }
    }

    fn build_ui(&mut self) {
        let (panel, mut vbox) = menu_panel::create_menu_panel();

        let mut title = Label::new_alloc();
        title.set_text("PAUSED");
        title.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_TITLE);
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
            label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_ROW);
            let color = if i == self.cursor.index() {
                super::rgb(ui_style::TEXT_SELECTED)
            } else {
                super::rgb(ui_style::TEXT_UNSELECTED)
            };
            label.add_theme_color_override(theme::FONT_COLOR, color);
            vbox.add_child(&label);
            self.labels.push(&label, ());
        }

        // The options rows, in the panel where the rows sit, hidden until
        // the Options row.
        let options = OptionsPanel::new_alloc();
        vbox.add_child(&options);
        self.options_panel = Some(LiveRef::new(&options));

        self.base_mut().add_child(&panel);
        self.panel = Some(LiveRef::new(&panel));

        // The controls screen, over everything, hidden until its row.
        let controls = ControlsPanel::new_alloc();
        self.base_mut().add_child(&controls);
        self.controls = Some(LiveRef::new(&controls));
    }

    /// Route a frame's input to the controls screen; when it closes
    /// itself, the rows come back.
    fn handle_controls_actions(&mut self, input: &Gd<Input>) {
        let outcome = self.controls.with(|c| c.bind_mut().handle_input(input));
        if outcome != Some(ControlsOutcome::Open) {
            self.close_controls();
        }
    }

    fn close_controls(&mut self) {
        self.controls.with(|c| c.bind_mut().close());
        self.panel.with(|p| p.set_visible(true));
        self.view = PauseView::Root;
        self.update_cursor();
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
        // Row order as `menu_items` lists them: Resume, Options, Controls,
        // New Game, Quit to Main Menu.
        match self.cursor.index() {
            0 => {
                self.base_mut().emit_signal(signals::RESUME_SELECTED, &[]);
            }
            1 => {
                self.show_options();
            }
            2 => {
                self.open_controls();
            }
            3 => {
                self.base_mut().emit_signal(signals::NEW_GAME_SELECTED, &[]);
            }
            4 => {
                self.base_mut().emit_signal(signals::QUIT_SELECTED, &[]);
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

    /// Show the options rows in the panel, the rows hidden.
    fn show_options(&mut self) {
        self.view = PauseView::Options;
        self.labels.for_each_live(|_, label, _| label.set_visible(false));
        self.options_panel.with(|p| p.bind_mut().open());
    }

    /// Route a frame's input to the options rows; relay a change to
    /// GameManager, and bring the rows back when they close.
    fn handle_options_actions(&mut self, input: &Gd<Input>) {
        match self.options_panel.with(|p| p.bind_mut().handle_input(input)) {
            Some(OptionsOutcome::Adjust { row, delta }) => {
                self.base_mut().emit_signal(
                    signals::OPTION_ADJUSTED,
                    &[GString::from(row.name()).to_variant(), delta.to_variant()],
                );
            }
            Some(OptionsOutcome::Open) => {}
            Some(OptionsOutcome::Closed) | None => self.close_options(),
        }
    }

    fn close_options(&mut self) {
        self.options_panel.with(|p| p.bind_mut().close());
        self.labels.for_each_live(|_, label, _| label.set_visible(true));
        self.view = PauseView::Root;
        self.update_cursor();
    }
}
