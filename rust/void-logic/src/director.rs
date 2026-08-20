//! The stereo DIRECTOR — cinema's model in code (experiment v2, owner's
//! design 2026-08-19; see docs in the plan memory).
//!
//! A stereographer parks the zero-parallax plane on the intended subject
//! and chooses the camera baseline per shot; convergence is a
//! subconscious gaze-directing signal, and baseline (interaxial) is the
//! depth-volume dial — pop at any distance is manufactured by widening
//! it (beam-splitter dialogue rigs run ~20–40 mm, stadium hyperstereo
//! runs far past anatomy). This module makes both continuous:
//!
//!   * ONE forward cone is the stage; candidates inside it compete on a
//!     ranked ladder — nearest visible THREAT, else nearest free
//!     FLOATER (caches, portal), else the WALL the aim ray strikes.
//!     Outside the cone: no influence, so peripheral objects sit
//!     off-plane and POP — an intentional attention channel.
//!   * Convergence tracks the subject's depth. There is no visible
//!     sight (owner removed the reticle, 2026-08-19): the converged
//!     plane itself is the "this is the subject" signal.
//!   * Interaxial keeps the subject "as spherical as possible":
//!     s = d / K_SPHERICITY (constant roundness — the cinema 1/N rule
//!     made continuous), clamped to the owner's band, with a total
//!     vergence cap resolved by NARROWING s — the subject's distance is
//!     truth, baseline is the free variable.
//!   * Both dials are eased, and subject loss is bridged by a grace
//!     hold so LOS blips and dying enemies never ping-pong the plane.
//!
//! Everything here is pure: the node side harvests candidates and feeds
//! `StereoDirector::tick`; ViewManager pours the result into
//! `StereoConfig` (frustum shift only — never camera rotation).

use crate::stereo::{exp_approach, CONVERGENCE_FAR, CONVERGENCE_NEAR};

/// Sphericity: interaxial = subject distance / this. 100 is the cinema
/// screen-era rule of thumb (1/100 of subject distance); the primary
/// in-glasses tuning dial.
pub const K_SPHERICITY: f32 = 100.0;
/// The owner's interaxial band (meters): hypostereo floor for close
/// work, hyperstereo ceiling below the cockpit-doubling limit (~125 mm =
/// 2x the proven VR-cockpit separation; see the plan memory's math).
pub const INTERAXIAL_MIN: f32 = 0.020;
pub const INTERAXIAL_MAX: f32 = 0.125;
/// Total vergence cap (degrees): the anti-cross-eye guard. Within the
/// owner's bounds it is DEFENSE-IN-DEPTH — s ≤ 125 mm converged no
/// nearer than CONVERGENCE_NEAR peaks at ~6° — but it hard-stops any
/// future widening of either band from going cross-eyed.
pub const MAX_VERGENCE_DEG: f32 = 10.0;
/// The candidate stage: half-angle of the forward cone (degrees). The
/// node side filters with this; it lives here as the one truth.
pub const CONE_HALF_ANGLE_DEG: f32 = 12.0;
/// Easing rates (1/s), deliberately gentle — the literature's discomfort
/// driver is the RATE of vergence change.
pub const CONVERGENCE_RATE: f32 = 3.0;
pub const INTERAXIAL_RATE: f32 = 2.0;
/// How long a vanished subject's depth is held before the ladder falls
/// through to the next rung (seconds) — LOS blips and kills inside this
/// window never move the plane.
pub const SUBJECT_GRACE: f32 = 0.4;

/// Nearest-in-cone candidate depths, harvested by the node side each
/// physics tick. `None` = that rung is empty this tick.
#[derive(Debug, Clone, Copy, Default)]
pub struct Candidates {
    /// Nearest visible (LOS-checked) enemy in the cone.
    pub threat: Option<f32>,
    /// Nearest free-floating object (cache, portal) in the cone.
    pub floater: Option<f32>,
    /// The aim ray's wall hit.
    pub wall: Option<f32>,
}

/// Subject rung; declaration order IS priority (lower = better), which
/// the derived `Ord` encodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rung {
    Threat,
    Floater,
    Wall,
}

/// The eased two-dial state. Construct once, `tick` every physics frame.
#[derive(Debug, Clone)]
pub struct StereoDirector {
    convergence: f32,
    interaxial: f32,
    /// The subject currently holding the plane, and the grace clock that
    /// keeps it through blips.
    held: Option<(Rung, f32)>,
    grace_left: f32,
}

impl Default for StereoDirector {
    fn default() -> Self {
        Self {
            convergence: CONVERGENCE_FAR,
            interaxial: (CONVERGENCE_FAR / K_SPHERICITY)
                .clamp(INTERAXIAL_MIN, INTERAXIAL_MAX),
            held: None,
            grace_left: 0.0,
        }
    }
}

