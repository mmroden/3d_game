use rand::rngs::SmallRng;
use rand::SeedableRng;

use crate::abstract_graph;
use crate::level_graph::LevelGraph;
use crate::room_template::{Connector, ConnectorFacing, RoomTemplate, SpawnPoint, TemplateKind};
use crate::spatial_layout;

/// Configuration for level generation.
pub struct GeneratorConfig {
    pub seed: u64,
    /// Maximum number of rooms. If 0, defaults to 200.
    pub max_rooms: usize,
    /// Minimum XZ extent for generated rooms.
    pub min_room_xz: u32,
    /// Maximum XZ extent for generated rooms.
    pub max_room_xz: u32,
    /// Minimum Y extent (stories) for generated rooms.
    pub min_room_y: u32,
    /// Maximum Y extent (stories) for generated rooms.
    pub max_room_y: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum GenerateError {
    Empty,
}

/// Generate a room shape procedurally with random extents and auto-computed connectors.
pub(crate) fn generate_room(rng: &mut SmallRng, config: &GeneratorConfig) -> RoomTemplate {
    use rand::RngExt;

    let ex = rng.random_range(config.min_room_xz..=config.max_room_xz);
    let ey = rng.random_range(config.min_room_y..=config.max_room_y);
    let ez = rng.random_range(config.min_room_xz..=config.max_room_xz);

    let connectors = auto_connectors(ex, ey, ez, rng);
    let enemy_spawns = auto_enemy_spawns(ex, ey, ez, rng);

    RoomTemplate {
        kind: TemplateKind::Room,
        connectors,
        enemy_spawns,
        loot_spawns: vec![],
        extents: [ex, ey, ez],
    }
}

/// Compute connectors for a room with the given extents.
fn auto_connectors(ex: u32, ey: u32, ez: u32, rng: &mut SmallRng) -> Vec<Connector> {
    use ConnectorFacing::*;

    let mid_x = (ex as i32) / 2;
    let mid_z = (ez as i32) / 2;
    let mut connectors = vec![
        Connector { offset: [0, 0, mid_z], facing: NegX },
        Connector { offset: [ex as i32 - 1, 0, mid_z], facing: PosX },
        Connector { offset: [mid_x, 0, 0], facing: NegZ },
        Connector { offset: [mid_x, 0, ez as i32 - 1], facing: PosZ },
    ];

    // All rooms get vertical connectors.
    connectors.push(Connector { offset: [mid_x, (ey as i32) - 1, mid_z], facing: PosY });
    connectors.push(Connector { offset: [mid_x, 0, mid_z], facing: NegY });

    // Multi-story rooms also get upper-story horizontal connectors.
    if ey > 1 {
        let top_y = (ey as i32) - 1;
        connectors.push(Connector { offset: [0, top_y, mid_z], facing: NegX });
        connectors.push(Connector { offset: [ex as i32 - 1, top_y, mid_z], facing: PosX });
    }

    // Extra connectors on larger faces.
    let _ = rng;
    if ex >= 5 {
        let q1 = (ez as i32) / 4;
        let q3 = (ez as i32) * 3 / 4;
        if q1 != mid_z {
            connectors.push(Connector { offset: [0, 0, q1], facing: NegX });
            connectors.push(Connector { offset: [ex as i32 - 1, 0, q1], facing: PosX });
        }
        if q3 != mid_z {
            connectors.push(Connector { offset: [0, 0, q3], facing: NegX });
            connectors.push(Connector { offset: [ex as i32 - 1, 0, q3], facing: PosX });
        }
    }
    if ez >= 5 {
        let q1 = (ex as i32) / 4;
        let q3 = (ex as i32) * 3 / 4;
        if q1 != mid_x {
            connectors.push(Connector { offset: [q1, 0, 0], facing: NegZ });
            connectors.push(Connector { offset: [q1, 0, ez as i32 - 1], facing: PosZ });
        }
        if q3 != mid_x {
            connectors.push(Connector { offset: [q3, 0, 0], facing: NegZ });
            connectors.push(Connector { offset: [q3, 0, ez as i32 - 1], facing: PosZ });
        }
    }

    connectors
}

/// Generate enemy spawn points at random interior positions.
fn auto_enemy_spawns(ex: u32, ey: u32, ez: u32, rng: &mut SmallRng) -> Vec<SpawnPoint> {
    use rand::RngExt;

    let cell_size = 4.0_f32;
    let story_height = 5.0_f32;
    let count = rng.random_range(1..=3u32);
    (0..count).map(|_| {
        let x = rng.random_range(1.0..(ex as f32 - 1.0).max(1.5)) * cell_size;
        let y = rng.random_range(0.0..ey as f32) * story_height;
        let z = rng.random_range(1.0..(ez as f32 - 1.0).max(1.5)) * cell_size;
        SpawnPoint { position: [x, y, z] }
    }).collect()
}

/// Compute the target room count for a given level number.
/// Level 1 starts at 15 rooms, each subsequent level adds 5.
pub fn rooms_for_level(level: u32) -> usize {
    let level = level.max(1);
    10 + level as usize * 5
}

/// Generate a level using the sweep pipeline:
///   Sweep 1: Build abstract topology (rooms + edges, no positions)
///   Sweep 2: Assign spatial positions (rooms placed, corridors generated)
pub fn generate(config: &GeneratorConfig) -> Result<LevelGraph, GenerateError> {
    let room_count = if config.max_rooms > 0 { config.max_rooms } else { 200 };

    let mut rng = SmallRng::seed_from_u64(config.seed);

    // Sweep 1: topology.
    let abstract_graph = abstract_graph::generate_topology(&mut rng, room_count, config);

    // Sweep 2: spatial layout.
    let level = spatial_layout::assign_positions(&abstract_graph);

    if level.room_count() == 0 {
        return Err(GenerateError::Empty);
    }

    Ok(level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room_template::*;

    fn test_config(seed: u64) -> GeneratorConfig {
        GeneratorConfig {
            seed,
            max_rooms: 10,
            min_room_xz: 3,
            max_room_xz: 6,
            min_room_y: 1,
            max_room_y: 6,
        }
    }

    // --- Procedural room shape tests ---

    #[test]
    fn generated_rooms_have_varied_extents() {
        let mut rng = SmallRng::seed_from_u64(42);
        let config = test_config(42);
        let mut extent_set = std::collections::HashSet::new();
        for _ in 0..20 {
            let room = generate_room(&mut rng, &config);
            extent_set.insert(room.extents);
        }
        assert!(extent_set.len() >= 3);
    }

    #[test]
    fn generated_room_extents_within_bounds() {
        let mut rng = SmallRng::seed_from_u64(42);
        let config = test_config(42);
        for _ in 0..50 {
            let room = generate_room(&mut rng, &config);
            assert!(room.extents[0] >= config.min_room_xz && room.extents[0] <= config.max_room_xz);
            assert!(room.extents[1] >= config.min_room_y && room.extents[1] <= config.max_room_y);
            assert!(room.extents[2] >= config.min_room_xz && room.extents[2] <= config.max_room_xz);
        }
    }

    #[test]
    fn generated_room_has_at_least_four_connectors() {
        let mut rng = SmallRng::seed_from_u64(42);
        let config = test_config(42);
        for _ in 0..50 {
            let room = generate_room(&mut rng, &config);
            assert!(room.connectors.len() >= 4);
        }
    }

    #[test]
    fn connectors_face_outward_from_boundary() {
        let mut rng = SmallRng::seed_from_u64(42);
        let config = test_config(42);
        for _ in 0..50 {
            let room = generate_room(&mut rng, &config);
            for c in &room.connectors {
                match c.facing {
                    ConnectorFacing::NegX => assert_eq!(c.offset[0], 0),
                    ConnectorFacing::PosX => assert_eq!(c.offset[0], room.extents[0] as i32 - 1),
                    ConnectorFacing::NegZ => assert_eq!(c.offset[2], 0),
                    ConnectorFacing::PosZ => assert_eq!(c.offset[2], room.extents[2] as i32 - 1),
                    ConnectorFacing::PosY => assert_eq!(c.offset[1], room.extents[1] as i32 - 1),
                    ConnectorFacing::NegY => assert_eq!(c.offset[1], 0),
                }
            }
        }
    }

    #[test]
    fn connectors_cover_all_four_xz_faces() {
        let mut rng = SmallRng::seed_from_u64(42);
        let config = test_config(42);
        for _ in 0..50 {
            let room = generate_room(&mut rng, &config);
            let has_neg_x = room.connectors.iter().any(|c| c.facing == ConnectorFacing::NegX);
            let has_pos_x = room.connectors.iter().any(|c| c.facing == ConnectorFacing::PosX);
            let has_neg_z = room.connectors.iter().any(|c| c.facing == ConnectorFacing::NegZ);
            let has_pos_z = room.connectors.iter().any(|c| c.facing == ConnectorFacing::PosZ);
            assert!(has_neg_x && has_pos_x && has_neg_z && has_pos_z);
        }
    }

    #[test]
    fn multi_story_rooms_have_vertical_connectors() {
        let mut rng = SmallRng::seed_from_u64(42);
        let config = GeneratorConfig {
            min_room_y: 2,
            max_room_y: 6,
            ..test_config(42)
        };
        for _ in 0..50 {
            let room = generate_room(&mut rng, &config);
            let has_pos_y = room.connectors.iter().any(|c| c.facing == ConnectorFacing::PosY);
            let has_neg_y = room.connectors.iter().any(|c| c.facing == ConnectorFacing::NegY);
            assert!(has_pos_y && has_neg_y);
        }
    }

    #[test]
    fn generated_room_has_enemy_spawns() {
        let mut rng = SmallRng::seed_from_u64(42);
        let config = test_config(42);
        for _ in 0..50 {
            let room = generate_room(&mut rng, &config);
            assert!(!room.enemy_spawns.is_empty());
        }
    }

    #[test]
    fn generated_room_is_tagged_as_room() {
        let mut rng = SmallRng::seed_from_u64(42);
        let config = test_config(42);
        let room = generate_room(&mut rng, &config);
        assert_eq!(room.kind, TemplateKind::Room);
    }

    // --- rooms_for_level tests ---

    #[test]
    fn rooms_for_level_starts_at_15() {
        assert_eq!(rooms_for_level(1), 15);
    }

    #[test]
    fn rooms_for_level_increases_by_5() {
        assert_eq!(rooms_for_level(2), 20);
        assert_eq!(rooms_for_level(3), 25);
        assert_eq!(rooms_for_level(4), 30);
    }

    #[test]
    fn rooms_for_level_zero_clamps_to_one() {
        assert_eq!(rooms_for_level(0), 15);
    }

    // --- Level generation tests ---

    #[test]
    fn generate_produces_connected_level() {
        let config = test_config(42);
        let level = generate(&config).expect("generation should succeed");
        assert!(level.is_fully_connected());
    }

    #[test]
    fn generate_places_multiple_rooms() {
        let config = test_config(42);
        let level = generate(&config).expect("generation should succeed");
        let rooms = level.room_indices()
            .filter(|&idx| level.room(idx).map(|r| r.template.kind == TemplateKind::Room).unwrap_or(false))
            .count();
        assert!(rooms >= 3, "expected ≥3 rooms, got {rooms}");
    }

    #[test]
    fn generate_is_deterministic() {
        let config = test_config(42);
        let level_a = generate(&config).expect("gen a");
        let level_b = generate(&config).expect("gen b");
        let positions_a: Vec<_> = level_a.room_indices()
            .filter_map(|idx| level_a.room(idx).map(|r| r.grid_pos))
            .collect();
        let positions_b: Vec<_> = level_b.room_indices()
            .filter_map(|idx| level_b.room(idx).map(|r| r.grid_pos))
            .collect();
        assert_eq!(positions_a, positions_b);
    }

    #[test]
    fn generate_always_succeeds() {
        for seed in 0..20 {
            let config = test_config(seed);
            assert!(generate(&config).is_ok(), "seed {seed} should succeed");
        }
    }
}
