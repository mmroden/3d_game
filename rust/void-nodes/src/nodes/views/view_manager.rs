use godot::prelude::*;
use godot::builtin::Signal;
use godot::classes::{
    Camera3D, CanvasLayer, DisplayServer, INode3D, MeshInstance3D, Node, Node3D,
    QuadMesh, StandardMaterial3D, SubViewport, SubViewportContainer, TextureRect,
    base_material_3d::{ShadingMode, Transparency, CullMode, Flags},
    display_server::WindowMode,
    texture_rect::StretchMode,
    sub_viewport::UpdateMode,
    viewport::Msaa,
};

use crate::nodes::constants::{methods, nodes, signals};
use void_logic::stereo::{
    frustum_offsets, left_eye_offset, right_eye_offset,
    single_viewport_size, ui_plane_size, ui_viewport_size,
    DisplayMode, StereoConfig, UI_NODE_NAMES,
};

/// Default distance (meters) from camera to the floating UI plane in SBS mode.
/// A moderate in-scene depth: close enough (2 m) forces hard convergence and
/// reads as nausea; this sits the HUD comfortably out in the scene. Comfort/
/// readability is governed more by keeping the HUD *inboard* (see the HUD's
/// safe-area band) than by this distance — disparity on a flat plane is uniform,
/// so distance doesn't fix the "off-center element, one eye reaching" blur. The
/// quad scales with distance, so on-screen size is unchanged. The material
/// disables depth test (`setup_ui_plane`) so this depth isn't occluded by walls.
const DEFAULT_UI_PLANE_DISTANCE: f32 = 10.0;

/// First-class view manager: owns the display pipeline (mono or SBS stereo).
///
/// Lives as a direct child of Main. Listens to GameManager's `options_changed`
/// signal and reconfigures rendering accordingly. Never duplicates state that
/// GameManager owns — receives the authoritative `sbs_enabled` via signal.
///
/// UI CanvasLayers always render into a fixed-size UIViewport (set once at
/// startup, never changed). In mono mode a MonoUILayer composites that texture
/// fullscreen; in SBS mode a 3D quad (UIPlane) in the shared World3D displays
/// the UIViewport texture, giving the UI real stereo depth.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct ViewManager {
    base: Base<Node3D>,

    #[export]
    eye_separation: f32,
    #[export]
    depth_strength: f32,
    #[export]
    convergence_distance: f32,
    #[export]
    ui_plane_distance: f32,

    current_mode: DisplayMode,
    /// Window size before entering SBS/fullscreen, so we can restore it.
    pre_sbs_window_size: Vector2i,
}

#[godot_api]
impl INode3D for ViewManager {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            eye_separation: 0.065,
            depth_strength: 1.0,
            convergence_distance: 0.0,
            ui_plane_distance: DEFAULT_UI_PLANE_DISTANCE,
            current_mode: DisplayMode::Mono,
            pre_sbs_window_size: Vector2i::new(0, 0),
        }
    }

    fn ready(&mut self) {
        self.setup_viewports();
        self.set_ui_viewport_once();
        self.connect_to_game_manager();
        self.connect_to_window_resize();
        godot_print!("ViewManager ready — {}", self.current_mode.label());
    }

    fn process(&mut self, _delta: f64) {
        // The left eye renders in BOTH modes (mono = left eye fullscreen, SBS =
        // both eyes), so keep it tracking the player camera every frame. The 3D
        // UI plane only exists in SBS.
        self.sync_eye_cameras();
        if self.current_mode == DisplayMode::SideBySide {
            self.sync_ui_plane();
        }
    }
}

#[godot_api]
impl ViewManager {
    /// Emitted whenever the set of viewports rendering the 3D world
    /// changes (mode toggle). Telemetry listens so it always measures
    /// the eyes that actually draw, never the root compositor.
    #[signal]
    fn render_viewports_changed(viewports: Array<Rid>);

    /// Called when the window resizes (fullscreen transition, manual resize, etc.)
    /// Recomputes all viewport and container sizes from the actual window dims.
    #[func]
    pub fn on_window_size_changed(&mut self) {
        if self.current_mode == DisplayMode::SideBySide {
            self.resize_viewports();
            self.resize_ui_plane();
        }
    }