impl StereoDirector {
    /// The ladder: highest occupied rung wins.
    pub fn choose(c: &Candidates) -> Option<(Rung, f32)> {
        c.threat
            .map(|d| (Rung::Threat, d))
            .or(c.floater.map(|d| (Rung::Floater, d)))
            .or(c.wall.map(|d| (Rung::Wall, d)))
    }

    /// One physics tick: pick the subject (with grace bridging), ease
    /// convergence toward its depth (clamped to the travel band) and
    /// interaxial toward the sphericity rule (clamped and vergence-capped
    /// against the ACTUAL rendered convergence). Returns
    /// `(convergence_distance, eye_separation)` for StereoConfig.
    pub fn tick(&mut self, c: &Candidates, dt: f32) -> (f32, f32) {
        // Subject selection: a same-or-better rung captures the plane
        // immediately and keeps the grace topped up; when the held rung
        // vanishes (only worse rungs or nothing remain), its depth
        // bridges the grace window before the ladder falls through.
        let fresh = Self::choose(c);
        self.held = match (fresh, self.held) {
            (Some((rung, depth)), Some((held_rung, _))) if rung <= held_rung => {
                self.grace_left = SUBJECT_GRACE;
                Some((rung, depth))
            }
            (fresh_any, Some(held)) => {
                self.grace_left -= dt;
                if self.grace_left > 0.0 {
                    Some(held)
                } else {
                    self.grace_left = SUBJECT_GRACE;
                    fresh_any
                }
            }
            (fresh_any, None) => {
                self.grace_left = SUBJECT_GRACE;
                fresh_any
            }
        };

        // Convergence: the subject's depth clamped to the travel band;
        // an empty stage rests deep.
        let d_target = self
            .held
            .map(|(_, depth)| depth.clamp(CONVERGENCE_NEAR, CONVERGENCE_FAR))
            .unwrap_or(CONVERGENCE_FAR);
        self.convergence = exp_approach(self.convergence, d_target, CONVERGENCE_RATE, dt);

        // Interaxial: sphericity for where the plane is HEADED, eased,
        // then capped against where the plane actually IS this frame —
        // the rendered pair never exceeds the vergence cap, even
        // mid-transient.
        let s_target = interaxial_for(d_target);
        self.interaxial = exp_approach(self.interaxial, s_target, INTERAXIAL_RATE, dt);
        self.interaxial = vergence_capped(self.interaxial, self.convergence);
        (self.convergence, self.interaxial)
    }

    pub fn convergence(&self) -> f32 {
        self.convergence
    }

    pub fn interaxial(&self) -> f32 {
        self.interaxial
    }
}

/// The sphericity rule: interaxial proportional to subject distance,
/// clamped to the owner's band.
pub fn interaxial_for(subject_distance: f32) -> f32 {
    (subject_distance / K_SPHERICITY).clamp(INTERAXIAL_MIN, INTERAXIAL_MAX)
}

/// Total vergence angle (degrees) of a baseline `s` converged at `d`.
pub fn vergence_deg(s: f32, d: f32) -> f32 {
    (2.0 * (s / (2.0 * d.max(f32::EPSILON))).atan()).to_degrees()
}

