//! Abstract level graph: topology only, no spatial positions.
//!
//! Sweep 1 of the generation pipeline. Produces a connected graph of rooms
//! with connector pairings but no grid coordinates.

use petgraph::graph::{NodeIndex, UnGraph};
use rand::rngs::SmallRng;

use crate::room_template::{ConnectorFacing, RoomTemplate};

/// Fraction of spanning-tree edges that take a vertical (up/down) connector
/// when both rooms offer one. Keeps vertical passages on the level's main
/// path rather than relegating them to incidental side branches. Tunable.
const VERTICAL_BIAS: f32 = 0.5;

/// Which connector on each room is wired in an edge.
#[derive(Debug, Clone, Copy)]
pub struct ConnectorPair {
    /// Index into the `from` room's connectors vec.
    pub from_connector_idx: usize,
    /// Index into the `to` room's connectors vec.
    pub to_connector_idx: usize,
}

/// An unpositioned graph of rooms connected by connector pairs.
pub struct AbstractGraph {
    pub(crate) graph: UnGraph<RoomTemplate, ConnectorPair>,
    pub(crate) root: NodeIndex,
}

impl AbstractGraph {
    pub fn root(&self) -> NodeIndex {
        self.root
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn room(&self, idx: NodeIndex) -> Option<&RoomTemplate> {
        self.graph.node_weight(idx)
    }

    pub fn node_indices(&self) -> impl Iterator<Item = NodeIndex> + '_ {
        self.graph.node_indices()
    }

    pub fn neighbors(&self, idx: NodeIndex) -> impl Iterator<Item = NodeIndex> + '_ {
        self.graph.neighbors(idx)
    }

    /// Iterate edges as (from, to, connector_pair).
    pub fn edges(&self) -> impl Iterator<Item = (NodeIndex, NodeIndex, &ConnectorPair)> + '_ {
        use petgraph::visit::EdgeRef;
        self.graph.edge_references().map(|e| {
            let (from, to) = (e.source(), e.target());
            (from, to, e.weight())
        })
    }
}

/// Build an abstract topology from a seed and config.
///
/// Generates rooms procedurally and connects them into a spanning tree.
/// Topology is independent of spatial layout.
pub fn generate_topology(
    rng: &mut SmallRng,
    room_count: usize,
    config: &crate::generator::GeneratorConfig,
) -> AbstractGraph {
    use rand::prelude::IndexedRandom;

    let mut graph = UnGraph::new_undirected();

    // Generate all rooms up front.
    let mut indices = Vec::with_capacity(room_count);
    for _ in 0..room_count {
        let room = crate::generator::generate_room(rng, config);
        indices.push(graph.add_node(room));
    }

    let root = indices[0];

    // Build a spanning tree: each room i (1..N) connects to a random existing room.
    for i in 1..room_count {
        let child_idx = indices[i];
        // Pick a random parent from rooms already in the tree.
        let &parent_idx = indices[..i].choose(rng).expect("non-empty slice");

        // Find a compatible connector pair: parent facing == child facing.opposite().
        let parent_room = &graph[parent_idx];
        let child_room = &graph[child_idx];

        if let Some(pair) = find_compatible_pair(parent_room, child_room, rng) {
            graph.add_edge(parent_idx, child_idx, pair);
        }
    }

    AbstractGraph { graph, root }
}

