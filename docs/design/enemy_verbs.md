# Enemy verbs (design capture — 2026-07-11)

Status: **building now** — verbs first, grammar application (planets 2 & 3)
after. Source: enemy-variety session with Mark, 2026-07-11.

## Principle

Every current archetype attacks the same player resource: HP. The latcher is
memorable because it attacks **thrust** instead. New verbs each attack a
different resource — movement, vision, aim, target priority — and enter the
grammar as open per-enemy switches (the `drain_dps` pattern: optional TOML
key, archetype-derived default, resolved onto `Behavior`). No new archetypes;
verbs compose onto any def.

Shared feel constants (tracer stretch, falloff shape) stay engine tuning.
Per-enemy balance (counts, radii, rates) is grammar.

## Wave 1 — weapon verbs (fire patterns)

### Burst fire — `burst_count`, `burst_seconds`

- `burst_count` (default 1 = today's single shot): bolts per trigger pull.
- `burst_seconds` (default 0.1): in-burst gap between bolts.
- Semantics: the FSM's existing `cooldown` gates *bursts*, not bolts. When a
  fire opportunity lands, the drone emits `burst_count` bolts spaced
  `burst_seconds` apart, then the cooldown interval runs. Machine-gun feel =
  high count, short gap, moderate cooldown — brrrt, pause, brrrt. DPS stays
  bounded by cooldown; the cooldown curve keeps scaling the whole pattern.
- Lives in `DroneAi` (pure logic, unit-tested); burst state survives across
  ticks, resets on disengage.

### Shotgun spread — `pellet_count`, `spread_deg`

- `pellet_count` (default 1), `spread_deg` (default 0): each fired bolt
  becomes a fan of pellets in a cone of `spread_deg` total angle.
- Damage is **per pellet**, as declared on the def (the damage stat means
  "per bolt" today and keeps meaning that; a shotgun def declares a small
  damage number). The damage curve scales pellets uniformly.
- Engine: `fire_bolt` loops fan directions (reuse
  `armament::cluster::fragment_directions`); pool capacity (2048) is
  generous, no pool change.
- Composes with burst (a burst of shotgun blasts is legal, if rude).

### Homing bolts — `bolt_turn_deg`

- `bolt_turn_deg` (default 0 = ballistic): max steering rate in deg/s.
- Engine: the homing machinery exists (`armament::homing::steer`,
  `enemy_bolt.rs steer_toward_target`, `BoltPool::fire_homing`) — currently
  player-only. Enemy path passes the player as target with the enemy faction.
- Tracking-missile feel: slow bolt (`bolt_speed` ~6), tight turn rate, long
  cooldown. No model needed — bolts are engine-drawn.

### Tracer visual — derived, NO switch

- Bolts stretch into oblong tracers along their velocity; stretch scales with
  speed above a threshold. Pure visual consequence of `bolt_speed` — one
  truth, no second knob to desync. Thresholds/ratios are engine feel
  constants in `enemy_bolt.rs`.

### Muzzle markers — `muzzles` (def identity, not a switch)

- Optional per-def barrel tips: `muzzles = [[x, y, z], ...]` in the **aim
  frame** — x right, y up, z toward the player, yaw-only (the frame
  `face_player` holds the model in), metres at the def's `size`.
- Shots round-robin the list, so bursts *walk the barrels* (the gatling
  read); the fan fires whole from the current barrel; the muzzle flash
  rides along. Aim stays gameplay-true: at the player, FROM the barrel.
- Omitted = the legacy centre + clearance convention. The linker rejects a
  tip reaching past `size` (unit/frame mistakes die loudly).
- Authored per def next to `yaw_offset_deg` — same category of imported-
  model presentation fact. Eyeball offsets from the bestiary view.

## Wave 2 — movement verb

### Tractor / repulsor — `pull_accel`

- Signed m/s² at the player, active while the enemy is engaged and the player
  is within `attack_range`. **Positive pulls the player toward the enemy**;
  negative shoves away (gust).
- Linear falloff: full `pull_accel` at distance 0 → zero at `attack_range`
  (no cliff at the boundary). Falloff math in void-logic, unit-tested.
- Engine: enemy tick mirrors `tick_swarm_slow` → new `#[func]` on
  `ShipController` accumulates a per-physics-frame force vector → summed in
  `fly` as `apply_central_force` (Jolt idiom: forces only, never velocity
  writes). Accumulator clears every frame; a dead/disengaged enemy stops
  pulling by simply not calling.
- The player fights it with thrust: pull magnitudes should sit near but below
  max thrust acceleration so escape is possible but costs the whole envelope.
- First customer: planet 2 final boss (fan sphere) — inhale (positive) drags
  you toward the gun; exhale (negative) scatters you. Boss alternation rides
  the existing FSM attack cadence, not a new phase system.

## Wave 3 — priority verbs (enemy↔enemy)

### Alarm aura — `alert_radius`

- Default 0 = inert. While an alarm enemy lives and is engaged, every enemy
  within `alert_radius` of it force-engages the player (detection bypassed).
- Attacks target priority: kill the screamer first or fight the room.
- Cashes the eye_drone blurb's "herds machines onto you" promise.

### Guardian link — `guard_radius`

- Default 0 = inert. Damage dealt to an enemy within `guard_radius` of a
  living guardian is redirected to the guardian's shield pool (`shield_frac`
  supplies the pool — guardian defs are naturally tanks) until it breaks.
- One door: the redirect lives in the single TAKE_DAMAGE path; a visible
  beam/tint marks guarded targets.

## Wave 4 — area verb

### Detonation cloud — `cloud_seconds`

- Default 0 = today's instant blast. A bomber detonation leaves an occluding
  cloud (fog sphere, radius = `blast_radius`) for `cloud_seconds`; while
  inside, the player's view is murked. Damage stays on the detonation tick —
  the cloud attacks **vision**, not hull (a DoT variant is a later switch if
  wanted).
- Faucet: clouds come from a pre-built dormant pool (BoltPool pattern), sized
  at level build from the bomber census.
- Biggest verb (new pooled node + visual); built last.

## Application targets (after verbs land)

- **Planet 2 boss layer** (replaces the evil_mech mix-ins): alien-troop
  grapple boss @3 (swarmer, drain), eared fan sphere @6 (`apartment_boss`
  model, `pull_accel` signature, hull container), white-sphere miniboss
  (scaled carrier + timed emitter) replacing `miniboss_brute` at levels 2/5.
- **Planet 3 weapon flavor** (grey spheres, human paradigm): stalker gets
  machine-gun bursts + tracers; a shotgun def for doorway ambushes; homing
  bolts on the warden or a new def. Owned by the planet-3 session.

## Open questions

- Guardian redirect vs. flat immunity while guarded — redirect chosen for
  now (damage is never wasted, the player learns the link exists).
- Cloud DoT (`cloud_dps`) — deferred until the vision-denial version has
  been felt in a playtest.
- Boss pod hit-zones (destructible rotors as phase gates) — separate chunk,
  not in this pass; the fan boss ships on `pull_accel` alternation alone.
