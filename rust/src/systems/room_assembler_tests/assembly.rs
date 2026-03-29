use super::*;

// ==========================================================================
// Assembled room structure tests — counts, positions, orientation, bounds,
// connectors, wall overlap, and vertical hatches.
// ==========================================================================

// --- Cell center placement ---

#[test]
fn all_placements_at_cell_centers() {
    // Ground truth: room_small.tscn places every mesh at (0,0,0), the center
    // of the single cell. For a multi-cell room, each mesh must be at the
    // center of its respective cell. No mesh should be at a cell corner.
    let cell_size = 4.0;
    let templates: &[RoomTemplate] = &[small_room(), large_room(), room_3x3()];

    for template in templates {
        let placements = assemble_default(template, &[], [0.0, 0.0, 0.0], cell_size);
        for p in &placements {
            // Cell center X: (cx + 0.5) * cell_size  →  fract == 0.5 * cell_size
            let frac_x = (p.position[0] / cell_size).fract();
            let frac_z = (p.position[2] / cell_size).fract();
            assert!(
                (frac_x - 0.5).abs() < 0.001 || (frac_x + 0.5).abs() < 0.001,
                "room '{}' mesh at {:?}: X fractional cell pos = {frac_x}, expected 0.5 (cell center). \
                 Ground truth: room_small.tscn places everything at cell center.",
                template.id, p.position
            );
            assert!(
                (frac_z - 0.5).abs() < 0.001 || (frac_z + 0.5).abs() < 0.001,
                "room '{}' mesh at {:?}: Z fractional cell pos = {frac_z}, expected 0.5 (cell center). \
                 Ground truth: room_small.tscn places everything at cell center.",
                template.id, p.position
            );
        }
    }
}

