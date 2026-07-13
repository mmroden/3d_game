# Physics: mass, impulse, and kinetic damage (design capture — 2026-07-11)

Status: **agreed shape, not yet built** — two verification gates before
implementation (§ Build order). Source: kinetics design session with Mark,
2026-07-11. Companion: docs/design/enemy_verbs.md (the hurl verb and wreck
ring consume this system); docs/architecture/physics_ownership.md (the
engine owns motion; nothing here writes velocities).

## The one law

Contact damage is priced by **momentum change**, not by who touched whom.
It's not the fall that kills you, it's the sudden stop:

```
Δv     = |contact impulse| / own_mass        (the solver's own integral, J = ∫F·dt)
damage = k · max(0, Δv − v_free)^p           (+ the attacker's declared ram, § weapons)
```

- The impulse comes from `PhysicsDirectBodyState3D::get_contact_impulse` —
  Jolt's solver already integrates the stop for us. No acceleration
  sampling (timestep-noisy), no pre-impact speed guess (blind to glancing
  vs head-on). A graze along a wall bleeds little momentum and prices as
  little; a dead stop converts everything.
- **Ginger flying is free, literally.** Below `v_free` the damage is zero —
  not 1 hp. Above it, fractional damage is fine (hull is f32 end to end).
- The flat 1-hp collision toll (`take_collision_damage`) **retires** —
  replaced by the curve, with `v_free` doing the mercy work.
- `ram_damage` folds INTO this module as the intent term (§ weapons) —
  it does not survive beside the law (one truth).

Three named knobs, one home (void-logic kinetics):
`KINETIC_V_FREE` (~4–5 m/s, a third of cruise), `KINETIC_EXPONENT`
(p = 2: energy-like — one 25 m/s slam far outprices five 5 m/s bumps),
`KINETIC_SCALE` (k: pick so a full-cruise head-on is a meaningful,
survivable bite). All three are playtest tuning.

## Mass: shape × size³ × density

Masses are real, derived, and never authored per-body:

```
shape  = clamp(Σ closed-shell volumes, hull volume) / longest_edge³   (probed, per MODEL)
mass   = ρ_eff · shape · S³                                           (computed, per USE)
```

- **shape** is a dimensionless, scale-free "stuff per unit size" factor the
  asset probe (`make assets`, headless Blender) measures once per model via
  the divergence theorem, alongside a `fill` diagnostic (shape ÷ hull
  fill ratio). Enemies/ships extend `models.generated.toml`; props need the
  same probe treatment added (they have no probed catalog today).
- **S** is whatever already recalibrates the model at its use site: the
  enemy def's `size`, a fixed kit's `scale`, a prop's authored meters.
  Same model, different sizes → cube-law masses, free:
  boss_brute (size 4) is ~4× miniboss_brute (size 2.5) on the same mesh.
  The 5×-scale apartment's furniture is 125× its normal-world mass — a
  giant teacup IS a boulder; fiction and physics agree.
- **ρ_eff** is ONE global effective density (start ~300 kg/m³): "hull plus
  whatever's inside." Closed shells (lockers, crates) measure as solid
  enclosed volume, so steel-solid density would be absurd — the effective
  constant is the honest reading, and it's deliberately the only density
  knob until playtests demand a per-category split.

### Why clamp, why hull (measured 2026-07-11, real assets)

Game meshes are not watertight (every sampled asset had thousands of
non-manifold edges) yet the signed-volume sum stays sane — EXCEPT when a
model contains overlapping closed shells (Prop_Crate_Large: sum-of-shells
exceeded its own hull by 5%). The sum counts overlap once per shell; the
true union can never exceed the hull of the vertices, so the hull is the
mathematically safe ceiling (the AABB is looser and never binds first).
The probe warns outside a plausible band (fill < 0.03 or > 1.2) so a
broken import gets eyes, never a silent weird mass.

Sampled shape discrimination (fill = enclosed ÷ hull): desk 0.21,
alien troop 0.27, shelves 0.46, sphere drone 0.48, barrel 0.69,
talon 0.71, locker 0.98, apartment boss 0.96. A ~5× honest mass spread
between "mostly air" and "solid block" at equal size, zero hand-authoring.

### Cruise speeds are invariant

Both drive-force sites (ship `fly`, enemy chase) compute
`force = desired × damp` against today's default 1 kg bodies. When masses
become real, both multiply by own mass: `force = m × desired × damp`.
Terminal speeds stay EXACTLY what the loadout and the `speed` stat declare
(player ~13 m/s base; enemies 6–14 m/s; bolts 13, tracer defs 30) — only
the momentum behind them becomes true.

## Collisions

- **Player vs world**: the law replaces the toll. Shield-first, as today
  (DamageOutcome unchanged); only the magnitude changes.
