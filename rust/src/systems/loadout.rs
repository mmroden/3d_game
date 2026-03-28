use crate::systems::upgrade::{Upgrade, UpgradeKind};

/// Base stats for the ship before upgrades.
#[derive(Debug, Clone)]
pub struct BaseStats {
    pub thrust_power: f32,
    pub rotation_speed: f32,
    pub damping: f32,
    pub max_health: f32,
    pub fire_rate: f32,
    pub projectile_speed: f32,
    pub projectile_damage: f32,
}

impl Default for BaseStats {
    fn default() -> Self {
        Self {
            thrust_power: 20.0,
            rotation_speed: 2.5,
            damping: 0.95,
            max_health: 100.0,
            fire_rate: 5.0,
            projectile_speed: 50.0,
            projectile_damage: 10.0,
        }
    }
}

/// The player's current ship configuration: base stats + collected upgrades.
#[derive(Debug, Clone, Default)]
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

    pub fn max_health(&self) -> f32 {
        self.effective(UpgradeKind::MaxHealth, self.base.max_health)
    }

    pub fn fire_rate(&self) -> f32 {
        self.effective(UpgradeKind::FireRate, self.base.fire_rate)
    }

    pub fn projectile_speed(&self) -> f32 {
        self.effective(UpgradeKind::ProjectileSpeed, self.base.projectile_speed)
    }

    pub fn projectile_damage(&self) -> f32 {
        self.effective(UpgradeKind::ProjectileDamage, self.base.projectile_damage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_stats_unchanged_without_upgrades() {
        let loadout = Loadout::new();
        assert_eq!(loadout.thrust_power(), 20.0);
        assert_eq!(loadout.rotation_speed(), 2.5);
    }

    #[test]
    fn single_upgrade_applies() {
        let mut loadout = Loadout::new();
        loadout.add_upgrade(Upgrade {
            name: "Test Booster".to_string(),
            kind: UpgradeKind::Thrust,
            multiplier: 1.5,
        });
        assert_eq!(loadout.thrust_power(), 30.0);
        // Other stats unaffected
        assert_eq!(loadout.rotation_speed(), 2.5);
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
        assert_eq!(loadout.thrust_power(), 60.0); // 20 * 1.5 * 2.0
    }
}
