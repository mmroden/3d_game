use super::*;
use crate::room_template::*;

fn small_room() -> RoomTemplate {
    RoomTemplate {

        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ, frame: FrameStyle::Door },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 1, 1],
    }
}

fn room_3x3() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [2, 0, 1], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
            Connector { offset: [1, 0, 0], facing: ConnectorFacing::NegZ, frame: FrameStyle::Door },
            Connector { offset: [1, 0, 2], facing: ConnectorFacing::PosZ, frame: FrameStyle::Door },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [3, 1, 3],
    }
}

fn corridor_ew() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Corridor,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 1, 1],
    }
}

// --- Grid dimensions ---

#[test]
fn grid_dimensions_match_template_extents() {
    let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(grid.extents, [3, 1, 3], "3x1x3 template should produce 3x1x3 grid");
    assert_eq!(grid.cells().len(), 9, "3x1x3 grid should have 9 cells");
}

#[test]
fn small_room_grid_has_one_cell() {
    let grid = CellGrid::new(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(grid.extents, [1, 1, 1]);
    assert_eq!(grid.cells().len(), 1);
}

// --- World center positions ---

#[test]
fn world_centers_at_cell_midpoints() {
    let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    let cell_00 = grid.cell_at(0, 0, 0).expect("cell (0,0,0) should exist");
    assert_eq!(cell_00.world_center, [2.0, 0.0, 2.0],
        "cell (0,0,0) center should be at (2.0, 0.0, 2.0)");

    let cell_21 = grid.cell_at(2, 0, 1).expect("cell (2,0,1) should exist");
    assert_eq!(cell_21.world_center, [10.0, 0.0, 6.0],
        "cell (2,0,1) center should be at (10.0, 0.0, 6.0)");
}

#[test]
fn world_centers_respect_origin_offset() {
    let grid = CellGrid::new(&small_room(), &[], [10.0, 5.0, 20.0], 4.0);
    let cell = grid.cell_at(0, 0, 0).expect("cell should exist");
    assert_eq!(cell.world_center, [12.0, 5.0, 22.0]);
}

// --- Cell kind classification: sealed room ---

#[test]
fn sealed_1x1_room_is_boundary_corner() {
    let grid = CellGrid::new(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
    let cell = grid.cell_at(0, 0, 0).expect("cell should exist");
    assert_eq!(cell.kind, CellKind::BoundaryCorner,
        "1x1 sealed room with 6 perpendicular sealed faces should be BoundaryCorner");
    assert_eq!(cell.sealed_faces.len(), 6,
        "1x1x1 sealed room should have 6 sealed faces (all directions)");
}

#[test]
fn sealed_3x1x3_cell_kinds() {
    // Y-axis faces don't affect classification for prop placement.
    // 3x1x3: 4 XZ-corner cells, 4 XZ-edge cells, 1 XZ-interior cell.
    let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);

    let corners: Vec<_> = grid.cells().iter()
        .filter(|c| c.kind == CellKind::BoundaryCorner)
        .collect();
    assert_eq!(corners.len(), 4,
        "3x1x3: 4 XZ-corners = 4 BoundaryCorner");

    let edges: Vec<_> = grid.cells().iter()
        .filter(|c| c.kind == CellKind::BoundaryEdge)
        .collect();
    assert_eq!(edges.len(), 4,
        "3x1x3: 4 XZ-edge cells = 4 BoundaryEdge");

    let interiors: Vec<_> = grid.cells().iter()
        .filter(|c| c.kind == CellKind::Interior)
        .collect();
    assert_eq!(interiors.len(), 1,
        "3x1x3: center cell [1,0,1] is Interior (only Y faces)");
}

#[test]
fn corner_cell_has_perpendicular_sealed_faces_including_y() {
    let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    // Cell (0,0,0) is at NegX + NegZ corner, plus NegY + PosY (single story)
    let cell = grid.cell_at(0, 0, 0).expect("cell should exist");
    assert_eq!(cell.kind, CellKind::BoundaryCorner);
    assert!(cell.sealed_faces.contains(&ConnectorFacing::NegX));
    assert!(cell.sealed_faces.contains(&ConnectorFacing::NegZ));
    assert!(cell.sealed_faces.contains(&ConnectorFacing::NegY));
    assert!(cell.sealed_faces.contains(&ConnectorFacing::PosY));
    assert_eq!(cell.sealed_faces.len(), 4,
        "single-story XZ-corner cell should have 4 sealed faces (NegX, NegZ, NegY, PosY)");
}

#[test]
fn xz_edge_cell_with_y_faces_is_edge() {
    let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    // Cell (1,0,0) has NegZ sealed + NegY + PosY. Only one XZ axis → BoundaryEdge.
    let cell = grid.cell_at(1, 0, 0).expect("cell should exist");
    assert_eq!(cell.kind, CellKind::BoundaryEdge,
        "single-story XZ-edge cell should be BoundaryEdge, not corner");
    assert!(cell.sealed_faces.contains(&ConnectorFacing::NegZ));
    assert!(cell.sealed_faces.contains(&ConnectorFacing::NegY));
    assert!(cell.sealed_faces.contains(&ConnectorFacing::PosY));
}

#[test]
fn xz_interior_cell_single_story_is_interior() {
    let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    // Cell (1,0,1) has no XZ boundary faces, only NegY + PosY → Interior.
    let cell = grid.cell_at(1, 0, 1).expect("cell should exist");
    assert_eq!(cell.kind, CellKind::Interior,
        "single-story center cell with only Y faces should be Interior");
    assert_eq!(cell.sealed_faces.len(), 2);
}

#[test]
fn multi_story_interior_cell_is_truly_interior() {
    // In a 3x2x3 room, cell (1,_,1) at cy not on boundary has no XZ or Y faces
    // But we need at least 3 stories for a truly interior cell.
    let tall_room = RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [3, 3, 3],
    };
    let grid = CellGrid::new(&tall_room, &[], [0.0, 0.0, 0.0], 4.0);
    let cell = grid.cell_at(1, 1, 1).expect("cell (1,1,1) should exist");
    assert_eq!(cell.kind, CellKind::Interior,
        "center cell in 3x3x3 room should be truly Interior");
    assert!(cell.sealed_faces.is_empty());
}

