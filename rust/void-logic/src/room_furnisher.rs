use crate::asset_catalog::{WALL_ADJACENT_PROPS, CENTER_PROPS, CORNER_PROPS};
use crate::room_assembler::MeshPlacement;
use crate::room_template::{Connector, ConnectorFacing, RoomTemplate};

/// Room furnishing density — controls how many props are placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomDensity {
    /// ~20% wall, ~12% center/corner — hangars, open spaces.
    Sparse,
    /// ~50% wall, ~33% center/corner — normal rooms (original behavior).
    Normal,
    /// ~80% wall, ~60% center, ~66% corner — storage rooms, labs.
    Dense,
}

/// Place props inside a room based on its template, active connectors, and a seed.
///
/// Returns mesh placements for furniture, crates, columns, etc. in **meter coordinates**.
/// Wall-adjacent props are offset toward the wall. Center props sit at cell midpoints.
/// No props are placed at active connector cells (openings must stay clear).
pub fn furnish(
    template: &RoomTemplate,
    active_connectors: &[Connector],
    world_origin: [f32; 3],
    cell_size: f32,
    seed: u64,
    density: RoomDensity,
) -> Vec<MeshPlacement> {
    let mut out = Vec::new();
    let ex = template.extents[0] as i32;
    let ez = template.extents[2] as i32;
    let mut rng = SimpleRng::new(seed);

    // Density-driven probability thresholds: (numerator, denominator).
    // A prop is placed when `rng % denom < num`.
    let (wall_num, wall_den, center_num, center_den, corner_num, corner_den) = match density {
        RoomDensity::Sparse  => (1, 5, 1, 8, 1, 6),
        RoomDensity::Normal  => (1, 2, 1, 3, 1, 3),
        RoomDensity::Dense   => (4, 5, 3, 5, 2, 3),
    };

    // Collect active connector cells so we can skip them (XZ projection).
    let connector_cells: Vec<(i32, i32)> = active_connectors
        .iter()
        .filter(|c| template.connectors.contains(c))
        .map(|c| (c.offset[0], c.offset[2]))
        .collect();

    // Pre-compute path-reserved cells: cells that must stay free for flight paths.
    let reserved = path_reserved_cells(template, active_connectors);

    for cx in 0..ex {
        for cz in 0..ez {
            // Skip active connector cells — openings must stay clear.
            if connector_cells.contains(&(cx, cz)) {
                continue;
            }

            let cell_center_x = world_origin[0] + (cx as f32 + 0.5) * cell_size;
            let cell_center_z = world_origin[2] + (cz as f32 + 0.5) * cell_size;
            let y = world_origin[1];

            let on_boundary = cx == 0 || cx == ex - 1 || cz == 0 || cz == ez - 1;
            let on_reserved_path = reserved.contains(&(cx, cz));

            if on_boundary {
                // Determine which faces of this cell are sealed walls (boundary + no active connector).
                let wall_faces = sealed_wall_faces(template, active_connectors, cx, cz, ex, ez);

                // Try to place a wall-adjacent prop against one of the walls.
                if !wall_faces.is_empty() {
                    let face = wall_faces[rng.next_usize() % wall_faces.len()];
                    if rng.next_usize() % wall_den < wall_num {
                        let prop = &WALL_ADJACENT_PROPS[rng.next_usize() % WALL_ADJACENT_PROPS.len()];
                        // Skip blocking props on reserved path cells.
                        if !(prop.blocks_flight && on_reserved_path) {
                            let (offset_x, offset_z, rot) = wall_adjacent_offset(face, cell_size);
                            out.push(MeshPlacement {
                                scene: prop.scene,
                                position: [cell_center_x + offset_x, y, cell_center_z + offset_z],
                                rotation_x: 0.0,
                                rotation_y: rot,
                            });
                        }
                    }
                }

                // Corner props where two walls meet — skip if blocking and on path.
                if wall_faces.len() >= 2 && rng.next_usize() % corner_den < corner_num {
                    let prop = &CORNER_PROPS[rng.next_usize() % CORNER_PROPS.len()];
                    if !(prop.blocks_flight && on_reserved_path) {
                        out.push(MeshPlacement {
                            scene: prop.scene,
                            position: [cell_center_x, y, cell_center_z],
                            rotation_x: 0.0,
                            rotation_y: 0.0,
                        });
                    }
                }
            } else {
                // Interior cell — place center props.
                if rng.next_usize() % center_den < center_num {
                    let prop = &CENTER_PROPS[rng.next_usize() % CENTER_PROPS.len()];
                    // Skip blocking props on reserved path cells.
                    if !(prop.blocks_flight && on_reserved_path) {
                        out.push(MeshPlacement {
                            scene: prop.scene,
                            position: [cell_center_x, y, cell_center_z],
                            rotation_x: 0.0,
                            rotation_y: 0.0,
                        });
                    }
                }
            }
        }
    }

    out
}