/// The transient guard: the widest baseline whose vergence at `d` stays
/// under the cap — never moves convergence, only narrows s.
pub fn vergence_capped(s: f32, d: f32) -> f32 {
    s.min(2.0 * d * (MAX_VERGENCE_DEG / 2.0).to_radians().tan())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn threats(d: f32) -> Candidates {
        Candidates { threat: Some(d), floater: Some(d + 5.0), wall: Some(d + 10.0) }
    }

    #[test]
    fn ladder_prefers_threat_then_floater_then_wall() {
        let all = Candidates { threat: Some(6.0), floater: Some(3.0), wall: Some(2.0) };
        assert_eq!(StereoDirector::choose(&all), Some((Rung::Threat, 6.0)));
        let no_threat = Candidates { threat: None, floater: Some(3.0), wall: Some(2.0) };
        assert_eq!(StereoDirector::choose(&no_threat), Some((Rung::Floater, 3.0)));
        let wall_only = Candidates { threat: None, floater: None, wall: Some(2.0) };
        assert_eq!(StereoDirector::choose(&wall_only), Some((Rung::Wall, 2.0)));
        assert_eq!(StereoDirector::choose(&Candidates::default()), None);
    }

    #[test]
    fn interaxial_is_proportional_between_the_clamps() {
        let d = 6.0;
        assert!((interaxial_for(d) - d / K_SPHERICITY).abs() < 1e-6);
    }

    #[test]
    fn interaxial_clamps_to_the_owners_band() {
        assert_eq!(interaxial_for(0.5), INTERAXIAL_MIN);
        assert_eq!(interaxial_for(400.0), INTERAXIAL_MAX);
    }

    #[test]
    fn proportional_tracking_holds_vergence_constant_and_far_under_cap() {
        // s = d/K gives a constant vergence angle (~0.57 deg at K=100):
        // the cap is a transient guard, never the steady-state governor.
        let baseline = vergence_deg(interaxial_for(4.0), 4.0);
        for d in [2.5_f32, 5.0, 8.0, 12.0] {
            let v = vergence_deg(interaxial_for(d), d);
            assert!((v - baseline).abs() < 0.05, "vergence drifts: {v} vs {baseline}");
            assert!(v < MAX_VERGENCE_DEG / 4.0);
        }
    }

    #[test]
    fn within_the_owners_bounds_the_cap_never_binds() {
        // Defense-in-depth fact: 125 mm converged at the near clamp peaks
        // near 6 deg — the 10 deg cap only matters if a band widens later.
        assert!(vergence_deg(INTERAXIAL_MAX, crate::stereo::CONVERGENCE_NEAR)
            < MAX_VERGENCE_DEG);
        assert_eq!(
            vergence_capped(INTERAXIAL_MAX, crate::stereo::CONVERGENCE_NEAR),
            INTERAXIAL_MAX
        );
    }

    #[test]
    fn the_cap_narrows_wide_baselines_on_near_subjects() {
        // Below today's near clamp (a future band widening, or a transient
        // before the convergence clamp catches up): the cap must bind and
        // resolve by narrowing s, never by moving d.
        let (s, d) = (INTERAXIAL_MAX, 0.5);
        assert!(vergence_deg(s, d) > MAX_VERGENCE_DEG);
        let capped = vergence_capped(s, d);
        assert!(capped < s);
        assert!(vergence_deg(capped, d) <= MAX_VERGENCE_DEG + 1e-3);
        // And a compliant pair passes through untouched.
        assert_eq!(vergence_capped(0.05, 6.0), 0.05);
    }

    #[test]
    fn tick_eases_toward_the_subject_without_overshoot() {
        let mut dir = StereoDirector::default();
        let start = dir.convergence();
        let (d1, _) = dir.tick(&threats(3.0), 0.05);
        assert!(d1 < start && d1 > 3.0, "one step lands between: {d1}");
        for _ in 0..400 {
            dir.tick(&threats(3.0), 1.0 / 60.0);
        }
        assert!((dir.convergence() - 3.0).abs() < 0.05);
        assert!((dir.interaxial() - interaxial_for(3.0)).abs() < 0.002);
    }

    #[test]
    fn grace_bridges_subject_blips() {
        let mut dir = StereoDirector::default();
        for _ in 0..400 {
            dir.tick(&threats(3.0), 1.0 / 60.0);
        }
        // Threat blinks out for less than the grace window: the plane
        // holds its depth instead of falling to the wall rung.
        let wall_only = Candidates { threat: None, floater: None, wall: Some(9.0) };
        for _ in 0..6 {
            dir.tick(&wall_only, 0.05);
        }
        assert!((dir.convergence() - 3.0).abs() < 0.3, "plane held through the blip");
        // Beyond grace, the ladder falls through and converges on the wall.
        for _ in 0..600 {
            dir.tick(&wall_only, 1.0 / 60.0);
        }
        assert!((dir.convergence() - 9.0).abs() < 0.1);
    }

    #[test]
    fn upgrades_are_immediate_no_grace_for_better_subjects() {
        let mut dir = StereoDirector::default();
        let wall_only = Candidates { threat: None, floater: None, wall: Some(9.0) };
        for _ in 0..400 {
            dir.tick(&wall_only, 1.0 / 60.0);
        }
        // A threat appears: the very next tick must start moving toward it.
        let before = dir.convergence();
        dir.tick(&threats(3.0), 1.0 / 60.0);
        assert!(dir.convergence() < before, "upgrade captures the plane immediately");
    }

    #[test]
    fn empty_stage_rests_deep() {
        let mut dir = StereoDirector::default();
        for _ in 0..400 {
            dir.tick(&threats(3.0), 1.0 / 60.0);
        }
        for _ in 0..2000 {
            dir.tick(&Candidates::default(), 1.0 / 60.0);
        }
        assert!((dir.convergence() - CONVERGENCE_FAR).abs() < 0.1);
    }

    #[test]
    fn convergence_clamps_to_the_travel_band() {
        let mut dir = StereoDirector::default();
        let point_blank = Candidates { threat: Some(0.3), floater: None, wall: None };
        for _ in 0..600 {
            dir.tick(&point_blank, 1.0 / 60.0);
        }
        assert!(dir.convergence() >= CONVERGENCE_NEAR - 1e-3);
    }
}
