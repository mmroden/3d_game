use std::collections::HashMap;
use petgraph::graph::UnGraph;
use petgraph::algo::connected_components;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use crate::room_template::{Connector, RoomTemplate, ConnectorFacing};

/// A placed room in the level, with its position on the grid.
/// "Room" here means any flyable space: arena, corridor segment,
/// hub, vertical shaft — they're all nodes.
#[derive(Debug, Clone)]
pub struct PlacedRoom {
    pub template: RoomTemplate,
    /// Position of the room's origin on the grid.
    pub grid_pos: [i32; 3],
}

impl PlacedRoom {
    /// Convert grid position to world-space origin.
    /// XZ uses `tile_width`, Y uses `story_height` — both derived from the wall set.
    pub fn world_position(&self, tile_width: f32, story_height: f32) -> [f32; 3] {
        [
            self.grid_pos[0] as f32 * tile_width,
            self.grid_pos[1] as f32 * story_height,
            self.grid_pos[2] as f32 * tile_width,
        ]
    }
}

/// Identifies which key opens a locked door.
/// Exhaustive — adding a variant forces handling at every match site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyId {
    Red,
    Blue,
    Gold,
}

impl std::fmt::Display for KeyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyId::Red => write!(f, "red_key"),
            KeyId::Blue => write!(f, "blue_key"),
            KeyId::Gold => write!(f, "gold_key"),
        }
    }
}

/// How two nodes are connected.
#[derive(Debug, Clone)]
pub enum EdgeKind {
    /// Physically adjacent — player flies directly between them.
    /// Requires matching connectors on the grid.
    Adjacent {
        from_connector: Connector,
        to_connector: Connector,
    },
    /// Teleporter — no spatial adjacency required.
    /// Player steps on a pad/trigger and is transported.
    Teleporter,
    /// One-way connection (e.g. a chute, vent, or collapse).
    /// Stored in an undirected graph but the direction is semantic:
    /// the player can only traverse from → to.
    OneWay {
        from_connector: Connector,
    },
    /// Locked door — requires a key or trigger to open.
    Locked {
        from_connector: Connector,
        to_connector: Connector,
        key_id: KeyId,
    },
}

#[derive(Debug)]
pub enum PlaceError {
    Overlap { cell: [i32; 3], existing_room: NodeIndex },
}

#[derive(Debug)]
pub enum ConnectError {
    InvalidRoomIndex,
    NoMatchingConnectors,
}

/// The full level layout as a graph of flyable spaces connected by edges.
/// Backed by petgraph for correct, battle-tested graph algorithms.
/// Generated in pure Rust, then handed to LevelManager (Godot node)
/// to instantiate the actual geometry.
#[derive(Debug)]
pub struct LevelGraph {
    graph: UnGraph<PlacedRoom, EdgeKind>,
    /// Tracks which grid cells are occupied to prevent overlap.
    occupied: HashMap<[i32; 3], NodeIndex>,
}

impl Default for LevelGraph {
    fn default() -> Self {
        Self {
            graph: UnGraph::new_undirected(),
            occupied: HashMap::new(),
        }
    }
}

