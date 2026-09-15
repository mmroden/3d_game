//! Fixed-geometry level construction: an environment's authored zones
//! become the SAME `LevelGraph` the procedural sweeps produce — `PlacedRoom`
//! nodes on the integer grid, mated doorway connectors on shared zone
//! faces — so culling, the recon map, `farthest_room_from`, portal and
//! boss placement all reuse verbatim. Pure function of the environment
//! def: deterministic by construction, no RNG anywhere.
//!
//! Zone boxes are authored in model-space meters on a 1-meter grid; the
//! kit's declared scale is the pitch, so `world_position` (grid × pitch)
//! lands every room at its model position × scale — the same transform the
//! scene mesh gets. Spawn points convert to room-local world units here,
//! honoring the `RoomTemplate` contract.

use crate::level_graph::LevelGraph;
use crate::planet::Pitch;
use crate::room_template::{
    Connector, ConnectorFacing, FrameStyle, RoomTemplate, SpawnPoint, TemplateKind,
};
use crate::roster::{EnvironmentDef, ZoneDef};

/// Build the level graph for a fixed environment. `structure_only` builds
/// just the start zone (the menu-backdrop contract: geometry, nobody home).
pub fn build_graph(env: &EnvironmentDef, pitch: Pitch, structure_only: bool) -> LevelGraph {
    let mut level = LevelGraph::new();

    // Start zone first: room index 0 IS the spawn room (house convention —
    // the manifest keeps it enemy-free, spawn_pose centers in it).
    let order: Vec<usize> = std::iter::once(env.start_zone)
        .chain((0..env.zones.len()).filter(|&i| i != env.start_zone))
        .take(if structure_only { 1 } else { env.zones.len() })
        .collect();

    let mut nodes = vec![None; env.zones.len()];
    for &zi in &order {
        let zone = &env.zones[zi];
        let template = RoomTemplate {
            kind: TemplateKind::Room,
            connectors: if structure_only { Vec::new() } else { connectors_for(env, zi) },
            enemy_spawns: room_local(&zone.enemy_spawns, zone, pitch),
            loot_spawns: room_local(&zone.loot_spawns, zone, pitch),
            extents: zone.extents,
        };
        let node = level
            .place_room(template, zone.min)
            .expect("linker guarantees disjoint zone boxes");
        nodes[zi] = Some(node);
    }

    for (zi, zone) in env.zones.iter().enumerate() {
        for &l in &zone.links {
            let (Some(a), Some(b)) = (nodes[zi], nodes[l]) else { continue };
            level
                .connect_adjacent(a, b)
                .expect("linker guarantees linked zones share a face");
        }
    }

    if !structure_only {
        level.boss_room = nodes[env.boss_zone];
    }
    level
}

/// Authored model-space points → room-local world units (the
/// `RoomTemplate` contract): subtract the zone's box min, scale by pitch.
fn room_local(points: &[[f32; 3]], zone: &ZoneDef, pitch: Pitch) -> Vec<SpawnPoint> {
    points
        .iter()
        .map(|p| SpawnPoint {
            position: [
                (p[0] - zone.min[0] as f32) * pitch.tile,
                (p[1] - zone.min[1] as f32) * pitch.story,
                (p[2] - zone.min[2] as f32) * pitch.tile,
            ],
        })
        .collect()
}

/// One frameless doorway connector per linked neighbor, placed mid-way
/// along the shared face (the house's own door frames are the visuals).
/// Both sides synthesize their half, so the pair mates under
/// `connect_adjacent`'s exact-cell rule.
fn connectors_for(env: &EnvironmentDef, zi: usize) -> Vec<Connector> {
    let zone = &env.zones[zi];
    let mut out = Vec::new();
    // Links are declared once (undirected): gather both directions.
    let neighbors = env.zones.iter().enumerate().filter_map(|(oi, o)| {
        if zone.links.contains(&oi) || o.links.contains(&zi) {
            Some(oi)
        } else {
            None
        }
    });
    for oi in neighbors {
        let other = &env.zones[oi];
        let Some((axis, positive)) = touching_face(zone, other) else {
            continue; // linker guarantees this never happens for linked pairs
        };
        let facing = match (axis, positive) {
            (0, true) => ConnectorFacing::PosX,
            (0, false) => ConnectorFacing::NegX,
            (1, true) => ConnectorFacing::PosY,
            (1, false) => ConnectorFacing::NegY,
            (2, true) => ConnectorFacing::PosZ,
            _ => ConnectorFacing::NegZ,
        };
        let mut offset = [0i32; 3];
        offset[axis] = if positive { zone.extents[axis] as i32 - 1 } else { 0 };
        for j in (0..3).filter(|&j| j != axis) {
            let lo = zone.min[j].max(other.min[j]);
            let hi = (zone.min[j] + zone.extents[j] as i32)
                .min(other.min[j] + other.extents[j] as i32);
            // Mid-cell of the overlap, in this zone's local coordinates.
            offset[j] = lo + (hi - lo - 1) / 2 - zone.min[j];
        }
        out.push(Connector { offset, facing, frame: FrameStyle::None });
    }
    out
}

