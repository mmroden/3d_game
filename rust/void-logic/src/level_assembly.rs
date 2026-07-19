//! Level assembly: builds meshes, lights, enemies, and collision boxes from a LevelGraph.

use crate::roster::EnemyKey;
#[cfg(test)]
use crate::roster::roster;
use crate::level_graph::LevelGraph;
use crate::planet::Pitch;
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
    /// The watertight collision shell (one solid slab per sealed cell
    /// face). The structure meshes above are the room's LOOK; these boxes
    /// are its PHYSICS (playtest 2026-07-06: render-triangle trimesh
    /// collision had the art's seam holes and caged grinding bodies).
    pub shell: Vec<crate::room_assembler::ShellSlab>,
}

/// Walk a generated level graph, assemble room geometry, furnish rooms,
/// and return all mesh placements plus light sources for the level.
pub fn spawn_list(
    graph: &LevelGraph,
    spec: &crate::level_spec::LevelSpec,
    seed: Seed,
    catalog: &crate::asset_catalog::AssetCatalog,
) -> (Vec<MeshPlacement>, Vec<LightSource>) {
    let mut meshes = Vec::new();
    let mut lights = Vec::new();
    for room in spawn_list_full(graph, spec, seed, catalog) {
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
    spec: &crate::level_spec::LevelSpec,
    seed: Seed,
    catalog: &crate::asset_catalog::AssetCatalog,
) -> Vec<RoomAssembly> {
    use crate::cell::CellGrid;
    use crate::level_graph::RENDER_ROOM_DEPTH;
    use crate::room_furnisher;
    use crate::room_theme;

    if let crate::level_spec::Paradigm::Fixed(env) = &spec.paradigm {
        return fixed_spawn_list(graph, env, spec.pitch, catalog);
    }

    let pitch = spec.pitch;
    let mut rooms = Vec::new();

    for (room_idx, idx) in graph.room_indices().enumerate() {
        let Some(room) = graph.room(idx) else { continue };
        let active = graph.active_connectors(idx);
        let theme = room_theme::theme_for_room(seed.value(), room_idx);
        // The level's pitch, not the wall set's: the single source (B11) —
        // the Pitch/wall-set agreement is pinned in planet.rs.
        let origin = room.world_position(pitch.tile, pitch.story);

        let mut grid = CellGrid::new(&room.template, &active, origin, pitch.tile, pitch.story);
        let room_seed = seed
            .value()
            .wrapping_add(room_idx as u64)
            .wrapping_mul(crate::seed::salt::ROOM_MIX);
        // Step 1 data — the room's shell. One paradigm per planet: the
        // megakit's layered walls on planet 1, the panel pool from planet 2
        // (cubic cells; see planet::panel_world and the B11 plan).
        let mut structure = if let crate::level_spec::Paradigm::Panel(kits) = &spec.paradigm {
            // ONE kit skins a room (visual coherence); the level mixes its
            // declared kits ACROSS rooms — the room seed picks, so the mix
            // is as reproducible as everything else it rolls. The spec
            // carries KIT IDS (scheduling facts); the plates stay owned
            // by the catalog.
            let kit = catalog.kit_def(kits[(room_seed % kits.len() as u64) as usize]);
            // Assembler v2 (role pools + course covering) the moment a
            // kit's bake provides them; the v1 one-plate-per-face path
            // carries kits the Transform hasn't served yet, and dies
            // with the pieces census when every kit has crossed.
            match kit.role_pools.as_ref() {
                Some(pools) => {
                    crate::room_assembler::assemble_role_pools_from_grid(&grid, pools, room_seed)
                }
                None => {
                    let set = kit
                        .panel_pool
                        .as_ref()
                        .expect("a panel kit has a v1 pool until its v2 bake lands");
                    crate::room_assembler::assemble_panels_from_grid(&grid, set, room_seed)
                }
            }
        } else {
            crate::room_assembler::assemble_from_grid(
                &grid,
                &room.template,
                &active,
                theme.wall_set,
                catalog,
            )
        };
        grid.populate(theme, room_seed, catalog);
        // Step 2 data — furnished fixtures (cell-rolled). The start room's
        // spawn square stays empty: the player materializes there and
        // shares it with nothing (playtest 2026-07-04).
        let mut props = grid.prop_placements();
        if room_idx == 0 {
            let (spawn, _) = spawn_pose(graph, pitch);
            let half = pitch.tile * 0.5;
            props.retain(|p| {
                (p.position[0] - spawn[0]).abs() > half
                    || (p.position[2] - spawn[2]).abs() > half
            });
        }

        let mut lights = Vec::new();
        for (mesh, light) in room_furnisher::light_fixtures(&room.template, &active, origin, pitch, room_seed, catalog) {
            // Light fixtures are part of the shell: they render in the structure
            // step and are passable, so they never join the merged collider.
            structure.push(mesh);
            lights.push(light);
        }

        // Step 2 data — organics containers, authored per template via loot
        // spawns (replaces the shell's old ad-hoc scatter). The start room's
        // spawn square is kept clear here too.
        let mut containers: Vec<[f32; 3]> = room
            .template
            .loot_spawns
            .iter()
            .map(|sp| [
                origin[0] + sp.position[0],
                origin[1] + sp.position[1],
                origin[2] + sp.position[2],
            ])
            .collect();
        if room_idx == 0 {
            let (spawn, _) = spawn_pose(graph, pitch);
            let half = pitch.tile * 0.5;
            containers.retain(|c| {
                (c[0] - spawn[0]).abs() > half || (c[2] - spawn[2]).abs() > half
            });
        }

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
        let half_cell = pitch.tile / 2.0;
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        let mut any = false;
        for cell in grid.cells() {
            any = true;
            let c = cell.world_center;
            min[0] = min[0].min(c[0] - half_cell);
            max[0] = max[0].max(c[0] + half_cell);
            min[1] = min[1].min(c[1]);
            max[1] = max[1].max(c[1] + pitch.story);
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
            shell: crate::room_assembler::shell_slabs(&grid),
        });
    }

    // Light accents by room role, sourced from the level graph (no
    // parallel structure): the start chamber (first room) reads blue;
    // the exit chamber — the farthest room, where the portal sits — and
    // everything visible through corridors from it reads red.
    let exit_red: std::collections::HashSet<usize> = graph
        .room_indices()
        .next()
        .and_then(|start| graph.exit_room(start))
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

/// The fixed-paradigm assembly: the house is ONE scene placement (room 0)
/// at the kit's declared scale — the art is already furnished and lit-able,
/// so no skinning, no props, no cell grid. Each zone contributes its
/// authored spawns, its box as bounds, and one warm ceiling light (v1).
/// Room 0 also carries the containment envelope: six slabs just outside the
/// zone-box union, because the archviz shell is leaky (recon 2026-07-11)
/// and the ship must stay inside the level however porous the art is.
/// Grammar-derived throughout — no RNG, no mesh probing.
fn fixed_spawn_list(
    graph: &LevelGraph,
    env: &crate::roster::EnvironmentDef,
    pitch: Pitch,
    catalog: &crate::asset_catalog::AssetCatalog,
) -> Vec<RoomAssembly> {
    use crate::room_assembler::{Collision, ShellSlab};
    use crate::room_furnisher::{LightAccent, LightSource, LightState};

    let scale = |k: usize| if k == 1 { pitch.story } else { pitch.tile };

    // The containment envelope hugs the authored footprint: one slab just
    // outside every unit zone-cell face not shared with another zone cell.
    // Window-tight (the archviz perimeter has ship-sized openings — leak
    // survey 2026-07-11): a player exiting through one bonks at the plane
    // of the opening instead of wandering a void between house and box.
    let mut cells = std::collections::HashSet::new();
    for zone in &env.zones {
        for x in 0..zone.extents[0] as i32 {
            for y in 0..zone.extents[1] as i32 {
                for z in 0..zone.extents[2] as i32 {
                    cells.insert([zone.min[0] + x, zone.min[1] + y, zone.min[2] + z]);
                }
            }
        }
    }
    let thickness = pitch.tile;
    let mut containment: Vec<ShellSlab> = Vec::new();
    let mut sorted: Vec<[i32; 3]> = cells.iter().copied().collect();
    sorted.sort_unstable(); // deterministic emission order
    for cell in &sorted {
        for (axis, dir) in [
            (0, [1, 0, 0]),
            (0, [-1, 0, 0]),
            (1, [0, 1, 0]),
            (1, [0, -1, 0]),
            (2, [0, 0, 1]),
            (2, [0, 0, -1]),
        ] {
            if cells.contains(&[cell[0] + dir[0], cell[1] + dir[1], cell[2] + dir[2]]) {
                continue;
            }
            // The face plane in world units, slab extending outward from it.
            let mut center = [
                (cell[0] as f32 + 0.5) * scale(0),
                (cell[1] as f32 + 0.5) * scale(1),
                (cell[2] as f32 + 0.5) * scale(2),
            ];
            let mut size = [scale(0), scale(1), scale(2)];
            let outward = dir[axis] as f32;
            center[axis] = (cell[axis] as f32 + 0.5 + 0.5 * outward) * scale(axis)
                + outward * thickness / 2.0;
            size[axis] = thickness;
            containment.push(ShellSlab { center, size });
        }
    }

    let mut rooms = Vec::new();
    for (room_idx, idx) in graph.room_indices().enumerate() {
        let Some(room) = graph.room(idx) else { continue };
        let origin = room.world_position(pitch.tile, pitch.story);
        let extents = room.template.extents;
        let bounds = RoomBounds {
            min: origin,
            max: [
                origin[0] + extents[0] as f32 * pitch.tile,
                origin[1] + extents[1] as f32 * pitch.story,
                origin[2] + extents[2] as f32 * pitch.tile,
            ],
        };

        // The whole house rides room 0: model origin IS world origin (zone
        // boxes are authored in the same model space the mesh occupies).
        let structure = if room_idx == 0 {
            vec![MeshPlacement {
                scene: catalog
                    .id_of(env.model)
                    .expect("environment scenes intern at catalog load"),
                position: [0.0, 0.0, 0.0],
                rotation_x: 0.0,
                rotation_y: 0.0,
                scale: pitch.tile,
                collision: Collision::Static,
            }]
        } else {
            Vec::new()
        };

        // Enemies from the authored spawns (start zone stays clear — the
        // linker already refuses authored spawns there).
        let mut enemies = Vec::new();
        if room_idx > 0 {
            for sp in &room.template.enemy_spawns {
                enemies.push([
                    origin[0] + sp.position[0],
                    origin[1] + sp.position[1],
                    origin[2] + sp.position[2],
                ]);
            }
        }
        let containers: Vec<[f32; 3]> = room
            .template
            .loot_spawns
            .iter()
            .map(|sp| {
                [
                    origin[0] + sp.position[0],
                    origin[1] + sp.position[1],
                    origin[2] + sp.position[2],
                ]
            })
            .collect();

        // One warm ceiling light per zone (v1): the house's own fixtures
        // are unlit art; this is the level's functional lighting.
        let state = LightState::On;
        let light = LightSource {
            position: [
                (bounds.min[0] + bounds.max[0]) / 2.0,
                bounds.max[1] - pitch.story * 0.15,
                (bounds.min[2] + bounds.max[2]) / 2.0,
            ],
            range: (bounds.max[0] - bounds.min[0]).max(bounds.max[2] - bounds.min[2]),
            energy: 1.2,
            state,
            color: LightAccent::Neutral.color(state.liveness()),
        };

        rooms.push(RoomAssembly {
            structure,
            lights: vec![light],
            props: Vec::new(),
            containers,
            enemies,
            bounds,
            shell: if room_idx == 0 { containment.clone() } else { Vec::new() },
        });
    }
    rooms
}

/// When a parent enemy's bound minions flip live.
/// Deserializes from the roster grammar (`trigger = "on_engage"`); the
/// timed form arrives as `{ every_seconds = N }` via the schema's
/// `TriggerRaw` (untagged strings-or-table).
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MinionTrigger {
    /// Activate when the parent dies.
    OnDeath,
    /// Activate the moment the parent engages — a boss's circling guard
    /// is up for the whole fight.
    OnEngage,
    /// A batch activates every N seconds while the parent lives, drawn
    /// from a capped pre-built ring (dead minions return to it — Faucet).
    #[serde(skip)]
    Every(f32),
}

/// Accumulating interval clock for timed minion emitters
/// (`trigger = {{ every_seconds = N }}`) — pure, so the cadence is testable
/// without an engine tick. Catch-up safe: a long frame yields every
/// interval it spanned.
#[derive(Debug, Clone, Copy)]
pub struct EmitterTimer {
    interval: f32,
    elapsed: f32,
}

impl EmitterTimer {
    pub fn new(interval: f32) -> Self {
        Self { interval: interval.max(f32::EPSILON), elapsed: 0.0 }
    }

    /// Advance by `dt`; returns how many intervals elapsed.
    pub fn tick(&mut self, dt: f32) -> u32 {
        self.elapsed += dt.max(0.0);
        let fires = (self.elapsed / self.interval) as u32;
        self.elapsed -= fires as f32 * self.interval;
        fires
    }
}

/// One dormant minion the manifest reserves for a parent enemy. The spawn
/// count is already expanded into one entry per minion, so the shell
/// pre-instantiates exactly `parent.minions.len()` bodies under the parent's
/// room and activates each on its trigger.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinionSpawn {
    pub enemy_type: EnemyKey,
    pub trigger: MinionTrigger,
}

