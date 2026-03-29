use std::f32::consts::{FRAC_PI_2, PI};

use crate::systems::asset_catalog;
use crate::systems::room_template::{ConnectorFacing, RoomTemplate, TemplateKind};

// Default Astra asset paths — used by tests via `super::WALL` etc.
#[cfg(test)]
const FLOOR: &str =
    "res://addons/quaternius/modularscifimegakit/platforms/Platform_Simple.gltf";
#[cfg(test)]
const WALL: &str =
    "res://addons/quaternius/modularscifimegakit/walls/WallAstra_Straight.gltf";
#[cfg(test)]
const CEILING: &str =
    "res://addons/quaternius/modularscifimegakit/walls/TopCables_Straight.gltf";
#[cfg(test)]
const CORNER: &str =
    "res://addons/quaternius/modularscifimegakit/walls/WallAstra_Corner_Round_Inner.gltf";
#[cfg(test)]
const DOOR: &str =
    "res://addons/quaternius/modularscifimegakit/platforms/Door_Frame_Square.gltf";
#[cfg(test)]
const FLOOR_CURVE: &str =
    "res://addons/quaternius/modularscifimegakit/platforms/Platform_Simple_Curve.gltf";

/// Per-room visual theme: which wall, corner, ceiling, and floor assets to use.
#[derive(Debug, Clone, Copy)]
pub struct RoomStyle {
    pub wall: &'static str,
    pub corner: &'static str,
    pub ceiling: &'static str,
    pub ceiling_corner: &'static str,
    pub floor: &'static str,
    pub floor_corner: &'static str,
}

impl RoomStyle {
    pub fn from_wall_set(ws: &asset_catalog::WallSet) -> Self {
        Self {
            wall: ws.wall_straight,
            corner: ws.wall_corner_inner,
            ceiling: ws.ceiling_straight,
            ceiling_corner: ws.ceiling_corner,
            floor: ws.floor,
            floor_corner: ws.floor_corner,
        }
    }
}

impl Default for RoomStyle {
    fn default() -> Self {
        Self::from_wall_set(&asset_catalog::WALL_SET_ASTRA)
    }
}

/// Vertical cell height in meters, determined by the Quaternius mesh geometry.
/// Wall + top-strip extend from Y≈0.2 to Y=5.0, so one story is 5m.
/// This is independent of the horizontal cell_size (4m).
pub(crate) const CELL_HEIGHT: f32 = 5.0;

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
/// For corridors, doors appear on boundary edges WITH an active connector.
/// For rooms, active connectors leave a gap (no wall, no door) — the
/// adjacent corridor provides the door frame.
/// Interior edges (between cells of the same multi-cell room) get nothing.
pub fn assemble(
    template: &RoomTemplate,
    active_facings: &[ConnectorFacing],
    world_origin: [f32; 3],
    cell_size: f32,
    style: &RoomStyle,
) -> Vec<MeshPlacement> {
    let mut out = Vec::new();
    let ex = template.extents[0] as i32;
    let ey = template.extents[1] as i32;
    let ez = template.extents[2] as i32;

    let door = asset_catalog::DOOR;

    for cx in 0..ex {
        for cy in 0..ey {
            for cz in 0..ez {
                let pos = [
                    world_origin[0] + (cx as f32 + 0.5) * cell_size,
                    world_origin[1] + cy as f32 * CELL_HEIGHT,
                    world_origin[2] + (cz as f32 + 0.5) * cell_size,
                ];

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
                        if template.kind == TemplateKind::Corridor {
                            let (door_pos, door_rot) = door_placement(pos, facing, cell_size);
                            out.push(MeshPlacement { scene: door, position: door_pos, rotation_x: 0.0, rotation_y: door_rot });
                        }
                    } else {
                        let (wall_pos, rot) = wall_placement(pos, facing, cell_size);
                        out.push(MeshPlacement { scene: style.wall, position: wall_pos, rotation_x: 0.0, rotation_y: rot });
                        out.push(MeshPlacement { scene: style.ceiling, position: wall_pos, rotation_x: 0.0, rotation_y: rot });
                        wall_present[i] = true;
                    }
                }

                // Corners at cell center (center-pivot meshes)
                let [neg_x, pos_x, neg_z, pos_z] = wall_present;
                let corner_rotations = [
                    (neg_x && neg_z, 0.0),
                    (pos_x && neg_z, -FRAC_PI_2),
                    (neg_x && pos_z, FRAC_PI_2),
                    (pos_x && pos_z, PI),
                ];
                let mut corner_rot_y: Option<f32> = None;
                for (present, rot) in corner_rotations {
                    if present {
                        out.push(MeshPlacement { scene: style.corner, position: pos, rotation_x: 0.0, rotation_y: rot });
                        out.push(MeshPlacement { scene: style.ceiling_corner, position: pos, rotation_x: 0.0, rotation_y: rot });
                        corner_rot_y = Some(rot);
                    }
                }

                // Floor — use curved variant at corner cells to match round wall corners
                let is_bottom = cy == 0;
                if is_bottom {
                    if !is_active_connector(template, active_facings, ConnectorFacing::NegY, cx, cy, cz) {
                        if let Some(rot) = corner_rot_y {
                            out.push(MeshPlacement { scene: style.floor_corner, position: pos, rotation_x: 0.0, rotation_y: rot });
                        } else {
                            out.push(MeshPlacement { scene: style.floor, position: pos, rotation_x: 0.0, rotation_y: 0.0 });
                        }
                    } else {
                        out.push(MeshPlacement { scene: door, position: pos, rotation_x: -FRAC_PI_2, rotation_y: 0.0 });
                    }
                }

                // Ceiling tile — use curved variant at corner cells
                let is_top = cy == ey - 1;
                if is_top {
                    let ceiling_pos = [pos[0], pos[1] + CELL_HEIGHT, pos[2]];
                    if !is_active_connector(template, active_facings, ConnectorFacing::PosY, cx, cy, cz) {
                        if let Some(rot) = corner_rot_y {
                            out.push(MeshPlacement { scene: style.floor_corner, position: ceiling_pos, rotation_x: PI, rotation_y: rot });
                        } else {
                            out.push(MeshPlacement { scene: style.floor, position: ceiling_pos, rotation_x: PI, rotation_y: 0.0 });
                        }
                    } else {
                        out.push(MeshPlacement { scene: door, position: ceiling_pos, rotation_x: FRAC_PI_2, rotation_y: 0.0 });
                    }
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
#[path = "room_assembler_tests/mod.rs"]
mod tests;
