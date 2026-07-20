// Themed sets of structural and decorative assets for procedural room assembly.
//
// All scene paths use Godot `res://` format pointing into `godot/addons/`.
// Downloaded assets live in `assets/` and are installed by `scripts/install-addons.sh`.

// ── Asset path macros (must precede submodule declarations) ────────────

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

// ── Submodules ─────────────────────────────────────────────────────────

mod wall_sets;
mod props;
mod lights;

pub mod catalog;
pub mod schema;

pub use catalog::{AssetCatalog, Fixture, KitDef, KitId, KitKind, SceneId};

/// The catalog's embedded sources (catalog/ on disk).
pub(crate) const KITS_TOML: &str = include_str!("../../../../catalog/kits.toml");
pub(crate) const KITS_GENERATED_TOML: &str =
    include_str!("../../../../catalog/kits.generated.toml");
pub(crate) const MODELS_TOML: &str = include_str!("../../../../catalog/models.generated.toml");
pub(crate) const ENVIRONMENTS_GENERATED_TOML: &str =
    include_str!("../../../../catalog/environments.generated.toml");

/// The one shared production catalog (parsed on first use; a bad catalog
/// panics with the full violation list, exactly like the roster).
pub fn catalog() -> &'static std::sync::Arc<AssetCatalog> {
    static CATALOG: std::sync::OnceLock<std::sync::Arc<AssetCatalog>> =
        std::sync::OnceLock::new();
    CATALOG.get_or_init(|| {
        std::sync::Arc::new(
            AssetCatalog::load(
                KITS_TOML,
                KITS_GENERATED_TOML,
                MODELS_TOML,
                ENVIRONMENTS_GENERATED_TOML,
            )
            .unwrap_or_else(|e| panic!("catalog/ must parse and link:\n{}", e.join("\n"))),
        )
    })
}
/// The asset probe (`make assets`'s accounting step): test-gated, writes
/// the generated catalogs. `pub(crate)` so the roster's stale-census
/// contract test can call [`probe::kit_grids`] until the linker split
/// moves that test here.
#[cfg(test)]
pub(crate) mod probe;

pub use wall_sets::*;
pub use props::*;
pub use lights::*;

// ── Cross-cutting validation ───────────────────────────────────────────

