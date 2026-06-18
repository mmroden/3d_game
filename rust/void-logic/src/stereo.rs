/// Active display mode for the view system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayMode {
    #[default]
    Mono,
    SideBySide,
}

impl DisplayMode {
    pub fn label(self) -> &'static str {
        match self {
            DisplayMode::Mono => "SBS OFF",
            DisplayMode::SideBySide => "SBS ON",
        }
    }
}

/// Whether a CanvasLayer's custom viewport should be reset to default.
/// Returns `false` when no custom viewport is set, preventing the Godot 4.6
/// error "Cannot set viewport to nullptr" on redundant resets.
pub fn should_reset_custom_viewport(has_custom: bool) -> bool {
    has_custom
}

/// Configuration for side-by-side stereoscopic rendering.
#[derive(Debug, Clone)]
pub struct StereoConfig {
    pub eye_separation: f32,
    pub depth_strength: f32,
    pub convergence_distance: f32,
    pub viewport_width: u32,
    pub viewport_height: u32,
}

impl Default for StereoConfig {
    fn default() -> Self {
        Self {
            eye_separation: 0.065,
            depth_strength: 1.0,
            convergence_distance: 0.0,
            viewport_width: 1920,
            viewport_height: 1080,
        }
    }
}

pub fn left_eye_offset(config: &StereoConfig) -> [f32; 3] {
    let half = config.eye_separation * config.depth_strength / 2.0;
    [-half, 0.0, 0.0]
}

pub fn right_eye_offset(config: &StereoConfig) -> [f32; 3] {
    let half = config.eye_separation * config.depth_strength / 2.0;
    [half, 0.0, 0.0]
}

/// Horizontal frustum shift per eye for off-axis stereo projection.
/// Returns [left_eye_shift, right_eye_shift].
/// When convergence_distance is 0 (parallel), returns [0, 0].
pub fn frustum_offsets(config: &StereoConfig) -> [f32; 2] {
    if config.convergence_distance <= 0.0 {
        return [0.0, 0.0];
    }
    let half_sep = config.eye_separation * config.depth_strength / 2.0;
    let shift = half_sep / config.convergence_distance;
    [shift, -shift]
}

pub fn single_viewport_size(config: &StereoConfig) -> [u32; 2] {
    [config.viewport_width, config.viewport_height]
}

/// Total output resolution for full SBS: [2 * per_eye_width, height].
pub fn total_output_size(config: &StereoConfig) -> [u32; 2] {
    [config.viewport_width * 2, config.viewport_height]
}

pub fn left_viewport_rect(config: &StereoConfig) -> [u32; 4] {
    [0, 0, config.viewport_width, config.viewport_height]
}

pub fn right_viewport_rect(config: &StereoConfig) -> [u32; 4] {
    [config.viewport_width, 0, config.viewport_width, config.viewport_height]
}

/// UI viewport size — same as per-eye resolution.
/// UI renders at native per-eye res regardless of SBS mode.
pub fn ui_viewport_size(config: &StereoConfig) -> [u32; 2] {
    [config.viewport_width, config.viewport_height]
}

/// Size of the world-space UI quad at a given distance from the camera.
/// Returns [width, height] in world units (meters).
/// The quad fills the camera's field of view at the given distance.
pub fn ui_plane_size(distance: f32, fov_degrees: f32, aspect_ratio: f32) -> [f32; 2] {
    let half_fov = (fov_degrees / 2.0).to_radians();
    let h = 2.0 * distance * half_fov.tan();
    [h * aspect_ratio, h]
}

/// Position the UI plane in front of the camera along its forward vector.
/// Returns [x, y, z] in world coordinates.
pub fn ui_plane_position(cam_origin: [f32; 3], cam_forward: [f32; 3], distance: f32) -> [f32; 3] {
    [
        cam_origin[0] + cam_forward[0] * distance,
        cam_origin[1] + cam_forward[1] * distance,
        cam_origin[2] + cam_forward[2] * distance,
    ]
}

/// Rect `[x, y, w, h]` for the UI TextureRect overlay in the left eye container.
/// Local coords inside the left SubViewportContainer — origin is (0,0).
pub fn ui_overlay_rect_left(config: &StereoConfig) -> [f32; 4] {
    [0.0, 0.0, config.viewport_width as f32, config.viewport_height as f32]
}

