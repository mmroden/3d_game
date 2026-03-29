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
    let ez = template.extents[2] as i32;

    for cx in 0..ex {
        for cz in 0..ez {
            let pos = [
                world_origin[0] + cx as f32 * cell_size,
                world_origin[1],
                world_origin[2] + cz as f32 * cell_size,
            ];

            // Floor
            out.push(MeshPlacement { scene: FLOOR, position: pos, rotation_y: 0.0 });

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

                if is_active_connector(template, active_facings, facing, cx, cz) {
                    let (door_pos, door_rot) = door_placement(pos, facing, cell_size);
                    out.push(MeshPlacement { scene: DOOR, position: door_pos, rotation_y: door_rot });
                } else {
                    let (wall_pos, rot) = wall_placement(pos, facing, cell_size);
                    out.push(MeshPlacement { scene: WALL, position: wall_pos, rotation_y: rot });
                    out.push(MeshPlacement { scene: CEILING, position: wall_pos, rotation_y: rot });
                    wall_present[i] = true;
                }
            }

            // Corners at actual cell corners
            let cs = cell_size;
            let [neg_x, pos_x, neg_z, pos_z] = wall_present;
            if neg_x && neg_z {
                out.push(MeshPlacement { scene: CORNER, position: pos, rotation_y: 0.0 });
            }
            if pos_x && neg_z {
                let p = [pos[0] + cs, pos[1], pos[2]];
                out.push(MeshPlacement { scene: CORNER, position: p, rotation_y: FRAC_PI_2 });
            }
            if neg_x && pos_z {
                let p = [pos[0], pos[1], pos[2] + cs];
                out.push(MeshPlacement { scene: CORNER, position: p, rotation_y: -FRAC_PI_2 });
            }
            if pos_x && pos_z {
                let p = [pos[0] + cs, pos[1], pos[2] + cs];
                out.push(MeshPlacement { scene: CORNER, position: p, rotation_y: PI });
            }
        }
    }

    out
}

/// Check whether a connector at cell (cx, cz) with the given facing is
/// both defined in the template AND present in the active list.
fn is_active_connector(
    template: &RoomTemplate,
    active: &[ConnectorFacing],
    facing: ConnectorFacing,
    cx: i32,
    cz: i32,
) -> bool {
    if !active.contains(&facing) {
        return false;
    }
    template.connectors.iter().any(|c| {
        c.facing == facing && c.offset[0] == cx && c.offset[2] == cz
    })
}

/// Position and Y rotation for a wall/ceiling asset.
/// Native mesh sits at NegX edge with pivot at origin, spanning +Z.
/// PosX/PosZ edges need offset to far corner so rotation places geometry correctly.
fn wall_placement(cell_pos: [f32; 3], facing: ConnectorFacing, cell_size: f32) -> ([f32; 3], f32) {
    let cs = cell_size;
    match facing {
        ConnectorFacing::NegX => (cell_pos, 0.0),
        ConnectorFacing::PosX => ([cell_pos[0] + cs, cell_pos[1], cell_pos[2] + cs], PI),
        ConnectorFacing::NegZ => (cell_pos, FRAC_PI_2),
        ConnectorFacing::PosZ => ([cell_pos[0] + cs, cell_pos[1], cell_pos[2] + cs], -FRAC_PI_2),
        _ => (cell_pos, 0.0),
    }
}

