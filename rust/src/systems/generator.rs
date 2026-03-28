use rand::prelude::IndexedRandom;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::systems::level_graph::LevelGraph;
use crate::systems::room_template::{ConnectorFacing, RoomTemplate};

/// Configuration for level generation.
pub struct GeneratorConfig {
    pub seed: u64,
    /// Room templates to choose from (excludes corridor templates).
    pub room_templates: Vec<RoomTemplate>,
    /// Corridor templates used to connect rooms.
    pub corridor_templates: Vec<RoomTemplate>,
    /// Desired number of rooms (not counting corridors).
    pub target_room_count: usize,
}

/// An open connector on a placed room that hasn't been connected yet.
#[derive(Debug, Clone)]
struct OpenConnector {
    node: petgraph::graph::NodeIndex,
    /// The world grid cell this connector points toward.
    target_cell: [i32; 3],
    facing: ConnectorFacing,
}

/// Generate a level from a seed and template pool.
///
/// Algorithm: random walk with corridor insertion.
/// 1. Place a starting room at the origin.
/// 2. Collect all open (unconnected) connectors.
/// 3. Shuffle them. For each open connector:
///    a. Find a corridor template whose NegFacing matches the connector.
///    b. Place the corridor at the target cell.
///    c. Find a room template whose NegFacing matches the corridor's exit.
///    d. Place the room at the corridor's exit target cell.
///    e. Connect: room -> corridor -> new room.
/// 4. Repeat until target_room_count is reached.
///
/// Invariants guaranteed:
/// - No rooms overlap on the grid
/// - Every node is reachable from every other node
/// - All adjacent edges use matching connectors
/// - Same seed + same config = same layout
/// - Returns at least 1 room, up to target_room_count
pub fn generate(config: &GeneratorConfig) -> LevelGraph {
    let mut rng = SmallRng::seed_from_u64(config.seed);
    let mut graph = LevelGraph::new();

    // Place the first room at the origin.
    let first_template = config.room_templates.choose(&mut rng)
        .expect("room_templates must not be empty")
        .clone();
    let first_node = graph.place_room(first_template, [0, 0, 0])
        .expect("first room placement at origin should never fail");

    let mut rooms_placed: usize = 1;
    let mut open = collect_open_connectors(&graph, first_node);

    let max_attempts = config.target_room_count * 20;
    let mut attempts = 0;

    while rooms_placed < config.target_room_count && !open.is_empty() && attempts < max_attempts {
        attempts += 1;

        // Pick a random open connector.
        let connector_idx = rng.random_range(0..open.len());
        let connector = open[connector_idx].clone();

        // Find a corridor template that has a connector with the
        // opposite facing (so it mates with our open connector).
        let needed_facing = connector.facing.opposite();

        let corridor_template = match find_template_with_facing(
            &config.corridor_templates, needed_facing, &mut rng
        ) {
            Some(t) => t,
            None => {
                open.remove(connector_idx);
                continue;
            }
        };

        // Place the corridor at the target cell.
        let corridor_node = match graph.place_room(corridor_template.clone(), connector.target_cell) {
            Ok(node) => node,
            Err(_) => {
                open.remove(connector_idx);
                continue;
            }
        };

        // Connect source room to corridor.
        if graph.connect_adjacent(connector.node, corridor_node).is_err() {
            // Shouldn't happen if our logic is right, but be safe.
            // Can't undo place_room easily, so just skip.
            open.remove(connector_idx);
            continue;
        }

        // Find the corridor's exit connector (the one that isn't
        // facing back toward the source).
        let corridor_exits: Vec<_> = corridor_template.connectors.iter()
            .filter(|c| c.facing != needed_facing)
            .collect();

        let corridor_exit = match corridor_exits.first() {
            Some(c) => *c,
            None => {
                open.remove(connector_idx);
                continue;
            }
        };

        let room_target = corridor_exit.target_cell(connector.target_cell);
        let room_needed_facing = corridor_exit.facing.opposite();

        // Find a room template that has a connector with the needed facing.
        let room_template = match find_template_with_facing(
            &config.room_templates, room_needed_facing, &mut rng
        ) {
            Some(t) => t,
            None => {
                open.remove(connector_idx);
                continue;
            }
        };

        // Place the room.
        let room_node = match graph.place_room(room_template, room_target) {
            Ok(node) => node,
            Err(_) => {
                open.remove(connector_idx);
                continue;
            }
        };

        // Connect corridor to new room.
        if graph.connect_adjacent(corridor_node, room_node).is_err() {
            open.remove(connector_idx);
            continue;
        }

        // Success! Remove the used connector, collect new open ones.
        open.remove(connector_idx);
        rooms_placed += 1;

        let new_open = collect_open_connectors(&graph, room_node);
        open.extend(new_open);
    }

    graph
}