/// One direct enemy spawn: its resolved type, world position, and the dormant
/// minions its death or engagement will rouse (the def's `minions` list).
#[derive(Debug, Clone, PartialEq)]
pub struct EnemySpawn {
    pub enemy_type: EnemyKey,
    pub position: [f32; 3],
    pub minions: Vec<MinionSpawn>,
}

/// The manifest's per-room slice: every direct enemy the room will hold, each
/// carrying its own dormant minions. Positions and types are already resolved,
/// so the shell parents this room's content under one container with no further
/// rolling.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RoomManifest {
    pub enemies: Vec<EnemySpawn>,
    /// Index into `enemies` of this room's dormant SEAL ANCHOR — a staged boss
    /// or a miniboss. Its room seals every active connector on entry and
    /// re-opens on the anchor's death; the anchor spawns dormant and rises on
    /// entry. `None` for an ordinary room. One field, boss and miniboss alike.
    pub anchor: Option<usize>,
}

/// A seed-deterministic enumeration of everything a level can *contain* — the
/// Faucet Principle's model half (`void-logic`), consumed by the shell's tier-1
/// pools (`void-nodes`). It resolves each direct enemy's type (the roll the
/// shell used to make inline), expands every declared minion into a dormant
/// entries bound to their parent, and — since a level drops one blue cache per
/// enemy — knows the exact cache bound. Nothing here touches Godot; the same
/// seed yields the same manifest, so the pool sizes and the bestiary coverage
/// are both derivable before a single node is instantiated.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LevelManifest {
    pub rooms: Vec<RoomManifest>,
}

impl LevelManifest {
    /// Total direct enemies across every room — the blue-cache bound (one
    /// cache per enemy) and the number of parent bodies the shell instantiates.
    pub fn enemy_count(&self) -> usize {
        self.rooms.iter().map(|r| r.enemies.len()).sum()
    }

    /// Every enemy type that can appear this level — direct spawns *and* the
    /// death-spawn minions they cough up — deduplicated, in roster declaration
    /// order. This is what the bestiary marks as seen, so a death-only type (the
    /// SpawnDrone) enters the catalog the moment a level can produce it, which
    /// `enemies_for_level` (direct-only) could never surface.
    pub fn enemy_coverage(&self, grammar: &crate::roster::Roster) -> Vec<EnemyKey> {
        let mut seen = std::collections::HashSet::new();
        for room in &self.rooms {
            for enemy in &room.enemies {
                seen.insert(enemy.enemy_type);
                for minion in &enemy.minions {
                    seen.insert(minion.enemy_type);
                }
            }
        }
        // Declaration order, deduplicated — the bestiary horizon.
        grammar.enemy_keys().filter(|k| seen.contains(k)).collect()
    }
}

pub fn manifest(
    grammar: &crate::roster::Roster,
    graph: &LevelGraph,
    spec: &crate::level_spec::LevelSpec,
    seed: Seed,
) -> LevelManifest {
    use rand::seq::IndexedRandom;
    use rand::rngs::SmallRng;
    use rand::SeedableRng;

    let rooms_assembly = spawn_list_full(graph, spec, seed, &grammar.catalog);
    let mut enemy_rng = SmallRng::seed_from_u64(seed.value());

    // A staged boss claims its arena outright: the schedule names the kind,
    // the graph marks the room, and no regular roll happens there.
    let boss_arena = graph
        .room_indices()
        .position(|idx| Some(idx) == graph.boss_room())
        .zip(spec.boss.as_ref());
    let arena_pos = boss_arena.map(|(pos, _)| pos);

    // The miniboss is placed like a boss — the MODEL picks its room, it is
    // NOT rolled into the world at random (owner 2026-07-09). The spec
    // derived WHETHER this level fields one ([`LevelSpec::miniboss`], the one
    // derivation); here it is pulled OUT of the ordinary roll and dropped,
    // dormant, into exactly ONE seed-chosen room as that room's seal anchor,
    // alongside the room's normal populace.
    let miniboss = spec.miniboss;
    let roll_pool: Vec<EnemyKey> = {
        let pool: Vec<EnemyKey> = spec
            .roster
            .iter()
            .copied()
            .filter(|k| !grammar.enemy(*k).behavior.miniboss)
            .collect();
        // A level with nothing BUT a miniboss is degenerate; fall back so the
        // roll always has something to draw.
        if pool.is_empty() { spec.roster.clone() } else { pool }
    };
    // The miniboss's room: never the start room (index 0 — the player begins
    // there, so it would seal on spawn), never the boss arena, and non-empty
    // (its anchor needs a spawn point).
    let miniboss_room = miniboss.and_then(|_| {
        let eligible: Vec<usize> = (0..rooms_assembly.len())
            .filter(|&p| p != 0 && Some(p) != arena_pos && !rooms_assembly[p].enemies.is_empty())
            .collect();
        eligible.choose(&mut enemy_rng).copied()
    });

    let rooms = rooms_assembly
        .iter()
        .enumerate()
        .map(|(room_pos, room)| {
            if let Some((arena_pos, staging)) = boss_arena {
                if room_pos == arena_pos {
                    let boss_type = staging.boss;
                    let adds = staging.adds;
                    // Escorts are declared on the boss slot (count = the
                    // staged adds); the def's own TIMED emitters reserve
                    // their rings beside them (cap slots each — the Faucet
                    // reservation, same as any enemy's declared minions).
                    let (minion_type, trigger) = staging.escorts;
                    let minions: Vec<MinionSpawn> = (0..adds)
                        .map(|_| MinionSpawn { enemy_type: minion_type, trigger })
                        .chain(
                            grammar
                                .enemy(boss_type)
                                .minions
                                .iter()
                                .filter(|m| matches!(m.trigger, MinionTrigger::Every(_)))
                                .flat_map(|m| {
                                    (0..m.cap).map(move |_| MinionSpawn {
                                        enemy_type: m.enemy,
                                        trigger: m.trigger,
                                    })
                                }),
                        )
                        .collect();
                    let enemies = room
                        .enemies
                        .iter()
                        .map(|pos| EnemySpawn {
                            enemy_type: boss_type,
                            position: *pos,
                            minions: minions.clone(),
                        })
                        .collect();
                    return RoomManifest { enemies, anchor: Some(0) };
                }
            }
            // The miniboss (if this is its chosen room) anchors index 0,
            // dormant; every other spawn point rolls an ordinary enemy from
            // the pool (the miniboss is never in that pool).
            let is_miniboss_room = Some(room_pos) == miniboss_room;
            let enemies = room
                .enemies
                .iter()
                .enumerate()
                .map(|(i, pos)| {
                    let enemy_type = if is_miniboss_room && i == 0 {
                        miniboss.expect("a miniboss room implies a declared miniboss")
                    } else {
                        *roll_pool
                            .choose(&mut enemy_rng)
                            .expect("the roll pool is non-empty for any valid level")
                    };
                    // Every declared minion entry reserves its RING (cap
                    // slots; one-shot entries have cap == count) with its
                    // declared trigger — the grammar's list is the Faucet
                    // reservation, timed emitters included.
                    let minions = grammar
                        .enemy(enemy_type)
                        .minions
                        .iter()
                        .flat_map(|m| {
                            (0..m.cap).map(move |_| MinionSpawn {
                                enemy_type: m.enemy,
                                trigger: m.trigger,
                            })
                        })
                        .collect();
                    EnemySpawn { enemy_type, position: *pos, minions }
                })
                .collect();
            RoomManifest { enemies, anchor: is_miniboss_room.then_some(0) }
        })
        .collect();

    LevelManifest { rooms }
}

