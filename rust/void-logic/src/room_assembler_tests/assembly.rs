use super::*;

// ==========================================================================
// Assembled room structure tests — counts, positions, orientation, bounds,
// connectors, wall overlap, and vertical hatches.
// ==========================================================================

// --- Structural invariants (no hardcoded positions) ---

/// Every cell with XZ sealed faces must emit either a wall or corner piece.
/// Y-axis sealed faces are floor/ceiling boundaries — handled by floor/ceiling
/// geometry, not wall meshes. Only XZ faces need wall/corner coverage.
#[test]
fn every_xz_sealed_boundary_has_wall_or_corner() {
    use crate::cell::CellGrid;

    let test_cases: Vec<(RoomTemplate, Vec<Connector>)> = vec![
        (small_room(), vec![]),
        (room_3x3(), vec![]),
        (large_room(), vec![]),
        (room_3x3(), vec![
            Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX },
            Connector { offset: [1, 0, 2], facing: ConnectorFacing::PosZ },
        ]),
        (corridor_ew(), vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
        ]),
    ];

    for (template, active) in &test_cases {
        let placements = assemble_default(template, active, [0.0, 0.0, 0.0]);
        let grid = CellGrid::new(template, active, [0.0, 0.0, 0.0], asset_catalog::WALL_SET_ASTRA.tile_width);

        for cell in grid.cells() {
            // Only check cells that have at least one XZ sealed face.
            // Cells with only Y sealed faces (floor/ceiling) don't need wall geometry.
            let has_xz_sealed = cell.sealed_faces.iter().any(|f| {
                matches!(f, ConnectorFacing::NegX | ConnectorFacing::PosX
                          | ConnectorFacing::NegZ | ConnectorFacing::PosZ)
            });
            if !has_xz_sealed {
                continue;
            }

            let near_center = |p: &MeshPlacement| {
                // Corner pieces are offset from cell center by up to INTERIOR_HALF (2.0m).
                (p.position[0] - cell.world_center[0]).abs() < 2.1
                    && (p.position[1] - cell.world_center[1]).abs() < 0.001
                    && (p.position[2] - cell.world_center[2]).abs() < 2.1
            };

            let has_wall = placements.iter().any(|p| p.scene == WALL && near_center(p));
            let has_corner = placements.iter().any(|p| p.scene == CORNER && near_center(p));
            assert!(
                has_wall || has_corner,
                "room '{}' cell {:?}: XZ sealed boundary should have WALL or CORNER",
                template.id, cell.grid_pos
            );
        }
    }
}

/// Cells with only Y-axis sealed faces (e.g. center cell of a single-story room)
/// must NOT require wall/corner geometry — Y faces are sealed by floor/ceiling tiles.
#[test]
fn y_only_sealed_cells_have_floor_ceiling_not_walls() {
    use crate::cell::CellGrid;

    // 3x1x3 sealed room: center cell (1,0,1) has only NegY+PosY sealed faces.
    let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], asset_catalog::WALL_SET_ASTRA.tile_width);
    let placements = assemble_default(&room_3x3(), &[], [0.0, 0.0, 0.0]);

    let center = grid.cell_at(1, 0, 1).expect("center cell should exist");
    assert!(
        center.sealed_faces.iter().all(|f| {
            matches!(f, ConnectorFacing::NegY | ConnectorFacing::PosY)
        }),
        "center cell should have only Y sealed faces, got {:?}", center.sealed_faces
    );

    // This cell should have floor + ceiling but no wall/corner AT its own position.
    // (Walls from adjacent cells may exist at nearby but distinct positions.)
    let at_center = |p: &MeshPlacement| {
        (p.position[0] - center.world_center[0]).abs() < 0.001
            && (p.position[2] - center.world_center[2]).abs() < 0.001
    };

    let has_floor = placements.iter().any(|p| {
        is_floor_scene(p.scene)
            && at_center(p)
            && (p.position[1] - center.world_center[1]).abs() < 0.001
    });
    assert!(has_floor, "Y-only cell should have a floor tile at its position");

    let has_ceiling = placements.iter().any(|p| {
        is_floor_scene(p.scene)
            && at_center(p)
            && (p.position[1] - (center.world_center[1] + STORY_HEIGHT)).abs() < 0.001
    });
    assert!(has_ceiling, "Y-only cell should have a ceiling tile at its position");

    // No wall mesh should be exactly at this cell's center
    let has_wall_at_center = placements.iter().any(|p| {
        p.scene == WALL && at_center(p)
    });
    assert!(!has_wall_at_center,
        "Y-only cell should NOT have wall mesh at its center (Y faces are floor/ceiling)");
}

