//! Kinetic core: the shared motion vocabulary and its invariants.
//!
//! The engine (rapier, behind `KineticWorld`) integrates all linear
//! motion; this module owns the types that constrain it — `Retention`
//! (dissipation violations are unrepresentable), `Restitution`,
//! `SpeedLimits`, `Mass`, `ControlInput`, `Impulse` — plus
//! `AngularState`, the exact per-second integrator for the ship's
//! local rotation feel (the camera is the ship).

use serde::{Deserialize, Serialize};

/// Per-frame velocity retention factor.
///
/// `FULL` (exactly 1.0) is reserved for ballistic drift; every other
/// value is forced below 1.0 so powered motion always dissipates. The
/// infinite-spin bug (2026-06-11) is unrepresentable in this type.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Retention(f32);

impl Retention {
    /// Ballistic drift: velocity preserved exactly between collisions.
    pub const FULL: Retention = Retention(1.0);

    /// Largest decaying retention; `decaying` clamps here so powered
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

    /// The thrust (acceleration) that sustains `target_speed` against
    /// this retention's decay: a = v · (−ln r). Zero for `FULL`
    /// (drifting bodies need no thrust to hold speed).
    pub fn cruise_thrust(self, target_speed: f32) -> f32 {
        target_speed * -self.0.ln()
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

/// Coefficient of restitution for a bounce, guaranteed < 1.0 so no
/// collision can add energy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Restitution(f32);

impl Restitution {
    /// Largest allowed coefficient; `clamped` clamps here.
    pub const MAX: f32 = 0.99;

    /// A restitution coefficient clamped to `[0.0, MAX]`.
    pub fn clamped(value: f32) -> Self {
        Self(value.clamp(0.0, Self::MAX))
    }

    pub fn coefficient(self) -> f32 {
        self.0
    }
}

/// Propulsion output for one tick. Ballistic bodies never construct one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlInput {
    pub thrust: [f32; 3],
    pub torque: [f32; 3],
}

impl ControlInput {
    pub const NONE: ControlInput = ControlInput {
        thrust: [0.0; 3],
        torque: [0.0; 3],
    };
}

/// An instantaneous momentum change (collision response). Consumed by
/// value: an impulse can only be applied once.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Impulse {
    pub linear: [f32; 3],
    pub angular: [f32; 3],
}

/// Hard velocity caps applied inside `step`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeedLimits {
    pub linear: f32,
    pub angular: f32,
}

/// Inertial mass in kilograms, strictly positive by construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mass(f32);

impl Mass {
    pub fn kilograms(value: f32) -> Self {
        Self(value.max(0.001))
    }

    pub fn as_f32(self) -> f32 {
        self.0
    }
}

/// Angular velocity integrated locally by a body that owns its own
/// orientation (the ship: the camera is the ship, so rotation feel
/// keeps this exact per-second math). Linear motion belongs to the
/// world. Mutation only through `step` and `halt` — no raw setter.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AngularState {
    velocity: [f32; 3],
}

impl AngularState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn velocity(&self) -> [f32; 3] {
        self.velocity
    }

    /// Integrate one tick: accelerate by `torque`, decay by
    /// `retention`, then sanitize (NaN reset, speed cap).
    pub fn step(&mut self, torque: [f32; 3], retention: Retention, max_speed: f32, delta: f32) {
        let r = retention.factor_for(delta);
        self.velocity = sanitize(scale(add(self.velocity, scale(torque, delta)), r), max_speed);
    }

    /// Stabilizer: kill all rotation immediately.
    pub fn halt(&mut self) {
        self.velocity = [0.0; 3];
    }
}