/// Where the run begins: the center of the start room's ground story at
/// flight height, yawed to face the room's first doorway. A first-cell,
/// corner-facing spawn reads as disorientation (playtest 2026-07-04).
pub fn spawn_pose(graph: &LevelGraph, pitch: Pitch) -> ([f32; 3], f32) {
    let Some(idx) = graph.room_indices().next() else { return ([0.0; 3], 0.0) };
    let Some(room) = graph.room(idx) else { return ([0.0; 3], 0.0) };
    let origin = room.world_position(pitch.tile, pitch.story);
    let [ex, _ey, ez] = room.template.extents;
    let pos = [
        origin[0] + ex as f32 * pitch.tile * 0.5,
        origin[1] + 1.5,
        origin[2] + ez as f32 * pitch.tile * 0.5,
    ];
    // Face the first doorway: -Z rotated by yaw must point from the spawn
    // toward the connector cell.
    let yaw = graph
        .active_connectors(idx)
        .first()
        .map(|c| {
            let cx = origin[0] + (c.offset[0] as f32 + 0.5) * pitch.tile;
            let cz = origin[2] + (c.offset[2] as f32 + 0.5) * pitch.tile;
            let (dx, dz) = (cx - pos[0], cz - pos[2]);
            if dx.abs() + dz.abs() < 1e-3 {
                0.0
            } else {
                (-dx).atan2(-dz)
            }
        })
        .unwrap_or(0.0);
    (pos, yaw)
}

/// The capture rig's reproducible artery: one pose per node on the
/// start→exit path (rooms AND corridors), each at its node's volumetric
/// center, yawed at its successor (the [`spawn_pose`] facing convention)
/// and pitched to track climbs. Angles in DEGREES — rig-ready
/// (`--shot=x,y,z,yaw,pitch;…`).
pub fn flythrough_poses(graph: &LevelGraph, pitch: Pitch) -> Vec<[f32; 5]> {
    let Some(start) = graph.room_indices().next() else { return Vec::new() };
    let Some(exit) = graph.exit_room(start) else { return Vec::new() };

    let centers: Vec<[f32; 3]> = graph
        .path_between(start, exit)
        .into_iter()
        .filter_map(|n| graph.room(n))
        .map(|room| {
            let origin = room.world_position(pitch.tile, pitch.story);
            let [ex, ey, ez] = room.template.extents;
            [
                origin[0] + ex as f32 * pitch.tile * 0.5,
                origin[1] + ey as f32 * pitch.story * 0.5,
                origin[2] + ez as f32 * pitch.tile * 0.5,
            ]
        })
        .collect();

    centers
        .iter()
        .enumerate()
        .map(|(i, &pos)| {
            // Look along the travel direction: at the next center, or —
            // for the final pose — onward from the previous one.
            let (from, to) = if i + 1 < centers.len() {
                (pos, centers[i + 1])
            } else if i > 0 {
                (centers[i - 1], pos)
            } else {
                return [pos[0], pos[1], pos[2], 0.0, 0.0];
            };
            let (dx, dy, dz) = (to[0] - from[0], to[1] - from[1], to[2] - from[2]);
            let horiz = (dx * dx + dz * dz).sqrt();
            let yaw = if horiz < 1e-3 { 0.0 } else { (-dx).atan2(-dz).to_degrees() };
            let pitch_deg = dy.atan2(horiz).to_degrees();
            [pos[0], pos[1], pos[2], yaw, pitch_deg]
        })
        .collect()
}

/// Exterior establishing shots for the capture rig: pull-backs derived
/// from the level's world AABB, aimed at its center — a high diagonal and
/// a top-down. Same degree/pose format as [`flythrough_poses`].
pub fn establishing_poses(graph: &LevelGraph, pitch: Pitch) -> Vec<[f32; 5]> {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    let mut any = false;
    for n in graph.room_indices() {
        let Some(room) = graph.room(n) else { continue };
        any = true;
        let origin = room.world_position(pitch.tile, pitch.story);
        let [ex, ey, ez] = room.template.extents;
        let max = [
            origin[0] + ex as f32 * pitch.tile,
            origin[1] + ey as f32 * pitch.story,
            origin[2] + ez as f32 * pitch.tile,
        ];
        for i in 0..3 {
            lo[i] = lo[i].min(origin[i]);
            hi[i] = hi[i].max(max[i]);
        }
    }
    if !any {
        return Vec::new();
    }
    let center = [
        (lo[0] + hi[0]) * 0.5,
        (lo[1] + hi[1]) * 0.5,
        (lo[2] + hi[2]) * 0.5,
    ];
    let diag = ((hi[0] - lo[0]).powi(2) + (hi[1] - lo[1]).powi(2) + (hi[2] - lo[2]).powi(2))
        .sqrt()
        .max(pitch.tile);

    // Aim a pose at the center from `pos`: yaw about Y (the spawn_pose
    // convention), pitch raising the nose with +Y.
    let aim = |pos: [f32; 3]| -> [f32; 5] {
        let (dx, dy, dz) = (center[0] - pos[0], center[1] - pos[1], center[2] - pos[2]);
        let horiz = (dx * dx + dz * dz).sqrt();
        let yaw = if horiz < 1e-3 { 0.0 } else { (-dx).atan2(-dz).to_degrees() };
        let pitch_deg = dy.atan2(horiz).to_degrees();
        [pos[0], pos[1], pos[2], yaw, pitch_deg]
    };

    vec![
        // High diagonal: outside the AABB along (1, 0.6, 1), a full
        // diagonal out — frames the whole cluster.
        aim([
            hi[0] + diag * 0.7,
            hi[1] + diag * 0.45,
            hi[2] + diag * 0.7,
        ]),
        // Top-down plan view.
        aim([center[0], hi[1] + diag, center[2]]),
    ]
}

/// Deterministic interior vantages for the visual containment audit: up
/// to `budget` poses on room INNER cells (a full tile clear of every XZ
/// wall — the vantage carries a chase camera), spread across the level's
/// rooms, each yawed toward its room's center. "Inside the level" is
/// derived — cell centers of placed rooms — never authored. Same pose
/// format as [`flythrough_poses`].
pub fn interior_probe_poses(graph: &LevelGraph, pitch: Pitch, budget: usize) -> Vec<[f32; 5]> {
    // Rooms only (a corridor vantage is a degenerate close-up), largest
    // first so the budget favors the spaces the player actually fights in.
    let mut rooms: Vec<_> = graph
        .room_indices()
        .filter_map(|n| graph.room(n))
        .filter(|r| r.template.kind == crate::room_template::TemplateKind::Room)
        .collect();
    rooms.sort_by_key(|r| {
        let [ex, ey, ez] = r.template.extents;
        std::cmp::Reverse((ex * ey * ez, r.grid_pos))
    });
    if rooms.is_empty() || budget == 0 {
        return Vec::new();
    }

    // Round-robin the budget across rooms; within a room, walk the INNER
    // cell diagonal (distinct cells for successive visits), looking at
    // the room's center.
    let mut out = Vec::with_capacity(budget);
    let mut round = 0;
    while out.len() < budget {
        let mut placed_this_round = false;
        for room in &rooms {
            if out.len() >= budget {
                break;
            }
            let [ex, ey, ez] = room.template.extents;
            // Inner cells: 1..extent-1 per XZ axis; a 1- or 2-wide room
            // has no camera-safe interior and hosts no probe.
            let inner = (ex.min(ez) as usize).saturating_sub(2);
            if round >= inner {
                continue;
            }
            let origin = room.world_position(pitch.tile, pitch.story);
            let (cx, cz) = (1.0 + round as f32, 1.0 + round as f32);
            let cy = (round % ey as usize) as f32;
            let pos = [
                origin[0] + (cx + 0.5) * pitch.tile,
                origin[1] + (cy + 0.5) * pitch.story,
                origin[2] + (cz + 0.5) * pitch.tile,
            ];
            let center = [
                origin[0] + ex as f32 * pitch.tile * 0.5,
                origin[2] + ez as f32 * pitch.tile * 0.5,
            ];
            let (dx, dz) = (center[0] - pos[0], center[1] - pos[2]);
            let yaw = if dx.abs() + dz.abs() < 1e-3 {
                (round % 4) as f32 * 90.0
            } else {
                (-dx).atan2(-dz).to_degrees()
            };
            out.push([pos[0], pos[1], pos[2], yaw, 0.0]);
            placed_this_round = true;
        }
        if !placed_this_round {
            break; // every room's safe interior is exhausted
        }
        round += 1;
    }
    out
}



