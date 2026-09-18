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


/// Views the display interface renders per frame: one eye in mono, two
/// side by side.
pub fn view_count(mode: DisplayMode) -> u32 {
    match mode {
        DisplayMode::Mono => 1,
        DisplayMode::SideBySide => 2,
    }
}


/// The render target one view needs for a `window` of `[w, h]` pixels:
/// the whole window in mono; side by side, each eye takes half the width.
pub fn per_eye_size(mode: DisplayMode, window: [u32; 2]) -> [u32; 2] {
    match mode {
        DisplayMode::Mono => window,
        DisplayMode::SideBySide => [window[0] / 2, window[1]],
    }
}


/// Whether the MSAA option may be applied to the render for `display` on
/// `rendering_driver` (Godot's driver name: "metal", "vulkan", …).
/// Godot 4.6.1's Metal driver cannot slice a multisample array — the
/// per-view MSAA resolve of a two-view render asserts inside Metal's
/// texture-view validation (`MTLTextureType2DMultisampleArray` viewed as
/// `MTLTextureType2D`, 2026-09-17, TAA on or off) — so on Metal MSAA is a
/// single-view feature. Re-test on an engine upgrade before widening.
pub fn msaa_allowed(display: Display, rendering_driver: &str) -> bool {
    !(rendering_driver == "metal" && display.views() > 1)
}


/// What renders the world this run: the SBS display interface in one of
/// its modes (the xReal as a crippled headset, or its single mono eye),
/// or an OpenXR runtime (a headset). OpenXR is enabled at process start
/// — Godot's `--xr-mode on` or the project setting — never from the
/// menu, so it is a fact of the run, not a preference; the saved SBS
/// preference only chooses between the SBS interface's modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Sbs(DisplayMode),
    OpenXr,
}

impl Display {
    /// The display in force: the runtime when one initialized, else the
    /// SBS interface in the preferred mode.
    pub fn effective(sbs_enabled: bool, openxr_active: bool) -> Display {
        if openxr_active {
            Display::OpenXr
        } else if sbs_enabled {
            Display::Sbs(DisplayMode::SideBySide)
        } else {
            Display::Sbs(DisplayMode::Mono)
        }
    }

    /// Views rendered per frame.
    pub fn views(self) -> u32 {
        match self {
            Display::Sbs(mode) => view_count(mode),
            Display::OpenXr => 2,
        }
    }

    /// Whether the HUD confines itself to the central band each eye sees:
    /// only side by side, where each eye's frustum covers the central half
    /// of the full-window UI plane; a headset's eye sees the whole plane.
    pub fn central_band(self) -> bool {
        self == Display::Sbs(DisplayMode::SideBySide)
    }

    /// Whether the UI shows on the cockpit-locked plane (else the flat
    /// mono layer draws it): every two-view display.
    pub fn ui_on_plane(self) -> bool {
        self.views() > 1
    }

    /// Whether the chase (third-person) view is offered: not in a headset
    /// (owner 2026-09-17: nauseating).
    pub fn chase_allowed(self) -> bool {
        self != Display::OpenXr
    }

    /// Whether the display takes the whole panel: the glasses want it; a
    /// headset's window is a mirror and mono keeps the player's preference.
    pub fn wants_fullscreen(self) -> bool {
        self == Display::Sbs(DisplayMode::SideBySide)
    }

    pub fn label(self) -> &'static str {
        match self {
            Display::Sbs(mode) => mode.label(),
            Display::OpenXr => "OpenXR",
        }
    }
}

/// Horizontal eye offset (meters, camera X) for `view` of `mode`: the
/// single eye sits on the camera axis; the stereo pair straddles it.
pub fn eye_offset_for_view(mode: DisplayMode, config: &StereoConfig, view: u32) -> f32 {
    match (mode, view) {
        (DisplayMode::Mono, _) => 0.0,
        (DisplayMode::SideBySide, 0) => left_eye_offset(config)[0],
        (DisplayMode::SideBySide, _) => right_eye_offset(config)[0],
    }
}

