//! Per-hull armament. Each [`ShipType`](crate::ship_type::ShipType) carries
//! exactly one weapon kind; the firing behaviors (homing steer, cluster
//! fragmentation, subdrone regen) land with the armament milestone — until
//! then every kind falls back to the hitscan path in the shell.

use serde::{Deserialize, Serialize};

/// What a hull fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeaponKind {
    /// The classic dual hitscan laser (the starter hull's ROYGBIV tree).
    HitscanLaser,
    /// Projectiles that steer toward the locked target.
    TrackingLaser,
    /// A shell that bursts into a spread of fragments.
    ClusterMunition,
    /// Regenerating attack drones that hunt on their own.
    SubdroneLauncher,
}

impl WeaponKind {
    pub const ALL: &[WeaponKind] = &[
        WeaponKind::HitscanLaser,
        WeaponKind::TrackingLaser,
        WeaponKind::ClusterMunition,
        WeaponKind::SubdroneLauncher,
    ];

    /// Stable id for GDScript crossings (position in `ALL`).
    pub fn id(&self) -> i32 {
        Self::ALL.iter().position(|w| w == self)
            .expect("WeaponKind::ALL must contain every variant") as i32
    }

    pub fn from_id(id: i32) -> Option<WeaponKind> {
        Self::ALL.get(id as usize).copied()
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::HitscanLaser => "Twin Lasers",
            Self::TrackingLaser => "Tracking Lasers",
            Self::ClusterMunition => "Cluster Cannon",
            Self::SubdroneLauncher => "Subdrone Bay",
        }
    }
}

/// Who fired a bolt: decides which group it passes through harmlessly and
/// which one it damages. One pool serves both sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Faction {
    #[default]
    Enemy,
    Player,
}

impl Faction {
    pub const ALL: &[Faction] = &[Faction::Enemy, Faction::Player];

    pub fn id(&self) -> i32 {
        Self::ALL.iter().position(|f| f == self)
            .expect("Faction::ALL must contain every variant") as i32
    }

    pub fn from_id(id: i32) -> Option<Faction> {
        Self::ALL.get(id as usize).copied()
    }
}

/// Homing math for the tracking laser.
pub mod homing {
    /// How hard a tracking bolt can turn (radians per second).
    pub const TURN_RATE: f32 = 2.5;

    fn length(v: [f32; 3]) -> f32 {
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    /// Rotate `velocity` toward `to_target` by at most `max_turn_rad * dt`,
    /// preserving speed. Zero-length inputs return the velocity unchanged —
    /// a bolt with no target direction flies ballistic, never NaN.
    pub fn steer(velocity: [f32; 3], to_target: [f32; 3], max_turn_rad: f32, dt: f32) -> [f32; 3] {
        let speed = length(velocity);
        let target_len = length(to_target);
        if speed <= f32::EPSILON || target_len <= f32::EPSILON {
            return velocity;
        }
        let current = [velocity[0] / speed, velocity[1] / speed, velocity[2] / speed];
        let desired = [to_target[0] / target_len, to_target[1] / target_len, to_target[2] / target_len];

        let dot = (current[0] * desired[0] + current[1] * desired[1] + current[2] * desired[2])
            .clamp(-1.0, 1.0);
        let angle = dot.acos();
        let max_step = max_turn_rad * dt;
        if angle <= max_step {
            return [desired[0] * speed, desired[1] * speed, desired[2] * speed];
        }

        // Slerp the heading by the capped fraction of the remaining angle.
        // Anti-parallel headings have no unique plane; nudge via any axis by
        // picking the desired direction's own frame (t stays tiny per step).
        let t = max_step / angle;
        let sin_angle = angle.sin();
        if sin_angle <= f32::EPSILON {
            return [desired[0] * speed, desired[1] * speed, desired[2] * speed];
        }
        let a = ((1.0 - t) * angle).sin() / sin_angle;
        let b = (t * angle).sin() / sin_angle;
        let blended = [
            current[0] * a + desired[0] * b,
            current[1] * a + desired[1] * b,
            current[2] * a + desired[2] * b,
        ];
        let blended_len = length(blended).max(f32::EPSILON);
        [
            blended[0] / blended_len * speed,
            blended[1] / blended_len * speed,
            blended[2] / blended_len * speed,
        ]
    }
}

/// Fragmentation math for the cluster cannon.
pub mod cluster {
    /// How many fragments a shell bursts into.
    pub const FRAGMENT_COUNT: usize = 8;

