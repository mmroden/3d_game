//! Per-level difficulty scaling for enemies.
//!
//! Enemies start sluggish and ramp up to their fastest tuning by [`PEAK_LEVEL`].
//! Only speed and fire rate scale — health does not, so a single shot still
//! kills an enemy no matter how fast and furious the late levels get.

/// Level at which enemies reach their peak speed/fire-rate.
pub const PEAK_LEVEL: u32 = 10;

// Movement speed multiplier: slow at level 1, a touch above baseline at peak.
const SPEED_AT_1: f32 = 0.6;
const SPEED_AT_PEAK: f32 = 1.15;

// Attack-cooldown multiplier: longer (slower fire) early, shorter (faster) late.
const COOLDOWN_AT_1: f32 = 1.4;
const COOLDOWN_AT_PEAK: f32 = 0.85;

/// Linear ramp from the level-1 value to the peak value, flat beyond peak.
fn ramp(level: u32, at_one: f32, at_peak: f32) -> f32 {
    let level = level.clamp(1, PEAK_LEVEL);
    let t = (level - 1) as f32 / (PEAK_LEVEL - 1) as f32;
    at_one + (at_peak - at_one) * t
}

/// Speed multiplier for enemies at the given level.
pub fn speed_multiplier(level: u32) -> f32 {
    ramp(level, SPEED_AT_1, SPEED_AT_PEAK)
}

/// Attack-cooldown multiplier (lower = faster fire) for enemies at the given level.
pub fn cooldown_multiplier(level: u32) -> f32 {
    ramp(level, COOLDOWN_AT_1, COOLDOWN_AT_PEAK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_1_is_slow() {
        assert_eq!(speed_multiplier(1), SPEED_AT_1);
        assert!(speed_multiplier(1) < 1.0, "early enemies move below baseline");
    }

    #[test]
    fn peak_level_is_a_touch_above_baseline() {
        assert_eq!(speed_multiplier(PEAK_LEVEL), SPEED_AT_PEAK);
        assert!(speed_multiplier(PEAK_LEVEL) > 1.0, "late enemies edge past baseline");
    }

    #[test]
    fn speed_rises_monotonically_with_level() {
        for w in (1..=PEAK_LEVEL).map(speed_multiplier).collect::<Vec<_>>().windows(2) {
            assert!(w[1] >= w[0], "speed must not drop as level rises");
        }
    }

    #[test]
    fn fire_gets_faster_with_level() {
        // Cooldown shrinks, so later enemies fire more often.
        assert!(cooldown_multiplier(1) > 1.0);
        assert!(cooldown_multiplier(PEAK_LEVEL) < 1.0);
        assert!(cooldown_multiplier(PEAK_LEVEL) < cooldown_multiplier(1));
    }

    #[test]
    fn scaling_flattens_beyond_peak() {
        assert_eq!(speed_multiplier(PEAK_LEVEL + 5), speed_multiplier(PEAK_LEVEL));
        assert_eq!(cooldown_multiplier(99), cooldown_multiplier(PEAK_LEVEL));
    }

    #[test]
    fn level_0_treated_as_level_1() {
        assert_eq!(speed_multiplier(0), speed_multiplier(1));
    }

    #[test]
    fn midpoint_is_between_endpoints() {
        let mid = speed_multiplier(5);
        assert!(mid > SPEED_AT_1 && mid < SPEED_AT_PEAK);
    }
}
