//! Pure-data enemy AI state machine. No Godot dependency.

use crate::newtypes::{Health, Damage};

/// Possible states for a drone enemy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DroneState {
    Idle,
    Chasing,
    Attacking,
    Dead,
}

/// Configuration for drone behavior thresholds.
#[derive(Debug, Clone)]
pub struct DroneConfig {
    pub detection_range: f32,
    pub attack_range: f32,
    pub disengage_range: f32,
    pub health: Health,
    pub attack_cooldown: f32,
}

impl Default for DroneConfig {
    fn default() -> Self {
        Self {
            detection_range: 25.0,
            attack_range: 5.0,
            disengage_range: 30.0, // detection_range * 1.2
            health: Health::new(3.0),
            attack_cooldown: 1.0,
        }
    }
}

/// Pure state machine for drone AI.
#[derive(Debug, Clone)]
pub struct DroneAi {
    pub state: DroneState,
    pub health: Health,
    pub attack_timer: f32,
    pub config: DroneConfig,
}

impl DroneAi {
    pub fn new(config: DroneConfig) -> Self {
        let health = config.health;
        Self {
            state: DroneState::Idle,
            health,
            attack_timer: 0.0,
            config,
        }
    }

    /// Update state based on distance to player. Returns whether the drone should fire.
    pub fn update(&mut self, distance_to_player: f32, delta: f32) -> bool {
        if self.state == DroneState::Dead {
            return false;
        }

        self.attack_timer = (self.attack_timer - delta).max(0.0);

        // State transitions
        match self.state {
            DroneState::Idle => {
                if distance_to_player <= self.config.detection_range {
                    self.state = DroneState::Chasing;
                }
            }
            DroneState::Chasing => {
                if distance_to_player <= self.config.attack_range {
                    self.state = DroneState::Attacking;
                    self.attack_timer = 0.0;
                } else if distance_to_player > self.config.disengage_range {
                    self.state = DroneState::Idle;
                }
            }
            DroneState::Attacking => {
                if distance_to_player > self.config.attack_range {
                    self.state = DroneState::Chasing;
                }
            }
            DroneState::Dead => {}
        }

        // Fire check (only in Attacking state)
        if self.state == DroneState::Attacking && self.attack_timer <= 0.0 {
            self.attack_timer = self.config.attack_cooldown;
            return true;
        }

        false
    }

    /// Apply damage. Returns true if the drone just died.
    pub fn take_damage(&mut self, amount: Damage) -> bool {
        if self.state == DroneState::Dead {
            return false;
        }
        self.health = self.health.take(amount);
        if !self.health.is_alive() {
            self.state = DroneState::Dead;
            true
        } else {
            false
        }
    }

