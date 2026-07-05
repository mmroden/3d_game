//! Spatial layout: assign grid positions to an abstract topology.
//!
//! Sweep 2 of the generation pipeline. Walks the abstract graph in BFS order,
//! placing rooms far enough apart that they never overlap, with corridors
//! generated dynamically to fill the gaps.

use crate::abstract_graph::AbstractGraph;
use crate::level_graph::LevelGraph;
use crate::room_template::{Connector, ConnectorFacing, FrameStyle, RoomTemplate, TemplateKind};

/// Generate a corridor template of the given length along a facing direction.
pub fn make_corridor(facing: ConnectorFacing, length: u32) -> RoomTemplate {
    // Vertical corridors share their cross-section with the opening width
    // (see ConnectorFacing::opening_span) so floor/ceiling holes get a
    // border and tile frames don't overlap; horizontal corridors stay 1 wide.
    let span = facing.opening_span() as u32;
    let (extents, c_in, c_out) = match facing {
        ConnectorFacing::PosX => (
            [length, 1, 1],
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [length as i32 - 1, 0, 0], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
        ),
        ConnectorFacing::NegX => (
            [length, 1, 1],
            Connector { offset: [length as i32 - 1, 0, 0], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
        ),
        ConnectorFacing::PosZ => (
            [1, 1, length],
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ, frame: FrameStyle::Door },
            Connector { offset: [0, 0, length as i32 - 1], facing: ConnectorFacing::PosZ, frame: FrameStyle::Door },
        ),
        ConnectorFacing::NegZ => (
            [1, 1, length],
            Connector { offset: [0, 0, length as i32 - 1], facing: ConnectorFacing::PosZ, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ, frame: FrameStyle::Door },
        ),
        ConnectorFacing::PosY => (
            [span, length, span],
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegY, frame: FrameStyle::None },
            Connector { offset: [0, length as i32 - 1, 0], facing: ConnectorFacing::PosY, frame: FrameStyle::None },
        ),
        ConnectorFacing::NegY => (
            [span, length, span],
            Connector { offset: [0, length as i32 - 1, 0], facing: ConnectorFacing::PosY, frame: FrameStyle::None },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegY, frame: FrameStyle::None },
        ),
    };

    RoomTemplate {
        kind: TemplateKind::Corridor,
        connectors: vec![c_in, c_out],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents,
    }
}

/// Outcome of a single fixed-length placement attempt.
enum AttemptOutcome {
    /// Corridor + child landed; carries `(corridor_idx, child_idx)`.
    Placed(petgraph::graph::NodeIndex, petgraph::graph::NodeIndex),
    /// Cells were occupied at this length — another length may fit.
    Blocked,
    /// The free-check passed but `place_room` still failed — abort probing
    /// this connector (matches the historical early-`break` semantics).
    Abort,
}

/// Try placing a child room off a parent connector through a corridor of
/// EXACTLY `corridor_len` cells. Shared by the ascending probe (regular
/// layout wants the shortest link) and the descending probe (the boss
/// approach wants the longest).
fn try_place_child_at(
    level: &mut LevelGraph,
    parent_level_idx: petgraph::graph::NodeIndex,
    parent_ci: usize,
    child_room: &RoomTemplate,
    child_ci: usize,
    corridor_len: i32,
) -> AttemptOutcome {
    let Some(parent_room) = level.room(parent_level_idx) else {
        return AttemptOutcome::Abort;
    };
    let parent_pos = parent_room.grid_pos;
    let parent_connector = &parent_room.template.connectors[parent_ci];
    let parent_facing = parent_connector.facing;
    let direction = parent_facing.grid_offset();

    let corridor_start = [
        parent_pos[0] + parent_connector.offset[0] + direction[0],
        parent_pos[1] + parent_connector.offset[1] + direction[1],
        parent_pos[2] + parent_connector.offset[2] + direction[2],
    ];

    let child_connector = &child_room.connectors[child_ci];

    let corridor_end = [
        corridor_start[0] + direction[0] * (corridor_len - 1),
        corridor_start[1] + direction[1] * (corridor_len - 1),
        corridor_start[2] + direction[2] * (corridor_len - 1),
    ];

    let child_connector_cell = [
        corridor_end[0] + direction[0],
        corridor_end[1] + direction[1],
        corridor_end[2] + direction[2],
    ];
    let child_pos = [
        child_connector_cell[0] - child_connector.offset[0],
        child_connector_cell[1] - child_connector.offset[1],
        child_connector_cell[2] - child_connector.offset[2],
    ];

    let corridor = make_corridor(parent_facing, corridor_len as u32);
    let corridor_origin = corridor_start_origin(corridor_start, parent_facing, &corridor);

    let corridor_cells = cells_for_at(&corridor, corridor_origin);
    if !corridor_cells.iter().all(|c| level.is_free(*c)) {
        return AttemptOutcome::Blocked;
    }

    let child_cells = cells_for_at(child_room, child_pos);
    if !child_cells.iter().all(|c| level.is_free(*c)) {
        return AttemptOutcome::Blocked;
    }

    if let Ok(corridor_idx) = level.place_room(corridor, corridor_origin) {
        let _ = level.connect_adjacent(parent_level_idx, corridor_idx);

        if let Ok(child_idx) = level.place_room(child_room.clone(), child_pos) {
            let _ = level.connect_adjacent(corridor_idx, child_idx);
            return AttemptOutcome::Placed(corridor_idx, child_idx);
        }
    }
    AttemptOutcome::Abort
}

