//! The options rows: one `Control` the main menu and the pause menu
//! both host in their panel, walking `OptionRow::ALL`. It shows the
//! authoritative `GameOptions` as GameManager broadcasts them and never
//! changes a value itself: the cursor's intent (this row, this delta)
//! goes back to the host, which signals GameManager, which adjusts,
//! saves and broadcasts — one truth, one door.

use godot::prelude::*;
use godot::classes::{Engine, IVBoxContainer, Input, Label, VBoxContainer};

use crate::nodes::constants::{actions, theme};
use crate::nodes::live_handle::LiveVec;
use void_logic::game_options::{GameOptions, OptionRow, RowKind};
use void_logic::menu_cursor::MenuCursor;
use void_logic::ui_style;

/// What a host does after handing the rows a frame's input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsOutcome {
    /// Still showing, nothing to relay.
    Open,
    /// The cursor asked to change a row: the host signals GameManager.
    Adjust { row: OptionRow, delta: i32 },
    /// Back closed the rows; the host shows its own rows again.
    Closed,
}

/// A VBoxContainer, so a host's own VBox sizes it by its rows.
#[derive(GodotClass)]
#[class(base=VBoxContainer)]
pub struct OptionsPanel {
    base: Base<VBoxContainer>,
    cursor: MenuCursor,
    /// The authoritative options as last broadcast — display only.
    options: GameOptions,
    labels: LiveVec<Label>,
}

#[godot_api]
impl IVBoxContainer for OptionsPanel {
    fn init(base: Base<VBoxContainer>) -> Self {
        Self {
            base,
            cursor: MenuCursor::new(OptionRow::ALL.len()),
            options: GameOptions::default(),
            labels: LiveVec::new(),
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        self.base_mut().set_name("OptionsPanel");
        self.build_rows();
        self.base_mut().set_visible(false);
    }
}

#[godot_api]
impl OptionsPanel {
    /// Test/inspection seam: the row texts as shown, top to bottom.
    #[func]
    pub fn row_texts(&self) -> PackedStringArray {
        let mut texts = PackedStringArray::new();
        self.labels.for_each_live(|_, label, _| texts.push(&label.get_text()));
        texts
    }

    /// Test/inspection seam: the cursor's row name.
    #[func]
    pub fn selected_row(&self) -> GString {
        GString::from(OptionRow::ALL[self.cursor.index()].name())
    }
}

impl OptionsPanel {
    /// Show the rows with the cursor at the top.
    pub fn open(&mut self) {
        self.cursor.reset();
        self.refresh();
        self.base_mut().set_visible(true);
    }

    pub fn close(&mut self) {
        self.base_mut().set_visible(false);
    }

    /// The broadcast landed: show it (the host relays its dictionary).
    pub fn set_options(&mut self, options: GameOptions) {
        self.options = options;
        self.refresh();
    }

    /// The options as displayed (the host's inspection seams read it).
    pub fn options(&self) -> &GameOptions {
        &self.options
    }

    /// The cursor's row index (the capture door's readback).
    pub fn cursor_index(&self) -> usize {
        self.cursor.index()
    }

    /// Park the cursor on a row (the capture door); out of range lands
    /// on the last row.
    pub fn set_cursor(&mut self, row: usize) {
        self.cursor = MenuCursor::new_at(row.min(OptionRow::ALL.len() - 1), OptionRow::ALL.len());
        self.refresh();
    }

    /// One frame's menu input: Up/Down walk, Left/Right step, Select
    /// flips a toggle, advances a choice, or leaves on Back; Back leaves.
    pub fn handle_input(&mut self, input: &Gd<Input>) -> OptionsOutcome {
        let row = OptionRow::ALL[self.cursor.index()];
        if input.is_action_just_pressed(actions::MENU_UP) {
            self.cursor.move_up();
            self.refresh();
        } else if input.is_action_just_pressed(actions::MENU_DOWN) {
            self.cursor.move_down();
            self.refresh();
        } else if input.is_action_just_pressed(actions::MENU_LEFT) {
            if row.kind() != RowKind::Back {
                return OptionsOutcome::Adjust { row, delta: -1 };
            }
        } else if input.is_action_just_pressed(actions::MENU_RIGHT) {
            if row.kind() != RowKind::Back {
                return OptionsOutcome::Adjust { row, delta: 1 };
            }
        } else if input.is_action_just_pressed(actions::MENU_SELECT) {
            return match row.kind() {
                RowKind::Back => {
                    self.close();
                    OptionsOutcome::Closed
                }
                // Select on a slider steps it up like Right; at the top it
                // is a no-op, which reads as "this is as high as it goes".
                RowKind::Toggle | RowKind::Choice | RowKind::Slider => {
                    OptionsOutcome::Adjust { row, delta: 1 }
                }
            };
        } else if input.is_action_just_pressed(actions::MENU_BACK) {
            self.close();
            return OptionsOutcome::Closed;
        }
        OptionsOutcome::Open
    }

    fn build_rows(&mut self) {
        for _ in OptionRow::ALL {
            let mut label = Label::new_alloc();
            label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_ROW);
            self.base_mut().add_child(&label);
            self.labels.push(&label, ());
        }
        self.refresh();
    }

    /// Reprint every row from the displayed options and the cursor.
    fn refresh(&mut self) {
        let selected = self.cursor.index();
        let options = &self.options;
        self.labels.for_each_live(|i, label, _| {
            let row = OptionRow::ALL[i];
            let value = row.value_text(options);
            let text = if value.is_empty() {
                row.label().to_string()
            } else {
                format!("{}: {value}", row.label())
            };
            let marker = if i == selected { "> " } else { "  " };
            label.set_text(&format!("{marker}{text}"));
            let color = if i == selected {
                super::rgb(ui_style::TEXT_SELECTED)
            } else {
                super::rgb(ui_style::TEXT_UNSELECTED)
            };
            label.add_theme_color_override(theme::FONT_COLOR, color);
        });
    }
}
