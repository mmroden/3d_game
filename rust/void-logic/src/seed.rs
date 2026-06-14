use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};
use serde::{Deserialize, Serialize};

/// Identifies the random stream for a run or a level.
///
/// Owns every conversion at the Godot boundary (Variant carries only
/// signed 64-bit integers), so raw integer casts never appear outside
/// this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Seed(u64);

impl Seed {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn value(self) -> u64 {
        self.0
    }

    /// Bit-preserving conversion from Godot's signed 64-bit integer.
    pub fn from_i64(value: i64) -> Self {
        Self(value as u64)
    }

    /// Bit-preserving conversion to Godot's signed 64-bit integer.
    pub fn as_i64(self) -> i64 {
        self.0 as i64
    }

    /// Derive the seed for `level` from this run seed: the `level`-th
    /// draw from the run's random stream. Unlike an arithmetic offset,
    /// draws from streams seeded differently never line up, so two runs
    /// can't share level layouts by seed coincidence.
    pub fn for_level(self, level: u32) -> Self {
        let mut stream = SmallRng::seed_from_u64(self.0);
        let mut derived = self.0;
        for _ in 0..=level {
            derived = stream.random();
        }
        Self(derived)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_roundtrips() {
        assert_eq!(Seed::new(42).value(), 42);
    }

    #[test]
    fn godot_i64_conversion_preserves_all_bits() {
        let seed = Seed::new(u64::MAX);
        assert_eq!(Seed::from_i64(seed.as_i64()), seed);
        assert_eq!(Seed::from_i64(-1).value(), u64::MAX);
    }

    #[test]
    fn for_level_is_deterministic() {
        assert_eq!(Seed::new(42).for_level(3), Seed::new(42).for_level(3));
    }

    #[test]
    fn for_level_differs_across_levels() {
        let run = Seed::new(42);
        assert_ne!(run.for_level(1), run.for_level(2));
    }

    #[test]
    fn for_level_differs_across_runs() {
        assert_ne!(Seed::new(42).for_level(1), Seed::new(999).for_level(1));
    }

    #[test]
    fn level_streams_from_different_runs_do_not_overlap() {
        // An arithmetic derivation collides: run A's level 2 equals run
        // B's level 1 whenever the run seeds differ by the stride. Two
        // players must never walk each other's levels just because their
        // seeds happen to be close.
        let run_a = Seed::new(42);
        let run_b = Seed::new(42 + 7919);
        assert_ne!(
            run_a.for_level(2),
            run_b.for_level(1),
            "per-level seeds must be decorrelated across run seeds"
        );
    }
}
