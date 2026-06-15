//! Level assembly: builds meshes, lights, enemies, and collision boxes from a LevelGraph.

use crate::level_graph::LevelGraph;
use crate::seed::Seed;
use crate::room_assembler::MeshPlacement;
use crate::room_furnisher::{LightAccent, LightSource};

/// Axis-aligned world bounds of a room plus its spatial adjacency —
/// the minimal geometry the shell needs to decide which rooms to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
    /// Room-list indices of spatially-adjacent nodes: reachable by
    /// flying through a portal, hence visible when this node is.
    /// Teleporter links (spatially distant) are excluded.
    pub neighbors: Vec<usize>,
    /// Corridors are connective tissue — you see *through* them into the
    /// rooms they join, so visibility passes through them rather than
    /// stopping. Rooms are opaque: visible, but not seen past.
    pub is_corridor: bool,
}

impl RoomBounds {
    /// Whether `point` lies within these bounds.
    pub fn contains(&self, point: [f32; 3]) -> bool {
        (0..3).all(|a| point[a] >= self.min[a] && point[a] <= self.max[a])
    }
}

/// Index of the room whose bounds contain `point`, if any.
pub fn room_at(point: [f32; 3], rooms: &[RoomBounds]) -> Option<usize> {
    rooms.iter().position(|r| r.contains(point))
}

/// The nodes that should render while the player occupies `current`:
/// the node itself, every node adjacent to it, and — since you see
/// *through* corridors — every node reachable along a path whose
/// interior is all corridors. Opaque rooms are lit but not seen past,
/// so sight stops at the first room beyond any corridor.
pub fn visible_rooms(current: usize, rooms: &[RoomBounds]) -> Vec<usize> {
    let mut visible = vec![current];
    let mut frontier = vec![current];
    while let Some(node) = frontier.pop() {
        let Some(bounds) = rooms.get(node) else { continue };
        // Expand outward only from the current node and from corridors
        // (transparent). A room reached by expansion is lit but opaque,
        // so we never expand through it.
        if node != current && !bounds.is_corridor {
            continue;
        }
        for &neighbor in &bounds.neighbors {
            if !visible.contains(&neighbor) {
                visible.push(neighbor);
                frontier.push(neighbor);
            }
        }
    }
    visible
}

/// All of one room's assembled content, grouped so the shell can parent
/// it under a single node and cull the whole room at once.
#[derive(Debug, Clone)]
pub struct RoomAssembly {
    pub meshes: Vec<MeshPlacement>,
    pub lights: Vec<LightSource>,
    pub enemy_positions: Vec<[f32; 3]>,
    pub bounds: RoomBounds,
}

/// Walk a generated level graph, assemble room geometry, furnish rooms,
/// and return all mesh placements plus light sources for the level.
pub fn spawn_list(
    graph: &LevelGraph,
    cell_size: f32,
    seed: Seed,
) -> (Vec<MeshPlacement>, Vec<LightSource>) {
    let mut meshes = Vec::new();
    let mut lights = Vec::new();
    for room in spawn_list_full(graph, cell_size, seed) {
        meshes.extend(room.meshes);
        lights.extend(room.lights);
    }
    (meshes, lights)
}

