//! Level assembly: builds meshes, lights, enemies, and collision boxes from a LevelGraph.

use crate::level_graph::LevelGraph;
use crate::seed::Seed;
use crate::room_assembler::{CollisionBox, MeshPlacement};
use crate::room_furnisher::LightSource;

/// Walk a generated level graph, assemble room geometry, furnish rooms,
/// and return all mesh placements plus light sources for the level.
pub fn spawn_list(
    graph: &LevelGraph,
    cell_size: f32,
    seed: Seed,
) -> (Vec<MeshPlacement>, Vec<LightSource>) {
    let (meshes, lights, _enemies, _colliders) = spawn_list_full(graph, cell_size, seed);
    (meshes, lights)
}

/// Like `spawn_list`, but also returns world-space enemy spawn positions
/// and collision boxes.
pub fn spawn_list_full(
    graph: &LevelGraph,
    cell_size: f32,
    seed: Seed,
) -> (Vec<MeshPlacement>, Vec<LightSource>, Vec<[f32; 3]>, Vec<CollisionBox>) {
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
        let theme = room_theme::theme_for_room(seed.value(), room_idx);
        let story_height = theme.wall_set.story_height;
        let origin = room.world_position(cell_size, story_height);

        let mut grid = CellGrid::new(&room.template, &active, origin, cell_size);
        meshes.extend(crate::room_assembler::assemble_from_grid(
            &grid,
            &room.template,
            &active,
            theme.wall_set,
        ));

        colliders.extend(crate::room_assembler::collision_boxes_from_grid(
            &grid,
            &room.template,
            &active,
            theme.wall_set,
        ));

        let room_seed = seed.value().wrapping_add(room_idx as u64).wrapping_mul(2654435761);
        grid.populate(theme, room_seed);
        meshes.extend(grid.prop_placements());

        for (mesh, light) in room_furnisher::light_fixtures(&room.template, &active, origin, cell_size) {
            meshes.push(mesh);
            lights.push(light);
        }

        if room_idx > 0 {
            for sp in &room.template.enemy_spawns {
                enemy_positions.push([
                    origin[0] + sp.position[0],
                    origin[1] + sp.position[1] + 1.5,
                    origin[2] + sp.position[2],
                ]);
            }
        }
    }

    (meshes, lights, enemy_positions, colliders)
}

/// Return the world-space center of every cell in the level (for player spawn).
pub fn cell_centers(
    graph: &LevelGraph,
    cell_size: f32,
) -> Vec<[f32; 3]> {
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
    use crate::generator::{generate, GeneratorConfig};
    use crate::level_graph::EdgeKind;
    use crate::room_template::ConnectorFacing;
    use crate::seed::Seed;

    /// A vertical passage's full-cell aperture must be unobstructed:
    /// no mesh placement may sit inside the hole column near the
    /// interface plane. Players report a ~1x1 m visible opening where
    /// the model intends 4x4 — this test either names the encroaching
    /// mesh or proves the obstruction is not a placed mesh.
    #[test]
    fn vertical_passages_are_unobstructed() {
        let cell = 4.0_f32;
        let story = crate::asset_catalog::WALL_SET_ASTRA.story_height;
        let mut passages_checked = 0u32;

        for seed in 0..30u64 {
            let config = GeneratorConfig {
                seed: Seed::new(seed),
                max_rooms: 20,
                min_room_xz: 3,
                max_room_xz: 6,
                min_room_y: 1,
                max_room_y: 6,
            };
            let Ok(graph) = generate(&config) else { continue };
            let (meshes, _lights) = spawn_list(&graph, cell, Seed::new(seed));

            for (a, _b, kind) in graph.edges() {
                let EdgeKind::Adjacent { from_connector, .. } = kind else {
                    continue;
                };
                if !matches!(
                    from_connector.facing,
                    ConnectorFacing::PosY | ConnectorFacing::NegY
                ) {
                    continue;
                }
                let Some(room) = graph.room(a) else { continue };
                let origin = room.world_position(cell, story);
                let hole_x = origin[0] + (from_connector.offset[0] as f32 + 0.5) * cell;
                let hole_z = origin[2] + (from_connector.offset[2] as f32 + 0.5) * cell;
                let plane_y = match from_connector.facing {
                    ConnectorFacing::PosY => {
                        origin[1] + (from_connector.offset[1] as f32 + 1.0) * story
                    }
                    _ => origin[1] + from_connector.offset[1] as f32 * story,
                };
                passages_checked += 1;

                for placement in &meshes {
                    let dx = (placement.position[0] - hole_x).abs();
                    let dz = (placement.position[2] - hole_z).abs();
                    let dy = (placement.position[1] - plane_y).abs();
                    // Strictly inside the hole column (rim tiles pivot
                    // at neighboring cell centers, 4 m away).
                    assert!(
                        !(dx < 1.9 && dz < 1.9 && dy < 1.0),
                        "seed {seed}: '{}' at {:?} obstructs the vertical passage at \
                         [{hole_x:.1}, {plane_y:.1}, {hole_z:.1}] (dx {dx:.2}, dy {dy:.2}, dz {dz:.2})",
                        placement.scene,
                        placement.position
                    );
                }
            }
        }
        assert!(
            passages_checked > 0,
            "no vertical passages generated across 30 seeds — widen the search"
        );
    }
}
