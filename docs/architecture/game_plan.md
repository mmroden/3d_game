# Void Scavenger: Full Implementation Plan

## Game Vision

6DOF drone-piloting roguelite. You're a scavenger in a gold rush to explore mysterious bases in the asteroid belt, built by draconic dinosaur-like creatures ~65Mya. A 4-way interplanetary war (asteroids, Mars, Earth, Venus) ended with rock bombardment — the Yucatan impact that killed the dinosaurs. You pilot drones through these bases, selling information to Earth scientists.

## What's Built (Phases 0-3 + Physics Remediation)

### Combat System
- Dual hitscan lasers (ROYGBIV progression, 7 levels, damage 1-7)
- 6 mechanical enemy types, each driven by an `Archetype` in `enemy_ai.rs`:
  GunDrone (kiter), QuadOrb (swarmer — latches within 2m and re-tags a
  compounding slow while it stays close), Bomber (suicide/detonate), EyeDrone
  (kiter; releases a SpawnDrone on death), QuadShell (shielded tank), and the
  SpawnDrone itself (weaker, faster harasser — never spawns directly, only
  from a dying EyeDrone). The node turns the per-tick intent into forces.
  A timed `SlowDebuff` (`debuff.rs`) drives the swarmer's slow, shown by a HUD
  "SLOWED" indicator. (`Archetype::Shooter` exists as the default but no roster
  enemy uses it yet.)
- Enemy projectiles (Area3D, red spheres, collision via body_entered)
- Ram/contact damage wired: enemies deal impact-scaled damage on player collision
- Player take_damage → signal → GameManager → RunState
- Ram damage on physical collision (scales with impact speed, both bounce)
- Stabilizer button (Tab/L1) zeroes angular velocity

### Shield System
- Shield newtype with absorb() → overflow to health
- ShieldState: regen (2/sec), delay (3s after hit), boost mode
- Power routing: Z/Square = ShieldBoost (5x regen, 0.3x fire, 0.5x thrust), X/Circle = WeaponBoost (0 regen, 1.5x fire)
- HUD: blue shield bar + power mode indicator ("SHIELDS" / "WEAPONS")

### Enemy Classification
- All enemies are mechanical; there is no enemy taxonomy enum (the old
  `EnemyCategory` was removed once organics moved off kills).
- Dual currency is wired, and **every reward is a physical pickup**: a kill
  credits nothing directly — it drops a blue cache carrying the type's tiered
  component reward (in-run, lost on run-over), and green caches placed at the
  level's loot spawns carry **organics** (permanent). One `CurrencyCache` node
  serves both, tinted by `CurrencyKind`.

### Bestiary Briefing (`Bestiary` phase)

- Between ship-select and the level, a briefing screen reuses the loadout
  backdrop room: one subject spins on a turntable while a low panel shows its
  name and lore; the player taps to step through the catalog and the final tap
  drops into the level. Flow: `ShipSelect → Bestiary → Playing`.
- The catalog (`void-logic/src/bestiary.rs`) always leads with the two pickups —
  the green **Organic Cache** (permanent, run-to-run upgrades) and the blue
  **Component Cache** (this-run upgrades) — so level 1, before any enemy is met,
  teaches the economy. Then it lists every enemy **seen so far**, in roster
  order. Enemies are marked on first encounter (level entry) and the seen-set is
  **permanent** across runs (`SeenEnemies` in `RunState`/`SaveGame`, like
  organics). GameManager owns the paging; `BestiaryUI` is the panel and the
  shared `Turntable` spins the subject (the same node the ship showcase uses).

### Physics Architecture (Remediated)
- Velocity readback after move_and_slide() on both player and enemies
- Death as event (in take_damage, not polled in physics_process)
- Self-destructing ephemeral nodes via SceneTreeTimer
- Pickup routing through GameManager (cache → signal → RunState; upgrades now enter via the shop, not drops)
- State sync on new game/continue (reset_loadout + push)
- Emission enabled on all emissive materials (was silently disabled everywhere)
- Enemy spawn Y range clamped to room height
- safe_look_at guard against colinear vectors

