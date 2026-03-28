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

pub fn scene_path(template_id: &str) -> Option<&'static str> {
    match template_id {
        "scifi_room_small" => Some("res://scenes/rooms/room_small.tscn"),
        "scifi_room_large" => Some("res://scenes/rooms/room_large.tscn"),
        "scifi_corridor_ew" => Some("res://scenes/corridors/corridor_ew.tscn"),
        "scifi_corridor_ns" => Some("res://scenes/corridors/corridor_ns.tscn"),
        _ => None,
    }
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
    fn every_template_has_a_scene_path() {
        let all: Vec<_> = room_templates()
            .into_iter()
            .chain(corridor_templates())
            .collect();
        for t in &all {
            assert!(
                scene_path(t.id).is_some(),
                "template '{}' has no scene path mapping",
                t.id
            );
        }
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
}
