# HUD Intel: Enemy Radar & Recon Map

> **Status:** implemented. Both are green permanent unlocks; the HUD renders
> them only while GameManager pushes the flags (`set_unlock_flags` — the HUD
> never invents state).

## Enemy Radar (`Unlock::Radar`)

Edge arrows pointing at enemies that are not visibly on screen.

- **Placement math is pure** (`void-logic/src/radar.rs::edge_arrow`):
  band-local coordinates in, arrow position/angle out. On-screen targets get
  no arrow; behind-camera targets always get one, flipped through the band
  center; every placement is clamped to the band border inset by
  `ARROW_INSET`. Degenerate directions return `None`, never NaN.
- **The shell feeds it** (`hud.rs::update_radar`): every Playing frame,
  project each member of the `"enemies"` group through the player camera
  (`unproject_position` + `is_position_behind`).
- **Scope is deliberately LOCAL** (owner's calls, 2026-07-04): standing in
  a room, the radar hears that room and its corridor mouths — never past a
  door; standing in a corridor, the corridor chain and the rooms it joins.
  One authority: `LevelGraph::radar_scope` rides the same room/corridor
  cost model as `visible_from`, budget picked by where you stand (room 0,
  corridor 1); rendering still lights `RENDER_ROOM_DEPTH` (2) rooms deep.
  `LevelManager::radar_contacts` resolves the enemy ids in that set;
  GameManager pushes them to the HUD on every room change
  (`set_radar_contacts`); the HUD draws ONLY pushed contacts (empty set =
  silent radar). `is_visible_in_tree` still filters dormant minions on top.
  Enemies activated mid-fight join at the next room change — they spawn in
  the player's own room, on screen anyway. The Threat Tracker's map dots
  share this same pushed set.
- **SBS confinement is structural**: the arrow pool (16 `Polygon2D`s,
  Faucet-style — built once, visibility-flipped) lives under the HUD's
  `safe_area` control, so the SBS central band applies to arrows by
  parenting, not arithmetic. Cost: tens of unprojects per frame — no
  throttling needed.

## Recon Map (`Unlock::FogMap`)

A corner widget (bottom-left of the safe band, 340px) drawing the level's
REAL rectilinear footprints — every visited room and corridor as its
top-down rectangle (playtest 2026-07-04: abstract dots were useless).

- **Built on the retained LevelGraph, never a parallel structure**
  (`void-logic/src/level_map.rs::map_rects`): the same petgraph the culling
  reads, including its corridor nodes. One uniform scale over the whole
  footprint — rectangles keep their true aspect, relative positions are
  exact, and the frame never re-scales mid-exploration. Fog rules: unvisited
  rooms never appear; an unvisited CORRIDOR adjacent to explored space draws
  faint — real geometry pointing into the dark without revealing the room
  beyond it. Opacity is the fog on screen: the current room near-opaque,
  explored space translucent, frontiers barely there — overlapping stories
  stay legible through the alpha.
- **One detection, two consumers**: `LevelManager::update_room_culling`'s
  room-change detection both re-culls and emits `room_changed` (deferred —
  the handler binds back into LevelManager for the graph). GameManager
  visits the room in RunState (`visit_room`) and re-pushes the view on
  EVERY room change — the fog only lifts on first visits, but the current
  marker must follow the player through known rooms too (playtest
  2026-07-04: it stuck on the last new room).
- **The live player marker**: each push carries the world→unit projection
  (`level_map::map_projection`, the same normalization as the rects), and
  MapPanel draws a heading triangle at the ship's actual position each
  frame it is visible. North-up map, turning arrow — the 6DOF-friendly
  convention (rotating a rectilinear map reads as soup; the genre's full
  answer, a rotating 3D automap screen, is a future feature).
- `MapPanel` (void-nodes/ui) is otherwise pure presentation: cached packed
  arrays, a custom `draw()`, `queue_redraw` on push (plus per-frame while
  visible, for the marker only).

## Testing altitude (both features)

Geometry/projection rules are pinned in Rust (fast, exhaustive). The GUT
suites (`test_radar_hud.gd`, `test_fog_map.gd`) cover only shell contracts —
unlock gating, group/visibility filtering, the room-changed wire — each
against one pinned level (see the pinned-seed anchors in level_assembly).
