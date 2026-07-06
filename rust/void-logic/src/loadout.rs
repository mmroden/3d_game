use crate::kinetics::Retention;
use crate::newtypes::{Health, Damage};
use crate::upgrade::{UpgradeKind, STAT_UPGRADE_MULTIPLIER};
use serde::{Deserialize, Serialize};

/// Base stats for the ship before upgrades.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaseStats {
    pub thrust_power: f32,
    pub rotation_speed: f32,
    pub damping: Retention,
    pub max_health: Health,
    pub fire_rate: f32,
    pub projectile_speed: f32,
    pub projectile_damage: Damage,
}

impl Default for BaseStats {
    fn default() -> Self {
        Self {
            thrust_power: 40.0,
            rotation_speed: 6.0,
            // Per-second retention; 0.046 ≡ the original 0.95-per-tick
            // feel at the historical 60 Hz tick (0.95^60).
            damping: Retention::decaying(0.046),
            max_health: Health::new(100.0),
            fire_rate: 2.0,
            projectile_speed: 50.0,
            projectile_damage: Damage::new(1.0),
        }
    }
}

/// The player's current ship configuration: base stats + collected upgrades.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Loadout {
    pub base: BaseStats,
    pub upgrades: Vec<UpgradeKind>,
}

impl Loadout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_upgrade(&mut self, kind: UpgradeKind) {
        self.upgrades.push(kind);
    }

    /// The total multiplier the loadout applies to `kind` (1.0 = stock).
    pub fn stat_multiplier(&self, kind: UpgradeKind) -> f32 {
        self.effective(kind, 1.0)
    }

    /// Compute effective stat: the fixed policy step, compounded once per
    /// purchase of `kind` (the loadout stores WHICH, policy stores HOW MUCH).
    fn effective(&self, kind: UpgradeKind, base_value: f32) -> f32 {
        let count = self.upgrades.iter().filter(|k| **k == kind).count();
        base_value * STAT_UPGRADE_MULTIPLIER.powi(count as i32)
    }

    pub fn thrust_power(&self) -> f32 {
        self.effective(UpgradeKind::Thrust, self.base.thrust_power)
    }

    pub fn rotation_speed(&self) -> f32 {
        self.effective(UpgradeKind::RotationSpeed, self.base.rotation_speed)
    }

    pub fn damping(&self) -> Retention {
        // Fixed base value: the "Stability" upgrade was retired with the
        // free-drop economy. Retention's own invariant (< 1.0 unless FULL)
        // keeps the infinite-spin bug unrepresentable.
        self.base.damping
    }

    pub fn max_health(&self) -> Health {
        Health::new(self.effective(UpgradeKind::MaxHealth, self.base.max_health.as_f32()))
    }

    pub fn fire_rate(&self) -> f32 {
        self.effective(UpgradeKind::FireRate, self.base.fire_rate)
    }

    pub fn projectile_speed(&self) -> f32 {
        // Fixed base value: the "Beam Focus" upgrade was retired with the
        // free-drop economy.
        self.base.projectile_speed
    }

    pub fn projectile_damage(&self) -> Damage {
        Damage::new(self.effective(UpgradeKind::ProjectileDamage, self.base.projectile_damage.as_f32()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_stats_unchanged_without_upgrades() {
        let loadout = Loadout::new();
        assert_eq!(loadout.thrust_power(), 40.0);
        assert_eq!(loadout.rotation_speed(), 6.0);
    }

    #[test]
    fn single_upgrade_applies_the_policy_step() {
        let mut loadout = Loadout::new();
        loadout.add_upgrade(UpgradeKind::Thrust);
        assert_eq!(loadout.thrust_power(), 40.0 * STAT_UPGRADE_MULTIPLIER);
        // Other stats unaffected
        assert_eq!(loadout.rotation_speed(), 6.0);
    }

    #[test]
    fn damping_and_projectile_speed_are_fixed_base_values() {
        // These stats have no upgrade kind (retired with the free-drop
        // economy): whatever the loadout collects, they stay at base.
        let mut loadout = Loadout::new();
        loadout.add_upgrade(UpgradeKind::Thrust);
        assert_eq!(loadout.damping().factor(), Loadout::new().damping().factor());
        assert_eq!(loadout.projectile_speed(), Loadout::new().projectile_speed());
    }

    #[test]
    fn multiple_upgrades_stack_multiplicatively() {
        let mut loadout = Loadout::new();
        loadout.add_upgrade(UpgradeKind::Thrust);
        loadout.add_upgrade(UpgradeKind::Thrust);
        assert_eq!(
            loadout.thrust_power(),
            40.0 * STAT_UPGRADE_MULTIPLIER * STAT_UPGRADE_MULTIPLIER,
            "each purchase compounds the ONE policy step"
        );
    }
}
