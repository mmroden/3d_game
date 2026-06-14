//! Domain-specific newtypes that prevent mixing semantically distinct values.
//! Health cannot be confused with Damage, Score cannot be confused with Level, etc.

use serde::{Deserialize, Serialize};

/// Hit points remaining on an entity.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Health(f32);

/// Damage dealt to an entity.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Damage(f32);

impl Health {
    pub const fn new(value: f32) -> Self {
        Self(value)
    }

    pub const fn as_f32(self) -> f32 {
        self.0
    }

    pub fn is_alive(self) -> bool {
        self.0 > 0.0
    }

    /// Subtract damage, flooring at zero.
    pub fn take(self, dmg: Damage) -> Self {
        Self((self.0 - dmg.0).max(0.0))
    }

    /// Fraction remaining (for health bars): self / max, clamped to [0, 1].
    pub fn fraction(self, max: Health) -> f32 {
        (self.0 / max.0).clamp(0.0, 1.0)
    }
}

/// Shield energy on an entity. Absorbs damage before health.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Shield(f32);

impl Shield {
    pub const fn new(value: f32) -> Self {
        Self(value)
    }

    pub const fn as_f32(self) -> f32 {
        self.0
    }

    /// Absorb damage. Returns (remaining shield, overflow damage that passes to health).
    pub fn absorb(self, dmg: Damage) -> (Shield, Damage) {
        if self.0 >= dmg.0 {
            (Shield(self.0 - dmg.0), Damage(0.0))
        } else {
            (Shield(0.0), Damage(dmg.0 - self.0))
        }
    }

    /// Fraction remaining: self / max, clamped to [0, 1].
    pub fn fraction(self, max: Shield) -> f32 {
        (self.0 / max.0).clamp(0.0, 1.0)
    }
}

impl Damage {
    pub const fn new(value: f32) -> Self {
        Self(value)
    }

    pub const fn as_f32(self) -> f32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Health tests ---

    #[test]
    fn health_is_alive_when_positive() {
        assert!(Health::new(1.0).is_alive());
        assert!(Health::new(100.0).is_alive());
    }

    #[test]
    fn health_is_dead_at_zero() {
        assert!(!Health::new(0.0).is_alive());
    }

    #[test]
    fn health_is_dead_when_negative() {
        assert!(!Health::new(-5.0).is_alive());
    }

    #[test]
    fn health_take_damage_reduces() {
        let hp = Health::new(10.0).take(Damage::new(3.0));
        assert_eq!(hp.as_f32(), 7.0);
    }

    #[test]
    fn health_take_damage_floors_at_zero() {
        let hp = Health::new(5.0).take(Damage::new(99.0));
        assert_eq!(hp.as_f32(), 0.0);
    }

    #[test]
    fn health_fraction_full() {
        let f = Health::new(100.0).fraction(Health::new(100.0));
        assert!((f - 1.0).abs() < 0.001);
    }

    #[test]
    fn health_fraction_half() {
        let f = Health::new(50.0).fraction(Health::new(100.0));
        assert!((f - 0.5).abs() < 0.001);
    }

    #[test]
    fn health_fraction_clamps_to_one() {
        let f = Health::new(150.0).fraction(Health::new(100.0));
        assert!((f - 1.0).abs() < 0.001);
    }

    #[test]
    fn health_roundtrips_through_f32() {
        assert_eq!(Health::new(42.5).as_f32(), 42.5);
    }

    // --- Damage tests ---

    #[test]
    fn damage_roundtrips_through_f32() {
        assert_eq!(Damage::new(7.0).as_f32(), 7.0);
    }

    // --- Shield tests ---

    #[test]
    fn shield_absorbs_fully_when_sufficient() {
        let (remaining, overflow) = Shield::new(50.0).absorb(Damage::new(30.0));
        assert_eq!(remaining, Shield::new(20.0));
        assert_eq!(overflow, Damage::new(0.0));
    }

    #[test]
    fn shield_overflow_passes_to_health() {
        let (remaining, overflow) = Shield::new(20.0).absorb(Damage::new(35.0));
        assert_eq!(remaining, Shield::new(0.0));
        assert_eq!(overflow, Damage::new(15.0));
    }

    #[test]
    fn shield_exact_depletion_zero_overflow() {
        let (remaining, overflow) = Shield::new(25.0).absorb(Damage::new(25.0));
        assert_eq!(remaining, Shield::new(0.0));
        assert_eq!(overflow, Damage::new(0.0));
    }

    #[test]
    fn empty_shield_passes_all_through() {
        let (remaining, overflow) = Shield::new(0.0).absorb(Damage::new(10.0));
        assert_eq!(remaining, Shield::new(0.0));
        assert_eq!(overflow, Damage::new(10.0));
    }

    #[test]
    fn shield_fraction_half() {
        let f = Shield::new(30.0).fraction(Shield::new(100.0));
        assert!((f - 0.3).abs() < 0.001);
    }

    #[test]
    fn health_and_damage_are_distinct_types() {
        // This test documents the type safety: Health(10.0) and Damage(10.0)
        // are not interchangeable at compile time. The test just verifies
        // they can coexist without confusion.
        let hp = Health::new(10.0);
        let dmg = Damage::new(10.0);
        assert_eq!(hp.as_f32(), dmg.as_f32());
        // But you can't do: hp.take(hp) — won't compile!
        // And you can't do: dmg.take(dmg) — Damage has no take method!
    }
}
