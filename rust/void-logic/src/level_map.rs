//! The fog-of-war recon map (a permanent unlock), derived from the retained
//! [`LevelGraph`] — the same authority that drives room culling; never a
//! parallel structure.
//!
//! The map draws the level's REAL rectilinear footprints (playtest
//! 2026-07-04: abstract dots were useless): every visited node — room or
//! corridor — renders as its top-down rectangle, uniformly scaled so the
//! geometry keeps its aspect. Fog rules: unvisited rooms never appear; an
//! unvisited CORRIDOR adjacent to a visited node appears faint — the
//! direction into the dark, with real geometry but no room reveal.

use crate::level_graph::LevelGraph;

/// One drawable footprint, in unit-square coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapRect {
    /// Room-list position (`room_indices()` order).
    pub id: usize,
    /// Top-down rect `[x, z, w, d]`, uniform scale (aspect preserved),
    /// centered inside the unit square.
    pub rect: [f32; 4],
    pub corridor: bool,
    pub current: bool,
    /// An unvisited corridor bordering explored space — drawn faint.
    pub frontier: bool,
}

/// The map's world→unit transform for the XZ plane, per axis:
/// `unit = world * scale + offset`. The panel uses it to place the LIVE
/// player marker — the map itself redraws per room change, but the marker
/// tracks the ship every frame (playtest 2026-07-04: a room-granular
/// marker reads as never having moved).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapProjection {
    pub scale: f32,
    pub offset: [f32; 2],
}

/// Derive the world→unit projection matching [`map_rects`]' normalization:
/// grid coords are world / cell, so `unit = world·(s/cell) + (offset − min·s)`.
pub fn map_projection(graph: &LevelGraph, cell_size: f32) -> MapProjection {
    let Some((min, scale, offset)) = grid_frame(graph) else {
        return MapProjection { scale: 0.0, offset: [0.0; 2] };
    };
    MapProjection {
        scale: scale / cell_size,
        offset: [offset[0] - min[0] * scale, offset[1] - min[1] * scale],
    }
}

/// The shared normalization frame in grid space: `(min, scale, offset)` with
/// `unit = offset + (grid − min)·scale`. One uniform scale (the larger
/// span), the minor axis centered.
fn grid_frame(graph: &LevelGraph) -> Option<([f32; 2], f32, [f32; 2])> {
    let mut any = false;
    let (mut min, mut max) = ([f32::MAX; 2], [f32::MIN; 2]);
    for n in graph.room_indices() {
        let Some(room) = graph.room(n) else { continue };
        any = true;
        let [ex, _ey, ez] = room.template.extents;
        let (x, z) = (room.grid_pos[0] as f32, room.grid_pos[2] as f32);
        min[0] = min[0].min(x);
        min[1] = min[1].min(z);
        max[0] = max[0].max(x + ex as f32);
        max[1] = max[1].max(z + ez as f32);
    }
    if !any {
        return None;
    }
    let span = [(max[0] - min[0]).max(1.0), (max[1] - min[1]).max(1.0)];
    let scale = 1.0 / span[0].max(span[1]);
    let offset = [
        (1.0 - span[0] * scale) * 0.5,
        (1.0 - span[1] * scale) * 0.5,
    ];
    Some((min, scale, offset))
}

