use crate::room_assembler::MeshPlacement;
use crate::room_template::{ConnectorFacing, RoomTemplate};
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

/// What occupies a cell (at most one item).
#[derive(Debug, Clone)]
pub enum CellOccupant {
    Empty,
    Prop(MeshPlacement),
    // Enemy(EnemySpawn),  // future
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
    /// Vertical cell height in meters (matches room_assembler::CELL_HEIGHT).
    const CELL_HEIGHT: f32 = 5.0;

    /// Build a cell grid from a room template, classifying each cell by its
    /// boundary structure and active connectors.
    pub fn new(
        template: &RoomTemplate,
        active_facings: &[ConnectorFacing],
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
                        world_origin[1] + cy as f32 * Self::CELL_HEIGHT,
                        world_origin[2] + (cz as f32 + 0.5) * cell_size,
                    ];

                    // Determine which boundary faces are sealed (boundary + no active connector).
                    let boundary_faces = [
                        (ConnectorFacing::NegX, cx == 0),
                        (ConnectorFacing::PosX, cx == ex - 1),
                        (ConnectorFacing::NegZ, cz == 0),
                        (ConnectorFacing::PosZ, cz == ez - 1),
                    ];

                    let has_active_connector = boundary_faces.iter().any(|&(facing, is_boundary)| {
                        is_boundary && Self::is_active_connector(template, active_facings, facing, cx, cy, cz)
                    });

                    let sealed_faces: Vec<ConnectorFacing> = boundary_faces
                        .iter()
                        .filter(|&&(facing, is_boundary)| {
                            is_boundary && !Self::is_active_connector(template, active_facings, facing, cx, cy, cz)
                        })
                        .map(|&(facing, _)| facing)
                        .collect();

