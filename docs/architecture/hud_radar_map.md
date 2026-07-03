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
- **Filtering reuses the dormancy/culling authority**: `is_visible_in_tree`
  skips dormant minions and room-culled enemies — no aggro bookkeeping, no
  parallel visibility logic. The radar sells "what's around you," which is
  exactly what the culling graph already decides.
- **SBS confinement is structural**: the arrow pool (16 `Polygon2D`s,
  Faucet-style — built once, visibility-flipped) lives under the HUD's
  `safe_area` control, so the SBS central band applies to arrows by
  parenting, not arithmetic. Cost: tens of unprojects per frame — no
  throttling needed.

## Recon Map (`Unlock::FogMap`)

A corner widget (bottom-left of the safe band) drawing the rooms the player
has visited and where unexplored corridors leave them.

- **Built on the retained LevelGraph, never a parallel structure**
  (`void-logic/src/level_map.rs::map_view`): the same petgraph the culling
  reads. Only visited rooms appear; a visited→unvisited edge renders as a
  short **frontier stub** (`FRONTIER_STUB` in unit space) pointing along the
  corridor without revealing the far room. Coordinates normalize over the
  whole footprint so the frame never re-scales mid-exploration.
- **One detection, two consumers**: `LevelManager::update_room_culling`'s
  room-change detection both re-culls and emits `room_changed` (deferred —
  the handler binds back into LevelManager for the graph). GameManager
  visits the room in RunState (`visit_room`, per-level state) and pushes a
  fresh view to the HUD **only on a first visit** — the map redraws per new
  room, never per frame.
- `MapPanel` (void-nodes/ui) is pure presentation: cached packed arrays, a
  custom `draw()`, `queue_redraw` on push.

## Testing altitude (both features)

Geometry/projection rules are pinned in Rust (fast, exhaustive). The GUT
suites (`test_radar_hud.gd`, `test_fog_map.gd`) cover only shell contracts —
unlock gating, group/visibility filtering, the room-changed wire — each
against one pinned level (see the pinned-seed anchors in level_assembly).
