//! Timed movement debuffs applied to the player.

/// A timed movement slow. While active it yields a movement multiplier < 1.0;
/// when it expires it returns to 1.0. Re-applying takes the *stronger* slow
/// (smaller multiplier) and refreshes the duration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlowDebuff {
    factor: f32,
    timer: f32,
}

impl SlowDebuff {
    pub fn new() -> Self {
        Self { factor: 1.0, timer: 0.0 }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Movement multiplier to apply to thrust this frame (1.0 when inactive).
    pub fn multiplier(&self) -> f32 {
        if self.is_active() { self.factor } else { 1.0 }
    }

    /// Apply a slow of `factor` (0..1) for `duration` seconds. Stacking takes
    /// the stronger factor and extends to the longer remaining duration.
    pub fn apply(&mut self, factor: f32, duration: f32) {
        let factor = factor.clamp(0.0, 1.0);
        self.factor = if self.is_active() { self.factor.min(factor) } else { factor };
        self.timer = self.timer.max(duration);
    }

    /// Advance time. Returns true if the active state changed this tick (so the
    /// UI indicator can be toggled only on transitions).
    pub fn tick(&mut self, delta: f32) -> bool {
        let was_active = self.is_active();
        if self.timer > 0.0 {
            self.timer = (self.timer - delta).max(0.0);
        }
        let now_active = self.is_active();
        if !now_active {
            self.factor = 1.0;
        }
        was_active != now_active
    }
}

impl Default for SlowDebuff {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_inactive_at_full_speed() {
        let d = SlowDebuff::new();
        assert!(!d.is_active());
        assert_eq!(d.multiplier(), 1.0);
    }

    #[test]
    fn apply_makes_it_active_and_slow() {
        let mut d = SlowDebuff::new();
        d.apply(0.5, 2.0);
        assert!(d.is_active());
        assert_eq!(d.multiplier(), 0.5);
    }

    #[test]
    fn expires_after_duration() {
        let mut d = SlowDebuff::new();
        d.apply(0.5, 1.0);
        d.tick(0.6);
        assert!(d.is_active());
        d.tick(0.6); // total 1.2 > 1.0
        assert!(!d.is_active());
        assert_eq!(d.multiplier(), 1.0);
    }

    #[test]
    fn stronger_slow_wins_while_active() {
        let mut d = SlowDebuff::new();
        d.apply(0.6, 2.0);
        d.apply(0.3, 1.0); // stronger (smaller) factor
        assert_eq!(d.multiplier(), 0.3);
    }

    #[test]
    fn weaker_slow_does_not_weaken_active_one() {
        let mut d = SlowDebuff::new();
        d.apply(0.3, 2.0);
        d.apply(0.8, 2.0); // weaker — must not raise the multiplier
        assert_eq!(d.multiplier(), 0.3);
    }

    #[test]
    fn reapply_extends_to_longer_duration() {
        let mut d = SlowDebuff::new();
        d.apply(0.5, 1.0);
        d.tick(0.9); // 0.1 left
        d.apply(0.5, 2.0); // refresh
        d.tick(1.5);
        assert!(d.is_active(), "duration should have been extended");
    }

    #[test]
    fn tick_reports_activation_transition() {
        let mut d = SlowDebuff::new();
        d.apply(0.5, 1.0);
        assert!(!d.tick(0.5), "still active — no transition");
        assert!(d.tick(0.6), "expired this tick — transition reported");
        assert!(!d.tick(0.6), "already inactive — no transition");
    }

    #[test]
    fn factor_clamped_to_unit_range() {
        let mut d = SlowDebuff::new();
        d.apply(1.5, 1.0);
        assert_eq!(d.multiplier(), 1.0);
    }
}
