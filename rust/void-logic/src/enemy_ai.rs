//! Pure-data enemy AI state machine. No Godot dependency.
//!
//! All enemies share one [`DroneState`] FSM (Idle → Chasing → Attacking → Dead)
//! but their *behaviour* — how they move and how they attack — is selected by an
//! [`Archetype`]. Each tick the logic returns an [`AiTick`] describing the
//! intent (move toward / away / strafe, and fire / ram / detonate); the node
//! layer turns that intent into forces and projectiles.

use crate::newtypes::{Damage, Health, Shield};

/// Possible states for an enemy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DroneState {
    Idle,
    Chasing,
    Attacking,
    Dead,
}

/// Behavioural archetype. Selects how an enemy moves and attacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Archetype {
    /// Chase to attack range and fire (the original behaviour).
    Shooter,
    /// Hold a stand-off range, retreat if the player closes, strafe, and fire.
    Kiter,
    /// No projectile: charge and ram for collision damage.
    Swarmer,
    /// Shooter with a damage-absorbing shield. Slow and durable.
    Tank,
    /// Charge to detonation range, burn a fuse, then AoE-detonate and die.
    Bomber,
}

/// How the enemy wants to move this tick. `speed_mul` scales its base speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Movement {
    /// Stay put.
    Hold,
    /// Move toward the player.
    Chase { speed_mul: f32 },
    /// Move away from the player.
    Retreat { speed_mul: f32 },
    /// Orbit the player (perpendicular movement; node picks handedness).
    Strafe { speed_mul: f32 },
}

/// What the enemy wants to do offensively this tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Attack {
    /// No attack this tick.
    None,
    /// Fire a projectile at the player.
    Fire,
    /// Deal contact (ram) damage — resolved by physics collision.
    Ram,
    /// Detonate, dealing area damage within `radius`. The enemy dies.
    Detonate { radius: f32 },
    /// Spawn `count` subsidiary drones. Reserved for the future boss.
    SpawnDrones { count: u8 },
}

/// The intent produced by [`DroneAi::update`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AiTick {
    pub movement: Movement,
    pub attack: Attack,
}

/// Configuration for enemy behaviour thresholds.
#[derive(Debug, Clone)]
pub struct DroneConfig {
    pub archetype: Archetype,
    pub detection_range: f32,
    pub attack_range: f32,
    pub disengage_range: f32,
    pub health: Health,
    pub attack_cooldown: f32,
    /// Kiter: preferred minimum distance; closer than this it retreats.
    pub standoff_range: f32,
    /// Bomber: seconds the fuse burns once in detonation range.
    pub fuse_seconds: f32,
    /// Bomber: area-damage radius on detonation.
    pub blast_radius: f32,
    /// Tank: optional shield that absorbs damage before health.
    pub shield: Option<Shield>,
}

impl Default for DroneConfig {
    fn default() -> Self {
        Self {
            archetype: Archetype::Shooter,
            detection_range: 25.0,
            attack_range: 5.0,
            disengage_range: 30.0, // detection_range * 1.2
            health: Health::new(3.0),
            attack_cooldown: 1.0,
            standoff_range: 0.0,
            fuse_seconds: 0.0,
            blast_radius: 0.0,
            shield: None,
        }
    }
}

/// State machine for enemy AI.
#[derive(Debug, Clone)]
pub struct DroneAi {
    pub state: DroneState,
    pub health: Health,
    pub shield: Option<Shield>,
    pub attack_timer: f32,
    pub fuse_timer: f32,
    pub config: DroneConfig,
}

impl DroneAi {
    pub fn new(config: DroneConfig) -> Self {
        let health = config.health;
        let shield = config.shield;
        Self {
            state: DroneState::Idle,
            health,
            shield,
            attack_timer: 0.0,
            fuse_timer: 0.0,
            config,
        }
    }

