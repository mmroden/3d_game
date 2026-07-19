use super::*;
use crate::asset_catalog::{AssetCatalog, PanelSet, SceneId};
use crate::cell::CellGrid;

/// A synthetic pool through the REAL door: a fixture catalog links a
/// pieces census — assembly contracts are about WHERE panels go and
/// THAT they come from the given pool, never about shipped kit
/// contents, and pools only ever come from a census. Distinct
/// thicknesses so the seating contract can't pass by accident.
fn test_catalog() -> &'static AssetCatalog {
    static CAT: std::sync::OnceLock<AssetCatalog> = std::sync::OnceLock::new();
    CAT.get_or_init(|| {
        let kits = "[kits.test_pool]\nparadigm = \"panel\"\n\
             install_dir = \"godot/pool\"\nwall_coverage = 0.9\n";
        let grids = concat!(
            "[kits.test_pool]\ntile = 3.0\nstory = 3.0\n",
            "[kits.test_pool.pieces.plate_a]\nface = [3.0, 3.0]\nthick = 0.1\n",
            "axis = \"y\"\ncoverage = 1.0\ntris = 10\ntextures = 1\n",
            "[kits.test_pool.pieces.plate_b]\nface = [3.0, 3.0]\nthick = 0.2\n",
            "axis = \"y\"\ncoverage = 1.0\ntris = 10\ntextures = 1\n",
            "[kits.test_pool.pieces.plate_c]\nface = [3.0, 3.0]\nthick = 0.3\n",
            "axis = \"y\"\ncoverage = 1.0\ntris = 10\ntextures = 1\n",
            "[kits.test_pool.pieces.plate_d]\nface = [3.0, 3.0]\nthick = 0.4\n",
            "axis = \"y\"\ncoverage = 1.0\ntris = 10\ntextures = 1\n",
            // Census v2: the baked role variants the v2 assembler consumes —
            // fillers in every surface role plus a two-cell wall so course
            // covering has a width family to exercise.
            "[kits.test_pool.variants.plate_a_floor]\nsources = [\"plate_a\"]\n",
            "role = \"floor\"\nface = [3.0, 3.0]\nthick = 0.1\naxis = \"y\"\n",
            "detail = \"pos_y\"\ncoverage = 1.0\nstretch = 0.0\ntris = 10\ntextures = 1\n",
            "[kits.test_pool.variants.plate_a_ceiling]\nsources = [\"plate_a\"]\n",
            "role = \"ceiling\"\nface = [3.0, 3.0]\nthick = 0.1\naxis = \"y\"\n",
            "detail = \"neg_y\"\ncoverage = 1.0\nstretch = 0.0\ntris = 10\ntextures = 1\n",
            "[kits.test_pool.variants.plate_a_wall]\nsources = [\"plate_a\"]\n",
            "role = \"wall\"\nface = [3.0, 3.0]\nthick = 0.1\naxis = \"z\"\n",
            "detail = \"pos_z\"\ncoverage = 1.0\nstretch = 0.0\ntris = 10\ntextures = 1\n",
            "[kits.test_pool.variants.plate_b_wide_wall]\nsources = [\"plate_b\"]\n",
            "role = \"wall\"\nface = [6.0, 3.0]\nthick = 0.2\naxis = \"z\"\n",
            "detail = \"pos_z\"\ncoverage = 1.0\nstretch = 0.0\ntris = 10\ntextures = 1\n",
        );
        AssetCatalog::load(kits, grids, "[models]\n", "[environments]\n")
            .unwrap_or_else(|e| panic!("test pool links: {}", e.join("\n")))
    })
}

fn test_pool() -> &'static PanelSet {
    test_catalog()
        .kit("test_pool")
        .expect("fixture kit links")
        .1
        .panel_pool
        .as_ref()
        .expect("panel fixture derives a pool")
}

/// The censused thickness of a placed plate.
fn thick_of(scene: SceneId) -> f32 {
    test_pool()
        .plates
        .iter()
        .find(|p| p.scene == scene)
        .expect("placement from the pool")
        .thick
}

// ==========================================================================
// Panel-world assembly (B11): planet 2+ rooms are cubic cells skinned by
// ONE panel pool serving every face — floor/wall/ceiling are terrestrial
// words with no meaning here; a face is a face and rotation is the only
// difference. These are the Panel twins of the Layered assembly contracts.
// ==========================================================================

/// The panel pitch: cubic cells, tile == story.
const P: f32 = 3.0;

fn cubic_grid(template: &RoomTemplate, active: &[Connector]) -> CellGrid {
    CellGrid::new(template, active, [0.0, 0.0, 0.0], P, P)
}

fn sealed_3x3x3() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [3, 3, 3],
    }
}

fn panels(template: &RoomTemplate, active: &[Connector], seed: u64) -> Vec<MeshPlacement> {
    let grid = cubic_grid(template, active);
    assemble_panels_from_grid(&grid, test_pool(), seed)
}