pub(crate) fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(crate) fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn scale(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

pub(crate) fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Reset NaN to rest; clamp magnitude to `max_speed`.
pub(crate) fn sanitize(v: [f32; 3], max_speed: f32) -> [f32; 3] {
    if v[0].is_nan() || v[1].is_nan() || v[2].is_nan() {
        return [0.0; 3];
    }
    let len = dot(v, v).sqrt();
    if len > max_speed {
        scale(v, max_speed / len)
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speed(v: [f32; 3]) -> f32 {
        dot(v, v).sqrt()
    }

    const MAX_SPIN: f32 = 5.0;
    const DT: f32 = 1.0 / 60.0;

    /// Spin a fresh state up to exactly `velocity`: one one-second
    /// FULL-retention step integrates torque·1s with no decay.
    fn spinning(velocity: [f32; 3]) -> AngularState {
        let mut state = AngularState::new();
        state.step(velocity, Retention::FULL, f32::MAX, 1.0);
        state
    }

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
        // One simulated second must decay identically whether it is
        // 60 ticks or 120: changing physics_ticks_per_second can never
        // change handling feel.
        let retention = Retention::decaying(0.05);
        let mut at_60 = spinning([30.0, 0.0, 0.0]);
        let mut at_120 = spinning([30.0, 0.0, 0.0]);
        for _ in 0..60 {
            at_60.step([0.0; 3], retention, f32::MAX, 1.0 / 60.0);
        }
        for _ in 0..120 {
            at_120.step([0.0; 3], retention, f32::MAX, 1.0 / 120.0);
        }
        let v60 = at_60.velocity()[0];
        let v120 = at_120.velocity()[0];
        assert!(
            (v60 - v120).abs() < 0.05,
            "decay must be tick-rate invariant: 60 Hz gave {v60}, 120 Hz gave {v120}"
        );
        // And one second at 5% per-second retention leaves ~5% of 30.
        assert!(
            (v60 - 1.5).abs() < 0.2,
            "per-second semantics: expected ~1.5 after one second, got {v60}"
        );
    }

    #[test]
    fn restitution_is_always_below_one() {
        assert!(Restitution::clamped(1.5).coefficient() < 1.0);
        assert_eq!(Restitution::clamped(0.5).coefficient(), 0.5);
        assert!(Restitution::clamped(-0.5).coefficient() >= 0.0);
    }

    #[test]
    fn torque_accelerates() {
        let mut state = AngularState::new();
        state.step([0.0, 2.0, 0.0], Retention::decaying(0.95), MAX_SPIN, DT);
        assert!(state.velocity()[1] > 0.0);
    }

    #[test]
    fn spin_converges_to_zero_once_input_stops() {
        // The invariant that would have caught the infinite-spin bug:
        // for any finite input history, no input means motion dies.
        let mut state = spinning([3.0, -2.0, 1.0]);
        // 5% retained per second over 10 simulated seconds.
        for _ in 0..600 {
            state.step([0.0; 3], Retention::decaying(0.05), MAX_SPIN, DT);
        }
        assert!(
            speed(state.velocity()) < 1e-3,
            "angular velocity must decay to zero, got {:?}",
            state.velocity()
        );
    }

    #[test]
    fn full_retention_preserves_spin_exactly() {
        let mut state = spinning([0.1, 0.2, -0.1]);
        let before = state;
        for _ in 0..600 {
            state.step([0.0; 3], Retention::FULL, MAX_SPIN, DT);
        }
        assert_eq!(state, before, "drift must neither decay nor grow");
    }

    #[test]
    fn speed_limit_is_enforced_inside_step() {
        let mut state = AngularState::new();
        for _ in 0..1000 {
            state.step([0.0, 1e6, 0.0], Retention::FULL, MAX_SPIN, DT);
        }
        assert!(speed(state.velocity()) <= MAX_SPIN + 1e-3);
    }

    #[test]
    fn nan_torque_is_reset_by_step() {
        let mut state = AngularState::new();
        state.step([0.0, f32::NAN, 0.0], Retention::decaying(0.95), MAX_SPIN, DT);
        assert_eq!(state.velocity(), [0.0; 3], "NaN must reset to rest");
    }

    #[test]
    fn halt_zeroes_rotation() {
        let mut state = spinning([2.0, 2.0, 2.0]);
        state.halt();
        assert_eq!(state.velocity(), [0.0; 3]);
    }
}
