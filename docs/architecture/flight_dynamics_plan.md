# Kinetic Regime: Unified Flight Dynamics & Collision Coherence

> **Superseded in part:** the powered/ballistic *authority split* this
> plan implemented (engine owns movers, KineticWorld owns props) is
> being replaced by the unified motion authority designed in
> `kinetic_engine_design.md`. The kinetic core (Phase A), collision
> coherence (Phase C), and the world's inner loop carry forward
> unchanged; the cross-regime bridges do not.

## Why

Three bugs traced to the same root: every moving thing hand-rolls its own
physics with its own private assumptions.

- **Infinite spin** (fixed 2026-06-11): ShipController's inline damping
  integration had its dissipation invariant silently inverted by upgrade
  arithmetic. The bug class: *energy injection without guaranteed
  dissipation*.
- **Blobs "almost" through walls**: enemy scenes hand-tune collision
  shapes; the slime's 0.4-radius sphere is far smaller than its mesh.
  Nothing enforces collider ≈ mesh.
- **Firing while embedded**: `enemy_ai::update(distance, delta)` has no
  line-of-sight concept, and Projectile (Area3D, layer 0, player-only
  mask) never collides with level geometry.

Planned features multiply the movers: drifting props that bounce around
their rooms as mobile obstacles, player bounce-on-collision, physical
projectiles, prop↔player momentum exchange.

## The two boundaries (low-frequency layout)

### 1. Propulsion is the type boundary between movers

There are exactly two kinds of moving object, and the difference is
whether they can generate force:

- **Powered movers** (player, enemies): `PoweredBody` = kinetic state +
  a propulsion source (player input, enemy AI) producing thrust/torque
  each tick.
- **Ballistic movers** (drifting props, future physical projectiles):
  `BallisticBody` = kinetic state with **no thrust API at all**. Their
  momentum changes only through collisions. The compiler enforces the
  regime: you cannot write code that propels a prop, and impulse
  application is `pub(crate)` inside the world simulation, not a public
  per-body mutator.

Both step through one **kinetic core**: `Retention` (validated < 1.0;
`Retention::FULL` = exactly 1.0 reserved for ballistic drift),
`Restitution` (validated < 1.0), `integrate`, `bounce`. Invariant
property tests live here once, for every mover:
- input stops ⇒ velocity converges to zero (powered);
- zero input at `Retention::FULL` ⇒ velocity preserved exactly, never
  amplified (ballistic);
- bounce never gains energy;
- impulses are one-shot by construction (consumed by the step that
  receives them) — sustained-contact re-injection is unrepresentable.

### 2. Representation is not rendering

`KineticWorld` (void-logic) is the *representation*: it owns every
ballistic body's full state — position, orientation, velocities — plus
the static collision boxes the level assembly already produces, and
steps them all centrally: integrate → collide vs room statics → bounce →
body-body exchange. It runs for every room, all the time, whether or not
anything is rendered. Trajectories exist independent of the camera.

`PropField` (void-nodes) is the *rendering*: a MultiMesh per room whose
instance transforms are copied from `KineticWorld` each frame — but only
for rooms within draw distance. Culling the render never touches the
simulation.