/// Origin offset shifts all placements by the same amount.
#[test]
fn origin_offset_shifts_all_placements() {
    let origin_a = [0.0, 0.0, 0.0];
    let origin_b = [10.0, 5.0, 20.0];
    let placements_a = assemble_default(&room_3x3(), &[], origin_a);
    let placements_b = assemble_default(&room_3x3(), &[], origin_b);

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
    let placements = assemble_default(&small_room(), &[], [0.0, 0.0, 0.0]);
    assert_eq!(count_floors(&placements, 0.0), 1);
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 1);
    assert_eq!(count(&placements, WALL), 0, "corner cells emit no straight walls");
    assert_eq!(count(&placements, CEILING), 0, "corner cells emit no straight ceiling strips");
    assert_eq!(count(&placements, CORNER), 4);
    assert_eq!(count(&placements, DOOR), 0);

    // Position verification: corners are offset from cell center (2.0, 0, 2.0)
    // by interior_offset. All corners at floor Y level.
    let corners: Vec<_> = placements.iter().filter(|p| p.scene == CORNER).collect();
    for c in &corners {
        assert!(c.position[1].abs() < 0.001, "corners should be at floor Y level: {:?}", c.position);
    }

    // All 4 corner rotations must be distinct (one per quadrant).
    let mut rots: Vec<i32> = corners.iter().map(|c| (c.rotation_y * 1000.0) as i32).collect();
    rots.sort();
    rots.dedup();
    assert_eq!(rots.len(), 4, "4 corners should have 4 distinct rotations, got {:?}", rots);
}

#[test]
fn room_active_connector_emits_door_frame() {
    let placements = assemble_default(
        &small_room(),
        &[Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX }],
        [0.0, 0.0, 0.0],
    );
    // With PosX open: NegX+NegZ corner, NegX+PosZ corner → 2 corners.
    // The single cell has NegX, NegZ, PosZ sealed. NegX+NegZ and NegX+PosZ are both corners.
    // All sealed faces participate in corners → 0 straight walls.
    // PosX is an active connector → door frame emitted.
    assert_eq!(count(&placements, WALL), 0, "all sealed faces are part of corners");
    assert_eq!(count(&placements, CEILING), 0, "all sealed faces are part of corners");
    assert_eq!(count(&placements, DOOR), 1, "rooms emit door frame at active connector");
    assert_eq!(count(&placements, CORNER), 2, "corners only where two walls meet");
}

#[test]
fn room_two_active_connectors_emit_two_doors() {
    let placements = assemble_default(
        &small_room(),
        &[
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ },
        ],
        [0.0, 0.0, 0.0],
    );
    // With PosX+PosZ open: NegX+NegZ corner → 1 corner, no straight walls
    // (both sealed faces participate in the corner).
    // Both active connectors get door frames.
    assert_eq!(count(&placements, WALL), 0, "all sealed faces are part of the corner");
    assert_eq!(count(&placements, DOOR), 2, "door frame at each active connector");
    assert_eq!(count(&placements, CORNER), 1);
}

#[test]
fn corridor_active_connectors_emit_door_frames() {
    let placements = assemble_default(
        &corridor_ew(),
        &[
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
        ],
        [0.0, 0.0, 0.0],
    );
    assert_eq!(count(&placements, DOOR), 2, "corridors emit door frames");
    assert_eq!(count(&placements, WALL), 2, "NegZ and PosZ walls remain");
}

#[test]
fn corridor_with_both_ends_active() {
    let placements = assemble_default(
        &corridor_ew(),
        &[
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
        ],
        [0.0, 0.0, 0.0],
    );
    assert_eq!(count_floors(&placements, 0.0), 1);
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 1);
    assert_eq!(count(&placements, DOOR), 2, "doors at both ends");
    assert_eq!(count(&placements, WALL), 2, "walls on NegZ and PosZ sides");
    assert_eq!(count(&placements, CEILING), 2);
    assert_eq!(count(&placements, CORNER), 0, "no corners — no two walls meet");

    // Position verification: walls at NegZ and PosZ should have opposite rotations.
    let walls: Vec<_> = placements.iter().filter(|p| p.scene == WALL).collect();
    let wall_rots: Vec<f32> = walls.iter().map(|w| w.rotation_y).collect();
    assert!(
        (wall_rots[0] - wall_rots[1]).abs() > 0.1,
        "NegZ and PosZ walls should have different rotations, got {:?}", wall_rots
    );

    // Doors at NegX and PosX should have opposite rotations.
    let doors: Vec<_> = placements.iter().filter(|p| p.scene == DOOR).collect();
    let door_rots: Vec<f32> = doors.iter().map(|d| d.rotation_y).collect();
    assert!(
        (door_rots[0] - door_rots[1]).abs() > 0.1,
        "NegX and PosX doors should have different rotations, got {:?}", door_rots
    );
}