    /// Deterministic fragment directions: `count` unit vectors in a forward
    /// cone of `spread_rad` around `forward`, seeded so the same shot bursts
    /// the same way.
    pub fn fragment_directions(forward: [f32; 3], count: usize, spread_rad: f32, seed: u64) -> Vec<[f32; 3]> {
        use rand::rngs::SmallRng;
        use rand::{RngExt, SeedableRng};

        let len = (forward[0].powi(2) + forward[1].powi(2) + forward[2].powi(2)).sqrt();
        if len <= f32::EPSILON {
            return Vec::new();
        }
        let fwd = [forward[0] / len, forward[1] / len, forward[2] / len];
        // An orthonormal frame around the forward axis: pick the world axis
        // least aligned with it to build the first perpendicular.
        let helper = if fwd[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let right = {
            let c = [
                fwd[1] * helper[2] - fwd[2] * helper[1],
                fwd[2] * helper[0] - fwd[0] * helper[2],
                fwd[0] * helper[1] - fwd[1] * helper[0],
            ];
            let l = (c[0].powi(2) + c[1].powi(2) + c[2].powi(2)).sqrt().max(f32::EPSILON);
            [c[0] / l, c[1] / l, c[2] / l]
        };
        let up = [
            fwd[1] * right[2] - fwd[2] * right[1],
            fwd[2] * right[0] - fwd[0] * right[2],
            fwd[0] * right[1] - fwd[1] * right[0],
        ];

        let mut rng = SmallRng::seed_from_u64(seed);
        (0..count)
            .map(|_| {
                let tilt = rng.random_range(0.0..spread_rad);
                let spin = rng.random_range(0.0..std::f32::consts::TAU);
                let (sin_t, cos_t) = tilt.sin_cos();
                let (sin_s, cos_s) = spin.sin_cos();
                [
                    fwd[0] * cos_t + (right[0] * cos_s + up[0] * sin_s) * sin_t,
                    fwd[1] * cos_t + (right[1] * cos_s + up[1] * sin_s) * sin_t,
                    fwd[2] * cos_t + (right[2] * cos_s + up[2] * sin_s) * sin_t,
                ]
            })
            .collect()
    }
}

/// The subdrone bay's regeneration clock.
pub mod subdrone {
    /// Drones the bay can field at once.
    pub const SQUAD_SIZE: usize = 2;
    /// Seconds to regrow one launch charge.
    pub const REGEN_SECONDS: f32 = 120.0;

    /// Counts up to ready; consuming a launch restarts the clock. Starts
    /// ready — the bay is stocked when the level begins.
    #[derive(Debug, Clone, PartialEq)]
    pub struct RegenTimer {
        elapsed: f32,
        period: f32,
    }

    impl RegenTimer {
        pub fn new(period: f32) -> Self {
            Self { elapsed: period, period }
        }

        pub fn tick(&mut self, dt: f32) {
            self.elapsed = (self.elapsed + dt).min(self.period);
        }

        pub fn ready(&self) -> bool {
            self.elapsed >= self.period
        }

        /// Spend the charge if ready; returns whether a launch happened.
        pub fn consume(&mut self) -> bool {
            if !self.ready() {
                return false;
            }
            self.elapsed = 0.0;
            true
        }
    }
}


/// The Valkyrie heavy center cannon — the Vanguard's purchasable keystone
/// (see [`Unlock::Valkyrie`](crate::unlocks::Unlock)). It layers onto the
/// hitscan trigger: ponderous cadence, a bolt that hits like a payload.
pub mod valkyrie {
    use crate::newtypes::Damage;
    use crate::weapon::WeaponState;

    /// Shots per second — deliberately heavy next to the 2/s hitscan.
    pub const FIRE_RATE: f32 = 0.5;
    /// Damage multiple over the laser's per-shot damage.
    pub const DAMAGE_MULT: f32 = 5.0;
    /// Muzzle speed of the heavy bolt (m/s).
    pub const BOLT_SPEED: f32 = 35.0;