#[cfg(test)]
mod emitter_tests {
    use super::EmitterTimer;

    #[test]
    fn the_emitter_fires_on_its_cadence_and_not_before() {
        let mut t = EmitterTimer::new(10.0);
        assert_eq!(t.tick(9.9), 0, "no early fire");
        assert_eq!(t.tick(0.1), 1, "fires exactly on the interval");
        assert_eq!(t.tick(9.9), 0, "the clock reset — no residue fire");
    }

    #[test]
    fn a_long_frame_yields_every_interval_it_spanned() {
        let mut t = EmitterTimer::new(5.0);
        assert_eq!(t.tick(17.5), 3, "catch-up: three intervals in one frame");
        assert_eq!(t.tick(2.5), 1, "the 2.5s remainder carried over");
    }

    #[test]
    fn degenerate_inputs_never_wedge_the_clock() {
        let mut t = EmitterTimer::new(0.0); // clamped to epsilon internally
        assert!(t.tick(0.016) > 0, "a zero interval still fires");
        let mut t = EmitterTimer::new(10.0);
        assert_eq!(t.tick(-1.0), 0, "negative dt is ignored, not banked");
    }
}

#[cfg(test)]
mod tests {
    use crate::test_fixtures::{cat, spath};

    /// Attribute spec for tests: fresh profile, pinned run seed. The
    /// GENERATION seed still travels separately.
    fn spec_for(level: u32) -> crate::level_spec::LevelSpec {
        crate::level_spec::LevelSpec::for_level(
            roster(),
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

    use crate::generator::{generate, GeneratorConfig};
    use crate::level_graph::{EdgeKind, RENDER_ROOM_DEPTH};
    use crate::room_template::ConnectorFacing;
    use crate::seed::Seed;

    #[test]
    fn spawn_pose_centers_the_start_room_facing_its_doorway() {
        for seed in 0..10u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let cell = 4.0;
            let (pos, yaw) = spawn_pose(&graph, TEST_PITCH);

            // Centered on the start room's footprint, at flight height.
            let idx = graph.room_indices().next().unwrap();
            let room = graph.room(idx).unwrap();
            let story = 5.0; // planet-1 story — tests may hold literals
            let origin = room.world_position(cell, story);
            let [ex, _ey, ez] = room.template.extents;
            assert!((pos[0] - (origin[0] + ex as f32 * cell * 0.5)).abs() < 0.01,
                "seed {seed}: spawn X centers the room");
            assert!((pos[2] - (origin[2] + ez as f32 * cell * 0.5)).abs() < 0.01,
                "seed {seed}: spawn Z centers the room");
            assert!(pos[1] > origin[1], "seed {seed}: spawn floats above the floor");

            // Facing the first doorway, not a corner: the yaw's forward
            // (-Z rotated by yaw) points at the connector.
            let connectors = graph.active_connectors(idx);
            let Some(c) = connectors.first() else { continue };
            let cx = origin[0] + (c.offset[0] as f32 + 0.5) * cell;
            let cz = origin[2] + (c.offset[2] as f32 + 0.5) * cell;
            let (dx, dz) = (cx - pos[0], cz - pos[2]);
            let len = (dx * dx + dz * dz).sqrt();
            if len < 0.1 { continue; } // doorway dead-center: any yaw works
            let forward = (-yaw.sin(), -yaw.cos());
            let dot = forward.0 * dx / len + forward.1 * dz / len;
            assert!(dot > 0.99,
                "seed {seed}: spawn must face its doorway (dot {dot})");
        }
    }

    #[test]
    fn the_spawn_square_holds_nothing_but_the_player() {
        for seed in 0..10u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let cell = 4.0;
            let (pos, _) = spawn_pose(&graph, TEST_PITCH);
            let rooms = spawn_list_full(&graph, &spec_for(1), Seed::new(seed), cat());
            let start = &rooms[0];
            let half = cell * 0.5;
            for p in &start.props {
                let (dx, dz) = ((p.position[0] - pos[0]).abs(), (p.position[2] - pos[2]).abs());
                assert!(dx > half || dz > half,
                    "seed {seed}: prop at ({}, {}) squats in the spawn square",
                    p.position[0], p.position[2]);
            }
            for c in &start.containers {
                let (dx, dz) = ((c[0] - pos[0]).abs(), (c[2] - pos[2]).abs());
                assert!(dx > half || dz > half,
                    "seed {seed}: container at ({}, {}) squats in the spawn square", c[0], c[2]);
            }
        }
    }

