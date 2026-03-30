use super::*;
use crate::systems::asset_catalog::ALL_WALL_SETS;

// ==========================================================================
// Corner tests — ceiling corners, mathematical corner detection, curved floors.
// ==========================================================================

/// Ceiling corners must be emitted at the same cell positions and rotations as
/// wall corners. This ensures the ceiling junction is sealed wherever two walls meet.
#[test]
fn ceiling_corners_emitted_at_same_positions_as_wall_corners() {
    let style = RoomStyle::default();
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0, &style);

    let wall_corners: Vec<_> = placements.iter()
        .filter(|p| p.scene == style.corner_inner.wall)
        .map(|p| ((p.position[0] * 100.0) as i32, (p.position[2] * 100.0) as i32, (p.rotation_y * 1000.0) as i32))
        .collect();

    let ceil_corners: Vec<_> = placements.iter()
        .filter(|p| p.scene == style.corner_inner.ceiling)
        .map(|p| ((p.position[0] * 100.0) as i32, (p.position[2] * 100.0) as i32, (p.rotation_y * 1000.0) as i32))
        .collect();

    assert_eq!(
        wall_corners.len(), 4,
        "sealed 3x3 room should have 4 inner wall corners"
    );
    assert_eq!(
        ceil_corners.len(), wall_corners.len(),
        "ceiling corner count ({}) should match wall corner count ({})",
        ceil_corners.len(), wall_corners.len()
    );

    for wc in &wall_corners {
        assert!(
            ceil_corners.contains(wc),
            "wall corner at ({}, {}, rot={}) has no matching ceiling corner",
            wc.0, wc.1, wc.2
        );
    }
}

// --- Mathematical corner detection ---

/// A corner is defined geometrically: it exists at a cell where two perpendicular
/// boundary walls meet. This test verifies corners appear only at cells where
/// both adjacent boundary edges are sealed walls.
#[test]
fn corner_emitted_where_perpendicular_walls_meet() {
    let style = RoomStyle::default();
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0, &style);

    let corners: Vec<_> = placements.iter()
        .filter(|p| p.scene == style.corner_inner.wall)
        .collect();

    // Cell (0,0): has NegX and NegZ walls -> 1 corner
    let cell_00 = corners.iter().filter(|p| {
        (p.position[0] - 2.0).abs() < 0.01 && (p.position[2] - 2.0).abs() < 0.01
    }).count();
    assert_eq!(cell_00, 1, "cell (0,0) with NegX and NegZ walls should have exactly 1 corner");

    // Cell (1,0): has NegZ wall only (no NegX or PosX) -> 0 corners
    let cell_10 = corners.iter().filter(|p| {
        (p.position[0] - 6.0).abs() < 0.01 && (p.position[2] - 2.0).abs() < 0.01
    }).count();
    assert_eq!(cell_10, 0, "cell (1,0) with only NegZ wall should have 0 corners");

    // Cell (2,2): has PosX and PosZ walls -> 1 corner
    let cell_22 = corners.iter().filter(|p| {
        (p.position[0] - 10.0).abs() < 0.01 && (p.position[2] - 10.0).abs() < 0.01
    }).count();
    assert_eq!(cell_22, 1, "cell (2,2) with PosX and PosZ walls should have exactly 1 corner");
}

/// When an active connector removes a wall from a boundary cell, corners involving
/// that wall must disappear.
#[test]
fn no_corner_where_wall_removed_by_active_connector() {
    let style = RoomStyle::default();
    let placements = assemble(
        &room_3x3(),
        &[ConnectorFacing::PosX],
        [0.0, 0.0, 0.0],
        4.0,
        &style,
    );

    let corners: Vec<_> = placements.iter()
        .filter(|p| p.scene == style.corner_inner.wall)
        .collect();

    // Cell (2,1) center: x=10, z=6. Active PosX removes that wall.
    let cell_21 = corners.iter().filter(|p| {
        (p.position[0] - 10.0).abs() < 0.01 && (p.position[2] - 6.0).abs() < 0.01
    }).count();
    assert_eq!(cell_21, 0, "cell (2,1) with PosX connector active should have 0 corners");

    assert_eq!(corners.len(), 4, "3x3 with connector at (2,1) should still have 4 corners");
}

// --- Curved floor at corners ---

