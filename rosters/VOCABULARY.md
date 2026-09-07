# Roster grammar vocabulary

GENERATED — do not edit; `make build` re-renders this file
(`make roster-vocab` runs just this step).
Every list below is a CLOSED vocabulary: any other value is a
parse error naming the legal options. Open references (enemy,
swarm, kit, curve, and model keys) are declared by the data —
models by the probed catalog — and checked by the linker.

## AI behaviours

`ai` accepts:

- `"shooter"` — chase to attack range, hold and fire while sighted; sight-blocked, circle for an angle
- `"kiter"` — hold a stand-off ring: retreat when crowded, strafe and fire; blocked orbits flip, then back out
- `"swarmer"` — no projectile: close and press — ram damage plus the compounding latch slow (drain on bosses)
- `"tank"` — shooter with a damage-absorbing shield (half its HP); slow, durable
- `"bomber"` — charge to detonation range, burn the fuse, AoE-detonate and die

## Minion triggers

`escorts.trigger` accepts:

- `"on_death"` — bound minions rise from the corpse
- `"on_engage"` — bound minions rise the moment the fight starts
- `"{ every_seconds = N }"` — a timed emitter: a batch of `count` rises every N seconds while the parent lives. `cap` is REQUIRED and is the size of the pre-built ring the batches draw from — at most `cap` are afield at once, dead ring members recycle back in, so the pressure never runs dry (the linker requires cap >= count). One-shot triggers take no cap: their `count` IS the whole brood

## Curve kinds

`curves.<name>.kind` accepts:

- `"flat"` — constant 1.0 — the stat does not scale (no parameters)
- `"ramp"` — multiplies the baseline stat: linear from `at_level_1` to `at_peak` (at `peak_level`), flat past peak. For interval stats (cooldown), below 1.0 means FASTER

## Curve anchors

`curves.<name>.anchor` accepts:

- `"absolute"` — curves ride the game level (the default)
- `"entry"` — curves restart from 1 at the enemy's fleet entry level

## Boss reward policies

`boss_slot.reward` accepts:

- `"consolation_pile"` — blue/green pile — mid-planet bosses
- `"hull_container"` — the red container (random unowned hull) — planet finals

## Kit paradigms

`kits.<name>.paradigm` accepts:

- `"layered"` — tile + story stacks (the megakit)
- `"panel"` — one flat panel per cell face, cubic cells
- `"fixed"` — a pre-modeled environment scene installed whole — the kit declares `environment` (its authored zone map) and `scale` (world units per model meter) instead of deriving a grid

## Entry shapes

### `[[enemy]]` (enemies.toml)

Identity (global): `key`, `id` (append-only numeric crossing), `name`,
`model` (a key in models.generated.toml — the catalog `make assets` probes
from the installed files), `size` (metres, longest edge), `yaw_offset_deg`,
`muzzles` (optional barrel tips, aim-frame metres [x right, y up, z toward
the player]; shots round-robin the list, omitted/empty = fire from the
hull centre), `ai`, `reward`,
`spawns_directly` (default true; false = death-spawned/escort/staged only).

Minions — pre-staged bound drones and when they rise:

```toml
minions = [{ enemy = "<enemy key>", count = N, trigger = "on_death" }]
```

Tuning (level-bound): `[enemy.stats]` baseline (`hp`, `speed`, `damage`,
`detection`, `attack_range`, `cooldown`) × `[enemy.scaling]` per-stat curve
refs (falls back to `[scaling_defaults]`).

### `[[swarm]]` (enemies.toml)

`key` + `members = [{ enemy = "<enemy key>", count = N }]` — placed as ONE
unit: same room, adjacent cells. Schedulable anywhere an enemy is.

### Planet files (planets/planet_N.toml)

`planet`, `levels` (declared length), `kits = ["<kit key>"]` (pitch and
paradigm DERIVE from the kit), `rooms = { base, per_level }` (GENERATED
planets only — a fixed planet's room count is its dealt environment's
zone count, and it stages a boss slot at EVERY level), one `[[level]]`
block per relative level — coverage must match `levels` EXACTLY, both
directions — and `[[boss_slot]]`. A FIXED planet's `kits` are its
locations (every one fixed, never mixed with generated kits): each run
deals them in a seeded order, one per level — distinct while the kits
last, then wrapping — so a planet declaring more kits than levels shows
a different subset every run:

