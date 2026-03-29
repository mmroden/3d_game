// Themed sets of structural and decorative assets for procedural room assembly.
//
// All scene paths use Godot `res://` format pointing into `godot/addons/`.
// Downloaded assets live in `assets/` and are installed by `scripts/install-addons.sh`.

// ── Wall sets ───────────────────────────────────────────────────────────

/// A themed group of matching wall, corner, ceiling, and floor assets.
#[derive(Debug, Clone, Copy)]
pub struct WallSet {
    pub id: &'static str,
    pub wall_straight: &'static str,
    pub wall_corner_inner: &'static str,
    pub ceiling_straight: &'static str,
    pub ceiling_corner: &'static str,
    pub floor: &'static str,
    pub floor_corner: &'static str,
}

macro_rules! megakit_wall {
    ($name:expr) => {
        concat!(
            "res://addons/quaternius/modularscifimegakit/walls/",
            $name
        )
    };
}
macro_rules! megakit_platform {
    ($name:expr) => {
        concat!(
            "res://addons/quaternius/modularscifimegakit/platforms/",
            $name
        )
    };
}

pub const WALL_SET_ASTRA: WallSet = WallSet {
    id: "astra",
    wall_straight: megakit_wall!("WallAstra_Straight.gltf"),
    wall_corner_inner: megakit_wall!("WallAstra_Corner_Round_Inner.gltf"),
    ceiling_straight: megakit_wall!("TopCables_Straight.gltf"),
    ceiling_corner: megakit_wall!("TopCables_Corner_Round_Inner.gltf"),
    floor: megakit_platform!("Platform_Simple.gltf"),
    floor_corner: megakit_platform!("Platform_Simple_Curve.gltf"),
};

pub const WALL_SET_BAND: WallSet = WallSet {
    id: "band",
    wall_straight: megakit_wall!("WallBand_Straight.gltf"),
    wall_corner_inner: megakit_wall!("WallBand_Corner_Round_Inner.gltf"),
    ceiling_straight: megakit_wall!("TopAstra_Straight.gltf"),
    ceiling_corner: megakit_wall!("TopAstra_Corner_Round_Inner.gltf"),
    floor: megakit_platform!("Platform_Metal.gltf"),
    floor_corner: megakit_platform!("Platform_Metal_Curve.gltf"),
};

pub const WALL_SET_PIPE: WallSet = WallSet {
    id: "pipe",
    wall_straight: megakit_wall!("WallPipe_Straight.gltf"),
    wall_corner_inner: megakit_wall!("WallPipe_Corner_Round_Inner.gltf"),
    ceiling_straight: megakit_wall!("TopPlates_Straight.gltf"),
    ceiling_corner: megakit_wall!("TopPlates_Corner_Round_Inner.gltf"),
    floor: megakit_platform!("Platform_DarkPlates.gltf"),
    floor_corner: megakit_platform!("Platform_DarkPlates_Curves.gltf"),
};

pub const WALL_SET_WIDEBAND: WallSet = WallSet {
    id: "wideband",
    wall_straight: megakit_wall!("WallWideBand_Straight.gltf"),
    wall_corner_inner: megakit_wall!("WallWideBand_Corner_Round_Inner.gltf"),
    ceiling_straight: megakit_wall!("TopSimple_Straight.gltf"),
    ceiling_corner: megakit_wall!("TopSimple_Corner_Round_Inner.gltf"),
    floor: megakit_platform!("Platform_CenterPlate.gltf"),
    floor_corner: megakit_platform!("Platform_CenterPlate_Curve.gltf"),
};

pub const WALL_SET_WINDOW: WallSet = WallSet {
    id: "window",
    wall_straight: megakit_wall!("WallWindow_Straight.gltf"),
    wall_corner_inner: megakit_wall!("WallWindow_Corner_Round_Inner.gltf"),
    ceiling_straight: megakit_wall!("TopWindow_Straight.gltf"),
    ceiling_corner: megakit_wall!("TopWindow_Corner_Curve_Inner.gltf"),
    floor: megakit_platform!("Platform_Squares.gltf"),
    floor_corner: megakit_platform!("Platform_Squares_Curve.gltf"),
};

