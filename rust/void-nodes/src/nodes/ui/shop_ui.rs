use godot::prelude::*;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, Control,
    Engine, Input,
};

use super::menu_panel;
use crate::nodes::constants::{actions, shop_flags, signals, theme};
use crate::nodes::live_handle::LiveVec;
use void_logic::menu_cursor::MenuCursor;
use void_logic::ui_style;

/// The between-level (and between-lives) shop. Pure presentation: GameManager
/// prices the catalog in void-logic (`shop::offers`) and pushes it here as
/// packed arrays; a buy emits the row's typed item id back and the authority
/// validates. Unpurchasable stock stays listed (dimmed) so the cursor never
/// reshuffles under the player.
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct ShopUI {
    base: Base<CanvasLayer>,
    cursor: MenuCursor,
    labels: LiveVec<Label>,
    ids: PackedInt32Array,
    flags: PackedByteArray,
}

#[godot_api]
impl ICanvasLayer for ShopUI {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            base,
            cursor: MenuCursor::new(1),
            labels: LiveVec::new(),
            ids: PackedInt32Array::new(),
            flags: PackedByteArray::new(),
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
            if index < self.ids.len() {
                // A buy: the authority (GameManager -> shop::purchase)
                // validates affordability; refused buys are a no-op.
                let item_id = self.ids[index];
                self.base_mut().emit_signal(signals::BUY_PRESSED, &[Variant::from(item_id)]);
            } else {
                self.base_mut().emit_signal(signals::CONTINUE_PRESSED, &[]);
            }
        }
    }
}

#[godot_api]
impl ShopUI {
    #[signal]
    fn buy_pressed(item_id: i32);

    #[signal]
    fn continue_pressed();

    /// Populate and show the shop. One row per offer (parallel arrays:
    /// typed id, label, detail line, cost, flag bits), then Continue.
    // A Variant-boundary crossing: the arg list IS the wire protocol
    // (balances + one packed array per row column), not a bundle of state
    // that wants a struct — GDScript callers can't pass one.
    #[allow(clippy::too_many_arguments)]
    #[func]
    pub fn show_shop(
        &mut self,
        components: i64,
        organics: i64,
        ids: PackedInt32Array,
        labels: PackedStringArray,
        details: PackedStringArray,
        costs: PackedInt64Array,
        flags: PackedByteArray,
    ) {
        // Keep the cursor's row across a refresh (a buy re-prices the catalog);
        // clamp in case the offer count ever changes.
        let keep_row = self.cursor.index().min(ids.len());

        for mut child in self.base().get_children().iter_shared() {
            child.queue_free();
        }
        self.labels.clear();
        self.ids = ids;
        self.flags = flags;
        self.cursor = MenuCursor::new(self.ids.len() + 1);
        for _ in 0..keep_row {
            self.cursor.move_down();
        }

        // Semi-transparent overlay for ship showcase visibility
        let overlay = menu_panel::create_showcase_overlay();
        self.base_mut().add_child(&overlay);

        let (panel, mut vbox) = menu_panel::create_menu_panel();

        // Title
        let mut title = Label::new_alloc();
        title.set_text("UPGRADE STATION");
        title.add_theme_font_size_override(theme::FONT_SIZE, 48);
        title.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.8, 0.6, 1.0));
        vbox.add_child(&title);

        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 20.0));
        vbox.add_child(&spacer);

        // Balances: blue and green side by side.
        let mut components_label = Label::new_alloc();
        components_label.set_text(&format!("Components: {}", components));
        components_label.add_theme_font_size_override(theme::FONT_SIZE, 28);
        components_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_COMPONENTS));
        vbox.add_child(&components_label);

        let mut organics_label = Label::new_alloc();
        organics_label.set_text(&format!("Organics: {}", organics));
        organics_label.add_theme_font_size_override(theme::FONT_SIZE, 28);
        organics_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_ORGANICS));
        vbox.add_child(&organics_label);

        let mut spacer2 = Control::new_alloc();
        spacer2.set_custom_minimum_size(Vector2::new(0.0, 30.0));
        vbox.add_child(&spacer2);

        // Offer rows.
        for i in 0..self.ids.len() {
            let flag = if i < self.flags.len() { self.flags[i] } else { 0 };
            let label_text = labels.get(i).map(|l| l.to_string()).unwrap_or_default();
            let cost = costs.get(i).unwrap_or(0);
            let currency_name = if flag & shop_flags::GREEN != 0 { "organics" } else { "components" };

            let text = if flag & shop_flags::PURCHASABLE == 0 {
                format!("  {}", label_text)
            } else {
                format!("  {} — {} {}", label_text, cost, currency_name)
            };
            let mut row = Label::new_alloc();
            row.set_text(&text);
            row.add_theme_font_size_override(theme::FONT_SIZE, 28);
            vbox.add_child(&row);
            self.labels.push(&row, ());

            // The implication line: what buying this row actually does.
            // Smaller and dimmer — context, not a second row (the cursor
            // tracks `labels`, so this never joins it).
            if let Some(detail) = details.get(i) {
                if !detail.is_empty() {
                    let mut hint = Label::new_alloc();
                    hint.set_text(&format!("      {}", detail));
                    hint.add_theme_font_size_override(theme::FONT_SIZE, 18);
                    hint.add_theme_color_override(
                        theme::FONT_COLOR,
                        Color::from_rgb(0.55, 0.6, 0.65),
                    );
                    vbox.add_child(&hint);
                }
            }
        }

        // Continue
        let mut continue_label = Label::new_alloc();
        continue_label.set_text("  Continue");
        continue_label.add_theme_font_size_override(theme::FONT_SIZE, 28);
        vbox.add_child(&continue_label);
        self.labels.push(&continue_label, ());

        self.base_mut().add_child(&panel);
        self.base_mut().set_visible(true);
        self.update_cursor();
    }

    /// Row coloring: the selected row highlights; unpurchasable stock is
    /// dimmed, unaffordable stock reads red, everything else neutral.
    fn update_cursor(&mut self) {
        let selected = self.cursor.index();
        let flags = self.flags.clone();
        let offer_count = self.ids.len();
        self.labels.for_each_live(|i, label, _| {
            let color = if i == selected {
                super::rgb(ui_style::TEXT_SELECTED)
            } else if i < offer_count {
                let flag = if i < flags.len() { flags[i] } else { 0 };
                if flag & shop_flags::PURCHASABLE == 0 {
                    Color::from_rgb(0.4, 0.4, 0.5)
                } else if flag & shop_flags::AFFORDABLE == 0 {
                    Color::from_rgb(0.6, 0.3, 0.3)
                } else {
                    super::rgb(ui_style::TEXT_UNSELECTED)
                }
            } else {
                super::rgb(ui_style::TEXT_UNSELECTED)
            };
            label.add_theme_color_override(theme::FONT_COLOR, color);
        });
    }
}
