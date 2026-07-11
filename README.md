# Void Scavenger

A 6DOF interplanetary-war roguelite: scavenge occupied planets room by room,
crack their bosses, and push deeper into the campaign. Immersive 3D is a
first-class target — the game renders SBS for xReal-class glasses, not as an
afterthought. Design docs live in [docs/architecture/](docs/architecture/),
starting with [game_plan.md](docs/architecture/game_plan.md).

## Layout

| Path | What it is |
| --- | --- |
| `rust/void-logic/` | The rules. Pure Rust, no Godot types — generation, AI, economy, the roster grammar. Everything testable lives here. |
| `rust/void-nodes/` | The shell. GodotClass nodes binding the rules to the engine via gdextension. |
| `godot/` | The Godot project: scenes, installed addons, and the GUT test suite (`godot/tests/`). |
| `rosters/` | **The game data.** Hand-authored TOML the game reads: enemies, swarms, curves, kits, planets. See below. |
| `assets/` | Raw asset packs (paid packs are downloaded manually; `make assets` refuses to run without them). |
| `scripts/` | Asset-pipeline helpers (Blender decimation, texture fixes, addon install). |
| `tools/` | The pinned Godot.app the Makefile drives. |
| `docs/architecture/` | Design and architecture docs (Faucet Principle, physics ownership, economy, …). |

## Getting started

```sh
make deps    # once: toolchains (rustup, Godot, GUT)
make assets  # once per asset change: install packs into godot/addons/ and import
make run     # release build, launch
```

`make run LEVEL=7` starts new games at level 7, `SEED=1` pins the run seed;
F9/F10 hop levels in flight. `make demo` runs a debug build, `make edit`
opens the editor. Every level entry logs a
`reproduce: make run SEED=… LEVEL=…` line — any playtest moment can be
replayed exactly.

Always drive builds through make — never bare cargo/godot; the Makefile owns
paths, the pinned engine, and the dylib install.

## Everyday targets

| Target | Does |
| --- | --- |
| `make check` | The full gate: clippy `-D warnings`, all Rust tests, full GUT suite. |
| `make test-rust [FILTER=name]` | Rust tests, optionally filtered, with output. |
| `make test-godot [F=script] [T=test]` | GUT against the installed dylib (no rebuild). Runs under an isolated HOME so test saves never touch yours. |
| `make build` / `make build-release` | Compile and install the gdextension dylib. |
| `make roster-template` | Re-render `rosters/TEMPLATE.toml`, the authoring scaffold (`make build` does this automatically). |
| `make roster-vocab` | Re-render `rosters/VOCABULARY.md`, the closed-vocabulary reference (also automatic in `build`). |
| `make assets-materials` | Re-copy sanitized materials + local patches, no reimport. |

## Authoring game content: `rosters/`

The TOML in `rosters/` is the single source the game reads — enemies exist,
change, and retire by editing it, no code involved. The workflow:

1. Copy the block you need out of [rosters/TEMPLATE.toml](rosters/TEMPLATE.toml)
   (generated; every field of every entry kind with its default spelled out).
2. Paste into the real file — `enemies.toml`, `kits.toml`, or a new
   `planets/planet_N.toml` — and fill it in.
3. Build. The linker validates the whole grammar in two phases and reports
   **every** violation in one error, naming each culprit. Closed vocabularies
   (archetypes, curve kinds, triggers, …) are listed in
   [rosters/VOCABULARY.md](rosters/VOCABULARY.md).

Rules the grammar enforces: the enemy `key` IS the identity — it flows
unchanged from the TOML through the model, the shell, saves, and the
bestiary (there are no numeric ids). Renaming a key retires the old enemy
as far as saves are concerned; old saves tolerate retired keys (ghosts
simply stop appearing). A planet declares its length and must supply
exactly one `[[level]]` roster per level — complete lists, nothing carries
over. Behaviour switches omitted in a def resolve to their archetype's
default in the linker, so authored files never bake defaults in.

