//! Timed movement debuffs applied to the player.

/// The hardest the player can be slowed. Repeated hits compound toward this
/// floor but never fully freeze movement, so the player can always crawl free.
const MIN_FACTOR: f32 = 0.1;

/// A timed movement slow. While active it yields a movement multiplier < 1.0;
/// when it expires it returns to 1.0. Re-applying *compounds*: each hit
/// multiplies the current slow, so a swarmer that keeps tagging the player
/// drives speed down toward [`MIN_FACTOR`] until the player breaks contact.
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

    /// Apply one slow "hit": `per_hit` (0..1) compounds onto the current slow
    /// (multiplying it, floored at [`MIN_FACTOR`]) and refreshes the window to at
    /// least `duration` seconds. A fresh hit on an inactive debuff starts from
    /// full speed, so the first tag is just `per_hit`; subsequent tags stack.
    pub fn apply(&mut self, per_hit: f32, duration: f32) {
        let per_hit = per_hit.clamp(0.0, 1.0);
        let base = if self.is_active() { self.factor } else { 1.0 };
        self.factor = (base * per_hit).max(MIN_FACTOR);
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
    fn repeated_hits_compound_the_slow() {
        let mut d = SlowDebuff::new();
        d.apply(0.7, 2.0);
        assert!((d.multiplier() - 0.7).abs() < 1e-6, "first tag is just per_hit");
        d.apply(0.7, 2.0);
        assert!((d.multiplier() - 0.49).abs() < 1e-6, "second tag compounds: 0.7*0.7");
        d.apply(0.7, 2.0);
        assert!((d.multiplier() - 0.343).abs() < 1e-6, "third tag: 0.7^3");
    }

    #[test]
    fn compounding_is_clamped_to_a_floor() {
        let mut d = SlowDebuff::new();
        for _ in 0..50 {
            d.apply(0.7, 2.0);
        }
        assert_eq!(d.multiplier(), 0.1, "never slower than the floor, never frozen");
    }

    #[test]
    fn expiry_resets_so_the_next_grab_starts_fresh() {
        let mut d = SlowDebuff::new();
        d.apply(0.7, 1.0);
        d.apply(0.7, 1.0); // compounded to 0.49
        d.tick(1.1); // expire
        assert_eq!(d.multiplier(), 1.0);
        d.apply(0.7, 1.0); // a brand-new grab starts from full speed
        assert!((d.multiplier() - 0.7).abs() < 1e-6, "stacks don't survive expiry");
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
