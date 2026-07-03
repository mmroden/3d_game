//! The fog-of-war recon map (a permanent unlock), derived from the retained
//! [`LevelGraph`] — the same authority that drives room culling; never a
//! parallel structure.
//!
//! Only visited rooms appear. An edge from a visited room to an unvisited one
//! renders as a short *frontier stub*: it says "an unexplored corridor leaves
//! this room, that way" without revealing where the far room sits. Room ids
//! are room-list positions (`room_indices()` order) — the same indexing the
//! shell's culling and bounds use.

use crate::level_graph::LevelGraph;

/// Length of a frontier stub in unit-square space: long enough to read as a
/// direction, far too short to place the unexplored room.
pub const FRONTIER_STUB: f32 = 0.06;

/// A revealed room, in unit-square coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapRoom {
    /// Room-list position (`room_indices()` order).
    pub id: usize,
    pub pos: [f32; 2],
    pub is_current: bool,
}

/// A drawable map edge: full corridors between visited rooms, or a frontier
/// stub pointing at the unexplored.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapEdge {
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub frontier: bool,
}

/// Everything the map widget draws.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MapView {
    pub rooms: Vec<MapRoom>,
    pub edges: Vec<MapEdge>,
}

/// Project the graph onto the map for a player who has visited `visited`
/// (room-list positions) and currently sits in `current`. Coordinates are
/// normalized to the unit square over the WHOLE level's footprint, so the
/// map's frame never re-scales as exploration grows.
pub fn map_view(graph: &LevelGraph, visited: &[usize], current: usize) -> MapView {
    use std::collections::HashMap;

    // Room-list position <-> node, and each room's unit-square position,
    // normalized over the WHOLE footprint so the frame never re-scales.
    let nodes: Vec<_> = graph.room_indices().collect();
    let grid: Vec<[f32; 2]> = nodes.iter()
        .filter_map(|n| graph.room(*n))
        .map(|room| [room.grid_pos[0] as f32, room.grid_pos[2] as f32])
        .collect();
    if grid.len() != nodes.len() || grid.is_empty() {
        return MapView::default();
    }
    let (mut min, mut max) = ([f32::MAX; 2], [f32::MIN; 2]);
    for p in &grid {
        for axis in 0..2 {
            min[axis] = min[axis].min(p[axis]);
            max[axis] = max[axis].max(p[axis]);
        }
    }
    let span = [(max[0] - min[0]).max(1.0), (max[1] - min[1]).max(1.0)];
    let unit = |p: [f32; 2]| [(p[0] - min[0]) / span[0], (p[1] - min[1]) / span[1]];
    let positions: Vec<[f32; 2]> = grid.into_iter().map(unit).collect();
    let id_of: HashMap<_, _> = nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let is_visited = |id: usize| visited.contains(&id);

    let rooms = (0..nodes.len())
        .filter(|id| is_visited(*id))
        .map(|id| MapRoom { id, pos: positions[id], is_current: id == current })
        .collect();

    let mut edges = Vec::new();
    for (a, b, _kind) in graph.edges() {
        let (Some(&ia), Some(&ib)) = (id_of.get(&a), id_of.get(&b)) else { continue };
        match (is_visited(ia), is_visited(ib)) {
            // A corridor between two known rooms draws in full.
            (true, true) => edges.push(MapEdge {
                from: positions[ia],
                to: positions[ib],
                frontier: false,
            }),
            // Known -> unknown: a short stub along the corridor's direction,
            // never reaching (or revealing) the far room.
            (true, false) | (false, true) => {
                let (known, hidden) = if is_visited(ia) { (ia, ib) } else { (ib, ia) };
                let (from, to) = (positions[known], positions[hidden]);
                let dir = [to[0] - from[0], to[1] - from[1]];
                let len = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
                if len <= f32::EPSILON {
                    continue;
                }
                let scale = FRONTIER_STUB / len;
                edges.push(MapEdge {
                    from,
                    to: [from[0] + dir[0] * scale, from[1] + dir[1] * scale],
                    frontier: true,
                });
            }
            (false, false) => {}
        }
    }

    MapView { rooms, edges }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{generate, GeneratorConfig};
    use crate::seed::Seed;

    /// The pinned GUT seed's level, through the shell's exact build path.
    fn level() -> LevelGraph {
        generate(&GeneratorConfig::standard(Seed::from_i64(1), 8))
            .expect("the pinned seed must generate")
    }

    fn all_rooms(graph: &LevelGraph) -> Vec<usize> {
        (0..graph.room_count()).collect()
    }

    #[test]
    fn only_visited_rooms_are_revealed() {
        let graph = level();
        let view = map_view(&graph, &[0], 0);
        assert_eq!(view.rooms.len(), 1, "one room visited, one room drawn");
        assert_eq!(view.rooms[0].id, 0);
        assert!(view.rooms[0].is_current);

        let everything = all_rooms(&graph);
        let full = map_view(&graph, &everything, 0);
        assert_eq!(full.rooms.len(), graph.room_count(), "full exploration draws every room");
    }

    #[test]
    fn current_room_is_flagged_exactly_once() {
        let graph = level();
        let everything = all_rooms(&graph);
        let view = map_view(&graph, &everything, 2);
        let current: Vec<_> = view.rooms.iter().filter(|r| r.is_current).collect();
        assert_eq!(current.len(), 1, "exactly one current room");
        assert_eq!(current[0].id, 2);
    }

    #[test]
    fn coordinates_stay_in_the_unit_square() {
        let graph = level();
        let everything = all_rooms(&graph);
        let view = map_view(&graph, &everything, 0);
        for room in &view.rooms {
            assert!((0.0..=1.0).contains(&room.pos[0]) && (0.0..=1.0).contains(&room.pos[1]),
                "room {} escaped the unit square: {:?}", room.id, room.pos);
        }
        for edge in &view.edges {
            for p in [edge.from, edge.to] {
                assert!((-0.01..=1.01).contains(&p[0]) && (-0.01..=1.01).contains(&p[1]),
                    "edge point escaped the unit square: {p:?}");
            }
        }
    }

    #[test]
    fn frontier_stubs_point_without_revealing() {
        let graph = level();
        // Only the start room visited: every drawn edge is a frontier stub.
        let view = map_view(&graph, &[0], 0);
        assert!(!view.edges.is_empty(), "the start room has unexplored corridors");
        let full = map_view(&graph, &all_rooms(&graph), 0);
        for edge in &view.edges {
            assert!(edge.frontier, "with one room visited, every edge is a frontier");
            let len = ((edge.to[0] - edge.from[0]).powi(2) + (edge.to[1] - edge.from[1]).powi(2)).sqrt();
            assert!(len <= FRONTIER_STUB + 1e-3,
                "a stub stays short (got {len}), it must not reach the hidden room");
            // The stub must be strictly shorter than the real corridor it
            // hints at — otherwise it IS the reveal.
            let real = full.edges.iter()
                .filter(|e| !e.frontier)
                .map(|e| {
                    let d0 = (e.from[0] - edge.from[0]).powi(2) + (e.from[1] - edge.from[1]).powi(2);
                    let len = ((e.to[0] - e.from[0]).powi(2) + (e.to[1] - e.from[1]).powi(2)).sqrt();
                    (d0, len)
                })
                .filter(|(d0, _)| *d0 < 1e-6)
                .map(|(_, len)| len)
                .fold(0.0f32, f32::max);
            assert!(real > len, "the real corridor ({real}) outreaches the stub ({len})");
        }
    }

    #[test]
    fn full_exploration_has_no_frontiers() {
        let graph = level();
        let view = map_view(&graph, &all_rooms(&graph), 0);
        assert!(!view.edges.is_empty(), "a connected level draws corridors");
        assert!(view.edges.iter().all(|e| !e.frontier),
            "nothing is unexplored once everything is visited");
    }
}
