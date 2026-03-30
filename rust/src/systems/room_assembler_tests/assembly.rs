use super::*;

// ==========================================================================
// Assembled room structure tests — counts, positions, orientation, bounds,
// connectors, wall overlap, and vertical hatches.
// ==========================================================================

// --- Structural invariants (no hardcoded positions) ---

/// Every sealed boundary cell must emit either a wall or corner piece.
/// This is a structural invariant: we check that the cell grid has geometry
/// at every sealed face, regardless of what world position that maps to.
#[test]
fn every_sealed_boundary_has_wall_or_corner() {
    use crate::systems::cell::CellGrid;

    let test_cases: Vec<(RoomTemplate, Vec<ConnectorFacing>)> = vec![
        (small_room(), vec![]),
        (room_3x3(), vec![]),
        (large_room(), vec![]),
        (room_3x3(), vec![ConnectorFacing::NegX, ConnectorFacing::PosZ]),
        (corridor_ew(), vec![ConnectorFacing::NegX, ConnectorFacing::PosX]),
    ];

    for (template, active) in &test_cases {
        let placements = assemble_default(template, active, [0.0, 0.0, 0.0], 4.0);
        let grid = CellGrid::new(template, active, [0.0, 0.0, 0.0], 4.0);

        for cell in grid.cells() {
            let near_center = |p: &MeshPlacement| {
                // Corner pieces are offset from cell center by up to INTERIOR_HALF (2.0m).
                (p.position[0] - cell.world_center[0]).abs() < 2.1
                    && (p.position[1] - cell.world_center[1]).abs() < 0.001
                    && (p.position[2] - cell.world_center[2]).abs() < 2.1
            };

            if !cell.sealed_faces.is_empty() {
                let has_wall = placements.iter().any(|p| p.scene == WALL && near_center(p));
                let has_corner = placements.iter().any(|p| p.scene == CORNER && near_center(p));
                assert!(
                    has_wall || has_corner,
                    "room '{}' cell {:?}: sealed boundary should have WALL or CORNER",
                    template.id, cell.grid_pos
                );
            }
        }
    }
}

/// Origin offset shifts all placements by the same amount.
#[test]
fn origin_offset_shifts_all_placements() {
    let origin_a = [0.0, 0.0, 0.0];
    let origin_b = [10.0, 5.0, 20.0];
    let placements_a = assemble_default(&room_3x3(), &[], origin_a, 4.0);
    let placements_b = assemble_default(&room_3x3(), &[], origin_b, 4.0);

    assert_eq!(placements_a.len(), placements_b.len());
    for (a, b) in placements_a.iter().zip(placements_b.iter()) {
        assert_eq!(a.scene, b.scene);
        assert!(
            (b.position[0] - a.position[0] - 10.0).abs() < 0.001,
            "X offset mismatch: {:?} vs {:?}", a.position, b.position
        );
        assert!(
            (b.position[1] - a.position[1] - 5.0).abs() < 0.001,
            "Y offset mismatch: {:?} vs {:?}", a.position, b.position
        );
        assert!(
            (b.position[2] - a.position[2] - 20.0).abs() < 0.001,
            "Z offset mismatch: {:?} vs {:?}", a.position, b.position
        );
    }
}

// --- Count-based tests ---

#[test]
fn sealed_small_room_wall_and_corner_counts() {
    // In a 1x1 room, all 4 faces participate in corners.
    // Corner cells emit only corner pieces, no straight walls.
    let placements = assemble_default(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count_floors(&placements, 0.0), 1);
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 1);
    assert_eq!(count(&placements, WALL), 0, "corner cells emit no straight walls");
    assert_eq!(count(&placements, CEILING), 0, "corner cells emit no straight ceiling strips");
    assert_eq!(count(&placements, CORNER), 4);
    assert_eq!(count(&placements, DOOR), 0);
}

