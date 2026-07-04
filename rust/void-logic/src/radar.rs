//! Off-screen enemy indicator math (the Enemy Radar unlock).
//!
//! Pure screen-space geometry: given where an enemy projects on screen (and
//! whether it is behind the camera), place an edge arrow inside the HUD's
//! safe band pointing toward it — or nothing, when the enemy is visibly on
//! screen. The shell feeds `Camera3D::unproject_position` in and positions
//! preallocated arrow polygons from the result; every placement decision
//! lives here where it is testable.
//!
//! Coordinates are screen-space: +x right, +y DOWN (Godot 2D), in the band's
//! own pixel units with the origin at the band's top-left.

/// The screen region arrows may occupy — the HUD safe band (in SBS the
/// central column both eyes see; full-window in mono).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandRect {
    pub width: f32,
    pub height: f32,
}

/// Distance arrows keep from the band's border, so a triangle never clips.
pub const ARROW_INSET: f32 = 24.0;

/// A placed edge arrow: where it sits and where it points (radians, atan2
/// convention with +y down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArrowPlacement {
    pub pos: [f32; 2],
    pub angle_rad: f32,
}

/// Place the edge arrow for a target that projects to `projected` (band-local
/// pixels). `behind` flags a target behind the camera, whose projection is
/// mirrored garbage — it always gets an arrow, flipped through the band
/// center. A target visibly inside the band gets `None` (no arrow needed).
/// Every returned position lies on the band border, inset by [`ARROW_INSET`].
pub fn edge_arrow(projected: [f32; 2], behind: bool, band: BandRect) -> Option<ArrowPlacement> {
    // A target genuinely on screen (in front, inside the band) needs no arrow.
    let inside = !behind
        && projected[0] >= 0.0 && projected[0] <= band.width
        && projected[1] >= 0.0 && projected[1] <= band.height;
    if inside {
        return None;
    }

    let center = [band.width / 2.0, band.height / 2.0];
    let mut dir = [projected[0] - center[0], projected[1] - center[1]];
    if behind {
        // A behind-camera projection is mirrored; flip it back through center.
        dir = [-dir[0], -dir[1]];
    }
    let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
    if len <= f32::EPSILON {
        return None; // no direction exists (behind, dead center) — no NaN
    }

    // Scale the center ray to the inset border: the smallest positive factor
    // that reaches a side is where the ray exits the band.
    let half_w = band.width / 2.0 - ARROW_INSET;
    let half_h = band.height / 2.0 - ARROW_INSET;
    let tx = if dir[0] != 0.0 { half_w / dir[0].abs() } else { f32::INFINITY };
    let ty = if dir[1] != 0.0 { half_h / dir[1].abs() } else { f32::INFINITY };
    let t = tx.min(ty);
    if !t.is_finite() {
        return None;
    }
    Some(ArrowPlacement {
        pos: [center[0] + dir[0] * t, center[1] + dir[1] * t],
        angle_rad: dir[1].atan2(dir[0]),
    })
}