/// Try placing a child room connected to a parent via a specific connector pair.
/// Returns `Some((corridor_idx, child_idx))` on success.
fn try_place_child(
    level: &mut LevelGraph,
    parent_level_idx: petgraph::graph::NodeIndex,
    parent_ci: usize,
    child_room: &RoomTemplate,
    child_ci: usize,
    max_probe: i32,
) -> Option<(petgraph::graph::NodeIndex, petgraph::graph::NodeIndex)> {
    for corridor_len in 1..=max_probe {
        match try_place_child_at(level, parent_level_idx, parent_ci, child_room, child_ci, corridor_len) {
            AttemptOutcome::Placed(corridor_idx, child_idx) => return Some((corridor_idx, child_idx)),
            AttemptOutcome::Blocked => continue,
            AttemptOutcome::Abort => break,
        }
    }

    None
}

/// Build all compatible connector pairs between a parent and child room.
fn all_compatible_pairs(parent: &RoomTemplate, child: &RoomTemplate) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for (pi, pc) in parent.connectors.iter().enumerate() {
        for (ci, cc) in child.connectors.iter().enumerate() {
            if pc.facing.opposite() == cc.facing {
                pairs.push((pi, ci));
            }
        }
    }
    pairs
}

/// Boss arena sizing (grid cells / stories). The arena dwarfs regular rooms
/// (3-6 cells): the scale shift is the encounter staging.
pub const BOSS_ROOM_XZ: u32 = 10;
pub const BOSS_ROOM_STORIES: u32 = 2;
/// Approach-corridor bounds (cells). The long walk in — sealed behind the
/// player once entered (B6) — is probed longest-first so the drama holds
/// even on crowded grids.
pub const MIN_BOSS_CORRIDOR: u32 = 12;
pub const MAX_BOSS_CORRIDOR: u32 = 32;

/// The boss arena: a giant single room with one mid-face door per side
/// (only the approach corridor's side gets wired; the assembler seals the
/// rest), one enemy spawn for the boss and one loot spawn for the reward
/// container, both centered.
fn boss_room_template(pitch: crate::planet::Pitch) -> RoomTemplate {
    use ConnectorFacing::*;

    let ex = BOSS_ROOM_XZ as i32;
    let ez = BOSS_ROOM_XZ as i32;
    let mid_x = ex / 2;
    let mid_z = ez / 2;
    let connectors = vec![
        Connector { offset: [0, 0, mid_z], facing: NegX, frame: FrameStyle::Door },
        Connector { offset: [ex - 1, 0, mid_z], facing: PosX, frame: FrameStyle::Door },
        Connector { offset: [mid_x, 0, 0], facing: NegZ, frame: FrameStyle::Door },
        Connector { offset: [mid_x, 0, ez - 1], facing: PosZ, frame: FrameStyle::Door },
    ];

    // Same local-coordinate convention as the generator's auto spawns;
    // loot near the floor so the container rests in view.
    let cell_size = pitch.tile;
    let center_x = BOSS_ROOM_XZ as f32 * cell_size / 2.0;
    let center_z = BOSS_ROOM_XZ as f32 * cell_size / 2.0;

    RoomTemplate {
        kind: crate::room_template::TemplateKind::Room,
        connectors,
        enemy_spawns: vec![crate::room_template::SpawnPoint {
            position: [center_x, 2.0, center_z],
        }],
        // The reward drops at the quarter point, NOT the center: the exit
        // portal materializes at the room center (`portal_position`), and a
        // reward on the same spot would hurl the player into it the moment
        // it activates.
        loot_spawns: vec![crate::room_template::SpawnPoint {
            position: [center_x * 0.5, 0.75, center_z],
        }],
        extents: [BOSS_ROOM_XZ, BOSS_ROOM_STORIES, BOSS_ROOM_XZ],
    }
}

