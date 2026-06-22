//! Level assembly: builds meshes, lights, enemies, and collision boxes from a LevelGraph.

use crate::level_graph::LevelGraph;
use crate::seed::Seed;
use crate::room_assembler::MeshPlacement;
use crate::room_furnisher::{LightAccent, LightSource};

/// Axis-aligned world bounds of a room — the minimal geometry the shell
/// needs to resolve which room a point is in. Adjacency and cull
/// visibility live in the [`LevelGraph`](crate::level_graph::LevelGraph),
/// the single source of level topology.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
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

/// All of one room's assembled content, grouped so the shell can parent it
/// under a single node and cull the whole room at once — and split into the
/// three load steps the shell builds a room in: structure, then non-enemy
/// inhabitants (props + containers), then enemies.
#[derive(Debug, Clone)]
pub struct RoomAssembly {
    /// The room's shell: walls/floors/ceilings plus light-fixture meshes. The
    /// Static pieces fuse into the room's one merged collider.
    pub structure: Vec<MeshPlacement>,
    pub lights: Vec<LightSource>,
    /// Furnished fixtures (cell-rolled props). Decorative/dynamic, never the
    /// shell — built in the inhabitants step alongside containers.
    pub props: Vec<MeshPlacement>,
    /// Organics container (green pickup) spawn positions, from the template's
    /// loot spawns.
    pub containers: Vec<[f32; 3]>,
    /// Enemy spawn positions, from the template's enemy spawns.
    pub enemies: Vec<[f32; 3]>,
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
        meshes.extend(room.structure);
        meshes.extend(room.props);
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
    use crate::level_graph::RENDER_ROOM_DEPTH;
    use crate::room_furnisher;
    use crate::room_theme;

    let mut rooms = Vec::new();