impl LevelGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check whether a grid cell is free.
    pub fn is_free(&self, pos: [i32; 3]) -> bool {
        !self.occupied.contains_key(&pos)
    }

    /// Place a room at a grid position. Returns the node index.
    /// Fails if any cell the room would occupy is already taken.
    pub fn place_room(&mut self, template: RoomTemplate, grid_pos: [i32; 3]) -> Result<NodeIndex, PlaceError> {
        let cells = cells_for(&template, grid_pos);
        for cell in &cells {
            if let Some(&existing) = self.occupied.get(cell) {
                return Err(PlaceError::Overlap { cell: *cell, existing_room: existing });
            }
        }

        let node = self.graph.add_node(PlacedRoom {
            template,
            grid_pos,
        });

        for cell in cells {
            self.occupied.insert(cell, node);
        }

        Ok(node)
    }

    /// Connect two spatially adjacent rooms through ALL matching connector pairs.
    /// Returns the number of edges created. Multi-story rooms may have multiple
    /// matching connectors on the same face at different Y levels.
    pub fn connect_adjacent(&mut self, from: NodeIndex, to: NodeIndex) -> Result<usize, ConnectError> {
        let from_room = self.graph.node_weight(from)
            .ok_or(ConnectError::InvalidRoomIndex)?;
        let to_room = self.graph.node_weight(to)
            .ok_or(ConnectError::InvalidRoomIndex)?;

        let from_origin = from_room.grid_pos;
        let to_origin = to_room.grid_pos;
        let from_template = from_room.template.clone();
        let to_template = to_room.template.clone();

        let to_cells = cells_for(&to_template, to_origin);

        let mut count = 0;
        for fc in &from_template.connectors {
            let target = fc.target_cell(from_origin);
            if !to_cells.contains(&target) {
                continue;
            }

            for tc in &to_template.connectors {
                if fc.mates_with(tc) {
                    // Both connectors must point at each other's source cell.
                    let tc_source = [to_origin[0] + tc.offset[0], to_origin[1] + tc.offset[1], to_origin[2] + tc.offset[2]];
                    let fc_source = [from_origin[0] + fc.offset[0], from_origin[1] + fc.offset[1], from_origin[2] + fc.offset[2]];
                    if target == tc_source && tc.target_cell(to_origin) == fc_source {
                        self.graph.add_edge(from, to, EdgeKind::Adjacent {
                            from_connector: *fc,
                            to_connector: *tc,
                        });
                        count += 1;
                    }
                }
            }
        }

        if count == 0 {
            Err(ConnectError::NoMatchingConnectors)
        } else {
            Ok(count)
        }
    }

    /// Connect two rooms via teleporter. No spatial adjacency required.
    pub fn connect_teleporter(&mut self, from: NodeIndex, to: NodeIndex) -> Result<(), ConnectError> {
        if self.graph.node_weight(from).is_none() {
            return Err(ConnectError::InvalidRoomIndex);
        }
        if self.graph.node_weight(to).is_none() {
            return Err(ConnectError::InvalidRoomIndex);
        }
        self.graph.add_edge(from, to, EdgeKind::Teleporter);
        Ok(())
    }

    /// Connect two rooms via locked door through ALL matching connectors.
    pub fn connect_locked(&mut self, from: NodeIndex, to: NodeIndex, key_id: KeyId) -> Result<usize, ConnectError> {
        let from_room = self.graph.node_weight(from)
            .ok_or(ConnectError::InvalidRoomIndex)?;
        let to_room = self.graph.node_weight(to)
            .ok_or(ConnectError::InvalidRoomIndex)?;

        let from_origin = from_room.grid_pos;
        let to_origin = to_room.grid_pos;
        let from_template = from_room.template.clone();
        let to_template = to_room.template.clone();

        let to_cells = cells_for(&to_template, to_origin);

        let mut count = 0;
        for fc in &from_template.connectors {
            let target = fc.target_cell(from_origin);
            if !to_cells.contains(&target) {
                continue;
            }

            for tc in &to_template.connectors {
                if fc.mates_with(tc) {
                    let tc_source = [to_origin[0] + tc.offset[0], to_origin[1] + tc.offset[1], to_origin[2] + tc.offset[2]];
                    let fc_source = [from_origin[0] + fc.offset[0], from_origin[1] + fc.offset[1], from_origin[2] + fc.offset[2]];
                    if target == tc_source && tc.target_cell(to_origin) == fc_source {
                        self.graph.add_edge(from, to, EdgeKind::Locked {
                            from_connector: *fc,
                            to_connector: *tc,
                            key_id,
                        });
                        count += 1;
                    }
                }
            }
        }

        if count == 0 {
            Err(ConnectError::NoMatchingConnectors)
        } else {
            Ok(count)
        }
    }

    pub fn room_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Check whether every room is reachable from every other room.
    pub fn is_fully_connected(&self) -> bool {
        connected_components(&self.graph) <= 1
    }

    /// Get a room by its node index.
    pub fn room(&self, index: NodeIndex) -> Option<&PlacedRoom> {
        self.graph.node_weight(index)
    }

    /// Iterate over all room node indices.
    pub fn room_indices(&self) -> impl Iterator<Item = NodeIndex> + '_ {
        self.graph.node_indices()
    }

    /// Iterate over all edges as (from, to, edge_kind).
    pub fn edges(&self) -> impl Iterator<Item = (NodeIndex, NodeIndex, &EdgeKind)> + '_ {
        self.graph.edge_indices().map(move |e| {
            let (a, b) = self.graph.edge_endpoints(e).unwrap();
            (a, b, self.graph.edge_weight(e).unwrap())
        })
    }

    /// Get neighbors of a room.
    pub fn neighbors(&self, index: NodeIndex) -> impl Iterator<Item = NodeIndex> + '_ {
        self.graph.neighbors(index)
    }

    /// Return the specific connectors that are actually wired to a neighbor.
    /// Each returned `Connector` has the exact offset + facing of the wired
    /// connection point, so consumers can distinguish y=0 from y=1 on the
    /// same face.
    pub fn active_connectors(&self, index: NodeIndex) -> Vec<Connector> {
        let mut connectors = Vec::new();
        for edge_ref in self.graph.edges(index) {
            let (from, _to) = self.graph.edge_endpoints(edge_ref.id()).unwrap();
            match edge_ref.weight() {
                EdgeKind::Adjacent { from_connector, to_connector } |
                EdgeKind::Locked { from_connector, to_connector, .. } => {
                    if from == index {
                        connectors.push(*from_connector);
                    } else {
                        connectors.push(*to_connector);
                    }
                }
                EdgeKind::Teleporter => {}
                EdgeKind::OneWay { from_connector } => {
                    if from == index {
                        connectors.push(*from_connector);
                    }
                }
            }
        }
        connectors
    }

    /// Return just the facings of active connectors. Convenience method
    /// for code that only needs direction, not position.
    pub fn active_facings(&self, index: NodeIndex) -> Vec<ConnectorFacing> {
        self.active_connectors(index)
            .into_iter()
            .map(|c| c.facing)
            .collect()
    }
}