// --- Active connectors ---

#[test]
fn active_connector_cell_is_connector_gap() {
    // 3x3 room with NegX active at [0,0,1]
    let grid = CellGrid::new(
        &room_3x3(),
        &[Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX, frame: FrameStyle::Door }],
        [0.0, 0.0, 0.0],
        4.0,
    );
    let cell = grid.cell_at(0, 0, 1).expect("cell should exist");
    assert_eq!(cell.kind, CellKind::ConnectorGap,
        "cell at active NegX connector should be ConnectorGap");
}

#[test]
fn corridor_with_both_ends_active_is_connector_gap() {
    let grid = CellGrid::new(
        &corridor_ew(),
        &[
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
        ],
        [0.0, 0.0, 0.0],
        4.0,
    );
    let cell = grid.cell_at(0, 0, 0).expect("cell should exist");
    // Both NegX and PosX are active connectors. NegZ and PosZ are sealed.
    // But since it has active connectors, it's a ConnectorGap.
    assert_eq!(cell.kind, CellKind::ConnectorGap,
        "corridor cell with active connectors should be ConnectorGap");
}

#[test]
fn non_connector_cells_unaffected_by_active_facings() {
    // 3x3 with NegX active — cell (2,0,2) is PosX+PosZ corner, unaffected
    let grid = CellGrid::new(
        &room_3x3(),
        &[Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX, frame: FrameStyle::Door }],
        [0.0, 0.0, 0.0],
        4.0,
    );
    let cell = grid.cell_at(2, 0, 2).expect("cell should exist");
    assert_eq!(cell.kind, CellKind::BoundaryCorner,
        "cell (2,0,2) far from connector should still be BoundaryCorner");
}

// --- Y-axis boundary detection (true 3D) ---

fn multi_story_room() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [2, 0, 1], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
            Connector { offset: [1, 0, 0], facing: ConnectorFacing::NegZ, frame: FrameStyle::Door },
            Connector { offset: [1, 0, 2], facing: ConnectorFacing::PosZ, frame: FrameStyle::Door },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [3, 2, 3],
    }
}

