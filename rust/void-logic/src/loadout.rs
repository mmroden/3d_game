use crate::kinetics::Retention;
use crate::newtypes::{Health, Damage};
use crate::upgrade::{Upgrade, UpgradeKind};
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

    pub fn damping(&self) -> Retention {
        // Stability upgrades scale the decay *rate* — the exponent of
        // the per-second retention: retention' = base^multiplier.
        // Multipliers > 1 settle the ship faster, stacking approaches
        // but never reaches zero, and the result can never exceed the
        // base — the invariants hold structurally, no clamps needed.
        // Retention's own invariant (< 1.0 unless FULL) keeps the
        // infinite-spin bug unrepresentable.
        let exponent = self.effective(UpgradeKind::Damping, 1.0);
        Retention::decaying(self.base.damping.factor().powf(exponent))
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

    fn stability(multiplier: f32) -> Upgrade {
        Upgrade {
            name: format!("Stability +{}%", ((multiplier - 1.0) * 100.0).round()),
            kind: UpgradeKind::Damping,
            multiplier,
        }
    }

    #[test]
    fn stability_upgrade_keeps_damping_below_one() {
        let mut loadout = Loadout::new();
        loadout.add_upgrade(stability(1.1));
        assert!(
            loadout.damping().factor() < 1.0,
            "damping is per-frame velocity retention; at >= 1.0 motion never \
             decays and the ship spins forever, got {}",
            loadout.damping().factor()
        );
    }

    #[test]
    fn stability_upgrade_settles_the_ship_faster() {
        let base = Loadout::new();
        let mut upgraded = Loadout::new();
        upgraded.add_upgrade(stability(1.2));
        assert!(
            upgraded.damping().factor() < base.damping().factor(),
            "a stability upgrade must decay velocity faster than base \
             (smaller retention), got {} vs base {}",
            upgraded.damping().factor(),
            base.damping().factor()
        );
    }

    #[test]
    fn stacked_stability_upgrades_never_exceed_base_retention() {
        let base = Loadout::new().damping().factor();
        let mut loadout = Loadout::new();
        for _ in 0..30 {
            loadout.add_upgrade(stability(1.3));
        }
        let damping = loadout.damping().factor();
        assert!(
            (0.0..=base).contains(&damping),
            "stacked stability upgrades must keep retention in [0, base], got {damping}"
        );
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
