use crate::room_assembler::{Collision, MeshPlacement};
use crate::room_furnisher::RoomDensity;
use crate::room_template::ConnectorFacing;
use crate::room_theme::RoomTheme;
use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use super::{CellGrid, CellKind, CellOccupant};

impl CellGrid {
    /// Populate cells with props based on the room theme.
    ///
    /// Each eligible cell gets at most one occupant, chosen from the theme's
    /// palette based on the cell's kind. Density controls probability.
    /// ConnectorGap cells stay empty. Blocking props skip reserved path cells.
    pub fn populate(&mut self, theme: &RoomTheme, seed: u64) {
        // Density thresholds: (numerator, denominator) per category.
        let (wall_num, wall_den, center_num, center_den, corner_num, corner_den) = match theme.density {
            RoomDensity::Sparse  => (1usize, 5usize, 1, 8, 1, 6),
            RoomDensity::Normal  => (1, 2, 1, 3, 1, 3),
            RoomDensity::Dense   => (4, 5, 3, 5, 2, 3),
        };

        let mut rng = SmallRng::seed_from_u64(seed);
        // Separate stream for free-standing props' yaw, so giving them random
        // facing doesn't perturb the placement RNG (and the level layout).
        // Dynamic props get their own full tumble in the node shell; this is for
        // the static free-standing ones (e.g. teleporters, holograms) that would
        // otherwise all face the same way.
        let mut orient = SmallRng::seed_from_u64(seed ^ 0x9E37_79B9_7F4A_7C15);

        // Collect ConnectorGap positions so we can skip columns adjacent to entrances.
        let gap_positions: std::collections::HashSet<[i32; 3]> = self.cells.iter()
            .filter(|c| c.kind == CellKind::ConnectorGap)
            .map(|c| c.grid_pos)
            .collect();

        for cell in &mut self.cells {
            // Skip connector gaps — must stay clear for passage.
            if cell.kind == CellKind::ConnectorGap {
                continue;
            }

            match cell.kind {
                CellKind::BoundaryCorner => {
                    // Skip columns near entrances (within 2 cells on XZ plane).
                    // Ignore Y: columns stack through all stories, and gaps
                    // may exist at any Y level.
                    let near_gap = gap_positions.iter().any(|gap| {
                        let dx = (gap[0] - cell.grid_pos[0]).abs();
                        let dz = (gap[2] - cell.grid_pos[2]).abs();
                        dx <= 2 && dz <= 2
                    });
                    if !theme.palette.corner.is_empty() && !near_gap && rng.random_range(0..corner_den) < corner_num {
                        let prop = &theme.palette.corner[rng.random_range(0..theme.palette.corner.len())];
                        let is_column = prop.scene.contains("/columns/");
                        let collision = Collision::for_prop(prop.scene);
                        if is_column {
                            // Stack columns at every story level for this XZ position.
                            let story_height = Self::DEFAULT_STORY_HEIGHT;
                            let ey = self.extents[1];
                            let base_y = cell.world_center[1] - cell.grid_pos[1] as f32 * story_height;
                            let placements: Vec<MeshPlacement> = (0..ey).map(|cy| {
                                MeshPlacement {
                                    scene: prop.scene,
                                    position: [
                                        cell.world_center[0],
                                        base_y + cy as f32 * story_height,
                                        cell.world_center[2],
                                    ],
                                    rotation_x: 0.0,
                                    rotation_y: 0.0,
                                    collision,
                                }
                            }).collect();
                            cell.occupant = CellOccupant::Props(placements);
                        } else {
                            cell.occupant = CellOccupant::Props(vec![MeshPlacement {
                                scene: prop.scene,
                                position: cell.world_center,
                                rotation_x: 0.0,
                                rotation_y: orient.random_range(0.0..TAU),
                                collision,
                            }]);
                        }
                    }
                }
                CellKind::BoundaryEdge => {
                    if !theme.palette.wall_adjacent.is_empty()
                        && !cell.sealed_faces.is_empty()
                        && rng.random_range(0..wall_den) < wall_num
                    {
                        let prop = &theme.palette.wall_adjacent[rng.random_range(0..theme.palette.wall_adjacent.len())];
                        let face = cell.sealed_faces[rng.random_range(0..cell.sealed_faces.len())];
                        let (rot, offset_x, offset_z) = match face {
                            ConnectorFacing::NegX => (0.0, -1.0, 0.0),
                            ConnectorFacing::PosX => (PI, 1.0, 0.0),
                            ConnectorFacing::NegZ => (-FRAC_PI_2, 0.0, -1.0),
                            ConnectorFacing::PosZ => (FRAC_PI_2, 0.0, 1.0),
                            // Y-axis faces: place prop at cell center with no XZ offset
                            ConnectorFacing::NegY | ConnectorFacing::PosY => (0.0, 0.0, 0.0),
                        };
                        cell.occupant = CellOccupant::Props(vec![MeshPlacement {
                            scene: prop.scene,
                            position: [
                                cell.world_center[0] + offset_x,
                                cell.world_center[1],
                                cell.world_center[2] + offset_z,
                            ],
                            rotation_x: 0.0,
                            rotation_y: rot,
                            collision: Collision::for_prop(prop.scene),
                        }]);
                    }
                }
                CellKind::Interior => {
                    if !theme.palette.center.is_empty() && rng.random_range(0..center_den) < center_num {
                        let prop = &theme.palette.center[rng.random_range(0..theme.palette.center.len())];
                        // A center prop only rests on a surface on the ground
                        // story; upper stories have no floor under them (the room
                        // floors only at cy 0), so the prop is floating — and
                        // floating means Dynamic, never a frozen platform hanging
                        // in mid-air.
                        let collision = if cell.grid_pos[1] == 0 {
                            Collision::for_prop(prop.scene)
                        } else {
                            Collision::Dynamic
                        };
                        cell.occupant = CellOccupant::Props(vec![MeshPlacement {
                            scene: prop.scene,
                            position: cell.world_center,
                            rotation_x: 0.0,
                            rotation_y: orient.random_range(0.0..TAU),
                            collision,
                        }]);
                    }
                }
                CellKind::ConnectorGap => unreachable!(),
            }
        }
    }

    /// Collect all prop placements from occupied cells.
    pub fn prop_placements(&self) -> Vec<MeshPlacement> {
        self.cells.iter().flat_map(|c| {
            if let CellOccupant::Props(ref ps) = c.occupant {
                ps.clone()
            } else {
                vec![]
            }
        }).collect()
    }
}
