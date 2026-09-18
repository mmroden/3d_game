use godot::prelude::*;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, Control, Engine, Input, Node, VBoxContainer,
    control::LayoutPreset, text_server::AutowrapMode,
};
use godot::global::HorizontalAlignment;

use super::controls_panel::{ControlsOutcome, ControlsPanel};
use super::menu_panel;
use super::options_panel::{OptionsOutcome, OptionsPanel};
use super::options_wire;
use crate::nodes::constants::{actions, methods, nodes, signals, theme};
use crate::nodes::live_handle::{LiveOpt, LiveRef, LiveVec};
use crate::nodes::views::view_manager::ViewManager;
use void_logic::credits::{self, Roll, RollDrive, RollEntry};
use void_logic::menu_cursor::MenuCursor;
use void_logic::ui_style;

/// One row of the main menu. Typed, so hiding a row (Continue without a
/// continuable run) can never mis-route a selection through a shifted index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    Continue,
    NewGame,
    Bestiary,
    Options,
    Controls,
    Credits,
    Exit,
}

impl MenuAction {
    fn label(&self) -> &'static str {
        match self {
            Self::Continue => "Continue",
            Self::NewGame => "New Game",
            Self::Bestiary => "Bestiary",
            Self::Options => "Options",
            Self::Controls => "Controls",
            Self::Credits => "Credits",
            Self::Exit => "Exit",
        }
    }
}

/// What the layer is showing: the panel's action rows, the panel's
/// options rows, the controls screen, or the credits crawl over the
/// whole screen. One typed state, so a new view can never arrive as a
/// second bool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuView {
    Root,
    Options,
    Controls,
    Credits,
}

/// FF-style main menu: [Continue] / New Game / Bestiary / Options / Credits /
/// Exit. Continue appears only while GameManager says a continuable run
/// exists (pushed via `set_continue_available` — the menu never reads disk).
/// Pure view — emits signals for all actions, never reaches into the scene tree.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct MainMenuUI {
    base: Base<CanvasLayer>,
    cursor: MenuCursor,
    actions: Vec<MenuAction>,
    /// Whether a continuable run exists. Defaults pessimistic; GameManager
    /// pushes the truth whenever the menu is shown.
    continue_available: bool,
    /// Post-run-over mode: the Continue row restarts sector 1 with the
    /// profile applied (its label says so) instead of resuming a snapshot.
    continue_restarts: bool,
    labels: LiveVec<Label>,
    /// The container the action rows live in, kept so a Continue-availability
    /// change can rebuild just the rows.
    items_parent: Option<LiveRef<VBoxContainer>>,
    view: MenuView,
    /// The options rows, in the panel where the action rows sit; shows
    /// GameManager's broadcast and relays the cursor's changes.
    options_panel: Option<LiveRef<OptionsPanel>>,
    /// The credits crawl: its motion (void_logic), the full-screen node it
    /// rises through and the centered column of every credit pair — the
    /// nodes alive only while the crawl runs.
    roll: Roll,
    /// A capture (or a test) parked the crawl here: re-applied every frame,
    /// so a park that landed before the column laid out still takes, and
    /// the crawl holds still for the frame that gets saved.
    roll_parked_at: Option<f32>,
    credits_crawl: Option<LiveRef<Control>>,
    credits_column: Option<LiveRef<VBoxContainer>>,
    /// The panel, hidden while the crawl or the controls screen runs and
    /// shown again after: each is the whole screen, nothing boxed.
    hidden_for_overlay: LiveVec<Control>,
    /// The controls screen, built once with the menu and shown on the
    /// Controls row (and by the `--screen=controls` capture boot).
    controls: Option<LiveRef<ControlsPanel>>,
}

/// The menu rows for a given Continue availability. New Game always leads
/// when Continue is absent.
fn actions_for(continue_available: bool) -> Vec<MenuAction> {
    // The bestiary is always browsable (it opens on the two currency entries
    // even before an enemy is sighted) — a root-level catalog, not a run.
    // Credits likewise: the crawl reads the catalog, never a run.
    if continue_available {
        vec![
            MenuAction::Continue,
            MenuAction::NewGame,
            MenuAction::Bestiary,
            MenuAction::Options,
            MenuAction::Controls,
            MenuAction::Credits,
            MenuAction::Exit,
        ]
    } else {
        vec![
            MenuAction::NewGame,
            MenuAction::Bestiary,
            MenuAction::Options,
            MenuAction::Controls,
            MenuAction::Credits,
            MenuAction::Exit,
        ]
    }
}

