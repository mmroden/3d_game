use crate::systems::room_template::{
    Connector, ConnectorFacing, RoomTemplate, SpawnPoint, TemplateKind,
};

pub fn room_templates() -> Vec<RoomTemplate> {
    vec![
        RoomTemplate {
            id: "scifi_room_small",
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX },
                Connector { offset: [2, 0, 1], facing: ConnectorFacing::PosX },
                Connector { offset: [1, 0, 0], facing: ConnectorFacing::NegZ },
                Connector { offset: [1, 0, 2], facing: ConnectorFacing::PosZ },
            ],
            enemy_spawns: vec![SpawnPoint { position: [4.0, 0.0, 4.0] }],
            loot_spawns: vec![SpawnPoint { position: [8.0, 0.0, 8.0] }],
            extents: [3, 1, 3],
        },
        RoomTemplate {
            id: "scifi_room_large",
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 2], facing: ConnectorFacing::NegX },
                Connector { offset: [4, 0, 2], facing: ConnectorFacing::PosX },
                Connector { offset: [2, 0, 0], facing: ConnectorFacing::NegZ },
                Connector { offset: [2, 0, 4], facing: ConnectorFacing::PosZ },
            ],
            enemy_spawns: vec![
                SpawnPoint { position: [4.0, 0.0, 4.0] },
                SpawnPoint { position: [12.0, 0.0, 12.0] },
            ],
            loot_spawns: vec![SpawnPoint { position: [8.0, 0.0, 8.0] }],
            extents: [5, 1, 5],
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
                        origin[0] + (cx as f32 + 0.5) * cell_size,
                        origin[1],
                        origin[2] + (cz as f32 + 0.5) * cell_size,
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

    #[test]
    fn cell_centers_are_at_cell_midpoints() {
        use crate::systems::level_graph::LevelGraph;
        use crate::systems::room_template::*;

        // Place a single 1x1x1 room at grid [0,0,0]
        let mut graph = LevelGraph::new();
        let template = RoomTemplate {
            id: "test_1x1",
            kind: TemplateKind::Room,
            connectors: vec![],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 1, 1],
        };
        graph.place_room(template, [0, 0, 0]).unwrap();

        let centers = cell_centers(&graph, 4.0);
        assert_eq!(centers.len(), 1);
        // Center of a 4m cell at origin should be [2, 0, 2], not [0, 0, 0]
        assert_eq!(centers[0][0], 2.0, "x should be at cell midpoint");
        assert_eq!(centers[0][2], 2.0, "z should be at cell midpoint");
    }

    // --- R1: Minimum room size ---

    #[test]
    fn all_room_templates_at_least_3x3() {
        for tmpl in room_templates() {
            assert!(
                tmpl.extents[0] >= 3 && tmpl.extents[2] >= 3,
                "room template '{}' is {}x{}, minimum is 3x3",
                tmpl.id, tmpl.extents[0], tmpl.extents[2]
            );
        }
    }

    // --- R9: Full integration — generated level geometry validation ---

    #[test]
    fn generated_level_apertures_correct_by_kind() {
        use crate::systems::generator::{generate, GeneratorConfig};
        use crate::systems::level_graph::EdgeKind;
        use crate::systems::room_assembler;
        use crate::systems::room_template::TemplateKind;

        let config = GeneratorConfig {
            seed: 42,
            room_templates: room_templates(),
            corridor_templates: corridor_templates(),
            target_room_count: 10,
        };
        let level = generate(&config).expect("generation should succeed");
        let cell_size = 4.0;
        let door_scene = "res://addons/quaternius/modularscifimegakit/platforms/Door_Frame_Square.gltf";
        let wall_scene = "res://addons/quaternius/modularscifimegakit/walls/WallAstra_Straight.gltf";

        // For each Adjacent edge, verify:
        //   - Corridors have archway geometry at the connector cell
        //   - Rooms have a gap (no wall AND no door) at the connector cell
        for (from, to, edge) in level.edges() {
            if let EdgeKind::Adjacent { from_facing, to_facing } = edge {
                for (idx, facing) in [(from, from_facing), (to, to_facing)] {
                    let room = level.room(idx).unwrap();
                    let active = level.active_facings(idx);
                    let placements = room_assembler::assemble(
                        &room.template,
                        &active,
                        room.world_position(cell_size),
                        cell_size,
                    );

                    if let Some(conn) = room.template.connectors.iter().find(|c| c.facing == *facing) {
                        let origin = room.world_position(cell_size);
                        let cell_pos = [
                            origin[0] + (conn.offset[0] as f32 + 0.5) * cell_size,
                            origin[1] + conn.offset[1] as f32 * cell_size,
                            origin[2] + (conn.offset[2] as f32 + 0.5) * cell_size,
                        ];

                        let at_pos = |p: &room_assembler::MeshPlacement| {
                            (p.position[0] - cell_pos[0]).abs() < 0.001
                                && (p.position[1] - cell_pos[1]).abs() < 0.001
                                && (p.position[2] - cell_pos[2]).abs() < 0.001
                        };
                        let has_door = placements.iter().any(|p| p.scene == door_scene && at_pos(p));
                        let has_wall = placements.iter().any(|p| p.scene == wall_scene && at_pos(p));

                        if room.template.kind == TemplateKind::Corridor {
                            assert!(
                                has_door,
                                "corridor '{}' at {:?} missing archway at {cell_pos:?} for {facing:?}",
                                room.template.id, room.grid_pos
                            );
                        } else {
                            assert!(
                                !has_door && !has_wall,
                                "room '{}' at {:?} should have gap at {cell_pos:?} for {facing:?}, \
                                 found wall={has_wall} door={has_door}",
                                room.template.id, room.grid_pos
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn generated_level_every_room_has_floor_per_cell() {
        use crate::systems::generator::{generate, GeneratorConfig};
        use crate::systems::room_assembler;

        let config = GeneratorConfig {
            seed: 42,
            room_templates: room_templates(),
            corridor_templates: corridor_templates(),
            target_room_count: 10,
        };
        let level = generate(&config).expect("generation should succeed");
        let cell_size = 4.0;

        let floor_scene = "res://addons/quaternius/modularscifimegakit/platforms/Platform_Simple.gltf";
        let ceiling_scene = floor_scene; // same asset, placed at ceiling height

        for idx in level.room_indices() {
            let room = level.room(idx).unwrap();
            let active = level.active_facings(idx);
            let placements = room_assembler::assemble(
                &room.template,
                &active,
                room.world_position(cell_size),
                cell_size,
            );

            let cell_count = (room.template.extents[0] * room.template.extents[2]) as usize;
            let floor_count = placements.iter().filter(|p| {
                p.scene == floor_scene && p.position[1] == room.world_position(cell_size)[1]
            }).count();
            assert_eq!(
                floor_count, cell_count,
                "room '{}' at {:?} should have {} floors, got {}",
                room.template.id, room.grid_pos, cell_count, floor_count
            );

            // Ceiling tiles at y + CELL_HEIGHT (mesh-native vertical cell size)
            let ceiling_count = placements.iter().filter(|p| {
                p.scene == ceiling_scene
                    && (p.position[1] - (room.world_position(cell_size)[1] + crate::systems::room_assembler::CELL_HEIGHT)).abs() < 0.001
            }).count();
            assert_eq!(
                ceiling_count, cell_count,
                "room '{}' at {:?} should have {} ceiling tiles, got {}",
                room.template.id, room.grid_pos, cell_count, ceiling_count
            );

            // Per-edge boundary coverage: every horizontal boundary edge must have
            // a wall or archway at the correct spatial position (not just Y).
            let wall_scene = "res://addons/quaternius/modularscifimegakit/walls/WallAstra_Straight.gltf";
            let door_scene = "res://addons/quaternius/modularscifimegakit/platforms/Door_Frame_Square.gltf";
            let origin = room.world_position(cell_size);
            let ex = room.template.extents[0] as i32;
            let ez = room.template.extents[2] as i32;

            for cx in 0..ex {
                for cz in 0..ez {
                    let cell_pos = [
                        origin[0] + (cx as f32 + 0.5) * cell_size,
                        origin[1],
                        origin[2] + (cz as f32 + 0.5) * cell_size,
                    ];

                    let faces = [
                        (ConnectorFacing::NegX, cx == 0),
                        (ConnectorFacing::PosX, cx == ex - 1),
                        (ConnectorFacing::NegZ, cz == 0),
                        (ConnectorFacing::PosZ, cz == ez - 1),
                    ];

                    for (facing, is_boundary) in faces {
                        if !is_boundary {
                            continue;
                        }

                        // Rooms with active connectors leave a gap (corridor
                        // provides the archway). Only check for geometry on
                        // sealed boundaries or corridor connectors.
                        let is_active = active.contains(&facing)
                            && room.template.connectors.iter().any(|c| {
                                c.facing == facing
                                    && c.offset[0] == cx
                                    && c.offset[1] == 0
                                    && c.offset[2] == cz
                            });
                        let is_room = room.template.kind == crate::systems::room_template::TemplateKind::Room;
                        if is_active && is_room {
                            // Room gap — no geometry expected here.
                            continue;
                        }

                        let (wp, wr) = room_assembler::wall_placement(cell_pos, facing, cell_size);
                        let (dp, _) = room_assembler::door_placement(cell_pos, facing, cell_size);
                        let has_wall = placements.iter().any(|p| {
                            p.scene == wall_scene
                                && (p.position[0] - wp[0]).abs() < 0.001
                                && (p.position[1] - wp[1]).abs() < 0.001
                                && (p.position[2] - wp[2]).abs() < 0.001
                                && (p.rotation_y - wr).abs() < 0.001
                        });
                        let has_door = placements.iter().any(|p| {
                            p.scene == door_scene
                                && (p.position[0] - dp[0]).abs() < 0.001
                                && (p.position[1] - dp[1]).abs() < 0.001
                                && (p.position[2] - dp[2]).abs() < 0.001
                        });
                        assert!(
                            has_wall || has_door,
                            "room '{}' at {:?} cell ({cx},{cz}) face {facing:?}: \
                             no wall at {wp:?} rot {wr} or door at {dp:?}",
                            room.template.id, room.grid_pos
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn generated_level_no_wall_overlap_between_adjacent_rooms() {
        use crate::systems::generator::{generate, GeneratorConfig};
        use crate::systems::level_graph::EdgeKind;
        use crate::systems::room_assembler;

        let config = GeneratorConfig {
            seed: 42,
            room_templates: room_templates(),
            corridor_templates: corridor_templates(),
            target_room_count: 10,
        };
        let level = generate(&config).expect("generation should succeed");
        let cell_size = 4.0;

        let wall_scene = "res://addons/quaternius/modularscifimegakit/walls/WallAstra_Straight.gltf";

        for (from, to, edge) in level.edges() {
            if let EdgeKind::Adjacent { .. } = edge {
                let from_room = level.room(from).unwrap();
                let to_room = level.room(to).unwrap();

                let from_placements = room_assembler::assemble(
                    &from_room.template,
                    &level.active_facings(from),
                    from_room.world_position(cell_size),
                    cell_size,
                );
                let to_placements = room_assembler::assemble(
                    &to_room.template,
                    &level.active_facings(to),
                    to_room.world_position(cell_size),
                    cell_size,
                );

                // Two walls overlap if same position AND same rotation
                // (same position with different rotation = perpendicular walls at a shared corner, which is fine)
                let walls_from: Vec<([f32; 3], i32)> = from_placements.iter()
                    .filter(|p| p.scene == wall_scene)
                    .map(|p| (p.position, (p.rotation_y * 1000.0) as i32))
                    .collect();
                let walls_to: Vec<([f32; 3], i32)> = to_placements.iter()
                    .filter(|p| p.scene == wall_scene)
                    .map(|p| (p.position, (p.rotation_y * 1000.0) as i32))
                    .collect();

                for (pf, rf) in &walls_from {
                    for (pt, rt) in &walls_to {
                        assert!(
                            pf != pt || rf != rt,
                            "wall overlap between '{}' and '{}' at {:?}",
                            from_room.template.id, to_room.template.id, pf
                        );
                    }
                }
            }
        }
    }
}
