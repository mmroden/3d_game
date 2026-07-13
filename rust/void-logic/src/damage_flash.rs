//! Screen damage feedback: the HULL-SEVERITY alarm (owner 2026-07-09).
//! The tint fires only when the hull actually DIMINISHES from a hit —
//! projectile or collision; a held shield raises no alarm — and its color
//! is the health bar's own post-hit zone: nothing while the bar is (or
//! would stay) green, yellow when it is-or-goes yellow, red when it
//! is-or-goes red. Bar and tint derive from ONE scale
//! ([`HealthZone`](crate::run_state::HealthZone)), so they can never
//! disagree. Pure decay + zone policy; the HUD holds one, ticks it each
//! frame, and renders the overlay.

use crate::run_state::HealthZone;

/// Seconds a flash takes to fade from full to nothing (owner: "decays over
/// five seconds or so").
pub const FLASH_DECAY_SECONDS: f32 = 5.0;
/// Peak tint opacity — a deep tinge, never a solid wash of the screen.
pub const FLASH_PEAK_ALPHA: f32 = 0.35;

/// The live damage-tint state: a normalized intensity that decays to zero,
/// plus the bar zone of the latest breach (picks yellow vs red).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageFlash {
    /// Remaining intensity in 0..=1; `alpha()` scales it by the peak.
    intensity: f32,
    /// The bar zone the latest breach left the hull in.
    zone: HealthZone,
}

impl Default for DamageFlash {
    fn default() -> Self {
        Self::new()
    }
}

impl DamageFlash {
    pub fn new() -> Self {
        Self { intensity: 0.0, zone: HealthZone::Green }
    }

    /// A hull breach landed, leaving the bar in `zone` (the POST-hit zone —
    /// "is or would be"). Green raises no alarm; below green every breach
    /// relights at the bar's color. Held-shield hits never reach here: the
    /// hull did not diminish, so there is nothing to announce.
    pub fn strike_hull(&mut self, zone: HealthZone) {
        if zone == HealthZone::Green {
            return;
        }
        self.intensity = 1.0;
        self.zone = zone;
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

    /// The tint RGB — the health bar's zone color family.
    pub fn color(&self) -> [f32; 3] {
        match self.zone {
            HealthZone::Green => [0.0, 0.0, 0.0], // unreachable while visible
            HealthZone::Yellow => [0.95, 0.85, 0.1],
            HealthZone::Red => [0.9, 0.15, 0.1],
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
    fn a_green_zone_breach_raises_no_alarm() {
        // The tint mirrors the health bar (owner 2026-07-09): while the bar
        // is green there is no alarm to raise.
        let mut f = DamageFlash::new();
        f.strike_hull(HealthZone::Green);
        assert!(!f.is_visible(), "green bar, no tint");
    }

    #[test]
    fn a_yellow_zone_breach_tints_yellow() {
        let mut f = DamageFlash::new();
        f.strike_hull(HealthZone::Yellow);
        assert!((f.alpha() - FLASH_PEAK_ALPHA).abs() < 1e-6, "a breach lights at peak");
        let c = f.color();
        assert!(c[1] > 0.5, "yellow bar, yellow tint (strong green channel): {c:?}");
        assert!(c[2] < c[1], "yellow, not white: {c:?}");
    }

    #[test]
    fn a_red_zone_breach_tints_red() {
        let mut f = DamageFlash::new();
        f.strike_hull(HealthZone::Red);
        assert!((f.alpha() - FLASH_PEAK_ALPHA).abs() < 1e-6, "a breach lights at peak");
        let c = f.color();
        assert!(c[0] > c[1] && c[0] > c[2], "red bar, red tint: {c:?}");
    }

    #[test]
    fn breaches_relight_every_time_at_the_bars_current_zone() {
        // Below green every breach matters — each relights, and the color
        // tracks the bar as it worsens.
        let mut f = DamageFlash::new();
        f.strike_hull(HealthZone::Yellow);
        f.tick(FLASH_DECAY_SECONDS * 0.5);
        f.strike_hull(HealthZone::Yellow);
        assert!((f.alpha() - FLASH_PEAK_ALPHA).abs() < 1e-6, "every breach relights");
        f.strike_hull(HealthZone::Red);
        let c = f.color();
        assert!(c[0] > c[1], "the tint worsens with the bar: {c:?}");
    }

    #[test]
    fn the_flash_decays_to_nothing_over_five_seconds() {
        let mut f = DamageFlash::new();
        f.strike_hull(HealthZone::Red);
        f.tick(FLASH_DECAY_SECONDS * 0.5);
        assert!((f.alpha() - FLASH_PEAK_ALPHA * 0.5).abs() < 1e-4,
            "half the decay window leaves half the tint (got {})", f.alpha());
        f.tick(FLASH_DECAY_SECONDS); // well past the end
        assert_eq!(f.alpha(), 0.0, "the flash fades to nothing");
        assert!(!f.is_visible());
    }
}