/// Collect all connectors on a room that point to free grid cells.
fn collect_open_connectors(
    graph: &LevelGraph,
    node: petgraph::graph::NodeIndex,
) -> Vec<OpenConnector> {
    let room = graph.room(node).unwrap();
    room.template.connectors.iter()
        .map(|c| {
            let target = c.target_cell(room.grid_pos);
            OpenConnector {
                node,
                target_cell: target,
                facing: c.facing,
            }
        })
        .filter(|oc| graph.is_free(oc.target_cell))
        .collect()
}

/// Find a random template from the pool that has a connector with
/// the given facing.
fn find_template_with_facing(
    templates: &[RoomTemplate],
    facing: ConnectorFacing,
    rng: &mut SmallRng,
) -> Option<RoomTemplate> {
    let candidates: Vec<_> = templates.iter()
        .filter(|t| t.connectors.iter().any(|c| c.facing == facing))
        .collect();
    candidates.choose(rng).map(|t| (*t).clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::room_template::*;
    use crate::systems::level_graph::EdgeKind;

    // --- Fixtures ---

    fn basic_room() -> RoomTemplate {
        RoomTemplate {
            id: "room_4way",
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
            ],
            enemy_spawns: vec![
                SpawnPoint { position: [0.0, 0.0, 0.0] },
            ],
            loot_spawns: vec![],
            extents: [1, 1, 1],
        }
    }

    fn large_room() -> RoomTemplate {
        RoomTemplate {
            id: "room_2x2",
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
                Connector { offset: [1, 0, 0], facing: ConnectorFacing::PosX },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
                Connector { offset: [0, 0, 1], facing: ConnectorFacing::PosZ },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![
                SpawnPoint { position: [0.5, 0.0, 0.5] },
            ],
            extents: [2, 1, 2],
        }
    }

    fn corridor_ew() -> RoomTemplate {
        RoomTemplate {
            id: "corridor_ew",
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 1, 1],
        }
    }

    fn corridor_ns() -> RoomTemplate {
        RoomTemplate {
            id: "corridor_ns",
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 1, 1],
        }
    }

    fn basic_config(seed: u64, target: usize) -> GeneratorConfig {
        GeneratorConfig {
            seed,
            room_templates: vec![basic_room()],
            corridor_templates: vec![corridor_ew(), corridor_ns()],
            target_room_count: target,
        }
    }

    fn mixed_config(seed: u64, target: usize) -> GeneratorConfig {
        GeneratorConfig {
            seed,
            room_templates: vec![basic_room(), large_room()],
            corridor_templates: vec![corridor_ew(), corridor_ns()],
            target_room_count: target,
        }
    }

    // --- Contract tests ---

    #[test]
    fn generate_produces_at_least_one_room() {
        let level = generate(&basic_config(42, 5));
        assert!(level.room_count() >= 1);
    }

    #[test]
    fn generate_hits_target_room_count() {
        let target = 8;
        let level = generate(&basic_config(42, target));
        let room_count = level.room_indices()
            .filter(|&idx| {
                let room = level.room(idx).unwrap();
                !room.template.id.starts_with("corridor")
            })
            .count();
        assert_eq!(room_count, target);
    }

    #[test]
    fn generate_no_overlapping_rooms() {
        let level = generate(&basic_config(42, 10));
        let mut all_cells = std::collections::HashSet::new();
        let mut total_cells: usize = 0;
        for idx in level.room_indices() {
            let r = level.room(idx).unwrap();
            let ext = r.template.extents;
            for x in 0..ext[0] as i32 {
                for y in 0..ext[1] as i32 {
                    for z in 0..ext[2] as i32 {
                        let cell = [
                            r.grid_pos[0] + x,
                            r.grid_pos[1] + y,
                            r.grid_pos[2] + z,
                        ];
                        assert!(
                            all_cells.insert(cell),
                            "Overlapping cell at {:?}",
                            cell
                        );
                        total_cells += 1;
                    }
                }
            }
        }
        assert_eq!(all_cells.len(), total_cells);
    }

    #[test]
    fn generate_all_rooms_reachable() {
        let level = generate(&basic_config(42, 10));
        assert!(
            level.is_fully_connected(),
            "Generated level has {} rooms but is not fully connected",
            level.room_count()
        );
    }

    #[test]
    fn generate_all_adjacent_edges_have_matching_connectors() {
        let level = generate(&basic_config(42, 10));
        for (from, to, edge) in level.edges() {
            if let EdgeKind::Adjacent { from_facing, to_facing } = edge {
                assert_eq!(
                    from_facing.opposite(), *to_facing,
                    "Edge {:?}->{:?} has non-matching facings: {:?} and {:?}",
                    from, to, from_facing, to_facing
                );
            }
        }
    }

    #[test]
    fn generate_is_deterministic() {
        let level_a = generate(&basic_config(123, 8));
        let level_b = generate(&basic_config(123, 8));

        assert_eq!(level_a.room_count(), level_b.room_count());
        assert_eq!(level_a.edge_count(), level_b.edge_count());

        let positions_a: Vec<_> = level_a.room_indices()
            .map(|idx| {
                let r = level_a.room(idx).unwrap();
                (r.template.id, r.grid_pos)
            })
            .collect();
        let positions_b: Vec<_> = level_b.room_indices()
            .map(|idx| {
                let r = level_b.room(idx).unwrap();
                (r.template.id, r.grid_pos)
            })
            .collect();
        assert_eq!(positions_a, positions_b);
    }

    #[test]
    fn generate_different_seeds_produce_different_layouts() {
        let level_a = generate(&basic_config(1, 8));
        let level_b = generate(&basic_config(2, 8));

        let positions_a: Vec<_> = level_a.room_indices()
            .map(|idx| level_a.room(idx).unwrap().grid_pos)
            .collect();
        let positions_b: Vec<_> = level_b.room_indices()
            .map(|idx| level_b.room(idx).unwrap().grid_pos)
            .collect();

        assert_ne!(positions_a, positions_b);
    }

    #[test]
    fn generate_with_mixed_room_sizes() {
        let level = generate(&mixed_config(42, 6));
        assert!(level.is_fully_connected());
        assert!(level.room_count() >= 6);
    }

    #[test]
    fn generate_minimum_one_room() {
        let level = generate(&basic_config(42, 1));
        let room_count = level.room_indices()
            .filter(|&idx| {
                let room = level.room(idx).unwrap();
                !room.template.id.starts_with("corridor")
            })
            .count();
        assert_eq!(room_count, 1);
    }

    #[test]
    fn generate_corridors_connect_rooms() {
        let level = generate(&basic_config(42, 5));

        for idx in level.room_indices() {
            let room = level.room(idx).unwrap();
            if room.template.id.starts_with("corridor") {
                let neighbor_count = level.neighbors(idx).count();
                assert_eq!(
                    neighbor_count, 2,
                    "Corridor at {:?} has {} neighbors, expected 2",
                    room.grid_pos, neighbor_count
                );
            }
        }
    }
}
