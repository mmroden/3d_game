# Physics Sanitization Architecture

## Problem

NaN values propagate through the physics engine and crash the game. The player spins wildly after killing enemies at close range.

## Root Causes (confirmed by GUT tests)

1. **godot-rust `normalized()` panics on zero-length vectors** — unlike GDScript which returns zero, the Rust binding panics at `vector3.rs:238`
2. **`look_at()` with colocated target** — Godot prints error, godot-rust may panic
3. **`update_health_bar()` unguarded** — called every frame with `normalized()` on player direction, no distance check
4. **Dead enemies continue physics** — `is_dead()` check was AFTER movement code, not before
5. **`Quaternion::from_euler(NaN)` produces NaN** — cascades forever through multiplication
6. **No NaN guards on player** — corrupted basis/quaternion from collision cascades accumulates

## Solution: Three layers

### Layer 1: Prevent NaN creation (enemy_drone.rs)
- Dead check at TOP of `physics_process` — immediate return, zero velocity
- Distance guard (MIN_DISTANCE = 0.1) before all `normalized()` and `look_at()` calls
- `update_health_bar()` guarded against zero-vector and parallel cross products

### Layer 2: Sanitize player state every frame (ship_controller.rs)
- `sanitize_velocity()` — clamp magnitude, reset NaN to zero
- `is_quat_finite()` — check before `set_quaternion()`, keep previous if NaN
- Velocity caps: linear 50 m/s, angular 5 rad/s
- Basis validation before reading transform

### Layer 3: Collision layer separation (future)
| Layer | What | Collides with |
|-------|------|--------------|
| 1 | Player | Geometry, Props |
| 2 | Enemies | Geometry only |
| 3 | Geometry | Everything |
| 4 | Props | Geometry, Player |

## Verification

- `make test-godot` — 11 GUT tests (9 exploratory + 2 production)
- `make check` — 378 Rust tests
- `make run` — fly into slimes, kill at close range, no spinning

## Key decisions

- NaN guards are in the Godot-layer Rust code (void-nodes), not in void-logic, because the NaN originates from Godot engine calls (`normalized()`, `look_at()`, `move_and_slide()`)
- Exploratory GUT tests document actual Godot 4.6 behavior (e.g., zero.normalized() returns zero in GDScript but panics in godot-rust)
- Production GUT tests load real scenes and reproduce the crash
