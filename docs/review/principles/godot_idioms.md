# Principle: Godot idioms

The Zen of the Tool applied to the engine. Godot 4 has a way it wants to be
used: a scene tree of nodes with servers underneath, signals for
notification, resources for shared data, viewports and cameras owned by the
renderer, an XR path for stereo, physics owned by the physics server on its
own tick, and a deferred boundary between them. Code that reproduces an
engine facility by hand from lower-level pieces is syntactically Godot and
semantically fighting it; the bill arrives as performance, as edge cases the
engine already handles, and as maintenance of a subsystem the engine already
maintains.

The owner's rule (2026-09-15): use the tools Godot provides, for cockpits,
stereo, and everything else, but only where they meet our needs as
storytellers. So the question this brief holds every Godot-facing hunk to
has two halves: does the engine already do this, and if the code does not
use that, what storytelling need does it name that the facility fails? A
hand-rolled subsystem with its need stated where the next reader will find
it is a decision; without one it is a finding. Decisions still open on this
question (the Frame convergence policy, MSAA under two views on Metal) are
recorded in `../ground_truth.md` under "Open decisions"; a diff there is
reviewed for keeping the question open, not for answering it, and this
brief carries no standing verdict on them. The stereo rig itself is
decided (the XR interface path, ground truth "Immersive 3D is the goal").

## Checks

1. **Engine facility reproduced by hand.** For every subsystem-sized piece of
   Godot-facing code, name the engine facility that does the same thing:
   stereo rendering (`use_xr` + `XRInterface`), camera smoothing and
   interpolation (physics interpolation, `Camera3D` modes), culling
   (`VisibleOnScreenNotifier3D`, visibility ranges, occlusion culling),
   LOD (mesh LOD, visibility ranges), UI scaling (stretch modes, content
   scale), audio positioning (`AudioListener3D`, buses), tweening
   (`Tween`), timers (`Timer`, `SceneTreeTimer`), input mapping
   (`InputMap`), object pooling of engine resources (server RIDs). A
   hand-rolled version is a finding with the facility named, unless the
   code or the ground truth states the storytelling need the facility
   fails; cite that statement or its absence.
2. **Nodes versus servers.** Per-frame hot paths that create, move, or
   toggle many objects belong on the servers (`RenderingServer`,
   `PhysicsServer3D`) with RIDs, not on thousands of nodes; a handful of
   long-lived objects belong on nodes, not on hand-managed RIDs. Cite the
   count and the path.
3. **Viewports and cameras.** One rendered viewport per output surface.
   Extra `SubViewport`s are for genuinely separate render targets (a
   minimap, a texture), never for a second copy of the main scene. The
   current camera is the renderer's, set through `make_current`, not a
   pose pushed by hand each frame; when the pose must be driven, the
   engine's interpolation is opted out deliberately and the reason is on
   the line. Asymmetric-frustum stereo (shifted projections) and toe-in
   convergence (rotated cameras) differ: toe-in produces vertical parallax
   at the frame edges. Cite which one a rig does; ours is the shifted
   frustum inside the display interface (docs/design/xr_rig.md §3), and a
   change there is a projection function plus the disparity contract's
   reading, per the ground truth.
4. **Signals, not polling.** State changes flow by signal to typed
   constants; a `process` that polls another node's state for a change is
   a finding. Signals connect at the right lifecycle point and disconnect
   on free; a connection to a node that may be freed first is a
   use-after-free.
5. **Lifecycle.** `_ready` runs children first, then parent, siblings in
   tree order; anything depending on a sibling's `_ready` is deferred.
   `_enter_tree`/`_exit_tree` pair. `queue_free` is the only free for nodes
   in the tree; `free` only for nodes never added. A stored `Gd<T>` is
   checked with `is_instance_valid` before each use.
6. **Physics on its tick.** Bodies move in `_physics_process`, through
   `move_and_slide` or forces, never by `set_position` per render frame.
   Collision layers and masks are declared, not toggled; where they must
   change, via `set_deferred` outside the physics callback. Jolt is the
   physics backend; its constraints (no direct state writes inside
   callbacks) are in `docs/architecture/godot_jolt_architecture.md`.
7. **Resources.** Shared data is a `Resource` loaded once and referenced,
   never rebuilt per instance; `preload` versus `load` chosen by lifetime;
   `ResourceLoader::load` never on a play path (Faucet Principle).
   Materials and meshes are shared, not duplicated per node unless
   per-instance state is needed, and then `duplicate()` is deliberate.
8. **Rendering settings live in the project.** Renderer choice, MSAA,
   scaling, VRS, shadow settings, environment are project settings or
   `Environment`/`CameraAttributes` resources, not per-viewport code
   unless a viewport genuinely differs. A setting applied in code that the
   project file could hold is a finding.
9. **Autoloads and globals.** Autoloads are for engine-wide singletons the
   scene tree needs (the mediator); everything else is owned by its parent
   and passed down. A second autoload for convenience is a finding.
10. **UI in Control, 3D in Node3D.** HUD and menus are `Control` nodes under
    a `CanvasLayer`, anchored and themed; a 3D quad with a viewport texture
    is used only where the UI must exist in world space (the SBS UI plane
    is that case; say whether it still would be under the XR path).
    Theme resources over per-node style overrides.
11. **Input through the map.** Actions from `InputMap`, read with
    `Input.is_action_*` or `_unhandled_input`, never raw keycodes in
    gameplay code; `_unhandled_input` for gameplay, `_gui_input` for
    controls, so UI consumes first.
12. **Audio through buses.** `AudioStreamPlayer3D` for positional,
    `AudioStreamPlayer` on a named bus for non-positional; mixing and
    effects on buses in the project, not per-player volume math.
13. **Groups and node paths.** Typed constants for node paths (Zen of Rust
    audit); groups for broadcast to many, signals for one-to-many with
    payload; `get_node` on a hard-coded path from a distant ancestor is a
    coupling finding.
14. **Engine version awareness.** A workaround for an engine bug carries
    the issue number and the version it is fixed in, so it can be removed
    (the Metal MSAA note on `stereo::msaa_allowed` is the model). A
    workaround without either is plaque.

## Procedure

For each touched file under `rust/void-nodes/` and `godot/`,
`get_symbols_overview` and read the whole file. For each engine class used,
ask what the engine offers at the level above it. For each `SubViewport`,
`Camera3D`, server call, or per-frame pose write, run check 1 and 3. Use
the Godot class reference for facts; cite the property or method by its
documented name, and mark a claim about engine internals `PLAUSIBLE` unless
the documentation states it.

## Severity guidance

An engine subsystem reproduced by hand on a rendering or physics hot path
with no stated need: BLOCKER, with the facility named and a `to settle:`
measurement for the performance claim; with the need stated in code only:
CONCERN, to be promoted into the ground truth; with the need in the ground
truth: not a finding. Toe-in stereo: cite it; the projection is one function of the display
interface and the ground truth names where it is measured.
Node-per-object on a hot path
where the server is the tool: CONCERN. Polling instead of signals, physics
moved per render frame, resources rebuilt per instance: CONCERN. Settings
in code, raw keycodes, per-node style overrides: NIT. Undated workaround:
CONCERN.
