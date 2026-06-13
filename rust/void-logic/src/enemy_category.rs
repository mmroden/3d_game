//! Classification of enemies as mechanical (engineered systems) or biological (de-evolved creatures).

/// Whether an enemy is a mechanical defense system or a biological creature.
/// Determines what currency it drops: components (mechanical) or organics (biological).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnemyCategory {
    /// Engineered defense systems: GunDrone, EyeDrone, QuadOrb, QuadShell
    Mechanical,
    /// De-evolved biological creatures: Slime, Bat, Shark, Raptor, Skeleton, Trilobite, Dragon
    Biological,
}