    /// End-to-end: through the full generation pipeline, a real level's
    /// vertical shafts are square (no rounded corner pieces) and lit by rim
    /// fixtures (their ceilings are all open, so any light they carry is a
    /// rim light). Proves both features survive generation — not just the
    /// synthetic unit cases — so "I can't see them" is a findability matter,
    /// not a rendering gap.
    #[test]
    fn generated_vertical_shafts_are_square_and_rim_lit() {
        let mut square_shaft_seen = false;
        let mut rim_lit_shaft_seen = false;

        'seeds: for seed in 0..30u64 {
            let config = GeneratorConfig {
                pitch: crate::planet::Pitch { tile: 4.0, story: 5.0 },
                seed: Seed::new(seed),
                max_rooms: 30,
                min_room_xz: 3,
                max_room_xz: 6,
                min_room_y: 1,
                max_room_y: 6,
            };
            let Ok(graph) = generate(&config) else { continue };
            let assemblies = spawn_list_full(&graph, &spec_for(1), Seed::new(seed), cat());

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
                    asm.structure.iter().filter(|m| spath(m.scene).contains("Corner_Round")).count(),
                    0,
                    "seed {seed}: vertical shaft still has rounded corner pieces"
                );
                if asm.structure.iter().any(|m| spath(m.scene).contains("_Straight")) {
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
        let story = 5.0_f32; // planet-1 story — tests may hold literals
        let mut passages_checked = 0u32;

        for seed in 0..30u64 {
            let config = GeneratorConfig {
                pitch: crate::planet::Pitch { tile: 4.0, story: 5.0 },
                seed: Seed::new(seed),
                max_rooms: 20,
                min_room_xz: 3,
                max_room_xz: 6,
                min_room_y: 1,
                max_room_y: 6,
            };
            let Ok(graph) = generate(&config) else { continue };
            let (meshes, _lights) = spawn_list(&graph, &spec_for(1), Seed::new(seed), cat());

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
                    if !spath(placement.scene).contains("Platform") {
                        continue; // only floor/ceiling slabs can cap the hole
                    }
                    let [px, py, pz] = placement.position;
                    let in_footprint = px > x0 && px < x1 && pz > z0 && pz < z1;
                    let on_plane = (py - plane_y).abs() < 1.0;
                    assert!(
                        !(in_footprint && on_plane),
                        "seed {seed}: '{}' at {:?} caps the vertical aperture \
                         (footprint x[{x0:.1},{x1:.1}] z[{z0:.1},{z1:.1}] plane y {plane_y:.1})",
                        spath(placement.scene),
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
            pitch: crate::planet::Pitch { tile: 4.0, story: 5.0 },
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
        let rooms = spawn_list_full(&graph, &spec_for(1), Seed::new(7), cat());
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
            let rooms = spawn_list_full(&graph, &spec_for(1), Seed::new(seed), cat());
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
            let rooms = spawn_list_full(&graph, &spec_for(1), Seed::new(seed), cat());
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
        let rooms = spawn_list_full(&graph, &spec_for(1), Seed::new(7), cat());
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
        let rooms = spawn_list_full(&graph, &spec_for(1), Seed::new(7), cat());
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
            let rooms = spawn_list_full(&graph, &spec_for(1), Seed::new(seed), cat());
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
            let rooms = spawn_list_full(&graph, &spec_for(1), Seed::new(seed), cat());
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

    // --- Level manifest (Faucet Principle, tier-1 derivation) ---

    /// A seed is a pure function into a manifest: two derivations from the same
    /// inputs are byte-identical, so the pool sizes and enemy mix the shell
    /// builds are reproducible.
    #[test]
    fn planet_two_rooms_are_skinned_with_panels_not_megakit() {
        // B11: planet 2+ structure comes from the panel pool — wholesale,
        // no megakit walls (one paradigm per planet, owner's call).
        let config = config_for(1, crate::generator::rooms_for_level(7), 7);
        let graph = generate(&config).expect("generates");
        let rooms = spawn_list_full(&graph, &spec_for(7), Seed::new(1), cat());
        // The level's wall pools, as the grammar derives them (census ×
        // policy) — the only panels allowed on structure.
        let grammar = crate::roster::roster();
        let mut pool_scenes: std::collections::HashSet<crate::asset_catalog::SceneId> =
            std::collections::HashSet::new();
        for &id in &grammar.panel_kits_for_level(7) {
            let kit = grammar.catalog.kit_def(id);
            if let Some(p) = kit.panel_pool.as_ref() {
                pool_scenes.extend(p.plates.iter().map(|pl| pl.scene));
            }
            if let Some(rp) = kit.role_pools.as_ref() {
                pool_scenes.extend(
                    rp.floor
                        .iter()
                        .chain(&rp.ceiling)
                        .chain(&rp.wall)
                        .map(|pl| pl.scene),
                );
            }
        }
        let mut any_panel = false;
        for room in &rooms {
            for m in &room.structure {
                // Light FIXTURES (props) may stay megakit for now — the ban
                // is on structural skin: walls, platforms, corners, trims.
                assert!(
                    !spath(m.scene).contains("megakit/walls")
                        && !spath(m.scene).contains("megakit/platforms"),
                    "planet 2 must not place megakit structure: {}",
                    spath(m.scene)
                );
                if spath(m.scene).contains("addons/walls") {
                    assert!(
                        pool_scenes.contains(&m.scene),
                        "{} skins a wall but is not in the derived pools \
                         (truss? strip? the census said no)",
                        spath(m.scene)
                    );
                    any_panel = true;
                }
            }
        }
        assert!(any_panel, "planet 2 rooms are skinned from the panel pool");
    }

    #[test]
    fn planet_one_keeps_the_megakit() {
        let config = config_for(1, crate::generator::rooms_for_level(1), 1);
        let graph = generate(&config).expect("generates");
        let rooms = spawn_list_full(&graph, &spec_for(1), Seed::new(1), cat());
        let any_megakit = rooms.iter().flat_map(|r| &r.structure)
            .any(|m| spath(m.scene).contains("quaternius"));
        assert!(any_megakit, "planet 1 stays terrestrial megakit");
    }

    #[test]
    fn manifest_is_seed_deterministic() {
        for seed in 0..20u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let a = manifest(roster(), &graph, &spec_for(5), Seed::new(seed));
            let b = manifest(roster(), &graph, &spec_for(5), Seed::new(seed));
            assert_eq!(a, b, "seed {seed}: manifest not deterministic");
        }
    }

    /// The manifest positions match the assembly's enemy positions one-for-one,
    /// in the same room-then-position order — the manifest resolves *types* onto
    /// the assembly's spawn points, it does not invent or drop any.
    #[test]
    fn manifest_covers_every_assembly_enemy_position() {
        for seed in 0..20u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let assembly = spawn_list_full(&graph, &spec_for(1), Seed::new(seed), cat());
            let m = manifest(roster(), &graph, &spec_for(5), Seed::new(seed));
            assert_eq!(m.rooms.len(), assembly.len(), "seed {seed}: room count differs");
            for (room, room_asm) in m.rooms.iter().zip(&assembly) {
                let positions: Vec<_> = room.enemies.iter().map(|e| e.position).collect();
                assert_eq!(positions, room_asm.enemies, "seed {seed}: positions differ");
            }
        }
    }

    /// Every enemy the manifest places carries exactly its def's declared
    /// minion expansion — Σ cap dormant slots, each matching a declared
    /// (type, trigger) — and types declaring none carry none. Derived from
    /// the grammar, never from named defs: the TOML is the owner's to
    /// retune (feedback 2026-07-06).
    #[test]
    fn manifest_expands_declared_minions() {
        let mut saw_declaring_parent = false;
        for level in 1..=6u32 {
            for seed in 0..10u64 {
                let Ok(graph) = generate(&test_config(seed)) else { continue };
                let m = manifest(roster(), &graph, &spec_for(level), Seed::new(seed));
                for room in &m.rooms {
                    for enemy in &room.enemies {
                        let declared = &roster().enemy(enemy.enemy_type).minions;
                        if declared.is_empty() {
                            assert!(enemy.minions.is_empty(),
                                "{:?} declares no minions but carries some", enemy.enemy_type);
                            continue;
                        }
                        saw_declaring_parent = true;
                        let total: usize = declared.iter().map(|m| m.cap as usize).sum();
                        assert_eq!(enemy.minions.len(), total,
                            "{:?} must reserve exactly its declared cap total", enemy.enemy_type);
                        for mn in &enemy.minions {
                            assert!(
                                declared.iter().any(|d| d.enemy == mn.enemy_type
                                    && d.trigger == mn.trigger),
                                "{:?} expanded a (type, trigger) its def never declared",
                                enemy.enemy_type,
                            );
                        }
                    }
                }
            }
        }
        // Non-vacuity, itself derived: demanded only while some scanned
        // roster actually stocks a minion-declaring def.
        let eligible = (1..=6u32).any(|lvl| spec_for(lvl).roster.iter()
            .any(|id| !roster().enemy(*id).minions.is_empty()));
        if eligible {
            assert!(saw_declaring_parent,
                "a minion-declaring def is rostered but never placed — widen the scan");
        }
    }

    // --- Boss staging (B5) ---

    /// Seed 1 with the arena attached — the exact graph the shell builds on a
    /// boss level (B6's GUT scenario mirrors this construction).
    fn pinned_boss_graph(level: u32) -> LevelGraph {
        let config = config_for(1, crate::generator::rooms_for_level(level), level);
        let mut graph = generate(&config).expect("pinned seed generates");
        let entry = graph.room_indices().next().expect("has rooms");
        crate::spatial_layout::attach_boss_room(&mut graph, entry, TEST_PITCH).expect("arena attaches");
        graph
    }

    fn arena_position(graph: &LevelGraph) -> usize {
        graph
            .room_indices()
            .position(|i| Some(i) == graph.boss_room())
            .expect("boss room is in the graph")
    }

    /// A level the campaign stages a boss on — found, never pinned, so
    /// retuning which levels carry bosses can't move these contracts.
    fn first_boss_level() -> u32 {
        (1..=24u32)
            .find(|l| spec_for(*l).boss.is_some())
            .expect("the campaign stages a boss in the first 24 levels")
    }

    #[test]
    fn a_staged_boss_stands_alone_in_the_arena_with_its_declared_escorts() {
        let level = first_boss_level();
        let spec = spec_for(level);
        let staging = spec.boss.as_ref().expect("first_boss_level stages a fight");
        let graph = pinned_boss_graph(level);
        let m = manifest(roster(), &graph, &spec, Seed::new(1));
        let arena = &m.rooms[arena_position(&graph)];
        assert_eq!(arena.enemies.len(), 1, "the arena holds the boss, nothing else");
        let boss = &arena.enemies[0];
        assert_eq!(boss.enemy_type, staging.boss, "the arena stages the spec's declared boss");
        // The slot's escorts reserve on the boss (Faucet: cap slots pre-built);
        // their count is the slot's declared call, never a formula.
        let escorts = boss.minions.iter()
            .filter(|mn| matches!(mn.trigger, MinionTrigger::OnDeath | MinionTrigger::OnEngage))
            .count();
        assert_eq!(escorts, staging.adds as usize,
            "the reserved escorts are exactly the slot's declared count");
        assert!(escorts > 0, "a staged fight reserves escorts");
    }

    #[test]
    fn bosses_live_in_the_arena_only() {
        let level = first_boss_level();
        let spec = spec_for(level);
        let staging = spec.boss.as_ref().expect("first_boss_level stages a fight");
        let graph = pinned_boss_graph(level);
        let m = manifest(roster(), &graph, &spec, Seed::new(1));
        let arena_pos = arena_position(&graph);
        for (pos, room) in m.rooms.iter().enumerate() {
            if pos == arena_pos {
                continue;
            }
            for enemy in &room.enemies {
                assert_ne!(enemy.enemy_type, staging.boss,
                    "room {pos}: the boss lives in the arena only");
            }
        }
    }

    #[test]
    fn no_arena_marker_means_no_boss() {
        // The shell only attaches the arena on boss levels; without the marker
        // (or if the attach failed), the manifest must not invent the boss.
        let level = first_boss_level();
        let spec = spec_for(level);
        let staging = spec.boss.as_ref().expect("first_boss_level stages a fight");
        let config = config_for(1, crate::generator::rooms_for_level(level), level);
        let graph = generate(&config).expect("generates");
        let m = manifest(roster(), &graph, &spec, Seed::new(1));
        for room in &m.rooms {
            for enemy in &room.enemies {
                assert_ne!(enemy.enemy_type, staging.boss, "no arena marker — no boss");
            }
        }
    }

    #[test]
    fn a_staged_boss_enters_bestiary_coverage() {
        let level = first_boss_level();
        let spec = spec_for(level);
        let staging = spec.boss.as_ref().expect("first_boss_level stages a fight");
        let graph = pinned_boss_graph(level);
        let m = manifest(roster(), &graph, &spec, Seed::new(1));
        assert!(m.enemy_coverage(roster()).contains(&staging.boss),
            "the bestiary logs the boss the level it can appear");
    }

    #[test]
    fn a_miniboss_level_seals_exactly_one_room_around_the_miniboss() {
        // Fixture-driven: the test grammar GUARANTEES level 1 fields a
        // miniboss beside a regular — rosters/ is the owner's tuning data
        // and never a test dependency (owner 2026-07-09). Across seeds: the
        // miniboss lands in exactly ONE room as its dormant anchor, never
        // the start room (it would seal on spawn), and NOWHERE else (placed,
        // never rolled).
        let grammar = crate::test_fixtures::fixture_grammar();
        let spec = crate::level_spec::LevelSpec::for_level(
            &grammar,
            Seed::new(1),
            1,
            &crate::unlocks::PermanentUnlocks::new(),
        );
        let mb = spec.miniboss.expect("the fixture's level 1 declares a miniboss");
        let mut proved = 0;
        for seed in 0..10u64 {
            let config = GeneratorConfig::for_spec(&spec, Seed::new(seed));
            let Ok(graph) = generate(&config) else { continue };
            let m = manifest(&grammar, &graph, &spec, Seed::new(seed));
            let anchored: Vec<usize> = m
                .rooms
                .iter()
                .enumerate()
                .filter(|(_, room)| room.anchor.is_some())
                .map(|(pos, _)| pos)
                .collect();
            assert_eq!(anchored.len(), 1, "seed {seed}: exactly one sealed room");
            let room = &m.rooms[anchored[0]];
            let a = room.anchor.expect("the sealed room carries its anchor");
            assert_eq!(room.enemies[a].enemy_type, mb,
                "seed {seed}: the anchor IS the miniboss");
            assert_ne!(anchored[0], 0,
                "seed {seed}: never the start room — it would seal on spawn");
            let mb_count = m
                .rooms
                .iter()
                .flat_map(|r| r.enemies.iter())
                .filter(|e| e.enemy_type == mb)
                .count();
            assert_eq!(mb_count, 1, "seed {seed}: placed once, never also rolled");
            proved += 1;
        }
        assert!(proved > 0, "at least one seed must generate");
    }

    #[test]
    fn pinned_gut_run_anchors_a_miniboss_room() {
        // GUT mirror: test_miniboss_seal.gd installs the FIXTURE grammar and
        // runs fixed_seed = 1, start_level = 1 — this pins that that exact
        // build (for_spec + the run seed's for_level derivation) anchors a
        // miniboss room. Fixture-owned: the owner's tuning can never move it.
        let grammar = crate::test_fixtures::fixture_grammar();
        let run_seed = Seed::from_i64(1);
        let spec = crate::level_spec::LevelSpec::for_level(
            &grammar,
            run_seed,
            1,
            &crate::unlocks::PermanentUnlocks::new(),
        );
        assert!(spec.miniboss.is_some(), "the fixture's level 1 declares a miniboss");
        let level_seed = run_seed.for_level(1);
        let graph = generate(&GeneratorConfig::for_spec(&spec, level_seed))
            .expect("the pinned run's level generates");
        let m = manifest(&grammar, &graph, &spec, level_seed);
        assert_eq!(
            m.rooms.iter().filter(|r| r.anchor.is_some()).count(),
            1,
            "the pinned run anchors exactly one miniboss room"
        );
    }

    /// Cache bound = one per direct enemy = total enemy count. The minions
    /// don't add caches (they're bound to their parent's), so the count is
    /// the sum of direct enemies only.
    // --- Pinned seeds for the GUT shell suite ---
    //
    // The shell tests (godot/tests) build exactly ONE level per scenario:
    // whether a seed produces a given property is a pure model question and
    // is pinned here, through the same `GeneratorConfig::standard` path the
    // shell builds with. If generation changes and one of these fails, fix
    // the constant here AND its mirror in the named GUT file — never by
    // reintroducing a seed scan on the engine side.

    /// Mirror: godot/tests/test_faucet_pools.gd `MINION_PARENT_SEED`.
    /// Seed 1 at level 4 (8 rooms) fields at least one enemy whose def
    /// declares minions — the GUT suite builds this exact level and pins
    /// the dormant reservation GENERICALLY, never by def name (feedback
    /// 2026-07-06: the TOML is the owner's to retune). If a retune empties
    /// the scanned roster of minions, re-pin the scenario here and in the
    /// GUT constants together.
    #[test]
    fn pinned_gut_seed_places_a_minion_declaring_parent() {
        // GUT mirror: test_faucet_pools.gd MINION_PARENT_SEED/LEVEL, run on
        // the FIXTURE grammar (owner 2026-07-09: mechanism scenarios never
        // depend on rosters/ tuning). The scenario KILLS this parent and
        // counts the brood flips, so the pin is a NON-ANCHOR parent with an
        // ON-DEATH brood — the miniboss anchor's timed ring must never be
        // what satisfies it. Budget 8 mirrors the GUT call's target_rooms.
        let grammar = crate::test_fixtures::fixture_grammar();
        let seed = Seed::from_i64(1);
        let mut spec = crate::level_spec::LevelSpec::for_level(
            &grammar,
            seed,
            1,
            &crate::unlocks::PermanentUnlocks::new(),
        );
        spec.room_budget = 8;
        let graph = generate(&crate::generator::GeneratorConfig::for_spec(&spec, seed))
            .expect("the pinned seed must generate");
        let m = manifest(&grammar, &graph, &spec, seed);
        assert!(
            m.rooms.iter().any(|r| {
                r.enemies.iter().enumerate().any(|(i, e)| {
                    r.anchor != Some(i)
                        && e.minions.iter().any(|mn| mn.trigger == MinionTrigger::OnDeath)
                })
            }),
            "fixture seed 1 must field a non-anchor on-death-brood parent — the GUT suite kills this exact parent"
        );
    }

    /// Mirror: godot/tests/test_faucet_pools.gd `GREEN_CACHE_RUN_SEED`, run
    /// on the FIXTURE grammar. A run with fixed_seed 1 places at least one
    /// loot container (a green cache) on level 1 via GameManager's
    /// run-seed → level-seed derivation.
    #[test]
    fn pinned_gut_run_seed_places_a_loot_container() {
        let grammar = crate::test_fixtures::fixture_grammar();
        let level_seed = Seed::from_i64(1).for_level(1);
        let spec = crate::level_spec::LevelSpec::for_level(
            &grammar,
            Seed::from_i64(1),
            1,
            &crate::unlocks::PermanentUnlocks::new(),
        );
        let graph = generate(&crate::generator::GeneratorConfig::for_spec(&spec, level_seed))
            .expect("the pinned seed must generate");
        let rooms = spawn_list_full(&graph, &spec, level_seed, cat());
        assert!(
            rooms.iter().any(|r| !r.containers.is_empty()),
            "fixture run seed 1 must place a green cache on level 1 — the GUT suite drives this run"
        );
    }

    #[test]
    fn manifest_cache_bound_is_one_per_enemy() {
        for seed in 0..20u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let m = manifest(roster(), &graph, &spec_for(5), Seed::new(seed));
            let direct: usize = m.rooms.iter().map(|r| r.enemies.len()).sum();
            assert_eq!(m.enemy_count(), direct);
        }
    }