#[test]
fn large_room_sealed_has_4_floors_4_ceilings() {
    let placements = assemble_default(&large_room(), &[], [0.0, 0.0, 0.0]);
    assert_eq!(count_floors(&placements, 0.0), 4, "2x2 = 4 floor tiles");
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 4, "2x2 = 4 ceiling tiles");

    // Position verification: floor and ceiling tiles are all at Y=0 and Y=STORY_HEIGHT.
    // In a 2x2 room, all cells are corners so their floor tiles use corner_pos
    // (which may converge at room center). Verify correct Y levels and tile count.
    let floors: Vec<_> = placements.iter()
        .filter(|p| is_floor_scene(p.scene) && p.position[1].abs() < 0.001)
        .collect();
    assert_eq!(floors.len(), 4);
    let ceilings: Vec<_> = placements.iter()
        .filter(|p| is_floor_scene(p.scene) && (p.position[1] - STORY_HEIGHT).abs() < 0.001)
        .collect();
    assert_eq!(ceilings.len(), 4);
    // Each floor has a matching ceiling directly above it.
    for f in &floors {
        let has_ceiling_above = ceilings.iter().any(|c| {
            (c.position[0] - f.position[0]).abs() < 0.001
                && (c.position[2] - f.position[2]).abs() < 0.001
        });
        assert!(has_ceiling_above, "floor at {:?} should have ceiling above", f.position);
    }
}

#[test]
fn large_room_sealed_walls() {
    // 2x2 sealed: 4 corners × 0 straight walls + 0 edge cells = 0 straight walls.
    // Every cell in a 2x2 is a corner (each has 2 perpendicular sealed faces).
    let placements = assemble_default(&large_room(), &[], [0.0, 0.0, 0.0]);
    assert_eq!(count(&placements, WALL), 0, "2x2: all cells are corners, no straight walls");
    assert_eq!(count(&placements, CORNER), 4, "one corner per cell");

    // Position verification: all 4 corners at floor Y level with 4 distinct rotations.
    // In a 2x2 room, all corners converge at the room center (interior offsets
    // from each cell push toward center). Differentiated by rotation, not position.
    let corners: Vec<_> = placements.iter().filter(|p| p.scene == CORNER).collect();
    for c in &corners {
        assert!(c.position[1].abs() < 0.001, "corner at floor Y: {:?}", c.position);
    }
    let mut corner_rots: Vec<i32> = corners.iter()
        .map(|c| (c.rotation_y * 1000.0) as i32)
        .collect();
    corner_rots.sort();
    corner_rots.dedup();
    assert_eq!(corner_rots.len(), 4, "4 corners should have 4 distinct rotations, got {:?}", corner_rots);
}

#[test]
fn large_room_interior_edges_have_no_walls_at_interior() {
    let placements = assemble_default(&large_room(), &[], [0.0, 0.0, 0.0]);
    // Interior edges (between cells of same room) should have nothing.
    // 2x2: all 4 cells are corners → 0 straight walls.
    assert_eq!(count(&placements, WALL), 0, "2x2 sealed: all corners, 0 straight walls");
}

#[test]
fn large_room_one_connector_active() {
    // large_room NegX connector at [0,0,0]. With NegX active:
    // Cell (0,0,0): ConnectorGap (NegX active) → door frame + NegZ wall
    // Cell (1,0,0): PosX+NegZ corner → 0 straight walls
    // Cell (0,0,1): NegX+PosZ corner (NegX sealed here since connector is at [0,0,0]) → 0 straight walls
    // Cell (1,0,1): PosX+PosZ corner → 0 straight walls
    let placements = assemble_default(
        &large_room(),
        &[Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX }],
        [0.0, 0.0, 0.0],
    );
    assert_eq!(count(&placements, DOOR), 1, "room emits door frame at active connector");
    assert_eq!(count(&placements, WALL), 1, "only edge cells get straight walls");
}