- **Player vs furniture**: self-mitigating, no special case. Impulse is
  bounded by the pair's REDUCED mass: a wall (infinite mass) converts your
  whole momentum; a 30 kg barrel yields, takes most of the Δv itself, and
  flies. Furniture is soft BECAUSE it moves. Dodging through a room
  scatters props at modest cost; walls are the hazard. (Giant-world
  furniture doesn't yield — correctly.)
- **Drones**: symmetric. Anything with a damage door prices its own Δv —
  wall slams, hurled debris, Valkyrie-tumbled props all hurt enemies too.
  `v_free` plus the AI's wall-feedback loop keeps ordinary chasing free.
- **Props**: contact monitoring is Jolt overhead, so a prop reports
  contacts only while moving above `v_free` (a velocity gate, not a
  who-threw-it flag).

### Weapons vs physics — the one deliberate split

A swarmer's ram is a WEAPON (grammar-tuned via its `damage` stat); a light
body under a pure mass law would bite like a pillow. So one contact event
prices two components:

```
total = declared_ram(attacker stat, existing PLAYER_RAM_FRACTION rules)
      + kinetic(Δv)                     — zero ram term for walls/props
```

Intent is grammar; physics is law. Neither masquerades as the other.

## Consequences owed a tuning pass (after the law ships)

- **Hull/shield economy**: a full-cruise wall slam outprices any bolt
  (agreed: kind of right). Shield value, hull upgrades, and shop prices
  were tuned against the 1-hp toll and shift meaning — retune with the
  three curve knobs as levers, in playtest.
- **Kill attribution**: kinetic kills drop their caches ownerless. If
  player-caused physics kills should reward, that needs a last-toucher
  tag (bookkeeping only — damage never consults it). Deferred.

## The hurl verb rides this for free (see enemy_verbs.md)

`hurl_speed` stays the only new grammar switch: the thrower yanks the
nearest Dynamic prop and launches it; damage emerges from the prop's mass
and impact impulse — no stamping, no armed state. Open tuning choice:
fixed launch SPEED (heavy hits harder, equal dodge window) vs fixed launch
ENERGY (heavy flies visibly slower — self-telegraphing; current lean).
The **wreck ring** (pooled scrap chunks activated at drone corpses,
Faucet tier 1 pattern) keeps throwers fed in prop-sparse boss arenas and
gives the Valkyrie blast tumble fodder.

## Build order

1. **Gate 1**: runtime check that godot-jolt fills `get_contact_impulse`
   faithfully (ship already has contact reporting on — read it in
   `integrate_forces`, log a few hits).
2. **Gate 2**: probe dry-run over the full prop/enemy/ship catalogs —
   eyeball the shape/fill table for degenerates before trusting it.
3. Kinetic law in void-logic (pure, TDD) + retire the toll + fold
   `ram_damage` into the intent term.
4. Probe fields + catalog schema + mass at spawn/load + force × mass at
   the two drive sites.
5. Hurl verb (`hurl_speed` switch, TDD like the other verbs) + prop
   contact gating.
6. Wreck ring (cloud-pool pattern).
7. Hull/shield retune playtest.

## Appendix: measurements (2026-07-11, headless Blender, raw model units)

| model | raw dims | enclosed m³ | hull m³ | fill |
|---|---|---|---|---|
| talon | 0.70×1.00×0.29 | 0.069 | 0.097 | 0.71 |
| hive | 6.19×3.59×4.78 | 7.218 | 33.64 | 0.22 |
| reaver | 0.69×0.96×0.26 | 0.010 | 0.056 | 0.19 |
| sphere_drone_01 | 179×132×142 | 837,480 | 1,751,697 | 0.48 |
| apartment_boss | 1.90×2.00×2.10 | 3.804 | 3.981 | 0.96 |
| alien_troop_01 | 0.12×0.11×0.11 | 0.0002 | 0.0006 | 0.27 |
| Prop_Shelves_WideTall | 1.98×0.54×2.67 | 1.292 | 2.837 | 0.46 |
| Prop_Crate_Large | 3.47×1.50×1.50 | 8.067 | 7.658 | **1.05** (overlap → clamp) |
| Prop_Locker | 0.98×0.50×2.67 | 1.267 | 1.288 | 0.98 |
| Prop_Desk_Medium | 2.84×1.03×0.94 | 0.541 | 2.647 | 0.21 |
| Prop_Barrel2_Closed | 0.54×0.54×0.77 | 0.099 | 0.145 | 0.69 |
| Prop_Ammo_Small | 0.40×0.43×0.28 | 0.030 | 0.034 | 0.88 |

Raw units are chaos (the sphere drone is 179 units long, the alien troop
0.12) — which is exactly why mass calibrates against the DECLARED size at
the use site, never the raw model. Ships: open question whether they get
fit-scaled like enemies (needs a declared size) or are trusted at raw
scale — decide at gate 2.