/// Off-axis perspective projection, column-major (Godot's `Projection`
/// layout): the symmetric perspective of `fov_v_deg` (vertical) and
/// `aspect`, its window slid horizontally by `shift` — the dimensionless
/// stereo shift per unit distance from `frustum_offsets`, which lands in
/// the matrix as the off-axis term `shift / (tan(fov_v/2) · aspect)`.
/// `shift = 0` is the plain perspective.
pub fn off_axis_projection(fov_v_deg: f32, aspect: f32, near: f32, far: f32, shift: f32) -> [f64; 16] {
    let tan_half = ((fov_v_deg as f64).to_radians() / 2.0).tan();
    let (n, f, a) = (near as f64, far as f64, aspect as f64);
    let mut m = [0.0; 16];
    m[0] = 1.0 / (tan_half * a);
    m[5] = 1.0 / tan_half;
    // Column 2: the off-axis slide (a frustum whose left/right bounds are
    // both moved by shift·near has (right+left)/(right−left) = shift/tan_h).
    m[8] = shift as f64 / (tan_half * a);
    m[10] = -(f + n) / (f - n);
    m[11] = -1.0;
    m[14] = -2.0 * f * n / (f - n);
    m
}

/// The projection for `view` of `mode`: mono and a parallel rig are the
/// symmetric perspective; a converged rig slides each eye's window by
/// its `frustum_offsets` entry.
pub fn projection_for_view(
    mode: DisplayMode,
    config: &StereoConfig,
    view: u32,
    fov_v_deg: f32,
    aspect: f32,
    near: f32,
    far: f32,
) -> [f64; 16] {
    let shift = match mode {
        DisplayMode::Mono => 0.0,
        DisplayMode::SideBySide => frustum_offsets(config)[view.min(1) as usize],
    };
    off_axis_projection(fov_v_deg, aspect, near, far, shift)
}

