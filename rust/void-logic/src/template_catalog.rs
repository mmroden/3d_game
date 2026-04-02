use crate::room_template::{
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
        RoomTemplate {
            id: "scifi_room_tall",
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX },
                Connector { offset: [2, 0, 1], facing: ConnectorFacing::PosX },
                Connector { offset: [1, 0, 0], facing: ConnectorFacing::NegZ },
                Connector { offset: [1, 0, 2], facing: ConnectorFacing::PosZ },
                Connector { offset: [0, 1, 1], facing: ConnectorFacing::NegX },
                Connector { offset: [2, 1, 1], facing: ConnectorFacing::PosX },
                Connector { offset: [1, 1, 1], facing: ConnectorFacing::PosY },
                Connector { offset: [1, 0, 1], facing: ConnectorFacing::NegY },
            ],
            enemy_spawns: vec![
                SpawnPoint { position: [4.0, 0.0, 4.0] },
                SpawnPoint { position: [4.0, 5.0, 4.0] },
            ],
            loot_spawns: vec![SpawnPoint { position: [8.0, 2.5, 8.0] }],
            extents: [3, 2, 3],
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
        RoomTemplate {
            id: "scifi_corridor_vertical",
            kind: TemplateKind::Corridor,
            connectors: vec![
                Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegY },
                Connector { offset: [0, 1, 0], facing: ConnectorFacing::PosY },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [1, 2, 1],
        },
    ]
}

/// Walk a generated level graph, assemble room geometry, furnish rooms,
/// and return all mesh placements plus light sources for the level.
///
/// Each room gets a deterministic visual theme based on its index + seed.
pub fn spawn_list(
    graph: &crate::level_graph::LevelGraph,
    cell_size: f32,
    seed: u64,
) -> (
    Vec<crate::room_assembler::MeshPlacement>,
    Vec<crate::room_furnisher::LightSource>,
) {
    let (meshes, lights, _enemies, _colliders) = spawn_list_full(graph, cell_size, seed);
    (meshes, lights)
}

/// Like `spawn_list`, but also returns world-space enemy spawn positions.
pub fn spawn_list_full(
    graph: &crate::level_graph::LevelGraph,
    cell_size: f32,
    seed: u64,
) -> (
    Vec<crate::room_assembler::MeshPlacement>,
    Vec<crate::room_furnisher::LightSource>,
    Vec<[f32; 3]>,
    Vec<crate::room_assembler::CollisionBox>,
) {
    use crate::cell::CellGrid;
    use crate::room_furnisher;
    use crate::room_theme;

    let mut meshes = Vec::new();
    let mut lights = Vec::new();
    let mut enemy_positions = Vec::new();
    let mut colliders = Vec::new();

    for (room_idx, idx) in graph.room_indices().enumerate() {
        let Some(room) = graph.room(idx) else { continue };
        let active = graph.active_connectors(idx);
        // Per-room theme: deterministic from seed + room index.
        let theme = room_theme::theme_for_room(seed, room_idx);
        let story_height = theme.wall_set.story_height;
        let origin = room.world_position(cell_size, story_height);

        // Build cell grid and structural geometry.
        let mut grid = CellGrid::new(&room.template, &active, origin, cell_size);
        meshes.extend(crate::room_assembler::assemble_from_grid(
            &grid,
            &room.template,
            &active,
            theme.wall_set,
        ));

        // Collision boxes for walls, floors, ceilings.
        colliders.extend(crate::room_assembler::collision_boxes_from_grid(
            &grid,
            &room.template,
            &active,
            theme.wall_set,
        ));

        // Populate cells with themed props.
        let room_seed = seed.wrapping_add(room_idx as u64).wrapping_mul(2654435761);
        grid.populate(theme, room_seed);
        meshes.extend(grid.prop_placements());

        // Light fixtures (mesh + co-located light source).
        for (mesh, light) in room_furnisher::light_fixtures(&room.template, &active, origin, cell_size) {
            meshes.push(mesh);
            lights.push(light);
        }

        // Enemy spawn positions (skip first room so player doesn't spawn into enemies)
        if room_idx > 0 {
            for sp in &room.template.enemy_spawns {
                enemy_positions.push([
                    origin[0] + sp.position[0],
                    origin[1] + sp.position[1] + 1.5, // Hover height
                    origin[2] + sp.position[2],
                ]);
            }
        }
    }

    (meshes, lights, enemy_positions, colliders)
}

