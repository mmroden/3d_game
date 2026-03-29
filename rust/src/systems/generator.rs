use rand::prelude::IndexedRandom;
use rand::prelude::SliceRandom;
use rand::rngs::SmallRng;
use rand::SeedableRng;

use crate::systems::level_graph::LevelGraph;
use crate::systems::room_template::RoomTemplate;

/// Configuration for level generation.
pub struct GeneratorConfig {
    pub seed: u64,
    /// Room templates to choose from.
    pub room_templates: Vec<RoomTemplate>,
    /// Corridor templates used to connect rooms.
    pub corridor_templates: Vec<RoomTemplate>,
    /// Desired number of rooms (not counting corridors).
    pub target_room_count: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum GenerateError {
    NoRoomTemplates,
    NoCorridorTemplates,
    ZeroTargetRooms,
    IncompatibleTemplates,
}

/// Generate a level from a seed and template pool.
pub fn generate(config: &GeneratorConfig) -> Result<LevelGraph, GenerateError> {
    if config.room_templates.is_empty() {
        return Err(GenerateError::NoRoomTemplates);
    }
    if config.corridor_templates.is_empty() {
        return Err(GenerateError::NoCorridorTemplates);
    }
    if config.target_room_count == 0 {
        return Err(GenerateError::ZeroTargetRooms);
    }

    let mut rng = SmallRng::seed_from_u64(config.seed);
    let mut graph = LevelGraph::new();

    let first_template = config.room_templates.choose(&mut rng)
        .expect("already validated non-empty")
        .clone();
    let first_idx = graph.place_room(first_template, [0, 0, 0])
        .expect("origin placement cannot overlap");

    let mut frontier = vec![first_idx];
    let mut rooms_placed: usize = 1;

    while rooms_placed < config.target_room_count {
        if frontier.is_empty() {
            return Err(GenerateError::IncompatibleTemplates);
        }

        let &source_idx = frontier.choose(&mut rng)
            .expect("already checked non-empty");
        let source = graph.room(source_idx)
            .expect("frontier contains valid indices");
        let source_origin = source.grid_pos;
        let source_connectors = source.template.connectors.clone();

        // Try each connector on the source in random order
        let mut connector_order: Vec<usize> = (0..source_connectors.len()).collect();
        connector_order.shuffle(&mut rng);

        let mut placed = false;
        for ci in connector_order {
            let connector = &source_connectors[ci];
            let target_pos = connector.target_cell(source_origin);

            if !graph.is_free(target_pos) {
                continue;
            }

            // Find a corridor template that can mate with this connector
            let needed_facing = connector.facing.opposite();
            let corridor_candidates: Vec<_> = config.corridor_templates.iter()
                .filter(|t| t.connectors.iter().any(|c| c.facing == needed_facing))
                .collect();

            let Some(&corridor_template) = corridor_candidates.choose(&mut rng) else {
                continue;
            };

            // Place the corridor
            let Ok(corridor_idx) = graph.place_room(corridor_template.clone(), target_pos) else {
                continue;
            };
            let _ = graph.connect_adjacent(source_idx, corridor_idx);

            // Find the corridor's outgoing connector (not the one facing back to source)
            let corridor_connectors = corridor_template.connectors.clone();
            let outgoing = corridor_connectors.iter()
                .find(|c| c.facing != needed_facing);

            let Some(out_connector) = outgoing else {
                // Corridor only faces one way — dead end, remove it
                continue;
            };

            let target_cell = out_connector.target_cell(target_pos);

            // Find a room template that mates with the corridor's outgoing connector
            let room_needed = out_connector.facing.opposite();
            let room_candidates: Vec<_> = config.room_templates.iter()
                .filter(|t| t.connectors.iter().any(|c| c.facing == room_needed))
                .collect();

            if let Some(&room_template) = room_candidates.choose(&mut rng) {
                // Find the matching connector on the room to compute the correct origin.
                // The room origin = target_cell - connector.offset so the connector
                // aligns with the corridor's outgoing target.
                let matching_connector = room_template.connectors.iter()
                    .find(|c| c.facing == room_needed);
                let Some(mc) = matching_connector else { continue };
                let room_pos = [
                    target_cell[0] - mc.offset[0],
                    target_cell[1] - mc.offset[1],
                    target_cell[2] - mc.offset[2],
                ];

                if let Ok(new_idx) = graph.place_room(room_template.clone(), room_pos) {
                    let _ = graph.connect_adjacent(corridor_idx, new_idx);
                    frontier.push(new_idx);
                    rooms_placed += 1;
                    placed = true;
                    break;
                }
            }
        }

        if !placed {
            // Remove exhausted node from frontier
            frontier.retain(|&idx| idx != source_idx);
        }
    }

    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::room_template::*;

    // --- Fixtures ---

    fn basic_room() -> RoomTemplate {
        RoomTemplate {
            id: "room_4way",
            kind: TemplateKind::Room,
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

    fn corridor_ew() -> RoomTemplate {
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

    fn corridor_ns() -> RoomTemplate {
        RoomTemplate {
            id: "corridor_ns",
            kind: TemplateKind::Corridor,
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

    fn count_by_kind(level: &LevelGraph, kind: TemplateKind) -> usize {
        level.room_indices()
            .filter(|&idx| {
                level.room(idx)
                    .map(|r| r.template.kind == kind)
                    .unwrap_or(false)
            })
            .count()
    }

    fn layout_snapshot(level: &LevelGraph) -> Vec<(&'static str, [i32; 3])> {
        let mut snap: Vec<_> = level.room_indices()
            .filter_map(|idx| {
                let r = level.room(idx)?;
                Some((r.template.id, r.grid_pos))
            })
            .collect();
        snap.sort_by_key(|(_, pos)| *pos);
        snap
    }

    // --- Error path tests ---

    #[test]
    fn empty_room_templates_returns_error() {
        let config = GeneratorConfig {
            seed: 42,
            room_templates: vec![],
            corridor_templates: vec![corridor_ew()],
            target_room_count: 5,
        };
        let result = generate(&config);
        assert!(
            matches!(result, Err(GenerateError::NoRoomTemplates)),
            "expected NoRoomTemplates, got {result:?}"
        );
    }

    #[test]
    fn empty_corridor_templates_returns_error() {
        let config = GeneratorConfig {
            seed: 42,
            room_templates: vec![basic_room()],
            corridor_templates: vec![],
            target_room_count: 5,
        };
        let result = generate(&config);
        assert!(
            matches!(result, Err(GenerateError::NoCorridorTemplates)),
            "expected NoCorridorTemplates, got {result:?}"
        );
    }

    #[test]
    fn zero_target_returns_error() {
        let result = generate(&basic_config(42, 0));
        assert!(
            matches!(result, Err(GenerateError::ZeroTargetRooms)),
            "expected ZeroTargetRooms, got {result:?}"
        );
    }

    // --- Happy path, built incrementally ---

    #[test]
    fn generate_target_one_returns_exactly_one_room() {
        let level = generate(&basic_config(42, 1)).expect("generation should succeed");
        let rooms = count_by_kind(&level, TemplateKind::Room);
        assert_eq!(rooms, 1, "expected exactly 1 room, got {rooms}");
    }

    // STEP 2: second room — forces growth logic
    #[test]
    fn generate_target_two_returns_two_rooms() {
        let level = generate(&basic_config(42, 2)).expect("generation should succeed");
        let rooms = count_by_kind(&level, TemplateKind::Room);
        assert_eq!(rooms, 2, "expected 2 rooms, got {rooms}");
    }

    // STEP 3: all rooms must be reachable
    #[test]
    fn all_rooms_reachable() {
        let level = generate(&basic_config(42, 3)).expect("generation should succeed");
        assert!(
            level.is_fully_connected(),
            "generated level should be fully connected"
        );
    }

    // STEP 4: rooms connected through corridors, not directly
    #[test]
    fn rooms_connected_through_corridors() {
        let level = generate(&basic_config(42, 3)).expect("generation should succeed");
        let corridors = count_by_kind(&level, TemplateKind::Corridor);
        let rooms = count_by_kind(&level, TemplateKind::Room);
        assert_eq!(rooms, 3, "expected 3 rooms, got {rooms}");
        // Each room beyond the first needs a corridor to connect it
        assert!(
            corridors >= rooms - 1,
            "expected at least {} corridors, got {corridors}",
            rooms - 1
        );
    }

    // STEP 5: no orphaned nodes — graph is connected at larger scale
    #[test]
    fn no_orphaned_nodes_at_scale() {
        let level = generate(&basic_config(42, 8)).expect("generation should succeed");
        let rooms = count_by_kind(&level, TemplateKind::Room);
        assert_eq!(rooms, 8, "expected 8 rooms, got {rooms}");
        assert!(
            level.is_fully_connected(),
            "generated level with 8 rooms should be fully connected"
        );
    }

    // STEP 7: determinism — same seed produces same layout
    #[test]
    fn deterministic_with_same_seed() {
        let config = basic_config(99, 5);
        let level_a = generate(&config).expect("generation should succeed");
        let level_b = generate(&config).expect("generation should succeed");
        let snap_a = layout_snapshot(&level_a);
        let snap_b = layout_snapshot(&level_b);
        assert_eq!(
            snap_a, snap_b,
            "same seed should produce identical layouts"
        );
    }

    // STEP 8: different seeds produce different layouts
    #[test]
    fn different_seeds_differ() {
        let level_a = generate(&basic_config(1, 5)).expect("generation should succeed");
        let level_b = generate(&basic_config(2, 5)).expect("generation should succeed");
        let snap_a = layout_snapshot(&level_a);
        let snap_b = layout_snapshot(&level_b);
        assert_ne!(
            snap_a, snap_b,
            "different seeds should produce different layouts"
        );
    }

    // STEP 9: incompatible templates — corridors can't mate with rooms
    #[test]
    fn incompatible_templates_returns_error() {
        // Room only has PosY/NegY connectors, corridor only has PosX/NegX.
        // They can never mate.
        let room = RoomTemplate {
            id: "room_up_down",
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosY },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegY },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 1, 1],
        };
        let config = GeneratorConfig {
            seed: 42,
            room_templates: vec![room],
            corridor_templates: vec![corridor_ew()],
            target_room_count: 3,
        };
        let result = generate(&config);
        assert!(
            matches!(result, Err(GenerateError::IncompatibleTemplates)),
            "expected IncompatibleTemplates, got {result:?}"
        );
    }
}
