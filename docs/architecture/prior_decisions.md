# Prior Architectural Decisions (extracted from plan history)

## From: Cell Model Architecture (hashed-skipping-simon)

- Cells are explicit spawn points for objects AND enemies
- Room-level themes ("warehouse" = shelves, "command" = displays)
- One occupant per cell with density control
- CellGrid/CellKind/Cell structs — **implemented**

## From: Zen of Rust Audit (quizzical-tinkering-puddle)

### Chunk 1: De-stringify — **partially implemented**
- Constants module for signals, methods, actions, node paths — done
- Scene path dedup — done

### Chunk 2: Cell type system overhaul — **NOT YET IMPLEMENTED**
- `FaceRole { Sealed, Passage, Opening }` replaces ad-hoc floor/ceiling/wall logic
- "floor" and "ceiling" are 2.5D concepts — in true 3D, they're just -Y and +Y faces
- Full cartesian join: `TextureSource x FaceDirection x FaceRole → MeshPlacement`
- Cell classification should be derived from face roles, not primary
- CellSize should be derived from mesh bounds, not passed as parameter

### Chunk 2B: True 3D geometry — **partially implemented**
- All 6 faces checked for boundary status — done in cell.rs
- Y connectors for ConnectorGap — done
- Corner pairs should expand to 3D (XY, YZ pairs, not just XZ) — NOT YET
- 3D flight path clearing (3D BFS, not 2D) — NOT YET

## From: Aperture Alignment Fix (imperative-squishing-cosmos)

- Connectors track full `Connector { offset, facing }` not just facing direction
- Active connectors include Y-level information to prevent wrong-floor activation
- **Implemented**

## From: Corner Gap Fix (starry-bouncing-cat)

- Corner pieces offset using CellExtents type system, not raw constants
- **Implemented**

## From: Mesh Pivot Fix (cozy-stargazing-hartmanis)

- All Quaternius meshes are center-pivot (extend ±2m from origin)
- Meshes placed at cell center with rotation only
- **Implemented**

## From: Y-axis Coordinate Fix (tingly-fluttering-bachman)

- Quaternius meshes have 5m visual cell height
- `story_height` is separate from `cell_size` (tile_width)
- **Implemented**

## Reconciliation: What's still outstanding

1. **FaceRole enum** — the cell system still uses `CellKind` + `sealed_faces` instead of per-face `FaceRole`. This is the Zen audit Chunk 2A work.
2. **TextureSource axis** — WallSet doesn't have a source type. Only Quaternius assets exist currently.
3. **CellSize as derived property** — still passed as a parameter everywhere.
4. **3D corner pairs** — only XZ corner pairs are handled; XY and YZ corners are not.
5. **Graph-first generation** — topology conflated with spatial layout. This is the current work.
