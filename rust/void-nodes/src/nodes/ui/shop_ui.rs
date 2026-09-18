use godot::prelude::*;
use godot::classes::{
    CanvasLayer, ICanvasLayer, Label, Control,
    Engine, Input,
};

use super::menu_panel;
use crate::nodes::constants::{actions, shop_flags, signals, theme};
use crate::nodes::live_handle::{LiveOpt, LiveRef, LiveVec};
use void_logic::menu_cursor::{window, MenuCursor};
use void_logic::shop::{Section, Sections};
use void_logic::ui_style::{self, distribute, rows_that_fit};

/// A section's heading, which also says how many rows its window leaves
/// out above and below — one fixed line per section, no marker rows.
struct SectionChrome {
    name: &'static str,
    heading: LiveRef<Label>,
}

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
    /// The cursor's rows — offers in display order, then Continue, then
    /// Save & Exit — each offer carrying its detail line.
    labels: LiveVec<Label, Option<LiveRef<Label>>>,
    /// Offers in display order (the wire's order permuted once, in
    /// `populate`): components stock, then organics.
    sections: Sections,
    section_chrome: [Option<SectionChrome>; 2],
    vbox: Option<LiveRef<godot::classes::VBoxContainer>>,
    /// The window sizes in force (components, organics) and the display
    /// rows they show — the inspection seams.
    window_rows: Vec<usize>,
    shown_rows: Vec<usize>,
    ids: PackedInt32Array,
    flags: PackedByteArray,
    /// The press that opened this screen is still `just_pressed` in the
    /// frame it becomes visible — swallow that one frame of input so it
    /// can't buy or Continue (playtest 2026-07-04: the kill-summary press
    /// closed the level-2 shop before it was ever seen).
    swallow_entry_press: bool,
    /// A green purchase awaiting confirmation: the info screen for this row
    /// (what it does + how to trigger it) is up — select again buys, circle
    /// cancels. Nobody buys a mystery (owner's ask, 2026-07-04).
    pending_confirm: Option<usize>,
    confirm_panel: Option<LiveRef<godot::classes::PanelContainer>>,
    /// Row text retained for the confirm screen's content.
    row_labels: PackedStringArray,
    row_details: PackedStringArray,
}

#[godot_api]
impl ICanvasLayer for ShopUI {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self {
            base,
            cursor: MenuCursor::new(1),
            labels: LiveVec::new(),
            sections: Sections::by_currency(&[]),
            section_chrome: Default::default(),
            vbox: None,
            window_rows: Vec::new(),
            shown_rows: Vec::new(),
            ids: PackedInt32Array::new(),
            flags: PackedByteArray::new(),
            swallow_entry_press: false,
            pending_confirm: None,
            confirm_panel: None,
            row_labels: PackedStringArray::new(),
            row_details: PackedStringArray::new(),
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
        if self.swallow_entry_press {
            self.swallow_entry_press = false;
            return;
        }
        let input = Input::singleton();

