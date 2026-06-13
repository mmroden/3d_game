use super::*;
use crate::asset_catalog::ALL_WALL_SETS;

// ==========================================================================
// Corner tests — ceiling corners, mathematical corner detection, curved floors.
// ==========================================================================

/// Ceiling corners must be emitted at the same cell positions and rotations as
/// wall corners. This ensures the ceiling junction is sealed wherever two walls meet.
#[test]
fn ceiling_corners_emitted_at_same_positions_as_wall_corners() {
    let ws = &asset_catalog::WALL_SET_ASTRA;
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], ws);

    let wall_corners: Vec<_> = placements.iter()
        .filter(|p| p.scene == ws.corner_inner.wall)
        .map(|p| ((p.position[0] * 100.0) as i32, (p.position[2] * 100.0) as i32, (p.rotation_y * 1000.0) as i32))
        .collect();

    let ceil_corners: Vec<_> = placements.iter()
        .filter(|p| p.scene == ws.corner_inner.ceiling)
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

// --- Corner detection via cell grid ---

/// XZ corner pieces appear only at cells with perpendicular XZ sealed faces.
/// In 3D, BoundaryCorner includes cells with Y+XZ perpendicular faces too,
/// but those don't emit XZ corner geometry — only XZ×XZ pairs do.
#[test]
fn corners_only_at_xz_corner_cells() {
    use crate::cell::CellGrid;
    let ws = &asset_catalog::WALL_SET_ASTRA;
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], ws);
    let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], ws.tile_width);

    // Count cells that have at least one XZ perpendicular corner pair.
    let has_xz_corner_pair = |cell: &crate::cell::Cell| -> bool {
        let has = |f: ConnectorFacing| cell.sealed_faces.contains(&f);
        (has(ConnectorFacing::NegX) && has(ConnectorFacing::NegZ))
            || (has(ConnectorFacing::PosX) && has(ConnectorFacing::NegZ))
            || (has(ConnectorFacing::NegX) && has(ConnectorFacing::PosZ))
            || (has(ConnectorFacing::PosX) && has(ConnectorFacing::PosZ))
    };

    let xz_corner_cells: Vec<_> = grid.cells().iter()
        .filter(|c| has_xz_corner_pair(c))
        .collect();
    let non_xz_corner_cells: Vec<_> = grid.cells().iter()
        .filter(|c| !has_xz_corner_pair(c))
        .collect();

    // Total inner wall corners = number of cells with XZ corner pairs
    let total_corners = placements.iter()
        .filter(|p| p.scene == ws.corner_inner.wall)
        .count();
    assert_eq!(
        total_corners, xz_corner_cells.len(),
        "total inner wall corners ({total_corners}) should equal XZ corner cell count ({})",
        xz_corner_cells.len()
    );

    // No corner pieces near non-XZ-corner cell centers
    for cell in &non_xz_corner_cells {
        let count = placements.iter().filter(|p| {
            p.scene == ws.corner_inner.wall
            && (p.position[0] - cell.world_center[0]).abs() < 0.01
            && (p.position[2] - cell.world_center[2]).abs() < 0.01
        }).count();
        assert_eq!(count, 0, "non-XZ-corner cell {:?} should have 0 inner wall corners near its center", cell.grid_pos);
    }
}

/// When an active connector removes a wall from a boundary cell, corners
/// involving that wall must disappear.
#[test]
fn active_connector_removes_corner() {
    use crate::cell::{CellGrid, CellKind};
    let ws = &asset_catalog::WALL_SET_ASTRA;
    let active = &[Connector { offset: [2, 0, 1], facing: ConnectorFacing::PosX, frame: FrameStyle::Door }];
    let placements = assemble(
        &room_3x3(),
        active,
        [0.0, 0.0, 0.0],
        ws,
    );
    let grid = CellGrid::new(&room_3x3(), active, [0.0, 0.0, 0.0], ws.tile_width);

    let corners: Vec<_> = placements.iter()
        .filter(|p| p.scene == ws.corner_inner.wall)
        .collect();

    // The connector gap cell should not have any corners
    let gap_cells: Vec<_> = grid.cells().iter()
        .filter(|c| c.kind == CellKind::ConnectorGap)
        .collect();
    for gap in &gap_cells {
        let count = corners.iter().filter(|p| {
            (p.position[0] - gap.world_center[0]).abs() < 0.01
            && (p.position[2] - gap.world_center[2]).abs() < 0.01
        }).count();
        assert_eq!(count, 0, "ConnectorGap cell {:?} should have 0 corners", gap.grid_pos);
    }

    assert_eq!(corners.len(), 4, "3x3 with one PosX connector should still have 4 corners");
}

// --- Curved floor at corners ---

/// Corner cells must use the curved floor platform (floor_corner) instead of the
/// square floor platform, so the floor/ceiling geometry matches the rounded wall corners.
#[test]
fn corner_cells_use_curved_floor_platform() {
    let ws = &asset_catalog::WALL_SET_ASTRA;
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], ws);

    // Count curved floor tiles at floor level (Y ~ 0).
    let curved_floors = placements.iter().filter(|p| {
        p.scene == FLOOR_CURVE && p.position[1].abs() < 0.001
    }).count();
    assert_eq!(curved_floors, 4, "4 corner cells should use curved floor, got {curved_floors}");

    // Count curved ceiling tiles at ceiling level (Y ~ STORY_HEIGHT).
    let curved_ceilings = placements.iter().filter(|p| {
        p.scene == FLOOR_CURVE && (p.position[1] - STORY_HEIGHT).abs() < 0.001
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
    let ws = &asset_catalog::WALL_SET_ASTRA;
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], ws);

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
        .filter(|p| p.scene == ws.corner_inner.ceiling).map(|p| key(p)).collect();
    let outer_ceilings: Vec<_> = placements.iter()
        .filter(|p| p.scene == ws.corner_outer.ceiling).map(|p| key(p)).collect();

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
    let ws = &asset_catalog::WALL_SET_ASTRA;
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], ws);

    let floor_curves: Vec<_> = placements.iter()
        .filter(|p| p.scene == FLOOR_CURVE && p.position[1].abs() < 0.001)
        .collect();

    let ceil_curves: Vec<_> = placements.iter()
        .filter(|p| p.scene == FLOOR_CURVE && (p.position[1] - STORY_HEIGHT).abs() < 0.001)
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