/// Return the world-space center of every cell in the level (for player spawn).
pub fn cell_centers(
    graph: &crate::level_graph::LevelGraph,
    cell_size: f32,
) -> Vec<[f32; 3]> {
    // Use WALL_SET_ASTRA story_height as default for cell center computation.
    let story_height = crate::asset_catalog::WALL_SET_ASTRA.story_height;
    graph
        .room_indices()
        .filter_map(|idx| {
            let room = graph.room(idx)?;
            let origin = room.world_position(cell_size, story_height);
            let [ex, ey, ez] = room.template.extents.map(|e| e as i32);
            let centers: Vec<_> = (0..ex).flat_map(|cx| {
                (0..ey).flat_map(move |cy| {
                    (0..ez).map(move |cz| [
                        origin[0] + (cx as f32 + 0.5) * cell_size,
                        origin[1] + cy as f32 * story_height,
                        origin[2] + (cz as f32 + 0.5) * cell_size,
                    ])
                })
            }).collect();
            Some(centers)
        })
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_catalog;

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
    fn catalog_rooms_have_vertical_connectors() {
        let rooms = room_templates();
        let has_posy = rooms.iter().any(|r|
            r.connectors.iter().any(|c| c.facing == ConnectorFacing::PosY)
        );
        let has_negy = rooms.iter().any(|r|
            r.connectors.iter().any(|c| c.facing == ConnectorFacing::NegY)
        );
        assert!(has_posy, "at least one room must have a PosY connector for vertical expansion");
        assert!(has_negy, "at least one room must have a NegY connector for vertical expansion");
    }

    #[test]
    fn generator_produces_multi_level_layout() {
        use crate::generator::{generate, GeneratorConfig};

        // With vertical connectors + vertical corridor, some seeds should produce
        // rooms at different Y levels.
        let mut has_vertical = false;
        for seed in 0..20 {
            let config = GeneratorConfig {
                seed,
                room_templates: room_templates(),
                corridor_templates: corridor_templates(),
                target_room_count: 5,
            };
            if let Ok(level) = generate(&config) {
                let y_values: std::collections::HashSet<i32> = level.room_indices()
                    .filter_map(|idx| level.room(idx).map(|r| r.grid_pos[1]))
                    .collect();
                if y_values.len() > 1 {
                    has_vertical = true;
                    break;
                }
            }
        }
        assert!(has_vertical, "at least one seed out of 20 should produce rooms at different Y levels");
    }

    #[test]
    fn corridors_cover_both_horizontal_axes() {
        use crate::room_template::ConnectorFacing;
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
        use crate::generator::{generate, GeneratorConfig};

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
        use crate::generator::{generate, GeneratorConfig};

        let config = GeneratorConfig {
            seed: 42,
            room_templates: room_templates(),
            corridor_templates: corridor_templates(),
            target_room_count: 3,
        };
        let level = generate(&config).expect("generation should succeed");
        let (placements, _lights) = spawn_list(&level, 4.0, 42);
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
        use crate::generator::{generate, GeneratorConfig};

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
        use crate::level_graph::LevelGraph;
        use crate::room_template::*;

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
        // Center of a 4m cell at grid origin [0,0,0]:
        // world_position = [0, 0, 0], cell center = world_position + [2, 0, 2]
        assert!(
            (centers[0][0] - 2.0).abs() < 0.001,
            "x should be at cell midpoint (2.0), got {}", centers[0][0]
        );
        assert!(
            (centers[0][2] - 2.0).abs() < 0.001,
            "z should be at cell midpoint (2.0), got {}", centers[0][2]
        );
    }

    // --- Multi-story templates ---

    #[test]
    fn catalog_has_at_least_one_multi_story_room() {
        let rooms = room_templates();
        let multi = rooms.iter().filter(|r| r.extents[1] > 1).count();
        assert!(multi >= 1, "expected at least 1 multi-story room template, got {multi}");
    }

    #[test]
    fn catalog_has_at_least_one_vertical_corridor() {
        let corridors = corridor_templates();
        let vertical = corridors.iter().filter(|c| {
            c.connectors.iter().any(|conn| {
                matches!(conn.facing, ConnectorFacing::PosY | ConnectorFacing::NegY)
            })
        }).count();
        assert!(vertical >= 1, "expected at least 1 vertical corridor, got {vertical}");
    }

    #[test]
    fn cell_centers_includes_y_levels_for_multi_story() {
        use crate::level_graph::LevelGraph;
        use crate::room_template::*;

        let mut graph = LevelGraph::new();
        let template = RoomTemplate {
            id: "test_2story",
            kind: TemplateKind::Room,
            connectors: vec![],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [2, 2, 2],
        };
        graph.place_room(template, [0, 0, 0]).unwrap();

        let centers = cell_centers(&graph, 4.0);
        // 2x2x2 = 8 cells
        assert_eq!(centers.len(), 8, "2x2x2 room should have 8 cell centers, got {}", centers.len());
        // At least some centers should have different Y values
        let y_values: std::collections::HashSet<i32> = centers.iter()
            .map(|c| (c[1] * 100.0) as i32)
            .collect();
        assert!(y_values.len() >= 2, "multi-story room should have centers at multiple Y levels, got {:?}", y_values);
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
        use crate::generator::{generate, GeneratorConfig};
        use crate::level_graph::EdgeKind;
        use crate::room_assembler;
        use crate::room_template::TemplateKind;

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
            if let EdgeKind::Adjacent { from_connector, to_connector } = edge {
                for (idx, conn) in [(from, from_connector), (to, to_connector)] {
                    let room = level.room(idx).unwrap();
                    let active = level.active_connectors(idx);
                    let ws = &asset_catalog::WALL_SET_ASTRA;
                    let story_height = ws.story_height;
                    let placements = room_assembler::assemble(
                        &room.template,
                        &active,
                        room.world_position(cell_size, story_height),
                        ws,
                    );

                    let facing = conn.facing;
                    // Y-axis connectors use hatches at floor/ceiling level,
                    // not vertical doors. Skip them here — tested by floor/ceiling coverage.
                    if matches!(facing, ConnectorFacing::PosY | ConnectorFacing::NegY) {
                        continue;
                    }
                    {
                        let origin = room.world_position(cell_size, story_height);
                        let cell_pos = [
                            origin[0] + (conn.offset[0] as f32 + 0.5) * cell_size,
                            origin[1] + conn.offset[1] as f32 * story_height,
                            origin[2] + (conn.offset[2] as f32 + 0.5) * cell_size,
                        ];

                        let at_pos = |p: &room_assembler::MeshPlacement| {
                            (p.position[0] - cell_pos[0]).abs() < 0.001
                                && (p.position[1] - cell_pos[1]).abs() < 0.001
                                && (p.position[2] - cell_pos[2]).abs() < 0.001
                        };
                        // Use the wall rotation for this facing to distinguish walls
                        // on this face from walls on perpendicular faces at the same cell.
                        let (_, expected_wall_rot) = room_assembler::wall_placement(
                            [0.0, 0.0, 0.0], facing
                        );
                        let has_door = placements.iter().any(|p| p.scene == door_scene && at_pos(p));
                        let has_wall_on_face = placements.iter().any(|p| {
                            p.scene == wall_scene && at_pos(p)
                                && (p.rotation_y - expected_wall_rot).abs() < 0.001
                        });

                        // Both rooms and corridors emit door frames at active
                        // XZ connectors, sealing the visual boundary.
                        assert!(
                            has_door,
                            "{kind} '{id}' at {pos:?} missing door frame at {cell_pos:?} for {facing:?}",
                            kind = if room.template.kind == TemplateKind::Corridor { "corridor" } else { "room" },
                            id = room.template.id,
                            pos = room.grid_pos
                        );
                        assert!(
                            !has_wall_on_face,
                            "{kind} '{id}' at {pos:?} has wall on active connector face at {cell_pos:?} for {facing:?}",
                            kind = if room.template.kind == TemplateKind::Corridor { "corridor" } else { "room" },
                            id = room.template.id,
                            pos = room.grid_pos
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn generated_level_every_room_has_floor_per_cell() {
        use crate::generator::{generate, GeneratorConfig};
        use crate::room_assembler;

        let config = GeneratorConfig {
            seed: 42,
            room_templates: room_templates(),
            corridor_templates: corridor_templates(),
            target_room_count: 10,
        };
        let level = generate(&config).expect("generation should succeed");
        let cell_size = 4.0;

        let floor_scene = "res://addons/quaternius/modularscifimegakit/platforms/Platform_Simple.gltf";
        let floor_curve_scene = "res://addons/quaternius/modularscifimegakit/platforms/Platform_Simple_Curve.gltf";

        let ws = &asset_catalog::WALL_SET_ASTRA;
        let story_height = ws.story_height;

        for idx in level.room_indices() {
            let room = level.room(idx).unwrap();
            let active = level.active_connectors(idx);
            let placements = room_assembler::assemble(
                &room.template,
                &active,
                room.world_position(cell_size, story_height),
                ws,
            );

            let door_scene = "res://addons/quaternius/modularscifimegakit/platforms/Door_Frame_Square.gltf";

            // Floor tiles at each XZ cell position.
            // Active NegY connectors leave a clean opening (no floor tile, no hatch).
            // Active PosY connectors leave a clean ceiling opening.
            let is_floor_piece = |p: &room_assembler::MeshPlacement| {
                let scene = p.scene;
                scene == floor_scene || scene == floor_curve_scene
            };

            // Count active NegY connectors (floor openings)
            let negy_openings = room.template.connectors.iter()
                .filter(|c| c.facing == ConnectorFacing::NegY && active.contains(c))
                .count();
            // Count active PosY connectors (ceiling openings)
            let posy_openings = room.template.connectors.iter()
                .filter(|c| c.facing == ConnectorFacing::PosY && active.contains(c))
                .count();

            let cell_count = (room.template.extents[0] * room.template.extents[2]) as usize;
            let expected_floors = cell_count - negy_openings;
            let floor_count = placements.iter().filter(|p| {
                is_floor_piece(p) && (p.position[1] - room.world_position(cell_size, story_height)[1]).abs() < 0.001
            }).count();
            assert_eq!(
                floor_count, expected_floors,
                "room '{}' at {:?} should have {} floor tiles ({} cells - {} NegY openings), got {}",
                room.template.id, room.grid_pos, expected_floors, cell_count, negy_openings, floor_count
            );

            // Ceiling tiles at room top: origin_y + extents_y * story_height
            let ey = room.template.extents[1] as f32;
            let ceiling_y = room.world_position(cell_size, story_height)[1] + ey * story_height;
            let expected_ceilings = cell_count - posy_openings;
            let ceiling_count = placements.iter().filter(|p| {
                is_floor_piece(p)
                    && (p.position[1] - ceiling_y).abs() < 0.001
            }).count();
            assert_eq!(
                ceiling_count, expected_ceilings,
                "room '{}' at {:?} should have {} ceiling tiles ({} cells - {} PosY openings), got {}",
                room.template.id, room.grid_pos, expected_ceilings, cell_count, posy_openings, ceiling_count
            );

            // Per-edge boundary coverage: every horizontal boundary edge must have
            // a wall or archway at the correct spatial position (not just Y).
            let origin = room.world_position(cell_size, story_height);
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
                        let is_active = active.iter().any(|c| {
                                c.facing == facing
                                    && c.offset[0] == cx
                                    && c.offset[2] == cz
                            });
                        let is_room = room.template.kind == crate::room_template::TemplateKind::Room;
                        if is_active && is_room {
                            // Room gap — no geometry expected here.
                            continue;
                        }

                        let (dp, _) = room_assembler::door_placement(cell_pos, facing);
                        // Check for any wall-like geometry near this cell.
                        // Corner pieces are offset up to 2.0m from cell center.
                        let has_wall_geometry = placements.iter().any(|p| {
                            (p.scene.contains("Wall") || p.scene.contains("Corner") || p.scene.contains("Curve"))
                                && (p.position[0] - cell_pos[0]).abs() < 2.1
                                && (p.position[1] - cell_pos[1]).abs() < 0.001
                                && (p.position[2] - cell_pos[2]).abs() < 2.1
                        });
                        let has_door = placements.iter().any(|p| {
                            p.scene == door_scene
                                && (p.position[0] - dp[0]).abs() < 0.001
                                && (p.position[1] - dp[1]).abs() < 0.001
                                && (p.position[2] - dp[2]).abs() < 0.001
                        });
                        assert!(
                            has_wall_geometry || has_door,
                            "room '{}' at {:?} cell ({cx},{cz}) face {facing:?}: \
                             no wall/corner geometry or door at {cell_pos:?}",
                            room.template.id, room.grid_pos
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn generated_level_no_wall_overlap_between_adjacent_rooms() {
        use crate::generator::{generate, GeneratorConfig};
        use crate::level_graph::EdgeKind;
        use crate::room_assembler;

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

                let ws = &asset_catalog::WALL_SET_ASTRA;
                let story_height = ws.story_height;
                let from_placements = room_assembler::assemble(
                    &from_room.template,
                    &level.active_connectors(from),
                    from_room.world_position(cell_size, story_height),
                    ws,
                );
                let to_placements = room_assembler::assemble(
                    &to_room.template,
                    &level.active_connectors(to),
                    to_room.world_position(cell_size, story_height),
                    ws,
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

    /// After wiring CellGrid::populate(), each room's props must come from
    /// that room's assigned theme palette — not just any palette.
    #[test]
    fn spawn_list_props_match_per_room_theme() {
        use crate::generator::{generate, GeneratorConfig};
        use crate::room_theme;

        let seed = 42u64;
        let config = GeneratorConfig {
            seed,
            room_templates: room_templates(),
            corridor_templates: corridor_templates(),
            target_room_count: 5,
        };
        let level = generate(&config).expect("generation should succeed");
        let cell_size = 4.0;
        let (all_placements, _lights) = spawn_list(&level, cell_size, seed);

        // Structural scenes from ALL wall sets (corridors inside rooms use different themes).
        let mut all_structural: std::collections::HashSet<&str> = std::collections::HashSet::new();
        all_structural.insert(crate::asset_catalog::DOOR);
        for ws in crate::asset_catalog::ALL_WALL_SETS {
            for triple in [&ws.straight, &ws.corner_inner, &ws.corner_outer] {
                all_structural.insert(triple.floor);
                all_structural.insert(triple.wall);
                all_structural.insert(triple.ceiling);
            }
            for layer in [&ws.short_wall, &ws.bottom] {
                all_structural.insert(layer.straight);
                all_structural.insert(layer.corner_inner);
                all_structural.insert(layer.corner_outer);
            }
        }
        // Light fixture scenes.
        let light_scenes: std::collections::HashSet<&str> =
            crate::asset_catalog::ALL_LIGHTS.iter().map(|l| l.scene).collect();

        // For each room, verify non-structural props belong to that theme's palette.
        for (room_idx, idx) in level.room_indices().enumerate() {
            let room = level.room(idx).unwrap();
            let theme = room_theme::theme_for_room(seed, room_idx);
            let story_height = theme.wall_set.story_height;
            let origin = room.world_position(cell_size, story_height);

            // Collect allowed prop scenes for this room's theme.
            let allowed: std::collections::HashSet<&str> = theme.palette.wall_adjacent.iter()
                .chain(theme.palette.center)
                .chain(theme.palette.corner)
                .chain(theme.palette.ceiling)
                .map(|p| p.scene)
                .collect();

            // Find placements that fall within this room's world bounds (XZ + Y).
            let ex = room.template.extents[0] as f32;
            let ey = room.template.extents[1] as f32;
            let ez = room.template.extents[2] as f32;
            let room_props: Vec<_> = all_placements.iter()
                .filter(|p| {
                    p.position[0] >= origin[0] && p.position[0] < origin[0] + ex * cell_size
                    && p.position[1] >= origin[1] - 0.1 && p.position[1] < origin[1] + ey * story_height + 0.1
                    && p.position[2] >= origin[2] && p.position[2] < origin[2] + ez * cell_size
                    && !all_structural.contains(p.scene)
                    && !light_scenes.contains(p.scene)
                })
                .collect();

            for p in &room_props {
                assert!(
                    allowed.contains(p.scene),
                    "room '{}' (theme '{}') has prop '{}' not in its palette",
                    room.template.id, theme.name, p.scene
                );
            }
        }
    }
}
