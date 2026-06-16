# Void Scavenger: Full Implementation Plan

## Game Vision

6DOF drone-piloting roguelite. You're a scavenger in a gold rush to explore mysterious bases in the asteroid belt, built by draconic dinosaur-like creatures ~65Mya. A 4-way interplanetary war (asteroids, Mars, Earth, Venus) ended with rock bombardment — the Yucatan impact that killed the dinosaurs. You pilot drones through these bases, selling information to Earth scientists.

## What's Built (Phases 0-3 + Physics Remediation)

### Combat System
- Dual hitscan lasers (ROYGBIV progression, 7 levels, damage 1-7)
- 5 mechanical enemy types, each with a distinct AI archetype: GunDrone (kiter),
  QuadOrb (swarmer — slows the player on contact), Bomber (suicide/detonate),
  EyeDrone (kiter, spawns a GunDrone on death), QuadShell (shielded tank).
  Behaviour is selected by an `Archetype` in `enemy_ai.rs`; the node turns the
  per-tick intent into forces. A timed `SlowDebuff` (`debuff.rs`) drives the
  swarmer's slow, shown by a HUD "SLOWED" indicator.
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
- All enemies are mechanical (drop components). The `EnemyCategory::Biological`
  variant is retained for forward-compatibility but currently unused.
- Dual currency is wired: mechanical kills earn **components** (in-run, lost on
  death); **organics** (permanent) are collected from glowing barrels, not kills.

### Physics Architecture (Remediated)
- Velocity readback after move_and_slide() on both player and enemies
- Death as event (in take_damage, not polled in physics_process)
- Self-destructing ephemeral nodes via SceneTreeTimer
- Upgrade routing through GameManager (lootbox → signal → RunState → push to ShipController)
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

**The economy split:** mechanical enemy kills drop components (in-run, lost on death); organics (permanent, kept across runs) are collected from glowing barrels scattered in the level debris, not from kills. Information caches (crystalline pickups, 1-2 per level) are a third permanent currency.

> **Status:** components + organics are implemented (`currency.rs`, `RunState`, `SaveGame`),
> with organics sourced from `OrganicBarrel` pickups. Information caches are still pending.

#### 4.1 Currency Types
- **New file:** `void-logic/src/currency.rs`
- `ComponentAccount` — in-run currency from mechanical kills, lost on death
- `OrganicAccount` — permanent currency from biological kills, kept across runs
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
- **Modify:** `void-logic/src/enemy_type.rs` — replace flat 1000 credits with tiered values
- Tougher enemies = more valuable drops
- Existing test `all_enemies_award_1000_credits` replaced with scaling test

#### 4.5 Economy UI Update
- **Modify:** HUD — show components + organics instead of single credits
- **Modify:** Shop UI — components for in-run purchases, organics shown for reference
- **Modify:** Kill summary — show component/organic breakdown

#### 4.6 Differentiated Loot Drops
- **Modify:** `void-nodes/src/nodes/enemy_drone.rs` — on death, spawn typed visual pickup (gear icon for mechanical, organic blob for biological) instead of generic lootbox
- Lootbox becomes component-specific or organic-specific

---

### Phase 5: Ship Selection and Loadout

#### 5.1 Ship Definition Data
- **New file:** `void-logic/src/ship.rs`
- `ShipModel` enum: Scout (Spaceship.obj), Interceptor (Spaceship2.obj), Corvette (Spaceship3.obj), Frigate (Spaceship4.obj), Dreadnought (Spaceship5.obj)
- `ShipSpec`: custom BaseStats, shield_capacity, shield_regen, hardpoint_count (2-4), model_path, display_name, organic_cost
- Compile-time SPECS table
- Scout: 2 hardpoints, low shields, fast, free (starter)
- Dreadnought: 4 hardpoints, heavy shields, slow, expensive

#### 5.2 Hardpoint + Weapon System
- **New file:** `void-logic/src/hardpoint.rs`
- `HardpointSlot`: Primary (R2), Secondary (L2), Tertiary (R1), Quaternary (L1)
- `WeaponType`: DualLaser, TrackingMissile, Grenade, Flamethrower, Stabilizer
- Each weapon: fire_rate, damage, ammo_type, ammo_cost
- Default: R2=DualLaser, L1=Stabilizer
- **Modify:** `void-nodes/src/nodes/ship_controller.rs` — read all 4 triggers
- **Modify:** `godot/project.godot` — fire_secondary, fire_tertiary, fire_quaternary actions

#### 5.3 Ship Selection Game Phase
- **Modify:** `void-logic/src/game_phase.rs` — add `ShipSelect`, transitions: `MainMenu → ShipSelect → Playing`
- **New file:** `void-nodes/src/nodes/ui/ship_select_ui.rs` — ship cards, stat comparison, organic cost
- Ships unlocked by spending organics (permanent progression)

#### 5.4 Ship Model as Player Geometry
- **Modify:** `void-nodes/src/nodes/ship_controller.rs` — load selected ship model as child MeshInstance3D around camera
- Import 5 ship OBJs into godot/addons/quaternius/spaceships/

#### 5.5 Inter-Level Rebuild Screen
- **New file:** `void-logic/src/crafting.rs` — recipes: components → weapon ammo, shield recharge, temporary upgrades
- **Modify:** `void-logic/src/game_phase.rs` — add `Rebuild` phase between KillSummary and next level
- **New file:** `void-nodes/src/nodes/ui/rebuild_ui.rs`

#### 5.6 Lives System
- **Modify:** `void-logic/src/run_state.rs` — add `lives: u32` (starts at 1, extras built from components)
- On death: if lives > 0, decrement and restart level; if 0, run ends → organics saved, components lost

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

- **All enemies are mechanical** (organic enemies removed; organics come from barrels)
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