```toml
[[level]]
relative = N                   # 1..=levels, each exactly once
enemies = ["<enemy key>"]     # the COMPLETE list this level fields —
swarms = ["<swarm key>"]      # nothing carries over between levels
```

Only `spawns_directly` enemies may be listed. Planets past the last declared
file repeat the newest planet (its final roster at every level, its boss
slots and kits) until their own files arrive.

### Kits (kits.toml, hand-authored + kits.generated.toml, probed)

`[kits.<name>]`: `paradigm` and `install_dir` (repo-relative; a disk pin
holds it populated by `make assets`). The kit's GRID — `tile`/`story`,
which the planet's pitch derives from — is never authored: the probe
derives it from the assembly recipe's meshes into kits.generated.toml and
the linker joins the two. FIXED kits are the one deliberate exception —
no recipe exists, so they declare `scale` (world units per model meter;
it IS the pitch) and `environment` (a key into rosters/environments/,
whose zone map the linker validates: one start zone, one boss zone
farthest from it, connected links, non-overlapping integer boxes,
spawns inside their boxes).

### Environment files (environments/<key>.toml, hand-authored)

`[environment]`: `key`, `model` (a key in environments.generated.toml —
probed from the installed scenes). Daylight is authored, not defaulted —
ALL of a fixed interior's light arrives from outside: `[sun]`
(`azimuth_deg` 0 = model north/-z, 90 = east/+x; `elevation_deg` in
(0, 90]; `energy`) and `[[window]]` panels (`center`, `size = [w, h]`,
`facing` = the inward normal, `energy`, optional `range`) — each window
renders as a wide shadowless spot shining inward (no real-time area
lights; the lightmap bake upgrades them to true emissive panels).
`[[zone]]`: `key`,
`box = { min = [x, y, z], extents = [w, h, d] }` (model-space meters on
the 1-meter authoring grid, min-inclusive max-exclusive), `links`
(adjacent zone keys, undirected), `enemy_spawns`/`loot_spawns`
(model-space points inside the box), `start = true` (exactly one,
enemy-free), `boss = true` (exactly one, exactly ONE enemy spawn — the
arena anchor).

Further rules the linker enforces: swarm members must spawn directly;
minions never nest (the engine binds one level deep); a slot's boss may
field its own TIMED emitters but never one-shot broods (the slot's
escorts ARE its on-death/on-engage minions — one door).

## Behaviour switches (optional per-enemy fields)

Each archetype's FSM lives in code; its parameters are OPEN switches on
any `[[enemy]]` (owner 2026-07-05: no hidden switches). Omitted = the
archetype's default derivation. This is the complete list:

- `standoff_frac` — fighting-ring radius as a fraction of `attack_range`
  (default 0.6 for shooter/kiter/tank, 0 for swarmer/bomber)
- `fuse_seconds` — bomber fuse burn (default 1.0 on bombers, else 0)
- `blast_frac` — detonation radius as a fraction of `attack_range`
  (default 1.5 on bombers, else 0)
- `cloud_seconds` — the detonation leaves an occluding dust cloud at
  blast radius for this long; the cloud attacks VISION, not hull
  (default 0 = instant blast only)
- `shield_frac` — shield pool as a fraction of `hp` (default 0.5 on
  tanks, else 0)
- `disengage_frac` — give-up-the-chase range as a fraction of
  `detection` (default 1.2)
- `drain_dps` — hull drain per second while latched (default 0; the
  boss latcher declares 6.0)
- `bolt_speed` — projectile speed in m/s for firing archetypes
  (default 13.0)
- `burst_count` — bolts per trigger pull; the `cooldown` stat gates
  BURSTS, `burst_seconds` spaces the bolts inside one. Breaking sight
  forfeits the remainder (default 1 = single shot; must be ≥ 1)
- `burst_seconds` — in-burst gap between a burst's bolts (default 0.1)
- `pellet_count` — pellets fanned per bolt; damage is PER PELLET as
  declared on the def (default 1 = no fan; must be ≥ 1)