/// Pre-compute cells that must remain clear for flight paths between openings.
/// Uses BFS to find shortest paths between all pairs of active connectors.
fn path_reserved_cells(
    template: &RoomTemplate,
    active_connectors: &[Connector],
) -> std::collections::HashSet<(i32, i32)> {
    use std::collections::{HashSet, VecDeque, HashMap};

    let ex = template.extents[0] as i32;
    let ez = template.extents[2] as i32;

    let openings: Vec<(i32, i32)> = active_connectors
        .iter()
        .filter(|c| template.connectors.contains(c))
        .map(|c| (c.offset[0], c.offset[2]))
        .collect();

    let mut reserved = HashSet::new();

    if openings.len() < 2 {
        return reserved;
    }

    // For each pair of openings, find shortest path via BFS and reserve those cells.
    for i in 0..openings.len() {
        for j in (i + 1)..openings.len() {
            let start = openings[i];
            let goal = openings[j];

            // BFS with parent tracking for path reconstruction.
            let mut visited: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
            let mut queue = VecDeque::new();
            queue.push_back(start);
            visited.insert(start, start);

            let mut found = false;
            while let Some((cx, cz)) = queue.pop_front() {
                if (cx, cz) == goal {
                    found = true;
                    break;
                }
                for (dx, dz) in &[(0, 1), (0, -1), (1, 0), (-1, 0)] {
                    let nx = cx + dx;
                    let nz = cz + dz;
                    if nx >= 0 && nx < ex && nz >= 0 && nz < ez && !visited.contains_key(&(nx, nz)) {
                        visited.insert((nx, nz), (cx, cz));
                        queue.push_back((nx, nz));
                    }
                }
            }

            // Trace path back and reserve cells.
            if found {
                let mut cur = goal;
                while cur != start {
                    reserved.insert(cur);
                    cur = visited[&cur];
                }
                reserved.insert(start);
            }
        }
    }

    reserved
}

/// Return which boundary faces of a cell are sealed walls (not openings).
fn sealed_wall_faces(
    _template: &RoomTemplate,
    active_connectors: &[Connector],
    cx: i32,
    cz: i32,
    ex: i32,
    ez: i32,
) -> Vec<ConnectorFacing> {
    let candidates = [
        (ConnectorFacing::NegX, cx == 0),
        (ConnectorFacing::PosX, cx == ex - 1),
        (ConnectorFacing::NegZ, cz == 0),
        (ConnectorFacing::PosZ, cz == ez - 1),
    ];

    candidates
        .iter()
        .filter(|(facing, is_boundary)| {
            *is_boundary && !is_active_connector_xz(active_connectors, *facing, cx, cz)
        })
        .map(|(facing, _)| *facing)
        .collect()
}

/// Check if any active connector at column (cx, *, cz) has the given facing.
/// This is a 2D (XZ) projection — any Y level counts.
fn is_active_connector_xz(
    active: &[Connector],
    facing: ConnectorFacing,
    cx: i32,
    cz: i32,
) -> bool {
    active.iter().any(|c| c.facing == facing && c.offset[0] == cx && c.offset[2] == cz)
}

/// Compute (offset_x, offset_z, rotation_y) for a wall-adjacent prop.
/// Offsets move from cell center toward the wall.
fn wall_adjacent_offset(facing: ConnectorFacing, cell_size: f32) -> (f32, f32, f32) {
    use std::f32::consts::{FRAC_PI_2, PI};
    let offset = cell_size * 0.25; // 1.0m for 4m cells — leaves 1m clearance from wall
    match facing {
        ConnectorFacing::NegX => (-offset, 0.0, 0.0),
        ConnectorFacing::PosX => (offset, 0.0, PI),
        ConnectorFacing::NegZ => (0.0, -offset, -FRAC_PI_2),
        ConnectorFacing::PosZ => (0.0, offset, FRAC_PI_2),
        // Y-axis faces don't have wall-adjacent offsets — no XZ displacement.
        ConnectorFacing::NegY | ConnectorFacing::PosY => (0.0, 0.0, 0.0),
    }
}

// ── Flyable path validation ─────────────────────────────────────────────

