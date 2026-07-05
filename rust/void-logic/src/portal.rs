//! Portal position logic for level exit placement.

use crate::level_graph::LevelGraph;

/// Returns the world-space center of the farthest room from the start as the
/// portal position, at hover height (1.5m above floor). In a boss arena the
/// portal shifts to the BACK third — away from the doorway, past the fight —
/// because the boss's reward cache drops at the corpse (usually mid-arena)
/// and a center portal would arm straight into the collecting player.
pub fn portal_position(graph: &LevelGraph, cell_size: f32) -> Option<[f32; 3]> {
    let first_idx = graph.room_indices().next()?;
    let farthest_idx = graph.farthest_room_from(first_idx)?;
    let room = graph.room(farthest_idx)?;
    let story_height = crate::asset_catalog::WALL_SET_ASTRA.story_height;
    let origin = room.world_position(cell_size, story_height);
    let ex = room.template.extents[0] as f32;
    let ez = room.template.extents[2] as f32;
    let mut pos = [
        origin[0] + (ex * cell_size) / 2.0,
        origin[1] + 1.5, // hover height
        origin[2] + (ez * cell_size) / 2.0,
    ];
    if graph.boss_room() == Some(farthest_idx) {
        if let Some(conn) = graph.active_connectors(farthest_idx).first() {
            let dir = conn.facing.grid_offset();
            pos[0] -= dir[0] as f32 * ex * cell_size / 3.0;
            pos[2] -= dir[2] as f32 * ez * cell_size / 3.0;
        }
    }
    Some(pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{generate, rooms_for_level, GeneratorConfig};
    use crate::seed::Seed;
    use crate::spatial_layout::attach_boss_room;

    const CELL: f32 = 4.0;

    fn pinned_boss_graph() -> LevelGraph {
        let config = GeneratorConfig::standard(Seed::new(1), rooms_for_level(3));
        let mut graph = generate(&config).expect("pinned seed generates");
        let entry = graph.room_indices().next().expect("has rooms");
        attach_boss_room(&mut graph, entry).expect("arena attaches");
        graph
    }

    #[test]
    fn the_boss_portal_opens_at_the_back_of_the_arena() {
        // The boss's reward cache drops at the corpse — usually mid-arena,
        // where a center-placed portal would arm straight into the player
        // and hurl them onward the moment they collect. The way onward
        // opens at the BACK instead: away from the doorway, past the fight.
        let graph = pinned_boss_graph();
        let boss_idx = graph.boss_room().expect("marked");
        let room = graph.room(boss_idx).expect("arena");
        let story = crate::asset_catalog::WALL_SET_ASTRA.story_height;
        let origin = room.world_position(CELL, story);
        let [ex, _ey, ez] = room.template.extents;
        let center_x = origin[0] + ex as f32 * CELL / 2.0;
        let center_z = origin[2] + ez as f32 * CELL / 2.0;

        let portal = portal_position(&graph, CELL).expect("portal placed");

        let off_center = (portal[0] - center_x).abs() + (portal[2] - center_z).abs();
        assert!(off_center > CELL,
            "the arena portal must sit well off the center (got offset {off_center})");
        // Still safely inside the arena walls.
        assert!(portal[0] > origin[0] + CELL * 0.5
            && portal[0] < origin[0] + ex as f32 * CELL - CELL * 0.5,
            "x inside the arena");
        assert!(portal[2] > origin[2] + CELL * 0.5
            && portal[2] < origin[2] + ez as f32 * CELL - CELL * 0.5,
            "z inside the arena");

        // And specifically AWAY from the doorway: farther from the wired
        // connector than the center is.
        let conn = graph.active_connectors(boss_idx)[0];
        let door_x = origin[0] + (conn.offset[0] as f32 + 0.5) * CELL;
        let door_z = origin[2] + (conn.offset[2] as f32 + 0.5) * CELL;
        let center_dist = ((center_x - door_x).powi(2) + (center_z - door_z).powi(2)).sqrt();
        let portal_dist = ((portal[0] - door_x).powi(2) + (portal[2] - door_z).powi(2)).sqrt();
        assert!(portal_dist > center_dist,
            "the way onward lies past the fight, not toward the entrance");
    }

    #[test]
    fn regular_levels_keep_the_center_portal() {
        let config = GeneratorConfig::standard(Seed::new(1), rooms_for_level(1));
        let graph = generate(&config).expect("generates");
        let first = graph.room_indices().next().expect("rooms");
        let far = graph.farthest_room_from(first).expect("farthest");
        let room = graph.room(far).expect("room");
        let story = crate::asset_catalog::WALL_SET_ASTRA.story_height;
        let origin = room.world_position(CELL, story);
        let [ex, _ey, ez] = room.template.extents;
        let portal = portal_position(&graph, CELL).expect("portal placed");
        assert!((portal[0] - (origin[0] + ex as f32 * CELL / 2.0)).abs() < 1e-4);
        assert!((portal[2] - (origin[2] + ez as f32 * CELL / 2.0)).abs() < 1e-4);
    }
}