#[godot_api]
impl ICanvasLayer for MainMenuUI {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            base,
            cursor: MenuCursor::new(3),
            actions: actions_for(false),
            continue_available: false,
            continue_restarts: false,
            labels: LiveVec::new(),
            items_parent: None,
            view: MenuView::Root,
            options_panel: None,
            roll: Roll::new(),
            roll_parked_at: None,
            credits_crawl: None,
            credits_column: None,
            hidden_for_overlay: LiveVec::new(),
            controls: None,
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        self.build_ui();
        self.connect_to_game_manager();
    }

    fn process(&mut self, delta: f64) {
        if !self.base().is_visible() {
            return;
        }

        let input = Input::singleton();

        match self.view {
            MenuView::Root => self.handle_menu_actions(&input),
            MenuView::Options => self.handle_options_actions(&input),
            MenuView::Controls => self.handle_controls_actions(&input),
            MenuView::Credits => self.handle_credits_actions(&input, delta as f32),
        }
    }
}

#[godot_api]
impl MainMenuUI {
    #[signal]
    fn new_game_selected();

    #[signal]
    fn bestiary_selected();

    #[signal]
    fn continue_selected();

    #[signal]
    fn exit_selected();

    /// The options rows asked for a change: (row name, delta). GameManager
    /// adjusts, remembers and broadcasts; the rows never change a value.
    #[signal]
    fn option_adjusted(row: GString, delta: i32);

    /// Test/inspection seam: the MSAA state this menu would display,
    /// which must always equal GameManager's authoritative option.
    #[func]
    pub fn displayed_msaa(&self) -> bool {
        self.options_panel.with(|p| p.bind().options().msaa_enabled).unwrap_or(false)
    }

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

    /// Pushed by GameManager whenever the menu is shown: whether a
    /// continuable run exists. Rebuilds the rows when the answer changes,
    /// with the cursor parked on New Game.
    #[func]
    pub fn set_continue_available(&mut self, available: bool, restarts: bool) {
        if self.continue_available == available
            && self.continue_restarts == restarts
            && !self.labels.is_empty()
        {
            return;
        }
        self.continue_available = available;
        self.continue_restarts = restarts;
        self.rebuild_items();
    }

    /// Start the credits crawl from below the bottom edge — the Credits
    /// row's own door, the `--screen=credits` boot's, and the test seam.
    #[func]
    pub fn open_credits(&mut self) {
        self.view = MenuView::Credits;
        self.roll = Roll::new();
        self.roll_parked_at = None;
        self.build_credits_crawl();
    }

    /// Test/inspection seam: whether the crawl is running.
    #[func]
    pub fn credits_visible(&self) -> bool {
        self.view == MenuView::Credits
    }

    /// Test/inspection seam and capture readback: the crawl's offset in
    /// pixels — how far the column has risen from below the bottom edge.
    #[func]
    pub fn credits_offset(&self) -> f32 {
        self.roll.offset()
    }

    /// Test/inspection seam: the crawl's full travel — the column's height
    /// plus the window's, bottom edge to past the top.
    #[func]
    pub fn credits_extent(&self) -> f32 {
        self.roll.extent()
    }

    /// Park the crawl at `offset` pixels and hold it there — the capture
    /// door (`--screen=credits --shot=<offset;…>`), also a test seam.
    #[func]
    pub fn set_credits_offset(&mut self, offset: f32) {
        self.roll_parked_at = Some(offset);
        self.roll.park(offset);
        self.apply_roll_offset();
    }

    /// Test seam: let a parked crawl run again from where it stands.
    #[func]
    pub fn release_credits(&mut self) {
        self.roll_parked_at = None;
    }

    /// Open the controls screen on the page for the device in hand —
    /// the Controls row's own door, the `--screen=controls` boot's, and
    /// the test seam.
    #[func]
    pub fn open_controls(&mut self) {
        self.view = MenuView::Controls;
        self.hide_panel_for_overlay();
        self.controls.with(|c| c.bind_mut().open_default());
    }

    /// Test/inspection seam: whether the controls screen is showing.
    #[func]
    pub fn controls_visible(&self) -> bool {
        self.view == MenuView::Controls
    }

