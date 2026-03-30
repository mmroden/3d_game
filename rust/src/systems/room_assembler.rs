use std::f32::consts::{FRAC_PI_2, PI};

use crate::systems::asset_catalog::{self, Triple};
use crate::systems::cell::{CellGrid, CellKind};
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
    "res://addons/quaternius/modularscifimegakit/walls/TopAstra_Straight.gltf";
#[cfg(test)]
const CORNER: &str =
    "res://addons/quaternius/modularscifimegakit/walls/WallAstra_Corner_Round_Inner.gltf";
#[cfg(test)]
const CORNER_OUTER: &str =
    "res://addons/quaternius/modularscifimegakit/walls/WallAstra_Corner_Round_Outer.gltf";
#[cfg(test)]
const DOOR: &str =
    "res://addons/quaternius/modularscifimegakit/platforms/Door_Frame_Square.gltf";
#[cfg(test)]
const FLOOR_CURVE: &str =
    "res://addons/quaternius/modularscifimegakit/platforms/Platform_Simple_Curve.gltf";

/// Per-room visual theme organized as structural triples (floor, wall, ceiling).
#[derive(Debug, Clone, Copy)]
pub struct RoomStyle {
    pub straight: Triple,
    pub corner_inner: Triple,
    pub corner_outer: Triple,
}

impl RoomStyle {
    pub fn from_wall_set(ws: &asset_catalog::WallSet) -> Self {
        Self {
            straight: ws.straight,
            corner_inner: ws.corner_inner,
            corner_outer: ws.corner_outer,
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
    let grid = CellGrid::new(template, active_facings, world_origin, cell_size);
    assemble_from_grid(&grid, template, active_facings, style, cell_size)
}

/// Build structural geometry from a pre-built cell grid.
///
/// Each cell's `kind` and `sealed_faces` drive geometry decisions:
/// - `BoundaryCorner`: corner pieces + straight walls on all sealed faces
/// - `BoundaryEdge`: straight walls on sealed faces only
/// - `ConnectorGap`: doors (corridors) or gaps (rooms)
/// - `Interior`: no horizontal wall geometry
pub fn assemble_from_grid(
    grid: &CellGrid,
    template: &RoomTemplate,
    active_facings: &[ConnectorFacing],
    style: &RoomStyle,
    _cell_size: f32,
) -> Vec<MeshPlacement> {
    let mut out = Vec::new();
    let ey = grid.extents[1] as i32;
    let door = asset_catalog::DOOR;

    for cell in grid.cells() {
        let pos = cell.world_center;
        let cy = cell.grid_pos[1];

        // Compute corner rotations from sealed faces.
        let has_face = |f: ConnectorFacing| cell.sealed_faces.contains(&f);
        let corner_rotations = [
            (has_face(ConnectorFacing::NegX) && has_face(ConnectorFacing::NegZ), 0.0),
            (has_face(ConnectorFacing::PosX) && has_face(ConnectorFacing::NegZ), -FRAC_PI_2),
            (has_face(ConnectorFacing::NegX) && has_face(ConnectorFacing::PosZ), FRAC_PI_2),
            (has_face(ConnectorFacing::PosX) && has_face(ConnectorFacing::PosZ), PI),
        ];

        // Place doors at active connectors (corridors only).
        if cell.kind == CellKind::ConnectorGap && template.kind == TemplateKind::Corridor {
            // Find which facings are active connectors at this cell.
            for facing in &[ConnectorFacing::NegX, ConnectorFacing::PosX,
                           ConnectorFacing::NegZ, ConnectorFacing::PosZ] {
                if is_active_connector(template, active_facings, *facing,
                    cell.grid_pos[0], cell.grid_pos[1], cell.grid_pos[2])
                {
                    let (door_pos, door_rot) = door_placement(pos, *facing, 0.0);
                    out.push(MeshPlacement { scene: door, position: door_pos, rotation_x: 0.0, rotation_y: door_rot });
                }
            }
        }

        // Place straight walls on ALL sealed faces — including corner cells.
        // Corner pieces render in front of straight walls (the concave curve
        // protrudes past the flat wall plane). Straight walls behind them seal
        // the boundary gaps that corner pieces alone can't cover.
        for &facing in &cell.sealed_faces {
            let (wall_pos, rot) = wall_placement(pos, facing, 0.0);
            out.push(MeshPlacement { scene: style.straight.wall, position: wall_pos, rotation_x: 0.0, rotation_y: rot });
            out.push(MeshPlacement { scene: style.straight.ceiling, position: wall_pos, rotation_x: 0.0, rotation_y: rot });
        }

        // Place corner pieces at cell center. Corner meshes are center-pivoted:
        // the geometry extends ~4.5m from origin into one quadrant, reaching the
        // cell boundary. Rotation selects which quadrant (NegX/NegZ, PosX/NegZ, etc.).
        let mut corner_rot_y: Option<f32> = None;
        for &(present, rot) in &corner_rotations {
            if present {
                out.push(MeshPlacement { scene: style.corner_inner.wall, position: pos, rotation_x: 0.0, rotation_y: rot });
                out.push(MeshPlacement { scene: style.corner_inner.ceiling, position: pos, rotation_x: 0.0, rotation_y: rot });
                out.push(MeshPlacement { scene: style.corner_outer.wall, position: pos, rotation_x: 0.0, rotation_y: rot });
                out.push(MeshPlacement { scene: style.corner_outer.ceiling, position: pos, rotation_x: 0.0, rotation_y: rot });
                corner_rot_y = Some(rot);
            }
        }

        // Floor — use curved variant at corner cells.
        let is_bottom = cy == 0;
        if is_bottom {
            if !is_active_connector(template, active_facings, ConnectorFacing::NegY,
                cell.grid_pos[0], cy, cell.grid_pos[2])
            {
                if let Some(rot) = corner_rot_y {
                    out.push(MeshPlacement { scene: style.corner_inner.floor, position: pos, rotation_x: 0.0, rotation_y: rot });
                } else {
                    out.push(MeshPlacement { scene: style.straight.floor, position: pos, rotation_x: 0.0, rotation_y: 0.0 });
                }
            } else {
                out.push(MeshPlacement { scene: door, position: pos, rotation_x: -FRAC_PI_2, rotation_y: 0.0 });
            }
        }

        // Ceiling tile — use curved variant at corner cells.
        // Godot YXZ rotation: Rx(PI) flips Z, so compensate with rot - PI/2
        let is_top = cy == ey - 1;
        if is_top {
            let ceiling_pos = [pos[0], pos[1] + CELL_HEIGHT, pos[2]];
            if !is_active_connector(template, active_facings, ConnectorFacing::PosY,
                cell.grid_pos[0], cy, cell.grid_pos[2])
            {
                if let Some(rot) = corner_rot_y {
                    out.push(MeshPlacement { scene: style.corner_inner.floor, position: ceiling_pos, rotation_x: PI, rotation_y: rot - FRAC_PI_2 });
                } else {
                    out.push(MeshPlacement { scene: style.straight.floor, position: ceiling_pos, rotation_x: PI, rotation_y: 0.0 });
                }
            } else {
                out.push(MeshPlacement { scene: door, position: ceiling_pos, rotation_x: FRAC_PI_2, rotation_y: 0.0 });
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