    /// Coverage includes death-spawn-only types once the level can produce
    /// them — via ANY covered parent whose def declares them — which the
    /// direct roster never surfaces. This is what fixes the bestiary gap.
    /// Derived from the grammar, never from named defs (feedback
    /// 2026-07-06).
    #[test]
    fn manifest_coverage_includes_death_spawn_types_at_level() {
        let mut saw_declaring_parent = false;
        for level in 1..=6u32 {
            for seed in 0..10u64 {
                let Ok(graph) = generate(&test_config(seed)) else { continue };
                let m = manifest(roster(), &graph, &spec_for(level), Seed::new(seed));
                let coverage = m.enemy_coverage(roster());
                let mut sorted = coverage.clone();
                sorted.dedup();
                assert_eq!(sorted, coverage, "coverage must be deduplicated");
                for id in &coverage {
                    let declared = &roster().enemy(*id).minions;
                    if declared.is_empty() {
                        continue;
                    }
                    saw_declaring_parent = true;
                    for d in declared {
                        assert!(
                            coverage.contains(&d.enemy),
                            "seed {seed} level {level}: {:?} covered but its declared minion {:?} missing",
                            id, d.enemy,
                        );
                    }
                }
            }
        }
        let eligible = (1..=6u32).any(|lvl| spec_for(lvl).roster.iter()
            .any(|id| !roster().enemy(*id).minions.is_empty()));
        if eligible {
            assert!(saw_declaring_parent,
                "a minion-declaring def is rostered but never covered — widen the scan");
        }
    }

    /// Coverage never invents types: every covered id is in the level's
    /// roster or declared as a minion by another covered def. The
    /// complement of the inclusion test above — derived, never named.
    #[test]
    fn manifest_coverage_never_exceeds_the_rosters_reach() {
        for level in 1..=6u32 {
            for seed in 0..10u64 {
                let Ok(graph) = generate(&test_config(seed)) else { continue };
                let spec = spec_for(level);
                let coverage = manifest(roster(), &graph, &spec, Seed::new(seed)).enemy_coverage(roster());
                for id in &coverage {
                    let direct = spec.roster.contains(id);
                    let via_parent = coverage.iter().any(|p| roster()
                        .enemy(*p)
                        .minions
                        .iter()
                        .any(|m| m.enemy == *id));
                    assert!(direct || via_parent,
                        "seed {seed} level {level}: {:?} covered but neither rostered nor declared by a covered parent",
                        id);
                }
            }
        }
    }

    // ── Fixed-paradigm assembly: the authored house, one scene, zone
    //    bounds, containment — all against the canonical fixed fixture. ──

    fn fixed_fixture() -> (crate::roster::Roster, crate::level_spec::LevelSpec) {
        let grammar = crate::test_fixtures::fixed_fixture_grammar();
        let spec = crate::level_spec::LevelSpec::for_level(
            &grammar,
            Seed::new(1),
            1,
            &crate::unlocks::PermanentUnlocks::new(),
        );
        (grammar, spec)
    }

    fn fixed_env(spec: &crate::level_spec::LevelSpec) -> &crate::roster::EnvironmentDef {
        let crate::level_spec::Paradigm::Fixed(env) = &spec.paradigm else {
            panic!("the fixture spec is fixed-paradigm");
        };
        env
    }

