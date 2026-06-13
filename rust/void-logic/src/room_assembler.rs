use std::f32::consts::{FRAC_PI_2, PI};

use crate::asset_catalog::{self, WallSet};
use crate::cell::{CellGrid, CellKind};
use crate::room_template::{Connector, ConnectorFacing, FrameStyle, RoomTemplate};

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


/// A single mesh to place in the level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshPlacement {
    pub scene: &'static str,
    pub position: [f32; 3],
    pub rotation_x: f32,
    pub rotation_y: f32,
    /// Loose props float freely in zero-g. Structural meshes and anchored
    /// equipment stay fixed.
    pub loose: bool,
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

// ── Corner offset geometry ──────────────────────────────────────────────

/// Corner inner wall: x ∈ [-4.465, 0.0], z ∈ [-4.468, 0.0]
const CORNER_REACH: f32 = 4.468;
/// Straight wall / floor platform: [-2.0, 2.0] × [-2.0, 2.0]
const INTERIOR_HALF: f32 = 2.0;

/// Compute the XZ offset to push corner pieces from cell center toward interior.
fn corner_interior_offset(pair: (ConnectorFacing, ConnectorFacing)) -> [f32; 2] {
    let (a, b) = pair;
    let neg_x = if a == ConnectorFacing::NegX || b == ConnectorFacing::NegX { CORNER_REACH } else { INTERIOR_HALF };
    let pos_x = if a == ConnectorFacing::PosX || b == ConnectorFacing::PosX { CORNER_REACH } else { INTERIOR_HALF };
    let neg_z = if a == ConnectorFacing::NegZ || b == ConnectorFacing::NegZ { CORNER_REACH } else { INTERIOR_HALF };
    let pos_z = if a == ConnectorFacing::PosZ || b == ConnectorFacing::PosZ { CORNER_REACH } else { INTERIOR_HALF };

    let ox = if neg_x > pos_x { pos_x } else if pos_x > neg_x { -neg_x } else { 0.0 };
    let oz = if neg_z > pos_z { pos_z } else if pos_z > neg_z { -neg_z } else { 0.0 };
    [ox, oz]
}

// ── Assembly ─────────────────────────────────────────────────────────────

/// Build all geometry for a room. Dimensions derived from `wall_set`.
///
/// Returns mesh placements for floors, walls (5-layer stack), ceilings,
/// corners, and doors.
pub fn assemble(
    template: &RoomTemplate,
    active_connectors: &[Connector],
    world_origin: [f32; 3],
    wall_set: &WallSet,
) -> Vec<MeshPlacement> {
    let grid = CellGrid::new(template, active_connectors, world_origin, wall_set.tile_width);
    assemble_from_grid(&grid, template, active_connectors, wall_set)
}

