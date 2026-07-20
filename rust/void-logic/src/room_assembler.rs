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
    /// Boundary skin — walls, floors, ceilings, door frames: render-only,
    /// because the room's watertight cell shell (`shell_slabs`) owns the
    /// physics at that plane. Still a named intent, never an omission
    /// (playtest 2026-07-06: using the render triangles as the collider
    /// gave the walls the art's seam holes).
    Skin,
    /// A solid piece protruding into the room (the curved corner stack):
    /// a `StaticBody3D` with a snug convex hull per mesh. Convex shapes
    /// have a real interior, so a grinding body is always pushed OUT —
    /// the hollow-trimesh cage is unrepresentable.
    ConvexSolid,
}

impl Collision {
    /// Classify a furnished prop: in zero-g everything floats and tumbles
    /// (`Dynamic`) unless it is anchored to a surface — wall/ceiling equipment,
    /// columns, the teleporter pad, cables, holograms — which stays `Static`.
    pub fn for_prop(scene: &str) -> Collision {
        if asset_catalog::is_surface_mounted(scene) {
            Collision::Static
        } else {
            Collision::Dynamic
        }
    }
}

/// A single mesh to place in the level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshPlacement {
    pub scene: crate::asset_catalog::SceneId,
    pub position: [f32; 3],
    pub rotation_x: f32,
    pub rotation_y: f32,
    /// Uniform scale applied at instantiation (1.0 = the scene's own size).
    /// The fixed-environment mesh rides its kit's declared scale here —
    /// never baked into the glb, so retuning is a grammar edit.
    pub scale: f32,
    /// How this mesh collides. Replaces the old `loose` flag: `Dynamic`
    /// is the former `loose: true`; structure is `Static`.
    pub collision: Collision,
}

// ── Watertight room shell ───────────────────────────────────────────────

/// Shell slab thickness (m). Chunky on purpose: anti-tunneling headroom
/// under pile-up pressure.
pub const SHELL_THICKNESS: f32 = 0.5;

/// One axis-aligned solid box of a room's collision shell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellSlab {
    pub center: [f32; 3],
    pub size: [f32; 3],
}

