# Architectural ground truth

Repo-specific facts every reviewer holds the diff to. Documents drift from
code, this one included, and nothing checks this file automatically; the
check is the reviewer's, made during the review. Before citing a site named
here, open it with `find_symbol` and confirm both that it still exists and
that it still does what this file says. Report drift under `Gaps`; never
judge the diff against a phantom.

The worked example, from the owner's read of this directory's first draft
(2026-09-15): the draft cited `HULL_SALT`, and a hull.rs file under
`rust/void-logic/src/`, as the salted-entropy pattern. `find_symbol` found
the symbol nowhere and the file did not exist; `git log -S HULL_SALT`
showed the salts coalesced into one registry, `seed::salt`, on 2026-07-05.
The site had moved two months before the doc was written. That is the drift
class this file guards against.

Primary design documents: `docs/architecture/game_plan.md`,
`docs/architecture/faucet_principle.md`, `docs/architecture/prior_decisions.md`.

## Model/shell split, compiler-enforced

`rust/void-logic` is pure logic with zero Godot dependency. It stays
deterministic and effect-free: no OS entropy, no I/O, no build-flavor policy.
`cfg!(debug_assertions)` deciding runtime behavior is a policy leak. Policy
belongs to the shell (`rust/void-nodes`), mechanism to the model. Separate
crates and module walls enforce this at compile time; convention alone is not
enforcement.

## GameManager is the sole mediator

All state mutations route through GameManager via signals into its private
`RunState`. `nodes/views/` and `nodes/ui/` must not import each other's types;
lint tests enforce this and must still pass for new modules. `GameOptions` is
the single source of configuration truth: consumers cache a copy seeded by
GameManager's startup broadcast, never an independent default literal.

## Lifecycle is FSM-driven

`GamePhase` plus `can_transition_to()` own the game lifecycle. New lifecycle
behavior (level generation, resets, screens) hangs off phase transitions, not
off `ready()`, ad-hoc calls, or build targets. An initial phase set inline in
`ready()` is undone by siblings' later `ready()`; it must be deferred.

## Newtype discipline

Domain values get newtypes; `Health`, `Damage`, `Shield` are the established
pattern. Raw scalars do not cross module or FFI boundaries. `as` casts at the
Godot boundary are a smell; conversions live in one place on a domain type.

## No stringly-typed identifiers

Signals, methods, and node paths go through typed constants or enums
(`rust/void-nodes/src/nodes/constants.rs`), per the Zen of Rust audit.

## One identity per concept, strings only at the boundary

The established pattern is `EnemyKey` (2026-07 identity coalescence): the
human-authored key, strongly typed as an interned newtype, flowing unchanged
from TOML through model, shell, saves, and UI. Raw strings exist at exactly two
places: TOML serde and the Godot/save crossing. No concept carries a second
runtime representation (array index, hand-authored numeric id, "crossing id").
One accessor resolves the identity, one boundary parser mints it, zero
remappers translate it. `SceneId` is the same pattern for catalog scenes.

## One entropy faucet, salted streams

Exactly one nondeterministic draw affects gameplay:
`GameManager::fresh_run_seed`. Everything else derives from the run seed via
`Seed::for_level` and `Seed::for_planet` and consumes it through
domain-salted `SmallRng` streams: the seed is mixed with a named salt from
the `seed::salt` registry in `rust/void-logic/src/seed.rs` (`HULL`,
`PANEL_COURSE`, `PROP_ORIENT`, `ROOM_MIX`, `KIT_PICK`), the pattern at
`rust/void-logic/src/boss.rs:30`. Two domains seeding from the same raw seed
value consume the same bit-stream; that is a defect (correlated streams),
not merely a smell. A new domain gets a new salt in the registry, never an
inline constant. Unseeded audio and cosmetics are the accepted exception.

## Faucet Principle: allocation only at build time

Everything a level can contain is preallocated during level creation (manifest
in `void-logic`, pools such as `BoltPool` and `CloudPool` in `void-nodes`);
play only flips entities dormant or active. Structural allocation on a
during-play path (`instantiate()`, `ResourceLoader::load`, new nodes,
`queue_free` of gameplay entities) is a violation. Dormancy flips ride the
deferred boundary; direct collision or monitoring toggles from physics
callbacks or signal flushes are engine-blocked and strand entities
half-dormant. Transient cosmetic FX are the accepted exception. Standard:
`docs/architecture/faucet_principle.md`.

## LevelGraph is retained and opaque

The petgraph `LevelGraph` lives the level's lifetime in LevelManager and is
authoritative: `visible_from` for culling, future pathfinding and minimap on
the same graph. Its API is method-only; `pub` fields on it are a violation. Use
petgraph's algorithms rather than reimplementing them.

## Data is grammar, not pins

`rosters/` TOML is the creature data (open-set enemies, switches, curves,
emitters); `catalog/` owns kits and scenes via `SceneId`. Tests derive
expectations from the declared grammar and never pin shipped numbers or name
specific defs, so tuning never breaks tests. Property tests live in Rust; GUT
tests are shell contracts only, one pinned level per scenario with the seed
pinned by a Rust anchor test.

## Build and tooling coherence