/// Project the graph onto the map for a player who has visited `visited`
/// (room-list positions) and currently sits in `current`. One uniform scale
/// over the WHOLE level footprint: the frame never re-scales and rectangles
/// never distort as exploration grows.
pub fn map_rects(graph: &LevelGraph, visited: &[usize], current: usize) -> Vec<MapRect> {
    use crate::room_template::TemplateKind;
    use std::collections::{HashMap, HashSet};

    let nodes: Vec<_> = graph.room_indices().collect();
    if nodes.is_empty() {
        return Vec::new();
    }

    // Grid footprints (top-down: x/z), and the level's total bounding box.
    let footprints: Vec<Option<([f32; 4], bool)>> = nodes.iter()
        .map(|n| graph.room(*n).map(|room| {
            let [ex, _ey, ez] = room.template.extents;
            (
                [
                    room.grid_pos[0] as f32,
                    room.grid_pos[2] as f32,
                    ex as f32,
                    ez as f32,
                ],
                room.template.kind == TemplateKind::Corridor,
            )
        }))
        .collect();
    // ONE uniform scale (the larger span), the minor axis centered: the
    // geometry keeps its aspect and the frame never re-scales. Shared with
    // `map_projection` so the live player marker lands where the rects are.
    let Some((min, scale, offset)) = grid_frame(graph) else {
        return Vec::new();
    };

    let id_of: HashMap<_, _> = nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    let visited_set: HashSet<usize> = visited.iter().copied().collect();

    // Frontier: unvisited CORRIDORS adjacent to explored space — real
    // geometry pointing into the dark, but never an unvisited room.
    let mut frontier: HashSet<usize> = HashSet::new();
    for (a, b, _kind) in graph.edges() {
        let (Some(&ia), Some(&ib)) = (id_of.get(&a), id_of.get(&b)) else { continue };
        for (this, other) in [(ia, ib), (ib, ia)] {
            if visited_set.contains(&other) && !visited_set.contains(&this) {
                if let Some((_, true)) = footprints[this] {
                    frontier.insert(this);
                }
            }
        }
    }

    (0..nodes.len())
        .filter_map(|id| {
            let (fp, corridor) = footprints[id]?;
            let shown = visited_set.contains(&id) || frontier.contains(&id);
            if !shown {
                return None;
            }
            Some(MapRect {
                id,
                rect: [
                    offset[0] + (fp[0] - min[0]) * scale,
                    offset[1] + (fp[1] - min[1]) * scale,
                    fp[2] * scale,
                    fp[3] * scale,
                ],
                corridor,
                current: id == current && visited_set.contains(&id),
                frontier: !visited_set.contains(&id),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{generate, GeneratorConfig};
    use crate::room_template::TemplateKind;
    use crate::seed::Seed;

    /// The pinned GUT seed's level, through the shell's exact build path.
    fn level() -> LevelGraph {
        generate(&GeneratorConfig::standard(Seed::from_i64(1), 8))
            .expect("the pinned seed must generate")
    }

    fn all_nodes(graph: &LevelGraph) -> Vec<usize> {
        (0..graph.room_count()).collect()
    }

    #[test]
    fn only_visited_nodes_and_frontier_corridors_appear() {
        let graph = level();
        let view = map_rects(&graph, &[0], 0);
        let solid: Vec<_> = view.iter().filter(|r| !r.frontier).collect();
        assert_eq!(solid.len(), 1, "one node visited, one solid rect");
        assert_eq!(solid[0].id, 0);
        assert!(solid[0].current);

        // Whatever else appears is a faint corridor at the frontier —
        // never an unvisited room.
        for r in view.iter().filter(|r| r.frontier) {
            assert!(r.corridor, "only corridors may hint into the dark, got room {}", r.id);
        }
        assert!(view.iter().any(|r| r.frontier),
            "the start room's exits hint at the unexplored");

        let full = map_rects(&graph, &all_nodes(&graph), 0);
        assert_eq!(full.len(), graph.room_count(), "full exploration draws every node");
        assert!(full.iter().all(|r| !r.frontier), "nothing is frontier once seen");
    }

    #[test]
    fn footprints_keep_their_true_aspect() {
        // Uniform scaling: a node's rect must have the same width/depth
        // ratio as its template extents — rectilinear geometry, no squish.
        let graph = level();
        let view = map_rects(&graph, &all_nodes(&graph), 0);
        let nodes: Vec<_> = graph.room_indices().collect();
        for r in &view {
            let room = graph.room(nodes[r.id]).expect("id maps to a node");
            let [ex, _ey, ez] = room.template.extents;
            let expect = ex as f32 / ez as f32;
            let got = r.rect[2] / r.rect[3];
            assert!((got - expect).abs() < 0.01,
                "node {}: footprint aspect {} must match template {}", r.id, got, expect);
        }
    }

    #[test]
    fn the_scale_is_shared_so_positions_are_true() {
        // Two nodes' rects must sit in the same relative arrangement as
        // their grid positions: one scale for the whole map.
        let graph = level();
        let view = map_rects(&graph, &all_nodes(&graph), 0);
        let nodes: Vec<_> = graph.room_indices().collect();
        // Derive the scale from the first node, verify on every other.
        let first = &view[0];
        let room0 = graph.room(nodes[first.id]).unwrap();
        let scale = first.rect[2] / room0.template.extents[0] as f32;
        for r in &view {
            let room = graph.room(nodes[r.id]).unwrap();
            let w = room.template.extents[0] as f32 * scale;
            assert!((r.rect[2] - w).abs() < 0.01,
                "node {}: width {} breaks the shared scale ({} expected)", r.id, r.rect[2], w);
        }
        // And the frame stays inside the unit square.
        for r in &view {
            assert!(r.rect[0] >= -0.01 && r.rect[0] + r.rect[2] <= 1.01,
                "node {} escapes horizontally: {:?}", r.id, r.rect);
            assert!(r.rect[1] >= -0.01 && r.rect[1] + r.rect[3] <= 1.01,
                "node {} escapes vertically: {:?}", r.id, r.rect);
        }
    }

    #[test]
    fn the_projection_places_world_points_on_their_map_rects() {
        let graph = level();
        let cell = 4.0;
        let view = map_rects(&graph, &all_nodes(&graph), 0);
        let proj = map_projection(&graph, cell);
        let nodes: Vec<_> = graph.room_indices().collect();
        let story = crate::asset_catalog::WALL_SET_ASTRA.story_height;
        for r in &view {
            let room = graph.room(nodes[r.id]).unwrap();
            let origin = room.world_position(cell, story);
            let [ex, _ey, ez] = room.template.extents;
            // The room's world center must land on its map rect's center.
            let ux = (origin[0] + ex as f32 * cell * 0.5) * proj.scale + proj.offset[0];
            let uz = (origin[2] + ez as f32 * cell * 0.5) * proj.scale + proj.offset[1];
            assert!((ux - (r.rect[0] + r.rect[2] * 0.5)).abs() < 1e-3,
                "node {}: world center X projects to {} but the rect centers at {}",
                r.id, ux, r.rect[0] + r.rect[2] * 0.5);
            assert!((uz - (r.rect[1] + r.rect[3] * 0.5)).abs() < 1e-3,
                "node {}: world center Z projects to {} but the rect centers at {}",
                r.id, uz, r.rect[1] + r.rect[3] * 0.5);
        }
    }

    #[test]
    fn current_node_is_flagged_exactly_once() {
        let graph = level();
        let view = map_rects(&graph, &all_nodes(&graph), 2);
        let current: Vec<_> = view.iter().filter(|r| r.current).collect();
        assert_eq!(current.len(), 1, "exactly one current node");
        assert_eq!(current[0].id, 2);
    }

    #[test]
    fn corridors_know_they_are_corridors() {
        let graph = level();
        let view = map_rects(&graph, &all_nodes(&graph), 0);
        let nodes: Vec<_> = graph.room_indices().collect();
        for r in &view {
            let room = graph.room(nodes[r.id]).unwrap();
            assert_eq!(r.corridor, room.template.kind == TemplateKind::Corridor,
                "node {} mislabels its kind", r.id);
        }
        assert!(view.iter().any(|r| r.corridor), "a connected level has corridors");
        assert!(view.iter().any(|r| !r.corridor), "and rooms");
    }
}
