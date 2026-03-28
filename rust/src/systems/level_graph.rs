use std::collections::{HashMap, HashSet, VecDeque};
use crate::systems::room_template::{RoomTemplate, ConnectorFacing};

/// A placed room in the level, with its position on the grid.
#[derive(Debug, Clone)]
pub struct PlacedRoom {
    pub template: RoomTemplate,
    /// Position of the room's origin on the grid.
    pub grid_pos: [i32; 3],
    pub room_index: usize,
}

/// Edge between two rooms (bidirectional).
#[derive(Debug, Clone)]
pub struct Corridor {
    pub from: usize,
    pub to: usize,
    /// Which connector facings were used to make this connection.
    pub from_facing: ConnectorFacing,
    pub to_facing: ConnectorFacing,
}

#[derive(Debug)]
pub enum PlaceError {
    Overlap { cell: [i32; 3], existing_room: usize },
}

#[derive(Debug)]
pub enum ConnectError {
    InvalidRoomIndex(usize),
    NoMatchingConnectors,
}

/// The full level layout as a graph of rooms connected by corridors.
/// Generated in pure Rust, then handed to LevelManager (Godot node)
/// to instantiate the actual geometry.
#[derive(Debug)]
pub struct LevelGraph {
    pub rooms: Vec<PlacedRoom>,
    pub corridors: Vec<Corridor>,
    /// Tracks which grid cells are occupied to prevent overlap.
    occupied: HashMap<[i32; 3], usize>,
}

impl LevelGraph {
    pub fn new() -> Self {
        Self {
            rooms: Vec::new(),
            corridors: Vec::new(),
            occupied: HashMap::new(),
        }
    }

    /// Check whether a grid cell is free.
    pub fn is_free(&self, pos: [i32; 3]) -> bool {
        !self.occupied.contains_key(&pos)
    }

    /// Place a room at a grid position. Returns the room index.
    /// Fails if any cell the room would occupy is already taken.
    pub fn place_room(&mut self, template: RoomTemplate, grid_pos: [i32; 3]) -> Result<usize, PlaceError> {
        let cells = cells_for(&template, grid_pos);
        for cell in &cells {
            if let Some(&existing) = self.occupied.get(cell) {
                return Err(PlaceError::Overlap { cell: *cell, existing_room: existing });
            }
        }

        let index = self.rooms.len();
        for cell in cells {
            self.occupied.insert(cell, index);
        }

        self.rooms.push(PlacedRoom {
            template,
            grid_pos,
            room_index: index,
        });

        Ok(index)
    }

    /// Connect two rooms through matching connectors.
    /// Validates that both indices exist and that the rooms have
    /// compatible connectors facing each other.
    pub fn connect(&mut self, from: usize, to: usize) -> Result<(), ConnectError> {
        if from >= self.rooms.len() {
            return Err(ConnectError::InvalidRoomIndex(from));
        }
        if to >= self.rooms.len() {
            return Err(ConnectError::InvalidRoomIndex(to));
        }

        // Find a pair of connectors that mate:
        // from's connector target_cell should land in to's grid space,
        // and to should have a connector with the opposite facing.
        let from_room = &self.rooms[from];
        let to_room = &self.rooms[to];

        for fc in &from_room.template.connectors {
            let target = fc.target_cell(from_room.grid_pos);
            // Check if target falls within to_room's grid cells
            let to_cells = cells_for(&to_room.template, to_room.grid_pos);
            if !to_cells.contains(&target) {
                continue;
            }

            for tc in &to_room.template.connectors {
                if fc.mates_with(tc) {
                    let tc_target = tc.target_cell(to_room.grid_pos);
                    let from_cells = cells_for(&from_room.template, from_room.grid_pos);
                    if from_cells.contains(&tc_target) {
                        self.corridors.push(Corridor {
                            from,
                            to,
                            from_facing: fc.facing,
                            to_facing: tc.facing,
                        });
                        return Ok(());
                    }
                }
            }
        }

        Err(ConnectError::NoMatchingConnectors)
    }

    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    /// Check whether every room is reachable from room 0 via corridors.
    pub fn is_fully_connected(&self) -> bool {
        if self.rooms.is_empty() {
            return true;
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(0usize);
        visited.insert(0usize);

        while let Some(current) = queue.pop_front() {
            for corridor in &self.corridors {
                let neighbor = if corridor.from == current {
                    Some(corridor.to)
                } else if corridor.to == current {
                    Some(corridor.from)
                } else {
                    None
                };

                if let Some(n) = neighbor {
                    if visited.insert(n) {
                        queue.push_back(n);
                    }
                }
            }
        }

        visited.len() == self.rooms.len()
    }
}

/// Enumerate all grid cells a room occupies given its origin.
fn cells_for(template: &RoomTemplate, origin: [i32; 3]) -> Vec<[i32; 3]> {
    let mut cells = Vec::new();
    for x in 0..template.extents[0] as i32 {
        for y in 0..template.extents[1] as i32 {
            for z in 0..template.extents[2] as i32 {
                cells.push([origin[0] + x, origin[1] + y, origin[2] + z]);
            }
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::room_template::*;

    // --- Test fixtures ---

    fn room_1x1_east_west() -> RoomTemplate {
        RoomTemplate {
            id: "1x1_ew",
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 1, 1],
        }
    }

    fn room_2x1_east_west() -> RoomTemplate {
        RoomTemplate {
            id: "2x1_ew",
            connectors: vec![
                // West entrance at [0,0,0]
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
                // East exit at far end [1,0,0]
                Connector { offset: [1, 0, 0], facing: ConnectorFacing::PosX },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [2, 1, 1],
        }
    }

    fn room_1x1_north_south() -> RoomTemplate {
        RoomTemplate {
            id: "1x1_ns",
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 1, 1],
        }
    }

    // --- Placement tests ---

    #[test]
    fn place_single_room() {
        let mut graph = LevelGraph::new();
        let idx = graph.place_room(room_1x1_east_west(), [0, 0, 0]);
        assert!(idx.is_ok());
        assert_eq!(graph.room_count(), 1);
    }

    #[test]
    fn overlap_is_rejected() {
        let mut graph = LevelGraph::new();
        graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let result = graph.place_room(room_1x1_east_west(), [0, 0, 0]);
        assert!(result.is_err());
    }

    #[test]
    fn multi_cell_room_occupies_all_cells() {
        let mut graph = LevelGraph::new();
        graph.place_room(room_2x1_east_west(), [0, 0, 0]).unwrap();
        // Both [0,0,0] and [1,0,0] should be occupied
        assert!(!graph.is_free([0, 0, 0]));
        assert!(!graph.is_free([1, 0, 0]));
        // Adjacent cell should still be free
        assert!(graph.is_free([2, 0, 0]));
    }

    #[test]
    fn partial_overlap_with_multi_cell_is_rejected() {
        let mut graph = LevelGraph::new();
        graph.place_room(room_2x1_east_west(), [0, 0, 0]).unwrap();
        // Try to place a 1x1 at [1,0,0] — overlaps second cell of the 2x1
        let result = graph.place_room(room_1x1_east_west(), [1, 0, 0]);
        assert!(result.is_err());
    }

    #[test]
    fn negative_coordinates_work() {
        let mut graph = LevelGraph::new();
        let idx = graph.place_room(room_1x1_east_west(), [-5, -3, -1]);
        assert!(idx.is_ok());
        assert!(!graph.is_free([-5, -3, -1]));
    }

    // --- Connection tests ---

    #[test]
    fn connect_adjacent_rooms_with_matching_connectors() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [1, 0, 0]).unwrap();
        // a has PosX connector, b has NegX connector — they should mate
        let result = graph.connect(a, b);
        assert!(result.is_ok());
        assert_eq!(graph.corridors.len(), 1);
    }

