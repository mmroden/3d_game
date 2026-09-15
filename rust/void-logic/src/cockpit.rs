//! Cockpit-shell nesting: how the first-person cockpit model fits around
//! the camera.
//!
//! The shell (extracted by the asset pipeline from the hull's authored
//! interior) is a separate near-field model rendered only in cockpit
//! view, scaled so its widest lateral reach (the canopy bows) lands at a
//! target distance from the pilot's eyes. Scaling is ABOUT THE EYEPOINT,
//! so the target distance changes stereo comfort only — the mono view is
//! angle-identical at any reach (similar triangles): framing is tuned by
//! the extraction oracle's eyepoint fractions, comfort by the reach.
//!
//! History: v1 nested the shell inside the flight capsule's radial
//! clearance so physics guaranteed nothing slipped between eye and shell
//! — geometrically elegant, but the resulting 0.35 m dash sat at HALF a
//! real cockpit's console distance and crossed the pilot's eyes in
//! glasses (owner, 2026-08-20). The reach target now lives with the
//! spawn (real-dash distance); walls being scraped may clip the canopy
//! edge, and comfort outranks that guarantee.

/// Widest sideways (x) reach of a shell from its eyepoint, given the
/// shell's x extent in native model units. The larger of the two sides:
/// authored cockpits are near-symmetric, but nothing requires it.
pub fn lateral_reach(lo_x: f32, hi_x: f32, eye_x: f32) -> f32 {
    (eye_x - lo_x).max(hi_x - eye_x)
}

/// Uniform scale that lands a shell's lateral reach exactly on the
/// target distance from the eyes. Scales down an oversized shell and up
/// an undersized one — the target is a budget, and the whole budget is
/// comfort.
pub fn shell_fit_scale(reach: f32, target: f32) -> f32 {
    target / reach.max(f32::EPSILON)
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
    fn fit_scale_lands_the_reach_exactly_on_the_target() {
        for (reach, target) in [(0.665, 0.6), (2.0, 0.5), (0.1, 0.35)] {
            let scale = shell_fit_scale(reach, target);
            assert!((scale * reach - target).abs() < 1e-6);
        }
    }

    #[test]
    fn fit_scale_shrinks_oversized_and_grows_undersized_shells() {
        assert!(shell_fit_scale(1.0, 0.6) < 1.0);
        assert!(shell_fit_scale(0.1, 0.6) > 1.0);
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