/// Like `spawn_list`, but groups every room's geometry, lights, enemy
/// spawns, and colliders under one `RoomAssembly` — preserving the room
/// identity the shell needs to parent and cull per room.
pub fn spawn_list_full(
    graph: &LevelGraph,
    cell_size: f32,
    seed: Seed,
) -> Vec<RoomAssembly> {
    use crate::cell::CellGrid;
    use crate::level_graph::EdgeKind;
    use crate::room_furnisher;
    use crate::room_theme;

    // Spatial adjacency between rooms (room-list indices). Every edge
    // except a teleporter is a portal you can see through, so it makes
    // the two rooms mutual neighbors; teleporters span the map and are
    // excluded.
    let pos_of: std::collections::HashMap<_, usize> =
        graph.room_indices().enumerate().map(|(i, n)| (n, i)).collect();
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); pos_of.len()];
    for (a, b, kind) in graph.edges() {
        if matches!(kind, EdgeKind::Teleporter) {
            continue;
        }
        if let (Some(&ai), Some(&bi)) = (pos_of.get(&a), pos_of.get(&b)) {
            if !adjacency[ai].contains(&bi) {
                adjacency[ai].push(bi);
            }
            if !adjacency[bi].contains(&ai) {
                adjacency[bi].push(ai);
            }
        }
    }

    let mut rooms = Vec::new();

    for (room_idx, idx) in graph.room_indices().enumerate() {
        let Some(room) = graph.room(idx) else { continue };
        let active = graph.active_connectors(idx);
        let theme = room_theme::theme_for_room(seed.value(), room_idx);
        let story_height = theme.wall_set.story_height;
        let origin = room.world_position(cell_size, story_height);

        let mut grid = CellGrid::new(&room.template, &active, origin, cell_size);
        let mut meshes = crate::room_assembler::assemble_from_grid(
            &grid,
            &room.template,
            &active,
            theme.wall_set,
        );

        let room_seed = seed.value().wrapping_add(room_idx as u64).wrapping_mul(2654435761);
        grid.populate(theme, room_seed);
        meshes.extend(grid.prop_placements());

        let mut lights = Vec::new();
        for (mesh, light) in room_furnisher::light_fixtures(&room.template, &active, origin, cell_size, room_seed) {
            meshes.push(mesh);
            lights.push(light);
        }

        let mut enemy_positions = Vec::new();
        if room_idx > 0 {
            for sp in &room.template.enemy_spawns {
                enemy_positions.push([
                    origin[0] + sp.position[0],
                    origin[1] + sp.position[1] + 1.5,
                    origin[2] + sp.position[2],
                ]);
            }
        }

        // World bounds = union of this room's cell AABBs (each cell spans
        // [floor, floor + story_height] in Y, ±half-cell in XZ). A
        // point-in-room test only needs to enclose the flyable interior.
        let half_cell = cell_size / 2.0;
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        let mut any = false;
        for cell in grid.cells() {
            any = true;
            let c = cell.world_center;
            min[0] = min[0].min(c[0] - half_cell);
            max[0] = max[0].max(c[0] + half_cell);
            min[1] = min[1].min(c[1]);
            max[1] = max[1].max(c[1] + story_height);
            min[2] = min[2].min(c[2] - half_cell);
            max[2] = max[2].max(c[2] + half_cell);
        }
        if !any {
            min = origin;
            max = origin;
        }

        rooms.push(RoomAssembly {
            meshes,
            lights,
            enemy_positions,
            bounds: RoomBounds {
                min,
                max,
                neighbors: std::mem::take(&mut adjacency[room_idx]),
                is_corridor: room.template.kind == crate::room_template::TemplateKind::Corridor,
            },
        });
    }

    // Light accents by room role, sourced from the level graph (no
    // parallel structure): the start chamber (first room) reads blue;
    // the exit chamber — the farthest room, where the portal sits — and
    // everything visible through corridors from it reads red.
    let bounds: Vec<RoomBounds> = rooms.iter().map(|r| r.bounds.clone()).collect();
    let exit_red: std::collections::HashSet<usize> = graph
        .room_indices()
        .next()
        .and_then(|start| graph.farthest_room_from(start))
        .and_then(|exit| pos_of.get(&exit).copied())
        .map(|exit_pos| visible_rooms(exit_pos, &bounds).into_iter().collect())
        .unwrap_or_default();
    for (pos, room) in rooms.iter_mut().enumerate() {
        let accent = if pos == 0 {
            LightAccent::Start
        } else if exit_red.contains(&pos) {
            LightAccent::Exit
        } else {
            continue; // Neutral: light_fixtures already set warm-white.
        };
        for light in &mut room.lights {
            light.color = accent.color(light.state.liveness());
        }
    }

    rooms
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

    fn test_config(seed: u64) -> GeneratorConfig {
        GeneratorConfig {
            seed: Seed::new(seed),
            max_rooms: 20,
            min_room_xz: 3,
            max_room_xz: 6,
            min_room_y: 1,
            max_room_y: 6,
        }
    }

    #[test]
    fn one_assembly_per_room() {
        let graph = generate(&test_config(7)).expect("generation");
        let rooms = spawn_list_full(&graph, 4.0, Seed::new(7));
        assert_eq!(rooms.len(), graph.room_count());
    }

    #[test]
    fn room_bounds_enclose_a_real_volume() {
        // Each room's bounds must be a non-degenerate box derived from
        // its geometry — the stub (min == max) fails this.
        let graph = generate(&test_config(7)).expect("generation");
        let rooms = spawn_list_full(&graph, 4.0, Seed::new(7));
        for (i, room) in rooms.iter().enumerate() {
            for a in 0..3 {
                assert!(
                    room.bounds.max[a] > room.bounds.min[a],
                    "room {i} axis {a} bounds are degenerate: {:?}..{:?}",
                    room.bounds.min,
                    room.bounds.max
                );
            }
        }
    }

    #[test]
    fn room_at_locates_interior_points_and_rejects_distant_ones() {
        let graph = generate(&test_config(7)).expect("generation");
        let rooms = spawn_list_full(&graph, 4.0, Seed::new(7));
        let bounds: Vec<_> = rooms.iter().map(|r| r.bounds.clone()).collect();

        for room in &rooms {
            let mid = [
                (room.bounds.min[0] + room.bounds.max[0]) * 0.5,
                (room.bounds.min[1] + room.bounds.max[1]) * 0.5,
                (room.bounds.min[2] + room.bounds.max[2]) * 0.5,
            ];
            let found = room_at(mid, &bounds);
            assert!(found.is_some(), "a room's own midpoint must land in some room");
            assert!(
                bounds[found.unwrap()].contains(mid),
                "room_at must return a room that actually contains the point"
            );
        }
        assert_eq!(
            room_at([1.0e6, 1.0e6, 1.0e6], &bounds),
            None,
            "a point far outside every room must not match"
        );
    }

    #[test]
    fn neighbors_match_spatial_edges_and_exclude_teleporters() {
        use std::collections::HashMap;
        let mut adjacent_checked = 0u32;
        for seed in 0..30u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let rooms = spawn_list_full(&graph, 4.0, Seed::new(seed));
            let pos_of: HashMap<_, usize> =
                graph.room_indices().enumerate().map(|(i, n)| (n, i)).collect();
            for (a, b, kind) in graph.edges() {
                let (Some(&ai), Some(&bi)) = (pos_of.get(&a), pos_of.get(&b)) else {
                    continue;
                };
                match kind {
                    EdgeKind::Teleporter => {
                        assert!(
                            !rooms[ai].bounds.neighbors.contains(&bi)
                                && !rooms[bi].bounds.neighbors.contains(&ai),
                            "teleporter-linked rooms must not be visible neighbors (seed {seed})"
                        );
                    }
                    _ => {
                        adjacent_checked += 1;
                        assert!(
                            rooms[ai].bounds.neighbors.contains(&bi),
                            "spatially adjacent rooms must be neighbors (seed {seed})"
                        );
                        assert!(
                            rooms[bi].bounds.neighbors.contains(&ai),
                            "adjacency must be symmetric (seed {seed})"
                        );
                    }
                }
            }
        }
        assert!(adjacent_checked > 0, "no adjacent edges across 30 seeds");
    }

    fn bounds_with(neighbors: Vec<usize>) -> RoomBounds {
        RoomBounds {
            min: [0.0; 3],
            max: [1.0; 3],
            neighbors,
            is_corridor: false,
        }
    }

    fn corridor_with(neighbors: Vec<usize>) -> RoomBounds {
        RoomBounds {
            min: [0.0; 3],
            max: [1.0; 3],
            neighbors,
            is_corridor: true,
        }
    }

    #[test]
    fn culling_lights_the_current_room_and_immediate_neighbors_only() {
        // Line graph 0—1—2—3—4. Standing in room 1 (joined to 0 and 2),
        // rooms 3 and 4 are two-plus hops away and must stay dark — the
        // defining behavior of "current + immediate neighbors".
        let rooms = vec![
            bounds_with(vec![1]),    // 0
            bounds_with(vec![0, 2]), // 1
            bounds_with(vec![1, 3]), // 2
            bounds_with(vec![2, 4]), // 3
            bounds_with(vec![3]),    // 4
        ];
        let mut visible = visible_rooms(1, &rooms);
        visible.sort_unstable();
        assert_eq!(visible, vec![0, 1, 2], "room 1 lights itself, 0, and 2");
        assert!(!visible.contains(&3), "room two hops away must stay dark");
        assert!(!visible.contains(&4), "room three hops away must stay dark");
    }

    #[test]
    fn culling_sees_through_a_corridor_into_the_room_beyond() {
        // Room 0 —[corridor 1]— Room 2. Standing in room 0 you look
        // down the corridor into room 2, so all three are lit. With
        // 1-deep neighbors, room 2 (two hops) would wrongly stay dark.
        let nodes = vec![
            bounds_with(vec![1]),    // 0: room
            corridor_with(vec![0, 2]), // 1: corridor between 0 and 2
            bounds_with(vec![1]),    // 2: room
        ];
        let mut visible = visible_rooms(0, &nodes);
        visible.sort_unstable();
        assert_eq!(
            visible,
            vec![0, 1, 2],
            "you see through a corridor into the room beyond it"
        );
    }

    #[test]
    fn culling_does_not_see_through_an_opaque_room() {
        // Room 0 —[corridor 1]— Room 2 —[corridor 3]— Room 4. From room
        // 0 you see through corridor 1 into room 2, but room 2 is opaque:
        // corridor 3 and room 4 behind it stay dark.
        let nodes = vec![
            bounds_with(vec![1]),       // 0: room
            corridor_with(vec![0, 2]),  // 1: corridor
            bounds_with(vec![1, 3]),    // 2: room (opaque)
            corridor_with(vec![2, 4]),  // 3: corridor beyond room 2
            bounds_with(vec![3]),       // 4: room
        ];
        let mut visible = visible_rooms(0, &nodes);
        visible.sort_unstable();
        assert_eq!(visible, vec![0, 1, 2], "sight stops at the opaque room 2");
    }

    #[test]
    fn visibility_is_transparent_through_corridors_for_every_room() {
        use std::collections::HashSet;
        // Integration over real generated graphs: place the player in
        // every node and assert that whenever a corridor is lit, every
        // room it opens onto is lit too — i.e. you always see through a
        // hallway into the room at its far end (never a black doorway).
        let mut corridors_checked = 0u32;
        for seed in 0..30u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let rooms = spawn_list_full(&graph, 4.0, Seed::new(seed));
            let bounds: Vec<_> = rooms.iter().map(|r| r.bounds.clone()).collect();

            for current in 0..bounds.len() {
                let visible: HashSet<usize> =
                    visible_rooms(current, &bounds).into_iter().collect();
                assert!(visible.contains(&current), "the player's own node is lit");
                for &node in &visible {
                    if bounds[node].is_corridor {
                        for &beyond in &bounds[node].neighbors {
                            assert!(
                                visible.contains(&beyond),
                                "seed {seed} in node {current}: corridor {node} is lit \
                                 but node {beyond} beyond it is dark"
                            );
                            corridors_checked += 1;
                        }
                    }
                }
            }
        }
        assert!(corridors_checked > 0, "no corridors exercised across 30 seeds");
    }

    #[test]
    fn visibility_does_not_spill_past_opaque_rooms() {
        use std::collections::HashSet;
        // Exclusion bound: a lit node (other than the player's own) must
        // be adjacent to the current node or to a lit corridor — sight
        // only ever extends *through corridors*, never through a room.
        for seed in 0..30u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let rooms = spawn_list_full(&graph, 4.0, Seed::new(seed));
            let bounds: Vec<_> = rooms.iter().map(|r| r.bounds.clone()).collect();

            for current in 0..bounds.len() {
                let visible: HashSet<usize> =
                    visible_rooms(current, &bounds).into_iter().collect();
                for &node in &visible {
                    if node == current {
                        continue;
                    }
                    let adjacent_to_current = bounds[current].neighbors.contains(&node);
                    let adjacent_to_lit_corridor = visible.iter().any(|&v| {
                        bounds[v].is_corridor && bounds[v].neighbors.contains(&node)
                    });
                    assert!(
                        adjacent_to_current || adjacent_to_lit_corridor,
                        "seed {seed} in node {current}: node {node} is lit but not \
                         reachable through a corridor — sight spilled through a room"
                    );
                }
            }
        }
    }

    #[test]
    fn culling_keeps_the_current_room_and_tolerates_malformed_adjacency() {
        // Dangling index, self-reference, isolated room.
        let rooms = vec![
            bounds_with(vec![99]), // 0: neighbor out of range
            bounds_with(vec![1]),  // 1: references itself
            bounds_with(vec![]),   // 2: isolated
        ];
        for current in 0..rooms.len() {
            assert!(
                visible_rooms(current, &rooms).contains(&current),
                "the player's own room is always visible"
            );
        }
        // A dangling neighbor is passed through (harmless downstream — no
        // node carries that index) but must not panic or be dropped.
        assert!(visible_rooms(0, &rooms).contains(&99));
        // An out-of-range current room yields just itself, no panic.
        assert_eq!(visible_rooms(7, &rooms), vec![7]);
    }

    #[test]
    fn integration_seed_to_lit_lights_for_a_player_in_a_room() {
        use std::collections::HashSet;
        // The whole pipeline, end to end: a seed generates the graph,
        // assembly places real rooms and lights, the player is dropped
        // into a room *by world position* (exercising room_at, not an
        // index), and we check which generated lights end up on vs off.
        let mut levels_that_culled_lights = 0u32;

        for seed in 0..30u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let rooms = spawn_list_full(&graph, 4.0, Seed::new(seed));
            if rooms.is_empty() {
                continue;
            }
            let bounds: Vec<_> = rooms.iter().map(|r| r.bounds.clone()).collect();

            // Stand at the center of room 0 and resolve the room from the
            // position — this is how the running game decides the room.
            let stand = [
                (bounds[0].min[0] + bounds[0].max[0]) * 0.5,
                (bounds[0].min[1] + bounds[0].max[1]) * 0.5,
                (bounds[0].min[2] + bounds[0].max[2]) * 0.5,
            ];
            let current = room_at(stand, &bounds).expect("own room must resolve");
            assert!(
                bounds[current].contains(stand),
                "seed {seed}: room_at must return a room containing the player"
            );

            let visible: HashSet<usize> =
                visible_rooms(current, &bounds).into_iter().collect();
            assert!(visible.contains(&current), "the player's room is lit");

            // Partition every generated light by whether its room renders.
            let mut on = 0u32;
            let mut off = 0u32;
            for (i, room) in rooms.iter().enumerate() {
                let count = room.lights.len() as u32;
                if visible.contains(&i) {
                    on += count;
                } else {
                    off += count;
                }
            }
            let total: u32 = rooms.iter().map(|r| r.lights.len() as u32).sum();
            assert_eq!(on + off, total, "seed {seed}: every light is on or off, none lost");

            // Every off-light belongs to a culled (non-visible) room — by
            // construction above — and culling must actually turn some
            // off, or the feature is a no-op.
            if off > 0 {
                levels_that_culled_lights += 1;
            }
        }

        assert!(
            levels_that_culled_lights > 0,
            "across 30 seeds, culling never turned a single generated light off"
        );
    }

    #[test]
    fn start_room_lights_carry_the_blue_accent() {
        // Role coloring sourced from the graph: the first room (where the
        // player spawns) reads blue. Without the accent pass its lights
        // would be warm-white (red ≈ 1.0), so this pins the wiring.
        for seed in 0..30u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let rooms = spawn_list_full(&graph, 4.0, Seed::new(seed));
            if rooms.is_empty() || rooms[0].lights.is_empty() {
                continue;
            }
            for light in &rooms[0].lights {
                assert!(
                    light.color[0] < 0.95,
                    "start light should be blue-tinted (red={})",
                    light.color[0]
                );
                assert!(
                    light.color[2] > light.color[1],
                    "start light should lean blue (blue > green): {:?}",
                    light.color
                );
            }
            return;
        }
        panic!("no seed produced a start room with lights");
    }
}