- `spread_deg` — total fan cone in degrees across the pellets
  (default 0)
- `bolt_turn_deg` — homing steer rate in deg/s; the bolt tracks the
  player (default 0 = ballistic). Pellets fly ballistic: a def declares
  a fan OR a steer rate, never both (link error)
- `pull_accel` — SIGNED tractor field in m/s² while engaged: positive
  drags the player toward this enemy, negative shoves away; linear
  falloff to zero at `attack_range`. The one switch where negative is
  legal. Magnitudes near the ship's thrust make escape cost the whole
  envelope (default 0 = no field)
- `alert_radius` — alarm aura in metres: while this enemy is engaged,
  machines within the radius of IT force-engage the player, detection
  bypassed. Kill the screamer first or fight the room (default 0 =
  no klaxon)
- `guard_radius` — guardian link in metres: damage to machines within
  the radius drinks into THIS enemy's shield first (declare
  `shield_frac` too — a shieldless guardian guards nothing); overflow
  stays with the victim; the guardian's hull is never touched through
  the link. Break the link or shoot blanks (default 0 = none)
- `jink_seconds` — erratic dodge: mean seconds between strafe re-rolls
  while engaged (tangent pulse, direction coin, vertical bob, varied
  cadence). Straight bolts whiff a jinker; homing bolts and the
  Valkyrie's tracking counter it (default 0 = smooth orbit)
- `latch_range` — metres within which a latcher counts as attached;
  the slow re-tags and the drain ticks inside it (default 2.0 on
  swarmers, else 0 = never latches)
- `slow_factor` — per-tag speed multiplier compounded onto the player,
  1.0 = no slow (default 0.7 on swarmers, else 1.0)
- `slow_duration` — seconds each slow tag lasts (default 2.0 on
  swarmers, else 0)
- `slow_interval` — re-tag period while latched (default 0.5 on
  swarmers, else 0)
- `miniboss` — while this enemy lives, its room's exits seal red and
  the boss music plays; death re-opens them. Declare it on any def
  (default false)

These are LIVE: `ai_config` builds from the resolved switches, drones
scale by the declared curves, and the roster/boss slots drive level
construction — edits here change the game. Shared feel constants
(escape-ladder timings, strafe/retreat speed multipliers, damping)
are engine tuning, not per-enemy balance — they stay code.

```toml
[[boss_slot]]
at = { relative = N }          # planet-relative level, 1..=6
boss = "<enemy key>"           # the def it fights as (size on the def)
escorts = { enemy = "<enemy key>", trigger = "on_engage", count = N }
track = N                      # boss music index
reward = "hull_container"      # or "consolation_pile"
```

Escort count is the slot's declared call — at least 1; there is no formula.
## Recipes — worked examples (linker-proven)

The switches form a large space; these are the named points in it
that make fights. Every block below is COMPILED: a test wraps each
in a standard preamble and feeds it through the real linker against
the shipped model catalog, so a recipe that stops linking fails the
build — the examples cannot go stale. Stats are starting points,
not law; scaling is omitted (defs fall back to [scaling_defaults]).
Copy, reskin, retune.

### Drift mine

Area denial — owns a doorway instead of chasing. Kill it at range or route around; the lingering cloud denies the corridor even after it pops.

```toml
[[enemy]]
key = "recipe_drift_mine"
name = "Drift Mine"
blurb = "A slow charge that owns a doorway."
model = "Enemy_QuadOrb"
size = 0.8
yaw_offset_deg = 0
ai = "bomber"
reward = 900
blast_frac = 3.0
fuse_seconds = 2.0
cloud_seconds = 5.0
[enemy.stats]
hp = 2.0
speed = 2.0
damage = 25.0
detection = 20.0
attack_range = 5.0
cooldown = 1.0
```

### Firecracker pack

Death by a dozen pops — tiny, fast, short-fused bombers placed as ONE unit. Each blast is survivable; the pack is the threat.

```toml
[[enemy]]
key = "recipe_firecracker"
name = "Firecracker"
blurb = "A fast little charge that hunts in packs."
model = "Enemy_QuadOrb"
size = 0.4
yaw_offset_deg = 0
ai = "bomber"
reward = 300
blast_frac = 0.8
fuse_seconds = 0.3
[enemy.stats]
hp = 1.0
speed = 14.0
damage = 6.0
detection = 25.0
attack_range = 4.0
cooldown = 1.0

[[swarm]]
key = "recipe_firecracker_pack"
members = [{ enemy = "recipe_firecracker", count = 4 }]
```