#[test]
fn room_active_connector_leaves_gap_no_door() {
    let placements = assemble_default(
        &small_room(),
        &[ConnectorFacing::PosX],
        [0.0, 0.0, 0.0],
        4.0,
    );
    // With PosX open: NegX+NegZ corner, NegX+PosZ corner → 2 corners.
    // No perpendicular pair without PosX, so no straight walls at corner cells.
    // But NegX still appears as a sealed face — it's part of both corners though.
    // The single cell has NegX, NegZ, PosZ sealed. NegX+NegZ and NegX+PosZ are both corners.
    // All faces participate in corners → 0 straight walls.
    assert_eq!(count(&placements, WALL), 0, "all sealed faces are part of corners");
    assert_eq!(count(&placements, CEILING), 0, "all sealed faces are part of corners");
    assert_eq!(count(&placements, DOOR), 0, "rooms should NOT emit door frames");
    assert_eq!(count(&placements, CORNER), 2, "corners only where two walls meet");
}

#[test]
fn room_two_active_connectors_leave_gaps() {
    let placements = assemble_default(
        &small_room(),
        &[ConnectorFacing::PosX, ConnectorFacing::PosZ],
        [0.0, 0.0, 0.0],
        4.0,
    );
    // With PosX+PosZ open: NegX+NegZ corner → 1 corner, no straight walls
    // (both sealed faces participate in the corner).
    assert_eq!(count(&placements, WALL), 0, "all sealed faces are part of the corner");
    assert_eq!(count(&placements, DOOR), 0, "rooms should NOT emit door frames");
    assert_eq!(count(&placements, CORNER), 1);
}

#[test]
fn corridor_active_connectors_emit_door_frames() {
    let placements = assemble_default(
        &corridor_ew(),
        &[ConnectorFacing::PosX, ConnectorFacing::NegX],
        [0.0, 0.0, 0.0],
        4.0,
    );
    assert_eq!(count(&placements, DOOR), 2, "corridors emit door frames");
    assert_eq!(count(&placements, WALL), 2, "NegZ and PosZ walls remain");
}

#[test]
fn corridor_with_both_ends_active() {
    let placements = assemble_default(
        &corridor_ew(),
        &[ConnectorFacing::NegX, ConnectorFacing::PosX],
        [0.0, 0.0, 0.0],
        4.0,
    );
    assert_eq!(count_floors(&placements, 0.0), 1);
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 1);
    assert_eq!(count(&placements, DOOR), 2, "doors at both ends");
    assert_eq!(count(&placements, WALL), 2, "walls on NegZ and PosZ sides");
    assert_eq!(count(&placements, CEILING), 2);
    assert_eq!(count(&placements, CORNER), 0, "no corners — no two walls meet");
}

#[test]
fn large_room_sealed_has_4_floors_4_ceilings() {
    let placements = assemble_default(&large_room(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count_floors(&placements, 0.0), 4, "2x2 = 4 floor tiles");
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 4, "2x2 = 4 ceiling tiles");
}

