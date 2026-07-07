//! Screen damage feedback: a decaying full-screen tint that flashes amber
//! when the shield absorbs a hit and red when the hull is breached
//! (playtest 2026-07-06: "briefly flash a yellow or red tinge... a deep
//! tinge that decays over five seconds or so"). Pure decay + color policy;
//! the HUD holds one, ticks it each frame, and renders the overlay.

use crate::run_state::DamageOutcome;

/// Seconds a flash takes to fade from full to nothing (owner: "decays over
/// five seconds or so").
pub const FLASH_DECAY_SECONDS: f32 = 5.0;
/// Peak tint opacity — a deep tinge, never a solid wash of the screen.
pub const FLASH_PEAK_ALPHA: f32 = 0.35;

/// The live damage-tint state: a normalized intensity that decays to zero,
/// plus which layer the latest strike hit (picks amber vs red).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageFlash {
    /// Remaining intensity in 0..=1; `alpha()` scales it by the peak.
    intensity: f32,
    /// The layer the latest strike hit — true = hull breach (red).
    hull: bool,
}

impl Default for DamageFlash {
    fn default() -> Self {
        Self::new()
    }
}

impl DamageFlash {
    pub fn new() -> Self {
        Self { intensity: 0.0, hull: false }
    }

    /// A hit landed: relight to full at the color of the layer it struck.
    pub fn strike(&mut self, outcome: DamageOutcome) {
        self.intensity = 1.0;
        self.hull = matches!(outcome, DamageOutcome::HullHit);
    }

    /// Fade the flash toward nothing — linear over `FLASH_DECAY_SECONDS`.
    pub fn tick(&mut self, delta: f32) {
        if FLASH_DECAY_SECONDS <= 0.0 {
            self.intensity = 0.0;
            return;
        }
        self.intensity = (self.intensity - delta / FLASH_DECAY_SECONDS).max(0.0);
    }

    /// Overlay opacity right now (0 = invisible).
    pub fn alpha(&self) -> f32 {
        self.intensity * FLASH_PEAK_ALPHA
    }

    /// Whether the overlay should draw at all.
    pub fn is_visible(&self) -> bool {
        self.intensity > 0.0
    }

    /// The tint RGB: red for a hull breach, amber for a held shield.
    pub fn color(&self) -> [f32; 3] {
        if self.hull {
            [0.9, 0.15, 0.1]
        } else {
            [0.95, 0.75, 0.1]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_flash_is_invisible() {
        let f = DamageFlash::new();
        assert_eq!(f.alpha(), 0.0);
        assert!(!f.is_visible());
    }

    #[test]
    fn a_hull_hit_flashes_red_at_full() {
        let mut f = DamageFlash::new();
        f.strike(DamageOutcome::HullHit);
        assert!(f.is_visible());
        assert!((f.alpha() - FLASH_PEAK_ALPHA).abs() < 1e-6, "a fresh strike is at peak");
        let c = f.color();
        assert!(c[0] > c[1] && c[0] > c[2], "a hull breach reads red: {c:?}");
    }

    #[test]
    fn a_held_shield_flashes_amber_not_red() {
        let mut f = DamageFlash::new();
        f.strike(DamageOutcome::ShieldHeld);
        assert!(f.is_visible());
        let c = f.color();
        assert!(c[1] > 0.5, "amber carries a strong green channel: {c:?}");
        assert!(c[2] < c[0] && c[2] < c[1], "amber, not white or blue: {c:?}");
    }

    #[test]
    fn the_flash_decays_to_nothing_over_five_seconds() {
        let mut f = DamageFlash::new();
        f.strike(DamageOutcome::HullHit);
        f.tick(FLASH_DECAY_SECONDS * 0.5);
        assert!((f.alpha() - FLASH_PEAK_ALPHA * 0.5).abs() < 1e-4,
            "half the decay window leaves half the tint (got {})", f.alpha());
        f.tick(FLASH_DECAY_SECONDS); // well past the end
        assert_eq!(f.alpha(), 0.0, "the flash fades to nothing");
        assert!(!f.is_visible());
    }

    #[test]
    fn a_new_hit_relights_the_flash() {
        let mut f = DamageFlash::new();
        f.strike(DamageOutcome::HullHit);
        f.tick(FLASH_DECAY_SECONDS * 0.9); // nearly faded
        f.strike(DamageOutcome::ShieldHeld); // fresh hit
        assert!((f.alpha() - FLASH_PEAK_ALPHA).abs() < 1e-6, "the new hit relights to full");
        assert!(f.color()[1] > 0.5, "and takes the new hit's color (amber)");
    }
}
