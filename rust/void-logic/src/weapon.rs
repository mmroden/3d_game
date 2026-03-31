//! Hitscan laser weapon state and logic.
//! Pure data — no Godot dependency, fully testable.

/// Result of attempting to fire the weapon.
#[derive(Debug, Clone, PartialEq)]
pub enum FireResult {
    /// Weapon fired successfully. Contains damage dealt.
    Fired { damage: f32 },
    /// Weapon still on cooldown.
    OnCooldown,
}

/// Tracks weapon state: cooldown timer, fire rate, damage.
#[derive(Debug, Clone)]
pub struct WeaponState {
    pub fire_rate: f32,
    pub damage: f32,
    pub max_range: f32,
    cooldown: f32,
}

impl WeaponState {
    pub fn new(fire_rate: f32, damage: f32, max_range: f32) -> Self {
        Self {
            fire_rate,
            damage,
            max_range,
            cooldown: 0.0,
        }
    }

    /// Advance cooldown by delta seconds.
    pub fn tick(&mut self, delta: f32) {
        self.cooldown = (self.cooldown - delta).max(0.0);
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
        Self::new(5.0, 1.0, 100.0)
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
        assert_eq!(result, FireResult::Fired { damage: 1.0 });
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
        let mut weapon = WeaponState::new(5.0, 1.0, 100.0);
        weapon.try_fire();
        // Cooldown = 1/5 = 0.2s
        weapon.tick(0.1);
        assert!(!weapon.is_ready());
        weapon.tick(0.1);
        assert!(weapon.is_ready());
    }

    #[test]
    fn can_fire_again_after_cooldown() {
        let mut weapon = WeaponState::new(5.0, 1.0, 100.0);
        weapon.try_fire();
        weapon.tick(0.2);
        let result = weapon.try_fire();
        assert_eq!(result, FireResult::Fired { damage: 1.0 });
    }

    #[test]
    fn fire_rate_affects_cooldown_duration() {
        let mut weapon = WeaponState::new(2.0, 1.0, 100.0);
        weapon.try_fire();
        // Cooldown = 1/2 = 0.5s
        weapon.tick(0.4);
        assert!(!weapon.is_ready());
        weapon.tick(0.1);
        assert!(weapon.is_ready());
    }

    #[test]
    fn damage_value_is_returned_on_fire() {
        let mut weapon = WeaponState::new(5.0, 25.0, 100.0);
        let result = weapon.try_fire();
        assert_eq!(result, FireResult::Fired { damage: 25.0 });
    }

    #[test]
    fn cooldown_does_not_go_negative() {
        let mut weapon = WeaponState::default();
        weapon.tick(10.0); // Way more than needed
        assert!(weapon.is_ready());
        assert_eq!(weapon.cooldown, 0.0);
    }
}
