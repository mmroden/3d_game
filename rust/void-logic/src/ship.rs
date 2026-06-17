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

    /// Accent/identity color [R, G, B, A] — derived from the variant's body
    /// style so the accent light and the painted hull are always the same color
    /// (one mapping, in `body_style`, that the two can't drift apart from).
    pub fn color(&self) -> [f32; 4] {
        STYLE_ACCENT[(self.body_style() - 1) as usize]
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

    /// Which packaged body style (1/2/3) this variant wears. The model ships
    /// with three painted-hull styles — green (1), cyan (2), white (3) — and the
    /// loadout color picks one. No red style exists, so Armored takes white.
    pub fn body_style(&self) -> u8 {
        match self {
            Self::Swift => 1,    // green
            Self::Standard => 2, // cyan
            Self::Armored => 3,  // white
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

/// Accent color for each painted body style (1/2/3), matching the hull texture:
/// green, cyan, white. The single source for both the accent glow and the hull,
/// indexed by `ShipColor::body_style()`.
const STYLE_ACCENT: [[f32; 4]; 3] = [
    [0.3, 0.95, 0.4, 1.0],  // Style_1 — green
    [0.2, 0.6, 1.0, 1.0],   // Style_2 — cyan
    [0.85, 0.83, 0.8, 1.0], // Style_3 — white
];

/// The one material whose textures change between body styles: the painted hull
/// panels (`SPC_Asset_4`). Parts 1-3 and 5 are byte-identical across all three
/// styles, so re-skinning only touches this material.
pub const STYLED_BODY_PART: u32 = 4;

/// The texture number for `part` (1-based) under body `style` (1-based). The
/// pack lays styles out in blocks of five: Style_1 = parts 1..=5, Style_2 =
/// 6..=10, Style_3 = 11..=15. So part 4 of Style_2 is `TX_spacecraft_1_9`.
pub fn style_texture_index(part: u32, style: u8) -> u32 {
    part + (style as u32 - 1) * 5
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
    fn body_style_maps_variants_to_the_three_styles() {
        // Standard→cyan(2), Swift→green(1), Armored→white(3). Distinct styles.
        assert_eq!(ShipColor::Swift.body_style(), 1);
        assert_eq!(ShipColor::Standard.body_style(), 2);
        assert_eq!(ShipColor::Armored.body_style(), 3);
        let styles: std::collections::HashSet<u8> =
            ShipColor::ALL.iter().map(|c| c.body_style()).collect();
        assert_eq!(styles.len(), 3, "each variant wears a distinct style");
    }

    #[test]
    fn accent_color_is_derived_from_the_body_style() {
        // Derived, not a parallel mapping: each variant's accent is exactly its
        // body style's palette color — they cannot drift apart.
        for v in ShipColor::ALL {
            assert_eq!(v.color(), STYLE_ACCENT[(v.body_style() - 1) as usize]);
        }
        // Swift→green, Standard→cyan, Armored→white, all distinct.
        assert_eq!(ShipColor::Swift.color(), [0.3, 0.95, 0.4, 1.0]);
        assert_eq!(ShipColor::Standard.color(), [0.2, 0.6, 1.0, 1.0]);
        assert_eq!(ShipColor::Armored.color(), [0.85, 0.83, 0.8, 1.0]);
    }

    #[test]
    fn style_texture_index_blocks_of_five() {
        // The styled body part (4) across the three styles.
        assert_eq!(style_texture_index(STYLED_BODY_PART, 1), 4);
        assert_eq!(style_texture_index(STYLED_BODY_PART, 2), 9);
        assert_eq!(style_texture_index(STYLED_BODY_PART, 3), 14);
        // General formula on the block edges.
        assert_eq!(style_texture_index(1, 1), 1);
        assert_eq!(style_texture_index(5, 1), 5);
        assert_eq!(style_texture_index(1, 2), 6);
        assert_eq!(style_texture_index(5, 3), 15);
    }

    #[test]
    fn serde_round_trip() {
        let json = serde_json::to_string(&ShipColor::Armored).unwrap();
        let back: ShipColor = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ShipColor::Armored);
    }
}