    #[test]
    fn fixed_assembly_places_the_scene_once_at_kit_scale() {
        let (_, spec) = fixed_fixture();
        let env = fixed_env(&spec).clone();
        let graph = crate::generator::generate_for_spec(&spec, Seed::new(1), false).unwrap();
        let rooms = spawn_list_full(&graph, &spec, Seed::new(1), cat());
        let placements: Vec<&MeshPlacement> =
            rooms.iter().flat_map(|r| &r.structure).collect();
        assert_eq!(placements.len(), 1, "ONE scene placement for the whole house");
        let house = placements[0];
        assert_eq!(
            house.scene,
            cat().id_of(env.model).expect("environment scene interned"),
            "the environment's installed scene"
        );
        assert_eq!(house.position, [0.0, 0.0, 0.0], "model origin IS world origin");
        assert_eq!(house.scale, spec.pitch.tile, "the kit's declared scale");
        assert_eq!(
            house.collision,
            crate::room_assembler::Collision::Static,
            "the house is the physics"
        );
        assert!(!rooms[0].structure.is_empty(), "the scene rides room 0");
        assert!(
            rooms.iter().all(|r| r.props.is_empty()),
            "the house is already furnished — no synthesized props"
        );
    }

    #[test]
    fn fixed_assembly_bounds_are_the_zone_boxes_scaled() {
        let (_, spec) = fixed_fixture();
        let env = fixed_env(&spec).clone();
        let graph = crate::generator::generate_for_spec(&spec, Seed::new(1), false).unwrap();
        let rooms = spawn_list_full(&graph, &spec, Seed::new(1), cat());
        assert_eq!(rooms.len(), env.zones.len());
        for (idx, room) in graph.room_indices().zip(&rooms) {
            let placed = graph.room(idx).expect("room");
            let zone = env
                .zones
                .iter()
                .find(|z| z.min == placed.grid_pos)
                .expect("each room is a zone");
            for k in 0..3 {
                let pitch = if k == 1 { spec.pitch.story } else { spec.pitch.tile };
                assert_eq!(room.bounds.min[k], zone.min[k] as f32 * pitch);
                assert_eq!(
                    room.bounds.max[k],
                    (zone.min[k] + zone.extents[k] as i32) as f32 * pitch
                );
            }
        }
    }

    #[test]
    fn fixed_assembly_keeps_the_start_clear_and_spawns_in_bounds() {
        let (_, spec) = fixed_fixture();
        let graph = crate::generator::generate_for_spec(&spec, Seed::new(1), false).unwrap();
        let rooms = spawn_list_full(&graph, &spec, Seed::new(1), cat());
        assert!(rooms[0].enemies.is_empty(), "the start zone stays clear");
        for (i, room) in rooms.iter().enumerate() {
            for e in &room.enemies {
                assert!(
                    room.bounds.contains(*e),
                    "room {i}: enemy spawn {e:?} outside bounds {:?}",
                    room.bounds
                );
            }
            assert_eq!(
                room.lights.len(),
                1,
                "room {i}: one authored ceiling light per zone (v1)"
            );
        }
    }

    #[test]
    fn fixed_assembly_walls_the_house_in_a_containment_shell() {
        // The archviz shell is leaky (recon 2026-07-11): six slabs just
        // outside the zone-box union keep the ship inside the level however
        // porous the art is. Grammar-derived — no mesh probing.
        let (_, spec) = fixed_fixture();
        let env = fixed_env(&spec).clone();
        let graph = crate::generator::generate_for_spec(&spec, Seed::new(1), false).unwrap();
        let rooms = spawn_list_full(&graph, &spec, Seed::new(1), cat());
        let slabs: Vec<_> = rooms.iter().flat_map(|r| &r.shell).collect();
        // Independent derivation of the boundary contract: one slab per
        // unit zone-cell face not shared with another zone cell — the
        // envelope hugs the authored footprint, window-tight.
        let mut cells = std::collections::HashSet::new();
        for z in &env.zones {
            for x in 0..z.extents[0] as i32 {
                for y in 0..z.extents[1] as i32 {
                    for c in 0..z.extents[2] as i32 {
                        cells.insert([z.min[0] + x, z.min[1] + y, z.min[2] + c]);
                    }
                }
            }
        }
        let expected: usize = cells
            .iter()
            .map(|c| {
                [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]
                    .iter()
                    .filter(|d| {
                        !cells.contains(&[c[0] + d[0], c[1] + d[1], c[2] + d[2]])
                    })
                    .count()
            })
            .sum();
        assert_eq!(
            slabs.len(),
            expected,
            "one slab per unshared zone-cell face (the tight wrap)"
        );
        // Every slab lies fully outside every zone's scaled box.
        let disjoint = |slab: &crate::room_assembler::ShellSlab,
                        zlo: [f32; 3],
                        zhi: [f32; 3]| {
            (0..3).any(|k| {
                slab.center[k] + slab.size[k] / 2.0 <= zlo[k]
                    || slab.center[k] - slab.size[k] / 2.0 >= zhi[k]
            })
        };
        for slab in &slabs {
            for zone in &env.zones {
                let zlo = [
                    zone.min[0] as f32 * spec.pitch.tile,
                    zone.min[1] as f32 * spec.pitch.story,
                    zone.min[2] as f32 * spec.pitch.tile,
                ];
                let zhi = [
                    (zone.min[0] + zone.extents[0] as i32) as f32 * spec.pitch.tile,
                    (zone.min[1] + zone.extents[1] as i32) as f32 * spec.pitch.story,
                    (zone.min[2] + zone.extents[2] as i32) as f32 * spec.pitch.tile,
                ];
                assert!(
                    disjoint(slab, zlo, zhi),
                    "containment slab {slab:?} intrudes into zone '{}'",
                    zone.key
                );
            }
        }
    }

    #[test]
    fn fixed_manifest_anchors_the_boss_and_covers_every_position() {
        let (grammar, spec) = fixed_fixture();
        let graph = crate::generator::generate_for_spec(&spec, Seed::new(1), false).unwrap();
        let rooms = spawn_list_full(&graph, &spec, Seed::new(1), cat());
        let m = manifest(&grammar, &graph, &spec, Seed::new(1));
        // Every assembly enemy position gets a type (the standing manifest
        // contract, exercised on the fixed path).
        for (ri, room) in rooms.iter().enumerate() {
            assert_eq!(
                m.rooms[ri].enemies.len(),
                room.enemies.len(),
                "room {ri}: the manifest covers each position exactly once"
            );
        }
        // The staged boss anchors the arena's single authored spawn.
        let staging = spec.boss.as_ref().expect("fixed levels stage a boss");
        let boss_room = graph.boss_room().expect("the arena is marked");
        let arena_pos = graph
            .room_indices()
            .position(|i| i == boss_room)
            .expect("arena in room order");
        assert!(
            m.rooms[arena_pos].enemies.iter().any(|e| e.enemy_type == staging.boss),
            "the arena manifests the staged boss"
        );
    }

    // --- flythrough_poses: the capture rig's reproducible artery ---

    /// A pose sits inside node `n`'s world AABB (small tolerance).
    fn pose_in_node(
        graph: &LevelGraph,
        n: petgraph::graph::NodeIndex,
        pose: &[f32; 5],
        pitch: crate::planet::Pitch,
    ) -> bool {
        let Some(room) = graph.room(n) else { return false };
        let origin = room.world_position(pitch.tile, pitch.story);
        let [ex, ey, ez] = room.template.extents;
        let eps = 0.01;
        pose[0] >= origin[0] - eps
            && pose[0] <= origin[0] + ex as f32 * pitch.tile + eps
            && pose[1] >= origin[1] - eps
            && pose[1] <= origin[1] + ey as f32 * pitch.story + eps
            && pose[2] >= origin[2] - eps
            && pose[2] <= origin[2] + ez as f32 * pitch.tile + eps
    }