    /// Cooldown state for the cannon, its punch scaled off the equipped
    /// laser so the keystone stays relevant up the ROYGBIV ladder.
    pub fn state(laser_damage: Damage) -> WeaponState {
        let damage = Damage::new(laser_damage.as_f32() * DAMAGE_MULT);
        WeaponState::new(FIRE_RATE, damage, 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn len(v: [f32; 3]) -> f32 {
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    #[test]
    fn faction_ids_round_trip() {
        for faction in Faction::ALL {
            assert_eq!(Faction::from_id(faction.id()), Some(*faction));
        }
        assert_eq!(Faction::from_id(99), None);
    }

    #[test]
    fn homing_converges_on_the_target() {
        // Flying +x, target at +y: repeated steering must swing the velocity
        // around until it points at the target.
        let mut velocity = [20.0, 0.0, 0.0];
        let target_dir = [0.0, 1.0, 0.0];
        for _ in 0..200 {
            velocity = homing::steer(velocity, target_dir, 2.0, 1.0 / 60.0);
        }
        assert!(velocity[1] > 19.0, "the bolt should end up flying at the target, got {velocity:?}");
        assert!(velocity[0].abs() < 2.0, "little of the original heading remains, got {velocity:?}");
    }

    #[test]
    fn homing_preserves_speed_and_respects_the_turn_cap() {
        let velocity = [20.0, 0.0, 0.0];
        let steered = homing::steer(velocity, [0.0, 1.0, 0.0], 2.0, 1.0 / 60.0);
        assert!((len(steered) - 20.0).abs() < 1e-3, "speed is preserved, got {}", len(steered));
        // One 60 Hz step at 2 rad/s turns at most ~1.9 degrees.
        let cos_angle = (velocity[0] * steered[0] + velocity[1] * steered[1] + velocity[2] * steered[2])
            / (len(velocity) * len(steered));
        let turned = cos_angle.clamp(-1.0, 1.0).acos();
        assert!(turned <= 2.0 / 60.0 + 1e-4, "turn rate capped, got {turned} rad");
        assert!(turned > 0.0, "it does actually turn");
    }

    #[test]
    fn homing_is_nan_safe_on_degenerate_inputs() {
        let v = homing::steer([0.0; 3], [1.0, 0.0, 0.0], 2.0, 0.016);
        assert!(v.iter().all(|c| c.is_finite()), "zero velocity stays finite");
        let v = homing::steer([10.0, 0.0, 0.0], [0.0; 3], 2.0, 0.016);
        assert!(v.iter().all(|c| c.is_finite()), "zero target dir stays finite");
        assert_eq!(v, [10.0, 0.0, 0.0], "no direction to steer toward — ballistic");
    }

    #[test]
    fn fragments_are_deterministic_unit_vectors_in_the_cone() {
        let forward = [0.0, 0.0, -1.0];
        let spread = 0.5f32;
        let a = cluster::fragment_directions(forward, cluster::FRAGMENT_COUNT, spread, 42);
        let b = cluster::fragment_directions(forward, cluster::FRAGMENT_COUNT, spread, 42);
        assert_eq!(a, b, "the same seed bursts the same way");
        assert_eq!(a.len(), cluster::FRAGMENT_COUNT);
        for dir in &a {
            assert!((len(*dir) - 1.0).abs() < 1e-3, "unit direction, got {dir:?}");
            let dot = -dir[2]; // cos(angle to forward)
            assert!(dot >= (spread.cos() - 1e-3), "inside the cone, got {dir:?}");
        }
        let c = cluster::fragment_directions(forward, cluster::FRAGMENT_COUNT, spread, 43);
        assert_ne!(a, c, "different seeds spread differently");
    }

    #[test]
    fn regen_timer_starts_stocked_and_regrows_after_consume() {
        use subdrone::RegenTimer;
        let mut timer = RegenTimer::new(10.0);
        assert!(timer.ready(), "the bay starts stocked");
        assert!(timer.consume(), "a stocked bay launches");
        assert!(!timer.ready(), "the charge is spent");
        assert!(!timer.consume(), "an empty bay refuses");
        timer.tick(9.0);
        assert!(!timer.ready(), "not regrown yet");
        timer.tick(1.5);
        assert!(timer.ready(), "regrown after the period");
        assert!(timer.consume());
    }

    #[test]
    fn ids_round_trip() {
        for kind in WeaponKind::ALL {
            assert_eq!(WeaponKind::from_id(kind.id()), Some(*kind));
        }
        assert_eq!(WeaponKind::from_id(-1), None);
        assert_eq!(WeaponKind::from_id(99), None);
    }

    #[test]
    fn every_kind_is_named() {
        for kind in WeaponKind::ALL {
            assert!(!kind.display_name().is_empty(), "{kind:?} needs a name");
        }
    }


    #[test]
    fn the_valkyrie_trades_cadence_for_weight() {
        use crate::newtypes::Damage;
        use crate::weapon::WeaponState;
        let laser = WeaponState::default();
        let cannon = valkyrie::state(laser.damage);
        assert!(cannon.fire_rate < laser.fire_rate,
            "the cannon must fire slower than the lasers ({} vs {})",
            cannon.fire_rate, laser.fire_rate);
        assert!(cannon.damage.as_f32() >= 4.0 * laser.damage.as_f32(),
            "the cannon must hit several times harder than one laser shot");

        // The punch scales with the equipped laser — the keystone never
        // goes obsolete as the ROYGBIV ladder climbs.
        let violet = valkyrie::state(Damage::new(7.0));
        assert!(violet.damage.as_f32() > cannon.damage.as_f32());
    }
}
