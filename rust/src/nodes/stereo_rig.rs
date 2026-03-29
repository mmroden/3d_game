use godot::prelude::*;
use godot::classes::{
    CanvasLayer, Camera3D, DisplayServer, INode3D, Node3D,
    SubViewport, SubViewportContainer,
    viewport::Msaa,
};

use crate::systems::stereo::{
    frustum_offsets, left_eye_offset, right_eye_offset, single_viewport_size, total_output_size,
    StereoConfig,
};

/// Side-by-side stereoscopic camera rig.
/// Creates a CanvasLayer with two SubViewportContainers rendering full-resolution
/// per-eye views placed side by side (full SBS: 3840x1080 total).
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
            enabled: true,
        }
    }

    fn ready(&mut self) {
        self.setup_viewports();
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
        godot_print!(
            "SBS stereo {}",
            if self.enabled { "enabled" } else { "disabled" }
        );
    }
}

impl StereoRig {
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

        self.base_mut().add_child(&canvas_layer);
        self.apply_visibility();
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
}
