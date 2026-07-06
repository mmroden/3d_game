//! The player hull roster — organics-purchasable ships, each with its own
//! stats and weapon. Follows the [`EnemyType`](crate::enemy_type::EnemyType)
//! pattern exactly: enum + `ALL` + parallel spec table + `id`/`from_id`.
//!
//! [`ShipColor`](crate::ship::ShipColor) stays orthogonal: it is the trim
//! variant of hulls that support painted styles (only the starter does).

use crate::armament::WeaponKind;

/// All player hulls, starter first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum ShipType {
    /// The starter: balanced, twin hitscan lasers, the three painted styles.
    #[default]
    Vanguard,
    /// Interceptor: fast and slippery, tracking lasers.
    Talon,
    /// Carrier: tough, slow, fights through its subdrones.
    Hive,
    /// Bruiser: cluster munitions for crowds.
    Reaver,
}

/// Everything a hull is: stat multipliers over the shared `BaseStats`, its
/// weapon, its price (None = starter, always owned), and its model fitting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShipSpec {
    pub thrust_mul: f32,
    pub rotation_mul: f32,
    pub shield_capacity_mul: f32,
    pub shield_regen_mul: f32,
    pub weapon: WeaponKind,
    pub organic_cost: Option<u32>,
    pub model_path: &'static str,
    /// Longest-edge fit target (meters).
    pub model_size: f32,
    /// Yaw correction for the model's imported front axis (radians) — the
    /// single knob to tune if a hull flies askew (see EnemyType's).
    pub model_yaw_offset: f32,
    /// Whether the hull has the three painted body styles (ShipColor trim).
    pub supports_styles: bool,
    pub display_name: &'static str,
    pub blurb: &'static str,
}

impl ShipType {
    pub const ALL: &[ShipType] = &[
        ShipType::Vanguard,
        ShipType::Talon,
        ShipType::Hive,
        ShipType::Reaver,
    ];

    /// Compile-time spec table indexed by variant order in `ALL`. Model yaw
    /// offsets correct each hull's imported front axis (the cgtrader ships
    /// model facing +Z, so a half turn points the nose along -Z forward);
    /// the per-hull value is the tuning knob if one imports askew, checked
    /// visually on first `make run` — like EnemyType's.
    const SPECS: &[ShipSpec] = &[
        // Vanguard — the starter: balanced, twin lasers, the three painted styles.
        ShipSpec {
            thrust_mul: 1.0, rotation_mul: 1.0,
            shield_capacity_mul: 1.0, shield_regen_mul: 1.0,
            weapon: WeaponKind::HitscanLaser,
            organic_cost: None,
            model_path: "res://addons/ships/Spacecraft_1.glb",
            model_size: 2.0, model_yaw_offset: std::f32::consts::PI,
            supports_styles: true,
            display_name: "Vanguard",
            blurb: "The workhorse that brought you here. Twin lasers, honest \
handling, and three paint jobs the yard will still argue about.",
        },
        // Talon — interceptor: fast, slippery, thin shields, never loses a lock.
        ShipSpec {
            thrust_mul: 1.2, rotation_mul: 1.15,
            shield_capacity_mul: 0.8, shield_regen_mul: 1.0,
            weapon: WeaponKind::TrackingLaser,
            organic_cost: Some(3_000),
            model_path: "res://addons/ships/talon.glb",
            model_size: 2.0, model_yaw_offset: std::f32::consts::PI,
            supports_styles: false,
            display_name: "Talon",
            blurb: "An interceptor built around one idea: the shot that chases. \
Thin plating, wicked speed, and lances that bend after whatever you paint.",
        },
        // Hive — carrier: tough and slow, fights through its drones.
        ShipSpec {
            thrust_mul: 0.85, rotation_mul: 0.9,
            shield_capacity_mul: 1.3, shield_regen_mul: 1.1,
            weapon: WeaponKind::SubdroneLauncher,
            organic_cost: Some(4_000),
            model_path: "res://addons/ships/hive.glb",
            model_size: 2.4, model_yaw_offset: std::f32::consts::PI,
            supports_styles: false,
            display_name: "Hive",
            blurb: "A slab of shields with a bay full of opinions. The Hive \
doesn't shoot back — it releases things that do, and grows them back.",
        },
        // Reaver — bruiser: cluster munitions for crowds.
        ShipSpec {
            thrust_mul: 1.0, rotation_mul: 0.95,
            shield_capacity_mul: 1.1, shield_regen_mul: 0.9,
            weapon: WeaponKind::ClusterMunition,
            // The roster's price cap (owner re-anchored 2026-07-03: hulls
            // are long-arc purchases — "300 is wayyyy too cheap").
            organic_cost: Some(5_000),
            model_path: "res://addons/ships/reaver.glb",
            model_size: 2.2, model_yaw_offset: std::f32::consts::PI,
            supports_styles: false,
            display_name: "Reaver",
            blurb: "Alien hull, human trigger discipline. One shell goes out, \
a dozen problems come apart — best fired into a room you've given up on.",
        },
    ];