    /// Called when GameManager emits options_changed.
    #[func]
    pub fn on_options_changed(&mut self, sbs_enabled: bool, msaa_enabled: bool) {
        let target = if sbs_enabled {
            DisplayMode::SideBySide
        } else {
            DisplayMode::Mono
        };

        if target != self.current_mode {
            let sbs = target == DisplayMode::SideBySide;
            self.resize_window(sbs);
            self.current_mode = target;
            self.resize_viewports();
            // Rebuild the UI plane to the new per-eye aspect on the same frame
            // as the toggle, rather than waiting on the OS resize event.
            self.resize_ui_plane();
            self.apply_visibility(sbs);
            self.park_player_camera();

            // Publish the now-active 3D viewports so telemetry re-targets
            // measurement onto the eyes (SBS) or the root (mono).
            let rids = self.active_viewport_rids();
            self.base_mut()
                .emit_signal(signals::RENDER_VIEWPORTS_CHANGED, &[rids.to_variant()]);

            godot_print!("SBS stereo {}", if sbs { "enabled" } else { "disabled" });
        }

        // ViewManager owns all viewport anti-aliasing: apply MSAA to
        // whatever is now the active 3D viewport(s). Runs on every
        // options change — a pure MSAA toggle and a post-mode-switch
        // re-apply both end up correct.
        self.apply_msaa(msaa_enabled);
    }
}

impl ViewManager {
    /// The viewport RIDs currently rendering the 3D world: the left eye
    /// sub-viewport in mono, both eye sub-viewports in SBS. The single
    /// source of truth for "what is being drawn", since ViewManager
    /// owns the display pipeline. Called by typed Rust collaborators
    /// (LevelManager) — never over a Godot string boundary.
    pub(crate) fn active_viewport_rids(&self) -> Array<Rid> {
        let mut rids = Array::new();
        match self.current_mode {
            DisplayMode::SideBySide => {
                for path in [nodes::LEFT_VIEWPORT, nodes::RIGHT_VIEWPORT] {
                    match self.base().try_get_node_as::<SubViewport>(path) {
                        Some(vp) => rids.push(vp.get_viewport_rid()),
                        // The eyes always exist after setup_viewports; a
                        // miss is a structural invariant break, not a
                        // soft skip — make it loud so telemetry can't
                        // silently under-measure.
                        None => godot_warn!(
                            "ViewManager: SBS eye viewport '{}' missing; render telemetry under-measures",
                            path
                        ),
                    }
                }
            }
            DisplayMode::Mono => {
                // Mono renders through the LEFT eye sub-viewport (shown
                // fullscreen), not the root viewport — so MSAA and telemetry
                // must target it, the same eye that actually draws.
                match self.base().try_get_node_as::<SubViewport>(nodes::LEFT_VIEWPORT) {
                    Some(vp) => rids.push(vp.get_viewport_rid()),
                    None => godot_warn!(
                        "ViewManager: mono left viewport missing; render telemetry under-measures"
                    ),
                }
            }
        }
        rids
    }

    /// ViewManager is a direct child of Main; GameManager is a sibling.
    fn connect_to_game_manager(&mut self) {
        let Some(main_scene) = self.base().get_parent() else {
            godot_warn!("ViewManager: could not find Main scene");
            return;
        };
        if let Some(game_mgr) = main_scene.try_get_node_as::<Node>(nodes::GAME_MANAGER) {
            let callable = self.base().callable(methods::ON_OPTIONS_CHANGED);
            if !game_mgr.is_connected(signals::OPTIONS_CHANGED, &callable) {
                let mut gm = game_mgr;
                gm.connect(signals::OPTIONS_CHANGED, &callable);
            }
        } else {
            godot_warn!("ViewManager: GameManager not found");
        }
    }

    /// Connect to the root viewport's size_changed signal so we reactively
    /// resize viewports whenever the window changes (fullscreen, drag, etc.)
    /// Uses CONNECT_DEFERRED to avoid re-entrant borrow panics — the callback
    /// runs next frame, not during the signal emission that triggered the resize.
    fn connect_to_window_resize(&mut self) {
        let Some(viewport) = self.base().get_viewport() else {
            godot_warn!("ViewManager: no viewport for size_changed signal");
            return;
        };
        let callable = self.base().callable(methods::ON_WINDOW_SIZE_CHANGED);
        let signal = Signal::from_object_signal(&viewport, signals::SIZE_CHANGED);
        if !signal.is_connected(&callable) {
            signal.connect_flags(
                &callable,
                godot::classes::object::ConnectFlags::DEFERRED,
            );
        }
    }