    #[test]
    fn connect_fails_with_invalid_index() {
        let mut graph = LevelGraph::new();
        graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let result = graph.connect(0, 99);
        assert!(matches!(result, Err(ConnectError::InvalidRoomIndex(99))));
    }

    #[test]
    fn connect_fails_when_no_matching_connectors() {
        let mut graph = LevelGraph::new();
        // Place two rooms with east/west connectors but along the Z axis
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [0, 0, 1]).unwrap();
        // They're adjacent on Z, but only have X-facing connectors
        let result = graph.connect(a, b);
        assert!(matches!(result, Err(ConnectError::NoMatchingConnectors)));
    }

    #[test]
    fn connect_non_adjacent_rooms_fails() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [5, 0, 0]).unwrap();
        let result = graph.connect(a, b);
        assert!(matches!(result, Err(ConnectError::NoMatchingConnectors)));
    }

    #[test]
    fn connect_multi_cell_rooms_through_far_connector() {
        let mut graph = LevelGraph::new();
        // 2x1 room at [0,0,0] — PosX connector at offset [1,0,0]
        let a = graph.place_room(room_2x1_east_west(), [0, 0, 0]).unwrap();
        // 1x1 room at [2,0,0] — NegX connector
        let b = graph.place_room(room_1x1_east_west(), [2, 0, 0]).unwrap();
        let result = graph.connect(a, b);
        assert!(result.is_ok());
    }

    // --- Connectivity tests ---

    #[test]
    fn empty_graph_is_connected() {
        let graph = LevelGraph::new();
        assert!(graph.is_fully_connected());
    }

    #[test]
    fn single_room_is_connected() {
        let mut graph = LevelGraph::new();
        graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        assert!(graph.is_fully_connected());
    }

    #[test]
    fn two_connected_rooms_are_fully_connected() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [1, 0, 0]).unwrap();
        graph.connect(a, b).unwrap();
        assert!(graph.is_fully_connected());
    }

    #[test]
    fn disconnected_rooms_are_not_fully_connected() {
        let mut graph = LevelGraph::new();
        graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        graph.place_room(room_1x1_east_west(), [5, 0, 0]).unwrap();
        // No connection made
        assert!(!graph.is_fully_connected());
    }

    #[test]
    fn chain_of_three_rooms_is_connected() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [1, 0, 0]).unwrap();
        let c = graph.place_room(room_1x1_east_west(), [2, 0, 0]).unwrap();
        graph.connect(a, b).unwrap();
        graph.connect(b, c).unwrap();
        assert!(graph.is_fully_connected());
        // a-c not directly connected, but reachable through b
    }

    #[test]
    fn mixed_axes_connection() {
        let mut graph = LevelGraph::new();
        // Room 0 at origin with N/S connectors
        let a = graph.place_room(room_1x1_north_south(), [0, 0, 0]).unwrap();
        // Room 1 at [0,0,1] with N/S connectors
        let b = graph.place_room(room_1x1_north_south(), [0, 0, 1]).unwrap();
        let result = graph.connect(a, b);
        assert!(result.is_ok());
    }
}
