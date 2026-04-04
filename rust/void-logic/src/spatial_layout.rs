//! Spatial layout: assign grid positions to an abstract topology.
//!
//! Sweep 2 of the generation pipeline. Walks the abstract graph in BFS order,
//! placing rooms far enough apart that they never overlap, with corridors
//! generated dynamically to fill the gaps.

use crate::abstract_graph::AbstractGraph;
use crate::level_graph::LevelGraph;
use crate::room_template::{Connector, ConnectorFacing, RoomTemplate, TemplateKind};

/// Generate a corridor template of the given length along a facing direction.
pub fn make_corridor(facing: ConnectorFacing, length: u32) -> RoomTemplate {
    let (extents, c_in, c_out) = match facing {
        ConnectorFacing::PosX => (
            [length, 1, 1],
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            Connector { offset: [length as i32 - 1, 0, 0], facing: ConnectorFacing::PosX },
        ),
        ConnectorFacing::NegX => (
            [length, 1, 1],
            Connector { offset: [length as i32 - 1, 0, 0], facing: ConnectorFacing::PosX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
        ),
        ConnectorFacing::PosZ => (
            [1, 1, length],
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
            Connector { offset: [0, 0, length as i32 - 1], facing: ConnectorFacing::PosZ },
        ),
        ConnectorFacing::NegZ => (
            [1, 1, length],
            Connector { offset: [0, 0, length as i32 - 1], facing: ConnectorFacing::PosZ },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
        ),
        ConnectorFacing::PosY => (
            [1, length, 1],
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegY },
            Connector { offset: [0, length as i32 - 1, 0], facing: ConnectorFacing::PosY },
        ),
        ConnectorFacing::NegY => (
            [1, length, 1],
            Connector { offset: [0, length as i32 - 1, 0], facing: ConnectorFacing::PosY },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegY },
        ),
    };

    RoomTemplate {
        kind: TemplateKind::Corridor,
        connectors: vec![c_in, c_out],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents,
    }
}

/// Try placing a child room connected to a parent via a specific connector pair.
/// Returns `Some((corridor_idx, child_idx))` on success.
fn try_place_child(
    level: &mut LevelGraph,
    parent_level_idx: petgraph::graph::NodeIndex,
    parent_ci: usize,
    child_room: &RoomTemplate,
    child_ci: usize,
    max_probe: i32,
) -> Option<(petgraph::graph::NodeIndex, petgraph::graph::NodeIndex)> {
    let parent_room = level.room(parent_level_idx)?;
    let parent_pos = parent_room.grid_pos;
    let parent_connector = &parent_room.template.connectors[parent_ci];
    let parent_facing = parent_connector.facing;
    let direction = parent_facing.grid_offset();

    let corridor_start = [
        parent_pos[0] + parent_connector.offset[0] + direction[0],
        parent_pos[1] + parent_connector.offset[1] + direction[1],
        parent_pos[2] + parent_connector.offset[2] + direction[2],
    ];

    let child_connector = &child_room.connectors[child_ci];

    for corridor_len in 1..=max_probe {
        let corridor_end = [
            corridor_start[0] + direction[0] * (corridor_len - 1),
            corridor_start[1] + direction[1] * (corridor_len - 1),
            corridor_start[2] + direction[2] * (corridor_len - 1),
        ];

        let child_connector_cell = [
            corridor_end[0] + direction[0],
            corridor_end[1] + direction[1],
            corridor_end[2] + direction[2],
        ];
        let child_pos = [
            child_connector_cell[0] - child_connector.offset[0],
            child_connector_cell[1] - child_connector.offset[1],
            child_connector_cell[2] - child_connector.offset[2],
        ];

        let corridor = make_corridor(parent_facing, corridor_len as u32);
        let corridor_origin = corridor_start_origin(corridor_start, parent_facing, &corridor);

        let corridor_cells = cells_for_at(&corridor, corridor_origin);
        if !corridor_cells.iter().all(|c| level.is_free(*c)) {
            continue;
        }

        let child_cells = cells_for_at(child_room, child_pos);
        if !child_cells.iter().all(|c| level.is_free(*c)) {
            continue;
        }

        if let Ok(corridor_idx) = level.place_room(corridor, corridor_origin) {
            let _ = level.connect_adjacent(parent_level_idx, corridor_idx);

            if let Ok(child_idx) = level.place_room(child_room.clone(), child_pos) {
                let _ = level.connect_adjacent(corridor_idx, child_idx);
                return Some((corridor_idx, child_idx));
            }
        }
        break;
    }

    None
}