    pub fn is_dead(&self) -> bool {
        self.state == DroneState::Dead
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_ai() -> DroneAi {
        DroneAi::new(DroneConfig::default())
    }

    #[test]
    fn starts_idle() {
        let ai = default_ai();
        assert_eq!(ai.state, DroneState::Idle);
    }

    #[test]
    fn stays_idle_when_player_far() {
        let mut ai = default_ai();
        ai.update(50.0, 0.016);
        assert_eq!(ai.state, DroneState::Idle);
    }

    #[test]
    fn transitions_to_chasing_when_player_in_range() {
        let mut ai = default_ai();
        ai.update(20.0, 0.016);
        assert_eq!(ai.state, DroneState::Chasing);
    }

    #[test]
    fn transitions_to_attacking_when_close() {
        let mut ai = default_ai();
        ai.update(20.0, 0.016); // → Chasing
        ai.update(4.0, 0.016);  // → Attacking
        assert_eq!(ai.state, DroneState::Attacking);
    }

    #[test]
    fn fires_on_entering_attack_range() {
        let mut ai = default_ai();
        ai.update(20.0, 0.016); // → Chasing
        let should_fire = ai.update(4.0, 0.016); // → Attacking, fire immediately
        assert!(should_fire);
    }

    #[test]
    fn does_not_fire_during_cooldown() {
        let mut ai = default_ai();
        ai.update(20.0, 0.016);
        ai.update(4.0, 0.016); // Fires, cooldown starts
        let should_fire = ai.update(4.0, 0.5); // 0.5s into 1.0s cooldown
        assert!(!should_fire);
    }

    #[test]
    fn fires_again_after_cooldown() {
        let mut ai = default_ai();
        ai.update(20.0, 0.016);
        ai.update(4.0, 0.016); // First fire
        ai.update(4.0, 0.5);   // Still cooling
        let should_fire = ai.update(4.0, 0.6); // Cooldown expired
        assert!(should_fire);
    }

    #[test]
    fn returns_to_chasing_when_player_leaves_attack_range() {
        let mut ai = default_ai();
        ai.update(20.0, 0.016); // → Chasing
        ai.update(4.0, 0.016);  // → Attacking
        ai.update(10.0, 0.016); // → Chasing (out of attack range)
        assert_eq!(ai.state, DroneState::Chasing);
    }

    #[test]
    fn returns_to_idle_when_player_disengages() {
        let mut ai = default_ai();
        ai.update(20.0, 0.016); // → Chasing
        ai.update(35.0, 0.016); // Beyond disengage_range (30)
        assert_eq!(ai.state, DroneState::Idle);
    }

    #[test]
    fn hysteresis_prevents_flicker_at_detection_boundary() {
        let mut ai = default_ai();
        ai.update(20.0, 0.016); // → Chasing
        // Just outside detection_range but inside disengage_range
        ai.update(27.0, 0.016);
        assert_eq!(ai.state, DroneState::Chasing); // Still chasing
    }

    #[test]
    fn damage_reduces_health() {
        let mut ai = default_ai();
        ai.take_damage(Damage::new(1.0));
        assert_eq!(ai.health, Health::new(2.0));
        assert_eq!(ai.state, DroneState::Idle); // Not dead yet
    }

    #[test]
    fn lethal_damage_kills() {
        let mut ai = default_ai();
        let died = ai.take_damage(Damage::new(3.0));
        assert!(died);
        assert!(ai.is_dead());
        assert_eq!(ai.state, DroneState::Dead);
    }

    #[test]
    fn overkill_damage_kills() {
        let mut ai = default_ai();
        let died = ai.take_damage(Damage::new(99.0));
        assert!(died);
        assert!(ai.is_dead());
    }

    #[test]
    fn dead_drone_does_not_update() {
        let mut ai = default_ai();
        ai.take_damage(Damage::new(30.0));
        let should_fire = ai.update(1.0, 0.016);
        assert!(!should_fire);
        assert_eq!(ai.state, DroneState::Dead);
    }

    #[test]
    fn dead_drone_ignores_further_damage() {
        let mut ai = default_ai();
        ai.take_damage(Damage::new(3.0));
        let died_again = ai.take_damage(Damage::new(1.0));
        assert!(!died_again);
    }

    #[test]
    fn exact_boundary_detection_triggers_chase() {
        let mut ai = default_ai();
        ai.update(25.0, 0.016); // Exactly at detection_range
        assert_eq!(ai.state, DroneState::Chasing);
    }

    #[test]
    fn dead_state_stable_across_repeated_updates() {
        let mut ai = default_ai();
        ai.take_damage(Damage::new(99.0));
        assert!(ai.is_dead());
        for _ in 0..100 {
            assert!(ai.is_dead());
            assert!(!ai.update(1.0, 0.016));
        }
    }

    #[test]
    fn exact_boundary_attack_triggers_attack() {
        let mut ai = default_ai();
        ai.update(20.0, 0.016); // → Chasing
        ai.update(5.0, 0.016);  // Exactly at attack_range
        assert_eq!(ai.state, DroneState::Attacking);
    }
}