/// Attach the boss arena to a generated level: a long corridor off the
/// farthest room, ending in a giant sealed-off arena. The arena becomes the
/// new farthest room, so `portal_position` (and the exit-room accents)
/// follow with zero changes. Marks `LevelGraph::boss_room` on success.
///
/// Probes longest corridor first (the walk in is the staging), each of the
/// far room's horizontal connectors in order — fully deterministic. Returns
/// `None` only if no length ≥ `MIN_BOSS_CORRIDOR` fits anywhere, which the
/// pinned-seed tests forbid for the levels the game actually schedules.
pub fn attach_boss_room(
    level: &mut LevelGraph,
    entry: petgraph::graph::NodeIndex,
    pitch: crate::planet::Pitch,
) -> Option<petgraph::graph::NodeIndex> {
    let far = level.farthest_room_from(entry)?;
    let arena = boss_room_template(pitch);

    let horizontal: Vec<(usize, ConnectorFacing)> = level
        .room(far)?
        .template
        .connectors
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            matches!(
                c.facing,
                ConnectorFacing::PosX | ConnectorFacing::NegX
                    | ConnectorFacing::PosZ | ConnectorFacing::NegZ
            )
        })
        .map(|(i, c)| (i, c.facing))
        .collect();

    for corridor_len in (MIN_BOSS_CORRIDOR..=MAX_BOSS_CORRIDOR).rev() {
        for &(parent_ci, facing) in &horizontal {
            let Some(arena_ci) = arena
                .connectors
                .iter()
                .position(|c| c.facing == facing.opposite())
            else {
                continue;
            };
            if let AttemptOutcome::Placed(_, boss_idx) = try_place_child_at(
                level, far, parent_ci, &arena, arena_ci, corridor_len as i32,
            ) {
                level.boss_room = Some(boss_idx);
                return Some(boss_idx);
            }
        }
    }

    None
}