### Infrastructure
- `make check` runs Rust clippy + tests + GUT tests in one pipeline
- 440+ Rust tests, 25 GUT tests
- Serena MCP server configured for semantic code navigation

---

## What's Next (Phases 4-6)

### Phase 4: Dual Currency System

**The economy split:** mechanical enemy kills drop a blue cache carrying the type's tiered component reward (in-run, lost on run-over); green caches at the level's loot spawns carry organics (permanent, kept across runs). Nothing is credited without flying through the pickup. Information caches (crystalline pickups, 1-2 per level) are a third permanent currency.

> **Status:** components + organics are implemented (`currency.rs`, `RunState`, `SaveGame`).
> The free-upgrade lootbox is gone: one `CurrencyCache` node (kind-tinted glow) serves
> blue kill-drops and green loot-spawn pickups, rewards are tiered per enemy
> (`EnemyType::reward`), and `random_upgrade` is deleted — stat upgrades are shop stock
> (Phase 5). Information caches are still pending.

#### 4.1 Currency Types
- **New file:** `void-logic/src/currency.rs`
- `ComponentAccount` — in-run currency from mechanical kills, lost on death
- `OrganicAccount` — permanent currency from glowing barrels, kept across runs
- Distinct types prevent accidental mixing at compile time
- Same earn/spend/can_afford API as existing CreditAccount

#### 4.2 Integrate into RunState + SaveGame
- **Modify:** `void-logic/src/run_state.rs` — replace `credits: CreditAccount` with `components: ComponentAccount` + `organics: OrganicAccount`
- `record_kill()` checks `enemy_type.category()` → award to correct account
- `apply_death_penalty()` zeros components, preserves organics
- **Modify:** `void-logic/src/save_game.rs` — organics persist through death saves

#### 4.3 Information Caches
- **New file:** `void-logic/src/information_cache.rs` — discovery ID, value, chapter association
- **New file:** `void-nodes/src/nodes/information_cache.rs` — Area3D crystalline pickup, 1-2 spawned per level
- **Modify:** `void-nodes/src/nodes/level_manager.rs` — spawn caches during generation
- Caches emit signal → GameManager routes to permanent storage

#### 4.4 Variable Credit Rewards
- **Done:** tiered per `EnemyType::reward` (GunDrone 800 → QuadShell 2 500,
  SpawnDrone 400), pinned by `rewards_scale_with_tier`. The reward is only
  ever credited through the dropped cache — kills pay nothing directly.

#### 4.5 Economy UI Update
- **Done:** HUD shows components, lives, and organics; the shop shows both
  balances over its two sections. Kill-summary component/organic breakdown
  still pending.

#### 4.6 Differentiated Loot Drops
- **Done as modified:** one `CurrencyCache` node, differentiated by `CurrencyKind`
  tint (blue components glow / green organics glow) rather than distinct meshes —
  matching how the bestiary turntable already presented the two pickups

---

### Phase 5: Ship Selection and Loadout

> **Status:** done as modified — see `docs/architecture/economy.md` for the
> shop/persistence design that superseded the sketches below.

