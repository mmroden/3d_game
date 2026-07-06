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

// ── Type scale ───────────────────────────────────────────────────
// ONE ladder for every menu screen, sized for couch/glasses distance
// (playtest 2026-07-04: "the menu text is _tiny_"). Screens never invent
// their own pixel sizes — pick the role. The in-level HUD keeps its own
// tighter sizes (it lives inside the SBS safe band).

/// Screen titles — "UPGRADE STATION", "SHIP LOADOUT", the death banner.
pub const FONT_TITLE: i32 = 72;
/// Section headers and balance lines.
pub const FONT_HEADING: i32 = 44;
/// Menu rows — the things the cursor walks.
pub const FONT_ROW: i32 = 40;
/// Body copy — summaries, blurbs, kept/lost lines.
pub const FONT_BODY: i32 = 32;
/// Fine print — row detail hints, prompts, pager position.
pub const FONT_DETAIL: i32 = 26;

// ── HUD type ─────────────────────────────────────────────────────
// The in-game HUD lives inside the SBS safe band, so its ladder is tighter
// than the menus' — but still couch-readable, and every HUD label wears a
// dark outline so it reads against any backdrop (playtest 2026-07-04).

/// The big readouts — health, the slow-warning banner.
pub const FONT_HUD_PRIMARY: i32 = 38;
/// Balances, shield, laser, lives, level.
pub const FONT_HUD_LABEL: i32 = 32;
/// Power mode, the controls hint.
pub const FONT_HUD_FINE: i32 = 26;
/// Outline width (px) on every HUD label.
pub const HUD_OUTLINE: i32 = 6;
/// Outline color — near-black, softened just enough not to ring.
pub const HUD_OUTLINE_COLOR: [f32; 3] = [0.02, 0.03, 0.05];

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
    fn the_type_scale_descends_and_stays_readable() {
        assert!(FONT_TITLE > FONT_HEADING);
        assert!(FONT_HEADING >= FONT_ROW);
        assert!(FONT_ROW > FONT_BODY);
        assert!(FONT_BODY > FONT_DETAIL);
        // The floors the playtest set: rows readable from the couch, the
        // title anchoring the screen.
        assert!(FONT_ROW >= 36, "menu rows must be readable from the couch");
        assert!(FONT_TITLE >= 64);
        assert!(FONT_DETAIL >= 22, "even fine print must not be squint-sized");
        // The HUD ladder: tighter than the menus (it shares the safe band
        // with the action) but never below the readable floor, and always
        // outlined.
        assert!(FONT_HUD_PRIMARY > FONT_HUD_LABEL);
        assert!(FONT_HUD_LABEL > FONT_HUD_FINE);
        assert!(FONT_HUD_FINE >= 26, "HUD fine print must be couch-readable");
        assert!(HUD_OUTLINE >= 4, "HUD text needs a real outline to read");
    }

    #[test]
    fn panel_bg_is_semi_transparent() {
        assert!(PANEL_BG_COLOR[3] < 1.0, "panel bg should be semi-transparent");
        assert!(PANEL_BG_COLOR[3] > 0.0, "panel bg should not be fully transparent");
    }
}
