//! Ram damage: contact damage from physical collision between player and enemies.
//! Damage scales with impact speed above a minimum threshold.

use crate::newtypes::Damage;

/// Minimum speed for a collision to deal damage (below this is just a nudge).
const MIN_RAM_SPEED: f32 = 8.0;
/// Damage per m/s above the threshold.
const RAM_SCALE: f32 = 0.5;
/// Fraction of ram damage the player takes (drone is sturdier than enemies).
pub const PLAYER_RAM_FRACTION: f32 = 0.3;

/// Compute ram damage from impact speed.
/// Below threshold: zero (just a nudge). Above: scales linearly.
pub fn ram_damage(impact_speed: f32) -> Damage {
    if impact_speed < MIN_RAM_SPEED {
        Damage::new(0.0)
    } else {
        Damage::new((impact_speed - MIN_RAM_SPEED) * RAM_SCALE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_nudge_deals_no_damage() {
        assert_eq!(ram_damage(5.0), Damage::new(0.0));
    }

    #[test]
    fn at_threshold_no_damage() {
        assert_eq!(ram_damage(MIN_RAM_SPEED), Damage::new(0.0));
    }

    #[test]
    fn above_threshold_deals_damage() {
        let dmg = ram_damage(MIN_RAM_SPEED + 1.0);
        assert_eq!(dmg, Damage::new(RAM_SCALE));
    }

    #[test]
    fn damage_scales_with_speed() {
        assert!(ram_damage(30.0).as_f32() > ram_damage(15.0).as_f32());
    }

    #[test]
    fn high_speed_ram_is_significant() {
        // At max speed (50 m/s): (50 - 8) * 0.5 = 21 damage
        let dmg = ram_damage(50.0);
        assert!((dmg.as_f32() - 21.0).abs() < 0.01);
    }

    #[test]
    fn player_fraction_is_less_than_one() {
        assert!(PLAYER_RAM_FRACTION < 1.0);
        assert!(PLAYER_RAM_FRACTION > 0.0);
    }
}
