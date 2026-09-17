//! The controls screen: one `Control` the main menu, the pause menu and
//! the pre-level briefing each host. Two pages — the gamepad silhouette
//! with callout lines to every bound control, and the keyboard with the
//! bound keys lit above a legend — composed by `void_logic::controls`
//! from the bindings this node reads out of the InputMap once, at
//! `ready`. Built once, shown and hidden after (no allocation while a
//! level runs); only the pad's vocabulary (Xbox or PlayStation names) is
//! re-read on open, since a pad can arrive between two openings.

use godot::prelude::*;
use godot::classes::{
    ColorRect, Control, Engine, IControl, Input, Label, Line2D, Panel, ResourceLoader,
    StyleBoxFlat, Texture2D, TextureRect,
    control::LayoutPreset,
    texture_rect::{ExpandMode, StretchMode},
};
use godot::global::{HorizontalAlignment, VerticalAlignment};

use super::input_bindings;
use super::menu_panel;
use crate::nodes::constants::{actions, textures, theme};
use crate::nodes::live_handle::{LiveOpt, LiveRef, LiveVec};
use void_logic::controls::{
    self, ActionBindings, Binding, Callout, KeyCap, Page, PadControl, PadFamily, Side,
};
use void_logic::ui_style;

/// What a host does after handing the screen a frame's input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlsOutcome {
    /// Still showing; the host keeps routing input here.
    Open,
    /// Back or Select closed it; the host shows its own rows again.
    Closed,
}

#[derive(GodotClass)]
#[class(base=Control)]
pub struct ControlsPanel {
    base: Base<Control>,
    page: Page,
    /// The InputMap's bindings, read once at `ready`.
    bindings: Vec<ActionBindings>,
    /// The gamepad page's callouts, in the order their labels were built.
    callouts: Vec<Callout>,
    page_title: Option<LiveRef<Label>>,
    /// The hint line: what pages and what closes, in the page's words.
    hint: Option<LiveRef<Label>>,
    /// The pad's vocabulary as of the last open.
    family: PadFamily,
    page_roots: [Option<LiveRef<Control>>; 2],
    /// One name label per callout, parallel to `callouts`, relabelled
    /// when the pad's vocabulary changes (the action text never does).
    callout_names: LiveVec<Label>,
    silhouette_loaded: bool,
    lit_keys: usize,
    /// Joypad bindings in the InputMap the silhouette does not draw.
    unplaced_pad_bindings: usize,
}

#[godot_api]
impl IControl for ControlsPanel {
    fn init(base: Base<Control>) -> Self {
        Self {
            base,
            page: Page::Keyboard,
            bindings: Vec::new(),
            callouts: Vec::new(),
            page_title: None,
            hint: None,
            family: PadFamily::Xbox,
            page_roots: [None, None],
            callout_names: LiveVec::new(),
            silhouette_loaded: false,
            lit_keys: 0,
            unplaced_pad_bindings: 0,
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        self.base_mut().set_name("ControlsPanel");
        // Anchors AND offsets: in the tree, the anchors-only preset keeps
        // the node's current (zero) rect by recomputing the offsets.
        self.base_mut().set_anchors_and_offsets_preset(LayoutPreset::FULL_RECT);
        self.bindings = input_bindings::read_bindings();
        self.unplaced_pad_bindings = self
            .bindings
            .iter()
            .flat_map(|b| b.bindings.iter())
            .filter(|b| matches!(b, Binding::PadButton(_) | Binding::PadAxis { .. }))
            .filter(|b| PadControl::for_binding(b).is_none())
            .count();
        self.build_ui();
        self.base_mut().set_visible(false);
    }
}

#[godot_api]
impl ControlsPanel {
    /// Test/inspection seam: whether the screen is showing.
    #[func]
    pub fn is_open(&self) -> bool {
        self.base().is_visible()
    }

    /// Test/inspection seam and the capture door's reading: the page
    /// index (0 controller, 1 keyboard).
    #[func]
    pub fn page_index(&self) -> i32 {
        self.page.index() as i32
    }

    /// The capture door's park: show page `index`.
    #[func]
    pub fn set_page_index(&mut self, index: i32) {
        self.set_page(Page::from_index(index.max(0) as usize));
    }

    /// Test seam: the silhouette texture was found and loaded.
    #[func]
    pub fn silhouette_loaded(&self) -> bool {
        self.silhouette_loaded
    }

    /// Test seam: callouts on the gamepad page.
    #[func]
    pub fn callout_count(&self) -> i32 {
        self.callouts.len() as i32
    }

