use crate::room_assembler::MeshPlacement;
use crate::room_template::{Connector, ConnectorFacing, RoomTemplate};

mod populate;

#[cfg(test)]
mod tests;

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
    pub(crate) const DEFAULT_STORY_HEIGHT: f32 = 5.0; // TODO: pass from WallSet

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
                        is_boundary && crate::room_assembler::is_active_connector(template, active_connectors, facing, cx, cy, cz)
                    });

                    let sealed_faces: Vec<ConnectorFacing> = boundary_faces
                        .iter()
                        .filter(|&&(facing, is_boundary)| {
                            is_boundary && !crate::room_assembler::is_active_connector(template, active_connectors, facing, cx, cy, cz)
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
}