/// Find a random compatible connector pair between two rooms.
/// Compatible means parent connector facing is the opposite of child connector facing.
fn find_compatible_pair(
    parent: &RoomTemplate,
    child: &RoomTemplate,
    rng: &mut SmallRng,
) -> Option<ConnectorPair> {
    use rand::prelude::SliceRandom;
    use rand::RngExt;

    // Collect all compatible pairs, split by orientation so vertical links
    // can be favored.
    let mut vertical = Vec::new();
    let mut horizontal = Vec::new();
    for (pi, pc) in parent.connectors.iter().enumerate() {
        for (ci, cc) in child.connectors.iter().enumerate() {
            if pc.facing.opposite() == cc.facing {
                let pair = ConnectorPair { from_connector_idx: pi, to_connector_idx: ci };
                if matches!(pc.facing, ConnectorFacing::PosY | ConnectorFacing::NegY) {
                    vertical.push(pair);
                } else {
                    horizontal.push(pair);
                }
            }
        }
    }

    // Favor a vertical link when one exists (and we win the bias roll, or
    // there is no horizontal alternative); otherwise take a horizontal one.
    let take_vertical = !vertical.is_empty()
        && (horizontal.is_empty() || rng.random_range(0.0f32..1.0) < VERTICAL_BIAS);
    let pool = if take_vertical { &mut vertical } else { &mut horizontal };
    pool.shuffle(rng);
    pool.first().copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::GeneratorConfig;
    use rand::SeedableRng;

    fn test_config() -> GeneratorConfig {
        GeneratorConfig {
            seed: crate::seed::Seed::new(42),
            max_rooms: 0,
            min_room_xz: 3,
            max_room_xz: 6,
            min_room_y: 1,
            max_room_y: 6,
        }
    }

    fn room_with_horizontal_and_vertical() -> RoomTemplate {
        use crate::room_template::*;
        let h = |facing| Connector { offset: [0, 0, 0], facing, frame: FrameStyle::Door };
        let v = |facing| Connector { offset: [0, 0, 0], facing, frame: FrameStyle::None };
        RoomTemplate {
            kind: TemplateKind::Room,
            connectors: vec![
                h(ConnectorFacing::NegX),
                h(ConnectorFacing::PosX),
                h(ConnectorFacing::NegZ),
                h(ConnectorFacing::PosZ),
                v(ConnectorFacing::PosY),
                v(ConnectorFacing::NegY),
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [4, 1, 4],
        }
    }

    /// When both rooms offer a vertical connector, the spanning tree favors
    /// it: vertical links should be chosen far above their uniform 2-of-6
    /// share (~0.33), so up/down passages land on the main path.
    #[test]
    fn vertical_links_are_favored_when_available() {
        use crate::room_template::ConnectorFacing;
        let room = room_with_horizontal_and_vertical();
        let mut rng = SmallRng::seed_from_u64(7);
        let n = 4000;
        let mut vertical = 0;
        for _ in 0..n {
            let pair = find_compatible_pair(&room, &room, &mut rng).unwrap();
            if matches!(
                room.connectors[pair.from_connector_idx].facing,
                ConnectorFacing::PosY | ConnectorFacing::NegY
            ) {
                vertical += 1;
            }
        }
        let share = vertical as f64 / n as f64;
        assert!(
            (0.42..=0.60).contains(&share),
            "vertical link share {share:.3} should sit near the 0.5 bias"
        );
    }

    #[test]
    fn topology_produces_requested_room_count() {
        let mut rng = SmallRng::seed_from_u64(42);
        let graph = generate_topology(&mut rng, 10, &test_config());
        assert_eq!(graph.node_count(), 10,
            "expected 10 rooms, got {}", graph.node_count());
    }

    #[test]
    fn topology_is_connected() {
        let mut rng = SmallRng::seed_from_u64(42);
        let graph = generate_topology(&mut rng, 10, &test_config());
        assert_eq!(
            petgraph::algo::connected_components(&graph.graph), 1,
            "abstract graph should be fully connected"
        );
    }

    #[test]
    fn topology_is_deterministic() {
        let config = test_config();
        let mut rng_a = SmallRng::seed_from_u64(42);
        let mut rng_b = SmallRng::seed_from_u64(42);
        let graph_a = generate_topology(&mut rng_a, 10, &config);
        let graph_b = generate_topology(&mut rng_b, 10, &config);
        assert_eq!(graph_a.node_count(), graph_b.node_count());
        // Same rooms in same order
        for (a, b) in graph_a.node_indices().zip(graph_b.node_indices()) {
            assert_eq!(
                graph_a.room(a).unwrap().extents,
                graph_b.room(b).unwrap().extents,
            );
        }
    }

    #[test]
    fn all_edges_have_compatible_facings() {
        let mut rng = SmallRng::seed_from_u64(42);
        let graph = generate_topology(&mut rng, 10, &test_config());
        for (from, to, pair) in graph.edges() {
            let from_room = graph.room(from).unwrap();
            let to_room = graph.room(to).unwrap();
            let from_facing = from_room.connectors[pair.from_connector_idx].facing;
            let to_facing = to_room.connectors[pair.to_connector_idx].facing;
            assert_eq!(
                from_facing.opposite(), to_facing,
                "edge connectors must have opposite facings: {:?} vs {:?}",
                from_facing, to_facing
            );
        }
    }

    #[test]
    fn every_non_root_room_has_at_least_one_edge() {
        let mut rng = SmallRng::seed_from_u64(42);
        let graph = generate_topology(&mut rng, 10, &test_config());
        for idx in graph.node_indices() {
            let degree = graph.neighbors(idx).count();
            assert!(degree >= 1,
                "room {:?} has 0 edges — disconnected", idx);
        }
    }
}
