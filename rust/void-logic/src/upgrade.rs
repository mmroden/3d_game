use serde::{Deserialize, Serialize};

/// What aspect of the ship an upgrade modifies — the shop's stat stock.
/// Damping ("Stability") and projectile speed ("Beam Focus") were retired
/// with the free-drop economy; their base values are fixed on `BaseStats`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpgradeKind {
    Thrust,
    RotationSpeed,
    MaxHealth,
    FireRate,
    ProjectileDamage,
    /// Shield envelope (+10% capacity per purchase, like every stat).
    /// Appended LAST: ids are positions in `ALL`, and the shop id space
    /// (laser, life, unlocks) continues after the stats.
    ShieldCapacity,
}

impl UpgradeKind {
    pub const ALL: &[UpgradeKind] = &[
        UpgradeKind::Thrust,
        UpgradeKind::RotationSpeed,
        UpgradeKind::MaxHealth,
        UpgradeKind::FireRate,
        UpgradeKind::ProjectileDamage,
        UpgradeKind::ShieldCapacity,
    ];

    /// Stable id for GDScript crossings (position in `ALL`), like `EnemyType`.
    pub fn id(&self) -> i32 {
        Self::ALL.iter().position(|k| k == self)
            .expect("UpgradeKind::ALL must contain every variant") as i32
    }

    pub fn from_id(id: i32) -> Option<UpgradeKind> {
        Self::ALL.get(id as usize).copied()
    }

    /// Display label for shop stock and upgrade names.
    pub fn label(self) -> &'static str {
        match self {
            Self::Thrust => "Thrust",
            Self::RotationSpeed => "Rotation",
            Self::MaxHealth => "Armor",
            Self::FireRate => "Fire Rate",
            Self::ProjectileDamage => "Damage",
            Self::ShieldCapacity => "Shields",
        }
    }
}

/// A single upgrade instance, bought at the between-level shop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Upgrade {
    pub name: String,
    pub kind: UpgradeKind,
    /// Multiplicative modifier (1.0 = no change, 1.1 = +10%).
    pub multiplier: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shop_stock_is_six_kinds() {
        // Damping ("Stability") and ProjectileSpeed ("Beam Focus") are retired:
        // nothing grants them under the shop economy. Their base stats remain
        // fixed values on BaseStats. Shields joined the stock 2026-07-04
        // (playtest ask), appended last so the id space didn't reshuffle.
        assert_eq!(UpgradeKind::ALL.len(), 6,
            "shop stock: Thrust, Rotation, Armor, Fire Rate, Damage, Shields");
        assert_eq!(UpgradeKind::ShieldCapacity.id(), 5,
            "Shields is appended — the laser/life/unlock ids sit after it");
        for kind in UpgradeKind::ALL {
            assert!(kind.label() != "Stability" && kind.label() != "Beam Focus",
                "{kind:?} is retired stock");
        }
    }

    #[test]
    fn kind_id_round_trips() {
        for kind in UpgradeKind::ALL {
            assert_eq!(UpgradeKind::from_id(kind.id()), Some(*kind),
                "{kind:?} must round-trip through its id");
        }
    }

    #[test]
    fn from_id_invalid_is_none() {
        assert_eq!(UpgradeKind::from_id(-1), None);
        assert_eq!(UpgradeKind::from_id(99), None);
    }
}
