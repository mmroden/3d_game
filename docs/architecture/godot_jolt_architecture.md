# Engine Ownership: Godot + Jolt

Status: ACTIVE — this is the architecture. It supersedes
`kinetic_engine_design.md` (the rapier/`KineticWorld`-owned sim) and
`bevy_migration.md` (the bevy/ECS exploration). Both described owning a
physics engine; we don't. We rent one.

## Principle

**Godot and Jolt own motion, collision, rendering, and threading. We own
the description of the world, the stereo camera rig, and gameplay
intent.** We construct objects and scenes, attach colliders, and hand
them to the engine. After the hand-off we never move or collide anything
ourselves. The only physics code we write computes *intent* (what force
to apply); never *integration* (what the force does).

This reverses the prior direction. We owned a physics engine (rapier
behind `KineticWorld`); the regressions — clipping through corners and
furniture, inverted death authority, three bespoke lifecycle paths —
lived entirely in the plumbing we wrote to own it. The engine collides
against everything in the scene automatically; the bug class disappears
when the engine owns geometry.

## Ownership

| Concern | Owner |
|---|---|
| Level generation, room graph, connectivity, seeding | **`void-logic`** (pure, tested) |
| Object description: mesh + collider spec + surface, per object | **`void-logic`** |
| Furnishing, run state, currency, upgrades, lore | **`void-logic`** |
| AI *decisions* and flight *intent* (force/torque to request) | **`void-logic`** (pure fns) |
| Scene construction: bodies + colliders, hand-off | **`void-nodes`** |
| Applying intent as forces; reading contacts/raycasts | **`void-nodes`** |
| Stereo camera rig (IPD, convergence, UI-in-both-eyes) | **`void-nodes`** + `stereo.rs` |
| Rigid-body integration, collision detect/response, rest, inertia | **Jolt** |
| Raycast / scene queries | **Godot** |
| Rendering, materials, lights, culling, window/display | **Godot** |
| Threading (physics solve, render thread) | **Godot/Jolt** |

`void-logic` keeps no `godot` dependency and no physics engine. It
*describes*; it does not simulate.

## Threading

Multithreaded at the engine level, single-threaded for our code:
- Jolt parallelizes collision + solver across worker threads.
- Godot renders on a dedicated thread; physics can run off the main
  thread via project settings.
- Our gameplay / scene-tree code runs on the **main thread**
  (`process` / `physics_process`); the scene tree is not thread-safe.

We write no threading. The expensive work parallelizes for free.

## The rule that prevents the regression returning

Collision geometry is derived from the **same source as the mesh** — one
pathway, not two. At scene build, every placed object gets a collider:
structural geometry as `StaticBody3D` with a collider generated from its
mesh (trimesh/convex, so it hugs the real shape — "collisions on the
edges of the textures"); props/movers as `RigidBody3D` with a collider.
An object may be collider-less only if its description explicitly says
`passable`. Completeness is a property of the description and is unit
tested without the engine; Jolt does the colliding.

## Stereo / camera rig (ours, by necessity)

Godot's native stereo is XR/OpenXR multiview, where IPD and eye poses are
**device-driven** — you cannot own inter-camera distance or convergence.
Because xReal presents as a flat side-by-side display and we want to own
those parameters, we keep our two-camera / two-viewport SBS rig.
`stereo.rs` already implements eye separation and **off-axis**
(asymmetric-frustum) convergence — the technically correct alternative to
toe-in, free of edge vertical-parallax.

UI in both eyes (the long-standing textbox problem): render the UI once
to a `SubViewport`, then display that viewport's texture in **both** eye
containers (a `TextureRect` per eye), or use the world-space UI quad
(`ui_plane_size` / `ui_plane_position`). The SubViewport-texture route is
deterministic and is the chosen approach.

## Test boundary

**We test (pure `void-logic`, no engine):**
- Generation & connectivity: rooms reachable, connectors pair, identical
  seed ⇒ identical level.
- Object/surface/collider assignment: what objects a room has, their
  surfaces, their collider specs.
- **Completeness invariant:** every placed renderable has a collider spec
  or explicit `passable`.
- Containment: objects within their cell/room bounds; openings and flight
  paths clear (this class already exists).
- Currency, upgrades, run state, save/load, AI decisions, stereo math,
  control-intent math.

**One thin engine-side (GUT) check at the seam:** a scene built from an
assembly has a `CollisionShape3D` on every body.

**Jolt/Godot own (we trust their tests, write none):** collision
detect/response, rigid-body integration/inertia/rest, raycast queries,
rendering / SBS compositing / culling, window/display.

We stop testing physics because we stop owning it.

## Migration checklist

**Engine config**
- [ ] 3D physics engine = Jolt (default in 4.6; set explicitly).
- [ ] `default_gravity = 0`; set physics ticks/sec.

**Delete the owned sim (`void-logic`)**
- [ ] Remove `kinetic_world.rs` and the `rapier3d` dependency.
- [ ] Delete the physics property tests.
- [ ] Assembly emits a collider spec per object.

**Delete the bridge (`void-nodes`)**
- [ ] Remove `body_registry.rs` (id↔node map, snapshot, interpolation,
  bolt lifecycle).
- [ ] Strip the physics tick from `level_manager::physics_process`.

**Build objects as Godot bodies (`void-nodes`)**
- [ ] Each placement → mesh + body + collider (Static for structure from
  mesh; Rigid for props/movers).

**Movers → `RigidBody3D`**
- [ ] `ship_controller`: `RigidBody3D`; apply thrust/torque + flight
  assist/stabilize from `void-logic` intent.
- [ ] `enemy_drone`: `RigidBody3D`; AI intent → forces; LOS via
  `RayCast3D`; contacts via signals.
- [ ] Bolts: `RigidBody3D`/`Area3D`, timer lifetime, damage on contact.
- [ ] Ram/projectile damage: Godot contact signals → GameManager.

**Camera / SBS rig (keep)**
- [ ] Per-eye cameras follow the player `RigidBody3D`.
- [ ] UI renders into both eyes via SubViewport texture.

**Verify**
- [ ] Fly into walls/corners/furniture: collisions hold; props tumble;
  bolts hit; both eyes render incl. UI; no clip-through.
- [ ] Pure tests + one GUT collider-per-body test green.
