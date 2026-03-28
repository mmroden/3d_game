use godot::prelude::*;
use godot::classes::{Node3D, INode3D};

/// Side-by-side stereoscopic camera rig.
/// When enabled, renders two viewports with an eye separation offset.
/// When disabled, falls back to a single centered camera.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct StereoRig {
    base: Base<Node3D>,

    #[export]
    eye_separation: f32,
    #[export]
    enabled: bool,
}

#[godot_api]
impl INode3D for StereoRig {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            eye_separation: 0.065, // ~65mm, average human IPD
            enabled: false,
        }
    }

    fn ready(&mut self) {
        // TODO: create left/right SubViewports and cameras,
        // arrange them side by side when enabled.
        godot_print!("StereoRig ready — SBS {}", if self.enabled { "ON" } else { "OFF" });
    }
}

#[godot_api]
impl StereoRig {
    #[func]
    pub fn toggle_stereo(&mut self) {
        self.enabled = !self.enabled;
        // TODO: show/hide SubViewports, reposition cameras
        godot_print!("SBS stereo {}", if self.enabled { "enabled" } else { "disabled" });
    }
}