/// Build all compatible connector pairs between a parent and child room.
fn all_compatible_pairs(parent: &RoomTemplate, child: &RoomTemplate) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for (pi, pc) in parent.connectors.iter().enumerate() {
        for (ci, cc) in child.connectors.iter().enumerate() {
            if pc.facing.opposite() == cc.facing {
                pairs.push((pi, ci));
            }
        }
    }
    pairs
}

/// Assign grid positions to rooms in an abstract graph, producing a fully
/// positioned LevelGraph with dynamically generated corridors.
///
/// BFS from the root. For each edge, try the designated connector pair first,
/// then fall back to all compatible pairs. Deferred rooms get a retry pass
/// against every already-placed room.
pub fn assign_positions(abstract_graph: &AbstractGraph) -> LevelGraph {
    use petgraph::visit::Bfs;
    use std::collections::{HashMap, HashSet};

    let mut level = LevelGraph::new();
    let mut placed: HashMap<petgraph::graph::NodeIndex, petgraph::graph::NodeIndex> = HashMap::new();
    let mut visited: HashSet<petgraph::graph::NodeIndex> = HashSet::new();

    let max_probe: i32 = 100;

    // Place root at origin.
    let root = abstract_graph.root();
    let root_room = abstract_graph.room(root).unwrap().clone();
    let root_level_idx = level.place_room(root_room, [0, 0, 0])
        .expect("root placement cannot overlap");
    placed.insert(root, root_level_idx);
    visited.insert(root);

    // Track rooms that couldn't be placed during BFS for a retry pass.
    let mut deferred: Vec<petgraph::graph::NodeIndex> = Vec::new();

    // BFS traversal.
    let mut bfs = Bfs::new(&abstract_graph.graph, root);
    bfs.next(&abstract_graph.graph); // skip root

    while let Some(abs_node) = bfs.next(&abstract_graph.graph) {
        if visited.contains(&abs_node) {
            continue;
        }
        visited.insert(abs_node);

        let edge = abstract_graph.edges().find(|(from, to, _)| {
            (*to == abs_node && placed.contains_key(from))
                || (*from == abs_node && placed.contains_key(to))
        });

        let Some((edge_from, edge_to, pair)) = edge else {
            deferred.push(abs_node);
            continue;
        };

        let (parent_abs, _child_abs, designated_pci, designated_cci) =
            if placed.contains_key(&edge_from) {
                (edge_from, edge_to, pair.from_connector_idx, pair.to_connector_idx)
            } else {
                (edge_to, edge_from, pair.to_connector_idx, pair.from_connector_idx)
            };

        let parent_level_idx = placed[&parent_abs];
        let child_room = abstract_graph.room(abs_node).unwrap().clone();

        // Try the designated connector pair first.
        if let Some((_corr, child_idx)) =
            try_place_child(&mut level, parent_level_idx, designated_pci, &child_room, designated_cci, max_probe)
        {
            placed.insert(abs_node, child_idx);
            continue;
        }

        // Fall back: try all compatible pairs between this parent and child.
        let parent_template = level.room(parent_level_idx).unwrap().template.clone();
        let pairs = all_compatible_pairs(&parent_template, &child_room);
        let mut success = false;
        for (pci, cci) in &pairs {
            if *pci == designated_pci && *cci == designated_cci {
                continue; // already tried
            }
            if let Some((_corr, child_idx)) =
                try_place_child(&mut level, parent_level_idx, *pci, &child_room, *cci, max_probe)
            {
                placed.insert(abs_node, child_idx);
                success = true;
                break;
            }
        }

        if !success {
            deferred.push(abs_node);
        }
    }

    // Retry pass: try deferred rooms against ALL placed rooms.
    for abs_node in &deferred {
        let child_room = abstract_graph.room(*abs_node).unwrap().clone();
        let placed_snapshot: Vec<_> = placed.values().copied().collect();

        let mut success = false;
        for parent_level_idx in &placed_snapshot {
            let parent_template = level.room(*parent_level_idx).unwrap().template.clone();
            let pairs = all_compatible_pairs(&parent_template, &child_room);
            for (pci, cci) in &pairs {
                if let Some((_corr, child_idx)) =
                    try_place_child(&mut level, *parent_level_idx, *pci, &child_room, *cci, max_probe)
                {
                    placed.insert(*abs_node, child_idx);
                    success = true;
                    break;
                }
            }
            if success {
                break;
            }
        }
    }

    level
}

