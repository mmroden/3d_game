use godot::prelude::*;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, Control,
    Engine, Input,
};

use super::menu_panel;
use crate::nodes::constants::{actions, signals, theme};
use crate::nodes::live_handle::LiveVec;
use void_logic::menu_cursor::MenuCursor;
use void_logic::ship::ShipColor;
use void_logic::ship_type::ShipType;
use void_logic::ui_style;

/// One row of the loadout screen. Typed, so a stage rebuild can never
/// mis-route a selection through a shifted index.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Row {
    /// An OWNED hull — locked hulls never reach the screen; the shop is
    /// where they are discovered.
    Hull(ShipType),
    /// A trim tradeoff (Standard/Armored/Swift) — offered for every hull.
    Trim(ShipColor),
    /// Return from the trim stage to the hull stage.
    Back,
    Continue,
}

/// Which of the two stages is on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Stage {
    /// Pick a hull (owned hulls only).
    Hulls,
    /// Pick a trim for the chosen hull, then Continue.
    Trims,
}

/// Between-level loadout screen, in two stages: pick a hull (only owned
/// hulls appear), then a trim — every hull carries the Standard/Armored/
/// Swift tradeoffs, whether or not it has painted styles. Choices apply
/// live (the showcase re-models and recolors); Continue starts the level.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct ShipSelectUI {
    base: Base<CanvasLayer>,
    cursor: MenuCursor,
    stage: Stage,
    rows: Vec<Row>,
    labels: LiveVec<Label>,
    /// Currently applied hull id (for the selection marker).
    selected_ship_id: i32,
    /// Currently applied trim id (marker on the trim rows).
    selected_color_id: i32,
    /// Per-hull ownership, indexed by `ShipType::id()`, pushed by GameManager.
    owned: PackedByteArray,
}

#[godot_api]
impl ICanvasLayer for ShipSelectUI {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            base,
            cursor: MenuCursor::new(1),
            stage: Stage::Hulls,
            rows: Vec::new(),
            labels: LiveVec::new(),
            selected_ship_id: 0,
            selected_color_id: 0,
            owned: PackedByteArray::new(),
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
        } else if input.is_action_just_pressed(actions::MENU_BACK) {
            // Circle is always back: the trim stage returns to the hulls
            // without walking to the Back row. The hull stage has nowhere
            // further back to go.
            if self.stage == Stage::Trims {
                self.stage = Stage::Hulls;
                self.build_options();
            }
        } else if input.is_action_just_pressed(actions::MENU_SELECT) {
            let Some(row) = self.rows.get(self.cursor.index()).copied() else { return };
            match row {
                Row::Hull(ship) => {
                    self.selected_ship_id = ship.id();
                    self.base_mut().emit_signal(
                        signals::SHIP_TYPE_SELECTED,
                        &[Variant::from(ship.id())],
                    );
                    self.stage = Stage::Trims;
                    self.build_options();
                }
                Row::Trim(trim) => {
                    self.selected_color_id = trim.id();
                    self.base_mut().emit_signal(
                        signals::SHIP_COLOR_SELECTED,
                        &[Variant::from(trim.id())],
                    );
                    // Re-render for the moved marker, but keep the cursor on
                    // the chosen trim — a pick must not fling it to the top.
                    let held = self.cursor.index();
                    self.build_options();
                    self.cursor = MenuCursor::new_at(held, self.rows.len());
                    self.update_cursor();
                }
                Row::Back => {
                    self.stage = Stage::Hulls;
                    self.build_options();
                }
                Row::Continue => {
                    self.base_mut().emit_signal(signals::CONTINUE_PRESSED, &[]);
                }
            }
        }
    }
}

#[godot_api]
impl ShipSelectUI {
    #[signal]
    fn ship_type_selected(id: i32);

    #[signal]
    fn ship_color_selected(id: i32);

    #[signal]
    fn continue_pressed();

    /// Show the screen: the active hull and trim marked, ownership flags
    /// (indexed by `ShipType::id()`) deciding which hulls appear at all.
    /// Always opens on the hull stage.
    #[func]
    pub fn show_ship_select(&mut self, ship_type_id: i32, color_id: i32, owned: PackedByteArray) {
        self.selected_ship_id = ship_type_id;
        self.selected_color_id = color_id;
        self.owned = owned;
        self.stage = Stage::Hulls;
        self.build_options();
        self.base_mut().set_visible(true);
    }