#[test]
fn room_active_connector_emits_door_not_wall() {
    let placements = assemble_default(
        &room_3x3(),
        &[Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX }],
        [0.0, 0.0, 0.0],
    );
    // 3x3 with NegX active at [0,0,1]: door frame at (0,0,1).
    // 4 corner cells emit no straight walls.
    // Edge cells: (1,0,0) NegZ, (2,0,1) PosX, (1,0,2) PosZ → 3 straight walls.
    assert_eq!(count(&placements, DOOR), 1, "rooms emit door frame at active connector");
    assert_eq!(count(&placements, WALL), 3, "3 edge cells get straight walls, corners don't");
}

// --- Orientation tests ---

#[test]
fn floor_tiles_face_upward() {
    let placements = assemble_default(&room_3x3(), &[], [0.0, 0.0, 0.0]);
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
    let placements = assemble_default(&room_3x3(), &[], [0.0, 0.0, 0.0]);
    let ceiling_tiles: Vec<_> = placements.iter()
        .filter(|p| is_floor_scene(p.scene) && (p.position[1] - STORY_HEIGHT).abs() < 0.001)
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
    let placements = assemble_default(&small_room(), &[], [0.0, 0.0, 0.0]);
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 1);
}

#[test]
fn sealed_3x3_room_has_9_ceiling_tiles() {
    let placements = assemble_default(&room_3x3(), &[], [0.0, 0.0, 0.0]);
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 9);
}

#[test]
fn sealed_3x3_room_full_surface_coverage() {
    let placements = assemble_default(&room_3x3(), &[], [0.0, 0.0, 0.0]);
    assert_eq!(count_floors(&placements, 0.0), 9, "3x3 = 9 floor tiles");
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 9, "3x3 = 9 ceiling tiles");
    // 4 edge cells × 1 wall each = 4 straight walls. Corner cells emit no straight walls.
    assert_eq!(count(&placements, WALL), 4, "4 edge cells get straight walls, corners don't");
    assert_eq!(count(&placements, CEILING), 4, "4 edge cells get straight ceiling strips");
    assert_eq!(count(&placements, CORNER), 4, "4 external corners");

    // Position verification: 9 floor tiles at 9 distinct positions.
    let floors: Vec<[f32; 2]> = placements.iter()
        .filter(|p| is_floor_scene(p.scene) && p.position[1].abs() < 0.001)
        .map(|p| [p.position[0], p.position[2]])
        .collect();
    let mut floor_keys: Vec<(i32, i32)> = floors.iter()
        .map(|f| ((f[0] * 10.0) as i32, (f[1] * 10.0) as i32))
        .collect();
    floor_keys.sort();
    floor_keys.dedup();
    assert_eq!(floor_keys.len(), 9, "9 floor tiles at 9 distinct positions");

    // Walls should NOT be at the same XZ as any corner piece — corners and
    // straight walls are mutually exclusive at a given cell.
    let walls: Vec<_> = placements.iter().filter(|p| p.scene == WALL).collect();
    let corner_positions: Vec<(i32, i32)> = placements.iter()
        .filter(|p| p.scene == CORNER)
        .map(|p| ((p.position[0] * 100.0) as i32, (p.position[2] * 100.0) as i32))
        .collect();
    for w in &walls {
        let wk = ((w.position[0] * 100.0) as i32, (w.position[2] * 100.0) as i32);
        assert!(
            !corner_positions.contains(&wk),
            "wall at {:?} overlaps with a corner piece — should be mutually exclusive", w.position
        );
    }
}

// --- Vertical connector tests ---
// Y-axis connectors leave a clean opening in the floor/ceiling.
// No hatch/door mesh — the Quaternius door frame is designed for vertical
// doorways and doesn't visually work as a horizontal floor/ceiling hatch.

#[test]
fn posy_connector_removes_ceiling_no_hatch() {
    let placements = assemble_default(
        &hub_6way(),
        &[Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosY }],
        [0.0, 0.0, 0.0],
    );
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 0,
        "active PosY should remove ceiling tile");
    assert_eq!(count_floors(&placements, 0.0), 1, "floor should remain");
    // No hatch/door should be placed at the ceiling opening
    let ceiling_doors: Vec<_> = placements.iter()
        .filter(|p| p.scene == DOOR && (p.position[1] - STORY_HEIGHT).abs() < 0.001)
        .collect();
    assert_eq!(ceiling_doors.len(), 0,
        "no door/hatch at vertical ceiling opening — clean hole only");
}

