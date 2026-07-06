# The Faucet Principle: preallocated entity lifecycle

Status: **standing principle** — tiers 1 and 2 implemented (2026-07). New
gameplay code is written against this, and pre-merge review enforces it: no
structural allocation during a level run, only during level creation.
Owner: entity lifecycle across `void-logic` (manifest) and `void-nodes` (pools).

Implementation note learned the hard way: dormancy flag flips (visibility,
process mode, monitoring/collision, freeze) must be applied on the deferred
boundary (`call_deferred`) — Godot blocks direct toggles inside physics
callbacks and signal flushes, stranding entities half-dormant. Keep logical
liveness in a Rust-side field so nothing reads the lagging engine flags.

## Principle

Before the spigots open — while the loading screen is up — every entity the
level can ever need is allocated, placed in the scene tree, and signal-wired.
Once play begins, **nothing is instantiated, loaded, or freed**: entities only
transition between *dormant* and *active*. Allocation is a loading-phase
activity; play is activation-only.

"Nothing allocated" means no *structural* allocation: no `instantiate()`, no
`ResourceLoader::load`, no `add_child` of new nodes, no `queue_free`. Godot
itself allocates Variants and server objects on its own schedule; that
impurity is accepted — the frame cost and hitches live in structural work.

## Two tiers

**Tier 1 — knowable counts, one life each, no reuse.**
Enemies, death-spawned minions, currency caches, dynamic props. The build manifest
sizes these exactly; each object activates at most once per level and is never
recycled, so no reset logic exists. All rigid bodies live here.

- Death minions are fully derivable at build time: the enemy roster is drawn
  from a seeded RNG and `EnemyType::death_spawn()` is pure. Every EyeDrone
  contributes its minions to the manifest as dormant entries, pre-parented
  under the same room container as their parent-to-be.
- Blue currency caches are bounded by enemy count: one dormant cache per
  enemy, activated on drop carrying the type's component reward. Green
  (organics) caches are placed at the manifest's loot spawns during the build,
  through the same `drop_at` activation path — one pickup mechanism, two tints.
- The player's subdrone squad is fixed at `SQUAD_SIZE`: pre-built dormant per
  level as LevelManager siblings (escorts must survive room culling), a
  launch is a dormancy flip, a deployment expiry flips back. One-life-per-
  level like the caches: freed and rebuilt on regeneration.

**Tier 2 — unbounded counts, ring buffer.**
Ammunition only. ONE ring serves both factions: a slot's `arm` stamps whose
bolt this life is (enemy or player), any homing lock (instance-id validated
per tick, the same guard discipline as the generation counter), and any
payload (a cluster shell's burst re-enters the pool as player fragments).
The ring bounds *concurrent* bolts, not total; slot reuse is
the recycle, and for an `Area3D` bolt the "reset" is the arm routine itself
(transform, velocity, damage, age, faction, lock, payload, monitoring on).
Size the ring generously —
dormant slots cost only memory; live cost scales with active bolts alone.
If every slot is live, overwrite the oldest bolt: a bolt vanishing in its
final tenths of a second is imperceptible, and it makes the buffer total (no
allocation fallback, no error path).

## Ownership across the crate wall

- `void-logic` owns the **manifest**: a pure, seed-deterministic enumeration
  of everything the level can contain — direct spawns, death-spawn expansions,
  cache bound, bolt-ring capacity. This is model: *what can exist*.
- `void-nodes` owns **pools and dormancy**: instantiating the manifest during
  the staged load, parenting under room containers, wiring signals once, and
  the dormant⇄active transitions. This is shell: *mechanism*.

## Dormancy contract

Dormant means all three, together, always:
1. invisible (`set_visible(false)`),
2. non-processing (`process_mode = DISABLED`),
3. non-colliding (collision layers/mask zeroed, or `monitoring` off for Areas).

Half-dormant states (visible but not colliding, processing while hidden) are
the failure mode; tests should assert all three flip together. Tier-1
activation is one-way per level (dormant → active), which is far easier to
verify than a recycle loop.

## What this dissolves

- `GameManager::connect_spawned_entities` — the per-frame tree scan — is
  deleted, not optimized. Everything that will ever emit `enemy_killed`,
  `cache_collected`, or `portal_entered` exists at build time; the mediator
  wires signals once, during the load.
- The death-minion ghost bug class: minions exist under their room container
  from the start, so mediator wiring and room culling cover them structurally.
- The bestiary gap: the manifest knows every type that can appear this level
  (including death-spawn types), so `mark_level_enemies_seen` reads the
  manifest instead of re-deriving from `spawns_directly()`.
- Mid-combat hitches: per-instance convex-hull builds, scene loads, and
  `queue_free` churn all move into the loading phase — which is exactly what
  the staged/chunked loading-screen design wants to have work to show.
- `Gd` handle lifetime bugs shrink: pooled handles stay valid for the level's
  lifetime.

## Sharp edges (known, small)

- **Stale deferred signals** (tier 2): a slot re-fired in the same frame its
  previous occupant's `body_entered` is still queued can register the old hit.
  Guard with a per-slot generation counter checked in the callback.
- **Interpolation streaks**: re-placing a pooled node across the map needs
  `reset_physics_interpolation()`, same as spawn-time placement today.
- **Rigid-body dormancy** (tier 1): a pre-instantiated enemy must sit frozen —
  no simulation, no collision — until activation. One-way transition; test it.

## Staging (effort/value order)

1. **Bolt ring buffer.** Self-contained, kills the highest-frequency
   structural allocation (fire path) and `queue_free` churn. Red-first:
   bolt reuse preserves behavior (damage, lifetime, hit detection), stale-hit
   generation guard, dormant slots cost no physics.
2. **Manifest + tier-1 pools** (minions, currency caches). `void-logic` manifest
   derivation with pure unit tests; shell pools built during the staged load.
   Deletes `connect_spawned_entities` and `spawn_death_minion`'s
   load-instantiate path; fixes bestiary marking via the manifest.
3. **Full enemy pooling** (enemies themselves pre-instantiated dormant,
   hulls cached per type). Only if profiling still shows load-time or
   spawn-time cost after 1–2.

Each stage lands green on the full suite before the next begins.
