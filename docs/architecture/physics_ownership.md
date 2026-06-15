# Physics Ownership: Engine vs. Us

Status: ACTIVE. Companion to `godot_jolt_architecture.md`.

This boundary is **not a preference**. It is how Godot/Jolt are designed
to be used — the "zen of the tool." Getting it wrong is what reintroduced
the infinite-spin bug: I picked the simulation body type and then drove
it like a custom-control body, mixing two idioms the engine keeps
separate on purpose.

## The engine's design (quoted, not invented)

Godot's physics defines body types with distinct *intended* uses:

- **`StaticBody3D` / `AnimatableBody3D`** — frozen, or moved only by code/
  animation. Never simulated. → walls and structure.
- **`RigidBody3D`** — *"You do not control a RigidBody directly. Instead,
  you apply forces to it and the physics engine calculates the resulting
  movement."* The docs warn outright: *"Altering the position,
  linear_velocity, or other physics properties of a rigid body can result
  in unexpected behavior."* The intended hook is **`integrate_forces(state)`**;
  the docs' own worked example is a thrust+torque ship that is *"not
  setting the linear_velocity or angular_velocity properties directly, but
  rather applying forces (thrust and torque)… and letting the physics
  engine calculate the resulting movement."* For objects interacted with
  in the world — a stack of crates you push, a tree you knock over.
  → **simulation.**
- **`CharacterBody3D`** — `move_and_collide()` / `move_and_slide()`; you
  read collision data and respond *"according to your design… you're not
  simulating reality, you're designing an experience."* → **custom
  control.**

The engine's own one-liner: **"RigidBody is for simulation; CharacterBody
is for custom physics."** These are two idioms. **Mixing them is the
anti-pattern** — and it is the source of both the infinite-spin
regression (hand-driving a RigidBody) and, historically, the six bridges
(faking dynamics on a kinematic body).

## Which idiom each mover uses, and why

A space ship's motion *is* a simulation — thrust as force, momentum,
drift, collisions that shove and tumble it. That is `RigidBody3D`'s
documented domain. So:

| Object | Body type | Idiom |
|---|---|---|
| Ship, enemies, loose props | `RigidBody3D` | **simulation** (apply forces) |
| Walls, corners, floors, anchored structure | `StaticBody3D` | frozen |
| Bolts | `Area3D` | kinematic trigger (we own its straight-line travel — not a simulated body) |

Choosing simulation for the ship/enemies **binds us to the rules below**;
they are not ours to bend. (The other valid choice — `CharacterBody3D`,
where *we* own all motion and hand-code every shove and bounce — is the
"designing an experience" path. It is what produced the six bridges last
time and is the wrong fit for momentum-based flight. We are not using it.)

## The zen of the simulation idiom (what the engine owns)

Because a `RigidBody3D` *"cannot be controlled directly,"* these are the
engine's, not ours:

| Concern | Owner | Our only participation |
|---|---|---|
| Position & velocity integration | **Engine** | nothing — never assign velocity to move |
| Collision detection & response (bounce, slide, push-back) | **Engine (Jolt)** | give every body a collider |
| Velocity decay / damping | **Engine** (`linear_damp`, `angular_damp`) | set the *coefficient* from `Retention` |
| Mass & inertia tensor | **Engine** (from collider + mass) | set `mass`; *read* inertia, never assume it |
| Rest / sleeping | **Engine** | `can_sleep` |
| No-tunneling (CCD) | **Engine** | `set_use_continuous_collision_detection(true)` |
| Gravity | **Engine** | set 0 (zero-g) |
| Render smoothing between ticks | **Engine** (`physics_interpolation`) | leave on; never hand-interpolate |
| **No-infinite-spin invariant** | **Engine**, guaranteed by `angular_damp > 0` | our sole duty: set `angular_damp > 0` |
| Thrust / steering **intent** | **Us** | `apply_central_force`, `apply_torque` each tick |
| Tuning parameters (thrust, rotation rate, `Retention`, mass, caps) | **Us** | data in `void-logic` |
| "Coast to a stop on release" feel | **Emergent** from `damp > 0` | not hand-coded |