/// Corner cells must use the curved floor platform (floor_corner) instead of the
/// square floor platform, so the floor/ceiling geometry matches the rounded wall corners.
#[test]
fn corner_cells_use_curved_floor_platform() {
    let style = RoomStyle::default();
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0, &style);

    // Count curved floor tiles at floor level (Y ~ 0).
    let curved_floors = placements.iter().filter(|p| {
        p.scene == FLOOR_CURVE && p.position[1].abs() < 0.001
    }).count();
    assert_eq!(curved_floors, 4, "4 corner cells should use curved floor, got {curved_floors}");

    // Count curved ceiling tiles at ceiling level (Y ~ CELL_HEIGHT).
    let curved_ceilings = placements.iter().filter(|p| {
        p.scene == FLOOR_CURVE && (p.position[1] - CELL_HEIGHT).abs() < 0.001
    }).count();
    assert_eq!(curved_ceilings, 4, "4 corner cells should use curved ceiling, got {curved_ceilings}");

    // Non-corner cells still use square floor.
    let square_floors = placements.iter().filter(|p| {
        p.scene == FLOOR && p.position[1].abs() < 0.001
    }).count();
    assert_eq!(square_floors, 5, "5 non-corner cells should use square floor, got {square_floors}");
}

// --- Outer corners ---

/// Outer corner wall+ceiling pieces must be emitted at the same positions and
/// rotations as inner corners, completing the rounded corner geometry.
#[test]
fn outer_corners_emitted_at_same_positions_as_inner_corners() {
    let style = RoomStyle::default();
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0, &style);

    let key = |p: &MeshPlacement| -> (i32, i32, i32) {
        ((p.position[0] * 100.0) as i32, (p.position[2] * 100.0) as i32, (p.rotation_y * 1000.0) as i32)
    };

    let inner_walls: Vec<_> = placements.iter().filter(|p| p.scene == CORNER).map(|p| key(p)).collect();
    let outer_walls: Vec<_> = placements.iter().filter(|p| p.scene == CORNER_OUTER).map(|p| key(p)).collect();

    assert_eq!(inner_walls.len(), 4, "should have 4 inner wall corners");
    assert_eq!(outer_walls.len(), 4, "should have 4 outer wall corners");

    for iw in &inner_walls {
        assert!(
            outer_walls.contains(iw),
            "inner wall corner at ({}, {}, rot={}) has no matching outer wall corner",
            iw.0, iw.1, iw.2
        );
    }

    let inner_ceilings: Vec<_> = placements.iter()
        .filter(|p| p.scene == style.corner_inner.ceiling).map(|p| key(p)).collect();
    let outer_ceilings: Vec<_> = placements.iter()
        .filter(|p| p.scene == style.corner_outer.ceiling).map(|p| key(p)).collect();

    assert_eq!(inner_ceilings.len(), 4);
    assert_eq!(outer_ceilings.len(), 4);

    for ic in &inner_ceilings {
        assert!(
            outer_ceilings.contains(ic),
            "inner ceiling corner at ({}, {}, rot={}) has no matching outer ceiling corner",
            ic.0, ic.1, ic.2
        );
    }
}

// --- Ceiling curved tile rotation ---

// --- Corner cells use corner pieces, not straight walls ---

/// Corner cells must have BOTH straight walls AND corner pieces.
/// Straight walls seal the boundary gaps; corner pieces render in front.
#[test]
fn corner_cells_have_straight_walls_and_corner_pieces() {
    let style = RoomStyle::default();
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0, &style);

    let corner_centers: Vec<(f32, f32)> = vec![
        (2.0, 2.0),   // cell (0,0): NegX+NegZ corner
        (10.0, 2.0),  // cell (2,0): PosX+NegZ corner
        (2.0, 10.0),  // cell (0,2): NegX+PosZ corner
        (10.0, 10.0), // cell (2,2): PosX+PosZ corner
    ];

    for (cx, cz) in &corner_centers {
        // Straight walls must be present to seal the boundary.
        let straight_at_corner = placements.iter().filter(|p| {
            (p.scene == style.straight.wall || p.scene == style.straight.ceiling)
                && (p.position[0] - cx).abs() < 0.01
                && (p.position[2] - cz).abs() < 0.01
        }).count();
        assert_eq!(
            straight_at_corner, 4,
            "corner cell at ({cx}, {cz}) should have 4 straight pieces (2 walls + 2 ceilings for 2 sealed faces)"
        );

        // Corner pieces must also be present.
        let corner_pieces = placements.iter().filter(|p| {
            (p.scene == style.corner_inner.wall || p.scene == style.corner_inner.ceiling
                || p.scene == style.corner_outer.wall || p.scene == style.corner_outer.ceiling)
                && (p.position[0] - cx).abs() < 0.01
                && (p.position[2] - cz).abs() < 0.01
        }).count();
        assert!(corner_pieces >= 4,
            "corner cell at ({cx}, {cz}) should have ≥4 corner pieces (inner+outer wall+ceiling)");
    }
}