// ══════════════════════════════════════════════════════════════════════
// Assembly v2 (phase 4): role pools + course covering. Watertightness is
// an AREA invariant — wide plates cover several faces at once.
// ══════════════════════════════════════════════════════════════════════

fn role_pools() -> &'static crate::asset_catalog::RolePools {
    test_catalog()
        .kit("test_pool")
        .expect("fixture kit links")
        .1
        .role_pools
        .as_ref()
        .expect("variant census derives role pools")
}

/// A placed plate's face area, from whichever role pool owns its scene.
fn area_of(scene: SceneId) -> f32 {
    let p = role_pools();
    p.floor
        .iter()
        .chain(&p.ceiling)
        .chain(&p.wall)
        .find(|pl| pl.scene == scene)
        .map(|pl| pl.face[0] * pl.face[1])
        .expect("placement from the role pools")
}

fn v2(template: &RoomTemplate, active: &[Connector], seed: u64) -> Vec<MeshPlacement> {
    let grid = cubic_grid(template, active);
    assemble_role_pools_from_grid(&grid, role_pools(), seed)
}

/// AREA watertightness: a sealed 3×3×3 room has 6 sides × 81 m² of
/// sealed surface; the v2 cover must equal it exactly — however many
/// plates that takes (the wide wall covers two faces at once).
#[test]
fn v2_sealed_room_covers_the_sealed_area_exactly() {
    let placements = v2(&sealed_3x3x3(), &[], 7);
    assert!(!placements.is_empty(), "a sealed room gets skinned");
    let covered: f32 = placements.iter().map(|p| area_of(p.scene)).sum();
    assert!(
        (covered - 6.0 * 81.0).abs() < 0.5,
        "covered {covered} m² of a 486 m² sealed shell"
    );
}

/// Role-correctness: floor plates lie on the room's bottom plane, ceiling
/// plates on its top plane, wall plates on the four side planes — pool
/// membership tells which is which.
#[test]
fn v2_roles_land_on_their_planes() {
    let p = role_pools();
    let in_pool = |pool: &[crate::asset_catalog::RolePlate], scene: SceneId| {
        pool.iter().any(|pl| pl.scene == scene)
    };
    for m in v2(&sealed_3x3x3(), &[], 7) {
        let [x, y, z] = m.position;
        if in_pool(&p.floor, m.scene) {
            assert!(y < 1.0, "floor plate at y={y}");
        } else if in_pool(&p.ceiling, m.scene) {
            assert!(y > 8.0, "ceiling plate at y={y}");
        } else {
            // Wall: seated near one of the four side planes of the 9 m cube.
            let near_side = x.abs() < 1.0
                || (x - 9.0).abs() < 1.0
                || z.abs() < 1.0
                || (z - 9.0).abs() < 1.0;
            assert!(near_side, "wall plate at ({x}, {y}, {z}) is not on a side plane");
        }
    }
}

/// Openings subtract from the covered area: one doorway face = 9 m² less,
/// and nothing may stand in the hole.
#[test]
fn v2_openings_stay_open_and_reduce_the_area() {
    let mut template = sealed_3x3x3();
    let door = Connector {
        offset: [0, 0, 1],
        facing: ConnectorFacing::NegX,
        frame: FrameStyle::Door,
    };
    template.connectors.push(door);
    let placements = v2(&template, &[door], 7);
    let covered: f32 = placements.iter().map(|p| area_of(p.scene)).sum();
    assert!(
        (covered - (6.0 * 81.0 - 9.0)).abs() < 0.5,
        "one open face: covered {covered} of 477 m²"
    );
    let hole = [0.0, P * 0.5, (1.0 + 0.5) * P];
    for m in &placements {
        let d = (m.position[0] - hole[0]).abs()
            + (m.position[1] - hole[1]).abs()
            + (m.position[2] - hole[2]).abs();
        assert!(d > 0.01, "a plate seals the doorway at {:?}", m.position);
    }
}

/// Same seed, same skin; different seed may differ — and the wide wall
/// plate actually participates across seeds (the coverer's variety).
#[test]
fn v2_is_seed_deterministic_and_uses_the_width_family() {
    let a = v2(&sealed_3x3x3(), &[], 11);
    let b = v2(&sealed_3x3x3(), &[], 11);
    assert_eq!(a, b, "same seed, same skin");

    let wide = role_pools()
        .wall
        .iter()
        .find(|pl| pl.face[0] > 4.0)
        .expect("fixture pools a wide wall")
        .scene;
    let wide_used = (0..24).any(|seed| v2(&sealed_3x3x3(), &[], seed).iter().any(|m| m.scene == wide));
    assert!(wide_used, "the 6 m wall plate never places across 24 seeds");
}