Enemy `model` values are keys into `rosters/models.generated.toml` — the
catalog of installed models that `make assets` probes from disk. A model
typo or a missing install is a link error, and the catalog also lists
models nothing wears yet (what's *available*, not just what's used).

Kit grids are never authored either: `kits.toml` declares only paradigm and
install dir, and the probe derives each kit's `tile`/`story` from its
assembly-recipe meshes into `rosters/kits.generated.toml`.

`TEMPLATE.toml`, `VOCABULARY.md`, `models.generated.toml`, and
`kits.generated.toml` are generated (by the build and the asset probe) and
never hand-edited; the data files are hand-authored and never generated.
Machinery does not write your files.

## Tuning knobs

Two tiers. Tests pin contracts, never these numbers — tuning can't break
the suite.

**Scene exports — edit the ViewManager node in
[godot/scenes/main.tscn](godot/scenes/main.tscn) (text or editor), relaunch
`make demo`, no rebuild:**

| Knob | Default | Effect |
| --- | --- | --- |
| `eye_separation` | 0.065 | Stereo camera baseline (m). Lower = flatter world, easier fusion; higher = rounder, harder. |
| `convergence_distance` | 0.0 | 0 = parallel eyes (screen plane at infinity — everything pops out). Set to `C` and the zero-parallax plane sits at C meters via off-axis frustum shift. |
| `depth_strength` | 1.0 | Multiplier on `eye_separation`. **Redundant** — every consumer reads only the product `separation × strength`, so this is one degree of freedom with two names (day-one scaffold cargo-culted from stereo tooling where the pair is real). Tune ONE of them; the pair collapses into viewer-IPD + computed baseline when the stereo rig lands (docs/design/stereo_convergence_and_scaling.md). |
| `ui_plane_distance` | 4.0 | Depth of the SBS HUD/reticle plane (m). Not in the tscn by default — add the line to override the constant. Keep ≤ `convergence_distance` when converged, or the HUD sits stereo-behind the screen while drawing on top of it. |

**Rust constants — `make build` (~15 s):**

| Constant | Where | Default |
| --- | --- | --- |
| `FIGHT_MUSIC_GAIN` | `void-logic/src/audio_catalog.rs` | 1.4 — fight-bed loudness over exploration (keep × `GAMEPLAY_MUSIC_VOL` ≤ 1.0; a test guards clipping) |
| `FIGHT_CROSSFADE_SECS` / `CROSSFADE_SECS` | audio_catalog | 0.4 / 2.0 — fade-in for fight beds vs everything else |
| `GAMEPLAY_MUSIC_VOL` | audio_catalog | 0.7 — in-run music level |
| `FLASH_DECAY_SECONDS` / `FLASH_PEAK_ALPHA` | `void-logic/src/damage_flash.rs` | 5.0 / 0.35 — damage-tint fade and depth |
| `HEALTH_YELLOW_BELOW` / `HEALTH_RED_BELOW` | `void-logic/src/run_state.rs` | 0.5 / 0.25 — the one zone scale the health bar AND the damage tint speak |
| `DEFAULT_UI_PLANE_DISTANCE` | `void-nodes/src/nodes/views/view_manager.rs` | 4.0 — fallback when the tscn doesn't override |

## Architecture ground rules

- **Two crates, one wall.** `void-logic` never imports Godot; `void-nodes`
  holds policy-free shell glue. Behavior is tested in Rust (unit + property
  tests); GUT pins shell contracts headless against one deterministic level
  per scenario.
- **Faucet Principle** ([doc](docs/architecture/faucet_principle.md)):
  structural allocation happens at level creation only. During a run,
  entities flip between dormant and active — nothing instantiates mid-fight.
- **Physics ownership** ([doc](docs/architecture/physics_ownership.md)):
  Godot/Jolt integrates motion; game code applies forces and never sets
  velocities on live bodies.
- **One truth, one door.** Config flows from `GameOptions` seeded at startup;
  game data flows from `rosters/`; bad references fail loud at link time —
  no silent defaults.