/// Compute the corridor's grid origin given the start cell and facing.
fn corridor_start_origin(start: [i32; 3], facing: ConnectorFacing, corridor: &RoomTemplate) -> [i32; 3] {
    // The corridor's inward connector (the one facing the parent) must be at `start`.
    // Find that connector.
    let inward_facing = facing.opposite();
    let inward_connector = corridor.connectors.iter()
        .find(|c| c.facing == inward_facing)
        .expect("corridor must have inward connector");

    [
        start[0] - inward_connector.offset[0],
        start[1] - inward_connector.offset[1],
        start[2] - inward_connector.offset[2],
    ]
}

/// Compute all cells a room occupies at a given origin.
fn cells_for_at(template: &RoomTemplate, origin: [i32; 3]) -> Vec<[i32; 3]> {
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
    use crate::abstract_graph::{self, ConnectorPair};
    use crate::generator::GeneratorConfig;
    use crate::room_template::*;
    use petgraph::graph::UnGraph;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    fn simple_room(ex: u32, ez: u32) -> RoomTemplate {
        let mid_x = (ex as i32) / 2;
        let mid_z = (ez as i32) / 2;
        RoomTemplate {
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, mid_z], facing: ConnectorFacing::NegX },
                Connector { offset: [ex as i32 - 1, 0, mid_z], facing: ConnectorFacing::PosX },
                Connector { offset: [mid_x, 0, 0], facing: ConnectorFacing::NegZ },
                Connector { offset: [mid_x, 0, ez as i32 - 1], facing: ConnectorFacing::PosZ },
                Connector { offset: [mid_x, 0, mid_z], facing: ConnectorFacing::PosY },
                Connector { offset: [mid_x, 0, mid_z], facing: ConnectorFacing::NegY },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [ex, 1, ez],
        }
    }

    fn build_abstract(rooms: Vec<RoomTemplate>, edges: Vec<(usize, usize, ConnectorPair)>) -> AbstractGraph {
        let mut graph = UnGraph::new_undirected();
        let indices: Vec<_> = rooms.into_iter().map(|r| graph.add_node(r)).collect();
        for (from, to, pair) in edges {
            graph.add_edge(indices[from], indices[to], pair);
        }
        AbstractGraph { graph, root: indices[0] }
    }

    // PosX connector on room A (idx 1) connects to NegX connector on room B (idx 0)
    fn posx_negx_pair(room_a: &RoomTemplate, room_b: &RoomTemplate) -> ConnectorPair {
        let from_idx = room_a.connectors.iter().position(|c| c.facing == ConnectorFacing::PosX).unwrap();
        let to_idx = room_b.connectors.iter().position(|c| c.facing == ConnectorFacing::NegX).unwrap();
        ConnectorPair { from_connector_idx: from_idx, to_connector_idx: to_idx }
    }

    fn posy_negy_pair(room_a: &RoomTemplate, room_b: &RoomTemplate) -> ConnectorPair {
        let from_idx = room_a.connectors.iter().position(|c| c.facing == ConnectorFacing::PosY).unwrap();
        let to_idx = room_b.connectors.iter().position(|c| c.facing == ConnectorFacing::NegY).unwrap();
        ConnectorPair { from_connector_idx: from_idx, to_connector_idx: to_idx }
    }

    #[test]
    fn make_corridor_posx_correct_extents() {
        let c = make_corridor(ConnectorFacing::PosX, 5);
        assert_eq!(c.extents, [5, 1, 1]);
        assert_eq!(c.kind, TemplateKind::Corridor);
        assert!(c.connectors.iter().any(|c| c.facing == ConnectorFacing::NegX));
        assert!(c.connectors.iter().any(|c| c.facing == ConnectorFacing::PosX));
    }

    #[test]
    fn make_corridor_posy_correct_extents() {
        let c = make_corridor(ConnectorFacing::PosY, 3);
        assert_eq!(c.extents, [1, 3, 1]);
        assert!(c.connectors.iter().any(|c| c.facing == ConnectorFacing::NegY));
        assert!(c.connectors.iter().any(|c| c.facing == ConnectorFacing::PosY));
    }

    #[test]
    fn two_rooms_east_no_overlap() {
        let a = simple_room(3, 3);
        let b = simple_room(3, 3);
        let pair = posx_negx_pair(&a, &b);
        let ag = build_abstract(vec![a, b], vec![(0, 1, pair)]);
        let level = assign_positions(&ag);
        // Both rooms should be placed.
        let room_count = level.room_indices()
            .filter(|&idx| level.room(idx).map(|r| r.template.kind == TemplateKind::Room).unwrap_or(false))
            .count();
        assert_eq!(room_count, 2, "expected 2 rooms placed");
        assert!(level.is_fully_connected());
    }

    #[test]
    fn corridor_exists_between_rooms() {
        let a = simple_room(3, 3);
        let b = simple_room(3, 3);
        let pair = posx_negx_pair(&a, &b);
        let ag = build_abstract(vec![a, b], vec![(0, 1, pair)]);
        let level = assign_positions(&ag);
        let corridor_count = level.room_indices()
            .filter(|&idx| level.room(idx).map(|r| r.template.kind == TemplateKind::Corridor).unwrap_or(false))
            .count();
        assert!(corridor_count >= 1, "expected at least 1 corridor, got {corridor_count}");
    }

    #[test]
    fn vertical_connection_places_above() {
        let a = simple_room(3, 3);
        let b = simple_room(3, 3);
        let pair = posy_negy_pair(&a, &b);
        let ag = build_abstract(vec![a, b], vec![(0, 1, pair)]);
        let level = assign_positions(&ag);
        let rooms: Vec<_> = level.room_indices()
            .filter(|&idx| level.room(idx).map(|r| r.template.kind == TemplateKind::Room).unwrap_or(false))
            .filter_map(|idx| level.room(idx).map(|r| r.grid_pos))
            .collect();
        assert_eq!(rooms.len(), 2);
        assert!(rooms[1][1] > rooms[0][1],
            "second room should be above first: {:?} vs {:?}", rooms[0], rooms[1]);
    }

    #[test]
    fn large_rooms_push_children_further() {
        let a = simple_room(6, 6);
        let b = simple_room(3, 3);
        let pair = posx_negx_pair(&a, &b);
        let ag = build_abstract(vec![a, b], vec![(0, 1, pair)]);
        let level = assign_positions(&ag);
        let rooms: Vec<_> = level.room_indices()
            .filter(|&idx| level.room(idx).map(|r| r.template.kind == TemplateKind::Room).unwrap_or(false))
            .filter_map(|idx| level.room(idx).map(|r| r.grid_pos))
            .collect();
        assert_eq!(rooms.len(), 2);
        // Room B's origin should be beyond room A's extent (6) + corridor (≥1).
        assert!(rooms[1][0] >= 7,
            "6-wide room A + corridor should push B to x≥7, got x={}", rooms[1][0]);
    }

    #[test]
    fn branching_no_overlap() {
        // A connects east to B and south to C.
        let a = simple_room(3, 3);
        let b = simple_room(3, 3);
        let c = simple_room(3, 3);
        let pair_ab = posx_negx_pair(&a, &b);
        let pair_ac = ConnectorPair {
            from_connector_idx: a.connectors.iter().position(|c| c.facing == ConnectorFacing::PosZ).unwrap(),
            to_connector_idx: c.connectors.iter().position(|c| c.facing == ConnectorFacing::NegZ).unwrap(),
        };
        let ag = build_abstract(vec![a, b, c], vec![(0, 1, pair_ab), (0, 2, pair_ac)]);
        let level = assign_positions(&ag);
        let room_count = level.room_indices()
            .filter(|&idx| level.room(idx).map(|r| r.template.kind == TemplateKind::Room).unwrap_or(false))
            .count();
        assert_eq!(room_count, 3, "all 3 rooms should be placed");
        assert!(level.is_fully_connected());
    }

    #[test]
    fn rooms_have_active_connectors_at_apertures() {
        // Every room-corridor connection must produce active connectors
        // so that CellGrid creates ConnectorGap cells (apertures).
        let a = simple_room(3, 3);
        let b = simple_room(3, 3);
        let pair = posx_negx_pair(&a, &b);
        let ag = build_abstract(vec![a, b], vec![(0, 1, pair)]);
        let level = assign_positions(&ag);

        // Each room should have at least 1 active connector (the one wired to the corridor).
        for idx in level.room_indices() {
            let active = level.active_connectors(idx);
            let room = level.room(idx).unwrap();
            if room.template.kind == TemplateKind::Room {
                assert!(!active.is_empty(),
                    "Room at {:?} should have active connectors, got none", room.grid_pos);
            }
        }
    }

    #[test]
    fn generated_level_rooms_all_have_active_connectors() {
        use crate::generator;

        for seed in 0..10 {
            let config = GeneratorConfig {
                seed,
                max_rooms: 10,
                min_room_xz: 3,
                max_room_xz: 6,
                min_room_y: 1,
                max_room_y: 6,
            };
            let level = generator::generate(&config).expect("generation should succeed");
            for idx in level.room_indices() {
                let room = level.room(idx).unwrap();
                if room.template.kind == TemplateKind::Room {
                    let active = level.active_connectors(idx);
                    assert!(!active.is_empty(),
                        "seed {seed}: room at {:?} (extents {:?}) has 0 active connectors",
                        room.grid_pos, room.template.extents);
                }
            }
        }
    }

    #[test]
    fn fifteen_rooms_mostly_placed() {
        use crate::generator;
        // With 15 rooms requested, at least 12 should survive spatial placement.
        for seed in 0..20 {
            let config = GeneratorConfig {
                seed,
                max_rooms: 15,
                min_room_xz: 3,
                max_room_xz: 6,
                min_room_y: 1,
                max_room_y: 6,
            };
            let level = generator::generate(&config).expect("generation should succeed");
            let rooms = level.room_indices()
                .filter(|&idx| level.room(idx).map(|r| r.template.kind == TemplateKind::Room).unwrap_or(false))
                .count();
            assert!(rooms >= 12, "seed {seed}: expected ≥12 rooms, got {rooms}");
        }
    }

    #[test]
    fn result_is_fully_connected() {
        let mut rng = SmallRng::seed_from_u64(42);
        let config = GeneratorConfig {
            seed: 42,

            max_rooms: 0,
            min_room_xz: 3,
            max_room_xz: 6,
            min_room_y: 1,
            max_room_y: 6,
        };
        let ag = abstract_graph::generate_topology(&mut rng, 10, &config);
        let level = assign_positions(&ag);
        assert!(level.is_fully_connected(), "positioned level should be fully connected");
    }
}
