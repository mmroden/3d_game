//! Classification of enemies as mechanical (engineered systems) or biological (de-evolved creatures).

/// Whether an enemy is a mechanical defense system or a biological creature.
/// Every current enemy is [`Mechanical`](EnemyCategory::Mechanical) and drops
/// components; organics are sourced from barrels instead. [`Biological`](EnemyCategory::Biological)
/// is retained for forward-compatibility should creature enemies return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnemyCategory {
    /// Engineered defense systems. Drops components (in-run currency).
    Mechanical,
    /// De-evolved biological creatures. Currently unused.
    Biological,
}
