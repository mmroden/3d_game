use super::*;

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
        .filter(|p| p.scene == style.corner)
        .map(|p| ((p.position[0] * 100.0) as i32, (p.position[2] * 100.0) as i32, (p.rotation_y * 1000.0) as i32))
        .collect();

    let ceil_corners: Vec<_> = placements.iter()
        .filter(|p| p.scene == style.ceiling_corner)
        .map(|p| ((p.position[0] * 100.0) as i32, (p.position[2] * 100.0) as i32, (p.rotation_y * 1000.0) as i32))
        .collect();

    assert_eq!(
        wall_corners.len(), 4,
        "sealed 3x3 room should have 4 wall corners"
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
        .filter(|p| p.scene == style.corner)
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
        .filter(|p| p.scene == style.corner)
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