                    let kind = if has_active_connector {
                        CellKind::ConnectorGap
                    } else if sealed_faces.is_empty() {
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

    /// Check if a set of sealed faces contains at least one perpendicular pair.
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
        use crate::room_furnisher::RoomDensity;
        use std::f32::consts::{FRAC_PI_2, PI};

        // Density thresholds: (numerator, denominator) per category.
        let (wall_num, wall_den, center_num, center_den, corner_num, corner_den) = match theme.density {
            RoomDensity::Sparse  => (1usize, 5usize, 1, 8, 1, 6),
            RoomDensity::Normal  => (1, 2, 1, 3, 1, 3),
            RoomDensity::Dense   => (4, 5, 3, 5, 2, 3),
        };

        let mut rng = SimpleRng::new(seed);

        for cell in &mut self.cells {
            // Skip connector gaps — must stay clear for passage.
            if cell.kind == CellKind::ConnectorGap {
                continue;
            }

            match cell.kind {
                CellKind::BoundaryCorner => {
                    if !theme.palette.corner.is_empty() && rng.next_usize() % corner_den < corner_num {
                        let prop = &theme.palette.corner[rng.next_usize() % theme.palette.corner.len()];
                        cell.occupant = CellOccupant::Prop(MeshPlacement {
                            scene: prop.scene,
                            position: cell.world_center,
                            rotation_x: 0.0,
                            rotation_y: 0.0,
                        });
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
                            _ => (0.0, 0.0, 0.0),
                        };
                        cell.occupant = CellOccupant::Prop(MeshPlacement {
                            scene: prop.scene,
                            position: [
                                cell.world_center[0] + offset_x,
                                cell.world_center[1],
                                cell.world_center[2] + offset_z,
                            ],
                            rotation_x: 0.0,
                            rotation_y: rot,
                        });
                    }
                }
                CellKind::Interior => {
                    if !theme.palette.center.is_empty() && rng.next_usize() % center_den < center_num {
                        let prop = &theme.palette.center[rng.next_usize() % theme.palette.center.len()];
                        cell.occupant = CellOccupant::Prop(MeshPlacement {
                            scene: prop.scene,
                            position: cell.world_center,
                            rotation_x: 0.0,
                            rotation_y: 0.0,
                        });
                    }
                }
                CellKind::ConnectorGap => unreachable!(),
            }
        }
    }

    /// Collect all prop placements from occupied cells.
    pub fn prop_placements(&self) -> Vec<MeshPlacement> {
        self.cells.iter().filter_map(|c| {
            if let CellOccupant::Prop(ref p) = c.occupant {
                Some(*p)
            } else {
                None
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

    fn room_3x3() -> RoomTemplate {
        RoomTemplate {
            id: "test_3x3",
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
            "1x1 sealed room with 4 perpendicular sealed faces should be BoundaryCorner");
        assert_eq!(cell.sealed_faces.len(), 4,
            "1x1 sealed room should have 4 sealed faces");
    }

    #[test]
    fn sealed_3x3_has_4_corners_4_edges_1_interior() {
        let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);

        let corners: Vec<_> = grid.cells().iter()
            .filter(|c| c.kind == CellKind::BoundaryCorner)
            .collect();
        assert_eq!(corners.len(), 4, "sealed 3x3 should have 4 BoundaryCorner cells");

        let edges: Vec<_> = grid.cells().iter()
            .filter(|c| c.kind == CellKind::BoundaryEdge)
            .collect();
        assert_eq!(edges.len(), 4, "sealed 3x3 should have 4 BoundaryEdge cells");

        let interiors: Vec<_> = grid.cells().iter()
            .filter(|c| c.kind == CellKind::Interior)
            .collect();
        assert_eq!(interiors.len(), 1, "sealed 3x3 should have 1 Interior cell");
    }

    #[test]
    fn corner_cell_has_two_perpendicular_sealed_faces() {
        let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
        // Cell (0,0,0) is at NegX + NegZ corner
        let cell = grid.cell_at(0, 0, 0).expect("cell should exist");
        assert_eq!(cell.kind, CellKind::BoundaryCorner);
        assert!(cell.sealed_faces.contains(&ConnectorFacing::NegX),
            "cell (0,0,0) should have NegX sealed face");
        assert!(cell.sealed_faces.contains(&ConnectorFacing::NegZ),
            "cell (0,0,0) should have NegZ sealed face");
        assert_eq!(cell.sealed_faces.len(), 2);
    }

    #[test]
    fn edge_cell_has_one_sealed_face() {
        let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
        // Cell (1,0,0) is on NegZ edge only (not a corner)
        let cell = grid.cell_at(1, 0, 0).expect("cell should exist");
        assert_eq!(cell.kind, CellKind::BoundaryEdge);
        assert_eq!(cell.sealed_faces, vec![ConnectorFacing::NegZ]);
    }

    #[test]
    fn interior_cell_has_no_sealed_faces() {
        let grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
        // Cell (1,0,1) is fully interior
        let cell = grid.cell_at(1, 0, 1).expect("cell should exist");
        assert_eq!(cell.kind, CellKind::Interior);
        assert!(cell.sealed_faces.is_empty());
    }

    // --- Active connectors ---

    #[test]
    fn active_connector_cell_is_connector_gap() {
        // 3x3 room with NegX active at [0,0,1]
        let grid = CellGrid::new(
            &room_3x3(),
            &[ConnectorFacing::NegX],
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
            &[ConnectorFacing::NegX, ConnectorFacing::PosX],
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
            &[ConnectorFacing::NegX],
            [0.0, 0.0, 0.0],
            4.0,
        );
        let cell = grid.cell_at(2, 0, 2).expect("cell should exist");
        assert_eq!(cell.kind, CellKind::BoundaryCorner,
            "cell (2,0,2) far from connector should still be BoundaryCorner");
    }

    // --- Populate tests ---

    #[test]
    fn dense_populate_fills_majority_of_cells() {
        use crate::room_theme::THEME_WAREHOUSE;
        // THEME_WAREHOUSE has Dense density.
        let mut grid = CellGrid::new(&room_3x3(), &[], [0.0, 0.0, 0.0], 4.0);
        grid.populate(&THEME_WAREHOUSE, 42);
        let occupied = grid.cells().iter()
            .filter(|c| matches!(c.occupant, CellOccupant::Prop(_)))
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
            .filter(|c| matches!(c.occupant, CellOccupant::Prop(_)))
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
            &[ConnectorFacing::NegX],
            [0.0, 0.0, 0.0],
            4.0,
        );
        grid.populate(&THEME_WAREHOUSE, 42);
        let gap_occupied = grid.cells().iter()
            .filter(|c| c.kind == CellKind::ConnectorGap)
            .any(|c| matches!(c.occupant, CellOccupant::Prop(_)));
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
            .filter(|c| matches!(c.occupant, CellOccupant::Prop(_)))
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
}