    /// Set custom_viewport on all UI CanvasLayers to point at UIViewport.
    /// Called once at startup — never changed again at runtime.
    fn set_ui_viewport_once(&self) {
        let Some(main_scene) = self.base().get_parent() else {
            return;
        };
        let Some(ui_vp) = self.base().try_get_node_as::<SubViewport>(nodes::UI_VIEWPORT) else {
            godot_warn!("ViewManager: UIViewport not found");
            return;
        };
        for name in UI_NODE_NAMES {
            if let Some(mut canvas) = main_scene.try_get_node_as::<CanvasLayer>(*name) {
                canvas.set_custom_viewport(&ui_vp);
            }
        }
    }

    fn stereo_config(&self) -> StereoConfig {
        let ds = DisplayServer::singleton();
        // In fullscreen, window_get_size() may not reflect the new dims yet
        // (macOS animates the transition). Use screen_get_size() which is
        // available immediately.
        let win = match ds.window_get_mode() {
            WindowMode::FULLSCREEN | WindowMode::EXCLUSIVE_FULLSCREEN => ds.screen_get_size(),
            _ => ds.window_get_size(),
        };
        // In SBS mode the window is 2x wide; per-eye width is half that.
        let w = if self.current_mode == DisplayMode::SideBySide {
            (win.x / 2) as u32
        } else {
            win.x as u32
        };
        StereoConfig {
            eye_separation: self.eye_separation,
            depth_strength: self.depth_strength,
            convergence_distance: self.convergence_distance,
            viewport_width: w,
            viewport_height: win.y as u32,
        }
    }

