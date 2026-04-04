use crate::newtypes::{Health, Damage};
use crate::upgrade::{Upgrade, UpgradeKind};

/// Base stats for the ship before upgrades.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseStats {
    pub thrust_power: f32,
    pub rotation_speed: f32,
    pub damping: f32,
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
            damping: 0.95,
            max_health: Health::new(100.0),
            fire_rate: 5.0,
            projectile_speed: 50.0,
            projectile_damage: Damage::new(1.0),
        }
    }
}

/// The player's current ship configuration: base stats + collected upgrades.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Loadout {
    pub base: BaseStats,
    pub upgrades: Vec<Upgrade>,
}

impl Loadout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_upgrade(&mut self, upgrade: Upgrade) {
        self.upgrades.push(upgrade);
    }

    /// Compute effective stat by applying all relevant upgrade multipliers.
    fn effective(&self, kind: UpgradeKind, base_value: f32) -> f32 {
        self.upgrades
            .iter()
            .filter(|u| u.kind == kind)
            .fold(base_value, |val, u| val * u.multiplier)
    }

    pub fn thrust_power(&self) -> f32 {
        self.effective(UpgradeKind::Thrust, self.base.thrust_power)
    }

    pub fn rotation_speed(&self) -> f32 {
        self.effective(UpgradeKind::RotationSpeed, self.base.rotation_speed)
    }

    pub fn damping(&self) -> f32 {
        self.effective(UpgradeKind::Damping, self.base.damping)
    }

    pub fn max_health(&self) -> Health {
        Health::new(self.effective(UpgradeKind::MaxHealth, self.base.max_health.as_f32()))
    }

    pub fn fire_rate(&self) -> f32 {
        self.effective(UpgradeKind::FireRate, self.base.fire_rate)
    }

    pub fn projectile_speed(&self) -> f32 {
        self.effective(UpgradeKind::ProjectileSpeed, self.base.projectile_speed)
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
    fn single_upgrade_applies() {
        let mut loadout = Loadout::new();
        loadout.add_upgrade(Upgrade {
            name: "Test Booster".to_string(),
            kind: UpgradeKind::Thrust,
            multiplier: 1.5,
        });
        assert_eq!(loadout.thrust_power(), 60.0);
        // Other stats unaffected
        assert_eq!(loadout.rotation_speed(), 6.0);
    }

    #[test]
    fn multiple_upgrades_stack_multiplicatively() {
        let mut loadout = Loadout::new();
        loadout.add_upgrade(Upgrade {
            name: "Boost A".to_string(),
            kind: UpgradeKind::Thrust,
            multiplier: 1.5,
        });
        loadout.add_upgrade(Upgrade {
            name: "Boost B".to_string(),
            kind: UpgradeKind::Thrust,
            multiplier: 2.0,
        });
        assert_eq!(loadout.thrust_power(), 120.0); // 40 * 1.5 * 2.0
    }
}
