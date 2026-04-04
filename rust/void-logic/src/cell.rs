use crate::room_assembler::MeshPlacement;
use crate::room_template::{Connector, ConnectorFacing, RoomTemplate};
use crate::room_theme::RoomTheme;

/// What role a cell plays in the room's boundary structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellKind {
    /// No boundary edges — fully interior cell.
    Interior,
    /// Exactly 1 sealed boundary face, or 2 parallel sealed faces (e.g. corridor).
    BoundaryEdge,
    /// 2+ perpendicular sealed boundary faces — a geometric corner.
    BoundaryCorner,
    /// Boundary cell with an active connector — left open for passage.
    ConnectorGap,
}

/// What occupies a cell.
#[derive(Debug, Clone)]
pub enum CellOccupant {
    Empty,
    /// One or more props (e.g., a column stacked at each story level).
    Props(Vec<MeshPlacement>),
}

/// A single cell in a room's grid, classified by its structural role.
#[derive(Debug, Clone)]
pub struct Cell {
    /// Grid coordinates within the room (cx, cy, cz).
    pub grid_pos: [i32; 3],
    /// World-space center of this cell.
    pub world_center: [f32; 3],
    /// Structural role.
    pub kind: CellKind,
    /// Which boundary faces are sealed walls (no connector).
    pub sealed_faces: Vec<ConnectorFacing>,
    /// What occupies this cell (prop, enemy, or nothing).
    pub occupant: CellOccupant,
}

/// A grid of classified cells for a room template.
pub struct CellGrid {
    cells: Vec<Cell>,
    pub extents: [usize; 3],
}

impl CellGrid {
    const DEFAULT_STORY_HEIGHT: f32 = 5.0; // TODO: pass from WallSet