/// Arrow scale for a contact `distance` metres away: close threats loom,
/// far ones shrink — but never below legibility (playtest 2026-07-04: the
/// arrows were tiny at any range).
pub fn arrow_scale(distance: f32) -> f32 {
    const NEAR: f32 = 2.5; // scale at point blank
    const FAR: f32 = 1.0; // resting scale — never smaller
    const RANGE: f32 = 60.0; // metres over which the loom decays
    if !distance.is_finite() {
        return FAR;
    }
    let t = (distance / RANGE).clamp(0.0, 1.0);
    NEAR + (FAR - NEAR) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAND: BandRect = BandRect { width: 800.0, height: 600.0 };
    const CENTER: [f32; 2] = [400.0, 300.0];

    #[test]
    fn onscreen_target_yields_no_arrow() {
        assert_eq!(edge_arrow(CENTER, false, BAND), None, "dead center needs no arrow");
        assert_eq!(edge_arrow([50.0, 50.0], false, BAND), None, "inside the band needs no arrow");
    }

    #[test]
    fn offscreen_right_pins_to_the_right_edge_pointing_right() {
        let arrow = edge_arrow([2000.0, 300.0], false, BAND).expect("off-screen gets an arrow");
        assert!((arrow.pos[0] - (BAND.width - ARROW_INSET)).abs() < 1e-3,
            "pinned to the inset right edge, got x={}", arrow.pos[0]);
        assert!((arrow.pos[1] - CENTER[1]).abs() < 1e-3,
            "centered vertically for a target at center height, got y={}", arrow.pos[1]);
        assert!(arrow.angle_rad.abs() < 1e-3, "points right, got {}", arrow.angle_rad);
    }

    #[test]
    fn offscreen_above_points_up() {
        let arrow = edge_arrow([400.0, -500.0], false, BAND).expect("off-screen gets an arrow");
        assert!((arrow.pos[1] - ARROW_INSET).abs() < 1e-3,
            "pinned to the inset top edge, got y={}", arrow.pos[1]);
        // +y is down, so "up" is -PI/2.
        assert!((arrow.angle_rad + std::f32::consts::FRAC_PI_2).abs() < 1e-3,
            "points up, got {}", arrow.angle_rad);
    }

    #[test]
    fn behind_camera_always_gets_an_arrow_flipped_through_center() {
        // A behind-camera target projects mirrored: it "appears" right when it
        // is really left. The arrow flips it back — and even a projection
        // inside the band gets an arrow, because the target is NOT on screen.
        let arrow = edge_arrow([500.0, 300.0], true, BAND).expect("behind always gets an arrow");
        assert!((arrow.pos[0] - ARROW_INSET).abs() < 1e-3,
            "flipped to the left edge, got x={}", arrow.pos[0]);
        assert!((arrow.angle_rad.abs() - std::f32::consts::PI).abs() < 1e-3,
            "points left, got {}", arrow.angle_rad);
    }

    #[test]
    fn every_placement_stays_inside_the_inset_band() {
        // Sweep far-flung projections (all quadrants, corners, both behind
        // states): every arrow must land on/within the inset border.
        for behind in [false, true] {
            for x in [-3000.0, -100.0, 0.0, 400.0, 900.0, 5000.0] {
                for y in [-2000.0, -50.0, 0.0, 300.0, 700.0, 4000.0] {
                    let Some(arrow) = edge_arrow([x, y], behind, BAND) else { continue };
                    assert!(
                        arrow.pos[0] >= ARROW_INSET - 1e-3
                            && arrow.pos[0] <= BAND.width - ARROW_INSET + 1e-3
                            && arrow.pos[1] >= ARROW_INSET - 1e-3
                            && arrow.pos[1] <= BAND.height - ARROW_INSET + 1e-3,
                        "arrow for ({x}, {y}, behind={behind}) escaped the band: {:?}",
                        arrow.pos
                    );
                }
            }
        }
    }

    #[test]
    fn arrow_angle_matches_its_direction_from_center() {
        // The arrow points along the center-to-target ray it sits on.
        let arrow = edge_arrow([900.0, 800.0], false, BAND).expect("off-screen gets an arrow");
        let expected = (800.0f32 - CENTER[1]).atan2(900.0 - CENTER[0]);
        assert!((arrow.angle_rad - expected).abs() < 1e-3,
            "angle {} should match the ray {}", arrow.angle_rad, expected);
    }

    #[test]
    fn degenerate_center_behind_is_safely_none() {
        // Behind the camera but projecting exactly onto the band center: no
        // direction exists — no arrow, no NaN.
        assert_eq!(edge_arrow(CENTER, true, BAND), None);
    }

    #[test]
    fn close_contacts_loom_and_far_ones_never_vanish() {
        // Monotonic: nearer is never smaller.
        let mut last = f32::MAX;
        for d in [0.0, 5.0, 10.0, 20.0, 40.0, 80.0, 200.0] {
            let s = arrow_scale(d);
            assert!(s <= last, "scale must not grow with distance ({d}m: {s} > {last})");
            last = s;
        }
        // Point-blank threats loom well past the resting size; the far end
        // stays legible.
        assert!(arrow_scale(0.0) >= 1.8, "a point-blank contact looms");
        assert!(arrow_scale(200.0) >= 1.0, "a distant contact stays legible");
        assert!(arrow_scale(200.0) < arrow_scale(0.0));
        // NaN-safe on nonsense input.
        assert!(arrow_scale(f32::NAN).is_finite());
    }
}
