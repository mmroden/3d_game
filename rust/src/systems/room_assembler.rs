use std::f32::consts::{FRAC_PI_2, PI};

use crate::systems::room_template::{ConnectorFacing, RoomTemplate};

// Quaternius Modular Sci-Fi MegaKit asset paths
const FLOOR: &str =
    "res://addons/quaternius/modularscifimegakit/platforms/Platform_Simple.gltf";
const WALL: &str =
    "res://addons/quaternius/modularscifimegakit/walls/WallAstra_Straight.gltf";
const CEILING: &str =
    "res://addons/quaternius/modularscifimegakit/walls/TopCables_Straight.gltf";
const CORNER: &str =
    "res://addons/quaternius/modularscifimegakit/walls/WallAstra_Corner_Square_Inner.gltf";
const DOOR: &str =
    "res://addons/quaternius/modularscifimegakit/platforms/Door_Frame_Square.gltf";

/// A single mesh to place in the level.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshPlacement {
    pub scene: &'static str,
    pub position: [f32; 3],
    pub rotation_x: f32,
    pub rotation_y: f32,
}

/// Build all geometry for a room based on its template, which connectors
/// are actively connected to neighbors, the world origin, and the cell size.
///
/// Returns mesh placements for floors, walls, ceilings, corners, and doors.
/// Walls appear on boundary edges that lack an active connector.
/// Doors appear on boundary edges WITH an active connector.
/// Interior edges (between cells of the same multi-cell room) get nothing.
pub fn assemble(
    template: &RoomTemplate,
    active_facings: &[ConnectorFacing],
    world_origin: [f32; 3],
    cell_size: f32,
) -> Vec<MeshPlacement> {
    let mut out = Vec::new();
    let ex = template.extents[0] as i32;
    let ey = template.extents[1] as i32;
    let ez = template.extents[2] as i32;

    for cx in 0..ex {
        for cy in 0..ey {
            for cz in 0..ez {
                let pos = [
                    world_origin[0] + (cx as f32 + 0.5) * cell_size,
                    world_origin[1] + cy as f32 * cell_size,
                    world_origin[2] + (cz as f32 + 0.5) * cell_size,
                ];

                // Floor — only on bottom boundary, or if NegY is not active at this cell
                let is_bottom = cy == 0;
                if is_bottom {
                    if !is_active_connector(template, active_facings, ConnectorFacing::NegY, cx, cy, cz) {
                        out.push(MeshPlacement { scene: FLOOR, position: pos, rotation_x: 0.0, rotation_y: 0.0 });
                    } else {
                        let door_pos = [pos[0], pos[1], pos[2]];
                        out.push(MeshPlacement { scene: DOOR, position: door_pos, rotation_x: -FRAC_PI_2, rotation_y: 0.0 });
                    }
                } else {
                    // Interior Y-edge: no floor between stories (open space)
                }

                // Ceiling tile — only on top boundary, gated on PosY connector
                let is_top = cy == ey - 1;
                if is_top {
                    let ceiling_pos = [pos[0], pos[1] + cell_size, pos[2]];
                    if !is_active_connector(template, active_facings, ConnectorFacing::PosY, cx, cy, cz) {
                        out.push(MeshPlacement { scene: FLOOR, position: ceiling_pos, rotation_x: PI, rotation_y: 0.0 });
                    } else {
                        out.push(MeshPlacement { scene: DOOR, position: ceiling_pos, rotation_x: FRAC_PI_2, rotation_y: 0.0 });
                    }
                }

                // Four horizontal edges — index order: NegX, PosX, NegZ, PosZ
                let boundary = [
                    (ConnectorFacing::NegX, cx == 0),
                    (ConnectorFacing::PosX, cx == ex - 1),
                    (ConnectorFacing::NegZ, cz == 0),
                    (ConnectorFacing::PosZ, cz == ez - 1),
                ];

                let mut wall_present = [false; 4];

                for (i, &(facing, is_boundary)) in boundary.iter().enumerate() {
                    if !is_boundary {
                        continue;
                    }

                    if is_active_connector(template, active_facings, facing, cx, cy, cz) {
                        let (door_pos, door_rot) = door_placement(pos, facing, cell_size);
                        out.push(MeshPlacement { scene: DOOR, position: door_pos, rotation_x: 0.0, rotation_y: door_rot });
                    } else {
                        let (wall_pos, rot) = wall_placement(pos, facing, cell_size);
                        out.push(MeshPlacement { scene: WALL, position: wall_pos, rotation_x: 0.0, rotation_y: rot });
                        out.push(MeshPlacement { scene: CEILING, position: wall_pos, rotation_x: 0.0, rotation_y: rot });
                        wall_present[i] = true;
                    }
                }

                // Corners at cell center (center-pivot meshes)
                let [neg_x, pos_x, neg_z, pos_z] = wall_present;
                if neg_x && neg_z {
                    out.push(MeshPlacement { scene: CORNER, position: pos, rotation_x: 0.0, rotation_y: 0.0 });
                }
                if pos_x && neg_z {
                    out.push(MeshPlacement { scene: CORNER, position: pos, rotation_x: 0.0, rotation_y: -FRAC_PI_2 });
                }
                if neg_x && pos_z {
                    out.push(MeshPlacement { scene: CORNER, position: pos, rotation_x: 0.0, rotation_y: FRAC_PI_2 });
                }
                if pos_x && pos_z {
                    out.push(MeshPlacement { scene: CORNER, position: pos, rotation_x: 0.0, rotation_y: PI });
                }
            }
        }
    }

    out
}

/// Check whether a connector at cell (cx, cy, cz) with the given facing is
/// both defined in the template AND present in the active list.
fn is_active_connector(
    template: &RoomTemplate,
    active: &[ConnectorFacing],
    facing: ConnectorFacing,
    cx: i32,
    cy: i32,
    cz: i32,
) -> bool {
    if !active.contains(&facing) {
        return false;
    }
    template.connectors.iter().any(|c| {
        c.facing == facing && c.offset[0] == cx && c.offset[1] == cy && c.offset[2] == cz
    })
}

pub(crate) fn wall_placement(cell_pos: [f32; 3], facing: ConnectorFacing, _cell_size: f32) -> ([f32; 3], f32) {
    match facing {
        ConnectorFacing::NegX => (cell_pos, 0.0),
        ConnectorFacing::PosX => (cell_pos, PI),
        ConnectorFacing::NegZ => (cell_pos, -FRAC_PI_2),
        ConnectorFacing::PosZ => (cell_pos, FRAC_PI_2),
        _ => (cell_pos, 0.0),
    }
}

pub(crate) fn door_placement(cell_pos: [f32; 3], facing: ConnectorFacing, _cell_size: f32) -> ([f32; 3], f32) {
    match facing {
        ConnectorFacing::NegX => (cell_pos, FRAC_PI_2),
        ConnectorFacing::PosX => (cell_pos, -FRAC_PI_2),
        ConnectorFacing::NegZ => (cell_pos, 0.0),
        ConnectorFacing::PosZ => (cell_pos, PI),
        _ => (cell_pos, 0.0),
    }
}

#[cfg(test)]
#[path = "room_assembler_tests.rs"]
mod tests;
