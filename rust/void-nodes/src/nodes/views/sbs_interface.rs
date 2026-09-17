//! The side-by-side display interface: Godot's `XRInterfaceExtension`
//! playing the xReal glasses as a crippled headset — two eye ports, no
//! head tracking, no eye tracking (docs/design/xr_rig.md §3). Mono is the
//! same interface with one view. The root viewport renders through it
//! (`use_xr`), one multiview pass; the eye math is `void_logic::stereo`,
//! pure and tested — this file only adapts it to the engine's virtuals.

use godot::classes::xr_interface::{Capabilities, PlayAreaMode, TrackingStatus};
use godot::classes::{DisplayServer, IXrInterfaceExtension, XrInterfaceExtension};
use godot::obj::EngineEnum;
use godot::prelude::*;

use void_logic::stereo::{
    blit_rects, eye_offset_for_view, per_eye_size, projection_for_view, view_count,
    DisplayMode, StereoConfig,
};

/// The OS window in pixels — the truth every eye size derives from.
/// On notched Macs a fullscreen window is SHORTER than the screen (the
/// camera-housing band), so the screen size is only the fallback for a
/// degenerate report (macOS animates the fullscreen transition and the
/// window can lag a beat; the next frame re-reads it — playtest 2026-07-09).
pub(crate) fn window_pixels() -> [u32; 2] {
    let ds = DisplayServer::singleton();
    let mut win = ds.window_get_size();
    if win.x <= 0 || win.y <= 0 {
        win = ds.screen_get_size();
    }
    [win.x.max(0) as u32, win.y.max(0) as u32]
}

// `tool`: a virtual extension class runs in the editor too (gdext requires
// the flag); the interface only registers when ViewManager asks, so the
// editor never renders through it.
#[derive(GodotClass)]
#[class(tool, base=XrInterfaceExtension)]
pub struct SbsInterface {
    base: Base<XrInterfaceExtension>,
    initialized: bool,
    mode: DisplayMode,
    /// The stereo dials as last pushed by ViewManager (interaxial, depth
    /// strength, convergence); the per-eye dimensions inside are refreshed
    /// from the window whenever the engine asks for the render target.
    config: StereoConfig,
    /// Vertical field of view (degrees) — the `XRCamera3D`'s authored
    /// `fov`, which the engine ignores under `use_xr`; this is its consumer.
    fov_v_deg: f32,
}

impl SbsInterface {
    /// The name the interface registers under with the `XRServer`.
    pub const NAME: &'static str = "SBS";

    /// ViewManager's one push per frame: display mode, the stereo dials,
    /// and the camera's vertical field of view.
    pub fn configure(&mut self, mode: DisplayMode, dials: StereoConfig, fov_v_deg: f32) {
        self.mode = mode;
        self.config = dials;
        self.fov_v_deg = fov_v_deg;
        self.refresh_window();
    }

    /// The per-eye render target for the current window and mode.
    pub fn render_target_size(&self) -> [u32; 2] {
        per_eye_size(self.mode, window_pixels())
    }

    fn refresh_window(&mut self) {
        let [w, h] = self.render_target_size();
        self.config.viewport_width = w;
        self.config.viewport_height = h;
    }
}

#[godot_api]
impl IXrInterfaceExtension for SbsInterface {
    fn init(base: Base<XrInterfaceExtension>) -> Self {
        Self {
            base,
            initialized: false,
            mode: DisplayMode::Mono,
            config: StereoConfig::default(),
            fov_v_deg: 75.0,
        }
    }

    fn get_name(&self) -> StringName {
        StringName::from(Self::NAME)
    }