The Makefile is the single entry point; every pipeline is a stage and its
inner steps are never invoked directly. Builds are reproducible and local.
Build flavor (debug/release) never selects gameplay behavior. Only one Godot
process runs at a time (GUT, `make check`, asset imports share `user://` and
the `.godot` cache). Downloads are declared and pinned in
`catalog/attributions.toml`; provenance is read by a census at ingestion,
never hand-transcribed. Asset audit gates are fidelity (every declared
material, texture, and face arrives; importer zero errors and warnings);
numeric thresholds are reported yardsticks, not gates.

## Code is symbol-addressable

Imports at the top of the module, never mid-file or inside functions. Policy
numbers are function defaults or type attributes, not bare module constants.
Edits are made symbolically; whole-file rewrites are for genuinely
module-level changes.

## Immersive 3D is the goal

Side-by-side stereo for xReal glasses is first-class. One full-window
`UIViewport`, never per-eye; each eye sees the central half; corner HUD in a
central safe-area band. No camera shake, ever; damage escalates via tints.
Panels serve any face (6DOF) via baked per-role variants, not runtime rotation.
The 3D world renders ONCE, through the root viewport under `use_xr`, by the
display interface the XRServer holds (docs/design/xr_rig.md): the
`SbsInterface` (`rust/void-nodes/src/nodes/views/sbs_interface.rs`) plays
the xReal as a crippled headset — one view in mono, two side by side, eyes
front, head still — and Godot's `OpenXRInterface` is the display whenever a
runtime came up at process start (`--xr-mode on`; the Meta XR Simulator on
the Mac via `make run-xr`, the Frame later). `void_logic::stereo::Display`
names the display in force; `views/openxr.rs` reads the XRServer for it.
The player's `XRCamera3D` under `Player/XROrigin3D` is the one camera; the `UIPlane` is a child of that origin (cockpit-locked). A second
render of the scene (a `SubViewport` per eye, a hand-driven camera) is a
finding, not an open decision.

## Open decisions

Questions the owner has left open. A diff in one of these areas is reviewed
for keeping the question open (isolation, no new tendrils, a measurement
path), not for answering it either way, and no brief carries a standing
verdict on one of them.

- **The stereo rig** — DECIDED 2026-09-17 (owner): the XR interface path,
  above; docs/design/xr_rig.md §7 carries the before/after measurement
  through the visual stage. What stays open inside it: the Frame's
  convergence policy (§5 there: `world_scale` law vs. relief-only remap —
  an in-headset experiment, dials exposed). Settled: the mono path (one
  interface at one view, working) and the OpenXR display (§6 there, on
  the Mac under the Meta XR Simulator); the UI as a composition layer,
  the action map and a headset capture path are phase 4's.
- **Convergence geometry.** The rig's convergence is shifted-frustum — the
  off-axis term in `void_logic::stereo::off_axis_projection`, read in code,
  not measured. The owner does not accept the frustum-vs-toe-in argument
  and holds it irrelevant to the goal; the projection is one swappable
  function of the interface, and the disparity contract in
  `rust/void-logic/tests/visual.rs` is where corner vertical parallax would
  be measured for either. A diff that changes the projection cites which it
  does.
- **MSAA under two views on Metal.** Godot 4.6.1's Metal driver asserts
  slicing a multisample array (per-view MSAA resolve), so
  `stereo::msaa_allowed` keeps MSAA single-view on that driver; TAA stays.
  Re-test on an engine upgrade before widening; a diff that widens it cites
  the run.

## Canonical sites for the house patterns

The reviewer opens every site in the third column before citing it and
confirms it exists and that the meaning in the second column still holds;
drift is reported under `Gaps`.

| Pattern | What it is | Canonical site to verify |
|---|---|---|
| Identity newtype | The authored TOML key, interned, flowing unchanged model to UI; strings only at serde and the Godot crossing | `EnemyKey` |
| Scene identity | Minted by the catalog, the one door to a scene path | `SceneId` |
| Salted entropy | One draw; the seed derives per level and planet; domains mix a registry salt | `GameManager::fresh_run_seed`, `Seed::for_level`, `seed::salt` |
| Faucet Principle | Preallocate at build, flip dormancy via the deferred boundary during play | `docs/architecture/faucet_principle.md`, `BoltPool`, `CloudPool` |
| Sole mediator | Signals into GameManager, mutations in private RunState | `GameManager`, `RunState` |
| FSM lifecycle | The phase enum owns transitions; behavior hangs off them | `GamePhase::can_transition_to` |
| Domain newtypes | Health, damage, shield as types; conversions in one place | `Health`, `Damage`, `Shield` |
| Typed identifiers | Signal, method, node-path constants and enums, not string literals | `rust/void-nodes/src/nodes/constants.rs` |
| Options single source | Seeded by the startup broadcast; no consumer default literal | `GameOptions` |
| Grammar not pins | Rosters and catalog TOML are the data; tests derive from grammar | `rosters/`, `catalog/` |
| Stage is the door | Every pipeline is a make target; inner steps never invoked directly; oracle to plan to apply | `Makefile`, `scripts/plan_apply.py` |
| Census at ingestion | Provider metadata read by a tool at the start of the stage; the audit joins it to authored policy | `scripts/audio_credits.py`, `catalog/attributions.toml` |
| Retained graph | Opaque, method-only, petgraph algorithms | `LevelGraph::visible_from` |
| Symbol-addressable code | Imports at top, policy numbers as defaults or attrs | every module; no single site |
