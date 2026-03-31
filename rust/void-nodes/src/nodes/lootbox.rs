use godot::prelude::*;
use godot::classes::{Area3D, IArea3D};

use super::constants::{groups, methods, signals};

/// A pickup that grants the player an upgrade.
#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct Lootbox {
    base: Base<Area3D>,

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
            bob_speed: 2.0,
            bob_amplitude: 0.3,
            time: 0.0,
            origin_y: 0.0,
            collected: false,
        }
    }

    fn ready(&mut self) {
        self.origin_y = self.base().get_global_position().y;

        // Monitor for bodies entering (player collision)
        self.base_mut().set_monitoring(true);
        self.base_mut().set_collision_mask(1); // Detect layer 1 (player)
        self.base_mut().set_collision_layer(0); // Don't block anything

        // Connect body_entered signal
        let callable = self.base().callable(methods::ON_BODY_ENTERED);
        self.base_mut().connect(signals::BODY_ENTERED, &callable);
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
    fn on_body_entered(&mut self, body: Gd<Node3D>) {
        if self.collected {
            return;
        }
        // Check if it's the player
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

        // Generate and apply a random upgrade to the player
        use void_logic::upgrade::random_upgrade;
        use rand::SeedableRng;
        use rand::rngs::SmallRng;

        // Use current position as entropy for variety
        let pos = self.base().get_global_position();
        let seed = ((pos.x * 1000.0) as u64)
            .wrapping_add((pos.z * 7777.0) as u64)
            .wrapping_add((self.time * 9999.0) as u64);
        let mut rng = SmallRng::seed_from_u64(seed);
        let upgrade = random_upgrade(&mut rng);

        godot_print!("Collected upgrade: {} (x{:.0}%)", upgrade.name, (upgrade.multiplier - 1.0) * 100.0);

        // Apply to player's loadout via signal/method call
        let tree = self.base().get_tree();
        let players = tree.get_nodes_in_group(groups::PLAYER);
        if let Some(player) = players.get(0) {
            let mut player: Gd<Node> = player.clone();
            player.call(
                methods::APPLY_UPGRADE,
                &[
                    Variant::from(GString::from(&upgrade.name)),
                    Variant::from(upgrade.kind as i32),
                    Variant::from(upgrade.multiplier),
                ],
            );
        }

        self.base_mut().queue_free();
    }
}