## The rules (the engine's, restated so they can't be broken quietly)

0. **Drive simulated bodies from `integrate_forces(state)`** — the
   engine's documented hook for applying forces and safely reading/writing
   physics state — not from `physics_process` with property setters.
1. **Never assign `linear_velocity`/`angular_velocity`/`position` to drive
   a simulated body.** The docs warn it "can result in unexpected
   behavior"; it bypasses integration *and* decay and deletes the
   invariant. Drive with `apply_central_force` / `apply_torque`.
2. **Always set `linear_damp > 0` and `angular_damp > 0`** (default `-1`
   = inherit, not a guarantee). Damp *is* the decay; decay *is* the
   no-infinite-spin invariant.
3. **The one allowed direct write is a discrete event, not steering:**
   *stabilize* may zero `state.angular_velocity` once on press, inside
   `integrate_forces`.

The regression: `angular_damp = 0` + `set_angular_velocity` every tick in
`physics_process`. Rules 0, 1, and 2 broken together → infinite spin.

## The grounded mapping (so we tune, not guess)

Godot damps per tick as `v *= 1 - damp / physics_ticks_per_second`,
i.e. `≈ e^(-damp)` per second. Therefore:

- **`Retention` ↔ damping are the same mechanism:** `damp = -ln(retention)`.
  retention 0.046 → `damp ≈ 3.08`.
- **Terminal linear speed:** `v* ≈ thrust_force / (mass · linear_damp)` —
  pick the cruise speed, derive the force.
- **Terminal angular speed:** `ω* ≈ torque / (inertia · angular_damp)` —
  inertia is the engine's and is *small* for a little sphere, so torque
  is derived from the actual inertia, never assumed.

## Adjacent engine rules (from the linked physics docs)

These come from the pages the introduction links to, and each one touches
code I wrote — so they are part of the boundary, not trivia.

- **Ray queries (LOS, hitscan):** the space is only safe to query
  *during `physics_process()`* (it is locked otherwise). Exclude the
  caster's own RID, and filter with `collision_mask` rather than long
  exclude lists. → so raycasts stay in `physics_process`, while
  velocity writes go through `integrate_forces` — the two cannot be
  merged into one callback.
- **Collision shapes:** a trimesh/`ConcavePolygonShape3D` *"can only be
  used within StaticBodies."* Dynamic bodies must use **primitives**
  (box/sphere/capsule) — the docs *"favor primitive shapes for dynamic
  objects… most reliable."* Convex is the middle option. → our structure
  uses trimesh-on-StaticBody (correct), our props/movers use sphere
  primitives (correct).
- **Physics interpolation:** the engine smooths every body between ticks
  automatically (we deleted our manual sync — correct). But it
  interpolates from the *previous* transform, so **after any spawn or
  teleport you must call `reset_physics_interpolation()`** or the body
  streaks from its old position. And transforms must only be changed in
  `physics_process`, never `process` (the health-bar billboard violates
  this and must move).
- **No scaling physics bodies:** *"Godot doesn't support scaling physics
  bodies / collision shapes — change extents, not scale."* Imported
  `.gltf` scenes that carry node scale can misalign generated trimesh
  colliders; verify at runtime.
- **Tunneling / stability:** CCD on fast bodies, 120 Hz ticks, thick
  static floor shapes, and Jolt over GodotPhysics — all of which we have.

## What we test vs. trust

- **We test (pure, no engine):** intent computation (input/state → the
  force/torque we request) and the parameter mappings (`retention → damp`,
  terminal-speed equations).
- **We trust (engine, untested by us):** integration, collision response,
  decay, inertia, sleeping, interpolation.

Sources: Godot `RigidBody3D` / physics-introduction docs and the
RigidBody-vs-CharacterBody design guidance (the body-type intents and the
"apply forces, don't control directly" rule are quoted above).