pub const WALL_SET_PADDED: WallSet = WallSet {
    id: "padded",
    wall_straight: megakit_wall!("WallPadded_Straight.gltf"),
    wall_corner_inner: megakit_wall!("WallPadded_Curve_Round_Inner.gltf"),
    ceiling_straight: megakit_wall!("TopPadded_Flat_Straight.gltf"),
    ceiling_corner: megakit_wall!("TopPadded_Flat_Curve_Round_Inner.gltf"),
    floor: megakit_platform!("Platform_Padded.gltf"),
    floor_corner: megakit_platform!("Platform_Padded.gltf"),
};

pub const ALL_WALL_SETS: &[WallSet] = &[
    WALL_SET_ASTRA,
    WALL_SET_BAND,
    WALL_SET_PIPE,
    WALL_SET_WIDEBAND,
    WALL_SET_WINDOW,
    WALL_SET_PADDED,
];

/// The door frame asset — structural, not themed.
pub const DOOR: &str = megakit_platform!("Door_Frame_Square.gltf");

// ── Props ───────────────────────────────────────────────────────────────

/// Where a prop should be placed relative to the room geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropPlacement {
    /// Against a wall face, rotated to match wall orientation.
    WallAdjacent,
    /// In the room interior, away from walls.
    Center,
    /// At a corner where two walls meet.
    Corner,
    /// Mounted on the ceiling.
    Ceiling,
}

/// A prop that can be placed inside rooms.
#[derive(Debug, Clone, Copy)]
pub struct PropEntry {
    pub scene: &'static str,
    pub placement: PropPlacement,
    /// Whether this prop blocks flight paths through the room.
    pub blocks_flight: bool,
}

macro_rules! megakit_prop {
    ($name:expr) => {
        concat!(
            "res://addons/quaternius/modularscifimegakit/props/",
            $name
        )
    };
}
macro_rules! essentials_prop {
    ($name:expr) => {
        concat!("res://addons/quaternius/essentials/props/", $name)
    };
}
macro_rules! megakit_column {
    ($name:expr) => {
        concat!(
            "res://addons/quaternius/modularscifimegakit/columns/",
            $name
        )
    };
}