// --- Every sealed face must have a straight wall (no void gaps) ---

/// The visual bug: corner cells suppress straight walls, leaving void gaps
/// visible through the curved corner pieces. The fix: every sealed boundary
/// face must have a straight wall at that cell, regardless of whether a corner
/// piece is also present. The corner piece renders in front; the straight wall
/// seals the boundary behind it.
#[test]
fn every_sealed_face_has_straight_wall_even_at_corners() {
    let style = RoomStyle::default();
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0, &style);

    // Build the cell grid to know which faces are sealed.
    let grid = crate::systems::cell::CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);

    for cell in grid.cells() {
        let pos = cell.world_center;
        for &facing in &cell.sealed_faces {
            let (expected_pos, expected_rot) = wall_placement(pos, facing, 0.0);
            let has_wall = placements.iter().any(|p| {
                p.scene == style.straight.wall
                    && (p.position[0] - expected_pos[0]).abs() < 0.01
                    && (p.position[1] - expected_pos[1]).abs() < 0.01
                    && (p.position[2] - expected_pos[2]).abs() < 0.01
                    && (p.rotation_y - expected_rot).abs() < 0.01
            });
            assert!(
                has_wall,
                "cell ({},{},{}) sealed face {:?} at pos {:?} must have a straight wall to prevent void gaps",
                cell.grid_pos[0], cell.grid_pos[1], cell.grid_pos[2], facing, expected_pos
            );
        }
    }
}

// --- No thin-strip ceiling corner pieces ---

/// Every wall set's corner ceiling pieces must be full quarter-cylinders, not thin
/// decorative strips that leave gaps.
#[test]
fn corner_ceiling_pieces_are_full_quarter_cylinders() {
    let thin_strips = [
        "TopAstra_Corner_Round_Inner",
        "TopWindow_Corner_Curve_Outer",
    ];
    for ws in ALL_WALL_SETS {
        for (label, triple) in [("inner", &ws.corner_inner), ("outer", &ws.corner_outer)] {
            for bad in &thin_strips {
                assert!(
                    !triple.ceiling.contains(bad),
                    "wall set '{}' {label} ceiling uses thin-strip piece '{bad}'",
                    ws.id
                );
            }
        }
    }
}

/// Ceiling curved tiles must land in the same XZ quadrant as their floor counterparts.
/// Godot applies Euler rotations in YXZ order: Rx(PI) flips Z, then Ry rotates.
/// The assembler must compensate by using rotation_y = rot - PI/2 for ceiling curves.
#[test]
fn ceiling_curved_tiles_land_in_same_quadrant_as_floor_curved_tiles() {
    let style = RoomStyle::default();
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0, &style);

    let floor_curves: Vec<_> = placements.iter()
        .filter(|p| p.scene == FLOOR_CURVE && p.position[1].abs() < 0.001)
        .collect();

    let ceil_curves: Vec<_> = placements.iter()
        .filter(|p| p.scene == FLOOR_CURVE && (p.position[1] - CELL_HEIGHT).abs() < 0.001)
        .collect();

    assert_eq!(floor_curves.len(), 4);
    assert_eq!(ceil_curves.len(), 4);

    for fc in &floor_curves {
        // Floor quadrant: just Ry
        let (fx, fz) = rotate_y(-3.0, -3.0, fc.rotation_y);
        let floor_qx = fx.signum() as i32;
        let floor_qz = fz.signum() as i32;

        // Find matching ceiling tile at same (x, z)
        let cc = ceil_curves.iter()
            .find(|p| {
                (p.position[0] - fc.position[0]).abs() < 0.01
                && (p.position[2] - fc.position[2]).abs() < 0.01
            })
            .expect("ceiling curved tile should exist at same (x,z) as floor curved tile");

        // Ceiling quadrant: Rx(PI) first (flips Z), then Ry
        // Rx(PI) on (-3, 0, -3) -> (-3, 0, 3)
        let (cx, cz) = rotate_y(-3.0, 3.0, cc.rotation_y);
        let ceil_qx = cx.signum() as i32;
        let ceil_qz = cz.signum() as i32;

        assert_eq!(
            (floor_qx, floor_qz), (ceil_qx, ceil_qz),
            "Ceiling curved tile at ({}, {}) lands in quadrant ({}, {}), \
             but floor tile lands in ({}, {}). rotation_y: floor={}, ceiling={}",
            fc.position[0], fc.position[2],
            ceil_qx, ceil_qz, floor_qx, floor_qz,
            fc.rotation_y, cc.rotation_y
        );
    }
}