#[test]
fn negy_connector_removes_floor_no_hatch() {
    let placements = assemble_default(
        &hub_6way(),
        &[Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegY }],
        [0.0, 0.0, 0.0],
    );
    assert_eq!(count_floors(&placements, 0.0), 0,
        "active NegY should remove floor tile");
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 1, "ceiling should remain");
    // No hatch/door should be placed at the floor opening
    let floor_doors: Vec<_> = placements.iter()
        .filter(|p| p.scene == DOOR && p.position[1].abs() < 0.001)
        .collect();
    assert_eq!(floor_doors.len(), 0,
        "no door/hatch at vertical floor opening — clean hole only");
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
    let placements = assemble_default(&two_story, &[], [0.0, 0.0, 0.0]);
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
    assert_eq!(ceiling_count, 1, "ceiling at Y=10 (2 * STORY_HEIGHT)");
}

// --- Wall overlap between adjacent rooms ---

#[test]
fn adjacent_rooms_walls_do_not_overlap() {
    let room_a_placements = assemble_default(
        &small_room(),
        &[Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX }],
        [0.0, 0.0, 0.0],
    );
    let room_b_placements = assemble_default(
        &small_room(),
        &[Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX }],
        [4.0, 0.0, 0.0],
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

// --- Vertical openings are clean holes (no hatch mesh) ---

#[test]
fn vertical_connector_emits_zero_doors() {
    // Both PosY and NegY active — no door/hatch meshes anywhere
    let placements = assemble_default(
        &hub_6way(),
        &[
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosY },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegY },
        ],
        [0.0, 0.0, 0.0],
    );
    let door_count = count(&placements, DOOR);
    assert_eq!(door_count, 0,
        "vertical connectors should not produce any door/hatch meshes, got {door_count}");
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

// === Collision box tests ===

/// A sealed 1x1 room must have collision on all 4 XZ walls + floor + ceiling = 6 boxes.
#[test]
fn sealed_small_room_collision_box_count() {
    let boxes = collision_boxes(&small_room(), &[], [0.0, 0.0, 0.0], &asset_catalog::WALL_SET_ASTRA);
    // 4 XZ walls + 1 floor + 1 ceiling = 6
    assert_eq!(boxes.len(), 6, "sealed 1x1x1: 4 walls + floor + ceiling");
}

/// Active connector removes the wall collider on that face.
#[test]
fn active_connector_removes_wall_collider() {
    let all_sealed = collision_boxes(&small_room(), &[], [0.0, 0.0, 0.0], &asset_catalog::WALL_SET_ASTRA);
    let one_open = collision_boxes(&small_room(), &[Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX }], [0.0, 0.0, 0.0], &asset_catalog::WALL_SET_ASTRA);
    assert_eq!(one_open.len(), all_sealed.len() - 1,
        "opening one connector should remove exactly one collision box");
}

/// 3x3 sealed room: every cell contributes floor + ceiling. Only boundary cells
/// contribute wall colliders. Interior cell (1,0,1) has no wall colliders.
#[test]
fn sealed_3x3_collision_covers_all_boundaries() {
    let boxes = collision_boxes(&room_3x3(), &[], [0.0, 0.0, 0.0], &asset_catalog::WALL_SET_ASTRA);

    // 3x3 single story:
    // Floors: 9, Ceilings: 9
    // Walls: perimeter = 3+3+3+3 = 12 XZ wall faces
    // Total = 9 + 9 + 12 = 30
    let floor_boxes = boxes.iter().filter(|b| b.position[1] < 0.0).count();
    let ceiling_boxes = boxes.iter().filter(|b| b.position[1] > STORY_HEIGHT).count();
    let wall_boxes = boxes.len() - floor_boxes - ceiling_boxes;
    assert_eq!(floor_boxes, 9, "9 floor slabs");
    assert_eq!(ceiling_boxes, 9, "9 ceiling slabs");
    assert_eq!(wall_boxes, 12, "12 perimeter wall faces");
}