#[test]
fn corners_at_cell_centers_not_cell_corners() {
    // Corner meshes are center-pivot. Ground truth: room_small.tscn places
    // all corners at (0,0,0) — the cell center, NOT at (-2,0,-2) the cell corner.
    let placements = assemble_default(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
    let corners: Vec<_> = placements.iter().filter(|p| p.scene == CORNER).collect();
    assert!(!corners.is_empty(), "sealed room should have corners");
    for c in &corners {
        assert_eq!(
            c.position, [2.0, 0.0, 2.0],
            "corner should be at cell center (2,0,2), not at cell corner. \
             Ground truth: room_small.tscn."
        );
    }
}

#[test]
fn every_boundary_has_wall_or_gap_at_cell_center() {
    let cs = 4.0_f32;

    let test_cases: Vec<(RoomTemplate, &[ConnectorFacing])> = vec![
        (small_room(), &[]),
        (room_3x3(), &[]),
        (large_room(), &[]),
        (room_3x3(), &[ConnectorFacing::NegX, ConnectorFacing::PosZ]),
        (corridor_ew(), &[ConnectorFacing::NegX, ConnectorFacing::PosX]),
    ];

    for (template, active) in &test_cases {
        let placements = assemble_default(template, active, [0.0, 0.0, 0.0], cs);
        let ex = template.extents[0] as i32;
        let ez = template.extents[2] as i32;

        for cx in 0..ex {
            for cz in 0..ez {
                let center = [
                    (cx as f32 + 0.5) * cs,
                    0.0,
                    (cz as f32 + 0.5) * cs,
                ];

                let faces = [
                    (ConnectorFacing::NegX, cx == 0),
                    (ConnectorFacing::PosX, cx == ex - 1),
                    (ConnectorFacing::NegZ, cz == 0),
                    (ConnectorFacing::PosZ, cz == ez - 1),
                ];

                for (facing, is_boundary) in faces {
                    if !is_boundary {
                        continue;
                    }

                    let is_active = active.contains(&facing)
                        && template.connectors.iter().any(|c| {
                            c.facing == facing && c.offset[0] == cx && c.offset[1] == 0 && c.offset[2] == cz
                        });
                    let is_room = template.kind == TemplateKind::Room;

                    let has_wall = placements.iter().any(|p| {
                        p.scene == WALL
                            && (p.position[0] - center[0]).abs() < 0.001
                            && (p.position[1] - center[1]).abs() < 0.001
                            && (p.position[2] - center[2]).abs() < 0.001
                    });
                    let has_door = placements.iter().any(|p| {
                        p.scene == DOOR
                            && (p.position[0] - center[0]).abs() < 0.001
                            && (p.position[1] - center[1]).abs() < 0.001
                            && (p.position[2] - center[2]).abs() < 0.001
                    });

                    if is_active && is_room {
                        assert!(
                            !has_wall && !has_door,
                            "room '{}' cell ({cx},{cz}) face {facing:?}: room should have \
                             gap (no geometry) at active connector, but found wall={has_wall} door={has_door}",
                            template.id
                        );
                    } else if is_active {
                        assert!(
                            has_door,
                            "corridor '{}' cell ({cx},{cz}) face {facing:?}: corridor should \
                             have DOOR at active connector",
                            template.id
                        );
                    } else {
                        assert!(
                            has_wall,
                            "room '{}' cell ({cx},{cz}) face {facing:?}: sealed boundary \
                             should have WALL at cell center {center:?}",
                            template.id
                        );
                    }
                }
            }
        }
    }
}

// --- Count-based tests ---

#[test]
fn sealed_small_room_has_4_walls_4_walltops_4_corners_1_floor_1_ceiling() {
    let placements = assemble_default(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count_floors(&placements, 0.0), 1);
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 1);
    assert_eq!(count(&placements, WALL), 4);
    assert_eq!(count(&placements, CEILING), 4);
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
    assert_eq!(count(&placements, WALL), 3, "3 sealed walls remain");
    assert_eq!(count(&placements, CEILING), 3, "ceiling strips match walls");
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
    assert_eq!(count(&placements, WALL), 2);
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
    let placements = assemble_default(&large_room(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count(&placements, WALL), 8);
}

#[test]
fn large_room_interior_edges_have_no_walls() {
    let placements = assemble_default(&large_room(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count(&placements, WALL), 8);
}

#[test]
fn large_room_one_connector_active() {
    let placements = assemble_default(
        &large_room(),
        &[ConnectorFacing::NegX],
        [0.0, 0.0, 0.0],
        4.0,
    );
    assert_eq!(count(&placements, DOOR), 0, "rooms leave gaps, no door frames");
    assert_eq!(count(&placements, WALL), 7);
}

#[test]
fn world_origin_offsets_all_positions() {
    let origin = [10.0, 5.0, 20.0];
    let placements = assemble_default(&small_room(), &[], origin, 4.0);
    let floor = placements.iter().find(|p| is_floor_scene(p.scene) && p.position[1].abs() < 6.0).unwrap();
    assert_eq!(
        floor.position, [12.0, 5.0, 22.0],
        "floor should be at cell center, not cell corner"
    );
}

#[test]
fn room_active_connector_leaves_gap_not_archway() {
    let placements = assemble_default(
        &room_3x3(),
        &[ConnectorFacing::NegX],
        [0.0, 0.0, 0.0],
        4.0,
    );
    assert_eq!(count(&placements, DOOR), 0, "rooms leave gaps, no door frames");
    assert_eq!(count(&placements, WALL), 11, "12 - 1 = 11 walls remaining");
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
    assert_eq!(count(&placements, WALL), 12, "3x3 perimeter = 12 wall segments");
    assert_eq!(count(&placements, CEILING), 12, "12 wall-top decorations");
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

// --- Bounds tests ---

#[test]
fn room_geometry_stays_within_cell_bounds() {
    let placements = assemble_default(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    let cs = 4.0_f32;
    let ch = 5.0_f32;
    let max_x = 3.0 * cs;
    let max_z = 3.0 * cs;
    let max_y = 1.0 * ch;

    for p in &placements {
        assert!(
            p.position[0] >= 0.0 && p.position[0] <= max_x,
            "mesh at {:?} exceeds X bounds [0, {max_x}]", p.position
        );
        assert!(
            p.position[1] >= 0.0 && p.position[1] <= max_y,
            "mesh at {:?} exceeds Y bounds [0, {max_y}]", p.position
        );
        assert!(
            p.position[2] >= 0.0 && p.position[2] <= max_z,
            "mesh at {:?} exceeds Z bounds [0, {max_z}]", p.position
        );
    }
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
        !asset_catalog::WALL_SET_ASTRA.ceiling_straight.contains("TopCables"),
        "Astra ceiling_straight should not use TopCables (gap at Y≈3.33), got '{}'",
        asset_catalog::WALL_SET_ASTRA.ceiling_straight
    );
    assert!(
        !asset_catalog::WALL_SET_ASTRA.ceiling_corner.contains("TopCables"),
        "Astra ceiling_corner should not use TopCables (gap at Y≈3.33), got '{}'",
        asset_catalog::WALL_SET_ASTRA.ceiling_corner
    );
}
