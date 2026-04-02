//! Portal position logic for level exit placement.

use crate::level_graph::LevelGraph;

/// Returns the world-space center of the last room as the portal position.
/// The portal is placed at hover height (1.5m above floor).
pub fn portal_position(graph: &LevelGraph, cell_size: f32) -> Option<[f32; 3]> {
    let room_indices: Vec<_> = graph.room_indices().collect();
    let last_idx = *room_indices.last()?;
    let room = graph.room(last_idx)?;
    let origin = room.world_position(cell_size);
    let ex = room.template.extents[0] as f32;
    let ez = room.template.extents[2] as f32;
    Some([
        origin[0] + (ex * cell_size) / 2.0,
        origin[1] + 1.5, // hover height
        origin[2] + (ez * cell_size) / 2.0,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{self, GeneratorConfig};
    use crate::template_catalog;

    fn make_test_level(seed: u64) -> LevelGraph {
        let config = GeneratorConfig {
            seed,
            room_templates: template_catalog::room_templates(),
            corridor_templates: template_catalog::corridor_templates(),
            target_room_count: 3,
        };
        generator::generate(&config).unwrap()
    }

    #[test]
    fn portal_position_is_some_for_generated_level() {
        let graph = make_test_level(42);
        let pos = portal_position(&graph, 4.0);
        assert!(pos.is_some());
    }

    #[test]
    fn portal_position_is_in_last_room() {
        let graph = make_test_level(42);
        let cell_size = 4.0;
        let pos = portal_position(&graph, cell_size).unwrap();
        // Should have positive coordinates (all rooms placed in positive space)
        assert!(pos[0] > 0.0);
        assert!(pos[2] > 0.0);
        // Should be at hover height above the last room's origin Y
        let room_indices: Vec<_> = graph.room_indices().collect();
        let last_room = graph.room(*room_indices.last().unwrap()).unwrap();
        let expected_y = last_room.world_position(cell_size)[1] + 1.5;
        assert!((pos[1] - expected_y).abs() < 0.01,
            "portal Y should be room origin Y + 1.5, expected {expected_y}, got {}", pos[1]);
    }

    #[test]
    fn portal_not_at_player_spawn() {
        let graph = make_test_level(42);
        let cell_size = 4.0;
        let portal_pos = portal_position(&graph, cell_size).unwrap();
        // Player spawns in first room center
        let centers = template_catalog::cell_centers(&graph, cell_size);
        let player_spawn = centers[0];
        // Portal should be in a different room, so positions differ
        let dist = ((portal_pos[0] - player_spawn[0]).powi(2)
            + (portal_pos[2] - player_spawn[2]).powi(2))
        .sqrt();
        assert!(dist > cell_size, "portal too close to player spawn: {}", dist);
    }
}
