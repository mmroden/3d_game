use crate::systems::asset_catalog::{self, PropEntry, WallSet};
use crate::systems::room_furnisher::RoomDensity;

/// A curated set of props for a particular room theme.
#[derive(Debug, Clone, Copy)]
pub struct PropPalette {
    pub name: &'static str,
    pub wall_adjacent: &'static [PropEntry],
    pub center: &'static [PropEntry],
    pub corner: &'static [PropEntry],
    pub ceiling: &'static [PropEntry],
}

/// A complete room theme combining visual style, prop palette, and density.
#[derive(Debug, Clone, Copy)]
pub struct RoomTheme {
    pub name: &'static str,
    pub wall_set: &'static WallSet,
    pub palette: &'static PropPalette,
    pub density: RoomDensity,
}

// ── Predefined palettes ────────────────────────────────────────────────

/// Generic palette — uses all available props.
pub const PALETTE_GENERIC: PropPalette = PropPalette {
    name: "generic",
    wall_adjacent: asset_catalog::WALL_ADJACENT_PROPS,
    center: asset_catalog::CENTER_PROPS,
    corner: asset_catalog::CORNER_PROPS,
    ceiling: asset_catalog::CEILING_PROPS,
};

/// Warehouse — shelves, crates, barrels.
pub const PALETTE_WAREHOUSE: PropPalette = PropPalette {
    name: "warehouse",
    wall_adjacent: asset_catalog::WAREHOUSE_WALL_PROPS,
    center: asset_catalog::WAREHOUSE_CENTER_PROPS,
    corner: asset_catalog::CORNER_PROPS,
    ceiling: asset_catalog::CEILING_PROPS,
};

/// Command — computers, displays, hologram maps, desks.
pub const PALETTE_COMMAND: PropPalette = PropPalette {
    name: "command",
    wall_adjacent: asset_catalog::COMMAND_WALL_PROPS,
    center: asset_catalog::COMMAND_CENTER_PROPS,
    corner: asset_catalog::CORNER_PROPS,
    ceiling: asset_catalog::CEILING_PROPS,
};

/// Laboratory — pods, teleporters, vents, access points.
pub const PALETTE_LABORATORY: PropPalette = PropPalette {
    name: "laboratory",
    wall_adjacent: asset_catalog::LABORATORY_WALL_PROPS,
    center: asset_catalog::LABORATORY_CENTER_PROPS,
    corner: asset_catalog::CORNER_PROPS,
    ceiling: asset_catalog::CEILING_PROPS,
};

// ── Predefined themes ──────────────────────────────────────────────────

pub const THEME_WAREHOUSE: RoomTheme = RoomTheme {
    name: "warehouse",
    wall_set: &asset_catalog::WALL_SET_PIPE,
    palette: &PALETTE_WAREHOUSE,
    density: RoomDensity::Dense,
};

pub const THEME_COMMAND: RoomTheme = RoomTheme {
    name: "command",
    wall_set: &asset_catalog::WALL_SET_ASTRA,
    palette: &PALETTE_COMMAND,
    density: RoomDensity::Normal,
};

pub const THEME_LABORATORY: RoomTheme = RoomTheme {
    name: "laboratory",
    wall_set: &asset_catalog::WALL_SET_WINDOW,
    palette: &PALETTE_LABORATORY,
    density: RoomDensity::Normal,
};

pub const THEME_GENERIC: RoomTheme = RoomTheme {
    name: "generic",
    wall_set: &asset_catalog::WALL_SET_ASTRA,
    palette: &PALETTE_GENERIC,
    density: RoomDensity::Normal,
};

pub const ALL_THEMES: &[RoomTheme] = &[
    THEME_WAREHOUSE,
    THEME_COMMAND,
    THEME_LABORATORY,
    THEME_GENERIC,
];

/// Select a room theme deterministically from a seed and room index.
pub fn theme_for_room(seed: u64, room_idx: usize) -> &'static RoomTheme {
    &ALL_THEMES[(seed as usize + room_idx) % ALL_THEMES.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_palette_has_props_in_every_category() {
        assert!(!PALETTE_GENERIC.wall_adjacent.is_empty(), "generic should have wall-adjacent props");
        assert!(!PALETTE_GENERIC.center.is_empty(), "generic should have center props");
        assert!(!PALETTE_GENERIC.corner.is_empty(), "generic should have corner props");
        assert!(!PALETTE_GENERIC.ceiling.is_empty(), "generic should have ceiling props");
    }

    #[test]
    fn warehouse_palette_has_at_least_one_center_prop() {
        assert!(!PALETTE_WAREHOUSE.center.is_empty(),
            "warehouse palette should have at least 1 center prop (crates/barrels)");
    }

    #[test]
    fn command_palette_has_at_least_one_wall_adjacent_prop() {
        assert!(!PALETTE_COMMAND.wall_adjacent.is_empty(),
            "command palette should have at least 1 wall-adjacent prop (computers/screens)");
    }

    #[test]
    fn laboratory_palette_has_at_least_one_center_prop() {
        assert!(!PALETTE_LABORATORY.center.is_empty(),
            "laboratory palette should have at least 1 center prop (pods/teleporters)");
    }

    #[test]
    fn all_themes_reference_valid_wall_set() {
        for theme in ALL_THEMES {
            assert!(!theme.wall_set.id.is_empty(),
                "theme '{}' should have a valid wall_set", theme.name);
            // Wall set must be one of the known sets.
            assert!(
                asset_catalog::ALL_WALL_SETS.iter().any(|ws| ws.id == theme.wall_set.id),
                "theme '{}' wall_set '{}' not found in ALL_WALL_SETS",
                theme.name, theme.wall_set.id
            );
        }
    }

    #[test]
    fn theme_selection_is_deterministic() {
        let t1 = theme_for_room(42, 0);
        let t2 = theme_for_room(42, 0);
        assert_eq!(t1.name, t2.name, "same seed+idx should produce same theme");
    }

    #[test]
    fn different_seeds_produce_different_themes() {
        // At least 2 of 4 seeds should pick different themes.
        let themes: Vec<&str> = (0..4).map(|s| theme_for_room(s, 0).name).collect();
        let unique: std::collections::HashSet<&&str> = themes.iter().collect();
        assert!(unique.len() >= 2,
            "expected at least 2 different themes across 4 seeds, got {:?}", themes);
    }
}