/// Build structural geometry from a pre-built cell grid.
///
/// Each sealed XZ face gets the full 5-layer wall stack:
/// Bottom + ShortWall + Wall + Top (straight or corner variants).
/// Floor/ceiling are Platform tiles at room boundaries.
pub fn assemble_from_grid(
    grid: &CellGrid,
    template: &RoomTemplate,
    active_connectors: &[Connector],
    wall_set: &WallSet,
) -> Vec<MeshPlacement> {
    let mut out = Vec::new();
    let ey = grid.extents[1] as i32;
    let door = asset_catalog::DOOR;
    let story_height = wall_set.story_height;

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

        // Place door frames at active XZ connectors (only if FrameStyle::Door).
        if cell.kind == CellKind::ConnectorGap {
            for facing in &[ConnectorFacing::NegX, ConnectorFacing::PosX,
                           ConnectorFacing::NegZ, ConnectorFacing::PosZ] {
                if let Some(frame) = active_connector_frame(template, active_connectors, *facing,
                    cell.grid_pos[0], cell.grid_pos[1], cell.grid_pos[2])
                {
                    if frame == FrameStyle::Door {
                        let (door_pos, door_rot) = door_placement(pos, *facing, wall_set.tile_width);
                        out.push(MeshPlacement { scene: door, position: door_pos, rotation_x: 0.0, rotation_y: door_rot, loose: false });
                    }
                }
            }
        }

        // Collect faces that participate in a corner pair.
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

        // Place straight walls (5-layer stack) on sealed XZ faces NOT part of a corner.
        for &facing in &cell.sealed_faces {
            if matches!(facing, ConnectorFacing::NegY | ConnectorFacing::PosY) {
                continue;
            }
            if is_corner_face(facing) {
                continue;
            }
            let (wall_pos, rot) = wall_placement(pos, facing);
            out.push(MeshPlacement { scene: wall_set.bottom.straight, position: wall_pos, rotation_x: 0.0, rotation_y: rot, loose: false });
            out.push(MeshPlacement { scene: wall_set.short_wall.straight, position: wall_pos, rotation_x: 0.0, rotation_y: rot, loose: false });
            out.push(MeshPlacement { scene: wall_set.straight.wall, position: wall_pos, rotation_x: 0.0, rotation_y: rot, loose: false });
            out.push(MeshPlacement { scene: wall_set.straight.ceiling, position: wall_pos, rotation_x: 0.0, rotation_y: rot, loose: false });
        }

        // Place corner pieces (5-layer stack) offset from cell center toward interior.
        // Track whether this cell has any corner, and the rotation of the first one
        // (used to orient the curved floor/ceiling platform).
        let mut has_corner = false;
        let mut first_corner_rot: f32 = 0.0;
        for (i, &(present, rot)) in corner_rotations.iter().enumerate() {
            if present {
                let pair = corner_pairs[i];
                let [ox, oz] = corner_interior_offset(pair);
                let corner_pos = [pos[0] + ox, pos[1], pos[2] + oz];
                // Bottom layer corners
                out.push(MeshPlacement { scene: wall_set.bottom.corner_inner, position: corner_pos, rotation_x: 0.0, rotation_y: rot, loose: false });
                out.push(MeshPlacement { scene: wall_set.bottom.corner_outer, position: corner_pos, rotation_x: 0.0, rotation_y: rot, loose: false });
                // ShortWall layer corners
                out.push(MeshPlacement { scene: wall_set.short_wall.corner_inner, position: corner_pos, rotation_x: 0.0, rotation_y: rot, loose: false });
                out.push(MeshPlacement { scene: wall_set.short_wall.corner_outer, position: corner_pos, rotation_x: 0.0, rotation_y: rot, loose: false });
                // Wall layer corners
                out.push(MeshPlacement { scene: wall_set.corner_inner.wall, position: corner_pos, rotation_x: 0.0, rotation_y: rot, loose: false });
                out.push(MeshPlacement { scene: wall_set.corner_outer.wall, position: corner_pos, rotation_x: 0.0, rotation_y: rot, loose: false });
                // Top layer corners
                out.push(MeshPlacement { scene: wall_set.corner_inner.ceiling, position: corner_pos, rotation_x: 0.0, rotation_y: rot, loose: false });
                out.push(MeshPlacement { scene: wall_set.corner_outer.ceiling, position: corner_pos, rotation_x: 0.0, rotation_y: rot, loose: false });
                if !has_corner {
                    first_corner_rot = rot;
                }
                has_corner = true;
            }
        }

        // Floor — curved variant at corner cells, always at cell center.
        let is_bottom = cy == 0;
        if is_bottom
            && !is_active_connector(template, active_connectors, ConnectorFacing::NegY,
                cell.grid_pos[0], cy, cell.grid_pos[2])
        {
            if has_corner {
                out.push(MeshPlacement { scene: wall_set.corner_inner.floor, position: pos, rotation_x: 0.0, rotation_y: first_corner_rot, loose: false });
            } else {
                out.push(MeshPlacement { scene: wall_set.straight.floor, position: pos, rotation_x: 0.0, rotation_y: 0.0, loose: false });
            }
        }

        // Ceiling tile — curved variant at corner cells, always at cell center.
        // Godot YXZ rotation: Rx(PI) flips Z, so compensate with rot - PI/2
        let is_top = cy == ey - 1;
        if is_top
            && !is_active_connector(template, active_connectors, ConnectorFacing::PosY,
                cell.grid_pos[0], cy, cell.grid_pos[2])
        {
            let ceiling_pos = [pos[0], pos[1] + story_height, pos[2]];
            if has_corner {
                out.push(MeshPlacement { scene: wall_set.corner_inner.floor, position: ceiling_pos, rotation_x: PI, rotation_y: first_corner_rot - FRAC_PI_2, loose: false });
            } else {
                out.push(MeshPlacement { scene: wall_set.straight.floor, position: ceiling_pos, rotation_x: PI, rotation_y: 0.0, loose: false });
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
    let matches = |c: &Connector| c.offset == [cx, cy, cz] && c.facing == facing;
    active.iter().any(matches)
        && template.connectors.iter().any(matches)
}

/// Like `is_active_connector`, but returns the connector's `FrameStyle` if active.
fn active_connector_frame(
    template: &RoomTemplate,
    active: &[Connector],
    facing: ConnectorFacing,
    cx: i32,
    cy: i32,
    cz: i32,
) -> Option<FrameStyle> {
    let matches = |c: &Connector| c.offset == [cx, cy, cz] && c.facing == facing;
    let in_active = active.iter().any(matches);
    if !in_active { return None; }
    template.connectors.iter()
        .find(|c| matches(c))
        .map(|c| c.frame)
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

pub(crate) fn door_placement(cell_pos: [f32; 3], facing: ConnectorFacing, cell_size: f32) -> ([f32; 3], f32) {
    let half = cell_size / 2.0;
    match facing {
        ConnectorFacing::NegX => ([cell_pos[0] - half, cell_pos[1], cell_pos[2]], FRAC_PI_2),
        ConnectorFacing::PosX => ([cell_pos[0] + half, cell_pos[1], cell_pos[2]], -FRAC_PI_2),
        ConnectorFacing::NegZ => ([cell_pos[0], cell_pos[1], cell_pos[2] - half], 0.0),
        ConnectorFacing::PosZ => ([cell_pos[0], cell_pos[1], cell_pos[2] + half], PI),
        ConnectorFacing::NegY | ConnectorFacing::PosY => {
            unreachable!("door_placement called with Y-axis facing {:?}", facing)
        }
    }
}

/// Generate collision boxes for a room's sealed boundaries, floor, and ceiling.
pub fn collision_boxes(
    template: &RoomTemplate,
    active_connectors: &[Connector],
    world_origin: [f32; 3],
    wall_set: &WallSet,
) -> Vec<CollisionBox> {
    let grid = CellGrid::new(template, active_connectors, world_origin, wall_set.tile_width);
    collision_boxes_from_grid(&grid, template, active_connectors, wall_set)
}

/// Generate collision boxes from a pre-built cell grid.
pub fn collision_boxes_from_grid(
    grid: &CellGrid,
    template: &RoomTemplate,
    active_connectors: &[Connector],
    wall_set: &WallSet,
) -> Vec<CollisionBox> {
    let mut out = Vec::new();
    let ey = grid.extents[1] as i32;
    let half_cell = wall_set.tile_width / 2.0;
    let story_height = wall_set.story_height;
    let half_height = story_height / 2.0;

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
                ConnectorFacing::NegY | ConnectorFacing::PosY => {}
            }
        }

        // Floor slab.
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

        // Ceiling slab.
        let is_top = cy == ey - 1;
        if is_top
            && !is_active_connector(template, active_connectors, ConnectorFacing::PosY,
                cell.grid_pos[0], cy, cell.grid_pos[2])
        {
            out.push(CollisionBox {
                position: [pos[0], pos[1] + story_height + SLAB_THICKNESS / 2.0, pos[2]],
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
