//! Cockpit-shell nesting: how the first-person cockpit model fits around
//! the camera.
//!
//! The shell (extracted by the asset pipeline from the hull's authored
//! interior) is a separate near-field model rendered only in cockpit view.
//! It must nest INSIDE the flight capsule's radial clearance so physics
//! itself guarantees no wall or enemy ever slips between the pilot's eyes
//! and the shell — that guarantee is what makes the cockpit a stereo
//! comfort anchor instead of a new occlusion-conflict source (a near frame
//! whose occlusion and disparity always agree, and which owns the screen
//! edges like a stereographer's floating window).
//!
//! The fit is anchored on the LATERAL reach (the canopy bows): the widest
//! sideways extent from the eyepoint lands exactly on the clearance, using
//! the whole budget — a cockpit wants to be as large (= as far from the
//! eyes) as the capsule allows. Below-eye furniture may overhang the
//! capsule slightly; it sits against the hull's own belly and the rig
//! frames judge whether that ever reads wrong.

/// Widest sideways (x) reach of a shell from its eyepoint, given the
/// shell's x extent in native model units. The larger of the two sides:
/// authored cockpits are near-symmetric, but nothing requires it.
pub fn lateral_reach(lo_x: f32, hi_x: f32, eye_x: f32) -> f32 {
    (eye_x - lo_x).max(hi_x - eye_x)
}

/// Uniform scale that lands a shell's lateral reach exactly on the radial
/// clearance available around the camera (capsule radius minus the
/// camera's own radial offset from the capsule axis). Scales down an
/// oversized shell and up an undersized one — the clearance is a budget,
/// and the whole budget is comfort.
pub fn shell_fit_scale(reach: f32, clearance: f32) -> f32 {
    clearance / reach.max(f32::EPSILON)
}

/// Where the shell model's origin goes, in camera-parent space, so that
/// the shell's authored eyepoint lands exactly on the camera position
/// after `yaw` and `scale` are applied. `yaw` is the hull spec's
/// `model_yaw_offset`: the interior faces wherever the exterior faces —
/// same authored model, same correction — so the eyepoint swings around
/// Y with the model before it is anchored.
pub fn shell_origin(
    eyepoint: (f32, f32, f32),
    scale: f32,
    camera: (f32, f32, f32),
    yaw: f32,
) -> (f32, f32, f32) {
    let (sin, cos) = yaw.sin_cos();
    let rotated = (
        eyepoint.0 * cos + eyepoint.2 * sin,
        eyepoint.1,
        eyepoint.2 * cos - eyepoint.0 * sin,
    );
    (
        camera.0 - rotated.0 * scale,
        camera.1 - rotated.1 * scale,
        camera.2 - rotated.2 * scale,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lateral_reach_is_the_wider_side() {
        // Eye off-center: the far side governs.
        assert_eq!(lateral_reach(-1.0, 3.0, 1.0), 2.0);
        assert_eq!(lateral_reach(-3.0, 1.0, -1.0), 2.0);
    }

    #[test]
    fn lateral_reach_of_a_centered_eye_is_half_the_extent() {
        assert_eq!(lateral_reach(-0.665, 0.665, 0.0), 0.665);
    }

    #[test]
    fn fit_scale_lands_the_reach_exactly_on_the_clearance() {
        for (reach, clearance) in [(0.665, 0.35), (2.0, 0.5), (0.1, 0.35)] {
            let scale = shell_fit_scale(reach, clearance);
            assert!((scale * reach - clearance).abs() < 1e-6);
        }
    }

    #[test]
    fn fit_scale_shrinks_oversized_and_grows_undersized_shells() {
        assert!(shell_fit_scale(1.0, 0.35) < 1.0);
        assert!(shell_fit_scale(0.1, 0.35) > 1.0);
    }

    #[test]
    fn shell_origin_puts_the_eyepoint_on_the_camera() {
        let eye = (0.0, 2.459, 3.434);
        let cam = (0.0, 0.1, 0.0);
        let scale = 0.5;
        let origin = shell_origin(eye, scale, cam, 0.0);
        for k in 0..3 {
            let e = [eye.0, eye.1, eye.2][k];
            let o = [origin.0, origin.1, origin.2][k];
            let c = [cam.0, cam.1, cam.2][k];
            assert!((e * scale + o - c).abs() < 1e-6);
        }
    }

    #[test]
    fn shell_origin_of_a_zero_eyepoint_is_the_camera_itself() {
        assert_eq!(
            shell_origin((0.0, 0.0, 0.0), 0.7, (0.0, 0.1, 0.0), 0.0),
            (0.0, 0.1, 0.0)
        );
    }

    #[test]
    fn shell_origin_honors_the_model_yaw() {
        // A half-turn (the hull spec's model_yaw_offset) mirrors the
        // eyepoint in x and z before anchoring it to the camera — the
        // backwards-cockpit frame of 2026-08-19 is the failure this pins.
        let eye = (0.5, 2.0, 3.0);
        let cam = (0.0, 0.1, 0.0);
        let scale = 0.5;
        let origin = shell_origin(eye, scale, cam, std::f32::consts::PI);
        let rotated = [-eye.0, eye.1, -eye.2];
        for k in 0..3 {
            let o = [origin.0, origin.1, origin.2][k];
            let c = [cam.0, cam.1, cam.2][k];
            assert!(
                (rotated[k] * scale + o - c).abs() < 1e-5,
                "component {k}: rotated eyepoint must land on the camera"
            );
        }
    }
}
