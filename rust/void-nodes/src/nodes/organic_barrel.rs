use godot::prelude::*;
use godot::classes::{Area3D, IArea3D};

use super::constants::{groups, methods, signals};
use super::godot_util;
use void_logic::audio_catalog::SfxEvent;

/// A barrel of organics floating in the debris. Flying into it collects the
/// organics (the permanent currency) — mirrors [`super::lootbox::Lootbox`] but
/// grants currency rather than an upgrade, and glows green rather than blue.
#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct OrganicBarrel {
    base: Base<Area3D>,

    #[export]
    organics_amount: i32,
    #[export]
    bob_speed: f32,
    #[export]
    bob_amplitude: f32,

    time: f32,
    origin_y: f32,
    origin_captured: bool,
    collected: bool,
}

#[godot_api]
impl IArea3D for OrganicBarrel {
    fn init(base: Base<Area3D>) -> Self {
        Self {
            base,
            organics_amount: 50,
            bob_speed: 1.5,
            bob_amplitude: 0.25,
            time: 0.0,
            origin_y: 0.0,
            origin_captured: false,
            collected: false,
        }
    }

    fn ready(&mut self) {
        // Detect the player flying into the barrel.
        self.base_mut().set_monitoring(true);
        self.base_mut().set_collision_mask(1); // layer 1 (player)
        self.base_mut().set_collision_layer(0); // don't block anything

        let callable = self.base().callable(methods::ON_BODY_ENTERED);
        self.base_mut().connect(signals::BODY_ENTERED, &callable);

        // Soft green glow marks this as an organics pickup.
        let mut node: Gd<Node3D> = self.base().clone().upcast();
        godot_util::attach_glow_light(&mut node, &[0.2, 0.9, 0.2], 4.0, 5.0);
    }

    fn process(&mut self, delta: f64) {
        if self.collected {
            return;
        }

        // Capture the bob origin lazily so it is correct regardless of whether
        // the spawner positioned the barrel before or after adding it to the tree.
        if !self.origin_captured {
            self.origin_y = self.base().get_global_position().y;
            self.origin_captured = true;
        }

        self.time += delta as f32;
        let mut pos = self.base().get_global_position();
        pos.y = self.origin_y + (self.time * self.bob_speed).sin() * self.bob_amplitude;
        self.base_mut().set_global_position(pos);
        self.base_mut().rotate_y((delta * 1.0) as f32);
    }
}

#[godot_api]
impl OrganicBarrel {
    #[signal]
    fn organics_collected(amount: i32);

    #[func]
    fn on_body_entered(&mut self, body: Gd<Node3D>) {
        if self.collected {
            return;
        }
        if body.is_in_group(groups::PLAYER) {
            self.collect();
        }
    }

    #[func]
    pub fn collect(&mut self) {
        if self.collected {
            return;
        }
        self.collected = true;

        let pos = self.base().get_global_position();
        if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
            audio.bind_mut().play_event_at(SfxEvent::LootPickup, pos);
        }

        let amount = self.organics_amount;
        self.base_mut().emit_signal(
            signals::ORGANICS_COLLECTED,
            &[Variant::from(amount)],
        );

        self.base_mut().queue_free();
    }
}
