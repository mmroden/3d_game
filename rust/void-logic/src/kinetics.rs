//! Kinetic core: the one integrator and bounce shared by every mover.
//!
//! Powered movers (ship, enemies) and ballistic movers (drifting props,
//! physical projectiles) all step their velocities through this module,
//! so the dissipation invariants live — and are tested — exactly once:
//!
//! - decaying retention is provably < 1.0 (motion always dies out);
//! - full retention preserves velocity exactly (drift never amplifies);
//! - a bounce never gains energy;
//! - speed limits and NaN recovery are applied inside `step`, not by
//!   each caller remembering to.

/// Per-frame velocity retention factor.
///
/// `FULL` (exactly 1.0) is reserved for ballistic drift; every other
/// value is forced below 1.0 so powered motion always dissipates. The
/// infinite-spin bug (2026-06-11) is unrepresentable in this type.
#[derive(Debug, Clone, Copy, PartialEq)]
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

/// Linear and angular velocity of a mover. Mutation only through
/// `step`, `apply_impulse`, `halt_rotation`, and the engine-readback
/// hook — there is no raw setter.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct KineticState {
    linear_velocity: [f32; 3],
    angular_velocity: [f32; 3],
}

impl KineticState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn linear_velocity(&self) -> [f32; 3] {
        self.linear_velocity
    }

    pub fn angular_velocity(&self) -> [f32; 3] {
        self.angular_velocity
    }

    /// Integrate one tick: accelerate by `control`, decay by
    /// `retention`, then sanitize (NaN reset, speed caps).
    pub fn step(
        &mut self,
        control: ControlInput,
        retention: Retention,
        limits: SpeedLimits,
        delta: f32,
    ) {
        let r = retention.factor();
        self.linear_velocity = sanitize(
            scale(add(self.linear_velocity, scale(control.thrust, delta)), r),
            limits.linear,
        );
        self.angular_velocity = sanitize(
            scale(add(self.angular_velocity, scale(control.torque, delta)), r),
            limits.angular,
        );
    }

    /// Apply a one-shot momentum change (collision response).
    pub fn apply_impulse(&mut self, impulse: Impulse) {
        self.linear_velocity = add(self.linear_velocity, impulse.linear);
        self.angular_velocity = add(self.angular_velocity, impulse.angular);
    }

    /// Stabilizer: kill all rotation immediately.
    pub fn halt_rotation(&mut self) {
        self.angular_velocity = [0.0; 3];
    }

    /// Accept the engine's collision-resolved linear velocity as truth
    /// (CharacterBody3D `move_and_slide` readback). Without this,
    /// thrust accumulates into walls until the mover clips through.
    pub fn accept_resolved_linear(&mut self, velocity: [f32; 3]) {
        self.linear_velocity = velocity;
    }
}

