//! Mouse look as a virtual stick (alpha note 2026-09-15: "using keys was
//! tough"). Mouse motion arrives per rendered frame in pixels; the ship
//! steers per physics tick toward a commanded angular rate. The pending
//! motion is spent over one frame's worth of ticks as a rate command in
//! the stick's own units — a deflection in [-1, 1] of the full turn
//! rate — so the mouse feels like the pad: move, the ship turns; stop,
//! it stops. Pure arithmetic, no engine types.

/// Pending mouse motion, in pixels, not yet turned into steering, and
/// the frame's total it is spent against (a fixed share of the frame's
/// motion per tick, so two ticks spend a frame evenly and completely).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MouseLook {
    pending: [f32; 2],
    frame: [f32; 2],
}

impl MouseLook {
    /// How long a frame's motion is spread over, in seconds: one rendered
    /// frame at 60 Hz, so the 120 Hz physics tick sees it twice and never
    /// alternates between a jolt and nothing.
    pub const HOLD: f32 = 1.0 / 60.0;
    /// Radians of turn per pixel at sensitivity 1; sensitivity 1..=10
    /// scales it. At 5, a 100 px sweep is 0.15 rad.
    pub const RADIANS_PER_PIXEL: f32 = 0.0003;

    pub fn new() -> Self {
        Self::default()
    }

    /// A frame's mouse motion: `dx` right, `dy` down, in pixels. Motion
    /// pushed while some is still pending joins it, and the frame's
    /// total becomes the new spending base.
    pub fn push(&mut self, dx: f32, dy: f32) {
        self.pending[0] += dx;
        self.pending[1] += dy;
        self.frame = self.pending;
    }

    /// Whether motion is waiting to be spent.
    pub fn is_pending(&self) -> bool {
        self.pending != [0.0, 0.0]
    }

    /// Spend this tick's share of the pending motion (`dt` seconds of a
    /// `HOLD`-long frame) as a stick deflection `[pitch, yaw]` in
    /// [-1, 1] of `max_rate` (rad/s): mouse up pitches up, mouse right
    /// yaws right — in the ship's sign convention (pitch up and yaw left
    /// positive), so `dy > 0` is negative pitch and `dx > 0` negative
    /// yaw. `invert_y` flips pitch. A flick past the full rate clamps.
    pub fn deflection(&mut self, dt: f32, sensitivity: u8, invert_y: bool, max_rate: f32) -> [f32; 2] {
        if !self.is_pending() || dt <= 0.0 || max_rate <= 0.0 {
            return [0.0, 0.0];
        }
        // This tick's share of the FRAME's motion, never more than is
        // still pending (the last tick of a hold takes the remainder).
        let share = (dt / Self::HOLD).min(1.0);
        let mut spent = [0.0; 2];
        for axis in 0..2 {
            let want = self.frame[axis] * share;
            spent[axis] = if want.abs() >= self.pending[axis].abs() { self.pending[axis] } else { want };
            self.pending[axis] -= spent[axis];
            if self.pending[axis].abs() < 1e-4 {
                self.pending[axis] = 0.0;
            }
        }
        // Pixels → radians → a rate over this tick → a fraction of the
        // stick's full rate.
        let radians_per_pixel = Self::RADIANS_PER_PIXEL * sensitivity.max(1) as f32;
        let rate = |pixels: f32| (pixels * radians_per_pixel / dt / max_rate).clamp(-1.0, 1.0);
        let pitch_sign = if invert_y { 1.0 } else { -1.0 };
        [rate(spent[1]) * pitch_sign, -rate(spent[0])]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 2.5;
    const TICK: f32 = 1.0 / 120.0;

    #[test]
    fn nothing_pending_steers_nothing() {
        let mut look = MouseLook::new();
        assert!(!look.is_pending());
        assert_eq!(look.deflection(TICK, 5, false, RATE), [0.0, 0.0]);
    }

    #[test]
    fn a_frames_motion_is_spent_over_one_hold_and_then_stops() {
        let mut look = MouseLook::new();
        look.push(20.0, 0.0);
        assert!(look.is_pending());
        // Two 120 Hz ticks make one 60 Hz hold: both steer, the third does not.
        let first = look.deflection(TICK, 5, false, RATE);
        let second = look.deflection(TICK, 5, false, RATE);
        assert!(first[1] < 0.0 && second[1] < 0.0, "mouse right yaws right (negative yaw)");
        assert!((first[1] - second[1]).abs() < 1e-5, "spent evenly across the hold");
        assert!(!look.is_pending(), "spent");
        assert_eq!(look.deflection(TICK, 5, false, RATE), [0.0, 0.0], "…and the ship stops turning");
    }

    #[test]
    fn the_turn_adds_up_to_the_sweep_times_the_sensitivity() {
        // 10 px at sensitivity 5 = 10 × 5 × RADIANS_PER_PIXEL radians of
        // yaw, delivered as rate × dt across the hold (a sweep small
        // enough to stay under the stick's full rate — a flick clamps).
        let mut look = MouseLook::new();
        look.push(10.0, 0.0);
        let mut turned = 0.0;
        for _ in 0..2 {
            let [_, yaw] = look.deflection(TICK, 5, false, RATE);
            turned += -yaw * RATE * TICK;
        }
        let expected = 10.0 * 5.0 * MouseLook::RADIANS_PER_PIXEL;
        assert!((turned - expected).abs() < 1e-5, "turned {turned}, expected {expected}");
    }

    #[test]
    fn sensitivity_scales_and_the_flick_clamps_at_the_full_rate() {
        let mut slow = MouseLook::new();
        slow.push(10.0, 0.0);
        let mut fast = MouseLook::new();
        fast.push(10.0, 0.0);
        let s = slow.deflection(TICK, 2, false, RATE)[1];
        let f = fast.deflection(TICK, 8, false, RATE)[1];
        assert!((f / s - 4.0).abs() < 1e-4, "sensitivity 8 turns four times sensitivity 2");
        let mut flick = MouseLook::new();
        flick.push(5000.0, -5000.0);
        let [pitch, yaw] = flick.deflection(TICK, 10, false, RATE);
        assert_eq!(yaw, -1.0, "a flick cannot out-turn the stick");
        assert_eq!(pitch, 1.0, "mouse up pitches up, at the full rate");
    }

    #[test]
    fn mouse_up_pitches_up_unless_inverted() {
        let mut look = MouseLook::new();
        look.push(0.0, -10.0);
        assert!(look.deflection(TICK, 5, false, RATE)[0] > 0.0, "mouse up = nose up");
        let mut inverted = MouseLook::new();
        inverted.push(0.0, -10.0);
        assert!(inverted.deflection(TICK, 5, true, RATE)[0] < 0.0, "inverted: mouse up = nose down");
    }

    #[test]
    fn motion_accumulates_within_a_frame() {
        let mut look = MouseLook::new();
        look.push(3.0, 0.0);
        look.push(4.0, 0.0);
        let mut single = MouseLook::new();
        single.push(7.0, 0.0);
        assert_eq!(
            look.deflection(TICK, 5, false, RATE),
            single.deflection(TICK, 5, false, RATE)
        );
    }
}