#[test]
fn multi_story_room_has_correct_cell_count() {
    let grid = CellGrid::new(&multi_story_room(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(grid.extents, [3, 2, 3]);
    assert_eq!(grid.cells().len(), 18, "3x2x3 grid should have 18 cells");
}

#[test]
fn bottom_cell_has_negy_sealed_face() {
    let grid = CellGrid::new(&multi_story_room(), &[], [0.0, 0.0, 0.0], 4.0);
    let cell = grid.cell_at(1, 0, 1).expect("cell (1,0,1) should exist");
    assert!(cell.sealed_faces.contains(&ConnectorFacing::NegY),
        "interior cell at cy==0 should have NegY sealed face, got {:?}", cell.sealed_faces);
}

#[test]
fn top_cell_has_posy_sealed_face() {
    let grid = CellGrid::new(&multi_story_room(), &[], [0.0, 0.0, 0.0], 4.0);
    let cell = grid.cell_at(1, 1, 1).expect("cell (1,1,1) should exist");
    assert!(cell.sealed_faces.contains(&ConnectorFacing::PosY),
        "interior cell at cy==ey-1 should have PosY sealed face, got {:?}", cell.sealed_faces);
}

#[test]
fn mid_cell_has_no_y_sealed_faces() {
    // In a 3-story room, the middle floor should have no Y sealed faces
    let tall_room = RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 3, 1],
    };
    let grid = CellGrid::new(&tall_room, &[], [0.0, 0.0, 0.0], 4.0);
    let cell = grid.cell_at(0, 1, 0).expect("cell (0,1,0) should exist");
    assert!(!cell.sealed_faces.contains(&ConnectorFacing::NegY),
        "mid-floor cell should not have NegY sealed");
    assert!(!cell.sealed_faces.contains(&ConnectorFacing::PosY),
        "mid-floor cell should not have PosY sealed");
}

#[test]
fn posy_active_connector_prevents_sealed_face() {
    let room_with_vertical_connector = RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 1, 0], facing: ConnectorFacing::PosY, frame: FrameStyle::Door },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 2, 1],
    };
    let grid = CellGrid::new(
        &room_with_vertical_connector,
        &[Connector { offset: [0, 1, 0], facing: ConnectorFacing::PosY, frame: FrameStyle::Door }],
        [0.0, 0.0, 0.0],
        4.0,
    );
    let cell = grid.cell_at(0, 1, 0).expect("cell (0,1,0) should exist");
    assert!(!cell.sealed_faces.contains(&ConnectorFacing::PosY),
        "active PosY connector should prevent PosY sealed face");
    assert_eq!(cell.kind, CellKind::ConnectorGap,
        "cell with active vertical connector should be ConnectorGap");
}

// --- Aperture alignment: unwired Y-level must be sealed ---

#[test]
fn tall_room_unwired_y1_connector_is_not_gap() {
    // A 3x2x3 room has NegX connectors at y=0 and y=1.
    // If only y=0 is wired, cell [0,1,1] should be sealed, not ConnectorGap.
    let template = RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [0, 1, 1], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [3, 2, 3],
    };
    // Only the y=0 connector is active.
    let active = &[Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX, frame: FrameStyle::Door }];
    let grid = CellGrid::new(&template, active, [0.0, 0.0, 0.0], 4.0);

    // y=0 cell should be ConnectorGap (it IS wired)
    let y0 = grid.cell_at(0, 0, 1).expect("cell (0,0,1)");
    assert_eq!(y0.kind, CellKind::ConnectorGap,
        "y=0 NegX connector cell should be ConnectorGap");

    // y=1 cell should NOT be ConnectorGap (it is NOT wired)
    let y1 = grid.cell_at(0, 1, 1).expect("cell (0,1,1)");
    assert_ne!(y1.kind, CellKind::ConnectorGap,
        "y=1 NegX connector cell should be sealed — only y=0 is wired");
}

// --- Populate tests ---

#[test]
fn dense_populate_fills_majority_of_cells() {
    use crate::room_theme::THEME_WAREHOUSE;
    // THEME_WAREHOUSE has Dense density.
    let mut grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    grid.populate(&THEME_WAREHOUSE, 42);
    let occupied = grid.cells().iter()
        .filter(|c| matches!(c.occupant, CellOccupant::Props(_)))
        .count();
    // Dense = ~60-80% fill. 9 cells → at least 5 occupied.
    assert!(occupied >= 5,
        "dense populate should fill ≥5 of 9 cells, got {occupied}");
}