/// Rect `[x, y, w, h]` for the UI TextureRect overlay in the right eye container.
/// Local coords inside the right SubViewportContainer — origin is (0,0).
pub fn ui_overlay_rect_right(config: &StereoConfig) -> [f32; 4] {
    [0.0, 0.0, config.viewport_width as f32, config.viewport_height as f32]
}

/// UI CanvasLayer node names that must be reparented when toggling SBS mode.
pub const UI_NODE_NAMES: &[&str] = &[
    "MainMenuUI",
    "HUD",
    "PauseMenuUI",
    "KillSummaryUI",
    "ShopUI",
    "DeathScreenUI",
    "ShipSelectUI",
    "BestiaryUI",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_eye_offset_is_negative_half_separation() {
        let cfg = StereoConfig::default();
        let offset = left_eye_offset(&cfg);
        assert_eq!(offset, [-0.065 / 2.0, 0.0, 0.0]);
    }

    #[test]
    fn viewports_tile_full_sbs_no_gap() {
        let cfg = StereoConfig::default();
        let l = left_viewport_rect(&cfg);
        let r = right_viewport_rect(&cfg);
        // Right starts where left ends
        assert_eq!(l[0] + l[2], r[0], "gap between viewports");
        // Total width = 2x per-eye width
        assert_eq!(l[2] + r[2], cfg.viewport_width * 2, "viewports don't cover full SBS width");
        // Each eye is full resolution
        assert_eq!(l[2], cfg.viewport_width);
        assert_eq!(r[2], cfg.viewport_width);
        // Heights match
        assert_eq!(l[3], cfg.viewport_height);
        assert_eq!(r[3], cfg.viewport_height);
    }

    #[test]
    fn left_viewport_starts_at_origin() {
        let cfg = StereoConfig::default();
        assert_eq!(left_viewport_rect(&cfg), [0, 0, 1920, 1080]);
    }

    #[test]
    fn right_viewport_starts_after_left() {
        let cfg = StereoConfig::default();
        assert_eq!(right_viewport_rect(&cfg), [1920, 0, 1920, 1080]);
    }

    #[test]
    fn total_output_is_double_width_for_full_sbs() {
        let cfg = StereoConfig::default(); // 1920x1080 per eye
        let [w, h] = total_output_size(&cfg);
        assert_eq!(w, 3840, "full SBS total width should be 2x per-eye width");
        assert_eq!(h, 1080, "height unchanged");
    }

    #[test]
    fn single_viewport_is_full_per_eye_resolution() {
        let cfg = StereoConfig::default();
        assert_eq!(single_viewport_size(&cfg), [1920, 1080]);
    }

    #[test]
    fn right_eye_offset_is_positive_half_separation() {
        let cfg = StereoConfig::default();
        let offset = right_eye_offset(&cfg);
        assert_eq!(offset, [0.065 / 2.0, 0.0, 0.0]);
    }

    #[test]
    fn convergence_produces_frustum_offset() {
        let cfg = StereoConfig {
            convergence_distance: 10.0,
            ..StereoConfig::default()
        };
        let [left_shift, right_shift] = frustum_offsets(&cfg);
        // Off-axis: shift = (half_sep) / convergence_distance
        let expected = (0.065 / 2.0) / 10.0;
        // Left eye frustum shifts right (positive), right eye shifts left (negative)
        assert!((left_shift - expected).abs() < 1e-6, "left shift: {left_shift}");
        assert!((right_shift - (-expected)).abs() < 1e-6, "right shift: {right_shift}");
    }

    #[test]
    fn parallel_mode_has_zero_frustum_offsets() {
        let cfg = StereoConfig::default(); // convergence_distance = 0.0
        let offsets = frustum_offsets(&cfg);
        assert_eq!(offsets, [0.0, 0.0], "parallel mode should have no frustum shift");
    }

    #[test]
    fn depth_strength_scales_eye_offsets() {
        let cfg = StereoConfig {
            depth_strength: 3.0,
            ..StereoConfig::default()
        };
        let l = left_eye_offset(&cfg);
        let r = right_eye_offset(&cfg);
        // 3x depth_strength means 3x the offset
        let base_half = 0.065 / 2.0;
        assert!((l[0] - (-base_half * 3.0)).abs() < 1e-6);
        assert!((r[0] - (base_half * 3.0)).abs() < 1e-6);
    }

    #[test]
    fn default_eye_separation_is_human_ipd() {
        let cfg = StereoConfig::default();
        assert!(
            (cfg.eye_separation - 0.065).abs() < 0.001,
            "default IPD should be ~0.065m (human average), got {}",
            cfg.eye_separation
        );
    }

    #[test]
    fn ui_viewport_size_matches_per_eye() {
        let cfg = StereoConfig::default();
        assert_eq!(ui_viewport_size(&cfg), [1920, 1080]);
    }

    #[test]
    fn ui_overlay_left_is_full_viewport() {
        let cfg = StereoConfig::default();
        assert_eq!(ui_overlay_rect_left(&cfg), [0.0, 0.0, 1920.0, 1080.0]);
    }

    #[test]
    fn ui_overlay_right_is_full_viewport_at_local_origin() {
        let cfg = StereoConfig::default();
        assert_eq!(ui_overlay_rect_right(&cfg), [0.0, 0.0, 1920.0, 1080.0]);
    }

    #[test]
    fn ui_node_names_non_empty() {
        assert!(!UI_NODE_NAMES.is_empty());
    }

    #[test]
    fn ui_node_names_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for name in UI_NODE_NAMES {
            assert!(seen.insert(name), "duplicate UI node name: {name}");
        }
    }

    #[test]
    fn every_screen_filling_ui_routes_through_the_ui_viewport() {
        // Each full-screen UI must render into the UIViewport so SBS shows it in
        // BOTH eyes (not once across the seam). The loadout and briefing screens
        // were the regression — they painted to the root viewport.
        for required in ["MainMenuUI", "HUD", "ShipSelectUI", "BestiaryUI"] {
            assert!(
                UI_NODE_NAMES.contains(&required),
                "{required} must route through the UIViewport for per-eye SBS"
            );
        }
    }

    #[test]
    fn display_mode_default_is_mono() {
        assert_eq!(DisplayMode::default(), DisplayMode::Mono);
    }

    #[test]
    fn should_reset_when_custom_viewport_is_set() {
        assert!(should_reset_custom_viewport(true));
    }

    #[test]
    fn should_skip_reset_when_already_default() {
        assert!(!should_reset_custom_viewport(false));
    }

    // --- UI plane tests (world-space 3D quad for SBS) ---

    #[test]
    fn ui_plane_size_fills_viewport_at_distance() {
        let distance = 2.0_f32;
        let fov_degrees = 75.0_f32;
        let aspect = 16.0 / 9.0;
        let [w, h] = ui_plane_size(distance, fov_degrees, aspect);
        let expected_h = 2.0 * distance * (fov_degrees.to_radians() / 2.0).tan();
        assert!((h - expected_h).abs() < 0.01, "height: got {h}, expected {expected_h}");
        assert!((w - expected_h * aspect).abs() < 0.01, "width: got {w}, expected {}", expected_h * aspect);
    }

    #[test]
    fn ui_plane_size_scales_with_distance() {
        let aspect = 16.0 / 9.0;
        let fov = 75.0_f32;
        let [w1, h1] = ui_plane_size(1.0, fov, aspect);
        let [w2, h2] = ui_plane_size(2.0, fov, aspect);
        assert!((w2 - w1 * 2.0).abs() < 0.01, "width should double with distance");
        assert!((h2 - h1 * 2.0).abs() < 0.01, "height should double with distance");
    }

    #[test]
    fn ui_plane_position_is_in_front_of_camera() {
        let cam_origin = [0.0, 0.0, 0.0];
        let cam_forward = [0.0, 0.0, -1.0]; // Godot: -Z is forward
        let distance = 2.0;
        let [x, y, z] = ui_plane_position(cam_origin, cam_forward, distance);
        assert!((x - 0.0).abs() < 0.01);
        assert!((y - 0.0).abs() < 0.01);
        assert!((z - (-2.0)).abs() < 0.01);
    }

    #[test]
    fn ui_plane_position_follows_arbitrary_camera() {
        let cam_origin = [10.0, 5.0, -3.0];
        // Forward along +X axis
        let cam_forward = [1.0, 0.0, 0.0];
        let distance = 3.0;
        let [x, y, z] = ui_plane_position(cam_origin, cam_forward, distance);
        assert!((x - 13.0).abs() < 0.01);
        assert!((y - 5.0).abs() < 0.01);
        assert!((z - (-3.0)).abs() < 0.01);
    }
}