/// Check that flight paths exist between all pairs of active connector openings
/// after props have been placed. Uses BFS on a cell-level occupancy grid.
///
/// Props with `blocks_flight: true` mark cells as occupied.
pub fn flight_paths_clear(
    template: &RoomTemplate,
    active_connectors: &[Connector],
    props: &[MeshPlacement],
    cell_size: f32,
) -> bool {
    use std::collections::{HashSet, VecDeque};

    let ex = template.extents[0] as i32;
    let ez = template.extents[2] as i32;

    // Build occupancy grid from blocking props.
    let mut blocked: HashSet<(i32, i32)> = HashSet::new();
    for p in props {
        let cx = (p.position[0] / cell_size).floor() as i32;
        let cz = (p.position[2] / cell_size).floor() as i32;
        if is_blocking_prop(p.scene) {
            blocked.insert((cx, cz));
        }
    }

    // Find active connector cell positions (XZ projection).
    let openings: Vec<(i32, i32)> = active_connectors
        .iter()
        .filter(|c| template.connectors.contains(c))
        .map(|c| (c.offset[0], c.offset[2]))
        .collect();

    // For rooms with 0 or 1 opening, no path needed.
    if openings.len() < 2 {
        return true;
    }

    // BFS from first opening to all others.
    let start = openings[0];
    let mut visited: HashSet<(i32, i32)> = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(start);
    visited.insert(start);

    while let Some((cx, cz)) = queue.pop_front() {
        for (dx, dz) in &[(0, 1), (0, -1), (1, 0), (-1, 0)] {
            let nx = cx + dx;
            let nz = cz + dz;
            if nx >= 0 && nx < ex && nz >= 0 && nz < ez
                && !visited.contains(&(nx, nz))
                && !blocked.contains(&(nx, nz))
            {
                visited.insert((nx, nz));
                queue.push_back((nx, nz));
            }
        }
    }

    // All openings must be reachable.
    openings.iter().all(|o| visited.contains(o))
}

/// Check if a prop scene path corresponds to a flight-blocking prop.
fn is_blocking_prop(scene: &str) -> bool {
    use crate::asset_catalog;

    let all_props = asset_catalog::WALL_ADJACENT_PROPS
        .iter()
        .chain(asset_catalog::CENTER_PROPS)
        .chain(asset_catalog::CORNER_PROPS)
        .chain(asset_catalog::CEILING_PROPS);

    all_props
        .filter(|p| p.blocks_flight)
        .any(|p| p.scene == scene)
}

// ── Light fixtures ──────────────────────────────────────────────────────

/// A light source co-located with a fixture mesh.
#[derive(Debug, Clone)]
pub struct LightSource {
    pub position: [f32; 3],
    pub range: f32,
    pub energy: f32,
}

/// Place light fixtures on ceilings and return both the fixture mesh placement
/// and the co-located light source. One fixture per cell.
pub fn light_fixtures(
    template: &RoomTemplate,
    world_origin: [f32; 3],
    cell_size: f32,
) -> Vec<(MeshPlacement, LightSource)> {
    use crate::asset_catalog::CEILING_LIGHTS;
    use crate::room_assembler::CELL_HEIGHT;

    let mut out = Vec::new();
    let ex = template.extents[0] as i32;
    let ey = template.extents[1] as i32;
    let ez = template.extents[2] as i32;

    for cx in 0..ex {
        for cy in 0..ey {
            for cz in 0..ez {
                // Alternate between ceiling light variants based on position
                let fixture = &CEILING_LIGHTS[((cx + cy + cz) as usize) % CEILING_LIGHTS.len()];

                let fixture_y = world_origin[1] + cy as f32 * CELL_HEIGHT + CELL_HEIGHT - 0.1;
                let mesh = MeshPlacement {
                    scene: fixture.scene,
                    position: [
                        world_origin[0] + (cx as f32 + 0.5) * cell_size,
                        fixture_y,
                        world_origin[2] + (cz as f32 + 0.5) * cell_size,
                    ],
                    rotation_x: 0.0,
                    rotation_y: 0.0,
                };

                let light = LightSource {
                    position: [
                        mesh.position[0] + fixture.light_offset[0],
                        mesh.position[1] + fixture.light_offset[1],
                        mesh.position[2] + fixture.light_offset[2],
                    ],
                    range: fixture.range,
                    energy: fixture.energy,
                };

                out.push((mesh, light));
            }
        }
    }

    out
}

// ── RNG ─────────────────────────────────────────────────────────────────

/// Minimal deterministic RNG (xorshift64) for prop selection.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_usize(&mut self) -> usize {
        self.next_u64() as usize
    }
}

#[cfg(test)]
#[path = "room_furnisher_tests.rs"]
mod tests;