pub const WALL_ADJACENT_PROPS: &[PropEntry] = &[
    // Megakit
    PropEntry { scene: megakit_prop!("Prop_Computer.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Vent_Big.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Vent_Small.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Vent_Wide.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Fan_Big.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Fan_Small.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_AccessPoint.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    // Essentials
    PropEntry { scene: essentials_prop!("Prop_Desk_Large.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Desk_Medium.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Desk_Small.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Desk_L.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Shelves_WideTall.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: true },
    PropEntry { scene: essentials_prop!("Prop_Shelves_WideShort.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Shelves_ThinTall.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: true },
    PropEntry { scene: essentials_prop!("Prop_Shelves_ThinShort.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Locker.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: true },
    PropEntry { scene: essentials_prop!("Prop_Screen.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_GunRack.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Computer.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
];

pub const CENTER_PROPS: &[PropEntry] = &[
    // Megakit
    PropEntry { scene: megakit_prop!("Prop_Crate1.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Crate2.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Crate3.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Crate4.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Barrel_Large.gltf"), placement: PropPlacement::Center, blocks_flight: true },
    PropEntry { scene: megakit_prop!("Prop_Barrel_Small.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Teleporter.gltf"), placement: PropPlacement::Center, blocks_flight: true },
    PropEntry { scene: megakit_prop!("Prop_Pod.gltf"), placement: PropPlacement::Center, blocks_flight: true },
    PropEntry { scene: megakit_prop!("Prop_Chest.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    // Essentials
    PropEntry { scene: essentials_prop!("Prop_Crate.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Crate_Large.gltf"), placement: PropPlacement::Center, blocks_flight: true },
    PropEntry { scene: essentials_prop!("Prop_Barrel1.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Barrel2_Closed.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_HologramMap1.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_HologramMap2.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Chest.gltf"), placement: PropPlacement::Center, blocks_flight: false },
];

pub const CORNER_PROPS: &[PropEntry] = &[
    PropEntry { scene: megakit_column!("Column_Astra.gltf"), placement: PropPlacement::Corner, blocks_flight: true },
    PropEntry { scene: megakit_column!("Column_Dark.gltf"), placement: PropPlacement::Corner, blocks_flight: true },
    PropEntry { scene: megakit_column!("Column_Simple.gltf"), placement: PropPlacement::Corner, blocks_flight: true },
    PropEntry { scene: megakit_column!("Column_Hollow.gltf"), placement: PropPlacement::Corner, blocks_flight: true },
    PropEntry { scene: megakit_column!("Column_Pipes.gltf"), placement: PropPlacement::Corner, blocks_flight: true },
    PropEntry { scene: megakit_prop!("Prop_Cable1.gltf"), placement: PropPlacement::Corner, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Cable2.gltf"), placement: PropPlacement::Corner, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Cable3.gltf"), placement: PropPlacement::Corner, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Cable4.gltf"), placement: PropPlacement::Corner, blocks_flight: false },
];

pub const CEILING_PROPS: &[PropEntry] = &[
    PropEntry { scene: essentials_prop!("Prop_CeilingComputer.gltf"), placement: PropPlacement::Ceiling, blocks_flight: false },
];

// ── Light fixtures ──────────────────────────────────────────────────────

/// A light fixture mesh with its co-located light source parameters.
/// The `light_offset` is relative to the fixture mesh origin, keeping the
/// light source physically inside the fixture geometry.
#[derive(Debug, Clone, Copy)]
pub struct LightFixture {
    pub scene: &'static str,
    /// Offset from fixture mesh origin to the light emitter point.
    pub light_offset: [f32; 3],
    /// Approximate half-extents of the fixture mesh (for bounds checking).
    pub fixture_bounds: [f32; 3],
    pub range: f32,
    pub energy: f32,
}

pub const LIGHT_CEILING_WIDE: LightFixture = LightFixture {
    scene: megakit_prop!("Prop_Light_Wide.gltf"),
    light_offset: [0.0, -0.3, 0.0],
    fixture_bounds: [1.0, 0.4, 0.5],
    range: 8.0,
    energy: 1.5,
};

pub const LIGHT_CEILING_SMALL: LightFixture = LightFixture {
    scene: megakit_prop!("Prop_Light_Small.gltf"),
    light_offset: [0.0, -0.2, 0.0],
    fixture_bounds: [0.3, 0.3, 0.3],
    range: 6.0,
    energy: 1.2,
};

pub const LIGHT_CORNER: LightFixture = LightFixture {
    scene: megakit_prop!("Prop_Light_Corner.gltf"),
    light_offset: [0.0, -0.2, 0.0],
    fixture_bounds: [0.4, 0.3, 0.4],
    range: 5.0,
    energy: 1.0,
};

pub const LIGHT_FLOOR: LightFixture = LightFixture {
    scene: megakit_prop!("Prop_Light_Floor.gltf"),
    light_offset: [0.0, 1.0, 0.0],
    fixture_bounds: [0.3, 1.2, 0.3],
    range: 6.0,
    energy: 1.0,
};

pub const CEILING_LIGHTS: &[LightFixture] = &[LIGHT_CEILING_WIDE, LIGHT_CEILING_SMALL];
pub const ALL_LIGHTS: &[LightFixture] = &[LIGHT_CEILING_WIDE, LIGHT_CEILING_SMALL, LIGHT_CORNER, LIGHT_FLOOR];

/// Return every `res://` scene path referenced in the catalog.
/// Used by validation tests to check all assets exist on disk.
pub fn all_scene_paths() -> Vec<&'static str> {
    let mut paths = Vec::new();

    // Wall set assets
    for ws in ALL_WALL_SETS {
        paths.push(ws.wall_straight);
        paths.push(ws.wall_corner_inner);
        paths.push(ws.ceiling_straight);
        paths.push(ws.ceiling_corner);
        paths.push(ws.floor);
        paths.push(ws.floor_corner);
    }

    // Door
    paths.push(DOOR);

    // Props
    for p in WALL_ADJACENT_PROPS {
        paths.push(p.scene);
    }
    for p in CENTER_PROPS {
        paths.push(p.scene);
    }
    for p in CORNER_PROPS {
        paths.push(p.scene);
    }
    for p in CEILING_PROPS {
        paths.push(p.scene);
    }

    // Light fixtures
    for lf in ALL_LIGHTS {
        paths.push(lf.scene);
    }

    // Deduplicate
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every scene path in the asset catalog must resolve to a real .gltf file
    /// on disk under the godot/ directory. This catches broken references at
    /// test time rather than as Godot runtime errors.
    #[test]
    fn all_catalog_scene_paths_exist_on_disk() {
        // Cargo runs tests with cwd = crate root (rust/), so godot/ is at ../godot/
        let godot_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("rust/ should have a parent dir")
            .join("godot");

        let mut missing = Vec::new();
        for res_path in all_scene_paths() {
            // Convert "res://addons/foo/bar.gltf" -> "godot/addons/foo/bar.gltf"
            let rel = res_path
                .strip_prefix("res://")
                .unwrap_or_else(|| panic!("scene path should start with res://: {res_path}"));
            let full = godot_dir.join(rel);
            if !full.exists() {
                missing.push(res_path);
            }
        }

        assert!(
            missing.is_empty(),
            "The following catalog scene paths do not exist on disk:\n{}",
            missing
                .iter()
                .map(|p| format!("  - {p}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    #[test]
    fn all_scene_paths_are_valid_res_format() {
        for path in all_scene_paths() {
            assert!(
                path.starts_with("res://"),
                "scene path should start with res://: {path}"
            );
            assert!(
                path.ends_with(".gltf"),
                "scene path should end with .gltf: {path}"
            );
        }
    }

    /// Every texture referenced by .tres material files in the materials
    /// directory must exist on disk. Missing textures cause cascading Godot
    /// load failures for any prop that uses the material.
    #[test]
    fn all_material_texture_dependencies_exist() {
        let godot_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("rust/ should have a parent dir")
            .join("godot");
        let materials_dir = godot_dir.join("addons/quaternius/materials");

        if !materials_dir.exists() {
            // Skip if assets haven't been installed yet (CI without assets)
            return;
        }

        let mut missing = Vec::new();
        for entry in std::fs::read_dir(&materials_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "tres") {
                let content = std::fs::read_to_string(&path).unwrap();
                // .tres files reference textures via: path="res://addons/quaternius/materials/Foo.png"
                for line in content.lines() {
                    if let Some(start) = line.find("path=\"res://") {
                        let rest = &line[start + 6..]; // skip `path="`
                        if let Some(end) = rest.find('"') {
                            let res_path = &rest[..end];
                            if res_path.ends_with(".png") {
                                let rel = res_path.strip_prefix("res://").unwrap();
                                let full = godot_dir.join(rel);
                                if !full.exists() {
                                    missing.push(format!(
                                        "{}: {}",
                                        path.file_name().unwrap().to_string_lossy(),
                                        res_path
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        assert!(
            missing.is_empty(),
            "Material files reference textures that do not exist on disk:\n{}",
            missing
                .iter()
                .map(|m| format!("  - {m}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    #[test]
    fn all_wall_sets_have_ceiling_corner() {
        for ws in ALL_WALL_SETS {
            assert!(
                ws.ceiling_corner.starts_with("res://")
                    && ws.ceiling_corner.ends_with(".gltf"),
                "wall set '{}' missing valid ceiling_corner path, got '{}'",
                ws.id,
                ws.ceiling_corner
            );
        }
    }

    #[test]
    fn all_wall_sets_use_round_or_curve_corners() {
        for ws in ALL_WALL_SETS {
            assert!(
                ws.wall_corner_inner.contains("Round_Inner")
                    || ws.wall_corner_inner.contains("Curve"),
                "wall set '{}' uses '{}' — expected Round_Inner or Curve variant",
                ws.id,
                ws.wall_corner_inner
            );
        }
    }

    #[test]
    fn no_duplicate_props_within_same_category() {
        let check = |name: &str, entries: &[PropEntry]| {
            let mut scenes: Vec<&str> = entries.iter().map(|p| p.scene).collect();
            scenes.sort();
            for pair in scenes.windows(2) {
                assert_ne!(
                    pair[0], pair[1],
                    "duplicate prop scene in {name}: {}",
                    pair[0]
                );
            }
        };
        check("WALL_ADJACENT_PROPS", WALL_ADJACENT_PROPS);
        check("CENTER_PROPS", CENTER_PROPS);
        check("CORNER_PROPS", CORNER_PROPS);
        check("CEILING_PROPS", CEILING_PROPS);
    }
}