/// Assign grid positions to rooms in an abstract graph, producing a fully
/// positioned LevelGraph with dynamically generated corridors.
///
/// BFS from the root. For each edge, try the designated connector pair first,
/// then fall back to all compatible pairs. Deferred rooms get a retry pass
/// against every already-placed room.
pub fn assign_positions(abstract_graph: &AbstractGraph) -> LevelGraph {
    use petgraph::visit::Bfs;
    use std::collections::{HashMap, HashSet};

    let mut level = LevelGraph::new();
    let mut placed: HashMap<petgraph::graph::NodeIndex, petgraph::graph::NodeIndex> = HashMap::new();
    let mut visited: HashSet<petgraph::graph::NodeIndex> = HashSet::new();

    let max_probe: i32 = 100;

    // Place root at origin.
    let root = abstract_graph.root();
    let root_room = abstract_graph.room(root).unwrap().clone();
    let root_level_idx = level.place_room(root_room, [0, 0, 0])
        .expect("root placement cannot overlap");
    placed.insert(root, root_level_idx);
    visited.insert(root);

    // Track rooms that couldn't be placed during BFS for a retry pass.
    let mut deferred: Vec<petgraph::graph::NodeIndex> = Vec::new();

    // BFS traversal.
    let mut bfs = Bfs::new(&abstract_graph.graph, root);
    bfs.next(&abstract_graph.graph); // skip root

    while let Some(abs_node) = bfs.next(&abstract_graph.graph) {
        if visited.contains(&abs_node) {
            continue;
        }
        visited.insert(abs_node);

        let edge = abstract_graph.edges().find(|(from, to, _)| {
            (*to == abs_node && placed.contains_key(from))
                || (*from == abs_node && placed.contains_key(to))
        });

        let Some((edge_from, edge_to, pair)) = edge else {
            deferred.push(abs_node);
            continue;
        };

        let (parent_abs, _child_abs, designated_pci, designated_cci) =
            if placed.contains_key(&edge_from) {
                (edge_from, edge_to, pair.from_connector_idx, pair.to_connector_idx)
            } else {
                (edge_to, edge_from, pair.to_connector_idx, pair.from_connector_idx)
            };

        let parent_level_idx = placed[&parent_abs];
        let child_room = abstract_graph.room(abs_node).unwrap().clone();

        // Try the designated connector pair first.
        if let Some((_corr, child_idx)) =
            try_place_child(&mut level, parent_level_idx, designated_pci, &child_room, designated_cci, max_probe)
        {
            placed.insert(abs_node, child_idx);
            continue;
        }

        // Fall back: try all compatible pairs between this parent and child.
        let parent_template = level.room(parent_level_idx).unwrap().template.clone();
        let pairs = all_compatible_pairs(&parent_template, &child_room);
        let mut success = false;
        for (pci, cci) in &pairs {
            if *pci == designated_pci && *cci == designated_cci {
                continue; // already tried
            }
            if let Some((_corr, child_idx)) =
                try_place_child(&mut level, parent_level_idx, *pci, &child_room, *cci, max_probe)
            {
                placed.insert(abs_node, child_idx);
                success = true;
                break;
            }
        }

        if !success {
            deferred.push(abs_node);
        }
    }

    // Retry pass: try deferred rooms against ALL placed rooms.
    for abs_node in &deferred {
        let child_room = abstract_graph.room(*abs_node).unwrap().clone();
        let placed_snapshot: Vec<_> = placed.values().copied().collect();

        let mut success = false;
        for parent_level_idx in &placed_snapshot {
            let parent_template = level.room(*parent_level_idx).unwrap().template.clone();
            let pairs = all_compatible_pairs(&parent_template, &child_room);
            for (pci, cci) in &pairs {
                if let Some((_corr, child_idx)) =
                    try_place_child(&mut level, *parent_level_idx, *pci, &child_room, *cci, max_probe)
                {
                    placed.insert(*abs_node, child_idx);
                    success = true;
                    break;
                }
            }
            if success {
                break;
            }
        }
    }

    level
}

/// Compute the corridor's grid origin given the start cell and facing.
fn corridor_start_origin(start: [i32; 3], facing: ConnectorFacing, corridor: &RoomTemplate) -> [i32; 3] {
    // The corridor's inward connector (the one facing the parent) must be at `start`.
    // Find that connector.
    let inward_facing = facing.opposite();
    let inward_connector = corridor.connectors.iter()
        .find(|c| c.facing == inward_facing)
        .expect("corridor must have inward connector");

    [
        start[0] - inward_connector.offset[0],
        start[1] - inward_connector.offset[1],
        start[2] - inward_connector.offset[2],
    ]
}