        if input.is_action_just_pressed(actions::MENU_UP) {
            self.dismiss_confirm();
            self.cursor.move_up();
            self.update_cursor();
        } else if input.is_action_just_pressed(actions::MENU_DOWN) {
            self.dismiss_confirm();
            self.cursor.move_down();
            self.update_cursor();
        } else if input.is_action_just_pressed(actions::MENU_BACK) {
            self.dismiss_confirm();
        } else if input.is_action_just_pressed(actions::MENU_SELECT) {
            let index = self.cursor.index();
            if index < self.ids.len() {
                // Green (permanent) purchases confirm through an info
                // screen first — what it does, how to trigger it. Blue
                // rows buy immediately; the authority (GameManager ->
                // shop::purchase) validates affordability either way.
                let flag = self.flags.get(index).unwrap_or(0);
                let green = flag & shop_flags::GREEN != 0;
                if green && self.pending_confirm != Some(index) {
                    self.show_confirm(index);
                } else {
                    self.dismiss_confirm();
                    let item_id = self.ids[index];
                    self.base_mut().emit_signal(signals::BUY_PRESSED, &[Variant::from(item_id)]);
                }
            } else if index == self.ids.len() {
                self.base_mut().emit_signal(signals::CONTINUE_PRESSED, &[]);
            } else {
                self.base_mut().emit_signal(signals::SAVE_EXIT_PRESSED, &[]);
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

    #[signal]
    fn save_exit_pressed();

    /// ENTER the shop: cursor at the top, and the press that opened the
    /// screen swallowed. Fresh-vs-refresh is explicit in the API — it must
    /// NOT be inferred from visibility, because the phase machine shows the
    /// layer before the mediator populates it (that inference let the
    /// level-2 summary press chain straight through a cursor still parked
    /// on Continue — playtest 2026-07-04).
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
        self.swallow_entry_press = true;
        self.populate(components, organics, ids, labels, details, costs, flags, 0);
    }

    /// RE-PRICE the open shop after a buy: same catalog wire, cursor kept
    /// on the row the player just used.
    #[allow(clippy::too_many_arguments)]
    #[func]
    pub fn refresh_shop(
        &mut self,
        components: i64,
        organics: i64,
        ids: PackedInt32Array,
        labels: PackedStringArray,
        details: PackedStringArray,
        costs: PackedInt64Array,
        flags: PackedByteArray,
    ) {
        let keep_row = self.cursor.index().min(ids.len());
        self.populate(components, organics, ids, labels, details, costs, flags, keep_row);
    }


    /// Inspection seam: how many offers the storefront lists (the cursor
    /// walks them, then Continue, then Save & Exit).
    #[func]
    pub fn offer_count(&self) -> i32 {
        self.ids.len() as i32
    }

    /// Inspection seam: how many rows each section's window shows —
    /// components, then organics.
    #[func]
    pub fn window_rows(&self) -> PackedInt32Array {
        self.window_rows.iter().map(|&n| n as i32).collect()
    }

    /// Inspection seam: the display rows (offers, cursor order) on screen.
    #[func]
    pub fn visible_offer_rows(&self) -> PackedInt32Array {
        self.shown_rows.iter().map(|&i| i as i32).collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn populate(
        &mut self,
        components: i64,
        organics: i64,
        ids: PackedInt32Array,
        labels: PackedStringArray,
        details: PackedStringArray,
        costs: PackedInt64Array,
        flags: PackedByteArray,
        keep_row: usize,
    ) {
        for mut child in self.base().get_children().iter_shared() {
            child.queue_free();
        }
        self.labels.clear();
        self.section_chrome = Default::default();
        self.vbox = None;
        self.pending_confirm = None;
        self.confirm_panel = None; // freed with the children above

        // The screen's order: components stock, then organics, each in the
        // catalog's order (`shop::Sections`). Every per-row array is
        // permuted once here, so from here on the cursor index IS the
        // display row and the wire's order is forgotten.
        let organics_rows: Vec<bool> =
            (0..ids.len()).map(|i| flags.get(i).unwrap_or(0) & shop_flags::GREEN != 0).collect();
        self.sections = Sections::by_currency(&organics_rows);
        let order = self.sections.order.clone();
        self.ids = order.iter().map(|&i| ids[i]).collect();
        self.flags = order.iter().map(|&i| flags.get(i).unwrap_or(0)).collect();
        self.row_labels = order.iter().map(|&i| labels.get(i).unwrap_or_default()).collect();
        self.row_details = order.iter().map(|&i| details.get(i).unwrap_or_default()).collect();
        let costs: Vec<i64> = order.iter().map(|&i| costs.get(i).unwrap_or(0)).collect();
        // Rows: the offers, then Continue, then Save & Exit.
        self.cursor = MenuCursor::new(self.ids.len() + 2);
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
        title.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_TITLE);
        title.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.8, 0.6, 1.0));
        vbox.add_child(&title);

        let mut spacer = Control::new_alloc();
        spacer.set_custom_minimum_size(Vector2::new(0.0, 20.0));
        vbox.add_child(&spacer);