/// Position and Y rotation for a door frame.
/// The native door frame spans X (±2.43) at Z≈0.
/// We rotate and translate it to sit at the cell boundary.
fn door_placement(cell_pos: [f32; 3], facing: ConnectorFacing, cell_size: f32) -> ([f32; 3], f32) {
    let half = cell_size / 2.0;
    match facing {
        // Door along Z axis at NegX boundary
        ConnectorFacing::NegX => (
            [cell_pos[0] - half, cell_pos[1], cell_pos[2]],
            FRAC_PI_2,
        ),
        // Door along Z axis at PosX boundary
        ConnectorFacing::PosX => (
            [cell_pos[0] + half, cell_pos[1], cell_pos[2]],
            -FRAC_PI_2,
        ),
        // Door along X axis at NegZ boundary
        ConnectorFacing::NegZ => (
            [cell_pos[0], cell_pos[1], cell_pos[2] - half],
            0.0,
        ),
        // Door along X axis at PosZ boundary
        ConnectorFacing::PosZ => (
            [cell_pos[0], cell_pos[1], cell_pos[2] + half],
            PI,
        ),
        _ => (cell_pos, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::room_template::*;

    fn small_room() -> RoomTemplate {
        RoomTemplate {
            id: "test_small",
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 1, 1],
        }
    }

    fn corridor_ew() -> RoomTemplate {
        RoomTemplate {
            id: "test_corridor_ew",
            kind: TemplateKind::Corridor,
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 1, 1],
        }
    }

    fn large_room() -> RoomTemplate {
        RoomTemplate {
            id: "test_large",
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
                Connector { offset: [1, 0, 0], facing: ConnectorFacing::PosX },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
                Connector { offset: [0, 0, 1], facing: ConnectorFacing::PosZ },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [2, 1, 2],
        }
    }

    fn count(placements: &[MeshPlacement], scene: &str) -> usize {
        placements.iter().filter(|p| p.scene == scene).count()
    }

    #[test]
    fn sealed_small_room_has_4_walls_4_ceilings_4_corners_1_floor() {
        let placements = assemble(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
        assert_eq!(count(&placements, FLOOR), 1);
        assert_eq!(count(&placements, WALL), 4);
        assert_eq!(count(&placements, CEILING), 4);
        assert_eq!(count(&placements, CORNER), 4);
        assert_eq!(count(&placements, DOOR), 0);
    }

    #[test]
    fn one_active_connector_replaces_wall_with_door() {
        let placements = assemble(
            &small_room(),
            &[ConnectorFacing::PosX],
            [0.0, 0.0, 0.0],
            4.0,
        );
        assert_eq!(count(&placements, WALL), 3);
        assert_eq!(count(&placements, CEILING), 3);
        assert_eq!(count(&placements, DOOR), 1);
        // PosX door removes NE and SE corners
        assert_eq!(count(&placements, CORNER), 2);
    }

    #[test]
    fn two_adjacent_active_connectors_remove_shared_corner() {
        let placements = assemble(
            &small_room(),
            &[ConnectorFacing::PosX, ConnectorFacing::PosZ],
            [0.0, 0.0, 0.0],
            4.0,
        );
        assert_eq!(count(&placements, WALL), 2);
        assert_eq!(count(&placements, DOOR), 2);
        // NW corner remains (NegX wall + NegZ wall)
        // NE corner gone (PosX is door)
        // SW corner gone (PosZ is door)
        // SE corner gone (both PosX and PosZ are doors)
        assert_eq!(count(&placements, CORNER), 1);
    }

    #[test]
    fn corridor_with_both_ends_active() {
        let placements = assemble(
            &corridor_ew(),
            &[ConnectorFacing::NegX, ConnectorFacing::PosX],
            [0.0, 0.0, 0.0],
            4.0,
        );
        assert_eq!(count(&placements, FLOOR), 1);
        assert_eq!(count(&placements, DOOR), 2, "doors at both ends");
        assert_eq!(count(&placements, WALL), 2, "walls on NegZ and PosZ sides");
        assert_eq!(count(&placements, CEILING), 2);
        assert_eq!(count(&placements, CORNER), 0, "no corners — no two walls meet");
    }

    #[test]
    fn large_room_sealed_has_4_floors() {
        let placements = assemble(&large_room(), &[], [0.0, 0.0, 0.0], 4.0);
        assert_eq!(count(&placements, FLOOR), 4, "2x2 = 4 floor tiles");
    }

    #[test]
    fn large_room_sealed_walls() {
        let placements = assemble(&large_room(), &[], [0.0, 0.0, 0.0], 4.0);
        // Perimeter of 2x2: 8 wall segments (2 per side)
        assert_eq!(count(&placements, WALL), 8);
    }

    #[test]
    fn large_room_interior_edges_have_no_walls() {
        // In a 2x2 room, cells (0,0)-(1,0) share an interior edge on X.
        // No wall should be placed there.
        let placements = assemble(&large_room(), &[], [0.0, 0.0, 0.0], 4.0);
        // If interior edges had walls, we'd have 16 walls (4 per cell).
        // We have 8, confirming interior edges are skipped.
        assert_eq!(count(&placements, WALL), 8);
    }

    #[test]
    fn large_room_one_connector_active() {
        let placements = assemble(
            &large_room(),
            &[ConnectorFacing::NegX],
            [0.0, 0.0, 0.0],
            4.0,
        );
        // NegX connector is at offset [0,0,0], only cell (0,0) gets a door on NegX
        // Cell (0,1) still has a wall on NegX (no connector at offset [0,0,1])
        assert_eq!(count(&placements, DOOR), 1);
        assert_eq!(count(&placements, WALL), 7);
    }

    #[test]
    fn world_origin_offsets_all_positions() {
        let origin = [10.0, 5.0, 20.0];
        let placements = assemble(&small_room(), &[], origin, 4.0);
        let floor = placements.iter().find(|p| p.scene == FLOOR).unwrap();
        assert_eq!(floor.position, [10.0, 5.0, 20.0]);
    }

    #[test]
    fn door_position_at_cell_boundary() {
        let placements = assemble(
            &small_room(),
            &[ConnectorFacing::NegX],
            [0.0, 0.0, 0.0],
            4.0,
        );
        let door = placements.iter().find(|p| p.scene == DOOR).unwrap();
        // NegX door should be at x = -2.0 (half cell size from origin)
        assert!((door.position[0] - (-2.0)).abs() < 0.001);
    }

    #[test]
    fn negx_wall_at_cell_origin() {
        let placements = assemble(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
        let walls: Vec<_> = placements.iter().filter(|p| p.scene == WALL).collect();
        // NegX wall: placed at cell origin with rotation 0
        let negx = walls.iter().find(|w| w.rotation_y.abs() < 0.001).unwrap();
        assert_eq!(negx.position, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn posx_wall_at_far_corner() {
        let placements = assemble(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
        let walls: Vec<_> = placements.iter().filter(|p| p.scene == WALL).collect();
        // PosX wall: placed at (cs, 0, cs) with rotation PI
        let posx = walls.iter().find(|w| (w.rotation_y - PI).abs() < 0.001).unwrap();
        assert_eq!(posx.position, [4.0, 0.0, 4.0]);
    }

    #[test]
    fn negz_wall_at_cell_origin() {
        let placements = assemble(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
        let walls: Vec<_> = placements.iter().filter(|p| p.scene == WALL).collect();
        // NegZ wall: placed at cell origin with rotation PI/2
        let negz = walls.iter().find(|w| (w.rotation_y - FRAC_PI_2).abs() < 0.001).unwrap();
        assert_eq!(negz.position, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn posz_wall_at_far_corner() {
        let placements = assemble(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
        let walls: Vec<_> = placements.iter().filter(|p| p.scene == WALL).collect();
        // PosZ wall: placed at (cs, 0, cs) with rotation -PI/2
        let posz = walls.iter().find(|w| (w.rotation_y - (-FRAC_PI_2)).abs() < 0.001).unwrap();
        assert_eq!(posz.position, [4.0, 0.0, 4.0]);
    }

    #[test]
    fn corner_positions_at_cell_corners() {
        let placements = assemble(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
        let corners: Vec<_> = placements.iter().filter(|p| p.scene == CORNER).collect();
        assert_eq!(corners.len(), 4);

        // NW at origin
        assert!(corners.iter().any(|c| c.position == [0.0, 0.0, 0.0] && c.rotation_y.abs() < 0.001));
        // NE at (cs, 0, 0)
        assert!(corners.iter().any(|c| c.position == [4.0, 0.0, 0.0] && (c.rotation_y - FRAC_PI_2).abs() < 0.001));
        // SW at (0, 0, cs)
        assert!(corners.iter().any(|c| c.position == [0.0, 0.0, 4.0] && (c.rotation_y - (-FRAC_PI_2)).abs() < 0.001));
        // SE at (cs, 0, cs)
        assert!(corners.iter().any(|c| c.position == [4.0, 0.0, 4.0] && (c.rotation_y - PI).abs() < 0.001));
    }
}
