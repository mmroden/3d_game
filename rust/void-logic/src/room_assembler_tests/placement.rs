use super::*;

// ==========================================================================
// Placement function properties — wall_placement, door_placement unit tests
//
// Ground truth sources:
//   godot/scenes/rooms/room_small.tscn — all walls/corners at (0,0,0)
//   godot/scenes/corridors/corridor_ew.tscn — doors at (0,0,0)
// ==========================================================================

#[test]
fn wall_placement_returns_cell_pos_unchanged() {
    // Ground truth: room_small.tscn places ALL walls at (0,0,0) — the cell center.
    // wall_placement must return the input position unchanged for all facings.
    let pos = [7.0, 3.0, 11.0];
    for facing in [ConnectorFacing::NegX, ConnectorFacing::PosX,
                   ConnectorFacing::NegZ, ConnectorFacing::PosZ] {
        let (result_pos, _rot) = wall_placement(pos, facing);
        assert_eq!(
            result_pos, pos,
            "wall_placement({pos:?}, {facing:?}) returned {result_pos:?}, expected {pos:?}. \
             Ground truth: room_small.tscn places all walls at cell center, no offset."
        );
    }
}

#[test]
fn door_placement_offsets_position_to_wall_boundary() {
    // The door mesh (Door_Frame_Square.gltf) is centered on its origin,
    // unlike the wall mesh which sits at the NegX edge.  door_placement
    // must shift the position by half a cell toward the wall boundary so
    // the door frame sits flush in the wall opening.
    let pos = [10.0, 3.0, 10.0];
    let cell_size = 4.0;
    let half = cell_size / 2.0;

    let cases = [
        (ConnectorFacing::NegX, [pos[0] - half, pos[1], pos[2]]),
        (ConnectorFacing::PosX, [pos[0] + half, pos[1], pos[2]]),
        (ConnectorFacing::NegZ, [pos[0], pos[1], pos[2] - half]),
        (ConnectorFacing::PosZ, [pos[0], pos[1], pos[2] + half]),
    ];
    for (facing, expected_pos) in cases {
        let (result_pos, _rot) = door_placement(pos, facing, cell_size);
        assert!(
            (result_pos[0] - expected_pos[0]).abs() < 0.001
                && (result_pos[1] - expected_pos[1]).abs() < 0.001
                && (result_pos[2] - expected_pos[2]).abs() < 0.001,
            "door_placement({pos:?}, {facing:?}, {cell_size}) = {result_pos:?}, \
             expected {expected_pos:?}. Door must sit at the wall boundary, not cell center."
        );
    }
}

#[test]
fn wall_rotations_match_reference_scenes() {
    let pos = [0.0, 0.0, 0.0];
    let cases = [
        (ConnectorFacing::NegX, 0.0),
        (ConnectorFacing::PosX, PI),
        (ConnectorFacing::NegZ, -FRAC_PI_2),
        (ConnectorFacing::PosZ, FRAC_PI_2),
    ];
    for (facing, expected_rot) in cases {
        let (_pos, rot) = wall_placement(pos, facing);
        assert!(
            (rot - expected_rot).abs() < 0.001,
            "wall_placement {facing:?}: rotation {rot}, expected {expected_rot}. \
             Ground truth: room_small.tscn"
        );
    }
}

#[test]
fn door_rotations_match_reference_scenes() {
    let pos = [0.0, 0.0, 0.0];
    let cases = [
        (ConnectorFacing::NegX, FRAC_PI_2),
        (ConnectorFacing::PosX, -FRAC_PI_2),
        (ConnectorFacing::NegZ, 0.0),
        (ConnectorFacing::PosZ, PI),
    ];
    for (facing, expected_rot) in cases {
        let (_pos, rot) = door_placement(pos, facing, 4.0);
        assert!(
            (rot - expected_rot).abs() < 0.001,
            "door_placement {facing:?}: rotation {rot}, expected {expected_rot}. \
             Ground truth: corridor_ew.tscn"
        );
    }
}

// ---------------------------------------------------------------------------
// Physical boundary tests — verify rotations place wall strips at the correct
// cell edge, not just that the angle matches a (possibly wrong) constant.
//
// The wall mesh natively sits at the NegX edge (x ~ -2.2, thin strip).
// Godot Y-rotation: x' = x*cos t + z*sin t,  z' = -x*sin t + z*cos t
// A representative point on the strip is (-2.2, 0, 0).
// After rotation the strip should land at the boundary named by the facing.
// ---------------------------------------------------------------------------

#[test]
fn negz_wall_rotation_places_strip_at_negative_z() {
    let (_, rot) = wall_placement([0.0, 0.0, 0.0], ConnectorFacing::NegZ);
    let (_, new_z) = rotate_y(-2.2, 0.0, rot);
    assert!(
        new_z < -1.0,
        "NegZ wall strip should be at negative Z, got z'={new_z}"
    );
}

#[test]
fn posz_wall_rotation_places_strip_at_positive_z() {
    let (_, rot) = wall_placement([0.0, 0.0, 0.0], ConnectorFacing::PosZ);
    let (_, new_z) = rotate_y(-2.2, 0.0, rot);
    assert!(
        new_z > 1.0,
        "PosZ wall strip should be at positive Z, got z'={new_z}"
    );
}

#[test]
fn corner_rotations_match_reference_scenes() {
    // Corner mesh natively fills the NegX/NegZ quadrant (x ~ -4.8..0, z ~ -4.8..0).
    // A representative point (-3, 0, -3) should end up in the correct quadrant after rotation.
    let cases: &[(f32, f32, f32)] = &[
        (0.0, -1.0, -1.0),
        (-FRAC_PI_2, 1.0, -1.0),
        (FRAC_PI_2, -1.0, 1.0),
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
    let placements = assemble_default(
        &small_room(),
        &[
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ },
        ],
        [0.0, 0.0, 0.0],
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
    let placements = assemble_default(
        &small_room(),
        &[
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
        ],
        [0.0, 0.0, 0.0],
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
