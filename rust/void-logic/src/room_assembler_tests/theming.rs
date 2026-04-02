use super::*;

// ==========================================================================
// RoomStyle theming tests — verify style-specific assets are used correctly.
// ==========================================================================

#[test]
fn pipe_style_uses_pipe_wall_assets() {
    // Use 3x3 room: 4 non-corner edges get straight walls.
    let style = RoomStyle::from_wall_set(&asset_catalog::WALL_SET_PIPE);
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0, &style);
    let walls: Vec<_> = placements.iter()
        .filter(|p| p.scene == asset_catalog::WALL_SET_PIPE.straight.wall)
        .collect();
    assert_eq!(walls.len(), 4, "sealed 3x3 room: 4 edge cells get straight walls, corners don't");
    assert_eq!(
        count(&placements, asset_catalog::WALL_SET_ASTRA.straight.wall), 0,
        "no Astra walls when using Pipe style"
    );
}

#[test]
fn pipe_style_uses_pipe_corner_assets() {
    let style = RoomStyle::from_wall_set(&asset_catalog::WALL_SET_PIPE);
    let placements = assemble(&small_room(), &[], [0.0, 0.0, 0.0], 4.0, &style);
    let corners: Vec<_> = placements.iter()
        .filter(|p| p.scene == asset_catalog::WALL_SET_PIPE.corner_inner.wall)
        .collect();
    assert_eq!(corners.len(), 4, "sealed 1x1 room should have 4 pipe inner corners");
}

#[test]
fn pipe_style_uses_pipe_ceiling_assets() {
    // Use 3x3 room: 4 non-corner edges get straight ceiling strips.
    let style = RoomStyle::from_wall_set(&asset_catalog::WALL_SET_PIPE);
    let placements = assemble(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0, &style);
    let ceilings: Vec<_> = placements.iter()
        .filter(|p| p.scene == asset_catalog::WALL_SET_PIPE.straight.ceiling)
        .collect();
    assert_eq!(ceilings.len(), 4, "sealed 3x3 room: 4 edge cells get straight ceiling strips, corners don't");
}

#[test]
fn pipe_style_uses_pipe_floor_assets() {
    let style = RoomStyle::from_wall_set(&asset_catalog::WALL_SET_PIPE);
    let placements = assemble(&small_room(), &[], [0.0, 0.0, 0.0], 4.0, &style);
    let pipe_floors: Vec<_> = placements.iter()
        .filter(|p| (p.scene == asset_catalog::WALL_SET_PIPE.straight.floor
                || p.scene == asset_catalog::WALL_SET_PIPE.corner_inner.floor)
                && (p.position[1]).abs() < 0.001)
        .collect();
    assert_eq!(pipe_floors.len(), 1, "1x1 room should have 1 pipe floor");
}

#[test]
fn door_asset_is_always_the_same_regardless_of_style() {
    let style = RoomStyle::from_wall_set(&asset_catalog::WALL_SET_WINDOW);
    let placements = assemble(
        &corridor_ew(),
        &[
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
        ],
        [0.0, 0.0, 0.0],
        4.0,
        &style,
    );
    let doors: Vec<_> = placements.iter()
        .filter(|p| p.scene == asset_catalog::DOOR)
        .collect();
    assert_eq!(doors.len(), 2, "corridor should have 2 doors regardless of style");
}