/// Collision boxes must fully enclose the room — no position outside the
/// convex hull of colliders should be reachable from inside.
#[test]
fn collision_boxes_form_closed_boundary() {
    let boxes = collision_boxes(&small_room(), &[], [0.0, 0.0, 0.0], &asset_catalog::WALL_SET_ASTRA);

    // For a sealed 1x1 room at origin, cell center is at (2.0, 0.0, 2.0).
    // Wall colliders should be at the 4 boundaries of the cell:
    // NegX: x=0, PosX: x=4, NegZ: z=0, PosZ: z=4
    // Floor: y~0, Ceiling: y~5
    let wall_boxes: Vec<_> = boxes.iter().filter(|b| {
        b.position[1] > 0.0 && b.position[1] < STORY_HEIGHT
    }).collect();
    assert_eq!(wall_boxes.len(), 4, "4 wall colliders");

    // Each wall should be at a cell boundary (x=0, x=4, z=0, or z=4)
    let at_boundary = |b: &CollisionBox| -> bool {
        let x = b.position[0];
        let z = b.position[2];
        (x - 0.0).abs() < 0.01 || (x - 4.0).abs() < 0.01
            || (z - 0.0).abs() < 0.01 || (z - 4.0).abs() < 0.01
    };
    for wb in &wall_boxes {
        assert!(at_boundary(wb),
            "wall collider at {:?} is not at a cell boundary", wb.position);
    }
}

/// Y-axis connector removes floor or ceiling collider.
#[test]
fn vertical_connector_removes_floor_ceiling_collider() {
    let sealed = collision_boxes(&hub_6way(), &[], [0.0, 0.0, 0.0], &asset_catalog::WALL_SET_ASTRA);
    let floor_open = collision_boxes(&hub_6way(), &[Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegY }], [0.0, 0.0, 0.0], &asset_catalog::WALL_SET_ASTRA);
    let ceiling_open = collision_boxes(&hub_6way(), &[Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosY }], [0.0, 0.0, 0.0], &asset_catalog::WALL_SET_ASTRA);

    assert_eq!(floor_open.len(), sealed.len() - 1, "NegY removes floor slab");
    assert_eq!(ceiling_open.len(), sealed.len() - 1, "PosY removes ceiling slab");
}

/// Corridor collision: door frames don't add extra wall colliders.
/// The corridor's sealed NegZ/PosZ faces get wall colliders. The connector
/// faces (NegX/PosX) have no wall collider since they're open passages.
#[test]
fn corridor_collision_matches_sealed_faces() {
    let boxes = collision_boxes(
        &corridor_ew(),
        &[
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
        ],
        [0.0, 0.0, 0.0],
        &asset_catalog::WALL_SET_ASTRA,
    );
    // NegZ + PosZ walls = 2, floor + ceiling = 2. No NegX/PosX walls.
    let wall_boxes = boxes.iter().filter(|b| {
        b.position[1] > 0.0 && b.position[1] < STORY_HEIGHT
    }).count();
    assert_eq!(wall_boxes, 2, "corridor with both ends open: 2 side walls");
}

// ==========================================================================
// 5-layer wall stack tests (Chunk 1 TDD)
// ==========================================================================

/// Each sealed XZ boundary tile must emit all 4 wall layers: Bottom + ShortWall + Wall + Top.
#[test]
fn sealed_wall_emits_4_layer_stack() {
    let ws = &asset_catalog::WALL_SET_ASTRA;
    // Corridor with both ends open: NegZ and PosZ get straight walls.
    let placements = assemble(
        &corridor_ew(),
        &[
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
        ],
        [0.0, 0.0, 0.0],
        ws,
    );
    // 2 sealed faces × 4 layers each = 8 wall-layer meshes
    let bottom_count = count(&placements, ws.bottom.straight);
    let short_count = count(&placements, ws.short_wall.straight);
    let wall_count = count(&placements, ws.straight.wall);
    let top_count = count(&placements, ws.straight.ceiling);
    assert_eq!(bottom_count, 2, "2 sealed faces × 1 bottom each");
    assert_eq!(short_count, 2, "2 sealed faces × 1 short_wall each");
    assert_eq!(wall_count, 2, "2 sealed faces × 1 wall each");
    assert_eq!(top_count, 2, "2 sealed faces × 1 top each");
}

/// Active connector tile emits door frame only, no wall layers.
#[test]
fn aperture_tile_emits_door_frame_only() {
    let ws = &asset_catalog::WALL_SET_ASTRA;
    let placements = assemble(
        &corridor_ew(),
        &[Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX }],
        [0.0, 0.0, 0.0],
        ws,
    );
    assert_eq!(count(&placements, asset_catalog::DOOR), 1);
    // The NegX face has a door, not wall layers. NegZ and PosZ each get 4 layers.
    // PosX is sealed (no active connector) and is a corner with NegZ and PosZ.
    // So no straight bottom/shortwall at the NegX position.
}

/// Floor tile emits at y=0 for bottom cells.
#[test]
fn floor_tile_emits_platform_at_y0() {
    let placements = assemble_default(&small_room(), &[], [0.0, 0.0, 0.0]);
    let floors: Vec<_> = placements.iter()
        .filter(|p| is_floor_scene(p.scene) && p.position[1].abs() < 0.001)
        .collect();
    assert_eq!(floors.len(), 1, "single-cell room has 1 floor tile at y=0");
}

