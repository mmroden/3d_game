use godot::prelude::*;
use godot::classes::{
    Camera3D, CanvasLayer, DisplayServer, INode3D, Node, Node3D,
    SubViewport, SubViewportContainer, TextureRect,
    texture_rect::StretchMode,
    sub_viewport::UpdateMode,
    viewport::Msaa,
};

use crate::nodes::constants::{methods, nodes, signals};
use void_logic::stereo::{
    frustum_offsets, left_eye_offset, right_eye_offset,
    single_viewport_size, ui_viewport_size,
    DisplayMode, StereoConfig, UI_NODE_NAMES,
};

/// First-class view manager: owns the display pipeline (mono or SBS stereo).
///
/// Lives as a direct child of Main. Listens to GameManager's `options_changed`
/// signal and reconfigures rendering accordingly. Never duplicates state that
/// GameManager owns — receives the authoritative `sbs_enabled` via signal.
///
/// UI CanvasLayers always render into a fixed-size UIViewport (set once at
/// startup, never changed). In mono mode a MonoUILayer composites that texture
/// fullscreen; in SBS mode per-eye TextureRects composite it into each eye.
/// Toggling only changes visibility — no runtime viewport redirect.
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

    current_mode: DisplayMode,
}

#[godot_api]
impl INode3D for ViewManager {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            eye_separation: 0.065,
            depth_strength: 1.0,
            convergence_distance: 0.0,
            current_mode: DisplayMode::Mono,
        }
    }

    fn ready(&mut self) {
        self.setup_viewports();
        self.set_ui_viewport_once();
        self.connect_to_game_manager();
        godot_print!("ViewManager ready — {}", self.current_mode.label());
    }

    fn process(&mut self, _delta: f64) {
        if self.current_mode != DisplayMode::SideBySide {
            return;
        }
        self.sync_eye_cameras();
    }
}

#[godot_api]
impl ViewManager {
    /// Called when GameManager emits options_changed.
    #[func]
    pub fn on_options_changed(&mut self, sbs_enabled: bool, _msaa_enabled: bool) {
        let target = if sbs_enabled {
            DisplayMode::SideBySide
        } else {
            DisplayMode::Mono
        };

        if target == self.current_mode {
            return;
        }

        let sbs = target == DisplayMode::SideBySide;
        self.resize_window(sbs);
        self.current_mode = target;
        self.resize_viewports();
        self.apply_visibility(sbs);
        self.toggle_mono_camera(sbs);

        godot_print!("SBS stereo {}", if sbs { "enabled" } else { "disabled" });
    }
}

impl ViewManager {
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
        let win = DisplayServer::singleton().window_get_size();
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
        let main_world = self.base().get_viewport().unwrap().get_world_3d();

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

        // UI TextureRect in left eye
        let mut left_ui_rect = TextureRect::new_alloc();
        left_ui_rect.set_name("LeftUIOverlay");
        left_ui_rect.set_size(Vector2::new(eye_w as f32, eye_h as f32));
        left_ui_rect.set_stretch_mode(StretchMode::SCALE);
        left_ui_rect.set_mouse_filter(godot::classes::control::MouseFilter::IGNORE);
        left_container.add_child(&left_ui_rect);

        // UI TextureRect in right eye
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

        // Set UIViewport texture on all three overlay rects
        let ui_texture = ui_viewport.get_texture().unwrap();
        left_ui_rect.set_texture(&ui_texture);
        right_ui_rect.set_texture(&ui_texture);
        mono_ui_rect.set_texture(&ui_texture);

        // Mono mode by default: MonoUILayer visible, StereoCanvas hidden
        self.apply_visibility(false);
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

        let l_off = left_eye_offset(&config);
        let r_off = right_eye_offset(&config);
        let [l_frustum, r_frustum] = frustum_offsets(&config);

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
        // StereoCanvas: visible in SBS mode
        if let Some(canvas) = self.base().try_get_node_as::<CanvasLayer>(nodes::STEREO_CANVAS) {
            let mut layer = canvas.clone();
            layer.set_visible(sbs);
        }
        // MonoUILayer: visible in mono mode
        if let Some(mono) = self.base().try_get_node_as::<CanvasLayer>(nodes::MONO_UI_LAYER) {
            let mut layer = mono.clone();
            layer.set_visible(!sbs);
        }
        // UI overlays in stereo eyes
        for path in [
            nodes::LEFT_UI_OVERLAY,
            nodes::RIGHT_UI_OVERLAY,
        ] {
            if let Some(overlay) = self.base().try_get_node_as::<TextureRect>(path) {
                let mut o = overlay.clone();
                o.set_visible(sbs);
            }
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

        // UIViewport has no SubViewportContainer parent — resize directly
        if let Some(mut vp) = self.base().try_get_node_as::<SubViewport>(nodes::UI_VIEWPORT) {
            vp.set_size(Vector2i::new(ui_w as i32, ui_h as i32));
        }
    }

    fn apply_aa(viewport: &mut SubViewport) {
        viewport.set_msaa_3d(Msaa::MSAA_4X);
        viewport.set_use_taa(true);
    }

    fn resize_window(&self, sbs: bool) {
        let mut ds = DisplayServer::singleton();
        let current = ds.window_get_size();
        if sbs {
            // Mono → SBS: double width
            ds.window_set_size(Vector2i::new(current.x * 2, current.y));
        } else {
            // SBS → Mono: halve width
            ds.window_set_size(Vector2i::new(current.x / 2, current.y));
        }
    }

    /// Toggle the mono camera when switching display modes.
    /// In SBS mode the mono camera is disabled; stereo cameras render instead.
    fn toggle_mono_camera(&self, sbs: bool) {
        let Some(main_scene) = self.base().get_parent() else {
            return;
        };
        if let Some(mut camera) = main_scene.try_get_node_as::<Camera3D>(nodes::PLAYER_CAMERA) {
            camera.set_current(!sbs);
        }
    }
}