/// Face coverage: every sealed boundary face gets exactly one panel — no
/// gaps, no doubles. A sealed 3×3×3 room has 9 faces per side × 6 sides.
#[test]
fn every_sealed_face_gets_exactly_one_panel() {
    let placements = panels(&sealed_3x3x3(), &[], 7);
    assert_eq!(placements.len(), 54, "6 sides × 9 faces, one panel each");

    // No two panels share a position (no doubled faces).
    for (i, a) in placements.iter().enumerate() {
        for b in placements.iter().skip(i + 1) {
            let d = (a.position[0] - b.position[0]).abs()
                + (a.position[1] - b.position[1]).abs()
                + (a.position[2] - b.position[2]).abs();
            assert!(d > 0.01, "two panels share a face at {:?}", a.position);
        }
    }

    // Every panel is SEATED on a face plane of the 9×9×9 m room volume:
    // its BACK on the plane, its body half a censused thickness inward —
    // centered plates leave a void slit at every corner (flythrough
    // 2026-07-13); seated ones close it.
    for p in &placements {
        let half = thick_of(p.scene) * 0.5;
        let seated = p.position.iter().any(|c| {
            let toward_interior = (c - half) / P;
            let toward_exterior = (c + half) / P;
            (toward_interior - toward_interior.round()).abs() < 1e-4
                || (toward_exterior - toward_exterior.round()).abs() < 1e-4
        });
        assert!(
            seated,
            "panel is not seated half a thickness off a face plane: {:?} ({})",
            p.position,
            test_catalog().path(p.scene)
        );
        let centered = p
            .position
            .iter()
            .any(|c| (c / P - (c / P).round()).abs() < 1e-4);
        assert!(
            !centered,
            "panel still CENTERED on a face plane (the corner-slit bug): {:?}",
            p.position
        );
    }
}

/// Connector openings stay open — the doorway face gets no panel.
#[test]
fn connector_openings_stay_open() {
    let mut template = sealed_3x3x3();
    let door = Connector {
        offset: [0, 0, 1],
        facing: ConnectorFacing::NegX,
        frame: FrameStyle::Door,
    };
    template.connectors.push(door);
    let placements = panels(&template, &[door], 7);
    assert_eq!(placements.len(), 53, "the doorway face is skipped");
    // The open face center: x = 0 plane, cell (0,0,1).
    let hole = [0.0, P * 0.5, (1.0 + 0.5) * P];
    for p in &placements {
        let d = (p.position[0] - hole[0]).abs()
            + (p.position[1] - hole[1]).abs()
            + (p.position[2] - hole[2]).abs();
        assert!(d > 0.01, "a panel seals the doorway at {:?}", p.position);
    }
}

/// Panel choice is a pure function of the room seed.
#[test]
fn panel_choice_is_seed_deterministic() {
    let a = panels(&sealed_3x3x3(), &[], 11);
    let b = panels(&sealed_3x3x3(), &[], 11);
    assert_eq!(a, b, "same seed, same skin");
    let c = panels(&sealed_3x3x3(), &[], 12);
    assert_ne!(a, c, "different seed, different skin");
}

/// The isotropy pin — the paradigm's whole point: every orientation draws
/// from the SAME pool. No floor list, no ceiling list; variety shows up on
/// every face direction, and nothing outside the pool ever appears.
#[test]
fn all_orientations_draw_from_the_one_pool() {
    use std::collections::HashSet;
    let mut down_scenes: HashSet<SceneId> = HashSet::new(); // faces with rot (0,0)
    let mut up_scenes: HashSet<SceneId> = HashSet::new(); // rot (π,0)
    let mut side_scenes: HashSet<SceneId> = HashSet::new(); // |rot_x| = π/2
    for seed in 0..12 {
        for p in panels(&sealed_3x3x3(), &[], seed) {
            assert!(test_pool().plates.iter().any(|pl| pl.scene == p.scene),
                "{} is not in the panel pool", test_catalog().path(p.scene));
            if p.rotation_x.abs() < 1e-4 {
                down_scenes.insert(p.scene);
            } else if (p.rotation_x.abs() - std::f32::consts::PI).abs() < 1e-4 {
                up_scenes.insert(p.scene);
            } else {
                side_scenes.insert(p.scene);
            }
        }
    }
    assert!(down_scenes.len() > 1, "the 'floor' direction shows pool variety");
    assert!(up_scenes.len() > 1, "the 'ceiling' direction shows pool variety");
    assert!(side_scenes.len() > 1, "the wall directions show pool variety");
}

/// Panels are structure: Static, into the fused room collider.
#[test]
fn panel_assembly_is_all_skin() {
    // Panel plates ARE the boundary plane — render-only skin; the
    // watertight cell shell owns that plane's physics (playtest
    // 2026-07-06: render triangles as collider = the art's holes).
    for p in panels(&sealed_3x3x3(), &[], 7) {
        assert_eq!(p.collision, Collision::Skin,
            "panel {} must be Skin", test_catalog().path(p.scene));
    }
}