    /// Build a cell grid from a room template, classifying each cell by its
    /// boundary structure and active connectors.
    pub fn new(
        template: &RoomTemplate,
        active_connectors: &[Connector],
        world_origin: [f32; 3],
        cell_size: f32,
    ) -> Self {
        let ex = template.extents[0] as i32;
        let ey = template.extents[1] as i32;
        let ez = template.extents[2] as i32;

        let mut cells = Vec::with_capacity((ex * ey * ez) as usize);

        for cx in 0..ex {
            for cy in 0..ey {
                for cz in 0..ez {
                    let world_center = [
                        world_origin[0] + (cx as f32 + 0.5) * cell_size,
                        world_origin[1] + cy as f32 * Self::DEFAULT_STORY_HEIGHT,
                        world_origin[2] + (cz as f32 + 0.5) * cell_size,
                    ];

                    // Determine which boundary faces are sealed (boundary + no active connector).
                    let boundary_faces = [
                        (ConnectorFacing::NegX, cx == 0),
                        (ConnectorFacing::PosX, cx == ex - 1),
                        (ConnectorFacing::NegZ, cz == 0),
                        (ConnectorFacing::PosZ, cz == ez - 1),
                        (ConnectorFacing::NegY, cy == 0),
                        (ConnectorFacing::PosY, cy == ey - 1),
                    ];

                    let has_active_connector = boundary_faces.iter().any(|&(facing, is_boundary)| {
                        is_boundary && Self::is_active_connector(template, active_connectors, facing, cx, cy, cz)
                    });

                    let sealed_faces: Vec<ConnectorFacing> = boundary_faces
                        .iter()
                        .filter(|&&(facing, is_boundary)| {
                            is_boundary && !Self::is_active_connector(template, active_connectors, facing, cx, cy, cz)
                        })
                        .map(|&(facing, _)| facing)
                        .collect();

                    // Classify by XZ boundary structure. Y-axis faces (floor/ceiling)
                    // are structural but don't affect prop placement classification.
                    let has_xz_face = sealed_faces.iter().any(|f| matches!(f,
                        ConnectorFacing::NegX | ConnectorFacing::PosX |
                        ConnectorFacing::NegZ | ConnectorFacing::PosZ));

                    let kind = if has_active_connector {
                        CellKind::ConnectorGap
                    } else if !has_xz_face {
                        CellKind::Interior
                    } else if Self::has_perpendicular_pair(&sealed_faces) {
                        CellKind::BoundaryCorner
                    } else {
                        CellKind::BoundaryEdge
                    };

                    cells.push(Cell {
                        grid_pos: [cx, cy, cz],
                        world_center,
                        kind,
                        sealed_faces,
                        occupant: CellOccupant::Empty,
                    });
                }
            }
        }

        Self {
            cells,
            extents: [ex as usize, ey as usize, ez as usize],
        }
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

    /// Check if a set of sealed faces contains at least one perpendicular XZ pair.
    /// Y-axis faces (floor/ceiling) are universal in single-story rooms and don't
    /// define structural corners for prop placement purposes.
    fn has_perpendicular_pair(faces: &[ConnectorFacing]) -> bool {
        let has_x = faces.iter().any(|f| matches!(f, ConnectorFacing::NegX | ConnectorFacing::PosX));
        let has_z = faces.iter().any(|f| matches!(f, ConnectorFacing::NegZ | ConnectorFacing::PosZ));
        has_x && has_z
    }

    /// Iterate over all cells in the grid.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Get the cell at the given grid position, if it exists.
    pub fn cell_at(&self, cx: i32, cy: i32, cz: i32) -> Option<&Cell> {
        self.cells.iter().find(|c| {
            c.grid_pos[0] == cx && c.grid_pos[1] == cy && c.grid_pos[2] == cz
        })
    }

    /// Populate cells with props based on the room theme.
    ///
    /// Each eligible cell gets at most one occupant, chosen from the theme's
    /// palette based on the cell's kind. Density controls probability.
    /// ConnectorGap cells stay empty. Blocking props skip reserved path cells.
    pub fn populate(&mut self, theme: &RoomTheme, seed: u64) {
        use crate::asset_catalog::is_loose_prop;
        use crate::room_furnisher::RoomDensity;
        use std::f32::consts::{FRAC_PI_2, PI};

        // Density thresholds: (numerator, denominator) per category.
        let (wall_num, wall_den, center_num, center_den, corner_num, corner_den) = match theme.density {
            RoomDensity::Sparse  => (1usize, 5usize, 1, 8, 1, 6),
            RoomDensity::Normal  => (1, 2, 1, 3, 1, 3),
            RoomDensity::Dense   => (4, 5, 3, 5, 2, 3),
        };

        let mut rng = SimpleRng::new(seed);

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
                    // Skip columns near entrances (within 2 cells).
                    let near_gap = gap_positions.iter().any(|gap| {
                        let dx = (gap[0] - cell.grid_pos[0]).abs();
                        let dz = (gap[2] - cell.grid_pos[2]).abs();
                        gap[1] == cell.grid_pos[1] && dx <= 2 && dz <= 2
                    });
                    if !theme.palette.corner.is_empty() && !near_gap && rng.next_usize() % corner_den < corner_num {
                        let prop = &theme.palette.corner[rng.next_usize() % theme.palette.corner.len()];
                        let is_column = prop.scene.contains("/columns/");
                        let loose = is_loose_prop(prop.scene);
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
                                    loose,
                                }
                            }).collect();
                            cell.occupant = CellOccupant::Props(placements);
                        } else {
                            cell.occupant = CellOccupant::Props(vec![MeshPlacement {
                                scene: prop.scene,
                                position: cell.world_center,
                                rotation_x: 0.0,
                                rotation_y: 0.0,
                                loose,
                            }]);
                        }
                    }
                }
                CellKind::BoundaryEdge => {
                    if !theme.palette.wall_adjacent.is_empty()
                        && !cell.sealed_faces.is_empty()
                        && rng.next_usize() % wall_den < wall_num
                    {
                        let prop = &theme.palette.wall_adjacent[rng.next_usize() % theme.palette.wall_adjacent.len()];
                        let face = cell.sealed_faces[rng.next_usize() % cell.sealed_faces.len()];
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
                            loose: is_loose_prop(prop.scene),
                        }]);
                    }
                }
                CellKind::Interior => {
                    if !theme.palette.center.is_empty() && rng.next_usize() % center_den < center_num {
                        let prop = &theme.palette.center[rng.next_usize() % theme.palette.center.len()];
                        cell.occupant = CellOccupant::Props(vec![MeshPlacement {
                            scene: prop.scene,
                            position: cell.world_center,
                            rotation_x: 0.0,
                            rotation_y: 0.0,
                            loose: is_loose_prop(prop.scene),
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

// ── RNG ─────────────────────────────────────────────────────────────────

/// Minimal deterministic RNG (xorshift64) for prop selection.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_usize(&mut self) -> usize {
        self.next_u64() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room_template::*;

    fn small_room() -> RoomTemplate {
        RoomTemplate {

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

    fn room_3x3() -> RoomTemplate {
        RoomTemplate {
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX },
                Connector { offset: [2, 0, 1], facing: ConnectorFacing::PosX },
                Connector { offset: [1, 0, 0], facing: ConnectorFacing::NegZ },
                Connector { offset: [1, 0, 2], facing: ConnectorFacing::PosZ },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [3, 1, 3],
        }
    }

    fn corridor_ew() -> RoomTemplate {
        RoomTemplate {
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

    // --- Grid dimensions ---

    #[test]
    fn grid_dimensions_match_template_extents() {
        let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
        assert_eq!(grid.extents, [3, 1, 3], "3x1x3 template should produce 3x1x3 grid");
        assert_eq!(grid.cells().len(), 9, "3x1x3 grid should have 9 cells");
    }

    #[test]
    fn small_room_grid_has_one_cell() {
        let grid = CellGrid::new(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
        assert_eq!(grid.extents, [1, 1, 1]);
        assert_eq!(grid.cells().len(), 1);
    }

    // --- World center positions ---

    #[test]
    fn world_centers_at_cell_midpoints() {
        let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
        let cell_00 = grid.cell_at(0, 0, 0).expect("cell (0,0,0) should exist");
        assert_eq!(cell_00.world_center, [2.0, 0.0, 2.0],
            "cell (0,0,0) center should be at (2.0, 0.0, 2.0)");

        let cell_21 = grid.cell_at(2, 0, 1).expect("cell (2,0,1) should exist");
        assert_eq!(cell_21.world_center, [10.0, 0.0, 6.0],
            "cell (2,0,1) center should be at (10.0, 0.0, 6.0)");
    }

    #[test]
    fn world_centers_respect_origin_offset() {
        let grid = CellGrid::new(&small_room(), &[], [10.0, 5.0, 20.0], 4.0);
        let cell = grid.cell_at(0, 0, 0).expect("cell should exist");
        assert_eq!(cell.world_center, [12.0, 5.0, 22.0]);
    }

    // --- Cell kind classification: sealed room ---

    #[test]
    fn sealed_1x1_room_is_boundary_corner() {
        let grid = CellGrid::new(&small_room(), &[], [0.0, 0.0, 0.0], 4.0);
        let cell = grid.cell_at(0, 0, 0).expect("cell should exist");
        assert_eq!(cell.kind, CellKind::BoundaryCorner,
            "1x1 sealed room with 6 perpendicular sealed faces should be BoundaryCorner");
        assert_eq!(cell.sealed_faces.len(), 6,
            "1x1x1 sealed room should have 6 sealed faces (all directions)");
    }

    #[test]
    fn sealed_3x1x3_cell_kinds() {
        // Y-axis faces don't affect classification for prop placement.
        // 3x1x3: 4 XZ-corner cells, 4 XZ-edge cells, 1 XZ-interior cell.
        let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);

        let corners: Vec<_> = grid.cells().iter()
            .filter(|c| c.kind == CellKind::BoundaryCorner)
            .collect();
        assert_eq!(corners.len(), 4,
            "3x1x3: 4 XZ-corners = 4 BoundaryCorner");

        let edges: Vec<_> = grid.cells().iter()
            .filter(|c| c.kind == CellKind::BoundaryEdge)
            .collect();
        assert_eq!(edges.len(), 4,
            "3x1x3: 4 XZ-edge cells = 4 BoundaryEdge");

        let interiors: Vec<_> = grid.cells().iter()
            .filter(|c| c.kind == CellKind::Interior)
            .collect();
        assert_eq!(interiors.len(), 1,
            "3x1x3: center cell [1,0,1] is Interior (only Y faces)");
    }

    #[test]
    fn corner_cell_has_perpendicular_sealed_faces_including_y() {
        let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
        // Cell (0,0,0) is at NegX + NegZ corner, plus NegY + PosY (single story)
        let cell = grid.cell_at(0, 0, 0).expect("cell should exist");
        assert_eq!(cell.kind, CellKind::BoundaryCorner);
        assert!(cell.sealed_faces.contains(&ConnectorFacing::NegX));
        assert!(cell.sealed_faces.contains(&ConnectorFacing::NegZ));
        assert!(cell.sealed_faces.contains(&ConnectorFacing::NegY));
        assert!(cell.sealed_faces.contains(&ConnectorFacing::PosY));
        assert_eq!(cell.sealed_faces.len(), 4,
            "single-story XZ-corner cell should have 4 sealed faces (NegX, NegZ, NegY, PosY)");
    }

    #[test]
    fn xz_edge_cell_with_y_faces_is_edge() {
        let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
        // Cell (1,0,0) has NegZ sealed + NegY + PosY. Only one XZ axis → BoundaryEdge.
        let cell = grid.cell_at(1, 0, 0).expect("cell should exist");
        assert_eq!(cell.kind, CellKind::BoundaryEdge,
            "single-story XZ-edge cell should be BoundaryEdge, not corner");
        assert!(cell.sealed_faces.contains(&ConnectorFacing::NegZ));
        assert!(cell.sealed_faces.contains(&ConnectorFacing::NegY));
        assert!(cell.sealed_faces.contains(&ConnectorFacing::PosY));
    }

    #[test]
    fn xz_interior_cell_single_story_is_interior() {
        let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
        // Cell (1,0,1) has no XZ boundary faces, only NegY + PosY → Interior.
        let cell = grid.cell_at(1, 0, 1).expect("cell should exist");
        assert_eq!(cell.kind, CellKind::Interior,
            "single-story center cell with only Y faces should be Interior");
        assert_eq!(cell.sealed_faces.len(), 2);
    }

    #[test]
    fn multi_story_interior_cell_is_truly_interior() {
        // In a 3x2x3 room, cell (1,_,1) at cy not on boundary has no XZ or Y faces
        // But we need at least 3 stories for a truly interior cell.
        let tall_room = RoomTemplate {
            kind: TemplateKind::Room,
            connectors: vec![],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [3, 3, 3],
        };
        let grid = CellGrid::new(&tall_room, &[], [0.0, 0.0, 0.0], 4.0);
        let cell = grid.cell_at(1, 1, 1).expect("cell (1,1,1) should exist");
        assert_eq!(cell.kind, CellKind::Interior,
            "center cell in 3x3x3 room should be truly Interior");
        assert!(cell.sealed_faces.is_empty());
    }

    // --- Active connectors ---

    #[test]
    fn active_connector_cell_is_connector_gap() {
        // 3x3 room with NegX active at [0,0,1]
        let grid = CellGrid::new(
            &room_3x3(),
            &[Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX }],
            [0.0, 0.0, 0.0],
            4.0,
        );
        let cell = grid.cell_at(0, 0, 1).expect("cell should exist");
        assert_eq!(cell.kind, CellKind::ConnectorGap,
            "cell at active NegX connector should be ConnectorGap");
    }

    #[test]
    fn corridor_with_both_ends_active_is_connector_gap() {
        let grid = CellGrid::new(
            &corridor_ew(),
            &[
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
            ],
            [0.0, 0.0, 0.0],
            4.0,
        );
        let cell = grid.cell_at(0, 0, 0).expect("cell should exist");
        // Both NegX and PosX are active connectors. NegZ and PosZ are sealed.
        // But since it has active connectors, it's a ConnectorGap.
        assert_eq!(cell.kind, CellKind::ConnectorGap,
            "corridor cell with active connectors should be ConnectorGap");
    }

    #[test]
    fn non_connector_cells_unaffected_by_active_facings() {
        // 3x3 with NegX active — cell (2,0,2) is PosX+PosZ corner, unaffected
        let grid = CellGrid::new(
            &room_3x3(),
            &[Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX }],
            [0.0, 0.0, 0.0],
            4.0,
        );
        let cell = grid.cell_at(2, 0, 2).expect("cell should exist");
        assert_eq!(cell.kind, CellKind::BoundaryCorner,
            "cell (2,0,2) far from connector should still be BoundaryCorner");
    }

    // --- Y-axis boundary detection (true 3D) ---

    fn multi_story_room() -> RoomTemplate {
        RoomTemplate {
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX },
                Connector { offset: [2, 0, 1], facing: ConnectorFacing::PosX },
                Connector { offset: [1, 0, 0], facing: ConnectorFacing::NegZ },
                Connector { offset: [1, 0, 2], facing: ConnectorFacing::PosZ },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [3, 2, 3],
        }
    }

    #[test]
    fn multi_story_room_has_correct_cell_count() {
        let grid = CellGrid::new(&multi_story_room(), &[], [0.0, 0.0, 0.0], 4.0);
        assert_eq!(grid.extents, [3, 2, 3]);
        assert_eq!(grid.cells().len(), 18, "3x2x3 grid should have 18 cells");
    }

    #[test]
    fn bottom_cell_has_negy_sealed_face() {
        let grid = CellGrid::new(&multi_story_room(), &[], [0.0, 0.0, 0.0], 4.0);
        let cell = grid.cell_at(1, 0, 1).expect("cell (1,0,1) should exist");
        assert!(cell.sealed_faces.contains(&ConnectorFacing::NegY),
            "interior cell at cy==0 should have NegY sealed face, got {:?}", cell.sealed_faces);
    }

    #[test]
    fn top_cell_has_posy_sealed_face() {
        let grid = CellGrid::new(&multi_story_room(), &[], [0.0, 0.0, 0.0], 4.0);
        let cell = grid.cell_at(1, 1, 1).expect("cell (1,1,1) should exist");
        assert!(cell.sealed_faces.contains(&ConnectorFacing::PosY),
            "interior cell at cy==ey-1 should have PosY sealed face, got {:?}", cell.sealed_faces);
    }

    #[test]
    fn mid_cell_has_no_y_sealed_faces() {
        // In a 3-story room, the middle floor should have no Y sealed faces
        let tall_room = RoomTemplate {
            kind: TemplateKind::Room,
            connectors: vec![],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 3, 1],
        };
        let grid = CellGrid::new(&tall_room, &[], [0.0, 0.0, 0.0], 4.0);
        let cell = grid.cell_at(0, 1, 0).expect("cell (0,1,0) should exist");
        assert!(!cell.sealed_faces.contains(&ConnectorFacing::NegY),
            "mid-floor cell should not have NegY sealed");
        assert!(!cell.sealed_faces.contains(&ConnectorFacing::PosY),
            "mid-floor cell should not have PosY sealed");
    }

    #[test]
    fn posy_active_connector_prevents_sealed_face() {
        let room_with_vertical_connector = RoomTemplate {
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 1, 0], facing: ConnectorFacing::PosY },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 2, 1],
        };
        let grid = CellGrid::new(
            &room_with_vertical_connector,
            &[Connector { offset: [0, 1, 0], facing: ConnectorFacing::PosY }],
            [0.0, 0.0, 0.0],
            4.0,
        );
        let cell = grid.cell_at(0, 1, 0).expect("cell (0,1,0) should exist");
        assert!(!cell.sealed_faces.contains(&ConnectorFacing::PosY),
            "active PosY connector should prevent PosY sealed face");
        assert_eq!(cell.kind, CellKind::ConnectorGap,
            "cell with active vertical connector should be ConnectorGap");
    }

    // --- Aperture alignment: unwired Y-level must be sealed ---

    #[test]
    fn tall_room_unwired_y1_connector_is_not_gap() {
        // A 3x2x3 room has NegX connectors at y=0 and y=1.
        // If only y=0 is wired, cell [0,1,1] should be sealed, not ConnectorGap.
        let template = RoomTemplate {
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX },
                Connector { offset: [0, 1, 1], facing: ConnectorFacing::NegX },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [3, 2, 3],
        };
        // Only the y=0 connector is active.
        let active = &[Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX }];
        let grid = CellGrid::new(&template, active, [0.0, 0.0, 0.0], 4.0);

        // y=0 cell should be ConnectorGap (it IS wired)
        let y0 = grid.cell_at(0, 0, 1).expect("cell (0,0,1)");
        assert_eq!(y0.kind, CellKind::ConnectorGap,
            "y=0 NegX connector cell should be ConnectorGap");

        // y=1 cell should NOT be ConnectorGap (it is NOT wired)
        let y1 = grid.cell_at(0, 1, 1).expect("cell (0,1,1)");
        assert_ne!(y1.kind, CellKind::ConnectorGap,
            "y=1 NegX connector cell should be sealed — only y=0 is wired");
    }

    // --- Populate tests ---

    #[test]
    fn dense_populate_fills_majority_of_cells() {
        use crate::room_theme::THEME_WAREHOUSE;
        // THEME_WAREHOUSE has Dense density.
        let mut grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
        grid.populate(&THEME_WAREHOUSE, 42);
        let occupied = grid.cells().iter()
            .filter(|c| matches!(c.occupant, CellOccupant::Props(_)))
            .count();
        // Dense = ~60-80% fill. 9 cells → at least 5 occupied.
        assert!(occupied >= 5,
            "dense populate should fill ≥5 of 9 cells, got {occupied}");
    }

    #[test]
    fn sparse_populate_leaves_most_cells_empty() {
        use crate::room_theme::{RoomTheme, PALETTE_GENERIC};
        use crate::room_furnisher::RoomDensity;
        let sparse_theme = RoomTheme {
            name: "test_sparse",
            wall_set: &crate::asset_catalog::WALL_SET_ASTRA,
            palette: &PALETTE_GENERIC,
            density: RoomDensity::Sparse,
        };
        let mut grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
        grid.populate(&sparse_theme, 42);
        let occupied = grid.cells().iter()
            .filter(|c| matches!(c.occupant, CellOccupant::Props(_)))
            .count();
        // Sparse = ~12-20%. 9 cells → at most 4 occupied.
        assert!(occupied <= 4,
            "sparse populate should fill ≤4 of 9 cells, got {occupied}");
    }

    #[test]
    fn connector_gap_cells_never_occupied() {
        use crate::room_theme::THEME_WAREHOUSE;
        let mut grid = CellGrid::new(
            &room_3x3(),
            &[Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX }],
            [0.0, 0.0, 0.0],
            4.0,
        );
        grid.populate(&THEME_WAREHOUSE, 42);
        let gap_occupied = grid.cells().iter()
            .filter(|c| c.kind == CellKind::ConnectorGap)
            .any(|c| matches!(c.occupant, CellOccupant::Props(_)));
        assert!(!gap_occupied, "ConnectorGap cells should never have props");
    }

    #[test]
    fn no_cell_has_more_than_one_occupant() {
        use crate::room_theme::THEME_WAREHOUSE;
        let mut grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
        grid.populate(&THEME_WAREHOUSE, 42);
        // Each cell is either Empty or Prop — by type system this is guaranteed,
        // but verify prop_placements count matches occupied cell count.
        let occupied = grid.cells().iter()
            .filter(|c| matches!(c.occupant, CellOccupant::Props(_)))
            .count();
        assert_eq!(grid.prop_placements().len(), occupied,
            "prop_placements count should match occupied cell count");
    }

    #[test]
    fn populate_uses_themed_props() {
        use crate::room_theme::THEME_WAREHOUSE;
        use crate::asset_catalog;
        let mut grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
        grid.populate(&THEME_WAREHOUSE, 42);
        let placements = grid.prop_placements();
        // All placed props should come from the warehouse palette.
        let warehouse_scenes: Vec<&str> = asset_catalog::WAREHOUSE_WALL_PROPS.iter()
            .chain(asset_catalog::WAREHOUSE_CENTER_PROPS)
            .chain(asset_catalog::CORNER_PROPS)
            .chain(asset_catalog::CEILING_PROPS)
            .map(|p| p.scene)
            .collect();
        for p in &placements {
            assert!(warehouse_scenes.contains(&p.scene),
                "prop '{}' not in warehouse palette", p.scene);
        }
    }

    /// Column props in multi-story rooms must be stacked at each story level.
    #[test]
    fn columns_stacked_in_multi_story_room() {
        use crate::room_theme::THEME_WAREHOUSE;
        let template = RoomTemplate {
            kind: TemplateKind::Room,
            connectors: vec![],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [3, 2, 3],
        };
        // Try many seeds until we get at least one column placement.
        let mut found_stacked = false;
        for seed in 0..200 {
            let mut grid = CellGrid::new(&template, &[], [0.0, 0.0, 0.0], 4.0);
            grid.populate(&THEME_WAREHOUSE, seed);
            let placements = grid.prop_placements();
            let columns: Vec<_> = placements.iter()
                .filter(|p| p.scene.contains("/columns/"))
                .collect();
            if columns.is_empty() {
                continue;
            }
            // For each column, there should be a matching column at the next story level.
            let story_height = CellGrid::DEFAULT_STORY_HEIGHT;
            for col in &columns {
                let has_pair = columns.iter().any(|other| {
                    other.scene == col.scene
                        && (other.position[0] - col.position[0]).abs() < 0.01
                        && (other.position[2] - col.position[2]).abs() < 0.01
                        && other.position[1] != col.position[1]
                        && ((other.position[1] - col.position[1]).abs() - story_height).abs() < 0.01
                });
                assert!(
                    has_pair,
                    "column at {:?} should have a matching column one story above/below (story_height={})",
                    col.position, story_height
                );
            }
            found_stacked = true;
            break;
        }
        assert!(found_stacked, "should find at least one stacked column across 200 seeds");
    }

    #[test]
    fn single_story_5x5_has_interior_cells() {
        let room = RoomTemplate {
            kind: TemplateKind::Room,
            connectors: vec![],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [5, 1, 5],
        };
        let grid = CellGrid::new(&room, &[], [0.0, 0.0, 0.0], 4.0);
        let interior_count = grid.cells().iter()
            .filter(|c| c.kind == CellKind::Interior)
            .count();
        // 5x5 room: the 3x3 inner cells should be Interior (9 cells)
        assert!(interior_count >= 1,
            "single-story 5x5 should have ≥1 Interior cell, got {interior_count}");
    }

    #[test]
    fn center_props_placed_in_single_story_room() {
        use crate::room_theme::THEME_GENERIC;
        let room = RoomTemplate {
            kind: TemplateKind::Room,
            connectors: vec![],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [5, 1, 5],
        };
        let center_scenes: std::collections::HashSet<&str> = crate::asset_catalog::CENTER_PROPS
            .iter().map(|p| p.scene).collect();
        let mut found_center = false;
        for seed in 0..100 {
            let mut grid = CellGrid::new(&room, &[], [0.0, 0.0, 0.0], 4.0);
            grid.populate(&THEME_GENERIC, seed);
            if grid.prop_placements().iter().any(|p| center_scenes.contains(p.scene)) {
                found_center = true;
                break;
            }
        }
        assert!(found_center, "center props should appear in single-story rooms across 100 seeds");
    }

    #[test]
    fn no_column_near_connector_gap_in_generated_rooms() {
        // Test with procedurally generated rooms — the actual production path.
        use crate::generator::{generate_room, GeneratorConfig};
        use crate::room_theme::THEME_WAREHOUSE;
        use rand::SeedableRng;
        use rand::rngs::SmallRng;

        let config = GeneratorConfig {
            seed: 42,
            max_rooms: 10,
            min_room_xz: 3,
            max_room_xz: 6,
            min_room_y: 1,
            max_room_y: 3,
        };

        for room_seed in 0..50 {
            let mut rng = SmallRng::seed_from_u64(room_seed);
            let room = generate_room(&mut rng, &config);

            // Activate a random subset of connectors (simulating connected rooms)
            let active: Vec<_> = room.connectors.iter()
                .enumerate()
                .filter(|(i, _)| i % 2 == 0)
                .map(|(_, c)| *c)
                .collect();

            for populate_seed in 0..20 {
                let mut grid = CellGrid::new(&room, &active, [0.0, 0.0, 0.0], 4.0);
                grid.populate(&THEME_WAREHOUSE, populate_seed);

                let gap_positions: std::collections::HashSet<[i32; 3]> = grid.cells().iter()
                    .filter(|c| c.kind == CellKind::ConnectorGap)
                    .map(|c| c.grid_pos)
                    .collect();

                for cell in grid.cells() {
                    if let CellOccupant::Props(ref props) = cell.occupant {
                        for p in props {
                            if p.scene.contains("/columns/") {
                                let near = gap_positions.iter().any(|gap| {
                                    let dx = (gap[0] - cell.grid_pos[0]).abs();
                                    let dz = (gap[2] - cell.grid_pos[2]).abs();
                                    gap[1] == cell.grid_pos[1] && dx <= 2 && dz <= 2
                                });
                                assert!(
                                    !near,
                                    "room_seed {room_seed}, pop_seed {populate_seed}: \
                                     column at {:?} within 2 cells of ConnectorGap",
                                    cell.grid_pos
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn no_column_adjacent_to_connector_gap() {
        // 3x1x3 room with active NegX connector at [0,0,1].
        // Cell [0,0,0] is a BoundaryCorner adjacent to the gap.
        // No column should be placed there regardless of seed.
        use crate::room_theme::THEME_WAREHOUSE;
        let active = vec![Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX }];
        for seed in 0..200 {
            let mut grid = CellGrid::new(&room_3x3(), &active, [0.0, 0.0, 0.0], 4.0);
            grid.populate(&THEME_WAREHOUSE, seed);
            for cell in grid.cells() {
                if let CellOccupant::Props(ref props) = cell.occupant {
                    for p in props {
                        if p.scene.contains("/columns/") {
                            // Check if this cell is XZ-adjacent to a ConnectorGap
                            let [cx, _, cz] = cell.grid_pos;
                            let adjacent_to_gap = grid.cells().iter().any(|other| {
                                other.kind == CellKind::ConnectorGap
                                    && ((other.grid_pos[0] - cx).abs() + (other.grid_pos[2] - cz).abs() == 1)
                                    && other.grid_pos[1] == cell.grid_pos[1]
                            });
                            assert!(
                                !adjacent_to_gap,
                                "seed {seed}: column at cell {:?} is adjacent to ConnectorGap",
                                cell.grid_pos
                            );
                        }
                    }
                }
            }
        }
    }
}