    #[test]
    fn flythrough_walks_the_artery_looking_where_it_goes() {
        for seed in 0..10u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let poses = flythrough_poses(&graph, TEST_PITCH);
            assert!(!poses.is_empty(), "seed {seed}: a level has an artery");

            // Starts in the start room, ends in the exit room.
            let start = graph.room_indices().next().unwrap();
            let exit = graph.exit_room(start).unwrap();
            assert!(pose_in_node(&graph, start, poses.first().unwrap(), TEST_PITCH),
                "seed {seed}: the artery departs from the start room");
            assert!(pose_in_node(&graph, exit, poses.last().unwrap(), TEST_PITCH),
                "seed {seed}: the artery arrives at the exit room");

            // The camera never leaves the level: every pose is inside
            // SOME node's volume.
            for (i, pose) in poses.iter().enumerate() {
                assert!(
                    graph.room_indices().any(|n| pose_in_node(&graph, n, pose, TEST_PITCH)),
                    "seed {seed}: pose {i} escapes the level"
                );
            }

            // Each pose looks where it goes: yaw (degrees) faces the
            // successor; climbs pitch the camera with the motion.
            for (i, pair) in poses.windows(2).enumerate() {
                let (p, q) = (&pair[0], &pair[1]);
                let (dx, dy, dz) = (q[0] - p[0], q[1] - p[1], q[2] - p[2]);
                let len = (dx * dx + dz * dz).sqrt();
                if len < 0.1 {
                    if dy.abs() > 0.1 {
                        assert!(p[4] * dy > 0.0,
                            "seed {seed}: pose {i} pitches with its vertical hop");
                    }
                    continue;
                }
                let yaw = p[3].to_radians();
                let dot = (-yaw.sin()) * dx / len + (-yaw.cos()) * dz / len;
                assert!(dot > 0.99,
                    "seed {seed}: pose {i} must face its successor (dot {dot})");
            }
        }
    }

    #[test]
    fn flythrough_is_deterministic() {
        let graph = generate(&test_config(3)).expect("seed 3 generates");
        assert_eq!(
            flythrough_poses(&graph, TEST_PITCH),
            flythrough_poses(&graph, TEST_PITCH),
            "same graph, same artery — the rig depends on it"
        );
    }

    #[test]
    fn establishing_poses_frame_the_level_from_outside() {
        for seed in 0..10u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let poses = establishing_poses(&graph, TEST_PITCH);
            assert!(!poses.is_empty(), "seed {seed}: a level can be framed");

            // The level's world AABB, for aim checks.
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for n in graph.room_indices() {
                let room = graph.room(n).unwrap();
                let origin = room.world_position(TEST_PITCH.tile, TEST_PITCH.story);
                let [ex, ey, ez] = room.template.extents;
                let max = [
                    origin[0] + ex as f32 * TEST_PITCH.tile,
                    origin[1] + ey as f32 * TEST_PITCH.story,
                    origin[2] + ez as f32 * TEST_PITCH.tile,
                ];
                for i in 0..3 {
                    lo[i] = lo[i].min(origin[i]);
                    hi[i] = hi[i].max(max[i]);
                }
            }
            let center = [
                (lo[0] + hi[0]) * 0.5,
                (lo[1] + hi[1]) * 0.5,
                (lo[2] + hi[2]) * 0.5,
            ];

            for (i, pose) in poses.iter().enumerate() {
                // Outside every node volume — these are pull-backs.
                assert!(
                    !graph.room_indices().any(|n| pose_in_node(&graph, n, pose, TEST_PITCH)),
                    "seed {seed}: establishing pose {i} sits inside the level"
                );

                // Aimed at the level: forward vector (yaw/pitch, degrees)
                // points at the AABB center.
                let (dx, dy, dz) =
                    (center[0] - pose[0], center[1] - pose[1], center[2] - pose[2]);
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                let yaw = pose[3].to_radians();
                let pitch_r = pose[4].to_radians();
                let fwd = [
                    -yaw.sin() * pitch_r.cos(),
                    pitch_r.sin(),
                    -yaw.cos() * pitch_r.cos(),
                ];
                let dot = (fwd[0] * dx + fwd[1] * dy + fwd[2] * dz) / dist;
                assert!(dot > 0.99,
                    "seed {seed}: establishing pose {i} must aim at the level (dot {dot})");
            }
        }
    }

    #[test]
    fn interior_probes_cover_the_level_from_inside() {
        for seed in 0..10u64 {
            let Ok(graph) = generate(&test_config(seed)) else { continue };
            let poses = interior_probe_poses(&graph, TEST_PITCH, 12);
            assert!(!poses.is_empty(), "seed {seed}: a level has interiors to probe");
            assert!(poses.len() <= 12, "seed {seed}: the probe respects its budget");

            // Every probe is INSIDE some node's volume — that is the
            // definition of an interior vantage.
            for (i, pose) in poses.iter().enumerate() {
                assert!(
                    graph.room_indices().any(|n| pose_in_node(&graph, n, pose, TEST_PITCH)),
                    "seed {seed}: probe {i} is not inside the level"
                );
            }

            // Coverage, not a cluster: probes land in more than one room
            // (any non-trivial level has several).
            if graph.room_count() > 3 {
                let mut hosts = std::collections::HashSet::new();
                for pose in &poses {
                    for n in graph.room_indices() {
                        if pose_in_node(&graph, n, pose, TEST_PITCH) {
                            hosts.insert(n.index());
                            break;
                        }
                    }
                }
                assert!(hosts.len() > 1,
                    "seed {seed}: probes cluster in one room ({hosts:?})");
            }

            // A vantage carries a chase camera: the probe stands clear of
            // its host's XZ walls (a boundary-cell park embeds the camera
            // in the wall — the BLIND frames of 2026-07-14) and looks
            // toward the room's center, never point-blank at a plate.
            for (i, pose) in poses.iter().enumerate() {
                let host = graph
                    .room_indices()
                    .find(|n| pose_in_node(&graph, *n, pose, TEST_PITCH))
                    .unwrap();
                let room = graph.room(host).unwrap();
                let origin = room.world_position(TEST_PITCH.tile, TEST_PITCH.story);
                let [ex, _ey, ez] = room.template.extents;
                let dx0 = pose[0] - origin[0];
                let dx1 = origin[0] + ex as f32 * TEST_PITCH.tile - pose[0];
                let dz0 = pose[2] - origin[2];
                let dz1 = origin[2] + ez as f32 * TEST_PITCH.tile - pose[2];
                let min_clear = dx0.min(dx1).min(dz0).min(dz1);
                assert!(
                    min_clear >= TEST_PITCH.tile - 1e-3,
                    "seed {seed}: probe {i} parks {min_clear:.2}m from a wall \
                     (needs a full tile of camera clearance)"
                );

                let center = [
                    origin[0] + ex as f32 * TEST_PITCH.tile * 0.5,
                    origin[2] + ez as f32 * TEST_PITCH.tile * 0.5,
                ];
                let (dx, dz) = (center[0] - pose[0], center[1] - pose[2]);
                if dx.abs() + dz.abs() > 0.1 {
                    let yaw = pose[3].to_radians();
                    let dot = -yaw.sin() * dx - yaw.cos() * dz;
                    assert!(dot > 0.0,
                        "seed {seed}: probe {i} looks away from its room");
                }
            }

            // Deterministic: the audit must see what the last run saw.
            assert_eq!(poses, interior_probe_poses(&graph, TEST_PITCH, 12),
                "seed {seed}: same graph, same probes");
        }
    }

    /// Usage census (debug instrument, the flythrough_dump pattern):
    /// how often each scene places across a generated level —
    /// repetition, pool balance, and per-kit mix at a glance.
    ///
    ///     LEVEL=7 SEED=1 make test-rust FILTER=panel_usage_census -- --ignored
    #[test]
    #[ignore = "debug instrumentation — run by hand with --nocapture"]
    fn panel_usage_census() {
        let level: u32 = std::env::var("LEVEL").ok().and_then(|v| v.parse().ok()).unwrap_or(7);
        let run_seed: u64 = std::env::var("SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
        let spec = spec_for(level);
        let config = config_for(run_seed, crate::generator::rooms_for_level(level), level);
        let graph = generate(&config).expect("generates");
        let rooms = spawn_list_full(&graph, &spec, Seed::new(run_seed), cat());
        let mut counts: std::collections::BTreeMap<&str, usize> =
            std::collections::BTreeMap::new();
        for room in &rooms {
            for m in &room.structure {
                *counts.entry(spath(m.scene)).or_default() += 1;
            }
        }
        let total: usize = counts.values().sum();
        println!("census: level {level} seed {run_seed} — {total} placements, {} distinct scenes", counts.len());
        let mut by_count: Vec<_> = counts.into_iter().collect();
        by_count.sort_by(|a, b| b.1.cmp(&a.1));
        for (scene, n) in by_count {
            println!("census: {n:5}  {scene}");
        }
    }

    /// Containment invariant over REAL generated panel levels: every
    /// sealed cell face of every room carries exactly one wall plate —
    /// what reads as a hole in a capture must be an aperture (active
    /// connector) or darkness, never missing geometry.
    #[test]
    fn generated_panel_rooms_are_watertight() {
        use crate::cell::CellGrid;
        let spec = spec_for(7);
        let pitch = spec.pitch;
        let grammar = crate::roster::roster();
        // Plate AREA by scene, across BOTH pool generations: v1 plates
        // are one cell face (pitch²); v2 role plates carry their face.
        // The invariant is AREA — covered == sealed — which v1 satisfies
        // as count×pitch² and v2 satisfies with wide plates.
        let mut plate_area: std::collections::HashMap<crate::asset_catalog::SceneId, f32> =
            std::collections::HashMap::new();
        for &id in &grammar.panel_kits_for_level(7) {
            let kit = grammar.catalog.kit_def(id);
            if let Some(p) = kit.panel_pool.as_ref() {
                for pl in &p.plates {
                    plate_area.insert(pl.scene, pitch.tile * pitch.tile);
                }
            }
            if let Some(rp) = kit.role_pools.as_ref() {
                for pl in rp.floor.iter().chain(&rp.ceiling).chain(&rp.wall) {
                    plate_area.insert(pl.scene, pl.face[0] * pl.face[1]);
                }
            }
        }
        for seed in 0..5u64 {
            let config = config_for(seed, crate::generator::rooms_for_level(7), 7);
            let graph = generate(&config).expect("generates");
            let rooms = spawn_list_full(&graph, &spec, Seed::new(seed), cat());
            for (room_idx, idx) in graph.room_indices().enumerate() {
                let room = graph.room(idx).unwrap();
                let active = graph.active_connectors(idx);
                let origin = room.world_position(pitch.tile, pitch.story);
                let grid =
                    CellGrid::new(&room.template, &active, origin, pitch.tile, pitch.story);
                let sealed: usize = grid.cells().iter().map(|c| c.sealed_faces.len()).sum();
                let sealed_area = sealed as f32 * pitch.tile * pitch.tile;
                let covered: f32 = rooms[room_idx]
                    .structure
                    .iter()
                    .filter_map(|m| plate_area.get(&m.scene))
                    .sum();
                // 1% relative: plate meshes run a hair under the grid
                // (2.9989 on a 3.0 pitch — the seam slop the snap
                // absorbs); a real missing plate is ~9 m², two orders
                // above this tolerance on any room.
                assert!(
                    (covered - sealed_area).abs() < sealed_area * 0.01,
                    "seed {seed} room {room_idx}: {sealed_area} m² sealed but \
                     {covered} m² covered — the shell leaks"
                );
            }
        }
    }

    
}