    /// Test seam: keys (and mouse buttons) lit on the keyboard page.
    #[func]
    pub fn lit_key_count(&self) -> i32 {
        self.lit_keys as i32
    }

    /// Test seam: joypad bindings the silhouette cannot place.
    #[func]
    pub fn unplaced_pad_bindings(&self) -> i32 {
        self.unplaced_pad_bindings as i32
    }
}

impl ControlsPanel {
    /// The page area: with the panel's padding and trim it stays inside
    /// the central half of a 1920-wide window, what each eye sees in SBS
    /// (see the SBS view geometry notes); the visual contract measures it.
    const PAGE_WIDTH: f32 = 900.0;
    const PAGE_HEIGHT: f32 = 500.0;
    /// The keyboard legend's columns under the caps.
    const LEGEND_COLUMNS: usize = 3;
    /// The silhouette's drawn width; the callout columns share the rest.
    const SILHOUETTE_WIDTH: f32 = 420.0;
    /// The silhouette's aspect (Wikimedia "Xbox Controller.svg", 744x500).
    const SILHOUETTE_ASPECT: f32 = 500.0 / 744.0;
    /// A callout is two lines — the control's name over its actions —
    /// and the rows stack from the page's top.
    const CALLOUT_ROW: f32 = 56.0;
    const CALLOUT_NAME_HEIGHT: f32 = 28.0;
    const CALLOUT_TOP: f32 = 8.0;
    const CALLOUT_LINE_WIDTH: f32 = 2.0;
    /// Air between key caps, and the caps' corner radius.
    const KEY_GAP: f32 = 4.0;
    const KEY_RADIUS: i32 = 5;
    const LEGEND_ROW: f32 = 30.0;
    const LEGEND_GAP: f32 = 14.0;
    /// A lit cap's fill; a dark cap's; the cap trim.
    const KEY_LIT: [f32; 4] = [0.25, 0.55, 0.95, 1.0];
    const KEY_DARK: [f32; 4] = [0.10, 0.14, 0.24, 1.0];
    const KEY_TRIM: [f32; 4] = [0.45, 0.55, 0.75, 1.0];
    const OVERLAY: [f32; 4] = [0.02, 0.02, 0.08, 0.88];

    /// Show the screen on the page for the device in hand: the
    /// controller when a pad is connected, the keyboard otherwise.
    pub fn open_default(&mut self) {
        let page = if Input::singleton().get_connected_joypads().is_empty() {
            Page::Keyboard
        } else {
            Page::Gamepad
        };
        self.open(page);
    }

    pub fn open(&mut self, page: Page) {
        self.relabel_for_pad();
        self.set_page(page);
        self.base_mut().set_visible(true);
    }

    pub fn close(&mut self) {
        self.base_mut().set_visible(false);
    }

    pub fn set_page(&mut self, page: Page) {
        self.page = page;
        for (i, root) in self.page_roots.iter().enumerate() {
            root.with(|r| r.set_visible(i == page.index()));
        }
        let title = format!("\u{25C0}  {}  \u{25B6}", page.title());
        self.page_title.with(|t| t.set_text(&title));
        let hint = controls::screen_hint(&self.bindings, self.family);
        self.hint.with(|h| h.set_text(&hint));
    }

    /// One frame's menu input: Left/Right page, Back or Select close.
    pub fn handle_input(&mut self, input: &Gd<Input>) -> ControlsOutcome {
        if input.is_action_just_pressed(actions::MENU_LEFT) {
            self.set_page(self.page.prev());
        } else if input.is_action_just_pressed(actions::MENU_RIGHT) {
            self.set_page(self.page.next());
        } else if input.is_action_just_pressed(actions::MENU_BACK)
            || input.is_action_just_pressed(actions::MENU_SELECT)
        {
            self.close();
            return ControlsOutcome::Closed;
        }
        ControlsOutcome::Open
    }


    /// Re-read the pad's vocabulary and reprint the callout names — the
    /// same callouts in the same order, only the names change.
    fn relabel_for_pad(&mut self) {
        self.family = input_bindings::pad_family();
        let callouts = controls::gamepad_callouts(&self.bindings, self.family);
        if callouts.len() != self.callouts.len() {
            return;
        }
        self.callouts = callouts;
        let callouts = &self.callouts;
        self.callout_names.for_each_live(|i, label, _| {
            if let Some(callout) = callouts.get(i) {
                label.set_text(&callout.name);
            }
        });
    }