    /// Update state from the player's distance and visibility, returning the
    /// movement + attack intent for this tick.
    pub fn update(&mut self, distance_to_player: f32, has_line_of_sight: bool, delta: f32) -> AiTick {
        if self.state == DroneState::Dead {
            return AiTick { movement: Movement::Hold, attack: Attack::None };
        }

        self.attack_timer = (self.attack_timer - delta).max(0.0);

        self.advance_state(distance_to_player);

        match self.config.archetype {
            Archetype::Shooter | Archetype::Tank => self.shooter_tick(has_line_of_sight),
            Archetype::Kiter => self.kiter_tick(distance_to_player, has_line_of_sight),
            Archetype::Swarmer => self.swarmer_tick(),
            Archetype::Bomber => self.bomber_tick(delta),
        }
    }

    /// Shared Idle/Chasing/Attacking/Dead transitions with hysteresis.
    fn advance_state(&mut self, distance: f32) {
        match self.state {
            DroneState::Idle => {
                if distance <= self.config.detection_range {
                    self.state = DroneState::Chasing;
                }
            }
            DroneState::Chasing => {
                if distance <= self.config.attack_range {
                    self.state = DroneState::Attacking;
                    self.attack_timer = 0.0;
                } else if distance > self.config.disengage_range {
                    self.state = DroneState::Idle;
                }
            }
            DroneState::Attacking => {
                if distance > self.config.attack_range {
                    self.state = DroneState::Chasing;
                }
            }
            DroneState::Dead => {}
        }
    }

    /// Fire if attacking, off cooldown, and able to see the player. A blocked
    /// shot does not consume the cooldown; it fires when sight returns.
    fn try_fire(&mut self, has_line_of_sight: bool) -> Attack {
        if self.state == DroneState::Attacking && self.attack_timer <= 0.0 && has_line_of_sight {
            self.attack_timer = self.config.attack_cooldown;
            Attack::Fire
        } else {
            Attack::None
        }
    }

    fn shooter_tick(&mut self, has_line_of_sight: bool) -> AiTick {
        let attack = self.try_fire(has_line_of_sight);
        let movement = match self.state {
            DroneState::Chasing => Movement::Chase { speed_mul: 1.0 },
            // Close slowly while attacking so it does not overrun the player.
            DroneState::Attacking => Movement::Chase { speed_mul: 0.3 },
            DroneState::Idle | DroneState::Dead => Movement::Hold,
        };
        AiTick { movement, attack }
    }

    fn kiter_tick(&mut self, distance: f32, has_line_of_sight: bool) -> AiTick {
        let attack = self.try_fire(has_line_of_sight);
        let movement = match self.state {
            DroneState::Idle | DroneState::Dead => Movement::Hold,
            DroneState::Chasing => Movement::Chase { speed_mul: 1.0 },
            DroneState::Attacking => {
                if distance < self.config.standoff_range {
                    Movement::Retreat { speed_mul: 1.0 }
                } else {
                    Movement::Strafe { speed_mul: 0.7 }
                }
            }
        };
        AiTick { movement, attack }
    }

    fn swarmer_tick(&mut self) -> AiTick {
        let movement = match self.state {
            DroneState::Idle | DroneState::Dead => Movement::Hold,
            DroneState::Chasing | DroneState::Attacking => Movement::Chase { speed_mul: 1.0 },
        };
        let attack = if self.state == DroneState::Attacking {
            Attack::Ram
        } else {
            Attack::None
        };
        AiTick { movement, attack }
    }

    fn bomber_tick(&mut self, delta: f32) -> AiTick {
        match self.state {
            DroneState::Idle | DroneState::Dead => {
                AiTick { movement: Movement::Hold, attack: Attack::None }
            }
            DroneState::Chasing => {
                // Arm the fuse so it is full when detonation range is reached.
                self.fuse_timer = self.config.fuse_seconds;
                AiTick { movement: Movement::Chase { speed_mul: 1.0 }, attack: Attack::None }
            }
            DroneState::Attacking => {
                self.fuse_timer = (self.fuse_timer - delta).max(0.0);
                if self.fuse_timer <= 0.0 {
                    self.state = DroneState::Dead;
                    AiTick {
                        movement: Movement::Hold,
                        attack: Attack::Detonate { radius: self.config.blast_radius },
                    }
                } else {
                    // Keep charging while the fuse burns to guarantee contact.
                    AiTick { movement: Movement::Chase { speed_mul: 1.0 }, attack: Attack::None }
                }
            }
        }
    }