Powered movers stay Godot-authoritative for their own wall collision
(CharacterBody3D + `move_and_slide` is the engine's strength), but their
dynamics math comes from the same kinetic core, and their kinetic state
is mirrored into `KineticWorld` each tick so props can bounce off them;
the world hands impulses back (push the prop, signal ram damage). Both
collision geometries — Godot's StaticBody3D boxes and the world's AABBs
— derive from the same `level_assembly::collision_boxes` output, so
there is one source of truth for "where the walls are."

## Performance posture

- Simulation is cheap and always-on: N props × (one integration + a
  handful of sphere-vs-AABB tests against *their own room's* boxes —
  per-room partitioning, props live in their rooms). Hundreds of bodies
  at 60 Hz is microseconds in Rust. Simulate everything; don't invent
  sleep states up front.
- Rendering is the actual cost, and it's already capped by draw
  distance/fog. MultiMesh keeps each visible room to one draw call;
  transform updates only for visible rooms.
- Escalation path *if measurement ever demands it* (in order): update
  far-room transforms at reduced rate → coarser far-room physics tick →
  closed-form ballistic segments between bounces. Measure first; the
  representation/render split is what makes all of these possible
  without touching gameplay code.

## Phases

**A — Kinetic core in void-logic (TDD).** `Retention`/`Restitution`
newtypes (folds in the spin-fix review follow-up: `BaseStats.damping`
becomes `Retention`, the bug class unrepresentable), `KineticState`,
`integrate`, `bounce`, one-shot impulses, the invariant property tests.

**B — Migrate ShipController onto the core.** Behavior-preserving;
sanitize clamps remain as shell-side last resorts; GUT suite unchanged.

**C — Collision coherence (independent of A/B).**
- Enemy projectiles get a level-geometry mask and despawn on wall hit
  (RED: GUT fires a bolt at a wall, asserts it frees early).
- `enemy_ai::update` gains `has_line_of_sight: bool` (shell raycasts;
  model stays pure). RED: in-range enemy with LOS=false must not fire.
- Collider-matches-mesh GUT audit: instantiate every enemy scene,
  compare collision shape extent to mesh AABB, fail under a coverage
  threshold. Fix offenders (slime first).

**D — KineticWorld + ballistic props.** World sim over per-room statics;
PropField MultiMesh view with distance culling. Existing loose props
(AnimatableBody3D, randomly rotated, static) migrate to ballistic bodies.

Initial conditions — *rest is the default* (decided 2026-06-11):
- Props spawn at zero velocity. With no propulsion and lossy bounces,
  rest is the model's own equilibrium — the 65My-abandoned look is the
  physics, not set dressing. A body at exactly zero stays at zero until
  an impulse arrives, so undisturbed rooms cost nothing to simulate and
  no heuristic sleep logic exists.
- **Rest capture**: post-bounce speed below a small epsilon snaps to
  zero (models micro-settling; disturbed rooms eventually return to
  equilibrium and re-arm the free sleeping).
- Motion is information: stillness = abandoned, drift = recent activity.
  Rooms with enemy spawns may seed 1–2 "recently disturbed" props with
  gentle drift/tumble from a `Seed` sub-stream (the occupants' doing);
  empty rooms and the spawn room are dead still.

**E — Momentum exchange.** Powered-mover state mirrored into the world;
impulses back out: props bounce off player/enemies, player bounces off
walls (contact onset only, sub-1.0 restitution), enemy-initiated ram
damage (fixing today's asymmetry where only the player's own slide
collisions deal damage), physical projectiles as ballistic bodies with a
damage payload.

## Order & discipline

A → B (no behavior change), C anytime, D → E. Each phase:
red-green-refactor, `make check` green, design-review agent on the diff
before merge.

## Implementation status (2026-06-12)

All five phases implemented red-green. Deliberate deviations, each on
the measure-first principle:

- **Flat statics + free sleeping instead of per-room partitioning.**
  Rest-by-default means only disturbed bodies test collision at all;
  a handful of moving props × all boxes is microseconds. Partition
  when profiling says so.
- **Per-node prop rendering instead of MultiMesh.** Loose props are
  heterogeneous scene instances and already exist as nodes; the view
  syncs their transforms from the world (moving bodies only). MultiMesh
  remains the escalation path.
- **No "recently disturbed" seeding yet.** Enemies mirrored as movers
  disturb props naturally during play, which covers the fiction;
  seeded ambience can come later via `Seed` sub-streams.
- **Prop↔prop and prop↔mover only; props do not push movers back.**
  Movers are infinite-mass in the world; the engine owns their side.
  Symmetric shove-back is future work if the feel wants it.
- Wall bounce uses contact onset (`was_colliding` latch) with
  restitution 0.35; enemy-initiated ram damage mirrors the player's
  ram check (same `MIN_RAM_SPEED` threshold, same 30/100 split).
