//! Portal position logic for level exit placement.

use crate::level_graph::LevelGraph;
use crate::planet::Pitch;

/// Returns the world-space center of the farthest room from the start as the
/// portal position, at hover height (1.5m above floor). In a boss arena the
/// portal shifts to the BACK third — away from the doorway, past the fight —
/// because the boss's reward cache drops at the corpse (usually mid-arena)
/// and a center portal would arm straight into the collecting player.
pub fn portal_position(graph: &LevelGraph, pitch: Pitch) -> Option<[f32; 3]> {
    let first_idx = graph.room_indices().next()?;
    let farthest_idx = graph.exit_room(first_idx)?;
    let room = graph.room(farthest_idx)?;
    let origin = room.world_position(pitch.tile, pitch.story);
    let ex = room.template.extents[0] as f32;
    let ez = room.template.extents[2] as f32;
    let mut pos = [
        origin[0] + (ex * pitch.tile) / 2.0,
        origin[1] + 1.5, // hover height
        origin[2] + (ez * pitch.tile) / 2.0,
    ];
    if graph.boss_room() == Some(farthest_idx) {
        if let Some(conn) = graph.active_connectors(farthest_idx).first() {
            let dir = conn.facing.grid_offset();
            pos[0] -= dir[0] as f32 * ex * pitch.tile / 3.0;
            pos[2] -= dir[2] as f32 * ez * pitch.tile / 3.0;
        }
    }
    Some(pos)
}

#[cfg(test)]
mod tests {
    /// Attribute spec for tests: fresh profile, pinned run seed. The
    /// GENERATION seed still travels separately.
    fn spec_for(level: u32) -> crate::level_spec::LevelSpec {
        crate::level_spec::LevelSpec::for_level(
            crate::roster::roster(),
            crate::seed::Seed::new(1),
            level,
            &crate::unlocks::PermanentUnlocks::new(),
        )
    }

    fn config_for(seed: u64, rooms: usize, level: u32) -> crate::generator::GeneratorConfig {
        let mut spec = spec_for(level);
        spec.room_budget = rooms;
        crate::generator::GeneratorConfig::for_spec(&spec, crate::seed::Seed::new(seed))
    }

    /// The planet-1 pitch, spelled out: tests may hold literals.
    const TEST_PITCH: crate::planet::Pitch =
        crate::planet::Pitch { tile: 4.0, story: 5.0 };

    use super::*;
    use crate::generator::{generate, rooms_for_level};

    use crate::spatial_layout::attach_boss_room;

    const CELL: f32 = 4.0;

    fn pinned_boss_graph() -> LevelGraph {
        let config = config_for(1, rooms_for_level(3), 3);
        let mut graph = generate(&config).expect("pinned seed generates");
        let entry = graph.room_indices().next().expect("has rooms");
        attach_boss_room(&mut graph, entry, TEST_PITCH).expect("arena attaches");
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
        let story = 5.0; // planet-1 story — tests may hold literals
        let origin = room.world_position(CELL, story);
        let [ex, _ey, ez] = room.template.extents;
        let center_x = origin[0] + ex as f32 * CELL / 2.0;
        let center_z = origin[2] + ez as f32 * CELL / 2.0;

        let portal = portal_position(&graph, TEST_PITCH).expect("portal placed");

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
    fn the_portal_rides_the_arena_even_on_farthest_ties() {
        // The fixed fixture's den (arena) and closet tie at two hops from
        // the porch. The linker only requires the arena to be AMONG the
        // farthest; the portal must not re-derive its own tie-break and
        // land in the closet (live bug: level 13's portal in the bath).
        let grammar = crate::test_fixtures::fixed_fixture_grammar();
        let spec = crate::level_spec::LevelSpec::for_level(
            &grammar,
            crate::seed::Seed::new(1),
            1,
            &crate::unlocks::PermanentUnlocks::new(),
        );
        let graph = crate::generator::generate_for_spec(&spec, crate::seed::Seed::new(1), false)
            .expect("the fixture builds");
        let arena = graph.boss_room().expect("fixed levels mark the arena");
        let room = graph.room(arena).expect("arena room");
        let origin = room.world_position(spec.pitch.tile, spec.pitch.story);
        let portal = portal_position(&graph, spec.pitch).expect("portal placed");
        for k in [0, 2] {
            let lo = origin[k];
            let hi = origin[k] + room.template.extents[k] as f32 * spec.pitch.tile;
            assert!(
                portal[k] >= lo && portal[k] <= hi,
                "the way onward lies in the arena: portal {portal:?} vs arena \
                 [{lo}, {hi}] on axis {k}"
            );
        }
    }

    #[test]
    fn regular_levels_keep_the_center_portal() {
        let config = config_for(1, rooms_for_level(1), 1);
        let graph = generate(&config).expect("generates");
        let first = graph.room_indices().next().expect("rooms");
        let far = graph.farthest_room_from(first).expect("farthest");
        let room = graph.room(far).expect("room");
        let story = 5.0; // planet-1 story — tests may hold literals
        let origin = room.world_position(CELL, story);
        let [ex, _ey, ez] = room.template.extents;
        let portal = portal_position(&graph, TEST_PITCH).expect("portal placed");
        assert!((portal[0] - (origin[0] + ex as f32 * CELL / 2.0)).abs() < 1e-4);
        assert!((portal[2] - (origin[2] + ez as f32 * CELL / 2.0)).abs() < 1e-4);
    }
}
