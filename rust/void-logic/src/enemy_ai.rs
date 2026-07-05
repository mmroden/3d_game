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

/// Desired strafe velocity for a kiting drone, in world units/sec. `tangent`
/// drives the orbit (applied along the perpendicular-to-player unit vector);
/// `radial` is signed and applied along the toward-player unit vector —
/// positive pulls inward when the drone has drifted past `standoff`, negative
/// pushes it back out when the player crowds it. All orbit geometry lives here;
/// the node only multiplies these scalars by its two unit vectors and adds them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrafeVelocity {
    pub tangent: f32,
    pub radial: f32,
}

/// Resolve a `Movement::Strafe` intent into concrete velocity components for a
/// drone cruising at `speed`, currently `distance` from the player, that wants
/// to orbit at `standoff`. `speed_mul` scales the tangential orbit speed. The
/// radial pull is capped at the drone's cruise speed so a far-off kiter doesn't
/// lunge, then halved so the orbit dominates the correction.
pub fn strafe_velocity(speed_mul: f32, distance: f32, standoff: f32, speed: f32) -> StrafeVelocity {
    let tangent = speed * speed_mul;
    let radius_error = distance - standoff;
    let radial = radius_error.clamp(-speed, speed) * 0.5;
    StrafeVelocity { tangent, radial }
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

/// Desired-vs-actual speed ratio below which a strafing drone counts as
/// blocked (a wall is eating its orbit).
pub const BLOCKED_RATIO: f32 = 0.35;
/// Consecutive blocked ticks before the orbit flips direction.
pub const BLOCKED_FLIP_TICKS: u32 = 24;
/// Ticks after a flip during which a second blockage means "flipping didn't
/// help — back out and reapproach" instead of flip-flopping in the corner.
pub const FLIP_MEMORY_TICKS: u32 = 240;
/// Ticks of forced retreat when both orbit directions are blocked.
pub const RETREAT_TICKS: u32 = 60;
/// Ticks of sidestep (strafe) when a straight chase is blocked — long enough
/// to clear a panel-relief pocket (~half a cubic cell), short enough that the
/// pursuit reads as a dodge, not a wander.
pub const EVADE_TICKS: u32 = 30;

/// State machine for enemy AI.
#[derive(Debug, Clone)]
pub struct DroneAi {
    pub state: DroneState,
    pub health: Health,
    pub shield: Option<Shield>,
    pub attack_timer: f32,
    pub fuse_timer: f32,
    pub config: DroneConfig,
    /// Movement feedback from the shell (actual/desired speed, last tick).
    /// 1.0 = moving freely; near 0 = something is eating the motion.
    feedback_ratio: f32,
    /// Consecutive strafe ticks spent blocked.
    blocked_ticks: u32,
    /// Orbit handedness (±1); the node multiplies its perpendicular by this.
    orbit_sign: f32,
    /// Ticks since the last orbit flip (saturating).
    ticks_since_flip: u32,
    /// Forced-retreat ticks remaining after both directions blocked.
    retreat_ticks: u32,
    /// Sidestep ticks remaining after a blocked straight chase.
    evade_ticks: u32,
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
            feedback_ratio: 1.0,
            blocked_ticks: 0,
            orbit_sign: 1.0,
            ticks_since_flip: u32::MAX,
            retreat_ticks: 0,
            evade_ticks: 0,
        }
    }

    /// Shell feedback: how much of last tick's desired speed was actually
    /// achieved (`|linear_velocity| / |desired|`, clamped by the caller).
    /// The AI never learns about walls any other way — this is the loop
    /// that stops orbiting drones parking in corners (playtest 2026-07-04).
    pub fn report_movement_feedback(&mut self, actual_speed_ratio: f32) {
        self.feedback_ratio = if actual_speed_ratio.is_finite() {
            actual_speed_ratio.clamp(0.0, 2.0)
        } else {
            1.0
        };
    }

    /// Orbit handedness for `Movement::Strafe` — the node multiplies its
    /// perpendicular unit vector by this. Flips when the orbit is blocked.
    pub fn orbit_sign(&self) -> f32 {
        self.orbit_sign
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

    /// One straight-chase tick under the wall-feedback loop (playtest
    /// 2026-07-05: a 6DOF chase pressed drones into panel-relief pockets and
    /// Jolt held them there). Sustained blockage sidesteps (strafe) out of
    /// the pocket; a blocked sidestep rides the shared orbit loop — flip
    /// handedness, then back out entirely. Nothing parks.
    fn chase_movement(&mut self, speed_mul: f32) -> Movement {
        if self.retreat_ticks > 0 {
            self.retreat_ticks -= 1;
            return Movement::Retreat { speed_mul: 0.8 };
        }
        if self.evade_ticks > 0 {
            self.evade_ticks -= 1;
            self.tick_orbit_blockage();
            return Movement::Strafe { speed_mul: 0.7 };
        }
        if self.feedback_ratio < BLOCKED_RATIO {
            self.blocked_ticks += 1;
        } else {
            self.blocked_ticks = 0;
        }
        if self.blocked_ticks >= BLOCKED_FLIP_TICKS {
            self.blocked_ticks = 0;
            self.evade_ticks = EVADE_TICKS;
            return Movement::Strafe { speed_mul: 0.7 };
        }
        Movement::Chase { speed_mul }
    }

    fn shooter_tick(&mut self, has_line_of_sight: bool) -> AiTick {
        let attack = self.try_fire(has_line_of_sight);
        let movement = match self.state {
            DroneState::Chasing => self.chase_movement(1.0),
            // In range and firing: HOLD. The old slow creep walked shooters
            // into the player's face, where rays starting inside their hull
            // can't hit them (playtest 2026-07-04). The Chasing/Attacking
            // hysteresis re-closes if the player pulls away.
            DroneState::Attacking => Movement::Hold,
            DroneState::Idle | DroneState::Dead => Movement::Hold,
        };
        AiTick { movement, attack }
    }

    fn kiter_tick(&mut self, distance: f32, has_line_of_sight: bool) -> AiTick {
        let attack = self.try_fire(has_line_of_sight);
        let movement = match self.state {
            DroneState::Idle | DroneState::Dead => Movement::Hold,
            DroneState::Chasing => self.chase_movement(1.0),
            DroneState::Attacking => {
                if self.retreat_ticks > 0 {
                    // Both orbit directions were blocked: back out, then
                    // reapproach — a maneuver, never a parking spot.
                    self.retreat_ticks -= 1;
                    Movement::Retreat { speed_mul: 0.8 }
                } else if distance < self.config.standoff_range {
                    Movement::Retreat { speed_mul: 1.0 }
                } else {
                    self.tick_orbit_blockage();
                    Movement::Strafe { speed_mul: 0.7 }
                }
            }
        };
        AiTick { movement, attack }
    }

    /// The wall-feedback loop for orbiting drones: sustained low
    /// actual-vs-desired speed flips the orbit direction; a second blockage
    /// soon after the flip means the corner has both directions covered —
    /// back out for [`RETREAT_TICKS`] and come in fresh.
    fn tick_orbit_blockage(&mut self) {
        self.ticks_since_flip = self.ticks_since_flip.saturating_add(1);
        if self.feedback_ratio < BLOCKED_RATIO {
            self.blocked_ticks += 1;
        } else {
            self.blocked_ticks = 0;
        }
        if self.blocked_ticks >= BLOCKED_FLIP_TICKS {
            self.blocked_ticks = 0;
            if self.ticks_since_flip <= FLIP_MEMORY_TICKS {
                self.retreat_ticks = RETREAT_TICKS;
            } else {
                self.orbit_sign = -self.orbit_sign;
                self.ticks_since_flip = 0;
            }
        }
    }

    fn swarmer_tick(&mut self) -> AiTick {
        let movement = match self.state {
            DroneState::Idle | DroneState::Dead => Movement::Hold,
            DroneState::Chasing => self.chase_movement(1.0),
            // Latched: the press against the player IS the attack — zero
            // speed here is success, never a wall verdict.
            DroneState::Attacking => Movement::Chase { speed_mul: 1.0 },
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
                AiTick { movement: self.chase_movement(1.0), attack: Attack::None }
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
    fn shooter_chases_then_holds_at_attack_range() {
        // "Chase TO attack range and fire" — the old slow-creep in Attacking
        // walked shooters into the player's face, where rays that start
        // inside their hull can't hit them (playtest 2026-07-04: hugging
        // enemies were unkillable).
        let mut ai = default_ai();
        let chasing = ai.update(20.0, true, 0.016);
        assert_eq!(chasing.movement, Movement::Chase { speed_mul: 1.0 });
        let attacking = ai.update(4.0, true, 0.016);
        assert_eq!(attacking.movement, Movement::Hold,
            "in range and firing: hold, never creep into the target");
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

    // --- Kiter orbit geometry: strafe_velocity (owned by void-logic) ---

    #[test]
    fn strafe_tangent_scales_with_speed_and_mul() {
        // At the standoff radius there is no radius error, so all motion is
        // tangential: speed (10) * speed_mul (0.7).
        let v = strafe_velocity(0.7, 5.0, 5.0, 10.0);
        assert_eq!(v.tangent, 7.0);
        assert_eq!(v.radial, 0.0);
    }

    #[test]
    fn strafe_pulls_inward_when_beyond_standoff() {
        // distance 8, standoff 5 → radius_error +3, within ±speed → +3 * 0.5.
        // Positive radial is applied along the toward-player vector: close in.
        let v = strafe_velocity(0.7, 8.0, 5.0, 10.0);
        assert_eq!(v.radial, 1.5);
    }

    #[test]
    fn strafe_pushes_outward_when_inside_standoff() {
        // distance 3, standoff 5 → radius_error −2 → −1.0: back away from player.
        let v = strafe_velocity(0.7, 3.0, 5.0, 10.0);
        assert_eq!(v.radial, -1.0);
    }

    #[test]
    fn strafe_radial_is_clamped_to_cruise_speed() {
        // A far-off kiter shouldn't lunge: radius_error 100 clamps to speed (4),
        // then halves → 2.0, never the full 50.
        let v = strafe_velocity(1.0, 104.0, 4.0, 4.0);
        assert_eq!(v.radial, 2.0);
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

    // --- Movement feedback: blocked orbits flip, then back out ---
    // (Playtest 2026-07-04: strafing drones wedged into corners forever —
    // the AI computed orbit intent but never learned the wall was eating it.)

    /// A kiter parked in its strafe band (attacking, outside standoff).
    fn strafing_kiter() -> DroneAi {
        let mut ai = DroneAi::new(DroneConfig {
            archetype: Archetype::Kiter,
            attack_range: 10.0,
            standoff_range: 6.0,
            ..DroneConfig::default()
        });
        ai.update(20.0, true, 0.016); // → Chasing
        let tick = ai.update(8.0, true, 0.016); // → Attacking, outside standoff
        assert_eq!(tick.movement, Movement::Strafe { speed_mul: 0.7 },
            "fixture sanity: the kiter must be strafing");
        ai
    }

    #[test]
    fn a_blocked_orbit_flips_direction() {
        let mut ai = strafing_kiter();
        assert_eq!(ai.orbit_sign(), 1.0);
        for _ in 0..BLOCKED_FLIP_TICKS {
            ai.report_movement_feedback(0.1);
            ai.update(8.0, true, 0.016);
        }
        assert_eq!(ai.orbit_sign(), -1.0,
            "a wall eating the orbit for {BLOCKED_FLIP_TICKS} ticks flips the direction");
    }

    #[test]
    fn noisy_but_moving_orbits_never_flip() {
        let mut ai = strafing_kiter();
        for i in 0..200 {
            // Alternate blocked/free — a graze, not a wedge.
            ai.report_movement_feedback(if i % 2 == 0 { 0.1 } else { 1.0 });
            ai.update(8.0, true, 0.016);
        }
        assert_eq!(ai.orbit_sign(), 1.0, "intermittent contact must not flip the orbit");
    }

    #[test]
    fn both_directions_blocked_backs_out_and_reapproaches() {
        let mut ai = strafing_kiter();
        // First wedge: flip.
        for _ in 0..BLOCKED_FLIP_TICKS {
            ai.report_movement_feedback(0.1);
            ai.update(8.0, true, 0.016);
        }
        assert_eq!(ai.orbit_sign(), -1.0);
        // Still wedged after flipping: the drone must back out, not park.
        let mut retreated = false;
        for _ in 0..(BLOCKED_FLIP_TICKS + 4) {
            ai.report_movement_feedback(0.1);
            let tick = ai.update(8.0, true, 0.016);
            if matches!(tick.movement, Movement::Retreat { .. }) {
                retreated = true;
                break;
            }
        }
        assert!(retreated, "blocked in both directions: back out and reapproach");

        // Once clear, the retreat expires and the strafe resumes.
        let mut strafed = false;
        for _ in 0..(RETREAT_TICKS + 8) {
            ai.report_movement_feedback(1.0);
            let tick = ai.update(8.0, true, 0.016);
            if matches!(tick.movement, Movement::Strafe { .. }) {
                strafed = true;
                break;
            }
        }
        assert!(strafed, "the retreat is a maneuver, not a new parking spot");
    }

    // --- Movement feedback: blocked chases sidestep, then back out ---
    // (Playtest 2026-07-05: a 6DOF chase pressed a drone into a panel-relief
    // pocket on planet 2 and Jolt held it there forever — the old contract
    // exempted Chasing from the feedback loop. Nothing parks. Ever.)

    #[test]
    fn a_blocked_chase_sidesteps_out_of_the_pocket() {
        let mut ai = default_ai();
        ai.update(20.0, true, 0.016); // → Chasing (Shooter fixture)
        let mut sidestepped = false;
        for _ in 0..(BLOCKED_FLIP_TICKS + 4) {
            ai.report_movement_feedback(0.1);
            let tick = ai.update(20.0, true, 0.016);
            if matches!(tick.movement, Movement::Strafe { .. }) {
                sidestepped = true;
                break;
            }
        }
        assert!(sidestepped,
            "a wall eating the chase for {BLOCKED_FLIP_TICKS} ticks must sidestep");

        // Once clear, the sidestep expires and the chase resumes.
        let mut chased = false;
        for _ in 0..(EVADE_TICKS + 8) {
            ai.report_movement_feedback(1.0);
            let tick = ai.update(20.0, true, 0.016);
            if matches!(tick.movement, Movement::Chase { .. }) {
                chased = true;
                break;
            }
        }
        assert!(chased, "the sidestep is a maneuver, not a new parking spot");
    }

    #[test]
    fn a_blocked_sidestep_backs_out_entirely() {
        let mut ai = default_ai();
        ai.update(20.0, true, 0.016); // → Chasing
        // Wedged no matter what it tries: chase blocked, sidestep blocked
        // both ways. The escape ladder must bottom out at Retreat.
        let mut retreated = false;
        for _ in 0..300 {
            ai.report_movement_feedback(0.1);
            let tick = ai.update(20.0, true, 0.016);
            if matches!(tick.movement, Movement::Retreat { .. }) {
                retreated = true;
                break;
            }
        }
        assert!(retreated, "blocked in every direction: back out and reapproach");
    }

    #[test]
    fn noisy_but_moving_chases_never_evade() {
        let mut ai = default_ai();
        ai.update(20.0, true, 0.016); // → Chasing
        for i in 0..200 {
            // Alternate blocked/free — a graze, not a wedge.
            ai.report_movement_feedback(if i % 2 == 0 { 0.1 } else { 1.0 });
            let tick = ai.update(20.0, true, 0.016);
            assert_eq!(tick.movement, Movement::Chase { speed_mul: 1.0 },
                "intermittent contact must not derail the chase");
        }
    }

    #[test]
    fn a_latched_swarmer_never_reads_as_blocked() {
        // A swarmer at latch range PRESSES into the player — zero speed is
        // its attack working, not a wall. It must never retreat off a latch.
        let mut ai = DroneAi::new(DroneConfig {
            archetype: Archetype::Swarmer,
            attack_range: 3.0,
            ..DroneConfig::default()
        });
        ai.update(20.0, true, 0.016); // → Chasing
        ai.update(2.0, true, 0.016); // → Attacking (latched)
        for _ in 0..200 {
            ai.report_movement_feedback(0.0);
            let tick = ai.update(2.0, true, 0.016);
            assert_eq!(tick.movement, Movement::Chase { speed_mul: 1.0 },
                "the latch press is the attack, never a blockage");
        }
    }

    #[test]
    fn a_charging_bomber_never_reads_as_blocked() {
        // A bomber riding its fuse into the player's hull is doing its job.
        let mut ai = DroneAi::new(DroneConfig {
            archetype: Archetype::Bomber,
            attack_range: 5.0,
            fuse_seconds: 100.0, // hold the fuse so the charge outlives the loop
            ..DroneConfig::default()
        });
        ai.update(20.0, true, 0.016); // → Chasing
        for _ in 0..200 {
            ai.report_movement_feedback(0.0);
            let tick = ai.update(3.0, true, 0.016); // in detonation range
            assert_eq!(tick.movement, Movement::Chase { speed_mul: 1.0 },
                "the fuse charge presses by design, never a blockage");
        }
    }
}
