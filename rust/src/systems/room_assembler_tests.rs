use super::*;
use crate::systems::room_template::*;

fn small_room() -> RoomTemplate {
    RoomTemplate {
        id: "test_small",
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 1, 1],
    }
}

fn corridor_ew() -> RoomTemplate {
    RoomTemplate {
        id: "test_corridor_ew",
        kind: TemplateKind::Corridor,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 1, 1],
    }
}

fn large_room() -> RoomTemplate {
    RoomTemplate {
        id: "test_large",
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            Connector { offset: [1, 0, 0], facing: ConnectorFacing::PosX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
            Connector { offset: [0, 0, 1], facing: ConnectorFacing::PosZ },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [2, 1, 2],
    }
}

fn count(placements: &[MeshPlacement], scene: &str) -> usize {
    placements.iter().filter(|p| p.scene == scene).count()
}

fn hub_6way() -> RoomTemplate {
    RoomTemplate {
        id: "test_hub_6way",
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosY },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegY },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 1, 1],
    }
}

fn room_3x3() -> RoomTemplate {
    RoomTemplate {
        id: "test_3x3",
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX },
            Connector { offset: [2, 0, 1], facing: ConnectorFacing::PosX },
            Connector { offset: [1, 0, 0], facing: ConnectorFacing::NegZ },
            Connector { offset: [1, 0, 2], facing: ConnectorFacing::PosZ },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [3, 1, 3],
    }
}

// Ceiling tile uses the same platform asset but placed at ceiling height.
fn count_floors(placements: &[MeshPlacement], origin_y: f32) -> usize {
    placements.iter().filter(|p| {
        p.scene == FLOOR && (p.position[1] - origin_y).abs() < 0.001
    }).count()
}

fn count_ceiling_tiles(placements: &[MeshPlacement], origin_y: f32, cell_height: f32) -> usize {
    placements.iter().filter(|p| {
        p.scene == FLOOR && (p.position[1] - (origin_y + cell_height)).abs() < 0.001
    }).count()
}

// ==========================================================================
// Property tests against EXTERNAL ground truth (handcrafted .tscn scenes)
//
// Ground truth sources:
//   godot/scenes/rooms/room_small.tscn — all walls/corners at (0,0,0)
//   godot/scenes/rooms/room_large.tscn — walls at cell centers (-2,0,-2) etc.
//   godot/scenes/corridors/corridor_ew.tscn — doors at (0,0,0)
// ==========================================================================

// --- Placement function properties ---

#[test]
fn wall_placement_returns_cell_pos_unchanged() {
    // Ground truth: room_small.tscn places ALL walls at (0,0,0) — the cell center.
    // wall_placement must return the input position unchanged for all facings.
    let pos = [7.0, 3.0, 11.0];
    let cs = 4.0;
    for facing in [ConnectorFacing::NegX, ConnectorFacing::PosX,
                   ConnectorFacing::NegZ, ConnectorFacing::PosZ] {
        let (result_pos, _rot) = wall_placement(pos, facing, cs);
        assert_eq!(
            result_pos, pos,
            "wall_placement({pos:?}, {facing:?}) returned {result_pos:?}, expected {pos:?}. \
             Ground truth: room_small.tscn places all walls at cell center, no offset."
        );
    }
}

#[test]
fn door_placement_returns_cell_pos_unchanged() {
    // Ground truth: corridor_ew.tscn places ALL doors at (0,0,0) — the cell center.
    // door_placement must return the input position unchanged for all facings.
    let pos = [7.0, 3.0, 11.0];
    let cs = 4.0;
    for facing in [ConnectorFacing::NegX, ConnectorFacing::PosX,
                   ConnectorFacing::NegZ, ConnectorFacing::PosZ] {
        let (result_pos, _rot) = door_placement(pos, facing, cs);
        assert_eq!(
            result_pos, pos,
            "door_placement({pos:?}, {facing:?}) returned {result_pos:?}, expected {pos:?}. \
             Ground truth: corridor_ew.tscn places all doors at cell center, no offset."
        );
    }
}

