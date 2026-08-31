//! Hitscan laser weapon state and logic.
//! Pure data — no Godot dependency, fully testable.

use crate::newtypes::Damage;

/// How long a trigger tap is remembered while the weapon cools down
/// (seconds). A press inside this window fires the instant the cooldown
/// expires — without it, taps landing mid-cooldown were simply eaten
/// ("shooting every other time I hit the trigger", owner playtest
/// 2026-08-20). Generous enough to bridge a human tap cadence against
/// the base cooldown, short enough that an abandoned press never fires
/// a surprise shot much later.
pub const TAP_BUFFER: f32 = 0.15;

/// Result of attempting to fire the weapon.
#[derive(Debug, Clone, PartialEq)]
pub enum FireResult {
    /// Weapon fired successfully. Contains damage dealt.
    Fired { damage: Damage },
    /// Weapon still on cooldown.
    OnCooldown,
}

/// Tracks weapon state: cooldown timer, fire rate, damage, and the
/// tap buffer.
#[derive(Debug, Clone)]
pub struct WeaponState {
    pub fire_rate: f32,
    pub damage: Damage,
    pub max_range: f32,
    cooldown: f32,
    buffered: f32,
}

impl WeaponState {
    pub fn new(fire_rate: f32, damage: Damage, max_range: f32) -> Self {
        Self {
            fire_rate,
            damage,
            max_range,
            cooldown: 0.0,
            buffered: 0.0,
        }
    }

    /// Record a trigger press. A press while ready is a no-op (the held
    /// fire path takes it this same tick); a press during cooldown arms
    /// the tap buffer so the shot leaves the moment the weapon is ready.
    pub fn press(&mut self) {
        if self.cooldown > 0.0 {
            self.buffered = TAP_BUFFER;
        }
    }

    /// Advance the clocks by `delta` seconds. Returns the buffered shot's
    /// damage when a remembered tap matures (cooldown just expired with
    /// the buffer alive) — the caller fires it exactly as if the trigger
    /// were down that tick.
    pub fn tick(&mut self, delta: f32) -> Option<Damage> {
        self.cooldown = (self.cooldown - delta).max(0.0);
        if self.buffered > 0.0 {
            self.buffered = (self.buffered - delta).max(0.0);
            if self.cooldown <= 0.0 && self.buffered > 0.0 {
                self.buffered = 0.0;
                self.cooldown = 1.0 / self.fire_rate;
                return Some(self.damage);
            }
        }
        None
    }

    /// Attempt to fire. Returns `Fired` with damage if ready, `OnCooldown` otherwise.
    pub fn try_fire(&mut self) -> FireResult {
        if self.cooldown > 0.0 {
            return FireResult::OnCooldown;
        }
        self.cooldown = 1.0 / self.fire_rate;
        FireResult::Fired { damage: self.damage }
    }

    pub fn is_ready(&self) -> bool {
        self.cooldown <= 0.0
    }
}

impl Default for WeaponState {
    fn default() -> Self {
        Self::new(2.0, Damage::new(1.0), 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_weapon_is_ready_to_fire() {
        let weapon = WeaponState::default();
        assert!(weapon.is_ready());
    }

    #[test]
    fn firing_puts_weapon_on_cooldown() {
        let mut weapon = WeaponState::default();
        let result = weapon.try_fire();
        assert_eq!(result, FireResult::Fired { damage: Damage::new(1.0) });
        assert!(!weapon.is_ready());
    }

    #[test]
    fn cannot_fire_during_cooldown() {
        let mut weapon = WeaponState::default();
        weapon.try_fire();
        let result = weapon.try_fire();
        assert_eq!(result, FireResult::OnCooldown);
    }

    #[test]
    fn cooldown_expires_after_sufficient_ticks() {
        let mut weapon = WeaponState::default();
        weapon.try_fire();
        // Cooldown = 1/2 = 0.5s
        weapon.tick(0.4);
        assert!(!weapon.is_ready());
        weapon.tick(0.1);
        assert!(weapon.is_ready());
    }

    #[test]
    fn can_fire_again_after_cooldown() {
        let mut weapon = WeaponState::default();
        weapon.try_fire();
        weapon.tick(0.5);
        let result = weapon.try_fire();
        assert_eq!(result, FireResult::Fired { damage: Damage::new(1.0) });
    }

    #[test]
    fn default_fire_rate_is_two() {
        let weapon = WeaponState::default();
        assert_eq!(weapon.fire_rate, 2.0);
    }

    #[test]
    fn fire_rate_affects_cooldown_duration() {
        let mut weapon = WeaponState::new(2.0, Damage::new(1.0), 100.0);
        weapon.try_fire();
        // Cooldown = 1/2 = 0.5s
        weapon.tick(0.4);
        assert!(!weapon.is_ready());
        weapon.tick(0.1);
        assert!(weapon.is_ready());
    }

    #[test]
    fn damage_value_is_returned_on_fire() {
        let mut weapon = WeaponState::new(5.0, Damage::new(25.0), 100.0);
        let result = weapon.try_fire();
        assert_eq!(result, FireResult::Fired { damage: Damage::new(25.0) });
    }

    #[test]
    fn cooldown_does_not_go_negative() {
        let mut weapon = WeaponState::default();
        assert_eq!(weapon.tick(10.0), None); // Way more than needed
        assert!(weapon.is_ready());
    }

    // --- the tap buffer ---

    #[test]
    fn tap_during_cooldown_fires_the_moment_the_weapon_is_ready() {
        let mut weapon = WeaponState::default();
        weapon.try_fire();
        weapon.tick(0.4); // 0.1s of cooldown left
        weapon.press(); // tap lands mid-cooldown — must not be eaten
        assert_eq!(weapon.tick(0.05), None, "still cooling");
        let shot = weapon.tick(0.06);
        assert_eq!(shot, Some(Damage::new(1.0)), "buffered tap fires on expiry");
        assert!(!weapon.is_ready(), "the buffered shot starts its own cooldown");
    }

    #[test]
    fn an_abandoned_tap_expires_with_the_buffer() {
        let mut weapon = WeaponState::default();
        weapon.try_fire();
        weapon.tick(0.1); // 0.4s of cooldown left — longer than the buffer
        weapon.press();
        for _ in 0..20 {
            assert_eq!(weapon.tick(0.05), None, "stale tap must never fire late");
        }
        assert!(weapon.is_ready());
    }

    #[test]
    fn press_while_ready_buffers_nothing() {
        // The held-fire path takes a ready press the same tick; buffering
        // it too would double-fire.
        let mut weapon = WeaponState::default();
        weapon.press();
        assert_eq!(weapon.tick(0.05), None);
        assert!(weapon.is_ready());
    }

    #[test]
    fn a_held_trigger_never_double_fires_around_the_buffer() {
        let mut weapon = WeaponState::default();
        weapon.try_fire(); // held: first shot
        weapon.press(); // same trigger, buffered against the fresh cooldown
        weapon.tick(0.3);
        let matured = weapon.tick(0.3); // cooldown expires, buffer long dead
        assert_eq!(matured, None, "buffer expired before the cooldown did");
        // The held path fires it instead — exactly one shot.
        assert_eq!(
            weapon.try_fire(),
            FireResult::Fired { damage: Damage::new(1.0) }
        );
    }
}