    fn setup_viewports(&mut self) {
        let config = self.stereo_config();
        let [eye_w, eye_h] = single_viewport_size(&config);

        // Share the main scene's World3D so stereo cameras see the same geometry
        let main_world = self.base().get_viewport()
            .expect("ViewManager must be in the scene tree during setup")
            .get_world_3d();

        // CanvasLayer renders on top of the 3D scene
        let mut canvas_layer = CanvasLayer::new_alloc();
        canvas_layer.set_name("StereoCanvas");

        // Left eye — full resolution, positioned at origin
        let mut left_container = SubViewportContainer::new_alloc();
        left_container.set_name("LeftContainer");
        left_container.set_stretch(true);
        left_container.set_position(Vector2::new(0.0, 0.0));
        left_container.set_size(Vector2::new(eye_w as f32, eye_h as f32));

        let mut left_viewport = SubViewport::new_alloc();
        left_viewport.set_name("LeftViewport");
        left_viewport.set_size(Vector2i::new(eye_w as i32, eye_h as i32));
        if let Some(world) = main_world.clone() {
            left_viewport.set_world_3d(&world);
        }
        Self::apply_aa(&mut left_viewport);
        // 3D audio listener: in Godot 4.6 positional audio routes through the
        // *current camera's* viewport (godot#94403, fixed in 4.7), and our only
        // current cameras are the eye cameras inside these SubViewports. So the
        // viewport that owns the listening camera must opt into 3D audio — without
        // this, every AudioStreamPlayer3D is silent. The left eye is the render
        // camera in both mono and SBS, and it tracks the player each frame
        // (sync_eye_cameras), so it's the right listener.
        left_viewport.set_as_audio_listener_3d(true);

        let mut left_cam = Camera3D::new_alloc();
        left_cam.set_name("LeftCamera");

        left_viewport.add_child(&left_cam);
        left_container.add_child(&left_viewport);
        canvas_layer.add_child(&left_container);

        // Right eye — full resolution, positioned after left
        let mut right_container = SubViewportContainer::new_alloc();
        right_container.set_name("RightContainer");
        right_container.set_stretch(true);
        right_container.set_position(Vector2::new(eye_w as f32, 0.0));
        right_container.set_size(Vector2::new(eye_w as f32, eye_h as f32));

        let mut right_viewport = SubViewport::new_alloc();
        right_viewport.set_name("RightViewport");
        right_viewport.set_size(Vector2i::new(eye_w as i32, eye_h as i32));
        if let Some(world) = main_world {
            right_viewport.set_world_3d(&world);
        }
        Self::apply_aa(&mut right_viewport);

        let mut right_cam = Camera3D::new_alloc();
        right_cam.set_name("RightCamera");

        right_viewport.add_child(&right_cam);
        right_container.add_child(&right_viewport);
        canvas_layer.add_child(&right_container);

        // --- UI SubViewport for overlaying UI on both eyes ---
        let [ui_w, ui_h] = ui_viewport_size(&config);

        let mut ui_viewport = SubViewport::new_alloc();
        ui_viewport.set_name("UIViewport");
        ui_viewport.set_size(Vector2i::new(ui_w as i32, ui_h as i32));
        ui_viewport.set_transparent_background(true);
        ui_viewport.set_update_mode(UpdateMode::ALWAYS);

        // UI TextureRect in left eye (kept for fallback but hidden in SBS)
        let mut left_ui_rect = TextureRect::new_alloc();
        left_ui_rect.set_name("LeftUIOverlay");
        left_ui_rect.set_size(Vector2::new(eye_w as f32, eye_h as f32));
        left_ui_rect.set_stretch_mode(StretchMode::SCALE);
        left_ui_rect.set_mouse_filter(godot::classes::control::MouseFilter::IGNORE);
        left_container.add_child(&left_ui_rect);

        // UI TextureRect in right eye (kept for fallback but hidden in SBS)
        let mut right_ui_rect = TextureRect::new_alloc();
        right_ui_rect.set_name("RightUIOverlay");
        right_ui_rect.set_size(Vector2::new(eye_w as f32, eye_h as f32));
        right_ui_rect.set_stretch_mode(StretchMode::SCALE);
        right_ui_rect.set_mouse_filter(godot::classes::control::MouseFilter::IGNORE);
        right_container.add_child(&right_ui_rect);

        self.base_mut().add_child(&canvas_layer);

        // Add UIViewport as child of this node (not under CanvasLayer)
        self.base_mut().add_child(&ui_viewport);

        // MonoUILayer — shows UIViewport texture in mono mode (fullscreen overlay)
        let mut mono_ui_layer = CanvasLayer::new_alloc();
        mono_ui_layer.set_name("MonoUILayer");

        let mut mono_ui_rect = TextureRect::new_alloc();
        mono_ui_rect.set_name("MonoUIRect");
        mono_ui_rect.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        mono_ui_rect.set_stretch_mode(StretchMode::SCALE);
        mono_ui_rect.set_mouse_filter(godot::classes::control::MouseFilter::IGNORE);

        mono_ui_layer.add_child(&mono_ui_rect);
        self.base_mut().add_child(&mono_ui_layer);

        // Set UIViewport texture on overlay rects
        let ui_texture = ui_viewport.get_texture()
            .expect("UIViewport must have a texture after creation");
        left_ui_rect.set_texture(&ui_texture);
        right_ui_rect.set_texture(&ui_texture);
        mono_ui_rect.set_texture(&ui_texture);

        // --- 3D UI plane: world-space quad for SBS stereo depth ---
        self.setup_ui_plane(ui_texture);

        // Mono by default: left eye fullscreen, right eye + UIPlane hidden, UI
        // via the fullscreen MonoUILayer. The player camera is parked non-current
        // so the eye viewport owns the render.
        self.apply_visibility(false);
        self.park_player_camera();
    }