#[test]
fn sparse_populate_leaves_most_cells_empty() {
    use crate::room_theme::{RoomTheme, PALETTE_GENERIC};
    use crate::room_furnisher::RoomDensity;
    let sparse_theme = RoomTheme {
        name: "test_sparse",
        wall_set: &crate::asset_catalog::WALL_SET_ASTRA,
        palette: &PALETTE_GENERIC,
        density: RoomDensity::Sparse,
    };
    let mut grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    grid.populate(&sparse_theme, 42);
    let occupied = grid.cells().iter()
        .filter(|c| matches!(c.occupant, CellOccupant::Props(_)))
        .count();
    // Sparse = ~12-20%. 9 cells → at most 4 occupied.
    assert!(occupied <= 4,
        "sparse populate should fill ≤4 of 9 cells, got {occupied}");
}

#[test]
fn connector_gap_cells_never_occupied() {
    use crate::room_theme::THEME_WAREHOUSE;
    let mut grid = CellGrid::new(
        &room_3x3(),
        &[Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX, frame: FrameStyle::Door }],
        [0.0, 0.0, 0.0],
        4.0,
    );
    grid.populate(&THEME_WAREHOUSE, 42);
    let gap_occupied = grid.cells().iter()
        .filter(|c| c.kind == CellKind::ConnectorGap)
        .any(|c| matches!(c.occupant, CellOccupant::Props(_)));
    assert!(!gap_occupied, "ConnectorGap cells should never have props");
}

#[test]
fn no_cell_has_more_than_one_occupant() {
    use crate::room_theme::THEME_WAREHOUSE;
    let mut grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    grid.populate(&THEME_WAREHOUSE, 42);
    // Each cell is either Empty or Prop — by type system this is guaranteed,
    // but verify prop_placements count matches occupied cell count.
    let occupied = grid.cells().iter()
        .filter(|c| matches!(c.occupant, CellOccupant::Props(_)))
        .count();
    assert_eq!(grid.prop_placements().len(), occupied,
        "prop_placements count should match occupied cell count");
}

#[test]
fn populate_uses_themed_props() {
    use crate::room_theme::THEME_WAREHOUSE;
    use crate::asset_catalog;
    let mut grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    grid.populate(&THEME_WAREHOUSE, 42);
    let placements = grid.prop_placements();
    // All placed props should come from the warehouse palette.
    let warehouse_scenes: Vec<&str> = asset_catalog::WAREHOUSE_WALL_PROPS.iter()
        .chain(asset_catalog::WAREHOUSE_CENTER_PROPS)
        .chain(asset_catalog::CORNER_PROPS)
        .chain(asset_catalog::CEILING_PROPS)
        .map(|p| p.scene)
        .collect();
    for p in &placements {
        assert!(warehouse_scenes.contains(&p.scene),
            "prop '{}' not in warehouse palette", p.scene);
    }
}

/// Column props in multi-story rooms must be stacked at each story level.
#[test]
fn columns_stacked_in_multi_story_room() {
    use crate::room_theme::THEME_WAREHOUSE;
    let template = RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [3, 2, 3],
    };
    // Try many seeds until we get at least one column placement.
    let mut found_stacked = false;
    for seed in 0..200 {
        let mut grid = CellGrid::new(&template, &[], [0.0, 0.0, 0.0], 4.0);
        grid.populate(&THEME_WAREHOUSE, seed);
        let placements = grid.prop_placements();
        let columns: Vec<_> = placements.iter()
            .filter(|p| p.scene.contains("/columns/"))
            .collect();
        if columns.is_empty() {
            continue;
        }
        // For each column, there should be a matching column at the next story level.
        let story_height = CellGrid::DEFAULT_STORY_HEIGHT;
        for col in &columns {
            let has_pair = columns.iter().any(|other| {
                other.scene == col.scene
                    && (other.position[0] - col.position[0]).abs() < 0.01
                    && (other.position[2] - col.position[2]).abs() < 0.01
                    && other.position[1] != col.position[1]
                    && ((other.position[1] - col.position[1]).abs() - story_height).abs() < 0.01
            });
            assert!(
                has_pair,
                "column at {:?} should have a matching column one story above/below (story_height={})",
                col.position, story_height
            );
        }
        found_stacked = true;
        break;
    }
    assert!(found_stacked, "should find at least one stacked column across 200 seeds");
}

#[test]
fn single_story_5x5_has_interior_cells() {
    let room = RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [5, 1, 5],
    };
    let grid = CellGrid::new(&room, &[], [0.0, 0.0, 0.0], 4.0);
    let interior_count = grid.cells().iter()
        .filter(|c| c.kind == CellKind::Interior)
        .count();
    // 5x5 room: the 3x3 inner cells should be Interior (9 cells)
    assert!(interior_count >= 1,
        "single-story 5x5 should have ≥1 Interior cell, got {interior_count}");
}

