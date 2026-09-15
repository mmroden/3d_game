# Architectural ground truth

Repo-specific facts every reviewer verifies against current code before holding
the diff to them. Documents drift from code (this one included): confirm each
item with Serena at its canonical site before citing it, and report drift as a
`Gaps` item rather than judging the diff against a phantom.

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

Signals, methods, and node paths go through typed constants or enums, per the
Zen of Rust audit.

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
`Seed::for_level` and consumes it through domain-salted `SmallRng` streams
(the `HULL_SALT` pattern). Two domains seeding from the same raw seed value
consume the same bit-stream; that is a correlation, not a smell. Unseeded audio
and cosmetics are the accepted exception.

## Faucet Principle: allocation only at build time

Everything a level can contain is preallocated during level creation (manifest
in `void-logic`, pools in `void-nodes`); play only flips entities dormant or
active. Structural allocation on a during-play path (`instantiate()`,
`ResourceLoader::load`, new nodes, `queue_free` of gameplay entities) is a
violation. Dormancy flips ride the deferred boundary; direct collision or
monitoring toggles from physics callbacks or signal flushes are engine-blocked
and strand entities half-dormant. Transient cosmetic FX are the accepted
exception. Standard: `docs/architecture/faucet_principle.md`.

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

## Canonical sites for the house patterns

Verify each with Serena on every run; report drift under `Gaps`.

| Pattern | What it is | Canonical site to verify |
|---|---|---|
| Identity newtype | `EnemyKey`: the authored TOML key, interned, flowing unchanged model to UI; strings only at serde and the Godot crossing | `EnemyKey` definition and its single boundary parser |
| Scene identity | `SceneId` minted by the catalog, the one door to a scene path | `SceneId` and the catalog's scene accessor |
| Salted entropy | `GameManager::fresh_run_seed` is the only draw; `Seed::for_level` derives; domains consume via salted `SmallRng` (`HULL_SALT`) | `Seed`, `HULL_SALT` |
| Faucet Principle | preallocate at build, flip dormancy via the deferred boundary during play | `docs/architecture/faucet_principle.md`, the pool types in `void-nodes` |
| Sole mediator | signals into GameManager, mutations in private `RunState` | `GameManager`, `RunState` |
| FSM lifecycle | `GamePhase` + `can_transition_to()`; behavior hangs off transitions | `GamePhase` |
| Domain newtypes | `Health`, `Damage`, `Shield`; conversions in one place | those types |
| Typed identifiers | signal, method, node-path constants and enums, not string literals | the signal constant module |
| Options single source | `GameOptions` seeded by the startup broadcast; no consumer default literal | `GameOptions`, the broadcast |
| Grammar not pins | rosters and catalog TOML are the data; tests derive from grammar | `rosters/`, `catalog/`, their loaders |
| Stage is the door | every pipeline is a make target; inner steps never invoked directly; oracle to plan to apply for asset conversion | `Makefile`, `scripts/plan_apply.py` |
| Census at ingestion | provider metadata read by a census, audit joins it to authored policy | the census scripts, `catalog/attributions.toml` |
| Retained graph | `LevelGraph` opaque, petgraph algorithms | `LevelGraph` |
| Symbol-addressable code | imports at top, policy numbers as defaults or attrs | any module |