/// Return every `res://` scene path referenced in the catalog.
/// Used by validation tests to check all assets exist on disk.
pub fn all_scene_paths() -> Vec<&'static str> {
    let mut paths = Vec::new();

    // Wall set assets
    for ws in ALL_WALL_SETS {
        for triple in [&ws.straight, &ws.corner_inner, &ws.corner_outer] {
            paths.push(triple.floor);
            paths.push(triple.wall);
            paths.push(triple.ceiling);
        }
        // New layers
        for path in [
            ws.short_wall.straight, ws.short_wall.corner_inner, ws.short_wall.corner_outer,
            ws.bottom.straight, ws.bottom.corner_inner, ws.bottom.corner_outer,
        ] {
            if !path.is_empty() {
                paths.push(path);
            }
        }
    }

    // Fixtures (the closed set of code-named scenes)
    for &(_, path) in catalog::FIXTURE_PATHS {
        paths.push(path);
    }

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

    /// Amendment A, mint/resolve round-trip: an id the catalog minted
    /// resolves to exactly the path it was minted from — for every
    /// authored const scene in the arena.
    #[test]
    fn scene_ids_round_trip_through_the_catalog() {
        let cat = catalog();
        for path in all_scene_paths() {
            let id = cat.id_of(path).expect("authored const scenes intern at load");
            assert_eq!(cat.path(id), path);
        }
    }

    /// Amendment A, foreign-id panic: a SceneId proves it was minted by
    /// the catalog IN HAND, not merely that some catalog once minted it.
    /// Tests hold production and fixture catalogs side by side; an
    /// in-bounds id resolved against the wrong instance must die loudly,
    /// never silently name the wrong scene. The twin here is the
    /// strictest case — same sources, different mint.
    #[test]
    #[should_panic(expected = "foreign SceneId")]
    fn resolving_a_foreign_scene_id_panics() {
        let twin = AssetCatalog::load(
            KITS_TOML,
            KITS_GENERATED_TOML,
            MODELS_TOML,
            ENVIRONMENTS_GENERATED_TOML,
        )
        .expect("the embedded catalog links");
        let foreign = twin.fixture(Fixture::DoorFrame);
        catalog().path(foreign);
    }

    /// Every scene path in the asset catalog must resolve to a real .gltf file
    /// on disk under the godot/ directory. This catches broken references at
    /// test time rather than as Godot runtime errors.
    #[test]
    fn all_catalog_scene_paths_exist_on_disk() {
        // CARGO_MANIFEST_DIR = rust/void-logic → parent = rust/ → parent = repo root
        let godot_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("void-logic/ should have a parent dir")
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
    fn all_wall_sets_have_ceiling_corners() {
        for ws in ALL_WALL_SETS {
            for (label, triple) in [("inner", &ws.corner_inner), ("outer", &ws.corner_outer)] {
                assert!(
                    triple.ceiling.starts_with("res://")
                        && triple.ceiling.ends_with(".gltf"),
                    "wall set '{}' {label} missing valid ceiling corner path, got '{}'",
                    ws.id,
                    triple.ceiling
                );
            }
        }
    }

    #[test]
    fn all_wall_sets_use_round_or_curve_corners() {
        for ws in ALL_WALL_SETS {
            assert!(
                ws.corner_inner.wall.contains("Round_Inner")
                    || ws.corner_inner.wall.contains("Curve"),
                "wall set '{}' inner uses '{}' — expected Round_Inner or Curve variant",
                ws.id,
                ws.corner_inner.wall
            );
            assert!(
                ws.corner_outer.wall.contains("Round_Outer")
                    || ws.corner_outer.wall.contains("Curve"),
                "wall set '{}' outer uses '{}' — expected Round_Outer or Curve variant",
                ws.id,
                ws.corner_outer.wall
            );
        }
    }

    #[test]
    fn wall_set_has_short_wall_layer() {
        for ws in ALL_WALL_SETS {
            assert!(
                !ws.short_wall.straight.is_empty(),
                "wall set '{}' missing short_wall.straight",
                ws.id
            );
            assert!(
                !ws.short_wall.corner_inner.is_empty(),
                "wall set '{}' missing short_wall.corner_inner",
                ws.id
            );
            assert!(
                !ws.short_wall.corner_outer.is_empty(),
                "wall set '{}' missing short_wall.corner_outer",
                ws.id
            );
        }
    }

    #[test]
    fn wall_set_has_bottom_layer() {
        for ws in ALL_WALL_SETS {
            assert!(
                !ws.bottom.straight.is_empty(),
                "wall set '{}' missing bottom.straight",
                ws.id
            );
            assert!(
                !ws.bottom.corner_inner.is_empty(),
                "wall set '{}' missing bottom.corner_inner",
                ws.id
            );
            assert!(
                !ws.bottom.corner_outer.is_empty(),
                "wall set '{}' missing bottom.corner_outer",
                ws.id
            );
        }
    }

    // (Grid dimensions are no longer fields here: the probe derives each
    // kit's tile/story from these recipes' meshes — roster/probe.rs — and
    // `the_derived_grids_match_the_recipes` pins the committed file.)

    #[test]
    fn short_wall_and_bottom_scene_paths_exist_on_disk() {
        let godot_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("void-logic/ should have a parent dir")
            .parent()
            .expect("rust/ should have a parent dir")
            .join("godot");

        let mut missing = Vec::new();
        for ws in ALL_WALL_SETS {
            for (label, path) in [
                ("short_wall.straight", ws.short_wall.straight),
                ("short_wall.corner_inner", ws.short_wall.corner_inner),
                ("short_wall.corner_outer", ws.short_wall.corner_outer),
                ("bottom.straight", ws.bottom.straight),
                ("bottom.corner_inner", ws.bottom.corner_inner),
                ("bottom.corner_outer", ws.bottom.corner_outer),
            ] {
                if path.is_empty() {
                    missing.push(format!("{} / {label}: (empty)", ws.id));
                    continue;
                }
                let rel = path.strip_prefix("res://").unwrap_or(path);
                let full = godot_dir.join(rel);
                if !full.exists() {
                    missing.push(format!("{} / {label}: {path}", ws.id));
                }
            }
        }

        assert!(
            missing.is_empty(),
            "ShortWall/Bottom scene paths missing on disk:\n{}",
            missing.iter().map(|m| format!("  - {m}")).collect::<Vec<_>>().join("\n")
        );
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

    #[test]
    fn is_surface_mounted_identifies_anchored_props() {
        // Surface-mounted (Static): wall/ceiling equipment, structural columns,
        // the teleporter pad, cables, holograms.
        assert!(is_surface_mounted("res://columns/Column_Astra.gltf"));
        assert!(is_surface_mounted("res://props/Prop_Computer.gltf"));
        assert!(is_surface_mounted("res://props/Prop_Teleporter.gltf"));
        assert!(is_surface_mounted("res://props/Prop_Vent_Big.gltf"));
        // Floating (Dynamic): debris AND free-standing furniture all tumble.
        assert!(!is_surface_mounted("res://props/Prop_Crate1.gltf"));
        assert!(!is_surface_mounted("res://props/Prop_Barrel_Large.gltf"));
        assert!(!is_surface_mounted("res://props/Prop_Desk_Large.gltf"));
        assert!(!is_surface_mounted("res://props/Prop_Pod.gltf"));
    }
}
