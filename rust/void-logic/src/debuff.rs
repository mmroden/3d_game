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

/// A latch drain: while the BossLatcher holds contact it siphons hull at a
/// fixed rate, but damage crosses to `RunState::take_damage` only in whole
/// points — the fractional remainder accrues here so the total over any
/// stretch of contact is exactly `floor(dps × seconds)` regardless of how
/// the physics ticks slice it (no float HP drift).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrainDebuff {
    dps: f32,
    /// Fractional damage accrued but not yet emitted. f64: the exactness
    /// contract must survive thousands of 8ms ticks without drift.
    accumulated: f64,
}

impl DrainDebuff {
    pub fn new(dps: f32) -> Self {
        Self { dps, accumulated: 0.0 }
    }

    /// Advance contact time; returns the whole damage points to apply now.
    pub fn tick(&mut self, delta: f32) -> u32 {
        self.accumulated += f64::from(self.dps) * f64::from(delta.max(0.0));
        let whole = self.accumulated.floor();
        self.accumulated -= whole;
        whole as u32
    }

    /// Contact broke: forfeit the fractional remainder so re-latching never
    /// banks damage from a previous grab.
    pub fn reset(&mut self) {
        self.accumulated = 0.0;
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

    // --- DrainDebuff (B5) ---

    #[test]
    fn drain_emits_floor_of_dps_times_time() {
        let mut d = DrainDebuff::new(7.5);
        assert_eq!(d.tick(2.0), 15);
    }

    #[test]
    fn drain_holds_fractions_until_a_whole_point_accrues() {
        let mut d = DrainDebuff::new(2.0);
        assert_eq!(d.tick(0.4), 0, "0.8 accrued — nothing whole yet");
        assert_eq!(d.tick(0.2), 1, "1.2 accrued — one point crosses");
    }

    #[test]
    fn drain_total_is_split_invariant() {
        // The same 10 seconds of contact must cost the same hull whether the
        // physics loop slices it into 8ms ticks or hands it over whole.
        // 3.25 is binary-exact and 32.5 sits far from an integer boundary,
        // so input representation error can never flip the floor.
        let mut fine = DrainDebuff::new(3.25);
        let mut fine_total: u32 = 0;
        let steps = 1250; // 1250 × 8ms = 10s
        for _ in 0..steps {
            fine_total += fine.tick(0.008);
        }
        let mut coarse = DrainDebuff::new(3.25);
        let coarse_total = coarse.tick(10.0);
        assert_eq!(fine_total, coarse_total, "tick slicing must not change cost");
        assert_eq!(coarse_total, 32, "floor(3.25 × 10)");
    }

    #[test]
    fn drain_reset_forfeits_the_fraction() {
        let mut d = DrainDebuff::new(2.0);
        assert_eq!(d.tick(0.4), 0); // 0.8 banked
        d.reset();
        assert_eq!(d.tick(0.4), 0, "re-latch starts from zero, not 1.6");
    }
}