#### 5.1 Ship Definition Data
- **Done as modified:** `void-logic/src/ship_type.rs` — `ShipType`
  {Vanguard (starter, styled), Talon 300, Hive 400, Reaver 500 organics —
  the alien hull caps the roster per the owner's stated ~500 anchor},
  compile-time `SPECS` (stat muls, weapon, model path/size/yaw, blurb),
  EnemyType-pattern id/from_id. Models from `assets/cgtrader_ships/
  ship_upgrades/`, installed to stable names by `make assets`. The
  `spheres/` FBX pack is future *enemies*, not hulls — out of scope here.
  `ShipColor` remains the orthogonal trim of styled hulls.

#### 5.2 Hardpoint + Weapon System
- **Superseded** (owner decision): one weapon per hull instead of hardpoint
  slots — `WeaponKind` {HitscanLaser, TrackingLaser, ClusterMunition,
  SubdroneLauncher} in `void-logic/src/armament.rs`, dispatched by the fire
  trigger per `ShipType::spec().weapon`. Homing steer, cluster fragmentation,
  and the subdrone regen clock are pure armament math; the shared bolt ring
  serves both factions (see faucet_principle.md).

#### 5.3 Ship Selection Game Phase
- **Done:** `ship_select_ui.rs` lists the roster — owned hulls selectable,
  locked hulls priced and pointed at the shop; hulls are bought with
  organics through the shop's green section (`ShopItemId::Unlock`).

#### 5.4 Ship Model as Player Geometry
- **Done:** `ShipController::spawn_ship_model` reads path/size/yaw from the
  hull's spec (one source of truth with the turntable); a hull change is a
  model respawn.

#### 5.5 Inter-Level Rebuild Screen
- **Superseded** (owner decision): no crafting phase — the existing Shop
  phase sells everything (stat upgrades, laser, lives with blue; permanent
  unlocks and hulls with green), and it also runs between lives.

#### 5.6 Lives System
- **Done:** `RunState::lives` (starts 1, extras bought at the shop,
  `10k × 2^bought`); a spare-life death keeps everything and restarts the
  level via `Death → Shop → Playing`; the last life ends the run (components
  zeroed, bought upgrades cleared, organics/unlocks kept). Continue on the
  main menu exists only for runs that finished a level (profile/run save
  split — economy.md).

---

### Phase 6: Lore and Chapter System

#### 6.1 Chapter Progression
- **New file:** `void-logic/src/chapter.rs`
- `Chapter` enum: Asteroid, Mars, Earth, Venus
- `chapter_for_level(level: u32) -> Chapter`: Asteroid 1-10, Mars 11-20, Earth 21-30, Venus 31-40
- Each chapter: name, description, environment theme, enemy pool adjustments

#### 6.2 Lore Entries
- **New file:** `void-logic/src/lore.rs`
- Static entries per chapter:
  - **Asteroid**: creature biology, aerie design (they flew, 3D spaces), basic tech
  - **Mars**: buried deep bases, conflict origins, military technology
  - **Earth**: Yucatan strike = dinosaur extinction (65Mya!), surface devastation
  - **Venus**: atmosphere defense, peace faction's back-channel portals between aeries
- `LoreProgress` tracking discovered entries
- Unlock triggers: kill thresholds (biological scan), cache pickups, level completion

#### 6.3 Information Cache Nodes
- Crystalline meshes in levels (1-2 per level)
- On pickup: unlock specific lore entry, award cache currency

#### 6.4 Codex UI
- **New file:** `void-nodes/src/nodes/ui/codex_ui.rs`
- Browsable lore by chapter, accessible from pause menu and main menu
- Entries greyed until discovered

#### 6.5 Mission Briefing
- Brief text at level start: "Entering Asteroid Base Alpha-7..."
- Current chapter context and objective

---

## Key Design Decisions

- **All enemies are mechanical** (organic enemies removed; organics come from green caches)
- **Every reward is a pickup** — kills credit nothing directly; uncollected caches are money left floating
- **10 levels per chapter, 40 total**
- **Components lost on death, organics permanent** — creates the roguelite tension
- **Player-enemy collision is gameplay** — ramming is a tactic, stabilizer is recovery
- **GameManager is sole state authority** — all mutations route through signals → RunState
- **Enemies don't collide with each other** (if jitter becomes a problem, add layer separation)
- **Loot-stealing enemies** are a natural extension of the signal-based upgrade routing

## Execution Order

Phases 4-6 have dependencies:
- Phase 4 (dual currency) is independent, start here
- Phase 5 (ships) needs Phase 4 (organics to buy ships)
- Phase 6 (lore) needs Phase 4 (information caches) and can partially parallel Phase 5