/// Derive the room's watertight collision shell straight from the cell
/// grid: one solid slab per sealed cell face, sitting flush OUTSIDE the
/// boundary plane and bled tangentially by the shell thickness so
/// orthogonal slabs overlap along edges and corners — no diagonal
/// pinholes. Unsealed faces (doorways, shaft mouths) stay open. The
/// render skin is decorative; THIS is the wall the physics knows
/// (playtest 2026-07-06: the fused render-triangle trimesh had a real
/// hole at a corner seam, and hollow trimeshes cage grinding bodies).
pub fn shell_slabs(grid: &CellGrid) -> Vec<ShellSlab> {
    let t = grid.tile;
    let s = grid.story;
    let th = SHELL_THICKNESS;
    let mut out = Vec::new();
    for cell in grid.cells() {
        // world_center: XZ at the cell center, Y at the cell FLOOR.
        let wc = cell.world_center;
        let y_mid = wc[1] + s * 0.5;
        for face in &cell.sealed_faces {
            out.push(match face {
                ConnectorFacing::NegX => ShellSlab {
                    center: [wc[0] - (t + th) * 0.5, y_mid, wc[2]],
                    size: [th, s + 2.0 * th, t + 2.0 * th],
                },
                ConnectorFacing::PosX => ShellSlab {
                    center: [wc[0] + (t + th) * 0.5, y_mid, wc[2]],
                    size: [th, s + 2.0 * th, t + 2.0 * th],
                },
                ConnectorFacing::NegZ => ShellSlab {
                    center: [wc[0], y_mid, wc[2] - (t + th) * 0.5],
                    size: [t + 2.0 * th, s + 2.0 * th, th],
                },
                ConnectorFacing::PosZ => ShellSlab {
                    center: [wc[0], y_mid, wc[2] + (t + th) * 0.5],
                    size: [t + 2.0 * th, s + 2.0 * th, th],
                },
                ConnectorFacing::NegY => ShellSlab {
                    center: [wc[0], wc[1] - th * 0.5, wc[2]],
                    size: [t + 2.0 * th, th, t + 2.0 * th],
                },
                ConnectorFacing::PosY => ShellSlab {
                    center: [wc[0], wc[1] + s + th * 0.5, wc[2]],
                    size: [t + 2.0 * th, th, t + 2.0 * th],
                },
            });
        }
    }
    out
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

/// Panel-world assembly v2 (phase 4): skin every sealed surface from the
/// kit's ROLE POOLS — floors from the floor pool, ceilings from the
/// ceiling pool, wall COURSES covered per-run by the wall pool's width
/// family through [`crate::coverer`]. Watertightness is an AREA
/// invariant (covered == sealed minus openings), not a plate count:
/// wide plates cover several cell faces at once. Deterministic per
/// `room_seed`.
pub fn assemble_role_pools_from_grid(
    grid: &CellGrid,
    pools: &asset_catalog::RolePools,
    room_seed: u64,
) -> Vec<MeshPlacement> {
    use crate::asset_catalog::RolePlate;
    use crate::coverer::{cover_run, split_runs};
    use rand::rngs::SmallRng;
    use rand::{RngExt, SeedableRng};
    use std::f32::consts::{FRAC_PI_2, PI};

    // Salted stream (seed-hygiene standard): never the raw room seed.
    let mut rng = SmallRng::seed_from_u64(room_seed ^ crate::seed::salt::PANEL_COURSE);
    let p = grid.tile;
    let [ex, ey, ez] = grid.extents.map(|e| e as i32);
    let origin = {
        let c0 = grid.cell_at(0, 0, 0).expect("a grid has cells").world_center;
        [c0[0] - p * 0.5, c0[1], c0[2] - p * 0.5]
    };
    let mut out = Vec::new();

    /// The distinct widths a pool offers the coverer.
    fn widths(pool: &[RolePlate]) -> Vec<f32> {
        let mut w: Vec<f32> = pool.iter().map(|pl| pl.face[0]).collect();
        w.sort_by(|a, b| a.total_cmp(b));
        w.dedup_by(|a, b| (*a - *b).abs() < 0.001);
        w
    }
    let wall_widths = widths(&pools.wall);
    let floor_widths = widths(&pools.floor);
    let ceiling_widths = widths(&pools.ceiling);

    // Cover ONE course: a line of `cells` cell-faces (true = sealed),
    // holes split it into runs, each run partitions into plate widths,
    // each width picks uniformly among the pool's plates of that width.
    // `place` maps (center-offset-along-course, plate) to a placement.
    let cover_course = |sealed: &[bool],
                            widths: &[f32],
                            pool: &[RolePlate],
                            rng: &mut SmallRng,
                            out: &mut Vec<MeshPlacement>,
                            place: &dyn Fn(f32, &RolePlate) -> MeshPlacement| {
        let len = sealed.len() as f32 * p;
        let holes: Vec<(f32, f32)> = sealed
            .iter()
            .enumerate()
            .filter(|&(_, &s)| !s)
            .map(|(i, _)| (i as f32 * p, p))
            .collect();
        for (run_off, run_len) in split_runs(len, &holes) {
            let cover = cover_run(run_len, widths, rng).unwrap_or_else(|| {
                panic!(
                    "a {run_len} m run must cover — the filler rule \
                     guarantees it (pool '{}')",
                    pools.id
                )
            });
            let mut cursor = run_off;
            for w in cover {
                let candidates: Vec<&RolePlate> = pool
                    .iter()
                    .filter(|pl| (pl.face[0] - w).abs() < 0.001)
                    .collect();
                let plate = candidates[rng.random_range(0..candidates.len())];
                out.push(place(cursor + w * 0.5, plate));
                cursor += w;
            }
        }
    };

    // WALL COURSES: each side plane, story by story. The baked wall pose
    // is width-X, height-Y, detail +Z; yaw turns +Z INTO the room and
    // the plate seats half its thickness inward off the face plane.
    struct Side {
        facing: ConnectorFacing,
        yaw: f32,
        /// inward unit (world), applied to the seat.
        inward: [f32; 3],
    }
    let sides = [
        Side { facing: ConnectorFacing::NegZ, yaw: 0.0, inward: [0.0, 0.0, 1.0] },
        Side { facing: ConnectorFacing::PosZ, yaw: PI, inward: [0.0, 0.0, -1.0] },
        Side { facing: ConnectorFacing::NegX, yaw: FRAC_PI_2, inward: [1.0, 0.0, 0.0] },
        Side { facing: ConnectorFacing::PosX, yaw: -FRAC_PI_2, inward: [-1.0, 0.0, 0.0] },
    ];
    for side in &sides {
        // The course axis is X for Z-sides, Z for X-sides; the row of
        // boundary cells supplying the sealed mask follows it.
        let along_x = matches!(side.facing, ConnectorFacing::NegZ | ConnectorFacing::PosZ);
        let course_cells = if along_x { ex } else { ez };
        let plane = match side.facing {
            ConnectorFacing::NegZ => origin[2],
            ConnectorFacing::PosZ => origin[2] + ez as f32 * p,
            ConnectorFacing::NegX => origin[0],
            ConnectorFacing::PosX => origin[0] + ex as f32 * p,
            ConnectorFacing::NegY | ConnectorFacing::PosY => {
                unreachable!("`sides` lists only lateral facings")
            }
        };
        for cy in 0..ey {
            let sealed: Vec<bool> = (0..course_cells)
                .map(|i| {
                    let (cx, cz) = match side.facing {
                        ConnectorFacing::NegZ => (i, 0),
                        ConnectorFacing::PosZ => (i, ez - 1),
                        ConnectorFacing::NegX => (0, i),
                        ConnectorFacing::PosX => (ex - 1, i),
                        ConnectorFacing::NegY | ConnectorFacing::PosY => {
                            unreachable!("`sides` lists only lateral facings")
                        }
                    };
                    grid.cell_at(cx, cy, cz)
                        .is_some_and(|c| c.sealed_faces.contains(&side.facing))
                })
                .collect();
            let y = origin[1] + cy as f32 * p + p * 0.5;
            cover_course(
                &sealed,
                &wall_widths,
                &pools.wall,
                &mut rng,
                &mut out,
                &|center, plate| {
                    let seat = plate.thick * 0.5;
                    let (x, z) = if along_x {
                        (origin[0] + center, plane + side.inward[2] * seat)
                    } else {
                        (plane + side.inward[0] * seat, origin[2] + center)
                    };
                    MeshPlacement {
                        scene: plate.scene,
                        position: [x, y, z],
                        rotation_x: 0.0,
                        rotation_y: side.yaw,
                        scale: 1.0,
                        collision: Collision::Skin,
                    }
                },
            );
        }
    }

    // FLOOR AND CEILING STRIPS: one course per cell row along X, at the
    // bottom (detail +Y, seated up off the floor plane) and the top
    // (detail -Y, seated down off the ceiling plane).
    for cz in 0..ez {
        let z = origin[2] + cz as f32 * p + p * 0.5;
        let floor_sealed: Vec<bool> = (0..ex)
            .map(|cx| {
                grid.cell_at(cx, 0, cz)
                    .is_some_and(|c| c.sealed_faces.contains(&ConnectorFacing::NegY))
            })
            .collect();
        cover_course(
            &floor_sealed,
            &floor_widths,
            &pools.floor,
            &mut rng,
            &mut out,
            &|center, plate| MeshPlacement {
                scene: plate.scene,
                position: [origin[0] + center, origin[1] + plate.thick * 0.5, z],
                rotation_x: 0.0,
                rotation_y: 0.0,
                scale: 1.0,
                collision: Collision::Skin,
            },
        );
        let top = origin[1] + ey as f32 * p;
        let ceiling_sealed: Vec<bool> = (0..ex)
            .map(|cx| {
                grid.cell_at(cx, ey - 1, cz)
                    .is_some_and(|c| c.sealed_faces.contains(&ConnectorFacing::PosY))
            })
            .collect();
        cover_course(
            &ceiling_sealed,
            &ceiling_widths,
            &pools.ceiling,
            &mut rng,
            &mut out,
            &|center, plate| MeshPlacement {
                scene: plate.scene,
                position: [origin[0] + center, top - plate.thick * 0.5, z],
                rotation_x: 0.0,
                rotation_y: 0.0,
                scale: 1.0,
                collision: Collision::Skin,
            },
        );
    }

    out
}

/// Build structural geometry from a pre-built cell grid.
///
/// Each sealed XZ face gets the wall stack: Bottom + Wall + Top (straight or
/// corner variants). The ShortWall layer is intentionally omitted — the
/// full-height main wall already covers the lower band, so emitting it there
/// only z-fought. Floor/ceiling are Platform tiles at room boundaries.
pub fn assemble_from_grid(
    grid: &CellGrid,
    template: &RoomTemplate,
    active_connectors: &[Connector],
    wall_set: &WallSet,
    catalog: &asset_catalog::AssetCatalog,
) -> Vec<MeshPlacement> {
    let mut out = Vec::new();
    let ey = grid.extents[1] as i32;
    let door = catalog.fixture(asset_catalog::Fixture::DoorFrame);
    let story_height = grid.story;

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
                        let (door_pos, door_rot) = door_placement(pos, *facing, grid.tile);
                        out.push(MeshPlacement { scene: door, position: door_pos, rotation_x: 0.0, rotation_y: door_rot, scale: 1.0, collision: Collision::Skin });
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
            out.push(MeshPlacement { scene: catalog.const_scene(wall_set.bottom.straight), position: wall_pos, rotation_x: 0.0, rotation_y: rot, scale: 1.0, collision: Collision::Skin });
            out.push(MeshPlacement { scene: catalog.const_scene(wall_set.straight.wall), position: wall_pos, rotation_x: 0.0, rotation_y: rot, scale: 1.0, collision: Collision::Skin });
            out.push(MeshPlacement { scene: catalog.const_scene(wall_set.straight.ceiling), position: wall_pos, rotation_x: 0.0, rotation_y: rot, scale: 1.0, collision: Collision::Skin });
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
                // The corner stack protrudes into the room: each piece is a
                // solid convex hull, never fused trimesh (playtest
                // 2026-07-06 — the corner seam was where bodies leaked).
                // Bottom layer corners
                out.push(MeshPlacement { scene: catalog.const_scene(wall_set.bottom.corner_inner), position: corner_pos, rotation_x: 0.0, rotation_y: rot, scale: 1.0, collision: Collision::ConvexSolid });
                out.push(MeshPlacement { scene: catalog.const_scene(wall_set.bottom.corner_outer), position: corner_pos, rotation_x: 0.0, rotation_y: rot, scale: 1.0, collision: Collision::ConvexSolid });
                // Wall layer corners
                out.push(MeshPlacement { scene: catalog.const_scene(wall_set.corner_inner.wall), position: corner_pos, rotation_x: 0.0, rotation_y: rot, scale: 1.0, collision: Collision::ConvexSolid });
                out.push(MeshPlacement { scene: catalog.const_scene(wall_set.corner_outer.wall), position: corner_pos, rotation_x: 0.0, rotation_y: rot, scale: 1.0, collision: Collision::ConvexSolid });
                // Top layer corners
                out.push(MeshPlacement { scene: catalog.const_scene(wall_set.corner_inner.ceiling), position: corner_pos, rotation_x: 0.0, rotation_y: rot, scale: 1.0, collision: Collision::ConvexSolid });
                out.push(MeshPlacement { scene: catalog.const_scene(wall_set.corner_outer.ceiling), position: corner_pos, rotation_x: 0.0, rotation_y: rot, scale: 1.0, collision: Collision::ConvexSolid });
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
                out.push(MeshPlacement { scene: catalog.const_scene(wall_set.corner_inner.floor), position: pos, rotation_x: 0.0, rotation_y: first_corner_rot, scale: 1.0, collision: Collision::Skin });
            } else {
                out.push(MeshPlacement { scene: catalog.const_scene(wall_set.straight.floor), position: pos, rotation_x: 0.0, rotation_y: 0.0, scale: 1.0, collision: Collision::Skin });
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
                out.push(MeshPlacement { scene: catalog.const_scene(wall_set.corner_inner.floor), position: ceiling_pos, rotation_x: PI, rotation_y: first_corner_rot - FRAC_PI_2, scale: 1.0, collision: Collision::Skin });
            } else {
                out.push(MeshPlacement { scene: catalog.const_scene(wall_set.straight.floor), position: ceiling_pos, rotation_x: PI, rotation_y: 0.0, scale: 1.0, collision: Collision::Skin });
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
