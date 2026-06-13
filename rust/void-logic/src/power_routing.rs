//! Power routing modes: divert energy between shields, weapons, and engines.

/// How the ship's power plant distributes energy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PowerMode {
    /// Default: no modifiers.
    #[default]
    Balanced,
    /// Square button: boost shields at expense of weapons and engines.
    ShieldBoost,
    /// Circle button: boost weapons, no shield regen.
    WeaponBoost,
}

impl PowerMode {
    pub fn shield_regen_multiplier(self) -> f32 {
        match self {
            Self::Balanced => 1.0,
            Self::ShieldBoost => 5.0,
            Self::WeaponBoost => 0.0,
        }
    }

    pub fn fire_rate_multiplier(self) -> f32 {
        match self {
            Self::Balanced => 1.0,
            Self::ShieldBoost => 0.3,
            Self::WeaponBoost => 1.5,
        }
    }

    pub fn thrust_multiplier(self) -> f32 {
        match self {
            Self::Balanced => 1.0,
            Self::ShieldBoost => 0.5,
            Self::WeaponBoost => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_is_all_ones() {
        assert_eq!(PowerMode::Balanced.shield_regen_multiplier(), 1.0);
        assert_eq!(PowerMode::Balanced.fire_rate_multiplier(), 1.0);
        assert_eq!(PowerMode::Balanced.thrust_multiplier(), 1.0);
    }

    #[test]
    fn shield_boost_5x_regen_severe_fire_rate_cut() {
        assert_eq!(PowerMode::ShieldBoost.shield_regen_multiplier(), 5.0);
        assert_eq!(PowerMode::ShieldBoost.fire_rate_multiplier(), 0.3);
        assert_eq!(PowerMode::ShieldBoost.thrust_multiplier(), 0.5);
    }

    #[test]
    fn weapon_boost_stops_regen_increases_fire_rate() {
        assert_eq!(PowerMode::WeaponBoost.shield_regen_multiplier(), 0.0);
        assert_eq!(PowerMode::WeaponBoost.fire_rate_multiplier(), 1.5);
        assert_eq!(PowerMode::WeaponBoost.thrust_multiplier(), 1.0);
    }

    #[test]
    fn shield_boost_is_a_tradeoff() {
        assert!(PowerMode::ShieldBoost.fire_rate_multiplier() < 1.0);
        assert!(PowerMode::ShieldBoost.thrust_multiplier() < 1.0);
    }

    #[test]
    fn default_is_balanced() {
        assert_eq!(PowerMode::default(), PowerMode::Balanced);
    }
}