    fn is_owned(&self, ship: ShipType) -> bool {
        self.owned.get(ship.id() as usize).unwrap_or(0) != 0
    }

    /// The rows for the current stage. Hull stage: owned hulls only. Trim
    /// stage: the three tradeoffs, a way back, and Continue.
    fn stage_rows(&self) -> Vec<Row> {
        match self.stage {
            Stage::Hulls => ShipType::ALL.iter()
                .filter(|ship| self.is_owned(**ship))
                .map(|ship| Row::Hull(*ship))
                .collect(),
            Stage::Trims => ShipColor::ALL.iter()
                .map(|trim| Row::Trim(*trim))
                .chain([Row::Back, Row::Continue])
                .collect(),
        }
    }

    fn stage_title(&self) -> String {
        match self.stage {
            Stage::Hulls => "SELECT HULL".to_string(),
            Stage::Trims => {
                let hull = ShipType::from_id(self.selected_ship_id).unwrap_or_default();
                format!("{} — SELECT TRIM", hull.spec().display_name.to_uppercase())
            }
        }
    }

    fn build_options(&mut self) {
        for mut child in self.base().get_children().iter_shared() {
            child.queue_free();
        }
        self.labels.clear();

        self.rows = self.stage_rows();
        self.cursor = MenuCursor::new(self.rows.len());

        let overlay = menu_panel::create_showcase_overlay();
        self.base_mut().add_child(&overlay);
        let (mut panel, mut vbox) = menu_panel::create_menu_panel();
        // Sit the loadout panel low so the rotating ship stays clear in the
        // middle of the screen, rather than centred behind the panel.
        menu_panel::seat_panel_low(&mut panel);

        let mut title = Label::new_alloc();
        title.set_text(&self.stage_title());
        title.add_theme_font_size_override(theme::FONT_SIZE, 48);
        title.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.6, 0.9, 1.0));
        vbox.add_child(&title);

        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 24.0));
        vbox.add_child(&spacer);

        let rows = self.rows.clone();
        for row in &rows {
            let (text, color) = self.row_presentation(*row);
            let mut label = Label::new_alloc();
            label.set_text(&text);
            label.add_theme_font_size_override(theme::FONT_SIZE, 28);
            label.add_theme_color_override(theme::FONT_COLOR, color);
            vbox.add_child(&label);
            self.labels.push(&label, ());
        }

        self.base_mut().add_child(&panel);
        self.update_cursor();
    }

    /// A row's resting text and color (the cursor overrides the color).
    fn row_presentation(&self, row: Row) -> (String, Color) {
        match row {
            Row::Hull(ship) => {
                let spec = ship.spec();
                let chosen = ship.id() == self.selected_ship_id;
                let mark = if chosen { "● " } else { "  " };
                (
                    format!("{}{} — {}", mark, spec.display_name, spec.weapon.display_name()),
                    Color::from_rgb(0.75, 0.85, 0.95),
                )
            }
            Row::Trim(trim) => {
                let c = trim.color();
                let chosen = trim.id() == self.selected_color_id;
                let mark = if chosen { "● " } else { "  " };
                (
                    format!("{}{} — {}", mark, trim.display_name(), trim.blurb()),
                    Color::from_rgba(c[0], c[1], c[2], c[3]),
                )
            }
            Row::Back => (
                "  ◄ Hulls".to_string(),
                super::rgb(ui_style::TEXT_UNSELECTED),
            ),
            Row::Continue => (
                "  Continue".to_string(),
                super::rgb(ui_style::TEXT_UNSELECTED),
            ),
        }
    }

    fn update_cursor(&mut self) {
        let selected = self.cursor.index();
        let rows = self.rows.clone();
        let presentations: Vec<Color> = rows.iter()
            .map(|row| self.row_presentation(*row).1)
            .collect();
        self.labels.for_each_live(|i, label, _| {
            let color = if i == selected {
                super::rgb(ui_style::TEXT_SELECTED)
            } else {
                presentations.get(i).copied().unwrap_or(super::rgb(ui_style::TEXT_UNSELECTED))
            };
            label.add_theme_color_override(theme::FONT_COLOR, color);
        });
    }
}