    /// One line of a callout, sized to its column and clipped there so a
    /// long name can never run into the silhouette or off the panel.
    fn callout_line(text: &str, rect: Rect2, align: HorizontalAlignment, size: i32, color: [f32; 3]) -> Gd<Label> {
        let mut label = Label::new_alloc();
        label.set_text(text);
        label.set_position(rect.position);
        label.set_size(rect.size);
        label.set_clip_text(true);
        label.set_horizontal_alignment(align);
        label.set_vertical_alignment(VerticalAlignment::CENTER);
        label.add_theme_font_size_override(theme::FONT_SIZE, size);
        label.add_theme_color_override(theme::FONT_COLOR, super::rgb(color));
        label
    }

    fn build_ui(&mut self) {
        let mut overlay = ColorRect::new_alloc();
        overlay.set_anchors_preset(LayoutPreset::FULL_RECT);
        overlay.set_color(Self::rgba(Self::OVERLAY));
        self.base_mut().add_child(&overlay);

        let (panel, mut vbox) = menu_panel::create_menu_panel();

        let mut title = Label::new_alloc();
        title.set_text("CONTROLS");
        title.set_horizontal_alignment(HorizontalAlignment::CENTER);
        title.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_TITLE);
        title.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.6, 0.8, 1.0));
        vbox.add_child(&title);

        let mut page_title = Label::new_alloc();
        page_title.set_horizontal_alignment(HorizontalAlignment::CENTER);
        page_title.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_HEADING);
        page_title.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_SECONDARY));
        vbox.add_child(&page_title);
        self.page_title = Some(LiveRef::new(&page_title));

        let mut page_area = Control::new_alloc();
        page_area.set_custom_minimum_size(Vector2::new(Self::PAGE_WIDTH, Self::PAGE_HEIGHT));
        vbox.add_child(&page_area);

        let gamepad = self.build_gamepad_page();
        page_area.add_child(&gamepad);
        let keyboard = self.build_keyboard_page();
        page_area.add_child(&keyboard);
        self.page_roots = [Some(LiveRef::new(&gamepad)), Some(LiveRef::new(&keyboard))];

        // The hint's text is the page's: set_page fills it.
        let mut hint = Label::new_alloc();
        hint.set_horizontal_alignment(HorizontalAlignment::CENTER);
        hint.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_DETAIL);
        hint.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_SELECTED));
        vbox.add_child(&hint);
        self.hint = Some(LiveRef::new(&hint));

        self.base_mut().add_child(&panel);
        self.family = input_bindings::pad_family();
        self.set_page(self.page);
    }

    /// The gamepad page: the silhouette centred, a label column either
    /// side, and one line per callout from its label to its control.
    fn build_gamepad_page(&mut self) -> Gd<Control> {
        let mut root = Control::new_alloc();
        root.set_size(Vector2::new(Self::PAGE_WIDTH, Self::PAGE_HEIGHT));

        let width = Self::SILHOUETTE_WIDTH;
        let height = width * Self::SILHOUETTE_ASPECT;
        let origin = Vector2::new(
            (Self::PAGE_WIDTH - width) / 2.0,
            (Self::PAGE_HEIGHT - height) / 2.0,
        );
        let mut silhouette = TextureRect::new_alloc();
        silhouette.set_position(origin);
        silhouette.set_size(Vector2::new(width, height));
        silhouette.set_expand_mode(ExpandMode::IGNORE_SIZE);
        silhouette.set_stretch_mode(StretchMode::KEEP_ASPECT_CENTERED);
        let texture: Option<Gd<Texture2D>> = ResourceLoader::singleton()
            .load(textures::GAMEPAD_SILHOUETTE)
            .and_then(|res| res.try_cast::<Texture2D>().ok());
        self.silhouette_loaded = texture.is_some();
        if let Some(texture) = texture {
            silhouette.set_texture(&texture);
        } else {
            godot_warn!(
                "ControlsPanel: {} not installed — run `make assets`",
                textures::GAMEPAD_SILHOUETTE
            );
        }
        root.add_child(&silhouette);

        let column = (Self::PAGE_WIDTH - width) / 2.0;
        let text_width = column - Self::KEY_GAP * 2.0;
        self.callouts = controls::gamepad_callouts(&self.bindings, self.family);
        for callout in &self.callouts {
            let y = Self::CALLOUT_TOP + callout.slot as f32 * Self::CALLOUT_ROW;
            let (x, align, line_x) = match callout.side {
                Side::Left => (0.0, HorizontalAlignment::RIGHT, column - Self::KEY_GAP),
                Side::Right => (Self::PAGE_WIDTH - column, HorizontalAlignment::LEFT, Self::PAGE_WIDTH - column + Self::KEY_GAP),
            };
            let name = Self::callout_line(
                &callout.name,
                Rect2::new(Vector2::new(x, y), Vector2::new(text_width, Self::CALLOUT_NAME_HEIGHT)),
                align, ui_style::FONT_DETAIL, ui_style::TEXT_SELECTED,
            );
            root.add_child(&name);
            self.callout_names.push(&name, ());
            let text = Self::callout_line(
                &callout.text,
                Rect2::new(
                    Vector2::new(x, y + Self::CALLOUT_NAME_HEIGHT),
                    Vector2::new(text_width, Self::CALLOUT_ROW - Self::CALLOUT_NAME_HEIGHT),
                ),
                align, ui_style::FONT_KEYCAP, ui_style::TEXT_SECONDARY,
            );
            root.add_child(&text);

            let anchor = origin + Vector2::new(callout.anchor[0] * width, callout.anchor[1] * height);
            let mut line = Line2D::new_alloc();
            line.add_point(Vector2::new(line_x, y + Self::CALLOUT_NAME_HEIGHT));
            line.add_point(anchor);
            line.set_width(Self::CALLOUT_LINE_WIDTH);
            line.set_default_color(super::rgb(ui_style::TEXT_SECONDARY));
            line.set_antialiased(true);
            root.add_child(&line);
        }
        root
    }

    /// The keyboard page: every cap of the drawn region, the bound ones
    /// lit, and the legend beneath in two columns.
    fn build_keyboard_page(&mut self) -> Gd<Control> {
        let mut root = Control::new_alloc();
        root.set_size(Vector2::new(Self::PAGE_WIDTH, Self::PAGE_HEIGHT));

        let unit = Self::PAGE_WIDTH / controls::keyboard_width();
        let caps = controls::keyboard_caps(&self.bindings);
        for cap in &caps {
            root.add_child(&Self::key_cap(cap, unit));
        }
        self.lit_keys = caps.iter().filter(|c| !c.actions.is_empty()).count();

        let legend = controls::keyboard_legend(&caps);
        let top = controls::keyboard_height() * unit + Self::LEGEND_GAP;
        let rows = legend.len().div_ceil(Self::LEGEND_COLUMNS);
        let column_width = Self::PAGE_WIDTH / Self::LEGEND_COLUMNS as f32;
        for (i, (key, text)) in legend.iter().enumerate() {
            let (col, row) = (i / rows, i % rows);
            let mut label = Label::new_alloc();
            label.set_text(&format!("{key}   {text}"));
            label.set_position(Vector2::new(
                col as f32 * column_width,
                top + row as f32 * Self::LEGEND_ROW,
            ));
            label.set_size(Vector2::new(column_width, Self::LEGEND_ROW));
            label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_DETAIL);
            label.add_theme_color_override(theme::FONT_COLOR, super::rgb(ui_style::TEXT_SECONDARY));
            root.add_child(&label);
        }
        root
    }

    fn key_cap(cap: &KeyCap, unit: f32) -> Gd<Panel> {
        let lit = !cap.actions.is_empty();
        let mut panel = Panel::new_alloc();
        panel.set_position(Vector2::new(
            cap.x * unit + Self::KEY_GAP / 2.0,
            cap.y * unit + Self::KEY_GAP / 2.0,
        ));
        panel.set_size(Vector2::new(cap.width * unit - Self::KEY_GAP, unit - Self::KEY_GAP));
        let mut style = StyleBoxFlat::new_gd();
        style.set_bg_color(Self::rgba(if lit { Self::KEY_LIT } else { Self::KEY_DARK }));
        style.set_border_color(Self::rgba(Self::KEY_TRIM));
        style.set_border_width_all(1);
        style.set_corner_radius_all(Self::KEY_RADIUS);
        panel.add_theme_stylebox_override("panel", &style);

        let mut label = Label::new_alloc();
        label.set_text(&cap.label);
        label.set_anchors_preset(LayoutPreset::FULL_RECT);
        label.set_horizontal_alignment(HorizontalAlignment::CENTER);
        label.set_vertical_alignment(VerticalAlignment::CENTER);
        label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_KEYCAP);
        let color = if lit { ui_style::TEXT_SELECTED } else { ui_style::TEXT_UNSELECTED };
        label.add_theme_color_override(theme::FONT_COLOR, super::rgb(color));
        panel.add_child(&label);
        panel
    }

    fn rgba(c: [f32; 4]) -> Color {
        Color::from_rgba(c[0], c[1], c[2], c[3])
    }
}