#[test]
fn wall_rotations_match_reference_scenes() {
    // Ground truth from room_small.tscn Transform3D matrices.
    // Godot Y-rotation basis: X_col=(cos θ, 0, -sin θ), Z_col=(sin θ, 0, cos θ).
    //   WallNegX: identity                    → rotation_y = 0
    //   WallPosX: (-1,0,0, 0,1,0, 0,0,-1)    → rotation_y = PI
    //   WallNegZ: (0,0,1, 0,1,0, -1,0,0)     → rotation_y = -PI/2
    //   WallPosZ: (0,0,-1, 0,1,0, 1,0,0)     → rotation_y = PI/2
    let pos = [0.0, 0.0, 0.0];
    let cs = 4.0;
    let cases = [
        (ConnectorFacing::NegX, 0.0),
        (ConnectorFacing::PosX, PI),
        (ConnectorFacing::NegZ, -FRAC_PI_2),
        (ConnectorFacing::PosZ, FRAC_PI_2),
    ];
    for (facing, expected_rot) in cases {
        let (_pos, rot) = wall_placement(pos, facing, cs);
        assert!(
            (rot - expected_rot).abs() < 0.001,
            "wall_placement {facing:?}: rotation {rot}, expected {expected_rot}. \
             Ground truth: room_small.tscn"
        );
    }
}

#[test]
fn door_rotations_match_reference_scenes() {
    // Ground truth from corridor_ew.tscn Transform3D matrices.
    // Godot Y-rotation basis: X_col=(cos θ, 0, -sin θ), Z_col=(sin θ, 0, cos θ).
    //   DoorNegX: (0,0,-1, 0,1,0, 1,0,0)  → rotation_y = PI/2
    //   DoorPosX: (0,0,1, 0,1,0, -1,0,0)   → rotation_y = -PI/2
    // Inferred for N/S (door mesh spans X natively, no rotation needed for NegZ):
    //   DoorNegZ: identity                  → rotation_y = 0
    //   DoorPosZ: (-1,0,0, 0,1,0, 0,0,-1)  → rotation_y = PI
    let pos = [0.0, 0.0, 0.0];
    let cs = 4.0;
    let cases = [
        (ConnectorFacing::NegX, FRAC_PI_2),
        (ConnectorFacing::PosX, -FRAC_PI_2),
        (ConnectorFacing::NegZ, 0.0),
        (ConnectorFacing::PosZ, PI),
    ];
    for (facing, expected_rot) in cases {
        let (_pos, rot) = door_placement(pos, facing, cs);
        assert!(
            (rot - expected_rot).abs() < 0.001,
            "door_placement {facing:?}: rotation {rot}, expected {expected_rot}. \
             Ground truth: corridor_ew.tscn"
        );
    }
}

// --- Assembled room properties ---

#[test]
fn all_placements_at_cell_centers() {
    // For any assembled room, every placement must be at a cell CENTER:
    //   position[x] = origin[x] + (int + 0.5) * cell_size
    // This property holds for any room size, origin, and connector configuration.
    let test_cases: Vec<(RoomTemplate, [f32; 3])> = vec![
        (small_room(), [0.0, 0.0, 0.0]),
        (large_room(), [0.0, 0.0, 0.0]),
        (room_3x3(), [0.0, 0.0, 0.0]),
        (small_room(), [12.0, 4.0, 8.0]),
        (room_3x3(), [100.0, 0.0, 200.0]),
    ];
    let cs = 4.0_f32;

    for (template, origin) in &test_cases {
        let placements = assemble(template, &[], *origin, cs);
        for p in &placements {
            // X axis: (pos - origin) / cs should be int + 0.5
            // i.e. dx * 2 is an odd integer
            let dx = (p.position[0] - origin[0]) / cs;
            let dx2 = dx * 2.0;
            let is_center_x = (dx2 - dx2.round()).abs() < 0.001 && (dx2.round() as i32 % 2 != 0);

            // Z axis: same check
            let dz = (p.position[2] - origin[2]) / cs;
            let dz2 = dz * 2.0;
            let is_center_z = (dz2 - dz2.round()).abs() < 0.001 && (dz2.round() as i32 % 2 != 0);

            assert!(
                is_center_x,
                "room '{}' at {origin:?}: placement at {:?} has x offset {dx} from origin, \
                 expected half-integer (0.5, 1.5, 2.5, ...). Meshes are center-pivot.",
                template.id, p.position
            );
            assert!(
                is_center_z,
                "room '{}' at {origin:?}: placement at {:?} has z offset {dz} from origin, \
                 expected half-integer (0.5, 1.5, 2.5, ...). Meshes are center-pivot.",
                template.id, p.position
            );
        }
    }
}

