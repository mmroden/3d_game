use std::f32::consts::{FRAC_PI_2, PI};

use crate::asset_catalog::{self, Triple};
use crate::cell::{CellGrid, CellKind};
use crate::room_template::{Connector, ConnectorFacing, RoomTemplate};

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
pub const CELL_HEIGHT: f32 = 5.0;

/// A single mesh to place in the level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshPlacement {
    pub scene: &'static str,
    pub position: [f32; 3],
    pub rotation_x: f32,
    pub rotation_y: f32,
}

/// An axis-aligned box collider for physics.
/// `half_extents` is the half-size along each axis BEFORE rotation.
/// `rotation_y` rotates the box around Y (same convention as MeshPlacement).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionBox {
    pub position: [f32; 3],
    pub half_extents: [f32; 3],
    pub rotation_y: f32,
}

/// Wall thickness for collision boxes (meters).
const WALL_THICKNESS: f32 = 0.3;
/// Floor/ceiling thickness for collision boxes (meters).
const SLAB_THICKNESS: f32 = 0.2;

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
    active_connectors: &[Connector],
    world_origin: [f32; 3],
    cell_size: f32,
    style: &RoomStyle,
) -> Vec<MeshPlacement> {
    let grid = CellGrid::new(template, active_connectors, world_origin, cell_size);
    assemble_from_grid(&grid, template, active_connectors, style)
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
    active_connectors: &[Connector],
    style: &RoomStyle,
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

        // Place door frames at active XZ connectors for both rooms and corridors.
        // This creates an airlock-style double door at every junction, ensuring
        // no visual gap between room and corridor.
        // Y-axis connectors leave a clean floor/ceiling opening (no door frame).
        if cell.kind == CellKind::ConnectorGap {
            // Find which XZ facings are active connectors at this cell.
            for facing in &[ConnectorFacing::NegX, ConnectorFacing::PosX,
                           ConnectorFacing::NegZ, ConnectorFacing::PosZ] {
                if is_active_connector(template, active_connectors, *facing,
                    cell.grid_pos[0], cell.grid_pos[1], cell.grid_pos[2])
                {
                    let (door_pos, door_rot) = door_placement(pos, *facing);
                    out.push(MeshPlacement { scene: door, position: door_pos, rotation_x: 0.0, rotation_y: door_rot });
                }
            }
        }

        // Collect faces that participate in a corner pair — these are sealed
        // by corner pieces, so no straight wall is needed.
        let corner_pairs = [
            (ConnectorFacing::NegX, ConnectorFacing::NegZ),
            (ConnectorFacing::PosX, ConnectorFacing::NegZ),
            (ConnectorFacing::NegX, ConnectorFacing::PosZ),
            (ConnectorFacing::PosX, ConnectorFacing::PosZ),
        ];
        let mut corner_faces: [bool; 4] = [false; 4];
        for (i, &(present, _rot)) in corner_rotations.iter().enumerate() {
            if present {
                let (f1, f2) = corner_pairs[i];
                for f in [f1, f2] {
                    match f {
                        ConnectorFacing::NegX => corner_faces[0] = true,
                        ConnectorFacing::PosX => corner_faces[1] = true,
                        ConnectorFacing::NegZ => corner_faces[2] = true,
                        ConnectorFacing::PosZ => corner_faces[3] = true,
                        ConnectorFacing::NegY | ConnectorFacing::PosY => {}
                    }
                }
            }
        }
        let is_corner_face = |f: ConnectorFacing| -> bool {
            match f {
                ConnectorFacing::NegX => corner_faces[0],
                ConnectorFacing::PosX => corner_faces[1],
                ConnectorFacing::NegZ => corner_faces[2],
                ConnectorFacing::PosZ => corner_faces[3],
                ConnectorFacing::NegY | ConnectorFacing::PosY => false,
            }
        };

        // Place straight walls only on sealed XZ faces that are NOT part of a corner.
        // Corner pieces are self-contained and seal the boundary themselves.
        // Y-axis faces are handled by the floor/ceiling code below.
        for &facing in &cell.sealed_faces {
            if matches!(facing, ConnectorFacing::NegY | ConnectorFacing::PosY) {
                continue;
            }
            if is_corner_face(facing) {
                continue;
            }
            let (wall_pos, rot) = wall_placement(pos, facing);
            out.push(MeshPlacement { scene: style.straight.wall, position: wall_pos, rotation_x: 0.0, rotation_y: rot });
            out.push(MeshPlacement { scene: style.straight.ceiling, position: wall_pos, rotation_x: 0.0, rotation_y: rot });
        }

        // Place corner pieces offset from cell center toward interior.
        // The offset is derived from CellExtents asymmetry via the type system.
        let mut corner_placement: Option<([f32; 3], f32)> = None;
        for (i, &(present, rot)) in corner_rotations.iter().enumerate() {
            if present {
                let pair = corner_pairs[i];
                let [ox, oz] = crate::cell_geometry::corner_extents(pair).interior_offset();
                let corner_pos = [pos[0] + ox, pos[1], pos[2] + oz];
                out.push(MeshPlacement { scene: style.corner_inner.wall, position: corner_pos, rotation_x: 0.0, rotation_y: rot });
                out.push(MeshPlacement { scene: style.corner_inner.ceiling, position: corner_pos, rotation_x: 0.0, rotation_y: rot });
                out.push(MeshPlacement { scene: style.corner_outer.wall, position: corner_pos, rotation_x: 0.0, rotation_y: rot });
                out.push(MeshPlacement { scene: style.corner_outer.ceiling, position: corner_pos, rotation_x: 0.0, rotation_y: rot });
                corner_placement = Some((corner_pos, rot));
            }
        }

        // Floor — use curved variant at corner cells (at corner offset position).
        // Active NegY connectors leave a clean opening (no floor tile, no hatch).
        let is_bottom = cy == 0;
        if is_bottom
            && !is_active_connector(template, active_connectors, ConnectorFacing::NegY,
                cell.grid_pos[0], cy, cell.grid_pos[2])
        {
            if let Some((corner_pos, rot)) = corner_placement {
                out.push(MeshPlacement { scene: style.corner_inner.floor, position: corner_pos, rotation_x: 0.0, rotation_y: rot });
            } else {
                out.push(MeshPlacement { scene: style.straight.floor, position: pos, rotation_x: 0.0, rotation_y: 0.0 });
            }
        }

        // Ceiling tile — use curved variant at corner cells (at corner offset position).
        // Active PosY connectors leave a clean opening (no ceiling tile, no hatch).
        // Godot YXZ rotation: Rx(PI) flips Z, so compensate with rot - PI/2
        let is_top = cy == ey - 1;
        if is_top
            && !is_active_connector(template, active_connectors, ConnectorFacing::PosY,
                cell.grid_pos[0], cy, cell.grid_pos[2])
        {
            let ceiling_pos = [pos[0], pos[1] + CELL_HEIGHT, pos[2]];
            if let Some((corner_pos, rot)) = corner_placement {
                let corner_ceiling = [corner_pos[0], corner_pos[1] + CELL_HEIGHT, corner_pos[2]];
                out.push(MeshPlacement { scene: style.corner_inner.floor, position: corner_ceiling, rotation_x: PI, rotation_y: rot - FRAC_PI_2 });
            } else {
                out.push(MeshPlacement { scene: style.straight.floor, position: ceiling_pos, rotation_x: PI, rotation_y: 0.0 });
            }
        }
    }

    out
}

