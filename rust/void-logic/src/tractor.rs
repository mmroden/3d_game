//! Tractor/repulsor field math — the `pull_accel` switch
//! (docs/design/enemy_verbs.md). A living, engaged enemy with a nonzero
//! `pull_accel` bends the player's motion: positive drags the player
//! toward it, negative shoves away. The player fights it with thrust,
//! so declared magnitudes should sit near but below the ship's thrust
//! acceleration — escape possible, but at the cost of the whole envelope.
//!
//! Only the falloff MATH lives here; the node layer supplies positions,
//! gates on engagement, and applies the result as a Jolt force (never a
//! velocity write — docs/architecture/physics_ownership.md).

/// The field's acceleration magnitude (m/s², signed like `pull_accel`)
/// at `distance` from the enemy: full strength at contact, linear falloff
/// to zero at `range` (no cliff at the boundary), zero outside it.
pub fn accel_at(pull_accel: f32, distance: f32, range: f32) -> f32 {
    if range <= 0.0 {
        return 0.0;
    }
    let closeness = 1.0 - (distance.max(0.0) / range);
    if closeness <= 0.0 {
        return 0.0;
    }
    pull_accel * closeness
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_is_full_at_contact_and_dies_at_range() {
        assert_eq!(accel_at(6.0, 0.0, 16.0), 6.0, "full pull at contact");
        assert_eq!(accel_at(6.0, 16.0, 16.0), 0.0, "zero AT the boundary — no cliff");
        assert_eq!(accel_at(6.0, 20.0, 16.0), 0.0, "silent outside the range");
    }

    #[test]
    fn the_falloff_is_linear() {
        assert!((accel_at(6.0, 8.0, 16.0) - 3.0).abs() < 1e-5, "half range = half pull");
        assert!((accel_at(6.0, 4.0, 16.0) - 4.5).abs() < 1e-5, "quarter range = 3/4 pull");
    }

    #[test]
    fn a_repulsor_keeps_its_sign() {
        assert!((accel_at(-6.0, 8.0, 16.0) - -3.0).abs() < 1e-5, "negative pushes, same shape");
    }

    #[test]
    fn degenerate_inputs_are_inert() {
        assert_eq!(accel_at(6.0, 5.0, 0.0), 0.0, "a zero range never divides");
        assert_eq!(accel_at(6.0, -1.0, 16.0), 6.0, "a negative distance clamps to contact");
        assert_eq!(accel_at(0.0, 3.0, 16.0), 0.0, "the default switch is inert");
    }
}
