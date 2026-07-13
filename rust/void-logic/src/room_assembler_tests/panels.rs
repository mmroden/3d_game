use super::*;
use crate::asset_catalog::PANEL_SET_VOL01;
use crate::cell::CellGrid;

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
    assemble_panels_from_grid(&grid, &PANEL_SET_VOL01, seed)
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

    // Every panel sits ON a face plane of the 9×9×9 m room volume: for the
    // axis it faces along, its coordinate is a multiple of the pitch.
    for p in &placements {
        let on_plane = p.position.iter().any(|c| (c / P - (c / P).round()).abs() < 1e-4);
        assert!(on_plane, "panel floats off every face plane: {:?}", p.position);
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
    let mut down_scenes: HashSet<&str> = HashSet::new(); // faces with rot (0,0)
    let mut up_scenes: HashSet<&str> = HashSet::new(); // rot (π,0)
    let mut side_scenes: HashSet<&str> = HashSet::new(); // |rot_x| = π/2
    for seed in 0..12 {
        for p in panels(&sealed_3x3x3(), &[], seed) {
            assert!(PANEL_SET_VOL01.panels.contains(&p.scene),
                "{} is not in the panel pool", p.scene);
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
            "panel {} must be Skin", p.scene);
    }
}