/// Reflect `velocity` off a surface with unit `normal`. Only the
/// approaching component is reflected (scaled by restitution); a
/// velocity already separating from the surface is returned unchanged.
pub fn bounce(velocity: [f32; 3], normal: [f32; 3], restitution: Restitution) -> [f32; 3] {
    let vn = dot(velocity, normal);
    if vn >= 0.0 {
        return velocity;
    }
    sub(velocity, scale(normal, (1.0 + restitution.coefficient()) * vn))
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

pub(crate) fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Reset NaN to rest; clamp magnitude to `max_speed`.
fn sanitize(v: [f32; 3], max_speed: f32) -> [f32; 3] {
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

    const LIMITS: SpeedLimits = SpeedLimits {
        linear: 50.0,
        angular: 5.0,
    };
    const DT: f32 = 1.0 / 60.0;

    fn impulse(linear: [f32; 3], angular: [f32; 3]) -> Impulse {
        Impulse { linear, angular }
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
    }

    #[test]
    fn restitution_is_always_below_one() {
        assert!(Restitution::clamped(1.5).coefficient() < 1.0);
        assert_eq!(Restitution::clamped(0.5).coefficient(), 0.5);
        assert!(Restitution::clamped(-0.5).coefficient() >= 0.0);
    }

    #[test]
    fn thrust_and_torque_accelerate() {
        let mut state = KineticState::new();
        state.step(
            ControlInput { thrust: [10.0, 0.0, 0.0], torque: [0.0, 2.0, 0.0] },
            Retention::decaying(0.95),
            LIMITS,
            DT,
        );
        assert!(state.linear_velocity()[0] > 0.0);
        assert!(state.angular_velocity()[1] > 0.0);
    }

    #[test]
    fn velocity_converges_to_zero_once_input_stops() {
        // The invariant that would have caught the infinite-spin bug:
        // for any finite input history, no input means motion dies.
        let mut state = KineticState::new();
        state.apply_impulse(impulse([30.0, -20.0, 10.0], [3.0, -2.0, 1.0]));
        for _ in 0..600 {
            state.step(ControlInput::NONE, Retention::decaying(0.95), LIMITS, DT);
        }
        assert!(
            speed(state.linear_velocity()) < 1e-3,
            "linear velocity must decay to zero, got {:?}",
            state.linear_velocity()
        );
        assert!(
            speed(state.angular_velocity()) < 1e-3,
            "angular velocity must decay to zero, got {:?}",
            state.angular_velocity()
        );
    }

    #[test]
    fn full_retention_preserves_drift_exactly() {
        let mut state = KineticState::new();
        state.apply_impulse(impulse([1.5, -0.5, 0.25], [0.1, 0.2, -0.1]));
        let before = state;
        for _ in 0..600 {
            state.step(ControlInput::NONE, Retention::FULL, LIMITS, DT);
        }
        assert_eq!(state, before, "ballistic drift must neither decay nor grow");
    }

    #[test]
    fn speed_limits_are_enforced_inside_step() {
        let mut state = KineticState::new();
        for _ in 0..1000 {
            state.step(
                ControlInput { thrust: [1e6, 0.0, 0.0], torque: [0.0, 1e6, 0.0] },
                Retention::FULL,
                LIMITS,
                DT,
            );
        }
        assert!(speed(state.linear_velocity()) <= LIMITS.linear + 1e-3);
        assert!(speed(state.angular_velocity()) <= LIMITS.angular + 1e-3);
    }

    #[test]
    fn nan_velocity_is_reset_by_step() {
        let mut state = KineticState::new();
        state.apply_impulse(impulse([f32::NAN, 0.0, 0.0], [0.0, f32::NAN, 0.0]));
        state.step(ControlInput::NONE, Retention::decaying(0.95), LIMITS, DT);
        assert_eq!(state.linear_velocity(), [0.0; 3], "NaN must reset to rest");
        assert_eq!(state.angular_velocity(), [0.0; 3], "NaN must reset to rest");
    }

    #[test]
    fn impulse_changes_both_velocities() {
        let mut state = KineticState::new();
        state.apply_impulse(impulse([1.0, 2.0, 3.0], [0.5, 0.0, -0.5]));
        assert_eq!(state.linear_velocity(), [1.0, 2.0, 3.0]);
        assert_eq!(state.angular_velocity(), [0.5, 0.0, -0.5]);
    }

    #[test]
    fn halt_rotation_zeroes_angular_only() {
        let mut state = KineticState::new();
        state.apply_impulse(impulse([1.0, 0.0, 0.0], [2.0, 2.0, 2.0]));
        state.halt_rotation();
        assert_eq!(state.angular_velocity(), [0.0; 3]);
        assert_eq!(state.linear_velocity(), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn engine_readback_replaces_linear_velocity() {
        let mut state = KineticState::new();
        state.apply_impulse(impulse([9.0, 9.0, 9.0], [0.0; 3]));
        state.accept_resolved_linear([1.0, 0.0, 0.0]);
        assert_eq!(state.linear_velocity(), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn bounce_reflects_the_approaching_component() {
        let out = bounce([0.0, -10.0, 0.0], [0.0, 1.0, 0.0], Restitution::clamped(0.5));
        assert_eq!(out, [0.0, 5.0, 0.0]);
    }

    #[test]
    fn bounce_preserves_the_tangential_component() {
        let out = bounce([3.0, -10.0, 4.0], [0.0, 1.0, 0.0], Restitution::clamped(0.5));
        assert_eq!(out, [3.0, 5.0, 4.0]);
    }

    #[test]
    fn bounce_never_gains_energy() {
        let cases = [
            ([5.0, -3.0, 2.0], [0.0, 1.0, 0.0]),
            ([-4.0, 0.0, -4.0], [0.70710678, 0.0, 0.70710678]),
            ([0.0, -1.0, 0.0], [0.0, 1.0, 0.0]),
        ];
        for (v, n) in cases {
            for e in [0.0, 0.5, 0.99] {
                let out = bounce(v, n, Restitution::clamped(e));
                assert!(
                    speed(out) <= speed(v) + 1e-4,
                    "bounce gained energy: {v:?} -> {out:?} (e={e})"
                );
            }
        }
    }

    #[test]
    fn bounce_leaves_separating_velocity_unchanged() {
        // Already moving away from the surface: no reflection, or a
        // resting contact would jitter forever.
        let v = [0.0, 10.0, 0.0];
        let out = bounce(v, [0.0, 1.0, 0.0], Restitution::clamped(0.5));
        assert_eq!(out, v);
    }
}