    /// Test/inspection seam and capture readback: the page showing
    /// (0 controller, 1 keyboard).
    #[func]
    pub fn controls_page(&self) -> i32 {
        self.controls.with(|c| c.bind().page_index()).unwrap_or(0)
    }

    /// Park the controls screen on a page — the capture door
    /// (`--screen=controls --shot=<page;…>`), also a test seam.
    #[func]
    pub fn set_controls_page(&mut self, page: i32) {
        self.controls.with(|c| c.bind_mut().set_page_index(page));
    }

    /// Open the options rows — the Options row's own door, the
    /// `--screen=options` capture boot's, and the test seam.
    #[func]
    pub fn open_options(&mut self) {
        self.show_options();
    }

    /// The capture door's park: put the options cursor on `row`.
    #[func]
    pub fn set_options_row(&mut self, row: i32) {
        self.options_panel.with(|p| p.bind_mut().set_cursor(row.max(0) as usize));
    }

    /// Capture readback and test seam: the options cursor's row index.
    #[func]
    pub fn options_row(&self) -> i32 {
        self.options_panel.with(|p| p.bind().cursor_index() as i32).unwrap_or(0)
    }
}

impl MainMenuUI {
    /// The title on the panel — and the crawl's first line.
    const TITLE: &'static str = "VOID SCAVENGER";
    /// The crawl's column: the bestiary's readable width, centered; in
    /// SBS each eye sees the window's central half, and this sits inside.
    const COLUMN_WIDTH: f32 = 760.0;
    /// Air under the title, and between one credit pair and the next.
    const PAIR_GAP: f32 = 48.0;

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
        // Dark overlay + a low panel so the showcase ship (and, later, live
        // action) shows above the menu — the same framing as the ship-select
        // and bestiary screens.
        let overlay = menu_panel::create_showcase_overlay();
        self.base_mut().add_child(&overlay);
        let (mut panel, mut vbox) = menu_panel::create_menu_panel();
        menu_panel::seat_panel_low(&mut panel);

