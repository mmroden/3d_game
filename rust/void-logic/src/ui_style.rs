//! Pure style constants for the FF-style menu panel.
//! No Godot dependency — fully testable.

/// Light blue panel background color [R, G, B, A].
pub const PANEL_BG_COLOR: [f32; 4] = [0.15, 0.25, 0.45, 0.9];

/// White trim border color [R, G, B, A].
pub const PANEL_BORDER_COLOR: [f32; 4] = [0.85, 0.9, 1.0, 1.0];

/// Border width in pixels.
pub const PANEL_BORDER_WIDTH: i32 = 3;

/// Corner radius in pixels.
pub const PANEL_CORNER_RADIUS: i32 = 12;

/// Inner padding in pixels.
pub const PANEL_PADDING: f32 = 24.0;

/// Background alpha for screens that show the ship showcase behind.
pub const SHOWCASE_BG_ALPHA: f32 = 0.4;

// ── Text colors ──────────────────────────────────────────────────

/// White — selected / highlighted menu item [R, G, B].
pub const TEXT_SELECTED: [f32; 3] = [1.0, 1.0, 1.0];

/// Muted blue-gray — unselected menu items, prompts [R, G, B].
pub const TEXT_UNSELECTED: [f32; 3] = [0.5, 0.5, 0.6];

/// Pale blue-gray — secondary labels, headers [R, G, B].
pub const TEXT_SECONDARY: [f32; 3] = [0.7, 0.7, 0.8];

/// Gold/yellow — components (in-run currency) display [R, G, B].
pub const TEXT_COMPONENTS: [f32; 3] = [1.0, 0.85, 0.2];

/// Green — organics (permanent currency) display [R, G, B].
pub const TEXT_ORGANICS: [f32; 3] = [0.4, 0.9, 0.4];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_bg_color_valid_rgba() {
        for &c in &PANEL_BG_COLOR {
            assert!((0.0..=1.0).contains(&c), "PANEL_BG_COLOR out of range: {c}");
        }
    }

    #[test]
    fn panel_border_color_valid_rgba() {
        for &c in &PANEL_BORDER_COLOR {
            assert!((0.0..=1.0).contains(&c), "PANEL_BORDER_COLOR out of range: {c}");
        }
    }

    #[test]
    fn border_width_positive() {
        assert!(PANEL_BORDER_WIDTH > 0);
    }

    #[test]
    fn corner_radius_positive() {
        assert!(PANEL_CORNER_RADIUS > 0);
    }

    #[test]
    fn padding_positive() {
        assert!(PANEL_PADDING > 0.0);
    }

    #[test]
    fn showcase_bg_alpha_valid() {
        assert!((0.0..=1.0).contains(&SHOWCASE_BG_ALPHA));
    }

    #[test]
    fn text_colors_valid_rgb() {
        for (name, color) in [
            ("TEXT_SELECTED", TEXT_SELECTED),
            ("TEXT_UNSELECTED", TEXT_UNSELECTED),
            ("TEXT_SECONDARY", TEXT_SECONDARY),
            ("TEXT_COMPONENTS", TEXT_COMPONENTS),
            ("TEXT_ORGANICS", TEXT_ORGANICS),
        ] {
            for &c in &color {
                assert!((0.0..=1.0).contains(&c), "{name} out of range: {c}");
            }
        }
    }

    #[test]
    fn panel_bg_is_semi_transparent() {
        assert!(PANEL_BG_COLOR[3] < 1.0, "panel bg should be semi-transparent");
        assert!(PANEL_BG_COLOR[3] > 0.0, "panel bg should not be fully transparent");
    }
}