Staging:

```toml
[[level]]
relative = 1
enemies = ["recipe_firecracker"]
swarms = ["recipe_firecracker_pack"]
```

### Tar pit

Attacks THRUST, not hull — near-zero damage, heavy compounding slow. It holds you still for whatever else is in the room; alone it is almost harmless, which is the trap.

```toml
[[enemy]]
key = "recipe_tar_pit"
name = "Tar Pit"
blurb = "It doesn't bite; it holds you for the ones that do."
model = "alien_troop_01"
size = 1.0
yaw_offset_deg = 180
ai = "swarmer"
reward = 800
slow_factor = 0.5
slow_duration = 3.0
[enemy.stats]
hp = 4.0
speed = 12.0
damage = 0.5
detection = 25.0
attack_range = 3.0
cooldown = 1.0
```

### Dust bunny

A splitter — shoot the ball, it bursts into biting motes (minions bind ONE level deep, so it splits exactly once).

```toml
[[enemy]]
key = "recipe_dust_bunny"
name = "Dust Bunny"
blurb = "A drifting clump that comes apart angry."
model = "sphere_drone_03"
size = 1.6
yaw_offset_deg = 180
ai = "tank"
reward = 1200
minions = [{ enemy = "recipe_dust_mote", count = 2, trigger = "on_death" }]
[enemy.stats]
hp = 10.0
speed = 5.0
damage = 4.0
detection = 25.0
attack_range = 6.0
cooldown = 1.2

[[enemy]]
key = "recipe_dust_mote"
name = "Dust Mote"
blurb = "A biting fragment of the bunny."
model = "sphere_drone_01"
size = 0.5
yaw_offset_deg = 180
ai = "swarmer"
reward = 200
spawns_directly = false
[enemy.stats]
hp = 1.0
speed = 14.0
damage = 2.0
detection = 25.0
attack_range = 3.0
cooldown = 1.0
```

### Glass cannon

A long grind then an instant pop — the shield IS the hull (shield_frac ≈ 1, tiny hp). The inverse feel of a standard tank.

```toml
[[enemy]]
key = "recipe_glass_cannon"
name = "Glass Cannon"
blurb = "All shell, no meat — crack it and it's over."
model = "sphere_ship_03"
size = 1.4
yaw_offset_deg = 180
ai = "tank"
reward = 2000
shield_frac = 0.9
[enemy.stats]
hp = 12.0
speed = 6.0
damage = 12.0
detection = 25.0
attack_range = 10.0
cooldown = 1.0
```

### Machine-gun stalker

The brrrt-pause-brrrt rhythm: the cooldown gates BURSTS, the in-burst gap spaces the bolts, and speed 32 stretches them into tracers. Damage is per bolt — keep it small.

```toml
[[enemy]]
key = "recipe_mg_stalker"
name = "Machine-Gun Stalker"
blurb = "It stitches the room in threes and fives."
model = "sphere_drone_02"
size = 1.2
yaw_offset_deg = 180
ai = "shooter"
reward = 1600
burst_count = 5
burst_seconds = 0.08
bolt_speed = 32.0
[enemy.stats]
hp = 8.0
speed = 11.0
damage = 3.0
detection = 35.0
attack_range = 18.0
cooldown = 1.4
```

### Scattergun warden

A doorway blaster — a fan of pellets across a cone. Damage is PER PELLET; point-blank it all lands, at range it's chip damage. Pellets fly ballistic (never combine with homing).

```toml
[[enemy]]
key = "recipe_scattergun"
name = "Scattergun Warden"
blurb = "Cross its doorway at speed or eat the whole fan."
model = "sphere_drone_03"
size = 1.8
yaw_offset_deg = 180
ai = "tank"
reward = 2200
pellet_count = 6
spread_deg = 24.0
bolt_speed = 20.0
[enemy.stats]
hp = 18.0
speed = 7.0
damage = 3.0
detection = 30.0
attack_range = 12.0
cooldown = 1.6
```