#[test]
fn corners_at_cell_centers_not_cell_corners() {
    // Ground truth: room_small.tscn places all 4 corners at (0,0,0).
    // room_large.tscn places corners at cell centers (-2,0,-2), (2,0,-2), etc.
    // All corner meshes in a cell must share the cell center position.
    let cs = 4.0_f32;
    let placements = assemble(&small_room(), &[], [0.0, 0.0, 0.0], cs);
    let corners: Vec<_> = placements.iter().filter(|p| p.scene == CORNER).collect();
    assert_eq!(corners.len(), 4, "sealed 1x1 room should have 4 corners");

    // All 4 corners must be at the SAME position: the cell center
    let first_pos = corners[0].position;
    for c in &corners {
        assert_eq!(
            c.position, first_pos,
            "corner at {:?} differs from first corner at {:?}. \
             Ground truth: room_small.tscn places all corners at cell center.",
            c.position, first_pos
        );
    }
}

#[test]
fn every_boundary_has_wall_or_gap_at_cell_center() {
    // For every boundary cell face:
    //   - Sealed boundary → WALL at cell center
    //   - Active connector on a Corridor → DOOR at cell center
    //   - Active connector on a Room → gap (no geometry) — corridor provides the arch
    let cs = 4.0_f32;

    let test_cases: Vec<(RoomTemplate, &[ConnectorFacing])> = vec![
        (small_room(), &[]),
        (room_3x3(), &[]),
        (large_room(), &[]),
        (room_3x3(), &[ConnectorFacing::NegX, ConnectorFacing::PosZ]),
        (corridor_ew(), &[ConnectorFacing::NegX, ConnectorFacing::PosX]),
    ];

    for (template, active) in &test_cases {
        let placements = assemble(template, active, [0.0, 0.0, 0.0], cs);
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

// --- Count-based tests (not position-dependent, still valid) ---

#[test]
fn sealed_small_room_has_4_walls_4_walltops_4_corners_1_floor_1_ceiling() {
    let placements = assemble(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count_floors(&placements, 0.0), 1);
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 1);
    assert_eq!(count(&placements, WALL), 4);
    assert_eq!(count(&placements, CEILING), 4);
    assert_eq!(count(&placements, CORNER), 4);
    assert_eq!(count(&placements, DOOR), 0);
}

#[test]
fn room_active_connector_leaves_gap_no_door() {
    // Rooms should leave a gap (no wall, no door) at active connectors.
    // Only corridors provide door frames. Ground truth: room_small.tscn has no doors.
    let placements = assemble(
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
    let placements = assemble(
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
    // Corridors SHOULD emit door frames at active connectors.
    // Ground truth: corridor_ew.tscn has door frames on PosX and NegX.
    let placements = assemble(
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
    let placements = assemble(
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
    let placements = assemble(&large_room(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count_floors(&placements, 0.0), 4, "2x2 = 4 floor tiles");
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 4, "2x2 = 4 ceiling tiles");
}

#[test]
fn large_room_sealed_walls() {
    let placements = assemble(&large_room(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count(&placements, WALL), 8);
}

#[test]
fn large_room_interior_edges_have_no_walls() {
    let placements = assemble(&large_room(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count(&placements, WALL), 8);
}

#[test]
fn large_room_one_connector_active() {
    let placements = assemble(
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
    // Floor of a 1x1 room at origin [10, 5, 20] should be at cell center:
    // [10 + 0.5*4, 5, 20 + 0.5*4] = [12, 5, 22]
    let origin = [10.0, 5.0, 20.0];
    let placements = assemble(&small_room(), &[], origin, 4.0);
    let floor = placements.iter().find(|p| p.scene == FLOOR).unwrap();
    assert_eq!(
        floor.position, [12.0, 5.0, 22.0],
        "floor should be at cell center, not cell corner"
    );
}

// --- Orientation tests ---

#[test]
fn floor_tiles_face_upward() {
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    for p in &placements {
        if p.scene == FLOOR && p.position[1].abs() < 0.001 {
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
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    let ceiling_tiles: Vec<_> = placements.iter()
        .filter(|p| p.scene == FLOOR && (p.position[1] - CELL_HEIGHT).abs() < 0.001)
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
    let placements = assemble(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 1);
}

#[test]
fn sealed_3x3_room_has_9_ceiling_tiles() {
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 9);
}

#[test]
fn sealed_3x3_room_full_surface_coverage() {
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    assert_eq!(count_floors(&placements, 0.0), 9, "3x3 = 9 floor tiles");
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 9, "3x3 = 9 ceiling tiles");
    assert_eq!(count(&placements, WALL), 12, "3x3 perimeter = 12 wall segments");
    assert_eq!(count(&placements, CEILING), 12, "12 wall-top decorations");
    assert_eq!(count(&placements, CORNER), 4, "4 external corners");
}

#[test]
fn room_active_connector_leaves_gap_not_archway() {
    // room_3x3 is TemplateKind::Room — active connectors leave gaps, no door frames.
    let placements = assemble(
        &room_3x3(),
        &[ConnectorFacing::NegX],
        [0.0, 0.0, 0.0],
        4.0,
    );
    assert_eq!(count(&placements, DOOR), 0, "rooms leave gaps, no door frames");
    assert_eq!(count(&placements, WALL), 11, "12 - 1 = 11 walls remaining");
}

// --- Vertical connector tests ---

#[test]
fn posy_connector_replaces_ceiling_with_archway() {
    let placements = assemble(
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
    let placements = assemble(
        &hub_6way(),
        &[ConnectorFacing::NegY],
        [0.0, 0.0, 0.0],
        4.0,
    );
    assert_eq!(count_floors(&placements, 0.0), 0);
    assert_eq!(count_ceiling_tiles(&placements, 0.0, 5.0), 1, "ceiling should remain");
}

// --- Bounds test ---

#[test]
fn room_geometry_stays_within_cell_bounds() {
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
    let cs = 4.0_f32;
    let ch = 5.0_f32; // CELL_HEIGHT: mesh-native vertical cell size
    let max_x = 3.0 * cs;
    let max_z = 3.0 * cs;
    let max_y = 1.0 * ch; // room_3x3 has extents[1] = 1

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
    // A 2-story room (extents [1, 2, 1]) should have all geometry within
    // Y ∈ [0, 2 * CELL_HEIGHT]. This ensures vertical stacking uses
    // the mesh-native 5m cell height, not the horizontal 4m cell_size.
    let two_story = RoomTemplate {
        id: "test_2story",
        kind: TemplateKind::Room,
        connectors: vec![],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 2, 1],
    };
    let ch = 5.0_f32;
    let placements = assemble(&two_story, &[], [0.0, 0.0, 0.0], 4.0);
    let max_y = 2.0 * ch; // 2 stories * 5m = 10m

    for p in &placements {
        assert!(
            p.position[1] >= 0.0 && p.position[1] <= max_y,
            "2-story room: mesh at {:?} exceeds Y bounds [0, {max_y}]", p.position
        );
    }

    // Should have 2 floors (bottom of each story) and 1 ceiling (top of upper story)
    let floor_count = count_floors(&placements, 0.0);
    assert_eq!(floor_count, 1, "bottom floor at Y=0");
    let ceiling_count = count_ceiling_tiles(&placements, 0.0, max_y);
    assert_eq!(ceiling_count, 1, "ceiling at Y=10 (2 * CELL_HEIGHT)");
}

// --- No wall overlap between adjacent rooms ---

#[test]
fn adjacent_rooms_walls_do_not_overlap() {
    let room_a_placements = assemble(
        &small_room(),
        &[ConnectorFacing::PosX],
        [0.0, 0.0, 0.0],
        4.0,
    );
    let room_b_placements = assemble(
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

#[test]
fn posy_hatch_lays_flat() {
    let placements = assemble(
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
    let placements = assemble(
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

// ---------------------------------------------------------------------------
// Physical boundary tests — verify rotations place wall strips at the correct
// cell edge, not just that the angle matches a (possibly wrong) constant.
//
// The wall mesh natively sits at the NegX edge (x ≈ -2.2, thin strip).
// Godot Y-rotation: x' = x·cos θ + z·sin θ,  z' = -x·sin θ + z·cos θ
// A representative point on the strip is (-2.2, 0, 0).
// After rotation the strip should land at the boundary named by the facing.
// ---------------------------------------------------------------------------

/// Apply Godot Y-rotation to a point and return (x', z').
fn rotate_y(x: f32, z: f32, theta: f32) -> (f32, f32) {
    let (s, c) = theta.sin_cos();
    (x * c + z * s, -x * s + z * c)
}

#[test]
fn negz_wall_rotation_places_strip_at_negative_z() {
    let (_, rot) = wall_placement([0.0, 0.0, 0.0], ConnectorFacing::NegZ, 4.0);
    let (_, new_z) = rotate_y(-2.2, 0.0, rot);
    assert!(
        new_z < -1.0,
        "NegZ wall strip should be at negative Z, got z'={new_z}"
    );
}

#[test]
fn posz_wall_rotation_places_strip_at_positive_z() {
    let (_, rot) = wall_placement([0.0, 0.0, 0.0], ConnectorFacing::PosZ, 4.0);
    let (_, new_z) = rotate_y(-2.2, 0.0, rot);
    assert!(
        new_z > 1.0,
        "PosZ wall strip should be at positive Z, got z'={new_z}"
    );
}

#[test]
fn corner_rotations_match_reference_scenes() {
    // Ground truth from room_small.tscn. Corner mesh natively fills NegX/NegZ quadrant.
    // Godot Y-rotation basis: X_col=(cos θ, 0, -sin θ), Z_col=(sin θ, 0, cos θ).
    //   CornerNW (NegX-NegZ): identity                 → rotation_y = 0
    //   CornerNE (PosX-NegZ): (0,0,1, 0,1,0, -1,0,0)  → rotation_y = -PI/2
    //   CornerSW (NegX-PosZ): (0,0,-1, 0,1,0, 1,0,0)  → rotation_y = PI/2
    //   CornerSE (PosX-PosZ): (-1,0,0, 0,1,0, 0,0,-1) → rotation_y = PI
    //
    // The corner mesh natively fills the NegX/NegZ quadrant (x ~ -4.8..0, z ~ -4.8..0).
    // A representative point (-3, 0, -3) should end up in the correct quadrant after rotation.
    let cases: &[(f32, f32, f32)] = &[
        // (rotation_y, expected_x_sign, expected_z_sign)
        // NegX-NegZ: identity → stays at (-3, -3)
        (0.0, -1.0, -1.0),
        // PosX-NegZ: rotation_y = -PI/2 → should map to (+x, -z)
        (-FRAC_PI_2, 1.0, -1.0),
        // NegX-PosZ: rotation_y = PI/2 → should map to (-x, +z)
        (FRAC_PI_2, -1.0, 1.0),
        // PosX-PosZ: rotation_y = PI → should map to (+x, +z)
        (PI, 1.0, 1.0),
    ];

    for &(rot, expect_x_sign, expect_z_sign) in cases {
        let (new_x, new_z) = rotate_y(-3.0, -3.0, rot);
        assert!(
            new_x * expect_x_sign > 0.0,
            "corner rot={rot}: expected x sign {expect_x_sign}, got x'={new_x}"
        );
        assert!(
            new_z * expect_z_sign > 0.0,
            "corner rot={rot}: expected z sign {expect_z_sign}, got z'={new_z}"
        );
    }
}

#[test]
fn posx_negz_corner_lands_in_correct_quadrant() {
    // Open NegX and PosZ connectors, leaving only PosX and NegZ walls.
    // Only ONE corner should appear: PosX-NegZ.
    // Reference room_small.tscn: CornerNE (PosX-NegZ) uses rotation -PI/2.
    // Corner mesh natively at (-3, 0, -3) should land at (+x, -z) after rotation.
    let placements = assemble(
        &small_room(),
        &[ConnectorFacing::NegX, ConnectorFacing::PosZ],
        [0.0, 0.0, 0.0],
        4.0,
    );
    let corners: Vec<_> = placements.iter().filter(|p| p.scene == CORNER).collect();
    assert_eq!(corners.len(), 1, "should have exactly 1 corner (PosX-NegZ)");

    let (rx, rz) = rotate_y(-3.0, -3.0, corners[0].rotation_y);
    assert!(
        rx > 0.0 && rz < 0.0,
        "PosX-NegZ corner should land in (+x, -z) quadrant, got ({rx}, {rz}). \
         rotation_y={}, expected -PI/2",
        corners[0].rotation_y
    );
}

#[test]
fn negx_posz_corner_lands_in_correct_quadrant() {
    // Open PosX and NegZ connectors, leaving only NegX and PosZ walls.
    // Only ONE corner should appear: NegX-PosZ.
    // Reference room_small.tscn: CornerSW (NegX-PosZ) uses rotation +PI/2.
    let placements = assemble(
        &small_room(),
        &[ConnectorFacing::PosX, ConnectorFacing::NegZ],
        [0.0, 0.0, 0.0],
        4.0,
    );
    let corners: Vec<_> = placements.iter().filter(|p| p.scene == CORNER).collect();
    assert_eq!(corners.len(), 1, "should have exactly 1 corner (NegX-PosZ)");

    let (rx, rz) = rotate_y(-3.0, -3.0, corners[0].rotation_y);
    assert!(
        rx < 0.0 && rz > 0.0,
        "NegX-PosZ corner should land in (-x, +z) quadrant, got ({rx}, {rz}). \
         rotation_y={}, expected +PI/2",
        corners[0].rotation_y
    );
}