/// Compute all cells a room occupies at a given origin.
fn cells_for_at(template: &RoomTemplate, origin: [i32; 3]) -> Vec<[i32; 3]> {
    let [ex, ey, ez] = template.extents.map(|e| e as i32);
    (0..ex).flat_map(|x| {
        (0..ey).flat_map(move |y| {
            (0..ez).map(move |z| [origin[0] + x, origin[1] + y, origin[2] + z])
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    /// The planet-1 pitch, spelled out: tests may hold literals.
    const TEST_PITCH: crate::planet::Pitch =
        crate::planet::Pitch { tile: 4.0, story: 5.0 };

    use super::*;
    use crate::abstract_graph::{self, ConnectorPair};
    use crate::generator::GeneratorConfig;
    use crate::room_template::*;
    use petgraph::graph::UnGraph;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    fn simple_room(ex: u32, ez: u32) -> RoomTemplate {
        let mid_x = (ex as i32) / 2;
        let mid_z = (ez as i32) / 2;
        RoomTemplate {
            kind: TemplateKind::Room,
            connectors: vec![
                Connector { offset: [0, 0, mid_z], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
                Connector { offset: [ex as i32 - 1, 0, mid_z], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
                Connector { offset: [mid_x, 0, 0], facing: ConnectorFacing::NegZ, frame: FrameStyle::Door },
                Connector { offset: [mid_x, 0, ez as i32 - 1], facing: ConnectorFacing::PosZ, frame: FrameStyle::Door },
                Connector { offset: [mid_x, 0, mid_z], facing: ConnectorFacing::PosY, frame: FrameStyle::Door },
                Connector { offset: [mid_x, 0, mid_z], facing: ConnectorFacing::NegY, frame: FrameStyle::Door },
            ],
            enemy_spawns: vec![],
            loot_spawns: vec![],
            extents: [ex, 1, ez],
        }
    }

    fn build_abstract(rooms: Vec<RoomTemplate>, edges: Vec<(usize, usize, ConnectorPair)>) -> AbstractGraph {
        let mut graph = UnGraph::new_undirected();
        let indices: Vec<_> = rooms.into_iter().map(|r| graph.add_node(r)).collect();
        for (from, to, pair) in edges {
            graph.add_edge(indices[from], indices[to], pair);
        }
        AbstractGraph { graph, root: indices[0] }
    }

    // PosX connector on room A (idx 1) connects to NegX connector on room B (idx 0)
    fn posx_negx_pair(room_a: &RoomTemplate, room_b: &RoomTemplate) -> ConnectorPair {
        let from_idx = room_a.connectors.iter().position(|c| c.facing == ConnectorFacing::PosX).unwrap();
        let to_idx = room_b.connectors.iter().position(|c| c.facing == ConnectorFacing::NegX).unwrap();
        ConnectorPair { from_connector_idx: from_idx, to_connector_idx: to_idx }
    }

    fn posy_negy_pair(room_a: &RoomTemplate, room_b: &RoomTemplate) -> ConnectorPair {
        let from_idx = room_a.connectors.iter().position(|c| c.facing == ConnectorFacing::PosY).unwrap();
        let to_idx = room_b.connectors.iter().position(|c| c.facing == ConnectorFacing::NegY).unwrap();
        ConnectorPair { from_connector_idx: from_idx, to_connector_idx: to_idx }
    }

    // --- Boss arena attachment (B4) ---

    /// Seed 1, level 3 — the first mid-boss level; B6's GUT scenario builds
    /// exactly this level, so these pins and the shell can never drift.
    fn pinned_boss_level() -> (LevelGraph, petgraph::graph::NodeIndex) {
        let config = GeneratorConfig::standard(
            crate::seed::Seed::new(1),
            crate::generator::rooms_for_level(3),
            3,
        );
        let level = crate::generator::generate(&config).expect("pinned seed generates");
        let entry = level.room_indices().next().expect("level has rooms");
        (level, entry)
    }

    #[test]
    fn the_boss_arena_attaches_and_is_marked() {
        let (mut level, entry) = pinned_boss_level();
        let before = level.room_count();
        let boss = attach_boss_room(&mut level, entry, TEST_PITCH)
            .expect("the pinned level must take a boss arena");
        assert_eq!(level.boss_room(), Some(boss), "the graph carries the marker");
        assert!(level.room_count() >= before + 2, "arena + approach corridor added");
        assert!(level.is_fully_connected(), "the arena hangs off the existing graph");
    }

    #[test]
    fn the_arena_dwarfs_regular_rooms() {
        let (mut level, entry) = pinned_boss_level();
        let boss = attach_boss_room(&mut level, entry, TEST_PITCH).expect("attach");
        let room = level.room(boss).expect("boss room exists");
        assert_eq!(room.template.kind, TemplateKind::Room);
        assert!(
            room.template.extents[0] >= BOSS_ROOM_XZ
                && room.template.extents[2] >= BOSS_ROOM_XZ,
            "extents {:?} must dwarf the 3-6 cell regular rooms",
            room.template.extents
        );
        assert!(room.template.extents[1] >= BOSS_ROOM_STORIES);
    }

    #[test]
    fn the_approach_is_a_long_corridor() {
        let (mut level, entry) = pinned_boss_level();
        let boss = attach_boss_room(&mut level, entry, TEST_PITCH).expect("attach");
        let neighbors: Vec<_> = level.neighbors(boss).collect();
        assert_eq!(neighbors.len(), 1, "the arena has exactly one way in");
        let corridor = level.room(neighbors[0]).expect("approach exists");
        assert_eq!(corridor.template.kind, TemplateKind::Corridor);
        let length = corridor.template.extents.into_iter().max().unwrap();
        assert!(
            length >= MIN_BOSS_CORRIDOR,
            "approach length {length} must sell the long walk in (≥ {MIN_BOSS_CORRIDOR})"
        );
    }

    #[test]
    fn the_arena_becomes_the_farthest_room_so_the_portal_follows() {
        let (mut level, entry) = pinned_boss_level();
        let boss = attach_boss_room(&mut level, entry, TEST_PITCH).expect("attach");
        assert_eq!(level.farthest_room_from(entry), Some(boss),
            "portal_position keys off the farthest room — it must be the arena");

        let cell_size = 4.0;
        let portal = crate::portal::portal_position(&level, TEST_PITCH)
            .expect("portal placed");
        let room = level.room(boss).unwrap();
        let story_height = crate::asset_catalog::WALL_SET_ASTRA.story_height;
        let origin = room.world_position(cell_size, story_height);
        let ex = room.template.extents[0] as f32 * cell_size;
        let ez = room.template.extents[2] as f32 * cell_size;
        assert!(portal[0] >= origin[0] && portal[0] <= origin[0] + ex,
            "portal x inside the arena");
        assert!(portal[2] >= origin[2] && portal[2] <= origin[2] + ez,
            "portal z inside the arena");
    }

    #[test]
    fn the_arena_carries_a_boss_spawn_and_a_reward_spot() {
        // One enemy spawn (the boss — B5's manifest places it) and one loot
        // spawn (the red/consolation container — B7) — both near the center.
        let (mut level, entry) = pinned_boss_level();
        let boss = attach_boss_room(&mut level, entry, TEST_PITCH).expect("attach");
        let template = &level.room(boss).unwrap().template;
        assert_eq!(template.enemy_spawns.len(), 1, "exactly the boss spawns here");
        assert_eq!(template.loot_spawns.len(), 1, "exactly the reward drops here");
    }

    #[test]
    fn boss_attachment_is_deterministic() {
        let (mut a, entry_a) = pinned_boss_level();
        let (mut b, entry_b) = pinned_boss_level();
        let boss_a = attach_boss_room(&mut a, entry_a, TEST_PITCH).expect("attach a");
        let boss_b = attach_boss_room(&mut b, entry_b, TEST_PITCH).expect("attach b");
        assert_eq!(a.room(boss_a).unwrap().grid_pos, b.room(boss_b).unwrap().grid_pos);
    }

    #[test]
    fn plain_generation_leaves_the_graph_unmarked() {
        let (level, _) = pinned_boss_level();
        assert_eq!(level.boss_room(), None,
            "only attach_boss_room may mark a boss arena");
    }

    #[test]
    fn make_corridor_posx_correct_extents() {
        let c = make_corridor(ConnectorFacing::PosX, 5);
        assert_eq!(c.extents, [5, 1, 1]);
        assert_eq!(c.kind, TemplateKind::Corridor);
        assert!(c.connectors.iter().any(|c| c.facing == ConnectorFacing::NegX));
        assert!(c.connectors.iter().any(|c| c.facing == ConnectorFacing::PosX));
    }

    #[test]
    fn make_corridor_posy_correct_extents() {
        let c = make_corridor(ConnectorFacing::PosY, 3);
        // Vertical corridor cross-section matches the 2-cell opening span.
        assert_eq!(c.extents, [2, 3, 2]);
        assert!(c.connectors.iter().any(|c| c.facing == ConnectorFacing::NegY));
        assert!(c.connectors.iter().any(|c| c.facing == ConnectorFacing::PosY));
    }

    #[test]
    fn two_rooms_east_no_overlap() {
        let a = simple_room(3, 3);
        let b = simple_room(3, 3);
        let pair = posx_negx_pair(&a, &b);
        let ag = build_abstract(vec![a, b], vec![(0, 1, pair)]);
        let level = assign_positions(&ag);
        // Both rooms should be placed.
        let room_count = level.room_indices()
            .filter(|&idx| level.room(idx).map(|r| r.template.kind == TemplateKind::Room).unwrap_or(false))
            .count();
        assert_eq!(room_count, 2, "expected 2 rooms placed");
        assert!(level.is_fully_connected());
    }

    #[test]
    fn corridor_exists_between_rooms() {
        let a = simple_room(3, 3);
        let b = simple_room(3, 3);
        let pair = posx_negx_pair(&a, &b);
        let ag = build_abstract(vec![a, b], vec![(0, 1, pair)]);
        let level = assign_positions(&ag);
        let corridor_count = level.room_indices()
            .filter(|&idx| level.room(idx).map(|r| r.template.kind == TemplateKind::Corridor).unwrap_or(false))
            .count();
        assert!(corridor_count >= 1, "expected at least 1 corridor, got {corridor_count}");
    }

    #[test]
    fn vertical_connection_places_above() {
        let a = simple_room(3, 3);
        let b = simple_room(3, 3);
        let pair = posy_negy_pair(&a, &b);
        let ag = build_abstract(vec![a, b], vec![(0, 1, pair)]);
        let level = assign_positions(&ag);
        let rooms: Vec<_> = level.room_indices()
            .filter(|&idx| level.room(idx).map(|r| r.template.kind == TemplateKind::Room).unwrap_or(false))
            .filter_map(|idx| level.room(idx).map(|r| r.grid_pos))
            .collect();
        assert_eq!(rooms.len(), 2);
        assert!(rooms[1][1] > rooms[0][1],
            "second room should be above first: {:?} vs {:?}", rooms[0], rooms[1]);
    }

    #[test]
    fn large_rooms_push_children_further() {
        let a = simple_room(6, 6);
        let b = simple_room(3, 3);
        let pair = posx_negx_pair(&a, &b);
        let ag = build_abstract(vec![a, b], vec![(0, 1, pair)]);
        let level = assign_positions(&ag);
        let rooms: Vec<_> = level.room_indices()
            .filter(|&idx| level.room(idx).map(|r| r.template.kind == TemplateKind::Room).unwrap_or(false))
            .filter_map(|idx| level.room(idx).map(|r| r.grid_pos))
            .collect();
        assert_eq!(rooms.len(), 2);
        // Room B's origin should be beyond room A's extent (6) + corridor (≥1).
        assert!(rooms[1][0] >= 7,
            "6-wide room A + corridor should push B to x≥7, got x={}", rooms[1][0]);
    }

    #[test]
    fn branching_no_overlap() {
        // A connects east to B and south to C.
        let a = simple_room(3, 3);
        let b = simple_room(3, 3);
        let c = simple_room(3, 3);
        let pair_ab = posx_negx_pair(&a, &b);
        let pair_ac = ConnectorPair {
            from_connector_idx: a.connectors.iter().position(|c| c.facing == ConnectorFacing::PosZ).unwrap(),
            to_connector_idx: c.connectors.iter().position(|c| c.facing == ConnectorFacing::NegZ).unwrap(),
        };
        let ag = build_abstract(vec![a, b, c], vec![(0, 1, pair_ab), (0, 2, pair_ac)]);
        let level = assign_positions(&ag);
        let room_count = level.room_indices()
            .filter(|&idx| level.room(idx).map(|r| r.template.kind == TemplateKind::Room).unwrap_or(false))
            .count();
        assert_eq!(room_count, 3, "all 3 rooms should be placed");
        assert!(level.is_fully_connected());
    }

    #[test]
    fn rooms_have_active_connectors_at_apertures() {
        // Every room-corridor connection must produce active connectors
        // so that CellGrid creates ConnectorGap cells (apertures).
        let a = simple_room(3, 3);
        let b = simple_room(3, 3);
        let pair = posx_negx_pair(&a, &b);
        let ag = build_abstract(vec![a, b], vec![(0, 1, pair)]);
        let level = assign_positions(&ag);

        // Each room should have at least 1 active connector (the one wired to the corridor).
        for idx in level.room_indices() {
            let active = level.active_connectors(idx);
            let room = level.room(idx).unwrap();
            if room.template.kind == TemplateKind::Room {
                assert!(!active.is_empty(),
                    "Room at {:?} should have active connectors, got none", room.grid_pos);
            }
        }
    }

    #[test]
    fn generated_level_rooms_all_have_active_connectors() {
        use crate::generator;

        for seed in 0..10 {
            let config = GeneratorConfig {
                pitch: crate::planet::Pitch { tile: 4.0, story: 5.0 },
                seed: crate::seed::Seed::new(seed),
                max_rooms: 10,
                min_room_xz: 3,
                max_room_xz: 6,
                min_room_y: 1,
                max_room_y: 6,
            };
            let level = generator::generate(&config).expect("generation should succeed");
            for idx in level.room_indices() {
                let room = level.room(idx).unwrap();
                if room.template.kind == TemplateKind::Room {
                    let active = level.active_connectors(idx);
                    assert!(!active.is_empty(),
                        "seed {seed}: room at {:?} (extents {:?}) has 0 active connectors",
                        room.grid_pos, room.template.extents);
                }
            }
        }
    }

    #[test]
    fn fifteen_rooms_mostly_placed() {
        use crate::generator;
        // With 15 rooms requested, at least 12 should survive spatial placement.
        for seed in 0..20 {
            let config = GeneratorConfig {
                pitch: crate::planet::Pitch { tile: 4.0, story: 5.0 },
                seed: crate::seed::Seed::new(seed),
                max_rooms: 15,
                min_room_xz: 3,
                max_room_xz: 6,
                min_room_y: 1,
                max_room_y: 6,
            };
            let level = generator::generate(&config).expect("generation should succeed");
            let rooms = level.room_indices()
                .filter(|&idx| level.room(idx).map(|r| r.template.kind == TemplateKind::Room).unwrap_or(false))
                .count();
            assert!(rooms >= 12, "seed {seed}: expected ≥12 rooms, got {rooms}");
        }
    }

    #[test]
    fn generated_levels_have_vertical_connections() {
        use crate::generator;

        // Over 20 seeds, at least some should have rooms at different Y levels,
        // connected via vertical corridors with active PosY/NegY connectors.
        let mut any_vertical = false;
        for seed in 0..20 {
            let config = GeneratorConfig {
                pitch: crate::planet::Pitch { tile: 4.0, story: 5.0 },
                seed: crate::seed::Seed::new(seed),
                max_rooms: 10,
                min_room_xz: 3,
                max_room_xz: 6,
                min_room_y: 1,
                max_room_y: 6,
            };
            let level = generator::generate(&config).expect("generation should succeed");

            // Vertical corridors have a 2×2 cross-section (the opening span)
            // and run along Y.
            for idx in level.room_indices() {
                let room = level.room(idx).unwrap();
                if room.template.kind == TemplateKind::Corridor
                    && room.template.extents[0] == 2
                    && room.template.extents[2] == 2
                    && room.template.extents[1] >= 1
                {
                    // Check if it has PosY/NegY connectors
                    let has_vert_connector = room.template.connectors.iter()
                        .any(|c| matches!(c.facing, ConnectorFacing::PosY | ConnectorFacing::NegY));
                    if has_vert_connector {
                        any_vertical = true;
                        break;
                    }
                }
            }
            if any_vertical { break; }
        }
        assert!(any_vertical,
            "across 20 seeds, at least one level should have a vertical corridor");
    }

    #[test]
    fn result_is_fully_connected() {
        let mut rng = SmallRng::seed_from_u64(42);
        let config = GeneratorConfig {
            pitch: crate::planet::Pitch { tile: 4.0, story: 5.0 },
            seed: crate::seed::Seed::new(42),

            max_rooms: 0,
            min_room_xz: 3,
            max_room_xz: 6,
            min_room_y: 1,
            max_room_y: 6,
        };
        let ag = abstract_graph::generate_topology(&mut rng, 10, &config);
        let level = assign_positions(&ag);
        assert!(level.is_fully_connected(), "positioned level should be fully connected");
    }
}