    pub fn spec(&self) -> ShipSpec {
        Self::SPECS[Self::ALL.iter().position(|s| s == self)
            .expect("ShipType::ALL must contain every variant")]
    }

    pub fn id(&self) -> i32 {
        Self::ALL.iter().position(|s| s == self)
            .expect("ShipType::ALL must contain every variant") as i32
    }

    pub fn from_id(id: i32) -> Option<ShipType> {
        Self::ALL.get(id as usize).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_table_matches_all_length() {
        assert_eq!(ShipType::ALL.len(), ShipType::SPECS.len());
    }

    #[test]
    fn ids_round_trip() {
        for ship in ShipType::ALL {
            assert_eq!(ShipType::from_id(ship.id()), Some(*ship));
        }
        assert_eq!(ShipType::from_id(-1), None);
        assert_eq!(ShipType::from_id(99), None);
    }

    #[test]
    fn the_starter_is_free_and_styled_and_everything_else_costs_organics() {
        assert_eq!(ShipType::Vanguard.spec().organic_cost, None,
            "the starter hull is always owned");
        assert!(ShipType::Vanguard.spec().supports_styles,
            "only the starter has the painted styles");
        for ship in ShipType::ALL {
            if *ship == ShipType::Vanguard {
                continue;
            }
            let spec = ship.spec();
            assert!(spec.organic_cost.unwrap_or(0) > 0, "{ship:?} must cost organics");
            assert!(!spec.supports_styles,
                "{ship:?} has a single livery — no style row on ship-select");
        }
    }

    #[test]
    fn every_hull_has_a_distinct_weapon() {
        let weapons: Vec<_> = ShipType::ALL.iter().map(|s| s.spec().weapon).collect();
        let mut deduped = weapons.clone();
        deduped.dedup();
        for (i, a) in weapons.iter().enumerate() {
            for (j, b) in weapons.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "two hulls share a weapon — the roster is the arsenal");
                }
            }
        }
        assert_eq!(ShipType::Vanguard.spec().weapon, WeaponKind::HitscanLaser,
            "the starter keeps the classic lasers");
    }

    #[test]
    fn models_are_installed_mesh_resources() {
        for ship in ShipType::ALL {
            let spec = ship.spec();
            assert!(spec.model_path.starts_with("res://addons/ships/"),
                "{ship:?} model must be an installed ship asset, got {}", spec.model_path);
            assert!(spec.model_path.ends_with(".glb"), "{ship:?} model should be a glb");
            assert!(spec.model_size > 0.0, "{ship:?} needs a fit size");
        }
    }

    #[test]
    fn stats_and_text_are_filled_in() {
        for ship in ShipType::ALL {
            let spec = ship.spec();
            assert!(spec.thrust_mul > 0.0 && spec.rotation_mul > 0.0
                && spec.shield_capacity_mul > 0.0 && spec.shield_regen_mul > 0.0,
                "{ship:?} multipliers must be positive");
            assert!(!spec.display_name.is_empty(), "{ship:?} needs a name");
            assert!(!spec.blurb.is_empty(), "{ship:?} needs a blurb");
        }
    }
}
