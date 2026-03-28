/// What aspect of the ship an upgrade modifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeKind {
    Thrust,
    RotationSpeed,
    Damping,
    MaxHealth,
    FireRate,
    ProjectileSpeed,
    ProjectileDamage,
}

/// A single upgrade instance, as found in a lootbox.
#[derive(Debug, Clone)]
pub struct Upgrade {
    pub name: String,
    pub kind: UpgradeKind,
    /// Multiplicative modifier (1.0 = no change, 1.1 = +10%).
    pub multiplier: f32,
}
