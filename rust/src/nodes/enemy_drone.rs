use godot::prelude::*;
use godot::classes::{CharacterBody3D, ICharacterBody3D};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DroneState {
    Idle,
    Dead,
}

/// A hostile drone that chases and attacks the player.
#[derive(GodotClass)]
#[class(base=CharacterBody3D)]
pub struct EnemyDrone {
    base: Base<CharacterBody3D>,

    #[export]
    speed: f32,
    #[export]
    health: f32,
    #[export]
    detection_range: f32,
    #[export]
    attack_range: f32,
    #[export]
    damage: f32,

    state: DroneState,
}

#[godot_api]
impl ICharacterBody3D for EnemyDrone {
    fn init(base: Base<CharacterBody3D>) -> Self {
        Self {
            base,
            speed: 8.0,
            health: 30.0,
            detection_range: 25.0,
            attack_range: 5.0,
            damage: 5.0,
            state: DroneState::Idle,
        }
    }

    fn physics_process(&mut self, _delta: f64) {
        match self.state {
            DroneState::Idle => {
                // TODO: check distance to player, add Chasing/Attacking states
            }
            DroneState::Dead => {}
        }
    }
}

#[godot_api]
impl EnemyDrone {
    #[func]
    pub fn take_damage(&mut self, amount: f32) {
        self.health -= amount;
        if self.health <= 0.0 {
            self.state = DroneState::Dead;
            // TODO: emit signal, spawn loot, queue_free
        }
    }
}
