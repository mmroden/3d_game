# Level Generation Architecture: Sweep-Based Pipeline

## Problem

The generator previously conflated topology (which rooms connect to which) with spatial layout (where rooms are placed on a grid). The single frontier loop checked `graph.is_free(target_pos)` during topology decisions — if a room couldn't fit spatially, the connection was never made. This produced sparse levels and rooms that overlapped.

The level generation pipeline should work in iterative sweeps, each consuming the output of the previous one.

## Sweep Architecture

```
Sweep 1: Topology          →  AbstractGraph (rooms + edges, no positions)
Sweep 2: Spatial Layout     →  LevelGraph (rooms with grid positions, corridors filling gaps)
Sweep 3: Cell Classification →  CellGrid per room (Interior/Edge/Corner/ConnectorGap)
Sweep 4: Geometry Assembly   →  MeshPlacement for walls/floors/ceilings/doors
Sweep 5: Collision           →  CollisionBox for physics
Sweep 6: Furnishing          →  MeshPlacement for props (themed, density-driven)
Sweep 7: Lighting            →  LightSource + fixture meshes
Sweep 8: Entity Spawns       →  Enemy positions, loot positions
Sweep 9: Portal              →  Level exit at farthest room (BFS)
Sweep 10: Godot Instantiation → Scene nodes for all of the above
```

### Sweep 1: Abstract Graph (topology only)

**Module:** `abstract_graph.rs`

Build a random connected graph of rooms with no spatial information.

- Generate N rooms using `generate_room()` (random extents, auto-connectors)
- Build a spanning tree: for each new room, pick a random existing room as parent, find compatible connectors (parent facing == child facing.opposite()), record the edge
- Optionally add cycle edges for loops
- Output: `AbstractGraph` — petgraph `UnGraph<RoomTemplate, ConnectorPair>` + root index

Key property: topology decisions are NEVER gated by spatial availability.

### Sweep 2: Spatial Layout

**Module:** `spatial_layout.rs`

Walk the abstract graph in BFS order, assign grid positions to each room.

- Place root at `[0, 0, 0]`
- For each BFS edge (parent → child): probe outward from the parent connector along its facing direction until all child cells are free. Generate a corridor of the exact length needed to fill the gap.
- Corridors are generated on-the-fly via `make_corridor(facing, length)` — no fixed corridor templates
- Output: `LevelGraph` (same struct as before, with `PlacedRoom.grid_pos` set)

Overlap guarantee: by construction, each child is placed at the minimum distance that avoids collision.

Vertical connections (PosY/NegY) work identically — facing determines direction, corridor is `[1, length, 1]`.

### Sweeps 3-9: Assembly (existing, per-room)

**Module:** `level_assembly.rs` (orchestrator), delegates to `cell.rs`, `room_assembler.rs`, `room_furnisher.rs`, `portal.rs`

For each room in the positioned graph:
- **Sweep 3:** `CellGrid::new()` — classify cells by boundary structure
- **Sweep 4:** `assemble_from_grid()` — walls, floors, ceilings, doors
- **Sweep 5:** `collision_boxes_from_grid()` — physics colliders
- **Sweep 6:** `CellGrid::populate()` — themed props (density-driven, entrance-aware)
- **Sweep 7:** `light_fixtures()` — ceiling lights with co-located OmniLight3D sources
- **Sweep 8:** Enemy/loot spawn position extraction

Then globally:
- **Sweep 9:** Portal placed at BFS-farthest room from start

### Sweep 10: Godot Instantiation

**Module:** `level_manager.rs` (void-nodes crate)

Consumes all mesh placements, collision boxes, light sources, enemy positions, portal position. Creates Godot scene nodes. Loose props wrapped in RigidBody3D with collision shapes and angular/linear velocity for zero-g tumble.

## Data Flow

```
GeneratorConfig
    ↓
generate_topology(rng, config) → AbstractGraph
    ↓
assign_positions(abstract_graph, rng) → LevelGraph
    ↓
spawn_list_full(level_graph, cell_size, seed) → (meshes, lights, enemies, colliders)
    ↓
portal_position(level_graph, cell_size) → [f32; 3]
    ↓
Godot instantiation
```

## Key Design Decisions

1. **Rooms are procedurally generated** — no hardcoded templates. `generate_room()` produces random extents with auto-computed connectors.
2. **Corridors are dynamic** — length computed to fill the spatial gap. No fixed corridor templates.
3. **All rooms have vertical connectors** — PosY/NegY connectors on every room, enabling floor/ceiling connections as first-class.
4. **Cell classification ignores Y-axis for prop placement** — `has_perpendicular_pair()` only considers XZ axes, so single-story rooms get Interior cells and receive center props.
5. **Columns skip entrance adjacency** — `populate()` checks for ConnectorGap neighbors before placing corner props.
6. **Portal uses BFS distance** — `farthest_room_from()` places the exit at maximum graph distance from spawn.
7. **No artificial room count limit** — the generator grows until the topology is satisfied, capped at 200 as a safety valve.

## Files

| File | Sweep | Role |
|------|-------|------|
| `abstract_graph.rs` | 1 | Topology generation |
| `spatial_layout.rs` | 2 | Position assignment, corridor generation |
| `generator.rs` | 1+2 | Orchestrates sweep 1 → sweep 2 |
| `level_assembly.rs` | 3-9 | Orchestrates per-room assembly |
| `cell.rs` | 3, 6 | Cell classification + prop placement |
| `room_assembler.rs` | 4, 5 | Geometry + collision |
| `room_furnisher.rs` | 7 | Lighting |
| `portal.rs` | 9 | Level exit placement |
| `level_manager.rs` | 10 | Godot instantiation |
