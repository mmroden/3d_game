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


/// How a placed mesh participates in the physics world. Every placement
/// must declare one — there is no way to emit a renderable without a
/// collision intent, so "a mesh with no collider" is unrepresentable.
/// The Godot shell turns each variant into a body + collider (or none)
/// at scene-build time; the engine owns the simulation from there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Collision {
    /// Fixed structure and anchored equipment: a `StaticBody3D`, collider
    /// derived from the mesh.
    Static,
    /// Loose props that float and tumble in zero-g: a `RigidBody3D`,
    /// collider derived from the mesh.
    Dynamic,
    /// Decorative only — no collider (cables, holograms, light fixtures).
    /// A deliberate, named choice, never an omission.
    Passable,
}

impl Collision {
    /// Classify a furnished prop: loose debris tumbles (`Dynamic`);
    /// everything else the furnisher places is anchored equipment fixed
    /// in place (`Static`).
    pub fn for_prop(scene: &str) -> Collision {
        if asset_catalog::is_loose_prop(scene) {
            Collision::Dynamic
        } else {
            Collision::Static
        }
    }
}

/// A single mesh to place in the level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshPlacement {
    pub scene: &'static str,
    pub position: [f32; 3],
    pub rotation_x: f32,
    pub rotation_y: f32,
    /// How this mesh collides. Replaces the old `loose` flag: `Dynamic`
    /// is the former `loose: true`; structure is `Static`.
    pub collision: Collision,
}

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

    // A vertical shaft (an up/down corridor) reads as a square right-angle
    // tube: straight walls on every sealed face instead of rounded corner
    // pieces. Ordinary rooms keep their curves.
    let square_shaft = template.kind == crate::room_template::TemplateKind::Corridor
        && template.connectors.iter().any(|c| {
            matches!(c.facing, ConnectorFacing::PosY | ConnectorFacing::NegY)
        });

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
                        out.push(MeshPlacement { scene: door, position: door_pos, rotation_x: 0.0, rotation_y: door_rot, collision: Collision::Static });
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
            // In a square shaft every sealed face gets a straight wall, so
            // perpendicular faces meet at a hard 90° corner.
            if is_corner_face(facing) && !square_shaft {
                continue;
            }
            let (wall_pos, rot) = wall_placement(pos, facing);
            out.push(MeshPlacement { scene: wall_set.bottom.straight, position: wall_pos, rotation_x: 0.0, rotation_y: rot, collision: Collision::Static });
            out.push(MeshPlacement { scene: wall_set.short_wall.straight, position: wall_pos, rotation_x: 0.0, rotation_y: rot, collision: Collision::Static });
            out.push(MeshPlacement { scene: wall_set.straight.wall, position: wall_pos, rotation_x: 0.0, rotation_y: rot, collision: Collision::Static });
            out.push(MeshPlacement { scene: wall_set.straight.ceiling, position: wall_pos, rotation_x: 0.0, rotation_y: rot, collision: Collision::Static });
        }

        // Place corner pieces (5-layer stack) offset from cell center toward interior.
        // Track whether this cell has any corner, and the rotation of the first one
        // (used to orient the curved floor/ceiling platform).
        let mut has_corner = false;
        let mut first_corner_rot: f32 = 0.0;
        for (i, &(present, rot)) in corner_rotations.iter().enumerate() {
            // A square shaft emits no rounded corner pieces; its corners are
            // formed by the straight walls placed above. has_corner stays
            // false, so the (absent) floor/ceiling would use straight tiles.
            if present && !square_shaft {
                let pair = corner_pairs[i];
                let [ox, oz] = corner_interior_offset(pair);
                let corner_pos = [pos[0] + ox, pos[1], pos[2] + oz];
                // Bottom layer corners
                out.push(MeshPlacement { scene: wall_set.bottom.corner_inner, position: corner_pos, rotation_x: 0.0, rotation_y: rot, collision: Collision::Static });
                out.push(MeshPlacement { scene: wall_set.bottom.corner_outer, position: corner_pos, rotation_x: 0.0, rotation_y: rot, collision: Collision::Static });
                // ShortWall layer corners
                out.push(MeshPlacement { scene: wall_set.short_wall.corner_inner, position: corner_pos, rotation_x: 0.0, rotation_y: rot, collision: Collision::Static });
                out.push(MeshPlacement { scene: wall_set.short_wall.corner_outer, position: corner_pos, rotation_x: 0.0, rotation_y: rot, collision: Collision::Static });
                // Wall layer corners
                out.push(MeshPlacement { scene: wall_set.corner_inner.wall, position: corner_pos, rotation_x: 0.0, rotation_y: rot, collision: Collision::Static });
                out.push(MeshPlacement { scene: wall_set.corner_outer.wall, position: corner_pos, rotation_x: 0.0, rotation_y: rot, collision: Collision::Static });
                // Top layer corners
                out.push(MeshPlacement { scene: wall_set.corner_inner.ceiling, position: corner_pos, rotation_x: 0.0, rotation_y: rot, collision: Collision::Static });
                out.push(MeshPlacement { scene: wall_set.corner_outer.ceiling, position: corner_pos, rotation_x: 0.0, rotation_y: rot, collision: Collision::Static });
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
                out.push(MeshPlacement { scene: wall_set.corner_inner.floor, position: pos, rotation_x: 0.0, rotation_y: first_corner_rot, collision: Collision::Static });
            } else {
                out.push(MeshPlacement { scene: wall_set.straight.floor, position: pos, rotation_x: 0.0, rotation_y: 0.0, collision: Collision::Static });
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
                out.push(MeshPlacement { scene: wall_set.corner_inner.floor, position: ceiling_pos, rotation_x: PI, rotation_y: first_corner_rot - FRAC_PI_2, collision: Collision::Static });
            } else {
                out.push(MeshPlacement { scene: wall_set.straight.floor, position: ceiling_pos, rotation_x: PI, rotation_y: 0.0, collision: Collision::Static });
            }
        }
    }

    out
}

/// Whether cell (cx, cy, cz) is covered by the opening of an active
/// connector with the given facing. A connector's opening is a
/// `facing.opening_span()`-wide footprint anchored at its offset and
/// extending toward +x/+z (so vertical openings are 2×2, horizontal ones
/// one cell). The single check shared by the cell grid (aperture
/// classification) and assembly (floor/ceiling removal) — one source of
/// truth for "where an opening is."
pub(crate) fn is_active_connector(
    template: &RoomTemplate,
    active: &[Connector],
    facing: ConnectorFacing,
    cx: i32,
    cy: i32,
    cz: i32,
) -> bool {
    let span = facing.opening_span();
    let covers = |c: &Connector| {
        c.facing == facing
            && c.offset[1] == cy
            && cx >= c.offset[0]
            && cx < c.offset[0] + span
            && cz >= c.offset[2]
            && cz < c.offset[2] + span
    };
    active.iter().any(covers) && template.connectors.iter().any(covers)
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


#[cfg(test)]
#[path = "room_assembler_tests/mod.rs"]
mod tests;