/// The axis on which `zone`'s face touches `other` (and whether it is
/// zone's positive side), when the other two axes overlap.
fn touching_face(zone: &ZoneDef, other: &ZoneDef) -> Option<(usize, bool)> {
    for axis in 0..3 {
        let overlap = (0..3).filter(|&j| j != axis).all(|j| {
            zone.min[j] < other.min[j] + other.extents[j] as i32
                && other.min[j] < zone.min[j] + zone.extents[j] as i32
        });
        if !overlap {
            continue;
        }
        if zone.min[axis] + zone.extents[axis] as i32 == other.min[axis] {
            return Some((axis, true));
        }
        if other.min[axis] + other.extents[axis] as i32 == zone.min[axis] {
            return Some((axis, false));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed::Seed;
    use crate::test_fixtures::fixed_fixture_grammar;

    fn fixture_env_and_pitch() -> (crate::roster::EnvironmentDef, Pitch) {
        let grammar = fixed_fixture_grammar();
        let env = grammar
            .environment_for_level(1, crate::seed::Seed::new(1))
            .expect("the fixture planet is fixed")
            .clone();
        let pitch = grammar.pitch_for_level(1, crate::seed::Seed::new(1));
        (env, pitch)
    }

    #[test]
    fn every_zone_places_with_the_start_zone_first() {
        let (env, pitch) = fixture_env_and_pitch();
        let level = build_graph(&env, pitch, false);
        assert_eq!(level.room_count(), env.zones.len());
        let first = level.room_indices().next().expect("rooms placed");
        let room0 = level.room(first).expect("room 0");
        assert_eq!(
            room0.grid_pos, env.zones[env.start_zone].min,
            "room 0 is the start zone"
        );
        assert!(level.is_fully_connected(), "authored links wire every zone");
    }

    #[test]
    fn the_boss_zone_is_the_boss_room_and_farthest_from_the_start() {
        let (env, pitch) = fixture_env_and_pitch();
        let level = build_graph(&env, pitch, false);
        let boss = level.boss_room().expect("the boss room is marked");
        assert_eq!(
            level.room(boss).expect("boss room").grid_pos,
            env.zones[env.boss_zone].min,
            "the marked boss room is the authored boss zone"
        );
        let start = level.room_indices().next().expect("rooms placed");
        assert_eq!(
            level.exit_room(start),
            Some(boss),
            "portal/arena coupling: the arena owns the exit"
        );
    }

    #[test]
    fn spawns_convert_to_room_local_world_units() {
        let (env, pitch) = fixture_env_and_pitch();
        let level = build_graph(&env, pitch, false);
        // Every authored point, re-derived through the same env def: the
        // template carries (point - zone.min) * pitch, exactly.
        for idx in level.room_indices() {
            let room = level.room(idx).expect("room");
            let zone = env
                .zones
                .iter()
                .find(|z| z.min == room.grid_pos)
                .expect("each room is a zone");
            assert_eq!(room.template.enemy_spawns.len(), zone.enemy_spawns.len());
            for (sp, p) in room.template.enemy_spawns.iter().zip(&zone.enemy_spawns) {
                assert_eq!(
                    sp.position,
                    [
                        (p[0] - zone.min[0] as f32) * pitch.tile,
                        (p[1] - zone.min[1] as f32) * pitch.story,
                        (p[2] - zone.min[2] as f32) * pitch.tile,
                    ]
                );
            }
        }
    }

    #[test]
    fn the_build_is_deterministic() {
        let (env, pitch) = fixture_env_and_pitch();
        let a = build_graph(&env, pitch, false);
        let b = build_graph(&env, pitch, false);
        assert_eq!(a.room_count(), b.room_count());
        assert_eq!(a.edge_count(), b.edge_count());
        assert_eq!(a.boss_room(), b.boss_room());
        let _ = Seed::new(1); // no RNG anywhere in this path — pure data
    }

    #[test]
    fn structure_only_builds_just_the_start_zone() {
        let (env, pitch) = fixture_env_and_pitch();
        let level = build_graph(&env, pitch, true);
        assert_eq!(level.room_count(), 1, "the backdrop is geometry, nobody home");
        assert_eq!(level.boss_room(), None, "the backdrop build stays bossless");
    }
}