    /// Create a MeshInstance3D with a QuadMesh textured with the UIViewport.
    /// In SBS mode this floats in front of the camera so both stereo cameras
    /// render it with natural parallax, giving the UI real depth.
    fn setup_ui_plane(&mut self, ui_texture: Gd<godot::classes::ViewportTexture>) {
        // Read FOV and aspect from the actual camera rather than hardcoding
        let (fov, aspect) = self.camera_fov_and_aspect();
        let [quad_w, quad_h] = ui_plane_size(self.ui_plane_distance, fov, aspect);

        let mut quad_mesh = QuadMesh::new_gd();
        quad_mesh.set_size(Vector2::new(quad_w, quad_h));

        let mut material = StandardMaterial3D::new_gd();
        material.set_shading_mode(ShadingMode::UNSHADED);
        material.set_transparency(Transparency::ALPHA);
        material.set_cull_mode(CullMode::DISABLED);
        // The plane floats deep in the scene for stereo comfort; without this it
        // would be occluded by any nearer wall. Draw it on top regardless — its
        // stereo depth still comes from its 3D distance, so the HUD reads deep
        // and stays visible.
        material.set_flag(Flags::DISABLE_DEPTH_TEST, true);
        // Upcast ViewportTexture → Texture2D for set_texture
        let texture_2d: Gd<godot::classes::Texture2D> = ui_texture.upcast();
        material.set_texture(
            godot::classes::base_material_3d::TextureParam::ALBEDO,
            &texture_2d,
        );

        quad_mesh.set_material(&material);

        let mut ui_plane = MeshInstance3D::new_alloc();
        ui_plane.set_name("UIPlane");
        ui_plane.set_mesh(&quad_mesh);
        ui_plane.set_visible(false);

        self.base_mut().add_child(&ui_plane);
    }

    /// FOV from the player camera, and the aspect of the UI plane. The plane
    /// shows the UIViewport texture, which spans the FULL window, so its aspect
    /// must be the full-window aspect (not per-eye) — otherwise a wide texture
    /// is crushed onto a narrow quad and the UI reads squished.
    fn camera_fov_and_aspect(&self) -> (f32, f32) {
        let Some(main_scene) = self.base().get_parent() else {
            return (75.0, 16.0 / 9.0);
        };
        let Some(camera) = main_scene.try_get_node_as::<Camera3D>(nodes::PLAYER_CAMERA) else {
            return (75.0, 16.0 / 9.0);
        };
        let fov = camera.get_fov();
        let config = self.stereo_config();
        let sbs = self.current_mode == DisplayMode::SideBySide;
        let full_w = if sbs { config.viewport_width * 2 } else { config.viewport_width };
        let aspect = if config.viewport_height > 0 {
            full_w as f32 / config.viewport_height as f32
        } else {
            16.0 / 9.0
        };
        (fov, aspect)
    }

    /// Resize the UI plane quad to match current camera FOV and aspect ratio.
    fn resize_ui_plane(&self) {
        let Some(ui_plane) = self.base().try_get_node_as::<MeshInstance3D>(nodes::UI_PLANE) else {
            return;
        };
        let (fov, aspect) = self.camera_fov_and_aspect();
        let [quad_w, quad_h] = ui_plane_size(self.ui_plane_distance, fov, aspect);

        let plane = ui_plane.clone();
        if let Some(mesh) = plane.get_mesh() {
            if let Ok(mut quad) = mesh.try_cast::<QuadMesh>() {
                quad.set_size(Vector2::new(quad_w, quad_h));
            }
        }
    }

    /// Position the UIPlane in front of the mono camera each frame.
    fn sync_ui_plane(&self) {
        let Some(main_scene) = self.base().get_parent() else {
            return;
        };
        let Some(camera) = main_scene.try_get_node_as::<Camera3D>(nodes::PLAYER_CAMERA) else {
            return;
        };
        let Some(ui_plane) = self.base().try_get_node_as::<MeshInstance3D>(nodes::UI_PLANE) else {
            return;
        };

        let cam_transform = camera.get_global_transform();
        let forward = -cam_transform.basis.col_c();
        let plane_origin = cam_transform.origin + forward * self.ui_plane_distance;

        let mut plane = ui_plane.clone();
        plane.set_global_transform(Transform3D::new(cam_transform.basis, plane_origin));
    }