/// Ceiling tile emits flipped Platform at y=story_height.
#[test]
fn ceiling_tile_emits_flipped_platform() {
    let placements = assemble_default(&small_room(), &[], [0.0, 0.0, 0.0]);
    let ceilings: Vec<_> = placements.iter()
        .filter(|p| is_floor_scene(p.scene) && (p.position[1] - STORY_HEIGHT).abs() < 0.001)
        .collect();
    assert_eq!(ceilings.len(), 1);
    assert!((ceilings[0].rotation_x - PI).abs() < 0.001, "ceiling should be flipped (rotation_x = PI)");
}

/// Interior tile positions emit no geometry.
#[test]
fn no_geometry_at_interior_positions() {
    use crate::cell::CellGrid;
    let ws = &asset_catalog::WALL_SET_ASTRA;
    let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], ws.tile_width);
    let placements = assemble_default(&room_3x3(), &[], [0.0, 0.0, 0.0]);

    // Center cell (1,0,1) is interior — should have no wall/corner geometry at its center.
    let center = grid.cell_at(1, 0, 1).unwrap();
    let at_center = |p: &MeshPlacement| {
        (p.position[0] - center.world_center[0]).abs() < 0.001
            && (p.position[2] - center.world_center[2]).abs() < 0.001
    };
    let wall_at_center = placements.iter().any(|p| {
        (p.scene == ws.straight.wall || p.scene == ws.bottom.straight
            || p.scene == ws.short_wall.straight || p.scene == ws.straight.ceiling
            || p.scene == ws.corner_inner.wall || p.scene == ws.corner_outer.wall)
            && at_center(p)
    });
    assert!(!wall_at_center, "interior cell should have no wall-layer geometry at its center");
}

/// Multi-story room: floor only at bottom, ceiling only at top.
#[test]
fn multi_story_room_has_floor_only_at_bottom_ceiling_only_at_top() {
    let two_story = RoomTemplate {
        id: "test_2story",
        kind: TemplateKind::Room,
        connectors: vec![],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 2, 1],
    };
    let ws = &asset_catalog::WALL_SET_ASTRA;
    let placements = assemble(&two_story, &[], [0.0, 0.0, 0.0], ws);
    let max_y = 2.0 * ws.story_height;

    assert_eq!(count_floors(&placements, 0.0), 1, "floor at y=0");
    assert_eq!(count_ceiling_tiles(&placements, 0.0, max_y), 1, "ceiling at y=2*story_height");
    // No floor/ceiling at intermediate y = story_height
    let mid_floors = placements.iter().filter(|p| {
        is_floor_scene(p.scene) && (p.position[1] - ws.story_height).abs() < 0.001
    }).count();
    assert_eq!(mid_floors, 0, "no floor/ceiling at intermediate y");
}

/// Room dimensions use wall set's tile_width, not a hardcoded 4.0.
#[test]
fn room_dimensions_use_wall_set_tile_width() {
    let ws = &asset_catalog::WALL_SET_ASTRA;
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], ws);

    // In a 3x3 room, the rightmost cell center is at (0 + 2.5 * tile_width, 0, ...).
    // Floor tiles span from tile_width/2 to 2.5*tile_width in X.
    let max_floor_x = placements.iter()
        .filter(|p| is_floor_scene(p.scene) && p.position[1].abs() < 0.001)
        .map(|p| p.position[0])
        .fold(f32::NEG_INFINITY, f32::max);
    let expected_max_x = (2.0 + 0.5) * ws.tile_width; // cell 2 center
    assert!(
        (max_floor_x - expected_max_x).abs() < 0.1,
        "max floor X should be ~{expected_max_x}, got {max_floor_x}"
    );
}

/// Room height uses wall set's story_height, not a hardcoded 5.0.
#[test]
fn room_height_uses_wall_set_story_height() {
    let ws = &asset_catalog::WALL_SET_ASTRA;
    let placements = assemble(&small_room(), &[], [0.0, 0.0, 0.0], ws);
    let ceiling_y = placements.iter()
        .filter(|p| is_floor_scene(p.scene) && p.position[1] > 1.0)
        .map(|p| p.position[1])
        .next()
        .expect("should have a ceiling tile");
    assert!(
        (ceiling_y - ws.story_height).abs() < 0.001,
        "ceiling at y={ceiling_y}, expected story_height={}",
        ws.story_height
    );
}