/// Where each rendered view lands in the window, `[x, y, w, h]` pixels,
/// in view order: the single eye fills the window; the pair tiles it
/// left|right.
pub fn blit_rects(mode: DisplayMode, config: &StereoConfig) -> Vec<[u32; 4]> {
    match mode {
        DisplayMode::Mono => vec![left_viewport_rect(config)],
        DisplayMode::SideBySide => vec![left_viewport_rect(config), right_viewport_rect(config)],
    }
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

/// Size of the world-space UI quad at a given distance from the camera.
/// Returns [width, height] in world units (meters).
/// The quad fills the camera's field of view at the given distance.
pub fn ui_plane_size(distance: f32, fov_degrees: f32, aspect_ratio: f32) -> [f32; 2] {
    let half_fov = (fov_degrees / 2.0).to_radians();
    let h = 2.0 * distance * half_fov.tan();
    [h * aspect_ratio, h]
}

// --- The convergence travel band ---
//
// The stereo director (see director.rs) parks the zero-parallax plane on
// the scene's current subject. Its travel is bounded here: NEAR sits
// safely past the cockpit shell (the capsule's radial clearance keeps
// the shell inside ~0.45 m), so the plane never converges into the
// pilot's own console; FAR is the deep rest when the stage is empty.
// (These bounds outlived the removed depth-adaptive reticle, which
// tracked the same band before the owner cut the visible sight,
// 2026-08-19 — the plane itself is the "look here" signal now.)

pub const CONVERGENCE_NEAR: f32 = 1.2;
pub const CONVERGENCE_FAR: f32 = 10.0;

/// One frame of exponential approach from `current` toward `target` at
/// `rate` (1/s). Never overshoots; `dt = 0` is the identity. The shared
/// easing under every depth-affecting dial — the comfort literature's
/// discomfort driver is the RATE of vergence change, so nothing that
/// moves perceived depth moves in steps.
pub fn exp_approach(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    current + (target - current) * (1.0 - (-rate * dt).exp())
}

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


    // --- The display interface's contract: views, eye offsets, projections, blits ---

    /// NDC x of a camera-space point through a column-major projection.
    fn ndc_x(m: &[f64; 16], p: [f64; 3]) -> f64 {
        let clip_x = m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12];
        let clip_w = m[3] * p[0] + m[7] * p[1] + m[11] * p[2] + m[15];
        clip_x / clip_w
    }

    /// The symmetric perspective, written out independently of the code
    /// under test (Godot's `Projection::set_perspective`, column-major).
    fn symmetric_perspective(fov_v_deg: f32, aspect: f32, near: f32, far: f32) -> [f64; 16] {
        let t = (fov_v_deg as f64).to_radians().tan_half();
        let (n, f, a) = (near as f64, far as f64, aspect as f64);
        let mut m = [0.0; 16];
        m[0] = 1.0 / (t * a);
        m[5] = 1.0 / t;
        m[10] = -(f + n) / (f - n);
        m[11] = -1.0;
        m[14] = -2.0 * f * n / (f - n);
        m
    }

    trait TanHalf {
        fn tan_half(self) -> f64;
    }
    impl TanHalf for f64 {
        fn tan_half(self) -> f64 {
            (self / 2.0).tan()
        }
    }

    const FOV: f32 = 75.0;
    const NEAR: f32 = 0.05;
    const FAR: f32 = 400.0;

    #[test]
    fn view_count_is_one_eye_in_mono_and_two_side_by_side() {
        assert_eq!(view_count(DisplayMode::Mono), 1);
        assert_eq!(view_count(DisplayMode::SideBySide), 2);
    }


    #[test]
    fn per_eye_size_is_the_window_in_mono_and_half_its_width_side_by_side() {
        assert_eq!(per_eye_size(DisplayMode::Mono, [3840, 1080]), [3840, 1080]);
        assert_eq!(per_eye_size(DisplayMode::SideBySide, [3840, 1080]), [1920, 1080]);
        // An odd window floors the half — the two views tile inside it.
        assert_eq!(per_eye_size(DisplayMode::SideBySide, [1145, 828]), [572, 828]);
    }


    #[test]
    fn msaa_is_single_view_only_on_the_metal_driver() {
        // Godot 4.6.1's Metal driver cannot slice a multisample array (the
        // per-view MSAA resolve asserts in texture-view creation, verified
        // 2026-09-17 with TAA on and off); Vulkan multiviews with MSAA fine.
        let mono = Display::Sbs(DisplayMode::Mono);
        let sbs = Display::Sbs(DisplayMode::SideBySide);
        assert!(msaa_allowed(mono, "metal"));
        assert!(!msaa_allowed(sbs, "metal"));
        assert!(!msaa_allowed(Display::OpenXr, "metal"), "a headset is two views too");
        assert!(msaa_allowed(sbs, "vulkan"));
        assert!(msaa_allowed(Display::OpenXr, "vulkan"));
        assert!(msaa_allowed(mono, "vulkan"));
        // Headless has no driver to speak of; mono there is the GUT shell.
        assert!(msaa_allowed(mono, ""));
    }

    // --- The effective display: the SBS interface in a mode, or the OpenXR runtime ---

    #[test]
    fn the_runtime_decides_the_display_when_it_runs_and_the_preference_otherwise() {
        // OpenXR is enabled at process start (Godot's --xr-mode / project
        // setting), never from the menu: when a runtime initialized, it is
        // the display whatever the saved SBS preference says.
        assert_eq!(Display::effective(true, true), Display::OpenXr);
        assert_eq!(Display::effective(false, true), Display::OpenXr);
        assert_eq!(Display::effective(true, false), Display::Sbs(DisplayMode::SideBySide));
        assert_eq!(Display::effective(false, false), Display::Sbs(DisplayMode::Mono));
    }

    #[test]
    fn a_display_renders_one_view_in_mono_and_two_in_either_stereo() {
        assert_eq!(Display::Sbs(DisplayMode::Mono).views(), 1);
        assert_eq!(Display::Sbs(DisplayMode::SideBySide).views(), 2);
        assert_eq!(Display::OpenXr.views(), 2);
    }

    #[test]
    fn only_side_by_side_confines_the_hud_to_the_central_band() {
        // The band exists because each SBS eye sees the central half of the
        // full-window UI plane; a headset's eye sees the whole plane.
        assert!(Display::Sbs(DisplayMode::SideBySide).central_band());
        assert!(!Display::Sbs(DisplayMode::Mono).central_band());
        assert!(!Display::OpenXr.central_band());
    }

    #[test]
    fn the_ui_rides_the_plane_in_both_two_view_displays() {
        assert!(!Display::Sbs(DisplayMode::Mono).ui_on_plane());
        assert!(Display::Sbs(DisplayMode::SideBySide).ui_on_plane());
        assert!(Display::OpenXr.ui_on_plane());
    }

    #[test]
    fn the_chase_view_is_not_offered_in_a_headset() {
        // Owner 2026-09-17: no chase view in VR — nauseating.
        assert!(Display::Sbs(DisplayMode::Mono).chase_allowed());
        assert!(Display::Sbs(DisplayMode::SideBySide).chase_allowed());
        assert!(!Display::OpenXr.chase_allowed());
    }

    #[test]
    fn only_side_by_side_takes_the_whole_panel() {
        // The glasses want the whole panel; a headset's window is a mirror
        // and mono keeps the player's window preference.
        assert!(Display::Sbs(DisplayMode::SideBySide).wants_fullscreen());
        assert!(!Display::Sbs(DisplayMode::Mono).wants_fullscreen());
        assert!(!Display::OpenXr.wants_fullscreen());
    }

    #[test]
    fn mono_eye_sits_on_the_camera_axis_and_the_pair_straddles_it() {
        let cfg = StereoConfig::default();
        assert_eq!(eye_offset_for_view(DisplayMode::Mono, &cfg, 0), 0.0);
        let l = eye_offset_for_view(DisplayMode::SideBySide, &cfg, 0);
        let r = eye_offset_for_view(DisplayMode::SideBySide, &cfg, 1);
        assert_eq!(l, left_eye_offset(&cfg)[0]);
        assert_eq!(r, right_eye_offset(&cfg)[0]);
        assert!(l < 0.0 && r > 0.0 && (l + r).abs() < 1e-9, "the eyes straddle the axis");
    }

    #[test]
    fn unshifted_projection_is_the_symmetric_perspective() {
        let aspect = 16.0 / 9.0;
        let got = off_axis_projection(FOV, aspect, NEAR, FAR, 0.0);
        let want = symmetric_perspective(FOV, aspect, NEAR, FAR);
        for (i, (g, w)) in got.iter().zip(&want).enumerate() {
            assert!((g - w).abs() < 1e-9, "element {i}: got {g}, want {w}");
        }
    }

    #[test]
    fn mono_and_parallel_views_project_symmetrically() {
        let aspect = 16.0 / 9.0;
        let want = symmetric_perspective(FOV, aspect, NEAR, FAR);
        let parallel = StereoConfig::default(); // convergence 0 = parallel
        for (mode, view) in [
            (DisplayMode::Mono, 0),
            (DisplayMode::SideBySide, 0),
            (DisplayMode::SideBySide, 1),
        ] {
            let got = projection_for_view(mode, &parallel, view, FOV, aspect, NEAR, FAR);
            assert_eq!(got, want, "{mode:?} view {view}");
        }
    }

    #[test]
    fn the_convergence_point_lands_on_the_same_pixel_in_both_eyes() {
        let cfg = StereoConfig {
            convergence_distance: 3.0,
            ..StereoConfig::default()
        };
        let aspect = 572.0 / 828.0;
        // The head-space point straight ahead at the convergence distance,
        // seen from each eye: offset by that eye's separation, projected
        // through that eye's window. Zero parallax means NDC x = 0 in both.
        for view in 0..2 {
            let eye_x = eye_offset_for_view(DisplayMode::SideBySide, &cfg, view) as f64;
            let m = projection_for_view(DisplayMode::SideBySide, &cfg, view, FOV, aspect, NEAR, FAR);
            let x = ndc_x(&m, [-eye_x, 0.0, -(cfg.convergence_distance as f64)]);
            assert!(x.abs() < 1e-6, "view {view} sees the convergence point at NDC x {x}");
        }
    }

    #[test]
    fn depth_reads_crossed_in_front_of_the_convergence_plane_and_uncrossed_behind() {
        let cfg = StereoConfig {
            convergence_distance: 3.0,
            ..StereoConfig::default()
        };
        let aspect = 16.0 / 9.0;
        let disparity = |z: f64| -> f64 {
            let mut x = [0.0; 2];
            for view in 0..2 {
                let eye_x = eye_offset_for_view(DisplayMode::SideBySide, &cfg, view) as f64;
                let m = projection_for_view(DisplayMode::SideBySide, &cfg, view, FOV, aspect, NEAR, FAR);
                x[view as usize] = ndc_x(&m, [-eye_x, 0.0, -z]);
            }
            x[1] - x[0] // right-eye x minus left-eye x, the rig's convention
        };
        assert!(disparity(1.5) < 0.0, "nearer than the plane: crossed (negative)");
        assert!(disparity(6.0) > 0.0, "beyond the plane: uncrossed (positive)");
        // Parallel rig: everything finite is crossed, infinity at the plane.
        let parallel = StereoConfig::default();
        let m_l = projection_for_view(DisplayMode::SideBySide, &parallel, 0, FOV, aspect, NEAR, FAR);
        let m_r = projection_for_view(DisplayMode::SideBySide, &parallel, 1, FOV, aspect, NEAR, FAR);
        let z = 10.0;
        let l = ndc_x(&m_l, [-(left_eye_offset(&parallel)[0] as f64), 0.0, -z]);
        let r = ndc_x(&m_r, [-(right_eye_offset(&parallel)[0] as f64), 0.0, -z]);
        assert!(r - l < 0.0, "parallel: finite depth reads crossed");
    }

    #[test]
    fn blit_rects_tile_the_window_in_view_order() {
        let cfg = StereoConfig::default();
        assert_eq!(
            blit_rects(DisplayMode::Mono, &cfg),
            vec![[0, 0, cfg.viewport_width, cfg.viewport_height]],
            "the single eye fills the window"
        );
        let sbs = blit_rects(DisplayMode::SideBySide, &cfg);
        assert_eq!(sbs.len(), 2);
        assert_eq!(sbs[0], left_viewport_rect(&cfg));
        assert_eq!(sbs[1], right_viewport_rect(&cfg));
        assert_eq!(sbs[0][0] + sbs[0][2], sbs[1][0], "no gap, no overlap");
        assert_eq!(sbs[1][0] + sbs[1][2], total_output_size(&cfg)[0]);
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

    // The UI-layer adoption contract moved from a name list here to a
    // STRUCTURAL rule: ViewManager adopts every CanvasLayer child of Main
    // into the UIViewport (a hand-maintained list went stale the moment a
    // new screen arrived and rendered in one eye). The pin now lives where
    // the tree is: godot/tests/test_sbs_ui.gd sweeps the real Main scene.

    #[test]
    fn display_mode_default_is_mono() {
        assert_eq!(DisplayMode::default(), DisplayMode::Mono);
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
    fn convergence_band_clears_the_cockpit_shell() {
        // The shell nests inside the flight capsule (radius 0.45); the
        // converged plane must never bury itself in the pilot's console.
        assert!(CONVERGENCE_NEAR > 0.45);
        assert!(CONVERGENCE_FAR > CONVERGENCE_NEAR);
    }

}