    for (room_idx, idx) in graph.room_indices().enumerate() {
        let Some(room) = graph.room(idx) else { continue };
        let active = graph.active_connectors(idx);
        let theme = room_theme::theme_for_room(seed.value(), room_idx);
        let story_height = theme.wall_set.story_height;
        let origin = room.world_position(cell_size, story_height);

        let mut grid = CellGrid::new(&room.template, &active, origin, cell_size);
        // Step 1 data — the room's shell.
        let mut structure = crate::room_assembler::assemble_from_grid(
            &grid,
            &room.template,
            &active,
            theme.wall_set,
        );

        let room_seed = seed.value().wrapping_add(room_idx as u64).wrapping_mul(2654435761);
        grid.populate(theme, room_seed);
        // Step 2 data — furnished fixtures (cell-rolled).
        let props = grid.prop_placements();

        let mut lights = Vec::new();
        for (mesh, light) in room_furnisher::light_fixtures(&room.template, &active, origin, cell_size, room_seed) {
            // Light fixtures are part of the shell: they render in the structure
            // step and are passable, so they never join the merged collider.
            structure.push(mesh);
            lights.push(light);
        }

        // Step 2 data — organics containers, authored per template via loot
        // spawns (replaces the shell's old ad-hoc scatter).
        let containers: Vec<[f32; 3]> = room
            .template
            .loot_spawns
            .iter()
            .map(|sp| [
                origin[0] + sp.position[0],
                origin[1] + sp.position[1],
                origin[2] + sp.position[2],
            ])
            .collect();

        // Step 3 data — enemies, authored per template via enemy spawns. The
        // start room (room_idx 0) stays clear so the player isn't ambushed on
        // spawn.
        let mut enemies = Vec::new();
        if room_idx > 0 {
            for sp in &room.template.enemy_spawns {
                enemies.push([
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
            structure,
            lights,
            props,
            containers,
            enemies,
            bounds: RoomBounds { min, max },
        });
    }

    // Light accents by room role, sourced from the level graph (no
    // parallel structure): the start chamber (first room) reads blue;
    // the exit chamber — the farthest room, where the portal sits — and
    // everything visible through corridors from it reads red.
    let exit_red: std::collections::HashSet<usize> = graph
        .room_indices()
        .next()
        .and_then(|start| graph.farthest_room_from(start))
        .map(|exit| {
            graph
                .visible_from(exit, RENDER_ROOM_DEPTH)
                .into_iter()
                .map(|n| n.index())
                .collect()
        })
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
    use crate::level_graph::{EdgeKind, RENDER_ROOM_DEPTH};
    use crate::room_template::ConnectorFacing;
    use crate::seed::Seed;

    /// End-to-end: through the full generation pipeline, a real level's
    /// vertical shafts are square (no rounded corner pieces) and lit by rim
    /// fixtures (their ceilings are all open, so any light they carry is a
    /// rim light). Proves both features survive generation — not just the
    /// synthetic unit cases — so "I can't see them" is a findability matter,
    /// not a rendering gap.
    #[test]
    fn generated_vertical_shafts_are_square_and_rim_lit() {
        let cell = 4.0_f32;
        let mut square_shaft_seen = false;
        let mut rim_lit_shaft_seen = false;

        'seeds: for seed in 0..30u64 {
            let config = GeneratorConfig {
                seed: Seed::new(seed),
                max_rooms: 30,
                min_room_xz: 3,
                max_room_xz: 6,
                min_room_y: 1,
                max_room_y: 6,
            };
            let Ok(graph) = generate(&config) else { continue };
            let assemblies = spawn_list_full(&graph, cell, Seed::new(seed));

            for (i, idx) in graph.room_indices().enumerate() {
                let room = graph.room(idx).unwrap();
                let is_vertical_shaft = room.template.kind
                    == crate::room_template::TemplateKind::Corridor
                    && room.template.extents[0] == 2
                    && room.template.extents[2] == 2;
                if !is_vertical_shaft {
                    continue;
                }
                let asm = &assemblies[i];
                // Square: a shaft emits straight walls and no rounded corners.
                assert_eq!(
                    asm.structure.iter().filter(|m| m.scene.contains("Corner_Round")).count(),
                    0,
                    "seed {seed}: vertical shaft still has rounded corner pieces"
                );
                if asm.structure.iter().any(|m| m.scene.contains("_Straight")) {
                    square_shaft_seen = true;
                }
                // Rim-lit: all ceilings are open, so any light is a rim light.
                if !asm.lights.is_empty() {
                    rim_lit_shaft_seen = true;
                }
                if square_shaft_seen && rim_lit_shaft_seen {
                    break 'seeds;
                }
            }
        }

        assert!(square_shaft_seen, "no square vertical shaft found across 30 seeds");
        assert!(rim_lit_shaft_seen, "no rim-lit vertical shaft found across 30 seeds");
    }

    /// A vertical passage's aperture must not be capped by a floor/ceiling
    /// slab. The reported failure was a ~1×1 m visible opening where the
    /// model intends the full hole — a Platform tile left across the aperture
    /// cells. This walks generated levels and fails if any Platform sits
    /// inside an aperture footprint on its interface plane.
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
                // The aperture is a `span`×`span` cell footprint anchored at
                // the connector offset. The way a vertical passage gets
                // obstructed is a floor/ceiling slab left across it (the
                // "1×1 visible where 4×4 intended" bug) — a Platform tile
                // sitting on an aperture cell at the interface plane. Walls
                // line the perimeter (never a Platform); props are barred from
                // connector cells by the furnisher; so a Platform inside the
                // footprint on the plane is exactly the cap to catch.
                let span = from_connector.facing.opening_span();
                let x0 = origin[0] + from_connector.offset[0] as f32 * cell;
                let x1 = x0 + span as f32 * cell;
                let z0 = origin[2] + from_connector.offset[2] as f32 * cell;
                let z1 = z0 + span as f32 * cell;
                let plane_y = match from_connector.facing {
                    ConnectorFacing::PosY => {
                        origin[1] + (from_connector.offset[1] as f32 + 1.0) * story
                    }
                    _ => origin[1] + from_connector.offset[1] as f32 * story,
                };
                passages_checked += 1;

                for placement in &meshes {
                    if !placement.scene.contains("Platform") {
                        continue; // only floor/ceiling slabs can cap the hole
                    }
                    let [px, py, pz] = placement.position;
                    let in_footprint = px > x0 && px < x1 && pz > z0 && pz < z1;
                    let on_plane = (py - plane_y).abs() < 1.0;
                    assert!(
                        !(in_footprint && on_plane),
                        "seed {seed}: '{}' at {:?} caps the vertical aperture \
                         (footprint x[{x0:.1},{x1:.1}] z[{z0:.1},{z1:.1}] plane y {plane_y:.1})",
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
    fn rooms_split_into_the_three_load_groups() {
        // Every room carries a non-empty structure (its shell); across a span of
        // seeds, props and containers both appear — proving inhabitants flow
        // through their own groups (containers via the revived loot spawns), kept
        // separate from structure for per-room, per-step loading.
        let mut any_container = false;
        let mut any_prop = false;
        let mut any_enemy = false;
        for seed in 0..30u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let rooms = spawn_list_full(&graph, 4.0, Seed::new(seed));
            for room in &rooms {
                assert!(!room.structure.is_empty(), "seed {seed}: a room had no structure");
                any_container |= !room.containers.is_empty();
                any_prop |= !room.props.is_empty();
                any_enemy |= !room.enemies.is_empty();
            }
        }
        assert!(any_container, "no container appeared across 30 seeds");
        assert!(any_prop, "no prop appeared across 30 seeds");
        assert!(any_enemy, "no enemy appeared across 30 seeds");
    }

    #[test]
    fn start_room_stays_clear_of_enemies() {
        // The first room (player spawn) must never carry enemies — no ambush on
        // spawn — even though its template may define enemy spawns.
        for seed in 0..30u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let rooms = spawn_list_full(&graph, 4.0, Seed::new(seed));
            if let Some(start) = rooms.first() {
                assert!(start.enemies.is_empty(), "seed {seed}: start room has enemies");
            }
        }
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

            // room_at returns a room-list index; the matching graph node is
            // the one at that placement position. Visibility is the graph's.
            let current_node =
                graph.room_indices().nth(current).expect("current room node exists");
            let visible: HashSet<usize> = graph
                .visible_from(current_node, RENDER_ROOM_DEPTH)
                .into_iter()
                .map(|n| n.index())
                .collect();
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
