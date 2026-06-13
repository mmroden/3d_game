# Kinetic Engine: Unified Motion Design

Status: DESIGN — supersedes the powered/ballistic split implemented from
`flight_dynamics_plan.md`. That implementation unified the *integrator*
but not the *world*: Godot's `move_and_slide` kept authority over
powered movers, and six hand-built bridges grew across the seam (Mover
mirroring, velocity readback, the `was_colliding` latch, two ram-damage
loops, an engine LOS raycast, Area3D projectiles). This document defines
the engine that removes the seam. Implementation does not begin until
this design is reviewed.

## Principle

**One motion authority.** Everything that moves — ship, enemies, props,
physical projectiles — is a body in `KineticWorld` (void-logic: pure,
deterministic, effect-free). Godot renders, collects input, plays audio,
and answers *queries against passive colliders*; it never moves a
gameplay body. There is exactly one place where physics state crosses
into the scene tree, and it is a view sync.

## Ontology (void-logic)

- `BodyCore` — position, `KineticState` (velocities), `Mass` (newtype,
  > 0), collider sphere radius, `Material`, `RoomId`. Orientation
  remains a view concern while colliders are spheres.
- `PoweredBody` — `BodyCore` + a `ControlInput` slot written each tick
  by the shell. Retention comes from its loadout (always `decaying`).
- `BallisticBody` — `BodyCore` only. No thrust API exists on it;
  retention is `FULL`. (Unchanged from today.)
- `Material { restitution: Restitution }` — slide is not a mode;
  it is restitution 0 with the tangential component preserved. Ship
  hull ≈ 0.35, props ≈ 0.6, walls neutral. Pair rule (revised in M1):
  `max(a, b)` — "the livelier surface governs" — so neutral walls let
  each body's own coefficient rule, exactly matching the bespoke
  solver's behavior the property tests pinned. (`min` with neutral
  walls would zero every wall bounce.) The Material *struct* lands in
  M3 with mover hull materials; until a second field exists it would
  be a one-field wrapper around `Restitution`.
- `Payload` — optional tag on a ballistic body (e.g. projectile damage)
  carried opaquely by the world and surfaced in contact events. The
  world never interprets payloads.
- `RoomId` newtype — deferred (decided during M1): its performance
  motivation (per-room partitioning) died with the bespoke solver,
  since rapier's broad-phase BVH *is* the spatial index; its remaining
  consumer is disturbance seeding, and it lands with that feature.

## Tick pipeline (fixed order, one place)

