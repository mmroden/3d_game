use godot::prelude::*;
use godot::classes::{
    CanvasLayer, Camera3D, DisplayServer, INode3D, Node, Node3D,
    SubViewport, SubViewportContainer, TextureRect,
    viewport::Msaa,
    texture_rect::StretchMode,
};

use crate::systems::stereo::{
    frustum_offsets, left_eye_offset, right_eye_offset, single_viewport_size, total_output_size,
    ui_viewport_size, UI_NODE_NAMES,
    StereoConfig,
};

/// Side-by-side stereoscopic camera rig.
/// Creates a CanvasLayer with two SubViewportContainers rendering full-resolution
/// per-eye views placed side by side (full SBS: 3840x1080 total).
///
/// UI CanvasLayers stay in the main scene tree (never reparented). When SBS is
/// enabled, each CanvasLayer's `custom_viewport` is pointed at a transparent
/// UIViewport whose texture is displayed via TextureRects in both eyes.
/// This keeps input flowing normally through Godot — no `push_input` needed.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct StereoRig {
    base: Base<Node3D>,

    #[export]
    eye_separation: f32,
    #[export]
    depth_strength: f32,
    #[export]
    convergence_distance: f32,
    #[export]
    enabled: bool,
}

#[godot_api]
impl INode3D for StereoRig {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            eye_separation: 0.065,
            depth_strength: 1.0,
            convergence_distance: 0.0,
            enabled: false,
        }
    }

    fn ready(&mut self) {
        self.setup_viewports();
        self.connect_to_game_manager();
        godot_print!(
            "StereoRig ready — SBS {}",
            if self.enabled { "ON" } else { "OFF" }
        );
    }

    fn process(&mut self, _delta: f64) {
        if !self.enabled {
            return;
        }
        self.sync_eye_cameras();
    }
}

#[godot_api]
impl StereoRig {
    #[func]
    pub fn toggle_stereo(&mut self) {
        self.enabled = !self.enabled;
        self.apply_visibility();
        self.resize_window();
        self.redirect_ui();
        godot_print!(
            "SBS stereo {}",
            if self.enabled { "enabled" } else { "disabled" }
        );
    }

    /// Called when GameManager emits options_changed.
    #[func]
    pub fn on_options_changed(&mut self, sbs_enabled: bool, _msaa_enabled: bool) {
        if sbs_enabled != self.enabled {
            self.toggle_stereo();
        }
    }
}

impl StereoRig {
    fn connect_to_game_manager(&mut self) {
        // StereoRig is at Player/StereoRig, GameManager is at Main/GameManager
        // Walk up: StereoRig → Player → Main → GameManager
        let Some(main_scene) = self.base().get_parent().and_then(|p| p.get_parent()) else {
            godot_warn!("StereoRig: could not find Main scene");
            return;
        };
        if let Some(game_mgr) = main_scene.try_get_node_as::<Node>("GameManager") {
            let callable = self.base().callable("on_options_changed");
            if !game_mgr.is_connected("options_changed", &callable) {
                let mut gm = game_mgr;
                gm.connect("options_changed", &callable);
            }
        } else {
            godot_warn!("StereoRig: GameManager not found");
        }
    }

    fn stereo_config(&self) -> StereoConfig {
        StereoConfig {
            eye_separation: self.eye_separation,
            depth_strength: self.depth_strength,
            convergence_distance: self.convergence_distance,
            viewport_width: 1920,
            viewport_height: 1080,
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

        // Now set the ViewportTexture on both TextureRects
        let ui_texture = ui_viewport.get_texture().unwrap();
        left_ui_rect.set_texture(&ui_texture);
        right_ui_rect.set_texture(&ui_texture);

        self.apply_visibility();

        // If starting enabled, redirect UI rendering to UIViewport
        if self.enabled {
            self.redirect_ui();
        }
    }

    fn sync_eye_cameras(&mut self) {
        let parent_transform = self.base().get_global_transform();
        let config = self.stereo_config();

        let l_off = left_eye_offset(&config);
        let r_off = right_eye_offset(&config);
        let [l_frustum, r_frustum] = frustum_offsets(&config);

        let local_x = parent_transform.basis.col_a();

        if let Some(left_cam) = self
            .base()
            .try_get_node_as::<Camera3D>("StereoCanvas/LeftContainer/LeftViewport/LeftCamera")
        {
            let mut cam = left_cam.clone();
            let mut t = parent_transform;
            t.origin += local_x * l_off[0];
            cam.set_global_transform(t);
            cam.set_frustum_offset(Vector2::new(l_frustum, 0.0));
        }

        if let Some(right_cam) = self
            .base()
            .try_get_node_as::<Camera3D>("StereoCanvas/RightContainer/RightViewport/RightCamera")
        {
            let mut cam = right_cam.clone();
            let mut t = parent_transform;
            t.origin += local_x * r_off[0];
            cam.set_global_transform(t);
            cam.set_frustum_offset(Vector2::new(r_frustum, 0.0));
        }
    }

    fn apply_visibility(&mut self) {
        if let Some(canvas) = self
            .base()
            .try_get_node_as::<CanvasLayer>("StereoCanvas")
        {
            let mut layer = canvas.clone();
            layer.set_visible(self.enabled);
        }

        // Show/hide UI overlay TextureRects
        if let Some(left_overlay) = self
            .base()
            .try_get_node_as::<TextureRect>("StereoCanvas/LeftContainer/LeftUIOverlay")
        {
            let mut overlay = left_overlay.clone();
            overlay.set_visible(self.enabled);
        }
        if let Some(right_overlay) = self
            .base()
            .try_get_node_as::<TextureRect>("StereoCanvas/RightContainer/RightUIOverlay")
        {
            let mut overlay = right_overlay.clone();
            overlay.set_visible(self.enabled);
        }
    }

    fn apply_aa(viewport: &mut SubViewport) {
        viewport.set_msaa_3d(Msaa::MSAA_4X);
        viewport.set_use_taa(true);
    }

    fn resize_window(&self) {
        let config = self.stereo_config();
        let mut ds = DisplayServer::singleton();
        if self.enabled {
            let [w, h] = total_output_size(&config);
            ds.window_set_size(Vector2i::new(w as i32, h as i32));
        } else {
            let [w, h] = single_viewport_size(&config);
            ds.window_set_size(Vector2i::new(w as i32, h as i32));
        }
    }

    /// Redirect UI CanvasLayers to render into UIViewport (SBS on)
    /// or back to the main viewport (SBS off).
    /// Nodes stay in the main scene tree — only the render target changes.
    /// Input continues to flow normally since nothing is reparented.
    fn redirect_ui(&self) {
        // Walk up to Main scene (StereoRig → Player → Main)
        let Some(main_scene) = self.base().get_parent().and_then(|p| p.get_parent()) else {
            return;
        };

        if self.enabled {
            let Some(ui_viewport) = self.base().try_get_node_as::<SubViewport>("UIViewport") else {
                return;
            };
            for name in UI_NODE_NAMES {
                if let Some(mut canvas) = main_scene.try_get_node_as::<CanvasLayer>(*name) {
                    canvas.set_custom_viewport(&ui_viewport);
                }
            }
        } else {
            for name in UI_NODE_NAMES {
                if let Some(mut canvas) = main_scene.try_get_node_as::<CanvasLayer>(*name) {
                    // Pass null to reset to default viewport
                    canvas.set_custom_viewport(Gd::null_arg());
                }
            }
        }
    }
}