/// Check whether a connector at cell (cx, cy, cz) with the given facing is
/// both defined in the template AND present in the active list.
fn is_active_connector(
    template: &RoomTemplate,
    active: &[Connector],
    facing: ConnectorFacing,
    cx: i32,
    cy: i32,
    cz: i32,
) -> bool {
    let candidate = Connector { offset: [cx, cy, cz], facing };
    active.contains(&candidate)
        && template.connectors.contains(&candidate)
}

pub(crate) fn wall_placement(cell_pos: [f32; 3], facing: ConnectorFacing) -> ([f32; 3], f32) {
    match facing {
        ConnectorFacing::NegX => (cell_pos, 0.0),
        ConnectorFacing::PosX => (cell_pos, PI),
        ConnectorFacing::NegZ => (cell_pos, -FRAC_PI_2),
        ConnectorFacing::PosZ => (cell_pos, FRAC_PI_2),
        ConnectorFacing::NegY | ConnectorFacing::PosY => {
            unreachable!("wall_placement called with Y-axis facing {:?}", facing)
        }
    }
}

pub(crate) fn door_placement(cell_pos: [f32; 3], facing: ConnectorFacing) -> ([f32; 3], f32) {
    match facing {
        ConnectorFacing::NegX => (cell_pos, FRAC_PI_2),
        ConnectorFacing::PosX => (cell_pos, -FRAC_PI_2),
        ConnectorFacing::NegZ => (cell_pos, 0.0),
        ConnectorFacing::PosZ => (cell_pos, PI),
        ConnectorFacing::NegY | ConnectorFacing::PosY => {
            unreachable!("door_placement called with Y-axis facing {:?}", facing)
        }
    }
}

