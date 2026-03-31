//! Reusable FF-style menu panel: light blue box with rounded edges and white trim.
//! Used by all menu screens (main menu, kill summary, shop, death).

use godot::prelude::*;
use godot::classes::{
    PanelContainer, StyleBoxFlat, VBoxContainer,
    control::LayoutPreset,
};

use crate::systems::ui_style;

/// Creates a centered, styled menu panel containing a VBoxContainer for content.
/// Returns `(panel, vbox)` — add content to the vbox, add panel to the CanvasLayer.
pub fn create_menu_panel() -> (Gd<PanelContainer>, Gd<VBoxContainer>) {
    let mut panel = PanelContainer::new_alloc();

    // Style: light blue background with white rounded border
    let mut style = StyleBoxFlat::new_gd();

    let bg = ui_style::PANEL_BG_COLOR;
    style.set_bg_color(Color::from_rgba(bg[0], bg[1], bg[2], bg[3]));

    let border = ui_style::PANEL_BORDER_COLOR;
    style.set_border_color(Color::from_rgba(border[0], border[1], border[2], border[3]));

    let bw = ui_style::PANEL_BORDER_WIDTH;
    style.set_border_width_all(bw);

    let cr = ui_style::PANEL_CORNER_RADIUS;
    style.set_corner_radius_all(cr);

    let pad = ui_style::PANEL_PADDING as i32;
    style.set_content_margin_all(pad as f32);

    panel.add_theme_stylebox_override("panel", &style);

    // Center in viewport: anchor at center, grow both directions
    panel.set_anchors_preset(LayoutPreset::CENTER);
    panel.set_h_grow_direction(godot::classes::control::GrowDirection::BOTH);
    panel.set_v_grow_direction(godot::classes::control::GrowDirection::BOTH);

    // VBoxContainer for content
    let vbox = VBoxContainer::new_alloc();
    panel.add_child(&vbox);

    (panel, vbox)
}

/// Creates a semi-transparent dark overlay behind a menu panel.
/// Used for screens that show the ship showcase in the background.
pub fn create_showcase_overlay() -> Gd<godot::classes::ColorRect> {
    let mut overlay = godot::classes::ColorRect::new_alloc();
    overlay.set_anchors_preset(LayoutPreset::FULL_RECT);
    overlay.set_color(Color::from_rgba(0.02, 0.02, 0.08, ui_style::SHOWCASE_BG_ALPHA));
    overlay
}
