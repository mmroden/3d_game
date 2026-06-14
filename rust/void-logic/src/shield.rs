//! Shield state: regenerating energy barrier that absorbs damage before health.

use crate::newtypes::{Damage, Shield};
use serde::{Deserialize, Serialize};

/// Manages shield current level, regeneration, and boost state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShieldState {
    pub current: Shield,
    pub max_capacity: Shield,
    pub regen_rate: f32,
    pub regen_delay: f32,
    delay_timer: f32,
    boosted: bool,
}

impl ShieldState {
    pub fn new(max_capacity: Shield, regen_rate: f32, regen_delay: f32) -> Self {
        Self {
            current: max_capacity,
            max_capacity,
            regen_rate,
            regen_delay,
            delay_timer: 0.0,
            boosted: false,
        }
    }

    /// Absorb damage. Returns overflow that passes through to health.
    /// Resets the regen delay timer.
    pub fn take_hit(&mut self, damage: Damage) -> Damage {
        let (remaining, overflow) = self.current.absorb(damage);
        self.current = remaining;
        self.delay_timer = self.regen_delay;
        overflow
    }

    /// Advance regen timer. Regenerates shield after delay expires.
    pub fn tick(&mut self, delta: f32) {
        if self.delay_timer > 0.0 {
            self.delay_timer = (self.delay_timer - delta).max(0.0);
            return;
        }

        let rate = if self.boosted {
            self.regen_rate * 2.0
        } else {
            self.regen_rate
        };

        let new_val = (self.current.as_f32() + rate * delta).min(self.max_capacity.as_f32());
        self.current = Shield::new(new_val);
    }

    pub fn set_boosted(&mut self, active: bool) {
        self.boosted = active;
    }

    /// Reset to full capacity (e.g., on death penalty / new run).
    pub fn reset(&mut self) {
        self.current = self.max_capacity;
        self.delay_timer = 0.0;
        self.boosted = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_shield() -> ShieldState {
        ShieldState::new(Shield::new(50.0), 5.0, 1.5)
    }

    #[test]
    fn starts_at_full_capacity() {
        let state = default_shield();
        assert_eq!(state.current, Shield::new(50.0));
    }

    #[test]
    fn take_hit_absorbs_returns_zero_overflow() {
        let mut state = default_shield();
        let overflow = state.take_hit(Damage::new(30.0));
        assert_eq!(overflow, Damage::new(0.0));
        assert_eq!(state.current, Shield::new(20.0));
    }

    #[test]
    fn take_hit_overflow_passes_through() {
        let mut state = default_shield();
        let overflow = state.take_hit(Damage::new(70.0));
        assert_eq!(overflow, Damage::new(20.0));
        assert_eq!(state.current, Shield::new(0.0));
    }

    #[test]
    fn no_regen_during_delay() {
        let mut state = default_shield();
        state.take_hit(Damage::new(20.0));
        state.tick(1.0); // 1.0 < 1.5 delay
        assert_eq!(state.current, Shield::new(30.0));
    }

    #[test]
    fn regen_starts_after_delay() {
        let mut state = default_shield();
        state.take_hit(Damage::new(20.0)); // at 30
        state.tick(1.5); // delay expires
        state.tick(1.0); // 1 second of regen at 5/sec
        assert_eq!(state.current, Shield::new(35.0));
    }

    #[test]
    fn regen_caps_at_max() {
        let mut state = default_shield();
        state.take_hit(Damage::new(10.0)); // at 40
        state.tick(1.5);
        state.tick(100.0);
        assert_eq!(state.current, Shield::new(50.0));
    }

    #[test]
    fn take_hit_resets_delay() {
        let mut state = default_shield();
        state.tick(2.0); // delay would have expired
        state.take_hit(Damage::new(10.0));
        state.tick(1.0); // within new delay
        assert_eq!(state.current, Shield::new(40.0));
    }

    #[test]
    fn boosted_regen_is_double() {
        let mut state = default_shield();
        state.take_hit(Damage::new(30.0)); // at 20
        state.set_boosted(true);
        state.tick(1.5); // delay
        state.tick(1.0); // 10/sec boosted
        assert_eq!(state.current, Shield::new(30.0));
    }

    #[test]
    fn unboosted_returns_to_normal() {
        let mut state = default_shield();
        state.take_hit(Damage::new(40.0)); // at 10
        state.set_boosted(true);
        state.tick(1.5);
        state.tick(1.0); // 10/sec → 20
        state.set_boosted(false);
        state.tick(1.0); // 5/sec → 25
        assert_eq!(state.current, Shield::new(25.0));
    }

    #[test]
    fn reset_restores_full() {
        let mut state = default_shield();
        state.take_hit(Damage::new(40.0));
        state.reset();
        assert_eq!(state.current, Shield::new(50.0));
    }
}
