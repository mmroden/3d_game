//! Planet structure: the campaign is divided into planets of six levels
//! each. Bosses are scheduled deterministically within every planet — a
//! mid-planet boss and a planet-final boss (owner's call 2026-07-04,
//! superseding the earlier chance-based roll): predictable rhythm is the
//! world-building, and the reward split (consolation pile vs red hull
//! container) hangs off the same schedule.

/// Levels per planet. The portal after each planet's final boss carries the
/// player to the next planet (new wall palette, new interstitial — B10).
pub const PLANET_LENGTH: u32 = 6;

/// 1-based planet index for a 1-based level: levels 1-6 → planet 1,
/// 7-12 → planet 2, and so on.
pub fn planet_of(level: u32) -> u32 {
    (level.saturating_sub(1)) / PLANET_LENGTH + 1
}

/// 1-based position of a level within its planet (1..=PLANET_LENGTH).
pub fn planet_relative(level: u32) -> u32 {
    (level.saturating_sub(1)) % PLANET_LENGTH + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_levels_per_planet() {
        for level in 1..=6 {
            assert_eq!(planet_of(level), 1, "level {level} is planet 1");
        }
        for level in 7..=12 {
            assert_eq!(planet_of(level), 2, "level {level} is planet 2");
        }
        assert_eq!(planet_of(13), 3);
    }

    #[test]
    fn planet_relative_cycles_one_through_six() {
        assert_eq!(planet_relative(1), 1);
        assert_eq!(planet_relative(3), 3);
        assert_eq!(planet_relative(6), 6);
        assert_eq!(planet_relative(7), 1, "planet 2 restarts the cycle");
        assert_eq!(planet_relative(9), 3);
        assert_eq!(planet_relative(12), 6);
    }

    #[test]
    fn the_algebra_reconstructs_the_level() {
        // planet_of and planet_relative are a proper quotient/remainder pair.
        for level in 1..=36 {
            let rebuilt = (planet_of(level) - 1) * PLANET_LENGTH + planet_relative(level);
            assert_eq!(rebuilt, level);
        }
    }
}
