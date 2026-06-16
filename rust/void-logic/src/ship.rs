//! Player ship color variants. Each color is a real loadout tradeoff, not
//! just cosmetics: it shifts shield capacity/regen and flight thrust.

use serde::{Deserialize, Serialize};

/// A player-selectable ship color / loadout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ShipColor {
    /// Balanced baseline.
    #[default]
    Standard,
    /// Tougher shields, slower.
    Armored,
    /// Weaker shields, faster.
    Swift,
}

impl ShipColor {
    pub const ALL: &[ShipColor] = &[ShipColor::Standard, ShipColor::Armored, ShipColor::Swift];

    /// Accent/identity color [R, G, B, A].
    pub fn color(&self) -> [f32; 4] {
        match self {
            Self::Standard => [0.2, 0.6, 1.0, 1.0], // cyan-blue
            Self::Armored => [1.0, 0.4, 0.15, 1.0], // red-orange
            Self::Swift => [0.3, 0.95, 0.4, 1.0],   // green
        }
    }

    /// Shield-capacity multiplier over the base.
    pub fn shield_capacity_mul(&self) -> f32 {
        match self {
            Self::Standard => 1.0,
            Self::Armored => 1.4,
            Self::Swift => 0.7,
        }
    }

    /// Shield-regen multiplier over the base.
    pub fn shield_regen_mul(&self) -> f32 {
        match self {
            Self::Standard => 1.0,
            Self::Armored => 1.2,
            Self::Swift => 1.0,
        }
    }

    /// Thrust (flight speed) multiplier over the base.
    pub fn thrust_mul(&self) -> f32 {
        match self {
            Self::Standard => 1.0,
            Self::Armored => 0.8,
            Self::Swift => 1.25,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::Armored => "Armored",
            Self::Swift => "Swift",
        }
    }

    /// One-line tradeoff blurb for the loadout screen.
    pub fn blurb(&self) -> &'static str {
        match self {
            Self::Standard => "Balanced shields and speed",
            Self::Armored => "Tougher shields, slower",
            Self::Swift => "Weaker shields, faster",
        }
    }

    pub fn from_id(id: i32) -> Option<ShipColor> {
        Self::ALL.get(id as usize).copied()
    }

    pub fn id(&self) -> i32 {
        Self::ALL.iter().position(|c| c == self)
            .expect("ShipColor::ALL must contain every variant") as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_standard() {
        assert_eq!(ShipColor::default(), ShipColor::Standard);
    }

    #[test]
    fn standard_is_neutral() {
        let s = ShipColor::Standard;
        assert_eq!(s.shield_capacity_mul(), 1.0);
        assert_eq!(s.shield_regen_mul(), 1.0);
        assert_eq!(s.thrust_mul(), 1.0);
    }

    #[test]
    fn armored_trades_speed_for_shields() {
        let a = ShipColor::Armored;
        assert!(a.shield_capacity_mul() > ShipColor::Standard.shield_capacity_mul());
        assert!(a.thrust_mul() < ShipColor::Standard.thrust_mul());
    }

    #[test]
    fn swift_trades_shields_for_speed() {
        let s = ShipColor::Swift;
        assert!(s.shield_capacity_mul() < ShipColor::Standard.shield_capacity_mul());
        assert!(s.thrust_mul() > ShipColor::Standard.thrust_mul());
    }

    #[test]
    fn shields_order_armored_standard_swift() {
        assert!(
            ShipColor::Armored.shield_capacity_mul()
                > ShipColor::Standard.shield_capacity_mul()
                && ShipColor::Standard.shield_capacity_mul()
                    > ShipColor::Swift.shield_capacity_mul()
        );
    }

    #[test]
    fn colors_are_valid_rgba() {
        for variant in ShipColor::ALL {
            for &c in &variant.color() {
                assert!((0.0..=1.0).contains(&c), "{:?} color out of range", variant);
            }
        }
    }

    #[test]
    fn names_and_blurbs_non_empty() {
        for variant in ShipColor::ALL {
            assert!(!variant.display_name().is_empty());
            assert!(!variant.blurb().is_empty());
        }
    }

    #[test]
    fn id_round_trips() {
        for variant in ShipColor::ALL {
            assert_eq!(ShipColor::from_id(variant.id()), Some(*variant));
        }
    }

    #[test]
    fn from_id_rejects_out_of_range() {
        assert_eq!(ShipColor::from_id(-1), None);
        assert_eq!(ShipColor::from_id(99), None);
    }

    #[test]
    fn serde_round_trip() {
        let json = serde_json::to_string(&ShipColor::Armored).unwrap();
        let back: ShipColor = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ShipColor::Armored);
    }
}