        // Title
        let mut title = Label::new_alloc();
        title.set_text(Self::TITLE);
        title.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_TITLE);
        title.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.6, 0.8, 1.0));
        vbox.add_child(&title);

        // Subtitle
        let mut subtitle = Label::new_alloc();
        subtitle.set_text("6DOF Roguelike Space Shooter");
        subtitle.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_DETAIL);
        subtitle.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.4, 0.5, 0.7));
        vbox.add_child(&subtitle);

        // Spacer
        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 40.0));
        vbox.add_child(&spacer);

        // Menu items live in this container; availability changes rebuild them.
        self.items_parent = Some(LiveRef::new(&vbox));
        self.rebuild_items();

        // The options rows, in the panel where the action rows sit,
        // hidden until the Options row.
        let options = OptionsPanel::new_alloc();
        vbox.add_child(&options);
        self.options_panel = Some(LiveRef::new(&options));

        self.base_mut().add_child(&panel);

        // The controls screen, over everything, hidden until its row.
        let controls = ControlsPanel::new_alloc();
        self.base_mut().add_child(&controls);
        self.controls = Some(LiveRef::new(&controls));
    }

    /// (Re)build the action rows for the current Continue availability, with
    /// the cursor parked on what the player almost always wants: Continue
    /// when a run exists, New Game otherwise.
    fn rebuild_items(&mut self) {
        self.actions = actions_for(self.continue_available);
        let preferred = if self.continue_available {
            MenuAction::Continue
        } else {
            MenuAction::NewGame
        };
        let default_row = self.actions.iter()
            .position(|a| *a == preferred)
            .unwrap_or(0);
        self.cursor = MenuCursor::new_at(default_row, self.actions.len());

        self.labels.for_each_live(|_, label, _| label.queue_free());
        self.labels.clear();

        let Some(items_parent) = &self.items_parent else { return };
        let actions = self.actions.clone();
        let selected = self.cursor.index();
        let restarts = self.continue_restarts;
        let mut new_labels: Vec<Gd<Label>> = Vec::new();
        items_parent.with(|vbox| {
            for (i, action) in actions.iter().enumerate() {
                let mut label = Label::new_alloc();
                let name = Self::action_label(*action, restarts);
                let text = if i == selected {
                    format!("> {name}")
                } else {
                    format!("  {name}")
                };
                label.set_text(&text);
                label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_ROW);
                let color = if i == selected {
                    super::rgb(ui_style::TEXT_SELECTED)
                } else {
                    super::rgb(ui_style::TEXT_UNSELECTED)
                };
                label.add_theme_color_override(theme::FONT_COLOR, color);
                vbox.add_child(&label);
                new_labels.push(label);
            }
        });
        for label in &new_labels {
            self.labels.push(label, ());
        }
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
        let Some(action) = self.actions.get(self.cursor.index()).copied() else { return };
        match action {
            MenuAction::Continue => {
                self.base_mut().emit_signal(signals::CONTINUE_SELECTED, &[]);
            }
            MenuAction::NewGame => {
                self.base_mut().emit_signal(signals::NEW_GAME_SELECTED, &[]);
            }
            MenuAction::Bestiary => {
                self.base_mut().emit_signal(signals::BESTIARY_SELECTED, &[]);
            }
            MenuAction::Options => {
                self.show_options();
            }
            MenuAction::Controls => {
                self.open_controls();
            }
            MenuAction::Credits => {
                self.open_credits();
            }
            MenuAction::Exit => {
                self.base_mut().emit_signal(signals::EXIT_SELECTED, &[]);
                self.base().get_tree().quit();
            }
        }
    }

    /// The row text for an action; the Continue row says what it will DO —
    /// resume the run, or restart sector 1 with the profile (post-run-over).
    fn action_label(action: MenuAction, restarts: bool) -> &'static str {
        match action {
            MenuAction::Continue if restarts => "Continue — Restart Sector 1",
            other => other.label(),
        }
    }

    fn update_cursor(&mut self) {
        let selected = self.cursor.index();
        let actions = &self.actions;
        let restarts = self.continue_restarts;
        self.labels.for_each_live(|i, label, _| {
            let color = if i == selected {
                super::rgb(ui_style::TEXT_SELECTED)
            } else {
                super::rgb(ui_style::TEXT_UNSELECTED)
            };
            label.add_theme_color_override(theme::FONT_COLOR, color);

            let Some(action) = actions.get(i) else { return };
            let name = Self::action_label(*action, restarts);
            if i == selected {
                label.set_text(&format!("> {name}"));
            } else {
                label.set_text(&format!("  {name}"));
            }
        });
    }

    /// Show the options rows in the panel, the action rows hidden.
    fn show_options(&mut self) {
        self.view = MenuView::Options;
        self.labels.for_each_live(|_, label, _| label.set_visible(false));
        self.options_panel.with(|p| p.bind_mut().open());
    }

    /// Route a frame's input to the options rows; relay a change to
    /// GameManager, and bring the action rows back when they close.
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
        self.view = MenuView::Root;
        self.update_cursor();
    }

    /// The crawl's pairs — contribution, then person: the team and every
    /// shipped source in the catalog's order (`void_logic::credits`, from
    /// catalog/attributions.toml).
    fn credits_roll() -> Vec<RollEntry> {
        credits::credits().roll()
    }

    /// The window the crawl rises through: the UI viewport's size.
    fn window_size(&self) -> Vector2 {
        self.base()
            .get_viewport()
            .map(|v| v.get_visible_rect().size)
            .unwrap_or(Vector2::ZERO)
    }

    /// Build the crawl: the panel goes, and a centered column — the title,
    /// then every pair, contribution above person — sits just below the
    /// bottom edge of a full-screen node, to rise through it. Every line
    /// wraps at the column's width. The column lays out a frame later; its
    /// height sets the crawl's extent from then on.
    fn build_credits_crawl(&mut self) {
        self.free_credits_crawl();
        self.hide_panel_for_overlay();

        let mut crawl = Control::new_alloc();
        crawl.set_anchors_preset(LayoutPreset::FULL_RECT);
        crawl.set_clip_contents(true);
        let mut column = VBoxContainer::new_alloc();
        column.set_custom_minimum_size(Vector2::new(Self::COLUMN_WIDTH, 0.0));
        column.add_child(&Self::crawl_label(Self::TITLE, ui_style::FONT_TITLE, ui_style::TEXT_SELECTED));
        column.add_child(&Self::crawl_gap());
        for entry in Self::credits_roll() {
            column.add_child(&Self::crawl_label(
                &entry.contribution, ui_style::FONT_DETAIL, ui_style::TEXT_SECONDARY));
            column.add_child(&Self::crawl_label(
                &entry.person, ui_style::FONT_HEADING, ui_style::TEXT_SELECTED));
            column.add_child(&Self::crawl_gap());
        }
        crawl.add_child(&column);
        self.base_mut().add_child(&crawl);

        self.credits_crawl = Some(LiveRef::new(&crawl));
        self.credits_column = Some(LiveRef::new(&column));
        self.apply_roll_offset();
    }

    /// One centered line of the crawl, wrapping at the column's width.
    fn crawl_label(text: &str, size: i32, color: [f32; 3]) -> Gd<Label> {
        let mut label = Label::new_alloc();
        label.set_text(text);
        label.add_theme_font_size_override(theme::FONT_SIZE, size);
        label.add_theme_color_override(theme::FONT_COLOR, super::rgb(color));
        label.set_horizontal_alignment(HorizontalAlignment::CENTER);
        label.set_autowrap_mode(AutowrapMode::WORD_SMART);
        label.set_custom_minimum_size(Vector2::new(Self::COLUMN_WIDTH, 0.0));
        label
    }

    fn crawl_gap() -> Gd<Control> {
        let mut gap = Control::new_alloc();
        gap.set_custom_minimum_size(Vector2::new(0.0, Self::PAIR_GAP));
        gap
    }

    /// Drive the crawl one frame: the extent is the column's height plus
    /// the window's (bottom edge to past the top), read once the column
    /// has laid out; move under the held direction or on its own; place
    /// the column. Back or Select leave early; the crawl's own end
    /// returns to the menu.
    fn handle_credits_actions(&mut self, input: &Gd<Input>, delta: f32) {
        if input.is_action_just_pressed(actions::MENU_SELECT)
            || input.is_action_just_pressed(actions::MENU_BACK)
        {
            self.close_credits();
            return;
        }
        let window = self.window_size();
        let column_height = self.credits_column.with(|c| c.get_size().y).unwrap_or(0.0);
        let extent = if column_height > 0.0 { column_height + window.y } else { 0.0 };
        self.roll.set_extent(extent);
        let drive = if let Some(parked) = self.roll_parked_at {
            self.roll.park(parked);
            RollDrive::Hold
        } else if input.is_action_pressed(actions::MENU_DOWN) {
            RollDrive::Forward
        } else if input.is_action_pressed(actions::MENU_UP) {
            RollDrive::Back
        } else {
            RollDrive::Auto
        };
        self.roll.advance(delta, drive);
        self.apply_roll_offset();
        if extent > 0.0 && self.roll_parked_at.is_none() && self.roll.finished() {
            self.close_credits();
        }
    }

    /// Place the column: centered, its top `offset` pixels above the
    /// window's bottom edge.
    fn apply_roll_offset(&mut self) {
        let window = self.window_size();
        let position = Vector2::new(
            ((window.x - Self::COLUMN_WIDTH) / 2.0).max(0.0),
            window.y - self.roll.offset(),
        );
        self.credits_column.with(|column| column.set_position(position));
    }

    fn free_credits_crawl(&mut self) {
        self.credits_column = None;
        if let Some(crawl) = self.credits_crawl.take() {
            crawl.with(|c| c.queue_free());
        }
    }

    fn close_credits(&mut self) {
        self.free_credits_crawl();
        self.roll_parked_at = None;
        self.show_panel_after_overlay();
        self.view = MenuView::Root;
        self.update_cursor();
    }

    /// Hide the menu panel under a whole-screen view (the crawl, the
    /// controls screen), remembering it for `show_panel_after_overlay`.
    fn hide_panel_for_overlay(&mut self) {
        let Some(items_parent) = &self.items_parent else { return };
        if let Some(mut panel) = items_parent
            .with(|vbox| vbox.get_parent())
            .flatten()
            .and_then(|p| p.try_cast::<Control>().ok())
        {
            panel.set_visible(false);
            self.hidden_for_overlay.push(&panel, ());
        }
    }

    fn show_panel_after_overlay(&mut self) {
        self.hidden_for_overlay.for_each_live(|_, control, _| control.set_visible(true));
        self.hidden_for_overlay.clear();
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
        self.show_panel_after_overlay();
        self.view = MenuView::Root;
        self.update_cursor();
    }
}
