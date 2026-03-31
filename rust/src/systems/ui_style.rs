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
    fn panel_bg_is_semi_transparent() {
        assert!(PANEL_BG_COLOR[3] < 1.0, "panel bg should be semi-transparent");
        assert!(PANEL_BG_COLOR[3] > 0.0, "panel bg should not be fully transparent");
    }
}