#[test]
fn center_props_placed_in_single_story_room() {
    use crate::room_theme::THEME_GENERIC;
    let room = RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [5, 1, 5],
    };
    let center_scenes: std::collections::HashSet<&str> = crate::asset_catalog::CENTER_PROPS
        .iter().map(|p| p.scene).collect();
    let mut found_center = false;
    for seed in 0..100 {
        let mut grid = CellGrid::new(&room, &[], [0.0, 0.0, 0.0], 4.0);
        grid.populate(&THEME_GENERIC, seed);
        if grid.prop_placements().iter().any(|p| center_scenes.contains(p.scene)) {
            found_center = true;
            break;
        }
    }
    assert!(found_center, "center props should appear in single-story rooms across 100 seeds");
}

#[test]
fn no_column_near_connector_gap_in_generated_rooms() {
    // Test with procedurally generated rooms — the actual production path.
    use crate::generator::{generate_room, GeneratorConfig};
    use crate::room_theme::THEME_WAREHOUSE;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    let config = GeneratorConfig {
        seed: crate::seed::Seed::new(42),
        max_rooms: 10,
        min_room_xz: 3,
        max_room_xz: 6,
        min_room_y: 1,
        max_room_y: 3,
    };

    for room_seed in 0..50 {
        let mut rng = SmallRng::seed_from_u64(room_seed);
        let room = generate_room(&mut rng, &config);

        // Activate a random subset of connectors (simulating connected rooms)
        let active: Vec<_> = room.connectors.iter()
            .enumerate()
            .filter(|(i, _)| i % 2 == 0)
            .map(|(_, c)| *c)
            .collect();

        for populate_seed in 0..20 {
            let mut grid = CellGrid::new(&room, &active, [0.0, 0.0, 0.0], 4.0);
            grid.populate(&THEME_WAREHOUSE, populate_seed);

            let gap_positions: std::collections::HashSet<[i32; 3]> = grid.cells().iter()
                .filter(|c| c.kind == CellKind::ConnectorGap)
                .map(|c| c.grid_pos)
                .collect();

            for cell in grid.cells() {
                if let CellOccupant::Props(ref props) = cell.occupant {
                    for p in props {
                        if p.scene.contains("/columns/") {
                            let near = gap_positions.iter().any(|gap| {
                                let dx = (gap[0] - cell.grid_pos[0]).abs();
                                let dz = (gap[2] - cell.grid_pos[2]).abs();
                                gap[1] == cell.grid_pos[1] && dx <= 2 && dz <= 2
                            });
                            assert!(
                                !near,
                                "room_seed {room_seed}, pop_seed {populate_seed}: \
                                 column at {:?} within 2 cells of ConnectorGap",
                                cell.grid_pos
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn no_column_adjacent_to_connector_gap() {
    // 3x1x3 room with active NegX connector at [0,0,1].
    // Cell [0,0,0] is a BoundaryCorner adjacent to the gap.
    // No column should be placed there regardless of seed.
    use crate::room_theme::THEME_WAREHOUSE;
    let active = vec![Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX, frame: FrameStyle::Door }];
    for seed in 0..200 {
        let mut grid = CellGrid::new(&room_3x3(), &active, [0.0, 0.0, 0.0], 4.0);
        grid.populate(&THEME_WAREHOUSE, seed);
        for cell in grid.cells() {
            if let CellOccupant::Props(ref props) = cell.occupant {
                for p in props {
                    if p.scene.contains("/columns/") {
                        // Check if this cell is XZ-adjacent to a ConnectorGap
                        let [cx, _, cz] = cell.grid_pos;
                        let adjacent_to_gap = grid.cells().iter().any(|other| {
                            other.kind == CellKind::ConnectorGap
                                && ((other.grid_pos[0] - cx).abs() + (other.grid_pos[2] - cz).abs() == 1)
                                && other.grid_pos[1] == cell.grid_pos[1]
                        });
                        assert!(
                            !adjacent_to_gap,
                            "seed {seed}: column at cell {:?} is adjacent to ConnectorGap",
                            cell.grid_pos
                        );
                    }
                }
            }
        }
    }
}