1. **Control** — the shell host writes `ControlInput` for each powered
   body (player input; enemy AI, fed `has_line_of_sight` from the
   world's own ray query — the engine raycast bridge is deleted).
   The control edge is where *intent* enters the world: player and AI
   are peer producers on it. Ballistic bodies have no control edge by
   type construction — a collision-tumbling bookshelf is mechanism
   responding to mechanism, entirely inside the physics node.
2. **Step** — `world.step(delta)`: substepped so the fastest body moves
   less than the thinnest static per substep (substep count derived
   from state — deterministic); integrate; resolve body–static and
   body–body contacts with mass-weighted exchange; rest-capture.
3. **Events** — `step` returns contact events `{ participants, impact
   speed, normal, position }`, emitted on contact *onset* only (the
   world tracks touching pairs; the shell latch is deleted).
4. **Consequences** — the host drains events and routes them through
   GameManager signals: ram damage (one rule replacing both per-node
   loops), projectile payload delivery, impact SFX.
5. **View sync** — the host writes body transforms to their nodes.
   Nothing else in the tree ever moves a gameplay node.

## Hardware target

M2-class Apple silicon; 120 Hz SBS glasses (xReal) are a first-class
display target alongside 60 Hz panels. The simulation's worst case
(every prop moving) is tens of microseconds per tick — physics is
never the budget; rendering is.

## Timebase: fixed tick, render-independent

Physics advances only in Godot's fixed physics tick — never with
render-frame deltas. The determinism invariant requires this;
variable-dt physics is the classic source of hardware-dependent
behavior. **Tick rate: 120 Hz** (`physics_ticks_per_second = 120`):
the cost is negligible and it halves input-sampling latency, which is
a comfort property on head-mounted displays.

Tick-rate invariance: `Retention` is defined **per second** and the
integrator derives the per-tick factor (`per_second.powf(dt)`), so
changing the tick rate never changes handling feel. (Today's factors
are per-tick — at 60 Hz, 0.95/tick ≈ 0.046/s retained; M1 converts
them, preserving current feel at the new rate.) `REST_SPEED` and speed
limits are already per-second quantities. Threading the simulation
is deferred for lack of payload — it solves expensive physics, ours is
sub-millisecond — not for risk: the world holds no Godot types, so it
is `Send` by construction, and `Gd<T>` being `!Send` means the
compiler enforces that scene-tree access can never leak into a physics
thread. The void-logic crate wall and the thread-safety wall are the
same wall. If body counts ever demand it, the escalation (threaded
step, or rayon over the body–statics pass) is compiler-checked, not
hoped-for. Rendering rate is free to differ
in both directions:

- **Accuracy** scales with substeps, not the global tick: substep count
  derives from the fastest body each tick, so thin-wall precision is
  paid only when something is actually fast. Raising the global tick
  rate is the blunt alternative and is not the plan.
- **Smoothness at high refresh (SBS/xReal first-class)**: the world
  retains previous + current state; view sync runs per rendered frame
  and draws each body at `lerp(prev, curr, fraction)` using the
  engine's physics-interpolation fraction. Interpolation stays even
  with tick = display rate: tick and vsync are not phase-locked, and
  drift frames otherwise judder. Stages 1–4 are tick-rate code; stage
  5 is frame-rate code. Interpolation is purely a view concern — it
  must never touch world state.

## Collider shapes: a closed ladder

`BodyCore` carries a closed `Collider` enum from M1, with
`Sphere(radius)` as the only initial variant — shape evolution is
exhaustive-match-driven, never a refactor.

1. **Sphere** (v1): rotation-invariant, so orientation stays cosmetic
   and no rotational dynamics exist.
2. **Capsule** (named next step, not v1): the payoff is real — the
   collider audit forced the shark into a 2.5 m sphere to cover its
   mesh honestly; a capsule covers elongated bodies *and* fits
   doorways. Cost: orientation enters the physics model (constrained,
   axis = facing — still no inertia tensors).
3. **Beyond capsules** (manifolds, stacking, inertia tensors) is
   writing a physics engine — see Engine below.

## Engine: rapier3d behind the facade, from M1

The bespoke world was requirements discovery; its property tests are
the engine-agnostic contract. M1 adopts `rapier3d` *inside void-logic*
(pure Rust — never a godot-physics plugin) behind the existing
`KineticWorld` facade, and the contract is: every current property
test stays green. Our roadmap items are rapier features we would
otherwise hand-build — capsules, CCD for fast projectiles, onset
contact events (`CollisionEvent::Started`), mass-weighted solving.

Invariants stay at our boundary: `Retention` (per-second) and
`Restitution` remain the facade's types, validated there and converted
to rapier parameters internally; raw library floats never appear in
caller code. Rest semantics map to rapier sleeping; if the threshold
feel differs from exact-rest capture, the facade reasserts it.

## Landing zone: snapshot publication

The only artifact that crosses from physics to rendering is an
immutable `WorldSnapshot` (tick index + transforms/velocities of
renderable bodies, ~KBs). Physics publishes one per tick; the view
keeps the previous snapshot it consumed and interpolates toward the
newest. View code receives snapshots and **cannot name
`KineticWorld`** — the boundary is compile-time, not convention.

- Single-threaded (today): publish/read on the main thread; no lock
  exists or is needed. The structure is already the concurrent one.
- Threaded (if ever): atomic swap / triple buffering (`arc-swap`-style
  latest-wins, never a draining queue — a hitched renderer should skip
  stale snapshots, not replay them). Writer and reader never block.
- The data-flow graph has exactly two one-directional edges:
  `ControlInput` → physics, snapshots → view. No shared mutable state;
  `Send` by construction.
- Cost accepted: interpolation renders ~one tick behind the sim
  (8.3 ms at 120 Hz); extrapolation is rejected (overshoots bounces).

### Latency budget (motion-to-photon, 120 Hz tick + display)

Device polling 4–8 ms (BT worse) → input-sampling quantization avg
~4 ms (defined by tick rate; exists in any fixed-tick design) →
physics ~0 → interpolation +8.3 ms (the one deliberate cost) →
render/vsync/panel ~8–12 ms. **Total ≈ 25–30 ms** — strong by genre
standards (typical shipped games: 60–100 ms). Escalation levers, in
order: raise tick rate (sim is microseconds; quantization and
interpolation costs both shrink), or render the player's own body
un-interpolated at the freshest tick while interpolating the world
(client-side-prediction trick; trims perceived rotation lag since the
camera is the ship). Neither is a redesign.

## Conservation invariants (tested, like the existing core)

All current kinetics/world property tests remain. New ones:

- **Third law**: in any body–body exchange, momentum is conserved
  (mass-weighted); a prop striking the ship moves the ship.
- **Onset-exactly-once**: a continuous contact emits one event.
- **Partition transparency**: obsolete as of M1 — spatial pruning is
  rapier's broad phase, not facade code; there is no second pathway to
  prove equivalent.
- **Determinism**: identical inputs ⇒ identical world state.

## Shell contract (void-nodes)

- **Host**: LevelManager owns and steps the world (it already builds it
  from generation); GameManager stays the sole *state* mediator,
  consuming the host's signals. Compile-time wall: the world is a
  private field of the host; no other node type can name it.
- **Registration**: ship and enemies register on entering the tree and
  receive a `BodyId`; their nodes keep `CollisionShape3D`s as **passive
  query colliders** (laser hitscan and Area3D pickups keep working
  unchanged — bodies moved by `set_position` still answer raycasts and
  overlaps). Engine colliders exist for queries only, never motion:
  `move_and_slide` has no remaining call sites when migration is done.
- **Control flow**: the host pulls a typed `ControlInput` from each
  registered mover per tick (Rust-typed `Gd<T>::bind`, no Variant
  stringly calls); movers never see the world.

## What this design makes trivial (acceptance criteria, not features)

Each must reduce to roughly one line or one rule, or the design failed:

1. Props push movers back → third-law exchange (delete `Mover`).
2. Physical projectiles → ballistic body + `Payload` + contact event.
3. Per-room partitioning → `RoomId` tagging at construction.
4. Disturbance seeding → `disturb` over `bodies_in_room` at setup.
5. MultiMesh → swap inside view sync; nothing else moves.

## Instrumentation: measure-first as a contract

No instrumentation exists yet; it is built alongside M1 (a system
should be born with a pulse, but we do not design dashboards for a
system that cannot tell time). The shape, decided now so M1 doesn't
improvise it:

- **Facade**: the `tracing` crate — Rust's vendor-neutral
  instrumentation layer (the OTel architecture pattern, minus OTel
  itself, which is request-tree-shaped and wrong for a 120 Hz loop).
  Backends attach without touching instrumented code.
- **Always-on path** (zero deps): ring buffer of per-tick stage
  durations → sliding-window percentiles → Godot
  `Performance.add_custom_monitor`, graphed in the editor for free.
- **Deep-dive path**: Tracy via `tracing-tracy` when flame-graph
  detail is needed. Attached on demand, never always-on.
- **SLO, symptom-side**: per-tick duration jitter, starting target
  p75 < 4 ms (half the tick budget; tune from data). Jitter is the
  metric because players adapt to constant delay but not to variance.
  Utilization percentages are explicitly not gates — they measure a
  cause-proxy, not harm.
- **Breach triggers diagnosis, not threading.** Threading is one
  remedy among several and rarely the first; a jitter spike is more
  often an O(n²) pass or allocation churn, which threading would
  merely make concurrent.

Determinism dividend, noted for later: a deterministic fixed-tick
world driven by recorded `ControlInput` streams is a replay system
(kilobytes per run) and is already lockstep-shaped if networked co-op
ever happens. Fairness in tick-based multiplayer is exactly this
property (cf. Quake 3's frame-rate-dependent jump physics, the exploit
class fixed ticks exist to kill; CS2's sub-tick timestamps refine it).

## Non-goals (v1)

Friction/tangential damping; non-sphere gameplay colliders; prop–prop
stacking rest contacts; replacing engine raycasts for *hitscan* (the
passive-collider query seam stays until the world's ray query has
proven itself with LOS).

## Migration map (mechanical once the design lands)

Derived strictly from the above; each step red-green, `make check`
green, design-reviewed; no step introduces a design decision:

- **M1** — world core: rapier3d behind the facade (existing property
  tests stay green as the acceptance gate); Mass, Material, contact
  events, ray query, RoomId tagging; `WorldSnapshot` publication +
  interpolating view sync; per-second Retention conversion.
  **Status: complete 2026-06-12**, including the always-on
  instrumentation (TimingWindow percentiles over the physics stage,
  surfaced as `kinetics/step_ms_p50|p99|jitter` custom monitors in the
  editor debugger; the Tracy deep-dive path attaches on first
  profiling need, per the contract). Notes from
  contact with rapier 0.32: math backend is glam (`Vec3`); the
  QueryPipeline is a borrowed view built from the broad phase, so ray
  queries reflect the world as of the latest `step`; contact slop
  tightened to 1e-4 (default parks bodies ~1 mm inside surfaces);
  never call `sleep()` by hand — assert exact-zero velocities and let
  the island manager sleep bodies itself.
- **M2** — enemies migrate (AI → ControlInput; ram loops deleted; LOS
  from world ray). **Complete 2026-06-12.**
- **M3** — ship migrates (readback + latch deleted; wall feel = hull
  material; `make run` feel check is the gate). **Complete 2026-06-12**
  — feel check pending. Notes: thrust applies as solver impulses and
  retention as engine damping (velocity overwrites erase contact
  corrections — learned by test); ship rotation stays facade-exact;
  bodies have friction (0.4/0.3) so glancing strikes tumble props and
  wall-scraping bleeds speed — a deliberate feel change to evaluate.
- **M4** — projectiles migrate (Area3D mover deleted); pickups remain
  Area3D. **Complete 2026-06-12** — enemy bolts are ballistic bodies
  with payload (they shove props); host owns spawn/expiry/detonation
  via contact events.

Post-migration verification (2026-06-12): zero `move_and_slide` call
sites remain (mover trait impls are init/ready/cosmetics only);
LevelManager is the only module that names `KineticWorld`; the Mover
mirror, both ram loops, the engine LOS raycast, the velocity readback,
the bounce latch, and the Area3D projectile are all deleted.
- The five acceptance criteria are verified as they become expressible,
  not as standalone work items.