/// Generate collision boxes for a room's sealed boundaries, floor, and ceiling.
///
/// Each sealed XZ face gets a wall collider (thin box at the cell boundary).
/// Bottom cells get a floor slab. Top cells get a ceiling slab.
/// Active connectors leave gaps — no collider on that face.
/// This is independent of the visual mesh system; collision always matches
/// the logical cell structure.
pub fn collision_boxes(
    template: &RoomTemplate,
    active_connectors: &[Connector],
    world_origin: [f32; 3],
    cell_size: f32,
) -> Vec<CollisionBox> {
    let grid = CellGrid::new(template, active_connectors, world_origin, cell_size);
    collision_boxes_from_grid(&grid, template, active_connectors, cell_size)
}

/// Generate collision boxes from a pre-built cell grid.
pub fn collision_boxes_from_grid(
    grid: &CellGrid,
    template: &RoomTemplate,
    active_connectors: &[Connector],
    cell_size: f32,
) -> Vec<CollisionBox> {
    let mut out = Vec::new();
    let ey = grid.extents[1] as i32;
    let half_cell = cell_size / 2.0;
    let half_height = CELL_HEIGHT / 2.0;

    for cell in grid.cells() {
        let pos = cell.world_center;
        let cy = cell.grid_pos[1];

        // XZ wall colliders on sealed faces.
        for &facing in &cell.sealed_faces {
            match facing {
                ConnectorFacing::NegX => {
                    out.push(CollisionBox {
                        position: [pos[0] - half_cell, pos[1] + half_height, pos[2]],
                        half_extents: [WALL_THICKNESS / 2.0, half_height, half_cell],
                        rotation_y: 0.0,
                    });
                }
                ConnectorFacing::PosX => {
                    out.push(CollisionBox {
                        position: [pos[0] + half_cell, pos[1] + half_height, pos[2]],
                        half_extents: [WALL_THICKNESS / 2.0, half_height, half_cell],
                        rotation_y: 0.0,
                    });
                }
                ConnectorFacing::NegZ => {
                    out.push(CollisionBox {
                        position: [pos[0], pos[1] + half_height, pos[2] - half_cell],
                        half_extents: [half_cell, half_height, WALL_THICKNESS / 2.0],
                        rotation_y: 0.0,
                    });
                }
                ConnectorFacing::PosZ => {
                    out.push(CollisionBox {
                        position: [pos[0], pos[1] + half_height, pos[2] + half_cell],
                        half_extents: [half_cell, half_height, WALL_THICKNESS / 2.0],
                        rotation_y: 0.0,
                    });
                }
                ConnectorFacing::NegY | ConnectorFacing::PosY => {
                    // Y-axis sealed faces are handled by floor/ceiling slabs below.
                }
            }
        }

        // Floor slab (bottom of room).
        let is_bottom = cy == 0;
        if is_bottom
            && !is_active_connector(template, active_connectors, ConnectorFacing::NegY,
                cell.grid_pos[0], cy, cell.grid_pos[2])
        {
            out.push(CollisionBox {
                position: [pos[0], pos[1] - SLAB_THICKNESS / 2.0, pos[2]],
                half_extents: [half_cell, SLAB_THICKNESS / 2.0, half_cell],
                rotation_y: 0.0,
            });
        }

        // Ceiling slab (top of room).
        let is_top = cy == ey - 1;
        if is_top
            && !is_active_connector(template, active_connectors, ConnectorFacing::PosY,
                cell.grid_pos[0], cy, cell.grid_pos[2])
        {
            out.push(CollisionBox {
                position: [pos[0], pos[1] + CELL_HEIGHT + SLAB_THICKNESS / 2.0, pos[2]],
                half_extents: [half_cell, SLAB_THICKNESS / 2.0, half_cell],
                rotation_y: 0.0,
            });
        }
    }

    out
}

#[cfg(test)]
#[path = "room_assembler_tests/mod.rs"]
mod tests;
