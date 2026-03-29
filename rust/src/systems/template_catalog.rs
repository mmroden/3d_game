use crate::systems::room_template::{
    Connector, ConnectorFacing, RoomTemplate, SpawnPoint, TemplateKind,
};

pub fn room_templates() -> Vec<RoomTemplate> {
    vec![
        RoomTemplate {
            id: "scifi_room_small",
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
            ],
            enemy_spawns: vec![SpawnPoint { position: [0.0, 0.0, 0.0] }],
            loot_spawns: vec![SpawnPoint { position: [2.0, 0.0, 2.0] }],
            extents: [1, 1, 1],
        },
        RoomTemplate {
            id: "scifi_room_large",
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
                Connector { offset: [1, 0, 0], facing: ConnectorFacing::PosX },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
                Connector { offset: [0, 0, 1], facing: ConnectorFacing::PosZ },
            ],
            enemy_spawns: vec![
                SpawnPoint { position: [2.0, 0.0, 2.0] },
                SpawnPoint { position: [6.0, 0.0, 6.0] },
            ],
            loot_spawns: vec![SpawnPoint { position: [4.0, 0.0, 4.0] }],
            extents: [2, 1, 2],
        },
    ]
}

pub fn corridor_templates() -> Vec<RoomTemplate> {
    vec![
        RoomTemplate {
            id: "scifi_corridor_ew",
            kind: TemplateKind::Corridor,
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 1, 1],
        },
        RoomTemplate {
            id: "scifi_corridor_ns",
            kind: TemplateKind::Corridor,
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 1, 1],
        },
    ]
}

/// Walk a generated level graph, assemble room geometry using the cell-grid
/// room assembler, and return all mesh placements for the level.
pub fn spawn_list(
    graph: &crate::systems::level_graph::LevelGraph,
    cell_size: f32,
) -> Vec<crate::systems::room_assembler::MeshPlacement> {
    graph
        .room_indices()
        .filter_map(|idx| {
            let room = graph.room(idx)?;
            let active = graph.active_facings(idx);
            Some(crate::systems::room_assembler::assemble(
                &room.template,
                &active,
                room.world_position(cell_size),
                cell_size,
            ))
        })
        .flatten()
        .collect()
}

/// Return the world-space center of every cell in the level (for lighting).
pub fn cell_centers(
    graph: &crate::systems::level_graph::LevelGraph,
    cell_size: f32,
) -> Vec<[f32; 3]> {
    graph
        .room_indices()
        .filter_map(|idx| {
            let room = graph.room(idx)?;
            let origin = room.world_position(cell_size);
            let ex = room.template.extents[0] as i32;
            let ez = room.template.extents[2] as i32;
            let mut centers = Vec::new();
            for cx in 0..ex {
                for cz in 0..ez {
                    centers.push([
                        origin[0] + cx as f32 * cell_size,
                        origin[1],
                        origin[2] + cz as f32 * cell_size,
                    ]);
                }
            }
            Some(centers)
        })
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_at_least_one_corridor_template() {
        let corridors = corridor_templates();
        assert!(
            !corridors.is_empty(),
            "expected at least 1 corridor template, got 0"
        );
    }

    #[test]
    fn catalog_has_at_least_two_room_templates() {
        let rooms = room_templates();
        assert!(
            rooms.len() >= 2,
            "expected at least 2 room templates, got {}",
            rooms.len()
        );
    }

    #[test]
    fn corridors_cover_both_horizontal_axes() {
        use crate::systems::room_template::ConnectorFacing;
        let corridors = corridor_templates();
        let has_ew = corridors.iter().any(|c|
            c.connectors.iter().any(|conn| conn.facing == ConnectorFacing::PosX)
        );
        let has_ns = corridors.iter().any(|c|
            c.connectors.iter().any(|conn| conn.facing == ConnectorFacing::PosZ)
        );
        assert!(has_ew, "catalog needs at least one east-west corridor");
        assert!(has_ns, "catalog needs at least one north-south corridor");
    }

    #[test]
    fn generator_succeeds_with_catalog_templates() {
        use crate::systems::generator::{generate, GeneratorConfig};

        let mut successes = 0;
        for seed in 0..10 {
            let config = GeneratorConfig {
                seed,
                room_templates: room_templates(),
                corridor_templates: corridor_templates(),
                target_room_count: 5,
            };
            if let Ok(level) = generate(&config) {
                assert!(
                    level.is_fully_connected(),
                    "seed {seed}: level should be fully connected"
                );
                successes += 1;
            }
        }
        assert!(
            successes >= 8,
            "expected at least 8 out of 10 seeds to succeed, got {successes}"
        );
    }

#[test]
    fn spawn_list_produces_mesh_placements() {
        use crate::systems::generator::{generate, GeneratorConfig};

        let config = GeneratorConfig {
            seed: 42,
            room_templates: room_templates(),
            corridor_templates: corridor_templates(),
            target_room_count: 3,
        };
        let level = generate(&config).expect("generation should succeed");
        let placements = spawn_list(&level, 4.0);
        // Each room/corridor cell produces at least 1 floor + walls
        assert!(
            placements.len() > level.room_count(),
            "expected more placements ({}) than rooms ({})",
            placements.len(),
            level.room_count()
        );
    }

    #[test]
    fn cell_centers_covers_all_cells() {
        use crate::systems::generator::{generate, GeneratorConfig};

        let config = GeneratorConfig {
            seed: 42,
            room_templates: room_templates(),
            corridor_templates: corridor_templates(),
            target_room_count: 3,
        };
        let level = generate(&config).expect("generation should succeed");
        let centers = cell_centers(&level, 4.0);
        assert!(
            !centers.is_empty(),
            "cell_centers should return at least one center"
        );
    }
}