    /// Sync stereo cameras to the mono camera's transform.
    /// ViewManager is a sibling of Player under Main, so we look up
    /// Player/Camera3D via the parent scene.
    fn sync_eye_cameras(&mut self) {
        let Some(main_scene) = self.base().get_parent() else {
            return;
        };
        let Some(camera) = main_scene.try_get_node_as::<Camera3D>(nodes::PLAYER_CAMERA) else {
            return;
        };

        let camera_transform = camera.get_global_transform();
        let config = self.stereo_config();

        // Mono = a single centered eye: no horizontal separation, no frustum
        // skew. SBS splits the eyes apart with the configured stereo offsets.
        let mono = self.current_mode != DisplayMode::SideBySide;
        let (l_off, r_off, l_frustum, r_frustum) = if mono {
            ([0.0_f32; 3], [0.0_f32; 3], 0.0_f32, 0.0_f32)
        } else {
            let [lf, rf] = frustum_offsets(&config);
            (left_eye_offset(&config), right_eye_offset(&config), lf, rf)
        };

        let local_x = camera_transform.basis.col_a();

        if let Some(left_cam) = self
            .base()
            .try_get_node_as::<Camera3D>(nodes::LEFT_CAMERA)
        {
            let mut cam = left_cam.clone();
            let mut t = camera_transform;
            t.origin += local_x * l_off[0];
            cam.set_global_transform(t);
            cam.set_frustum_offset(Vector2::new(l_frustum, 0.0));
        }

        // The right eye only renders in SBS, but positioning it in mono is
        // harmless (its viewport is hidden) and keeps the code branch-free.
        if let Some(right_cam) = self
            .base()
            .try_get_node_as::<Camera3D>(nodes::RIGHT_CAMERA)
        {
            let mut cam = right_cam.clone();
            let mut t = camera_transform;
            t.origin += local_x * r_off[0];
            cam.set_global_transform(t);
            cam.set_frustum_offset(Vector2::new(r_frustum, 0.0));
        }
    }

    fn apply_visibility(&mut self, sbs: bool) {
        // StereoCanvas hosts the eye viewports and is ALWAYS visible now — the
        // left eye is the render path in both modes (mono = left eye fullscreen).
        if let Some(mut canvas) = self.base().try_get_node_as::<CanvasLayer>(nodes::STEREO_CANVAS) {
            canvas.set_visible(true);
        }
        // The right eye only renders in SBS; in mono the left container is sized
        // fullscreen (see resize_viewports / stereo_config).
        if let Some(mut right) = self.base().try_get_node_as::<SubViewportContainer>(nodes::RIGHT_CONTAINER) {
            right.set_visible(sbs);
        }
        if let Some(mut left) = self.base().try_get_node_as::<SubViewportContainer>(nodes::LEFT_CONTAINER) {
            left.set_visible(true);
        }
        // UI: in mono the HUD is drawn by the fullscreen MonoUILayer (a proper
        // CanvasLayer at 1:1, so the tiny center reticle survives); in SBS the
        // 3D UIPlane takes over, so the flat layer hides. The in-eye overlays
        // live inside the SubViewportContainers, which scale the reticle away —
        // they're retired (always hidden), kept only so the texture wiring in
        // setup stays uniform.
        if let Some(mut mono) = self.base().try_get_node_as::<CanvasLayer>(nodes::MONO_UI_LAYER) {
            mono.set_visible(!sbs);
        }
        if let Some(mut overlay) = self.base().try_get_node_as::<TextureRect>(nodes::LEFT_UI_OVERLAY) {
            overlay.set_visible(false);
        }
        if let Some(mut overlay) = self.base().try_get_node_as::<TextureRect>(nodes::RIGHT_UI_OVERLAY) {
            overlay.set_visible(false);
        }
        // 3D UI plane: visible in SBS, hidden in mono
        if let Some(mut plane) = self.base().try_get_node_as::<MeshInstance3D>(nodes::UI_PLANE) {
            plane.set_visible(sbs);
        }
    }