### Tracking sniper

Slow, curving bolts that follow you — dodge by breaking the chase geometry, not by strafing. Long cooldown is the mercy.

```toml
[[enemy]]
key = "recipe_tracker"
name = "Tracking Sniper"
blurb = "Its bolts don't miss; they arrive late."
model = "sphere_ship_02"
size = 1.2
yaw_offset_deg = 180
ai = "kiter"
reward = 1800
bolt_turn_deg = 120.0
bolt_speed = 7.0
[enemy.stats]
hp = 5.0
speed = 10.0
damage = 10.0
detection = 35.0
attack_range = 20.0
cooldown = 2.5
```

### Herald (alarm)

Attacks target PRIORITY — barely fights, but while it lives and is engaged, everything near it force-engages you. Silence it first or fight the room.

```toml
[[enemy]]
key = "recipe_herald"
name = "Herald"
blurb = "A siren with a popgun."
model = "sphere_ship_01"
size = 0.9
yaw_offset_deg = 180
ai = "kiter"
reward = 1800
alert_radius = 25.0
[enemy.stats]
hp = 4.0
speed = 13.0
damage = 2.0
detection = 30.0
attack_range = 14.0
cooldown = 1.5
```

### Aegis (guardian)

Kill-order puzzle — allies inside its radius drink from ITS shield first. Declare shield_frac or it guards nothing. Break the guardian, then the guarded.

```toml
[[enemy]]
key = "recipe_aegis"
name = "Aegis"
blurb = "The others don't bleed until it does."
model = "sphere_ship_03"
size = 1.4
yaw_offset_deg = 180
ai = "tank"
reward = 2600
shield_frac = 0.9
guard_radius = 12.0
[enemy.stats]
hp = 8.0
speed = 6.0
damage = 6.0
detection = 30.0
attack_range = 9.0
cooldown = 1.3
```

### Erratic striker

A dogfighter that refuses straight lines — jink re-rolls its strafe on a varied cadence. Straight bolts whiff it; the Valkyrie's tracking is the counter.

```toml
[[enemy]]
key = "recipe_erratic"
name = "Erratic Striker"
blurb = "It flies like it's guilty of something."
model = "sphere_ship_02"
size = 1.0
yaw_offset_deg = 180
ai = "kiter"
reward = 1400
jink_seconds = 0.4
[enemy.stats]
hp = 3.0
speed = 14.0
damage = 5.0
detection = 30.0
attack_range = 14.0
cooldown = 0.9
```

### Fan boss (tractor)