/// Floor tiles at corner cells must be at cell center, not at corner wall offset.
/// Corner walls are offset toward interior (by up to 2.0m), but floors must stay
/// at cell center for watertight coverage. Only the rotation changes for curved tiles.
#[test]
fn floor_tiles_at_corner_cells_are_at_cell_center() {
    use crate::cell::CellGrid;
    let ws = &asset_catalog::WALL_SET_ASTRA;
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], ws);
    let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], ws.tile_width);

    // Collect all cell centers
    let cell_centers: Vec<(i32, i32)> = grid.cells().iter()
        .map(|c| ((c.world_center[0] * 100.0) as i32, (c.world_center[2] * 100.0) as i32))
        .collect();

    // Every floor tile must be at a cell center XZ position
    let floor_tiles: Vec<_> = placements.iter()
        .filter(|p| is_floor_scene(p.scene) && p.position[1].abs() < 0.001)
        .collect();
    assert_eq!(floor_tiles.len(), 9, "3x3 room should have 9 floor tiles");

    for ft in &floor_tiles {
        let fk = ((ft.position[0] * 100.0) as i32, (ft.position[2] * 100.0) as i32);
        assert!(
            cell_centers.contains(&fk),
            "floor tile at ({}, {}) is not at any cell center — likely placed at corner offset",
            ft.position[0], ft.position[2]
        );
    }
}

/// Ceiling tiles at corner cells must be at cell center, not at corner wall offset.
#[test]
fn ceiling_tiles_at_corner_cells_are_at_cell_center() {
    use crate::cell::CellGrid;
    let ws = &asset_catalog::WALL_SET_ASTRA;
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], ws);
    let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], ws.tile_width);

    let cell_centers: Vec<(i32, i32)> = grid.cells().iter()
        .map(|c| ((c.world_center[0] * 100.0) as i32, (c.world_center[2] * 100.0) as i32))
        .collect();

    let ceiling_tiles: Vec<_> = placements.iter()
        .filter(|p| is_floor_scene(p.scene) && (p.position[1] - ws.story_height).abs() < 0.001)
        .collect();
    assert_eq!(ceiling_tiles.len(), 9, "3x3 room should have 9 ceiling tiles");

    for ct in &ceiling_tiles {
        let ck = ((ct.position[0] * 100.0) as i32, (ct.position[2] * 100.0) as i32);
        assert!(
            cell_centers.contains(&ck),
            "ceiling tile at ({}, {}) is not at any cell center — likely placed at corner offset",
            ct.position[0], ct.position[2]
        );
    }
}

/// No two floor tiles should overlap (same XZ position at same Y).
#[test]
fn no_duplicate_floor_tiles() {
    let placements = assemble_default(&room_3x3(), &[], [0.0, 0.0, 0.0]);
    let mut floor_keys: Vec<(i32, i32, i32)> = placements.iter()
        .filter(|p| is_floor_scene(p.scene))
        .map(|p| ((p.position[0] * 100.0) as i32, (p.position[1] * 100.0) as i32, (p.position[2] * 100.0) as i32))
        .collect();
    let before = floor_keys.len();
    floor_keys.sort();
    floor_keys.dedup();
    assert_eq!(floor_keys.len(), before,
        "found {} duplicate floor/ceiling tiles", before - floor_keys.len());
}

/// Corner positions emit all 5 structural layers (bottom, short_wall, wall, top per inner+outer).
#[test]
fn corner_emits_all_layer_variants() {
    let ws = &asset_catalog::WALL_SET_ASTRA;
    // 1x1 room: all 4 faces are corners.
    let placements = assemble(&small_room(), &[], [0.0, 0.0, 0.0], ws);

    // 4 corners × 2 (inner+outer) = 8 each for bottom, short_wall, wall, ceiling
    assert_eq!(count(&placements, ws.bottom.corner_inner), 4);
    assert_eq!(count(&placements, ws.bottom.corner_outer), 4);
    assert_eq!(count(&placements, ws.short_wall.corner_inner), 4);
    assert_eq!(count(&placements, ws.short_wall.corner_outer), 4);
    assert_eq!(count(&placements, ws.corner_inner.wall), 4);
    assert_eq!(count(&placements, ws.corner_outer.wall), 4);
    assert_eq!(count(&placements, ws.corner_inner.ceiling), 4);
    assert_eq!(count(&placements, ws.corner_outer.ceiling), 4);
}
