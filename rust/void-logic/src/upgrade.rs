use rand::Rng;
use rand::RngExt;
use serde::{Deserialize, Serialize};

/// What aspect of the ship an upgrade modifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpgradeKind {
    Thrust,
    RotationSpeed,
    Damping,
    MaxHealth,
    FireRate,
    ProjectileSpeed,
    ProjectileDamage,
}

impl UpgradeKind {
    pub const ALL: &[UpgradeKind] = &[
        UpgradeKind::Thrust,
        UpgradeKind::RotationSpeed,
        UpgradeKind::Damping,
        UpgradeKind::MaxHealth,
        UpgradeKind::FireRate,
        UpgradeKind::ProjectileSpeed,
        UpgradeKind::ProjectileDamage,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Thrust => "Thrust",
            Self::RotationSpeed => "Rotation",
            Self::Damping => "Stability",
            Self::MaxHealth => "Armor",
            Self::FireRate => "Fire Rate",
            Self::ProjectileSpeed => "Beam Focus",
            Self::ProjectileDamage => "Damage",
        }
    }
}

/// A single upgrade instance, as found in a lootbox.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Upgrade {
    pub name: String,
    pub kind: UpgradeKind,
    /// Multiplicative modifier (1.0 = no change, 1.1 = +10%).
    pub multiplier: f32,
}

/// Generate a random upgrade with multiplier between 1.1x and 1.3x.
pub fn random_upgrade(rng: &mut impl Rng) -> Upgrade {
    let kind = UpgradeKind::ALL[rng.random_range(0..UpgradeKind::ALL.len())];
    // Multiplier: 1.1 to 1.3 in 0.05 increments
    let steps = rng.random_range(0..=4_u32);
    let multiplier = 1.1 + steps as f32 * 0.05;
    let percent = ((multiplier - 1.0) * 100.0).round() as u32;
    Upgrade {
        name: format!("{} +{}%", kind.label(), percent),
        kind,
        multiplier,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn random_upgrade_produces_valid_kind() {
        let mut rng = SmallRng::seed_from_u64(42);
        for _ in 0..100 {
            let upgrade = random_upgrade(&mut rng);
            assert!(UpgradeKind::ALL.contains(&upgrade.kind));
        }
    }

    #[test]
    fn random_upgrade_multiplier_in_range() {
        let mut rng = SmallRng::seed_from_u64(42);
        for _ in 0..100 {
            let upgrade = random_upgrade(&mut rng);
            assert!(
                upgrade.multiplier >= 1.09 && upgrade.multiplier <= 1.31,
                "multiplier {} out of range", upgrade.multiplier
            );
        }
    }

    #[test]
    fn random_upgrade_has_name() {
        let mut rng = SmallRng::seed_from_u64(42);
        let upgrade = random_upgrade(&mut rng);
        assert!(!upgrade.name.is_empty());
        assert!(upgrade.name.contains('+'));
    }

    #[test]
    fn different_seeds_produce_variety() {
        let mut kinds = std::collections::HashSet::new();
        for seed in 0..50 {
            let mut rng = SmallRng::seed_from_u64(seed);
            kinds.insert(format!("{:?}", random_upgrade(&mut rng).kind));
        }
        assert!(kinds.len() >= 3, "expected variety, got {:?}", kinds);
    }
}