    /// Update container/overlay sizes to match current window geometry.
    /// SubViewports with stretch-enabled parents resize automatically —
    /// only touch containers and overlay TextureRects.
    fn resize_viewports(&mut self) {
        let config = self.stereo_config();
        let [eye_w, eye_h] = single_viewport_size(&config);
        let [ui_w, ui_h] = ui_viewport_size(&config);

        // Left container: position (0,0), size = per-eye
        if let Some(mut c) = self.base().try_get_node_as::<SubViewportContainer>(nodes::LEFT_CONTAINER) {
            c.set_position(Vector2::new(0.0, 0.0));
            c.set_size(Vector2::new(eye_w as f32, eye_h as f32));
        }
        if let Some(mut r) = self.base().try_get_node_as::<TextureRect>(nodes::LEFT_UI_OVERLAY) {
            r.set_size(Vector2::new(eye_w as f32, eye_h as f32));
        }

        // Right container: position (eye_w, 0), size = per-eye
        if let Some(mut c) = self.base().try_get_node_as::<SubViewportContainer>(nodes::RIGHT_CONTAINER) {
            c.set_position(Vector2::new(eye_w as f32, 0.0));
            c.set_size(Vector2::new(eye_w as f32, eye_h as f32));
        }
        if let Some(mut r) = self.base().try_get_node_as::<TextureRect>(nodes::RIGHT_UI_OVERLAY) {
            r.set_size(Vector2::new(eye_w as f32, eye_h as f32));
        }

        // UIViewport spans the FULL window, not per-eye. The UI CanvasLayers
        // anchor their controls to the root window (custom_viewport redirects
        // rendering, not layout), so a centered panel sits at window-center. If
        // the UIViewport were only per-eye wide, window-center would land at its
        // right edge — which is exactly the "menus shoved right in SBS" bug.
        let sbs = self.current_mode == DisplayMode::SideBySide;
        let ui_full_w = if sbs { ui_w * 2 } else { ui_w };
        if let Some(mut vp) = self.base().try_get_node_as::<SubViewport>(nodes::UI_VIEWPORT) {
            vp.set_size(Vector2i::new(ui_full_w as i32, ui_h as i32));
        }
    }

    /// Set up eye anti-aliasing at construction. MSAA starts disabled —
    /// it is driven by the authoritative option via `apply_eye_msaa`,
    /// applied at startup by GameManager's options broadcast. TAA is
    /// always on (cheap, separate from the MSAA option).
    fn apply_aa(viewport: &mut SubViewport) {
        viewport.set_msaa_3d(Msaa::DISABLED);
        viewport.set_use_taa(true);
    }

    /// Apply the MSAA option to the viewport(s) actually rendering the
    /// 3D world — both eye sub-viewports in SBS, the LEFT eye sub-viewport
    /// (shown fullscreen) in mono. The root viewport never draws the world
    /// (it only hosts the eye canvas), so it must never be targeted — same
    /// set as `active_viewport_rids`. ViewManager is the sole owner of
    /// viewport anti-aliasing.
    fn apply_msaa(&mut self, enabled: bool) {
        let msaa = if enabled { Msaa::MSAA_4X } else { Msaa::DISABLED };
        let paths: &[&str] = match self.current_mode {
            DisplayMode::SideBySide => &[nodes::LEFT_VIEWPORT, nodes::RIGHT_VIEWPORT],
            DisplayMode::Mono => &[nodes::LEFT_VIEWPORT],
        };
        for path in paths {
            if let Some(mut vp) = self.base().try_get_node_as::<SubViewport>(*path) {
                vp.set_msaa_3d(msaa);
            }
        }
    }

    fn resize_window(&mut self, sbs: bool) {
        let mut ds = DisplayServer::singleton();
        if sbs {
            // Remember current window size before going fullscreen
            self.pre_sbs_window_size = ds.window_get_size();
            ds.window_set_mode(WindowMode::FULLSCREEN);
        } else {
            ds.window_set_mode(WindowMode::WINDOWED);
            // Restore the window size from before SBS was enabled
            if self.pre_sbs_window_size.x > 0 && self.pre_sbs_window_size.y > 0 {
                ds.window_set_size(self.pre_sbs_window_size);
            }
        }
    }

    /// The player's own `Camera3D` is never the render camera now — the eye
    /// SubViewport cameras draw the world in both modes (mono = left eye). It
    /// stays non-current so it can't fight the eye viewports for the root
    /// surface, which was the black-in-mono bug. It remains the *reference*
    /// transform the eye cameras track each frame.
    fn park_player_camera(&self) {
        let Some(main_scene) = self.base().get_parent() else {
            return;
        };
        if let Some(mut camera) = main_scene.try_get_node_as::<Camera3D>(nodes::PLAYER_CAMERA) {
            camera.set_current(false);
        }
    }

    // (3D audio listener is enabled directly on the left eye SubViewport in
    // setup_viewports — see the `set_as_audio_listener_3d` call there.)
}