#[test]
fn large_room_sealed_walls() {
    // 2x2 sealed: 4 corners × 0 straight walls + 0 edge cells = 0 straight walls.
    // Every cell in a 2x2 is a corner (each has 2 perpendicular sealed faces).
    let placements = assemble_default(&large_room(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count(&placements, WALL), 0, "2x2: all cells are corners, no straight walls");
    assert_eq!(count(&placements, CORNER), 4, "one corner per cell");
}

#[test]
fn large_room_interior_edges_have_no_walls_at_interior() {
    let placements = assemble_default(&large_room(), &[], [0.0, 0.0, 0.0], 4.0);
    // Interior edges (between cells of same room) should have nothing.
    // 2x2: all 4 cells are corners → 0 straight walls.
    assert_eq!(count(&placements, WALL), 0, "2x2 sealed: all corners, 0 straight walls");
}

#[test]
fn large_room_one_connector_active() {
    // large_room NegX connector at [0,0,0]. With NegX active:
    // Cell (0,0): ConnectorGap (NegX active), NegZ sealed but no perpendicular pair → edge, 1 wall
    // Cell (1,0): PosX+NegZ corner → 0 straight walls
    // Cell (0,1): NegX+PosZ corner (NegX sealed here since connector is at [0,0,0]) → 0 straight walls
    // Cell (1,1): PosX+PosZ corner → 0 straight walls
    // Total: 1 straight wall
    let placements = assemble_default(
        &large_room(),
        &[ConnectorFacing::NegX],
        [0.0, 0.0, 0.0],
        4.0,
    );
    assert_eq!(count(&placements, DOOR), 0, "rooms leave gaps, no door frames");
    assert_eq!(count(&placements, WALL), 1, "only edge cells get straight walls");
}

#[test]
fn room_active_connector_leaves_gap_not_archway() {
    let placements = assemble_default(
        &room_3x3(),
        &[ConnectorFacing::NegX],
        [0.0, 0.0, 0.0],
        4.0,
    );
    // 3x3 with NegX active at [0,0,1]: gap at (0,1). 4 corner cells emit no straight walls.
    // Edge cells: (1,0) NegZ, (2,1) PosX, (1,2) PosZ → 3 straight walls.
    assert_eq!(count(&placements, DOOR), 0, "rooms leave gaps, no door frames");
    assert_eq!(count(&placements, WALL), 3, "3 edge cells get straight walls, corners don't");
}

// --- Orientation tests ---

#[test]
fn floor_tiles_face_upward() {
    let placements = assemble_default(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    for p in &placements {
        if is_floor_scene(p.scene) && p.position[1].abs() < 0.001 {
            assert!(
                p.rotation_x.abs() < 0.001,
                "floor tile at {:?} has rotation_x {}, expected 0.0",
                p.position, p.rotation_x
            );
        }
    }
}

#[test]
fn ceiling_tiles_face_downward() {
    let placements = assemble_default(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    let ceiling_tiles: Vec<_> = placements.iter()
        .filter(|p| is_floor_scene(p.scene) && (p.position[1] - CELL_HEIGHT).abs() < 0.001)
        .collect();
    assert!(!ceiling_tiles.is_empty(), "should have ceiling tiles");
    for p in &ceiling_tiles {
        assert!(
            (p.rotation_x - PI).abs() < 0.001,
            "ceiling tile at {:?} has rotation_x {}, expected PI ({})",
            p.position, p.rotation_x, PI
        );
    }
}

#[test]
fn sealed_room_has_one_ceiling_tile_per_cell() {
    let placements = assemble_default(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 1);
}

#[test]
fn sealed_3x3_room_has_9_ceiling_tiles() {
    let placements = assemble_default(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 9);
}

#[test]
fn sealed_3x3_room_full_surface_coverage() {
    let placements = assemble_default(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count_floors(&placements, 0.0), 9, "3x3 = 9 floor tiles");
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 9, "3x3 = 9 ceiling tiles");
    // 4 edge cells × 1 wall each = 4 straight walls. Corner cells emit no straight walls.
    assert_eq!(count(&placements, WALL), 4, "4 edge cells get straight walls, corners don't");
    assert_eq!(count(&placements, CEILING), 4, "4 edge cells get straight ceiling strips");
    assert_eq!(count(&placements, CORNER), 4, "4 external corners");
}

// --- Vertical connector tests ---

#[test]
fn posy_connector_replaces_ceiling_with_archway() {
    let placements = assemble_default(
        &hub_6way(),
        &[ConnectorFacing::PosY],
        [0.0, 0.0, 0.0],
        4.0,
    );
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 0);
    assert_eq!(count_floors(&placements, 0.0), 1, "floor should remain");
    let vertical_archways: Vec<_> = placements.iter()
        .filter(|p| p.scene == DOOR && p.position[1] > 0.0)
        .collect();
    assert_eq!(vertical_archways.len(), 1, "one archway on top for PosY");
}

#[test]
fn negy_connector_replaces_floor_with_archway() {
    let placements = assemble_default(
        &hub_6way(),
        &[ConnectorFacing::NegY],
        [0.0, 0.0, 0.0],
        4.0,
    );
    assert_eq!(count_floors(&placements, 0.0), 0);
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 1, "ceiling should remain");
}

#[test]
fn multi_story_room_geometry_fits_within_height() {
    let two_story = RoomTemplate {
        id: "test_2story",
        kind: TemplateKind::Room,
        connectors: vec![],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 2, 1],
    };
    let ch = 5.0_f32;
    let placements = assemble_default(&two_story, &[], [0.0, 0.0, 0.0], 4.0);
    let max_y = 2.0 * ch;

    for p in &placements {
        assert!(
            p.position[1] >= 0.0 && p.position[1] <= max_y,
            "2-story room: mesh at {:?} exceeds Y bounds [0, {max_y}]", p.position
        );
    }

    let floor_count = count_floors(&placements, 0.0);
    assert_eq!(floor_count, 1, "bottom floor at Y=0");
    let ceiling_count = count_ceiling_tiles(&placements, 0.0, max_y);
    assert_eq!(ceiling_count, 1, "ceiling at Y=10 (2 * CELL_HEIGHT)");
}

