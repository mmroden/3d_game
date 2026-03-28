use godot::prelude::*;
use godot::classes::{Area3D, IArea3D};

/// A pickup that grants the player an upgrade.
#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct Lootbox {
    base: Base<Area3D>,

    #[export]
    upgrade_name: GString,
    #[export]
    bob_speed: f32,
    #[export]
    bob_amplitude: f32,

    time: f32,
    origin_y: f32,
    collected: bool,
}

#[godot_api]
impl IArea3D for Lootbox {
    fn init(base: Base<Area3D>) -> Self {
        Self {
            base,
            upgrade_name: GString::new(),
            bob_speed: 2.0,
            bob_amplitude: 0.3,
            time: 0.0,
            origin_y: 0.0,
            collected: false,
        }
    }

    fn ready(&mut self) {
        self.origin_y = self.base().get_global_position().y;
    }

    fn process(&mut self, delta: f64) {
        if self.collected {
            return;
        }

        // Gentle bobbing animation
        self.time += delta as f32;
        let mut pos = self.base().get_global_position();
        pos.y = self.origin_y + (self.time * self.bob_speed).sin() * self.bob_amplitude;
        self.base_mut().set_global_position(pos);

        // Slow rotation
        self.base_mut().rotate_y((delta * 1.5) as f32);
    }
}

#[godot_api]
impl Lootbox {
    #[func]
    pub fn collect(&mut self) {
        if self.collected {
            return;
        }
        self.collected = true;
        // TODO: emit signal with upgrade data, play effect, queue_free
        self.base_mut().queue_free();
    }
}