    /// Apply damage through the optional shield, then health. Returns true if
    /// the enemy just died.
    pub fn take_damage(&mut self, amount: Damage) -> bool {
        if self.state == DroneState::Dead {
            return false;
        }
        let overflow = match self.shield {
            Some(shield) => {
                let (remaining, overflow) = shield.absorb(amount);
                self.shield = Some(remaining);
                overflow
            }
            None => amount,
        };
        self.health = self.health.take(overflow);
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

    fn config_with(archetype: Archetype) -> DroneConfig {
        DroneConfig { archetype, ..DroneConfig::default() }
    }

    /// Drive an AI into the Attacking state at the given distance.
    fn engage(ai: &mut DroneAi, distance: f32) {
        ai.update(20.0, true, 0.016); // → Chasing
        ai.update(distance, true, 0.016); // → Attacking
    }

    // --- Shared FSM (Shooter is the regression anchor for original behaviour) ---

    #[test]
    fn starts_idle() {
        assert_eq!(default_ai().state, DroneState::Idle);
    }

    #[test]
    fn stays_idle_when_player_far() {
        let mut ai = default_ai();
        ai.update(50.0, true, 0.016);
        assert_eq!(ai.state, DroneState::Idle);
    }

    #[test]
    fn transitions_to_chasing_when_player_in_range() {
        let mut ai = default_ai();
        ai.update(20.0, true, 0.016);
        assert_eq!(ai.state, DroneState::Chasing);
    }

    #[test]
    fn transitions_to_attacking_when_close() {
        let mut ai = default_ai();
        engage(&mut ai, 4.0);
        assert_eq!(ai.state, DroneState::Attacking);
    }

    #[test]
    fn fires_on_entering_attack_range() {
        let mut ai = default_ai();
        ai.update(20.0, true, 0.016); // → Chasing
        let tick = ai.update(4.0, true, 0.016); // → Attacking, fire immediately
        assert_eq!(tick.attack, Attack::Fire);
    }

    #[test]
    fn does_not_fire_during_cooldown() {
        let mut ai = default_ai();
        engage(&mut ai, 4.0); // fires, cooldown starts
        let tick = ai.update(4.0, true, 0.5); // 0.5s into 1.0s cooldown
        assert_eq!(tick.attack, Attack::None);
    }

    #[test]
    fn fires_again_after_cooldown() {
        let mut ai = default_ai();
        engage(&mut ai, 4.0); // first fire
        ai.update(4.0, true, 0.5); // still cooling
        let tick = ai.update(4.0, true, 0.6); // cooldown expired
        assert_eq!(tick.attack, Attack::Fire);
    }

    #[test]
    fn returns_to_chasing_when_player_leaves_attack_range() {
        let mut ai = default_ai();
        engage(&mut ai, 4.0);
        ai.update(10.0, true, 0.016); // → Chasing (out of attack range)
        assert_eq!(ai.state, DroneState::Chasing);
    }

    #[test]
    fn returns_to_idle_when_player_disengages() {
        let mut ai = default_ai();
        ai.update(20.0, true, 0.016); // → Chasing
        ai.update(35.0, true, 0.016); // Beyond disengage_range (30)
        assert_eq!(ai.state, DroneState::Idle);
    }

    #[test]
    fn hysteresis_prevents_flicker_at_detection_boundary() {
        let mut ai = default_ai();
        ai.update(20.0, true, 0.016); // → Chasing
        ai.update(27.0, true, 0.016); // outside detection, inside disengage
        assert_eq!(ai.state, DroneState::Chasing);
    }

    #[test]
    fn exact_boundary_detection_triggers_chase() {
        let mut ai = default_ai();
        ai.update(25.0, true, 0.016); // exactly at detection_range
        assert_eq!(ai.state, DroneState::Chasing);
    }

    #[test]
    fn exact_boundary_attack_triggers_attack() {
        let mut ai = default_ai();
        ai.update(20.0, true, 0.016); // → Chasing
        ai.update(5.0, true, 0.016); // exactly at attack_range
        assert_eq!(ai.state, DroneState::Attacking);
    }

    #[test]
    fn shooter_chases_then_closes_slowly() {
        let mut ai = default_ai();
        let chasing = ai.update(20.0, true, 0.016);
        assert_eq!(chasing.movement, Movement::Chase { speed_mul: 1.0 });
        let attacking = ai.update(4.0, true, 0.016);
        assert_eq!(attacking.movement, Movement::Chase { speed_mul: 0.3 });
    }

    // --- Line of sight ---

    #[test]
    fn does_not_fire_without_line_of_sight() {
        let mut ai = default_ai();
        engage(&mut ai, 4.0); // fires, cooldown starts
        let tick = ai.update(4.0, false, 1.1); // cooldown expires but blind
        assert_eq!(tick.attack, Attack::None);
    }

    #[test]
    fn fires_immediately_when_sight_is_restored() {
        let mut ai = default_ai();
        engage(&mut ai, 4.0); // fires, cooldown starts
        ai.update(4.0, false, 1.1); // cooldown expires, sight blocked: held
        let tick = ai.update(4.0, true, 0.016);
        assert_eq!(tick.attack, Attack::Fire);
    }

    // --- Damage / death ---

    #[test]
    fn damage_reduces_health() {
        let mut ai = default_ai();
        ai.take_damage(Damage::new(1.0));
        assert_eq!(ai.health, Health::new(2.0));
        assert_eq!(ai.state, DroneState::Idle);
    }

    #[test]
    fn lethal_damage_kills() {
        let mut ai = default_ai();
        let died = ai.take_damage(Damage::new(3.0));
        assert!(died);
        assert!(ai.is_dead());
    }

    #[test]
    fn overkill_damage_kills() {
        let mut ai = default_ai();
        assert!(ai.take_damage(Damage::new(99.0)));
    }

    #[test]
    fn dead_drone_does_not_update() {
        let mut ai = default_ai();
        ai.take_damage(Damage::new(30.0));
        let tick = ai.update(1.0, true, 0.016);
        assert_eq!(tick.attack, Attack::None);
        assert_eq!(tick.movement, Movement::Hold);
        assert_eq!(ai.state, DroneState::Dead);
    }

    #[test]
    fn dead_drone_ignores_further_damage() {
        let mut ai = default_ai();
        ai.take_damage(Damage::new(3.0));
        assert!(!ai.take_damage(Damage::new(1.0)));
    }

    #[test]
    fn dead_state_stable_across_repeated_updates() {
        let mut ai = default_ai();
        ai.take_damage(Damage::new(99.0));
        for _ in 0..100 {
            assert!(ai.is_dead());
            let tick = ai.update(1.0, true, 0.016);
            assert_eq!(tick.attack, Attack::None);
        }
    }

    // --- Tank: shield absorbs before health ---

    #[test]
    fn tank_shield_absorbs_before_health() {
        let mut config = config_with(Archetype::Tank);
        config.health = Health::new(10.0);
        config.shield = Some(Shield::new(5.0));
        let mut ai = DroneAi::new(config);
        let died = ai.take_damage(Damage::new(4.0));
        assert!(!died);
        assert_eq!(ai.shield, Some(Shield::new(1.0)));
        assert_eq!(ai.health, Health::new(10.0)); // health untouched
    }

    #[test]
    fn tank_overflow_passes_to_health() {
        let mut config = config_with(Archetype::Tank);
        config.health = Health::new(10.0);
        config.shield = Some(Shield::new(5.0));
        let mut ai = DroneAi::new(config);
        ai.take_damage(Damage::new(8.0)); // 5 to shield, 3 to health
        assert_eq!(ai.shield, Some(Shield::new(0.0)));
        assert_eq!(ai.health, Health::new(7.0));
    }

    #[test]
    fn tank_still_fires_like_a_shooter() {
        let mut config = config_with(Archetype::Tank);
        config.shield = Some(Shield::new(5.0));
        let mut ai = DroneAi::new(config);
        ai.update(20.0, true, 0.016); // → Chasing
        let tick = ai.update(4.0, true, 0.016); // → Attacking
        assert_eq!(tick.attack, Attack::Fire);
    }

    // --- Kiter: stand off, strafe, retreat ---

    #[test]
    fn kiter_strafes_inside_attack_range() {
        let mut config = config_with(Archetype::Kiter);
        config.standoff_range = 3.0;
        let mut ai = DroneAi::new(config);
        engage(&mut ai, 4.0); // inside attack (5) but beyond standoff (3)
        let tick = ai.update(4.0, true, 0.016);
        assert_eq!(tick.movement, Movement::Strafe { speed_mul: 0.7 });
    }

    #[test]
    fn kiter_retreats_when_player_too_close() {
        let mut config = config_with(Archetype::Kiter);
        config.standoff_range = 3.0;
        let mut ai = DroneAi::new(config);
        engage(&mut ai, 2.0); // closer than standoff
        let tick = ai.update(2.0, true, 0.016);
        assert_eq!(tick.movement, Movement::Retreat { speed_mul: 1.0 });
    }

    #[test]
    fn kiter_fires_while_kiting() {
        let mut config = config_with(Archetype::Kiter);
        config.standoff_range = 3.0;
        let mut ai = DroneAi::new(config);
        ai.update(20.0, true, 0.016); // → Chasing
        let tick = ai.update(4.0, true, 0.016); // → Attacking
        assert_eq!(tick.attack, Attack::Fire);
    }

    #[test]
    fn kiter_chases_when_out_of_attack_range() {
        let mut ai = DroneAi::new(config_with(Archetype::Kiter));
        let tick = ai.update(20.0, true, 0.016);
        assert_eq!(tick.movement, Movement::Chase { speed_mul: 1.0 });
    }

    // --- Swarmer: rams, no projectile ---

    #[test]
    fn swarmer_rams_in_attack_range() {
        let mut ai = DroneAi::new(config_with(Archetype::Swarmer));
        engage(&mut ai, 4.0);
        let tick = ai.update(4.0, true, 0.016);
        assert_eq!(tick.attack, Attack::Ram);
        assert_eq!(tick.movement, Movement::Chase { speed_mul: 1.0 });
    }

    #[test]
    fn swarmer_never_fires() {
        let mut ai = DroneAi::new(config_with(Archetype::Swarmer));
        engage(&mut ai, 4.0);
        for _ in 0..10 {
            let tick = ai.update(4.0, true, 0.2);
            assert_ne!(tick.attack, Attack::Fire);
        }
    }

    // --- Bomber: fuse then detonate ---

    #[test]
    fn bomber_burns_fuse_before_detonating() {
        let mut config = config_with(Archetype::Bomber);
        config.fuse_seconds = 1.0;
        config.blast_radius = 6.0;
        let mut ai = DroneAi::new(config);
        engage(&mut ai, 4.0); // arms + enters Attacking
        let mid = ai.update(4.0, true, 0.5); // fuse partway
        assert_eq!(mid.attack, Attack::None);
        assert!(!ai.is_dead());
    }

    #[test]
    fn bomber_detonates_when_fuse_expires() {
        let mut config = config_with(Archetype::Bomber);
        config.fuse_seconds = 1.0;
        config.blast_radius = 6.0;
        let mut ai = DroneAi::new(config);
        engage(&mut ai, 4.0);
        ai.update(4.0, true, 0.6);
        let boom = ai.update(4.0, true, 0.6); // fuse exhausted
        assert_eq!(boom.attack, Attack::Detonate { radius: 6.0 });
        assert!(ai.is_dead());
    }

    #[test]
    fn bomber_resets_fuse_if_player_escapes() {
        let mut config = config_with(Archetype::Bomber);
        config.fuse_seconds = 1.0;
        let mut ai = DroneAi::new(config);
        engage(&mut ai, 4.0);
        ai.update(4.0, true, 0.9); // almost detonates
        ai.update(10.0, true, 0.016); // player escapes → Chasing, fuse re-armed
        let tick = ai.update(4.0, true, 0.5); // back in range, fuse full again
        assert_eq!(tick.attack, Attack::None);
        assert!(!ai.is_dead());
    }
}
