//! Kinetic core: `Retention`, the velocity-dissipation invariant that
//! drives the ship's flight-assist.
//!
//! The bespoke physics vocabulary (`Restitution`, `Mass`, `ControlInput`,
//! `Impulse`, `SpeedLimits`, `AngularState`) was removed with the owned
//! `KineticWorld` — Godot/Jolt owns motion now. Only `Retention`
//! survives, because the flight-assist brake is expressed in its terms.

use serde::{Deserialize, Serialize};

/// Per-second velocity retention factor.
///
/// `FULL` (exactly 1.0) is reserved for ballistic drift; every other
/// value is forced below 1.0 so assisted motion always dissipates. The
/// infinite-spin bug (2026-06-11) is unrepresentable in this type.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Retention(f32);

impl Retention {
    /// Ballistic drift: velocity preserved exactly.
    pub const FULL: Retention = Retention(1.0);

    /// Largest decaying retention; `decaying` clamps here so assisted
    /// motion can never reach the perpetual-drift regime by accident.
    pub const MAX_DECAYING: f32 = 0.99;

    /// A retention factor guaranteed to dissipate: clamped to
    /// `[0.0, MAX_DECAYING]`.
    pub fn decaying(value: f32) -> Self {
        Self(value.clamp(0.0, Self::MAX_DECAYING))
    }

    pub fn factor(self) -> f32 {
        self.0
    }

    /// The retention factor for one tick of length `dt` seconds.
    /// Retention is defined per second so handling is identical at any
    /// tick rate. `FULL` stays exactly 1.0 (drift exactness theorem).
    pub fn factor_for(self, dt: f32) -> f32 {
        if self.0 == 1.0 {
            return 1.0;
        }
        self.0.powf(dt.max(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decaying_retention_is_always_below_one() {
        // 0.95 * 1.1 = 1.045 was the infinite-spin bug; the type must
        // refuse to represent it as a decaying factor.
        assert!(Retention::decaying(1.045).factor() < 1.0);
        assert!(Retention::decaying(f32::MAX).factor() < 1.0);
        assert_eq!(Retention::decaying(0.95).factor(), 0.95);
    }

    #[test]
    fn full_retention_is_exactly_one() {
        assert_eq!(Retention::FULL.factor(), 1.0);
        assert_eq!(Retention::FULL.factor_for(1.0 / 120.0), 1.0);
    }

    #[test]
    fn retention_is_tick_rate_invariant() {
        // One simulated second must decay identically whether it is 60
        // ticks or 120: changing physics_ticks_per_second can never
        // change handling feel.
        let r = Retention::decaying(0.05);
        let mut v60 = 1.0_f32;
        for _ in 0..60 {
            v60 *= r.factor_for(1.0 / 60.0);
        }
        let mut v120 = 1.0_f32;
        for _ in 0..120 {
            v120 *= r.factor_for(1.0 / 120.0);
        }
        assert!(
            (v60 - v120).abs() < 1e-4,
            "decay must be tick-rate invariant: 60 Hz {v60}, 120 Hz {v120}"
        );
        assert!(
            (v60 - 0.05).abs() < 0.01,
            "per-second semantics: one second at 5%/s leaves ~5%, got {v60}"
        );
    }
}