/// Enumerate all grid cells a room occupies given its origin.
fn cells_for(template: &RoomTemplate, origin: [i32; 3]) -> Vec<[i32; 3]> {
    let [ex, ey, ez] = template.extents.map(|e| e as i32);
    (0..ex).flat_map(|x| {
        (0..ey).flat_map(move |y| {
            (0..ez).map(move |z| [origin[0] + x, origin[1] + y, origin[2] + z])
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room_template::*;

    // --- Test fixtures ---

    fn room_1x1_east_west() -> RoomTemplate {
        RoomTemplate {
            id: "1x1_ew",
            kind: TemplateKind::Room,
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
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
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
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 1, 1],
        }
    }

    fn corridor_1x1_east_west() -> RoomTemplate {
        RoomTemplate {
            id: "corridor_ew",
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
        assert!(!graph.is_free([0, 0, 0]));
        assert!(!graph.is_free([1, 0, 0]));
        assert!(graph.is_free([2, 0, 0]));
    }

    #[test]
    fn partial_overlap_with_multi_cell_is_rejected() {
        let mut graph = LevelGraph::new();
        graph.place_room(room_2x1_east_west(), [0, 0, 0]).unwrap();
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

    // --- KeyId tests ---

    #[test]
    fn key_id_display_matches_legacy_string() {
        assert_eq!(format!("{}", KeyId::Red), "red_key");
        assert_eq!(format!("{}", KeyId::Blue), "blue_key");
        assert_eq!(format!("{}", KeyId::Gold), "gold_key");
    }

    #[test]
    fn key_id_roundtrips_through_display() {
        for key in [KeyId::Red, KeyId::Blue, KeyId::Gold] {
            let s = format!("{}", key);
            assert!(!s.is_empty());
        }
    }

    // --- Adjacent connection tests ---

    #[test]
    fn connect_adjacent_rooms_with_matching_connectors() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [1, 0, 0]).unwrap();
        let result = graph.connect_adjacent(a, b);
        assert!(result.is_ok());
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn connect_adjacent_fails_with_invalid_index() {
        let mut graph = LevelGraph::new();
        graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let bogus = NodeIndex::new(99);
        let result = graph.connect_adjacent(NodeIndex::new(0), bogus);
        assert!(matches!(result, Err(ConnectError::InvalidRoomIndex)));
    }

    #[test]
    fn connect_adjacent_fails_when_no_matching_connectors() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [0, 0, 1]).unwrap();
        let result = graph.connect_adjacent(a, b);
        assert!(matches!(result, Err(ConnectError::NoMatchingConnectors)));
    }

    #[test]
    fn connect_adjacent_non_adjacent_rooms_fails() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [5, 0, 0]).unwrap();
        let result = graph.connect_adjacent(a, b);
        assert!(matches!(result, Err(ConnectError::NoMatchingConnectors)));
    }

    #[test]
    fn connect_adjacent_multi_cell_rooms_through_far_connector() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_2x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [2, 0, 0]).unwrap();
        let result = graph.connect_adjacent(a, b);
        assert!(result.is_ok());
    }

    #[test]
    fn adjacent_edge_records_facings() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [1, 0, 0]).unwrap();
        graph.connect_adjacent(a, b).unwrap();

        let (_, _, edge) = graph.edges().next().unwrap();
        match edge {
            EdgeKind::Adjacent { from_connector, to_connector } => {
                assert_eq!(from_connector.facing, ConnectorFacing::PosX);
                assert_eq!(to_connector.facing, ConnectorFacing::NegX);
            }
            _ => panic!("Expected Adjacent edge"),
        }
    }

    // --- Corridor-as-node tests ---

    #[test]
    fn room_corridor_room_chain() {
        // The classic pattern: [Room A] -- [Corridor] -- [Room B]
        let mut graph = LevelGraph::new();
        let room_a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let corridor = graph.place_room(corridor_1x1_east_west(), [1, 0, 0]).unwrap();
        let room_b = graph.place_room(room_1x1_east_west(), [2, 0, 0]).unwrap();

        graph.connect_adjacent(room_a, corridor).unwrap();
        graph.connect_adjacent(corridor, room_b).unwrap();

        assert_eq!(graph.room_count(), 3);
        assert_eq!(graph.edge_count(), 2);
        assert!(graph.is_fully_connected());

        // Corridor has two neighbors
        let corridor_neighbors: Vec<_> = graph.neighbors(corridor).collect();
        assert_eq!(corridor_neighbors.len(), 2);
        assert!(corridor_neighbors.contains(&room_a));
        assert!(corridor_neighbors.contains(&room_b));
    }

    #[test]
    fn corridor_active_facings_has_both_directions() {
        // Corridor between two rooms must report BOTH NegX and PosX,
        // not [PosX, PosX] — regardless of which node was `from` in add_edge.
        let mut graph = LevelGraph::new();
        let room_a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let corridor = graph.place_room(corridor_1x1_east_west(), [1, 0, 0]).unwrap();
        let room_b = graph.place_room(room_1x1_east_west(), [2, 0, 0]).unwrap();

        // room_a is source of first edge, corridor is source of second
        graph.connect_adjacent(room_a, corridor).unwrap();
        graph.connect_adjacent(corridor, room_b).unwrap();

        let mut facings = graph.active_facings(corridor);
        facings.sort_by_key(|f| format!("{f:?}"));
        let mut expected = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];
        expected.sort_by_key(|f| format!("{f:?}"));
        assert_eq!(
            facings, expected,
            "corridor active_facings should be [NegX, PosX], got {facings:?}"
        );
    }

    #[test]
    fn corridor_is_not_directly_connecting_distant_rooms() {
        // Rooms at [0,0,0] and [3,0,0] can't be directly connected
        // even with a corridor at [1,0,0] — room B is too far.
        let mut graph = LevelGraph::new();
        let room_a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let _corridor = graph.place_room(corridor_1x1_east_west(), [1, 0, 0]).unwrap();
        let room_b = graph.place_room(room_1x1_east_west(), [3, 0, 0]).unwrap();

        // Direct connection between A and B should fail
        let result = graph.connect_adjacent(room_a, room_b);
        assert!(matches!(result, Err(ConnectError::NoMatchingConnectors)));
    }

    // --- Teleporter tests ---

    #[test]
    fn teleporter_connects_non_adjacent_rooms() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [100, 0, 0]).unwrap();
        // Can't connect adjacently
        assert!(graph.connect_adjacent(a, b).is_err());
        // But teleporter works
        let result = graph.connect_teleporter(a, b);
        assert!(result.is_ok());
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn teleporter_connects_rooms_with_incompatible_connectors() {
        // E/W room and N/S room can't connect adjacently on any axis,
        // but a teleporter doesn't care about connectors.
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_north_south(), [0, 0, 1]).unwrap();
        assert!(graph.connect_adjacent(a, b).is_err());
        assert!(graph.connect_teleporter(a, b).is_ok());
    }

    #[test]
    fn teleporter_fails_with_invalid_index() {
        let mut graph = LevelGraph::new();
        graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let bogus = NodeIndex::new(99);
        let result = graph.connect_teleporter(NodeIndex::new(0), bogus);
        assert!(matches!(result, Err(ConnectError::InvalidRoomIndex)));
    }

    #[test]
    fn teleporter_edge_is_typed_correctly() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [50, 0, 0]).unwrap();
        graph.connect_teleporter(a, b).unwrap();

        let (_, _, edge) = graph.edges().next().unwrap();
        assert!(matches!(edge, EdgeKind::Teleporter));
    }

    #[test]
    fn teleporter_counts_for_connectivity() {
        // Two distant rooms connected only by teleporter should be
        // considered fully connected.
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [50, 0, 0]).unwrap();
        assert!(!graph.is_fully_connected());
        graph.connect_teleporter(a, b).unwrap();
        assert!(graph.is_fully_connected());
    }

    // --- Locked door tests ---

    #[test]
    fn locked_door_requires_matching_connectors() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [5, 0, 0]).unwrap();
        let result = graph.connect_locked(a, b, KeyId::Red);
        assert!(matches!(result, Err(ConnectError::NoMatchingConnectors)));
    }

    #[test]
    fn locked_door_works_with_adjacent_connectors() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [1, 0, 0]).unwrap();
        let result = graph.connect_locked(a, b, KeyId::Red);
        assert!(result.is_ok());

        let (_, _, edge) = graph.edges().next().unwrap();
        match edge {
            EdgeKind::Locked { key_id, .. } => assert_eq!(*key_id, KeyId::Red),
            _ => panic!("Expected Locked edge"),
        }
    }

    // --- Mixed edge connectivity tests ---

    #[test]
    fn mixed_adjacent_and_teleporter_connectivity() {
        // [A] --adjacent-- [B]     [C] --teleporter-- [D]
        //                   \--teleporter--/
        // All four should be reachable.
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [1, 0, 0]).unwrap();
        let c = graph.place_room(room_1x1_east_west(), [50, 0, 0]).unwrap();
        let d = graph.place_room(room_1x1_east_west(), [51, 0, 0]).unwrap();

        graph.connect_adjacent(a, b).unwrap();
        graph.connect_teleporter(b, c).unwrap();
        graph.connect_adjacent(c, d).unwrap();

        assert!(graph.is_fully_connected());
        assert_eq!(graph.edge_count(), 3);
    }

    // --- General connectivity tests ---

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
    fn disconnected_rooms_are_not_fully_connected() {
        let mut graph = LevelGraph::new();
        graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        graph.place_room(room_1x1_east_west(), [5, 0, 0]).unwrap();
        assert!(!graph.is_fully_connected());
    }

    #[test]
    fn mixed_axes_connection() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_north_south(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_north_south(), [0, 0, 1]).unwrap();
        let result = graph.connect_adjacent(a, b);
        assert!(result.is_ok());
    }

    #[test]
    fn neighbors_returns_connected_rooms() {
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_1x1_east_west(), [1, 0, 0]).unwrap();
        let c = graph.place_room(room_1x1_east_west(), [2, 0, 0]).unwrap();
        graph.connect_adjacent(a, b).unwrap();
        graph.connect_adjacent(b, c).unwrap();

        let b_neighbors: Vec<_> = graph.neighbors(b).collect();
        assert_eq!(b_neighbors.len(), 2);
        assert!(b_neighbors.contains(&a));
        assert!(b_neighbors.contains(&c));

        let a_neighbors: Vec<_> = graph.neighbors(a).collect();
        assert_eq!(a_neighbors.len(), 1);
        assert!(a_neighbors.contains(&b));
    }

    // --- World position tests ---

    #[test]
    fn world_position_scales_grid_by_cell_size_and_cell_height() {
        let mut graph = LevelGraph::new();
        let idx = graph.place_room(room_1x1_east_west(), [3, -1, 2]).unwrap();
        let room = graph.room(idx).unwrap();
        let pos = room.world_position(10.0, 5.0);
        // XZ uses cell_size (10.0), Y uses CELL_HEIGHT (5.0)
        assert_eq!(pos, [30.0, -5.0, 20.0]);
    }

    // --- R2: Hub room with 6 cardinal connections ---

    fn room_3x1x3_hub_6way() -> RoomTemplate {
        RoomTemplate {
            id: "3x3_hub",
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX },
                Connector { offset: [2, 0, 1], facing: ConnectorFacing::PosX },
                Connector { offset: [1, 0, 0], facing: ConnectorFacing::NegZ },
                Connector { offset: [1, 0, 2], facing: ConnectorFacing::PosZ },
                Connector { offset: [1, 0, 1], facing: ConnectorFacing::PosY },
                Connector { offset: [1, 0, 1], facing: ConnectorFacing::NegY },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [3, 1, 3],
        }
    }

    fn room_1x1_vertical() -> RoomTemplate {
        RoomTemplate {
            id: "1x1_vert",
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosY },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegY },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 1, 1],
        }
    }

    #[test]
    fn hub_room_connects_six_neighbors() {
        let mut graph = LevelGraph::new();
        let hub = graph.place_room(room_3x1x3_hub_6way(), [0, 0, 0]).unwrap();

        // NegX neighbor at [-1, 0, 1] (targets hub's NegX connector at offset [0,0,1])
        let neg_x = graph.place_room(room_1x1_east_west(), [-1, 0, 1]).unwrap();
        // PosX neighbor at [3, 0, 1] (targets hub's PosX connector at offset [2,0,1])
        let pos_x = graph.place_room(room_1x1_east_west(), [3, 0, 1]).unwrap();
        // NegZ neighbor at [1, 0, -1] (targets hub's NegZ connector at offset [1,0,0])
        let neg_z = graph.place_room(room_1x1_north_south(), [1, 0, -1]).unwrap();
        // PosZ neighbor at [1, 0, 3] (targets hub's PosZ connector at offset [1,0,2])
        let pos_z = graph.place_room(room_1x1_north_south(), [1, 0, 3]).unwrap();
        // PosY neighbor at [1, 1, 1] (targets hub's PosY connector at offset [1,0,1])
        let pos_y = graph.place_room(room_1x1_vertical(), [1, 1, 1]).unwrap();
        // NegY neighbor at [1, -1, 1] (targets hub's NegY connector at offset [1,0,1])
        let neg_y = graph.place_room(room_1x1_vertical(), [1, -1, 1]).unwrap();

        graph.connect_adjacent(hub, neg_x).unwrap();
        graph.connect_adjacent(hub, pos_x).unwrap();
        graph.connect_adjacent(hub, neg_z).unwrap();
        graph.connect_adjacent(hub, pos_z).unwrap();
        graph.connect_adjacent(hub, pos_y).unwrap();
        graph.connect_adjacent(hub, neg_y).unwrap();

        assert_eq!(graph.edge_count(), 6, "hub should have 6 edges");
        let neighbors: Vec<_> = graph.neighbors(hub).collect();
        assert_eq!(neighbors.len(), 6, "hub should have 6 neighbors");
        assert!(graph.is_fully_connected(), "all 7 rooms should be connected");
    }

    #[test]
    fn hub_active_facings_returns_all_six() {
        let mut graph = LevelGraph::new();
        let hub = graph.place_room(room_3x1x3_hub_6way(), [0, 0, 0]).unwrap();

        let neg_x = graph.place_room(room_1x1_east_west(), [-1, 0, 1]).unwrap();
        let pos_x = graph.place_room(room_1x1_east_west(), [3, 0, 1]).unwrap();
        let neg_z = graph.place_room(room_1x1_north_south(), [1, 0, -1]).unwrap();
        let pos_z = graph.place_room(room_1x1_north_south(), [1, 0, 3]).unwrap();
        let pos_y = graph.place_room(room_1x1_vertical(), [1, 1, 1]).unwrap();
        let neg_y = graph.place_room(room_1x1_vertical(), [1, -1, 1]).unwrap();

        graph.connect_adjacent(hub, neg_x).unwrap();
        graph.connect_adjacent(hub, pos_x).unwrap();
        graph.connect_adjacent(hub, neg_z).unwrap();
        graph.connect_adjacent(hub, pos_z).unwrap();
        graph.connect_adjacent(hub, pos_y).unwrap();
        graph.connect_adjacent(hub, neg_y).unwrap();

        let mut facings = graph.active_facings(hub);
        facings.sort_by_key(|f| format!("{f:?}"));
        let mut expected = vec![
            ConnectorFacing::NegX, ConnectorFacing::PosX,
            ConnectorFacing::NegZ, ConnectorFacing::PosZ,
            ConnectorFacing::PosY, ConnectorFacing::NegY,
        ];
        expected.sort_by_key(|f| format!("{f:?}"));
        assert_eq!(facings, expected, "active_facings should return all 6 directions");
    }

    #[test]
    fn vertical_adjacent_connection() {
        let mut graph = LevelGraph::new();
        let bottom = graph.place_room(room_1x1_vertical(), [0, 0, 0]).unwrap();
        let top = graph.place_room(room_1x1_vertical(), [0, 1, 0]).unwrap();
        graph.connect_adjacent(bottom, top).unwrap();

        let (_, _, edge) = graph.edges().next().unwrap();
        match edge {
            EdgeKind::Adjacent { from_connector, to_connector } => {
                assert_eq!(from_connector.facing, ConnectorFacing::PosY);
                assert_eq!(to_connector.facing, ConnectorFacing::NegY);
            }
            _ => panic!("Expected Adjacent edge"),
        }
    }

    // --- Aperture alignment: multi-Y connector tests ---

    fn room_3x2x3_tall() -> RoomTemplate {
        RoomTemplate {
            id: "3x2x3_tall",
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX },
                Connector { offset: [2, 0, 1], facing: ConnectorFacing::PosX },
                Connector { offset: [0, 1, 1], facing: ConnectorFacing::NegX },
                Connector { offset: [2, 1, 1], facing: ConnectorFacing::PosX },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [3, 2, 3],
        }
    }

    #[test]
    fn connect_adjacent_wires_all_matching_y_levels() {
        // Two tall rooms side by side should create edges for BOTH y=0 and y=1 pairs.
        let mut graph = LevelGraph::new();
        let a = graph.place_room(room_3x2x3_tall(), [0, 0, 0]).unwrap();
        let b = graph.place_room(room_3x2x3_tall(), [3, 0, 0]).unwrap();
        graph.connect_adjacent(a, b).unwrap();
        assert_eq!(graph.edge_count(), 2,
            "two matching connector pairs (y=0 and y=1) should produce 2 edges");
    }

    #[test]
    fn tall_room_to_short_corridor_active_facings_precise() {
        // A tall room connected to a 1-cell corridor at y=0.
        // active_facings currently returns [NegX] which activates BOTH y=0 and y=1.
        // After fix: active_connectors should return only the y=0 connector.
        let mut graph = LevelGraph::new();
        let tall = graph.place_room(room_3x2x3_tall(), [0, 0, 0]).unwrap();
        let corridor = graph.place_room(corridor_1x1_east_west(), [-1, 0, 1]).unwrap();
        graph.connect_adjacent(tall, corridor).unwrap();

        let active = graph.active_connectors(tall);
        // Should contain the y=0 NegX connector
        assert!(active.iter().any(|c|
            c.facing == ConnectorFacing::NegX && c.offset == [0, 0, 1]),
            "should have y=0 NegX connector active");
        // Should NOT contain the y=1 NegX connector
        assert!(!active.iter().any(|c|
            c.facing == ConnectorFacing::NegX && c.offset == [0, 1, 1]),
            "y=1 NegX connector should NOT be active — nothing is wired there");
    }

    // --- R3: Multiple connections on the same face ---

    fn room_3x1x3_multi_negz() -> RoomTemplate {
        RoomTemplate {
            id: "3x3_multi_negz",
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
                Connector { offset: [2, 0, 0], facing: ConnectorFacing::NegZ },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [3, 1, 3],
        }
    }

    #[test]
    fn large_room_multiple_connections_same_face() {
        let mut graph = LevelGraph::new();
        let room = graph.place_room(room_3x1x3_multi_negz(), [0, 0, 0]).unwrap();

        // Two neighbors on the NegZ face at different X positions
        let n1 = graph.place_room(room_1x1_north_south(), [0, 0, -1]).unwrap();
        let n2 = graph.place_room(room_1x1_north_south(), [2, 0, -1]).unwrap();

        graph.connect_adjacent(room, n1).unwrap();
        graph.connect_adjacent(room, n2).unwrap();

        assert_eq!(graph.edge_count(), 2, "two connections on the same face");
        assert!(graph.is_fully_connected());
    }
}
