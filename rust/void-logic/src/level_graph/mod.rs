use std::collections::HashMap;
use petgraph::graph::UnGraph;
use petgraph::algo::connected_components;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use crate::room_template::{Connector, ConnectorFacing, RoomTemplate};

mod types;
pub use types::*;

#[cfg(test)]
mod tests;

/// How many opaque rooms deep cull visibility reaches from the player's
/// room: 2 lights the current room, the room through a corridor, and the
/// room one corridor past that. Corridors between them are transparent and
/// don't count. See [`LevelGraph::visible_from`].
pub const RENDER_ROOM_DEPTH: usize = 2;

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

    /// Find the room node farthest from `start` by BFS hop count.
    /// Only considers Room nodes, not corridors.
    pub fn farthest_room_from(&self, start: NodeIndex) -> Option<NodeIndex> {
        use petgraph::visit::Bfs;

        let mut bfs = Bfs::new(&self.graph, start);
        let mut farthest = None;

        while let Some(node) = bfs.next(&self.graph) {
            if let Some(room) = self.graph.node_weight(node) {
                if room.template.kind == crate::room_template::TemplateKind::Room {
                    farthest = Some(node);
                }
            }
        }

        farthest
    }

    /// Room nodes visible from `start` for rendering: the node itself plus
    /// every node reachable without entering more than `budget` opaque
    /// rooms. Corridors are transparent (free to pass — they have exactly
    /// two opposite connectors, so sight flows straight through); entering
    /// a room costs one unit of `budget`. Teleporter edges are never
    /// traversed — they span the map, so the room on the far side isn't in
    /// view. This is the single authority for cull visibility; the shell
    /// resolves the player's current node and calls it.
    pub fn visible_from(&self, start: NodeIndex, budget: usize) -> Vec<NodeIndex> {
        use petgraph::algo::dijkstra;
        use petgraph::visit::EdgeFiltered;
        use crate::room_template::TemplateKind;

        // Teleporters span the map — you don't see the room on the far
        // side, so they aren't traversable for visibility.
        let by_flight =
            EdgeFiltered::from_fn(&self.graph, |e| !matches!(e.weight(), EdgeKind::Teleporter));
        // Cost to step onto a node: a room spends one unit of budget, a
        // corridor none. dijkstra advances to `e.target()`, so costing the
        // target is exactly the cost of entering that node.
        let costs = dijkstra(&by_flight, start, None, |e| {
            match self.graph.node_weight(e.target()) {
                Some(room) if room.template.kind == TemplateKind::Room => 1usize,
                _ => 0usize,
            }
        });
        costs
            .into_iter()
            .filter(|&(_, cost)| cost <= budget)
            .map(|(node, _)| node)
            .collect()
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