The forced-movement boss: a SIGNED pull field drags you toward the gun while bursts stitch the gap — you fight it with thrust. Staged by a slot; its timed emitter is legal (one-shot broods on slot bosses are not — escorts are the slot's call).

```toml
[[enemy]]
key = "recipe_fan_boss"
name = "Fan Boss"
blurb = "Thrust is the only thing between you and the intake."
model = "apartment_boss"
size = 5.0
yaw_offset_deg = 180
ai = "kiter"
reward = 14000
spawns_directly = false
pull_accel = 7.0
burst_count = 3
burst_seconds = 0.12
bolt_speed = 30.0
minions = [{ enemy = "recipe_boss_picket", count = 2, trigger = { every_seconds = 5.0 }, cap = 6 }]
[enemy.stats]
hp = 130.0
speed = 8.0
damage = 9.0
detection = 60.0
attack_range = 18.0
cooldown = 1.5

[[enemy]]
key = "recipe_boss_picket"
name = "Picket"
blurb = "Escort stock for the staged fight."
model = "umsfd_02"
size = 0.8
yaw_offset_deg = 180
ai = "kiter"
reward = 400
spawns_directly = false
[enemy.stats]
hp = 3.0
speed = 14.0
damage = 4.0
detection = 30.0
attack_range = 9.0
cooldown = 1.0

[[enemy]]
key = "recipe_fan_patrol"
name = "Patrol"
blurb = "The level's line roster."
model = "sphere_ship_01"
size = 1.0
yaw_offset_deg = 180
ai = "kiter"
reward = 800
[enemy.stats]
hp = 3.0
speed = 12.0
damage = 5.0
detection = 25.0
attack_range = 10.0
cooldown = 1.0
```

Staging:

```toml
[[level]]
relative = 1
enemies = ["recipe_fan_patrol"]

[[boss_slot]]
at = { relative = 1 }
boss = "recipe_fan_boss"
escorts = { enemy = "recipe_boss_picket", trigger = "on_engage", count = 2 }
track = 1
reward = "hull_container"
```

### Grapple boss (lamprey)

The latch-and-drain warden: it clamps on, compounds the slow, and drinks hull by the second while its brood rises — break its grip or die by inches.

```toml
[[enemy]]
key = "recipe_grapple_boss"
name = "Grapple Boss"
blurb = "Break its grip before it breaks you."
model = "alien_troop_01"
size = 3.0
yaw_offset_deg = 180
ai = "swarmer"
reward = 11000
spawns_directly = false
drain_dps = 5.0
slow_factor = 0.6
minions = [{ enemy = "recipe_grapple_picket", count = 2, trigger = { every_seconds = 6.0 }, cap = 4 }]
[enemy.stats]
hp = 70.0
speed = 11.0
damage = 6.0
detection = 45.0
attack_range = 3.0
cooldown = 1.0

[[enemy]]
key = "recipe_grapple_picket"
name = "Picket"
blurb = "Escort stock for the staged fight."
model = "Enemy_GunDrone"
size = 0.75
yaw_offset_deg = 180
ai = "kiter"
reward = 400
spawns_directly = false
[enemy.stats]
hp = 2.0
speed = 11.0
damage = 3.0
detection = 25.0
attack_range = 8.0
cooldown = 1.2

[[enemy]]
key = "recipe_grapple_patrol"
name = "Patrol"
blurb = "The level's line roster."
model = "sphere_ship_01"
size = 1.0
yaw_offset_deg = 180
ai = "kiter"
reward = 800
[enemy.stats]
hp = 3.0
speed = 12.0
damage = 5.0
detection = 25.0
attack_range = 10.0
cooldown = 1.0
```

Staging:

```toml
[[level]]
relative = 1
enemies = ["recipe_grapple_patrol"]

[[boss_slot]]
at = { relative = 1 }
boss = "recipe_grapple_boss"
escorts = { enemy = "recipe_grapple_picket", trigger = "on_death", count = 3 }
track = 2
reward = "consolation_pile"
```

## Which file owns a fact

Six layers, each answering one question — new switches and fields land by
rule, not taste:

- PROBE (generated catalogs) — what is the asset, MEASURED? Mesh paths,
  kit grids, shape/fill. Never authored, only re-measured (`make assets`).
- DEF (`[[enemy]]`) — who is this INDIVIDUAL? Identity (key, name, blurb,
  model, size, muzzles, yaw offset), magnitudes the tier pumps (hp,
  damage, reward), and BINDINGS (minions, spawns_directly).
- SWITCHES on the def — what KIND of thing is it? Movement physicality
  and the shape of its violence in time (burst rhythm, fan, steer rate,
  fuse, latch, jink). Ratios and rhythms are character; see below.
- CURVES — how does it grow with the war? Per-stat level scaling.
- PLANET FILES — where and when does it appear? Rosters, swarms, boss
  slots, escorts, tracks, reward policy.
- ENGINE CONSTANTS (code) — what does the WORLD feel like? Damping,
  tracer stretch, klaxon sweep cadence. Universal, never per-enemy
  (owner 2026-07-05: no hidden switches — anything per-enemy tunable
  must be a declared switch).

Litmus tests for the border disputes:

1. THE TUNING TEST — when this knob turns, who should move? Every wearer
   of the concept: switch/role territory. One fleet member: def. Every
   machine in the game: engine constant. Everything at level N: curve.
2. FLEET-AGNOSTICISM — reusable character never names another def. A
   habit ("prints pickets every 5s") is character; WHICH picket is a
   binding, and bindings live on defs and slots.
3. THE RATIO TEST — fractions of other stats (shield_frac, blast_frac,
   slow_factor) are character shapes; absolute magnitudes the war
   inflates (hp, damage, reward) are tier facts. The shipped
   scaling_defaults agree: ramped stats are tier, flat ones character.
