pub mod main_menu_ui;
pub mod pause_menu_ui;
pub mod kill_summary_ui;
pub mod shop_ui;
pub mod death_screen_ui;
pub mod hud;
pub mod menu_panel;

use godot::prelude::Color;

/// Convert an `[f32; 3]` style constant to a Godot `Color`.
pub fn rgb(c: [f32; 3]) -> Color {
    Color::from_rgb(c[0], c[1], c[2])
}