    fn get_capabilities(&self) -> u32 {
        (Capabilities::MONO.ord() | Capabilities::STEREO.ord()) as u32
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn initialize(&mut self) -> bool {
        self.initialized = true;
        true
    }

    fn uninitialize(&mut self) {
        self.initialized = false;
    }

    fn get_system_info(&self) -> AnyDictionary {
        VarDictionary::new().upcast_any_dictionary()
    }

    /// No head pose exists: eyes front, head still. `NOT_TRACKING` is the
    /// honest report; `XRCamera3D` keeps its authored transform.
    fn get_tracking_status(&self) -> TrackingStatus {
        TrackingStatus::NOT_TRACKING
    }

    fn supports_play_area_mode(&self, _mode: PlayAreaMode) -> bool {
        false
    }

    fn get_play_area_mode(&self) -> PlayAreaMode {
        PlayAreaMode::UNKNOWN
    }

    fn set_play_area_mode(&self, _mode: PlayAreaMode) -> bool {
        false
    }

    fn get_play_area(&self) -> PackedVector3Array {
        PackedVector3Array::new()
    }

    /// Polled by the engine every frame — the window IS the eye size, so a
    /// resize can never leave a stale render target behind.
    fn get_render_target_size(&mut self) -> Vector2 {
        self.refresh_window();
        Vector2::new(self.config.viewport_width as f32, self.config.viewport_height as f32)
    }

    fn get_view_count(&mut self) -> u32 {
        view_count(self.mode)
    }

    /// The head sits on the origin: the `XRCamera3D` is the eyepoint.
    fn get_camera_transform(&mut self) -> Transform3D {
        Transform3D::IDENTITY
    }

    /// Each eye straddles the camera along its local X by half the
    /// interaxial; the single mono eye sits on the axis.
    fn get_transform_for_view(&mut self, view: u32, cam_transform: Transform3D) -> Transform3D {
        let offset = eye_offset_for_view(self.mode, &self.config, view);
        cam_transform * Transform3D::new(Basis::IDENTITY, Vector3::new(offset, 0.0, 0.0))
    }

    /// The symmetric perspective per eye, its window slid by the
    /// convergence shift — the shipped off-axis geometry.
    fn get_projection_for_view(&mut self, view: u32, aspect: f64, z_near: f64, z_far: f64) -> PackedFloat64Array {
        let m = projection_for_view(
            self.mode,
            &self.config,
            view,
            self.fov_v_deg,
            aspect as f32,
            z_near as f32,
            z_far as f32,
        );
        PackedFloat64Array::from(&m)
    }

    fn get_vrs_texture(&mut self) -> Rid {
        Rid::Invalid
    }

    fn process(&mut self) {}

    fn pre_render(&mut self) {}

    fn pre_draw_viewport(&mut self, _render_target: Rid) -> bool {
        true
    }

    /// The rendered views land in the window: the single eye fills it, the
    /// pair tiles it left|right. Layer blits address the multiview array;
    /// the mono target is a plain texture and is blitted as such.
    fn post_draw_viewport(&mut self, render_target: Rid, screen_rect: Rect2) {
        if screen_rect == Rect2::default() {
            // A SubViewport, not the window: nothing to put on screen.
            return;
        }
        let layered = view_count(self.mode) > 1;
        let origin = screen_rect.position;
        for (layer, [x, y, w, h]) in blit_rects(self.mode, &self.config).into_iter().enumerate() {
            let dst = Rect2i::new(
                Vector2i::new(origin.x as i32 + x as i32, origin.y as i32 + y as i32),
                Vector2i::new(w as i32, h as i32),
            );
            self.base_mut().add_blit(
                render_target,
                Rect2::new(Vector2::ZERO, Vector2::ONE),
                dst,
                layered,
                layer as u32,
                false,
                Vector2::ZERO,
                0.0,
                0.0,
                1.0,
                1.0,
            );
        }
    }

    fn end_frame(&mut self) {}

    fn get_suggested_tracker_names(&self) -> PackedStringArray {
        PackedStringArray::new()
    }

    fn get_suggested_pose_names(&self, _tracker_name: StringName) -> PackedStringArray {
        PackedStringArray::new()
    }
}