        // Balances: blue and green side by side.
        let mut components_label = Label::new_alloc();
        components_label.set_text(&format!("Components: {}", components));
        components_label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_HEADING);
        components_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_COMPONENTS));
        vbox.add_child(&components_label);

        let mut organics_label = Label::new_alloc();
        organics_label.set_text(&format!("Organics: {}", organics));
        organics_label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_HEADING);
        organics_label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_ORGANICS));
        vbox.add_child(&organics_label);

        let mut spacer2 = Control::new_alloc();
        spacer2.set_custom_minimum_size(Vector2::new(0.0, 30.0));
        vbox.add_child(&spacer2);

        // The two sections, each a window over its rows under a heading
        // that also counts the rows the window leaves out, as
        // `layout_windows` slides it.
        let split = self.sections.components;
        let ranges = [
            (Section::Components, 0..split, "COMPONENTS", ui_style::TEXT_COMPONENTS),
            (Section::Organics, split..self.ids.len(), "ORGANICS", ui_style::TEXT_ORGANICS),
        ];
        for (section, rows, heading, tint) in ranges {
            if rows.is_empty() {
                continue;
            }
            let mut header = Label::new_alloc();
            header.set_text(heading);
            header.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_DETAIL);
            header.add_theme_color_override(theme::FONT_COLOR, super::rgb(tint));
            vbox.add_child(&header);
            self.section_chrome[section as usize] =
                Some(SectionChrome { name: heading, heading: LiveRef::new(&header) });
            for i in rows.clone() {
                let flag = self.flags[i];
                let label_text = self.row_labels[i].to_string();
                let currency_name = if flag & shop_flags::GREEN != 0 { "organics" } else { "components" };
                let text = if flag & shop_flags::PURCHASABLE == 0 {
                    format!("  {}", label_text)
                } else {
                    format!("  {} — {} {}", label_text, costs[i], currency_name)
                };
                let mut row = Label::new_alloc();
                row.set_text(&text);
                row.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_ROW);
                vbox.add_child(&row);

                // The implication line: what buying this row actually does.
                // Smaller and dimmer — context, not a second row (the cursor
                // tracks `labels`; the hint rides along as the row's data so
                // it hides and shows with its row).
                let detail = self.row_details[i].to_string();
                let hint = (!detail.is_empty()).then(|| {
                    let mut hint = Label::new_alloc();
                    hint.set_text(&format!("      {}", detail));
                    hint.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_DETAIL);
                    hint.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.55, 0.6, 0.65));
                    vbox.add_child(&hint);
                    LiveRef::new(&hint)
                });
                self.labels.push(&row, hint);
            }
        }

        // Continue, then Save & Exit — the run banks at the shop.
        let mut continue_label = Label::new_alloc();
        continue_label.set_text("  Continue");
        continue_label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_ROW);
        vbox.add_child(&continue_label);
        self.labels.push(&continue_label, None);

        let mut save_exit_label = Label::new_alloc();
        save_exit_label.set_text("  Save & Exit");
        save_exit_label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_ROW);
        vbox.add_child(&save_exit_label);
        self.labels.push(&save_exit_label, None);

        self.vbox = Some(LiveRef::new(&vbox));
        self.base_mut().add_child(&panel);
        self.base_mut().set_visible(true);
        self.update_cursor();
    }

    /// Slide each section's window to its cursor and show only the rows
    /// inside it. The window sizes are measured, not assumed: the rows the
    /// panel can hold once the title, balances, headings and exits have
    /// taken their height, dealt to the sections a row at a time.
    fn layout_windows(&mut self) {
        let offers = self.ids.len();
        let Some(sep) = self.vbox.with(|v| v.get_theme_constant("separation") as f32) else {
            return;
        };
        // Row cost: the tallest row with its hint; chrome: every other
        // child of the column, markers included whether or not shown.
        let mut row_px: f32 = 0.0;
        let mut chrome: f32 = 0.0;
        let mut row_ids = std::collections::HashSet::new();
        self.labels.for_each_live(|i, label, hint| {
            let own = label.get_combined_minimum_size().y + sep;
            if i < offers {
                row_ids.insert(label.instance_id());
                let hint_px = hint
                    .as_ref()
                    .and_then(|h| h.with(|h| {
                        row_ids.insert(h.instance_id());
                        h.get_combined_minimum_size().y + sep
                    }))
                    .unwrap_or(0.0);
                row_px = row_px.max(own + hint_px);
            } else {
                chrome += own;
            }
        });
        self.vbox.with(|v| {
            for child in v.get_children().iter_shared() {
                if row_ids.contains(&child.instance_id()) {
                    continue;
                }
                if let Ok(control) = child.try_cast::<Control>() {
                    chrome += control.get_combined_minimum_size().y + sep;
                }
            }
        });
        let viewport_px = self
            .base()
            .get_viewport()
            .map(|v| v.get_visible_rect().size.y)
            .unwrap_or(0.0);
        let available = viewport_px - 4.0 * ui_style::PANEL_PADDING - chrome;
        let split = self.sections.components;
        let lens = [split, self.sections.organics()];
        let shares = distribute(rows_that_fit(available, row_px), &lens);

        let focus = self.sections.locate(self.cursor.index());
        let focus_in = |section: Section| focus.filter(|(s, _)| *s == section).map(|(_, i)| i);
        let windows = [
            window(split, focus_in(Section::Components), shares[0]),
            window(self.sections.organics(), focus_in(Section::Organics), shares[1]),
        ];
        let mut shown = Vec::new();
        self.labels.for_each_live(|i, label, hint| {
            if i >= offers {
                return;
            }
            let visible = if i < split { windows[0].contains(&i) } else { windows[1].contains(&(i - split)) };
            label.set_visible(visible);
            if let Some(h) = hint {
                h.with(|h| h.set_visible(visible));
            }
            if visible {
                shown.push(i);
            }
        });
        for (section, len) in [(Section::Components, split), (Section::Organics, self.sections.organics())] {
            let w = &windows[section as usize];
            if let Some(chrome) = &self.section_chrome[section as usize] {
                let text = Self::heading_text(chrome.name, w.start, len - w.end);
                chrome.heading.with(|h| h.set_text(&text));
            }
        }
        self.window_rows = shares;
        self.shown_rows = shown;
    }

    /// The heading with the rows its window leaves out: `COMPONENTS`,
    /// `COMPONENTS · ▼ 4 more`, `COMPONENTS · ▲ 2 · ▼ 2`.
    fn heading_text(name: &str, above: usize, below: usize) -> String {
        let mut text = name.to_string();
        if above > 0 {
            text.push_str(&format!(" · ▲ {above}"));
        }
        if below > 0 {
            text.push_str(&format!(" · ▼ {below}"));
        }
        if above > 0 || below > 0 {
            text.push_str(" more");
        }
        text
    }

    /// Raise the info screen for a green row: name, what it does, how to
    /// trigger it, and the confirm/cancel prompt.
    fn show_confirm(&mut self, index: usize) {
        self.dismiss_confirm();
        let (mut panel, mut vbox) = menu_panel::create_menu_panel();

        let mut title = Label::new_alloc();
        title.set_text(&self.row_labels.get(index).map(|l| l.to_string()).unwrap_or_default());
        title.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_HEADING);
        title.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_ORGANICS));
        vbox.add_child(&title);

        // "blurb | trigger" — one line each.
        let detail = self.row_details.get(index).map(|d| d.to_string()).unwrap_or_default();
        for part in detail.split('|') {
            let mut line = Label::new_alloc();
            line.set_text(part.trim());
            line.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_BODY);
            line.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_SECONDARY));
            vbox.add_child(&line);
        }

        let mut prompt = Label::new_alloc();
        prompt.set_text("SELECT: buy    CIRCLE: back");
        prompt.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_DETAIL);
        prompt.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_UNSELECTED));
        vbox.add_child(&prompt);

        // Above the catalog, centered by the shared panel builder.
        panel.set_z_index(10);
        self.base_mut().add_child(&panel);
        self.confirm_panel = Some(LiveRef::new(&panel));
        self.pending_confirm = Some(index);
    }

    /// Drop the info screen, if up.
    fn dismiss_confirm(&mut self) {
        if let Some(panel) = self.confirm_panel.take() {
            panel.with(|p| p.queue_free());
        }
        self.pending_confirm = None;
    }

    /// Row coloring: the selected row highlights; unpurchasable stock is
    /// dimmed, unaffordable stock reads red, everything else neutral. Then
    /// the section windows follow the cursor.
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
        self.layout_windows();
    }
}