// --- Wall overlap between adjacent rooms ---

#[test]
fn adjacent_rooms_walls_do_not_overlap() {
    let room_a_placements = assemble_default(
        &small_room(),
        &[ConnectorFacing::PosX],
        [0.0, 0.0, 0.0],
        4.0,
    );
    let room_b_placements = assemble_default(
        &small_room(),
        &[ConnectorFacing::NegX],
        [4.0, 0.0, 0.0],
        4.0,
    );

    let walls_a: Vec<([f32; 3], i32)> = room_a_placements.iter()
        .filter(|p| p.scene == WALL)
        .map(|p| (p.position, (p.rotation_y * 1000.0) as i32))
        .collect();
    let walls_b: Vec<([f32; 3], i32)> = room_b_placements.iter()
        .filter(|p| p.scene == WALL)
        .map(|p| (p.position, (p.rotation_y * 1000.0) as i32))
        .collect();

    for (pos_a, rot_a) in &walls_a {
        for (pos_b, rot_b) in &walls_b {
            assert!(
                pos_a != pos_b || rot_a != rot_b,
                "wall from room A at {pos_a:?} (rot {rot_a}) overlaps identical wall from room B"
            );
        }
    }
}

// --- Hatch orientation ---

#[test]
fn posy_hatch_lays_flat() {
    let placements = assemble_default(
        &hub_6way(),
        &[ConnectorFacing::PosY],
        [0.0, 0.0, 0.0],
        4.0,
    );
    let hatch: Vec<_> = placements.iter()
        .filter(|p| p.scene == DOOR && (p.position[1] - CELL_HEIGHT).abs() < 0.001)
        .collect();
    assert_eq!(hatch.len(), 1, "should have one PosY hatch");
    assert!(
        (hatch[0].rotation_x - FRAC_PI_2).abs() < 0.001,
        "PosY hatch should have rotation_x PI/2, got {}",
        hatch[0].rotation_x
    );
}

#[test]
fn negy_hatch_lays_flat() {
    let placements = assemble_default(
        &hub_6way(),
        &[ConnectorFacing::NegY],
        [0.0, 0.0, 0.0],
        4.0,
    );
    let hatch: Vec<_> = placements.iter()
        .filter(|p| p.scene == DOOR && p.position[1].abs() < 0.001)
        .collect();
    assert_eq!(hatch.len(), 1, "should have one NegY hatch");
    assert!(
        (hatch[0].rotation_x - (-FRAC_PI_2)).abs() < 0.001,
        "NegY hatch should have rotation_x -PI/2, got {}",
        hatch[0].rotation_x
    );
}

// --- Ceiling strip mesh selection ---

/// TopCables meshes have a ~0.31 unit gap above wall tops (Y≈3.33 vs wall Y≈3.02).
/// Other Top* meshes (TopAstra, TopPlates, etc.) start at Y≈3.0, naturally overlapping
/// with wall tops. Astra wall set must NOT use TopCables to avoid visible gaps.
#[test]
fn astra_ceiling_does_not_use_topcables() {
    assert!(
        !asset_catalog::WALL_SET_ASTRA.straight.ceiling.contains("TopCables"),
        "Astra straight ceiling should not use TopCables (gap at Y≈3.33), got '{}'",
        asset_catalog::WALL_SET_ASTRA.straight.ceiling
    );
    assert!(
        !asset_catalog::WALL_SET_ASTRA.corner_inner.ceiling.contains("TopCables"),
        "Astra corner_inner ceiling should not use TopCables (gap at Y≈3.33), got '{}'",
        asset_catalog::WALL_SET_ASTRA.corner_inner.ceiling
    );
    assert!(
        !asset_catalog::WALL_SET_ASTRA.corner_outer.ceiling.contains("TopCables"),
        "Astra corner_outer ceiling should not use TopCables (gap at Y≈3.33), got '{}'",
        asset_catalog::WALL_SET_ASTRA.corner_outer.ceiling
    );
}
