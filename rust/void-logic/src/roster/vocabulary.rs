//! The generated grammar reference: every CLOSED vocabulary the roster TOML
//! accepts, rendered for `rosters/VOCABULARY.md`.
//!
//! Each renderer matches EXHAUSTIVELY over its enum — adding a behaviour (a
//! new [`Archetype`] variant, a new curve kind…) refuses to compile until it
//! is described here, and `make build` re-renders the committed file
//! mechanically. The doc cannot drift: code → this renderer → the committed
//! reference, one direction — machinery writes it, hands never do.

use crate::enemy_ai::Archetype;
use crate::level_assembly::MinionTrigger;

use super::schema::{BossRewardPolicy, CurveAnchor, CurveKind, KitParadigm};

fn archetype_line(a: Archetype) -> (&'static str, &'static str) {
    match a {
        Archetype::Shooter => (
            "shooter",
            "chase to attack range, hold and fire while sighted; sight-blocked, \
             circle for an angle",
        ),
        Archetype::Kiter => (
            "kiter",
            "hold a stand-off ring: retreat when crowded, strafe and fire; \
             blocked orbits flip, then back out",
        ),
        Archetype::Swarmer => (
            "swarmer",
            "no projectile: close and press — ram damage plus the compounding \
             latch slow (drain on bosses)",
        ),
        Archetype::Tank => (
            "tank",
            "shooter with a damage-absorbing shield (half its HP); slow, \
             durable",
        ),
        Archetype::Bomber => (
            "bomber",
            "charge to detonation range, burn the fuse, AoE-detonate and die",
        ),
    }
}

fn trigger_line(t: MinionTrigger) -> (&'static str, &'static str) {
    match t {
        MinionTrigger::OnDeath => ("on_death", "bound minions rise from the corpse"),
        MinionTrigger::OnEngage => ("on_engage", "bound minions rise the moment the fight starts"),
        MinionTrigger::Every(_) => (
            "{ every_seconds = N }",
            "a batch of `count` rises every N seconds while the parent lives, \
             drawn from a pre-built ring of `cap` (dead minions return to it)",
        ),
    }
}

fn curve_kind_line(k: CurveKind) -> (&'static str, &'static str) {
    match k {
        CurveKind::Flat => ("flat", "constant 1.0 — the stat does not scale (no parameters)"),
        CurveKind::Ramp => (
            "ramp",
            "multiplies the baseline stat: linear from `at_level_1` to `at_peak` (at `peak_level`), flat past peak. For interval stats (cooldown), below 1.0 means FASTER",
        ),
    }
}

fn anchor_line(a: CurveAnchor) -> (&'static str, &'static str) {
    match a {
        CurveAnchor::Absolute => ("absolute", "curves ride the game level (the default)"),
        CurveAnchor::Entry => ("entry", "curves restart from 1 at the enemy's fleet entry level"),
    }
}

fn reward_line(r: BossRewardPolicy) -> (&'static str, &'static str) {
    match r {
        BossRewardPolicy::ConsolationPile => {
            ("consolation_pile", "blue/green pile — mid-planet bosses")
        }
        BossRewardPolicy::HullContainer => (
            "hull_container",
            "the red container (random unowned hull) — planet finals",
        ),
    }
}

fn paradigm_line(p: KitParadigm) -> (&'static str, &'static str) {
    match p {
        KitParadigm::Layered => ("layered", "tile + story stacks (the megakit)"),
        KitParadigm::Panel => ("panel", "one flat panel per cell face, cubic cells"),
    }
}

fn section(out: &mut String, title: &str, field: &str, rows: &[(&str, &str)]) {
    out.push_str(&format!("## {title}\n\n`{field}` accepts:\n\n"));
    for (value, desc) in rows {
        out.push_str(&format!("- `\"{value}\"` — {desc}\n"));
    }
    out.push('\n');
}

/// Render the full vocabulary reference (the content of
/// `rosters/VOCABULARY.md`).
pub fn vocabulary() -> String {
    let mut out = String::from(
        "# Roster grammar vocabulary\n\n\
         GENERATED — do not edit; `make build` re-renders this file\n\
         (`make roster-vocab` runs just this step).\n\
         Every list below is a CLOSED vocabulary: any other value is a\n\
         parse error naming the legal options. Open references (enemy,\n\
         swarm, kit, curve, and model keys) are declared by the data —\n\
         models by the probed catalog — and checked by the linker.\n\n",
    );
    section(
        &mut out,
        "AI behaviours",
        "ai",
        &[
            archetype_line(Archetype::Shooter),
            archetype_line(Archetype::Kiter),
            archetype_line(Archetype::Swarmer),
            archetype_line(Archetype::Tank),
            archetype_line(Archetype::Bomber),
        ],
    );
    section(
        &mut out,
        "Minion triggers",
        "escorts.trigger",
        &[
            trigger_line(MinionTrigger::OnDeath),
            trigger_line(MinionTrigger::OnEngage),
            trigger_line(MinionTrigger::Every(0.0)),
        ],
    );
    section(
        &mut out,
        "Curve kinds",
        "curves.<name>.kind",
        &[curve_kind_line(CurveKind::Flat), curve_kind_line(CurveKind::Ramp)],
    );
    section(
        &mut out,
        "Curve anchors",
        "curves.<name>.anchor",
        &[anchor_line(CurveAnchor::Absolute), anchor_line(CurveAnchor::Entry)],
    );
    section(
        &mut out,
        "Boss reward policies",
        "boss_slot.reward",
        &[
            reward_line(BossRewardPolicy::ConsolationPile),
            reward_line(BossRewardPolicy::HullContainer),
        ],
    );
    section(
        &mut out,
        "Kit paradigms",
        "kits.<name>.paradigm",
        &[paradigm_line(KitParadigm::Layered), paradigm_line(KitParadigm::Panel)],
    );
    out.push_str(SHAPES);
    out
}

/// The grammar's SHAPES — what each entry kind carries. Values in `<angle
/// brackets>` are open references (declared by the data, checked by the
/// linker); quoted values come from the closed vocabularies above.
const SHAPES: &str = "\
## Entry shapes

### `[[enemy]]` (enemies.toml)

Identity (global): `key`, `id` (append-only numeric crossing), `name`,
`model` (a key in models.generated.toml — the catalog `make assets` probes
from the installed files), `size` (metres, longest edge), `yaw_offset_deg`,
`ai`, `reward`,
`spawns_directly` (default true; false = death-spawned/escort/staged only).

Minions — pre-staged bound drones and when they rise:

```toml
minions = [{ enemy = \"<enemy key>\", count = N, trigger = \"on_death\" }]
```

Tuning (level-bound): `[enemy.stats]` baseline (`hp`, `speed`, `damage`,
`detection`, `attack_range`, `cooldown`) × `[enemy.scaling]` per-stat curve
refs (falls back to `[scaling_defaults]`).

### `[[swarm]]` (enemies.toml)

`key` + `members = [{ enemy = \"<enemy key>\", count = N }]` — placed as ONE
unit: same room, adjacent cells. Schedulable anywhere an enemy is.

### Planet files (planets/planet_N.toml)

`planet`, `levels` (declared length), `kits = [\"<kit key>\"]` (pitch and
paradigm DERIVE from the kit), `rooms = { base, per_level }`, one `[[level]]`
block per relative level — coverage must match `levels` EXACTLY, both
directions — and `[[boss_slot]]`:

```toml
[[level]]
relative = N                   # 1..=levels, each exactly once
enemies = [\"<enemy key>\"]     # the COMPLETE list this level fields —
swarms = [\"<swarm key>\"]      # nothing carries over between levels
```

Only `spawns_directly` enemies may be listed. Planets past the last declared
file repeat the newest planet (its final roster at every level, its boss
slots and kits) until their own files arrive.

### Kits (kits.toml, hand-authored + kits.generated.toml, probed)

`[kits.<name>]`: `paradigm` and `install_dir` (repo-relative; a disk pin
holds it populated by `make assets`). The kit's GRID — `tile`/`story`,
which the planet's pitch derives from — is never authored: the probe
derives it from the assembly recipe's meshes into kits.generated.toml and
the linker joins the two.

Further rules the linker enforces: swarm members must spawn directly;
minions never nest (the engine binds one level deep); a slot's boss must
not declare def-level minions (the slot's escorts ARE its minions).

## Behaviour switches (optional per-enemy fields)

Each archetype's FSM lives in code; its parameters are OPEN switches on
any `[[enemy]]` (owner 2026-07-05: no hidden switches). Omitted = the
archetype's default derivation. This is the complete list:

- `standoff_frac` — fighting-ring radius as a fraction of `attack_range`
  (default 0.6 for shooter/kiter/tank, 0 for swarmer/bomber)
- `fuse_seconds` — bomber fuse burn (default 1.0 on bombers, else 0)
- `blast_frac` — detonation radius as a fraction of `attack_range`
  (default 1.5 on bombers, else 0)
- `shield_frac` — shield pool as a fraction of `hp` (default 0.5 on
  tanks, else 0)
- `disengage_frac` — give-up-the-chase range as a fraction of
  `detection` (default 1.2)
- `drain_dps` — hull drain per second while latched (default 0; the
  boss latcher declares 6.0)
- `bolt_speed` — projectile speed in m/s for firing archetypes
  (default 13.0)
- `latch_range` — metres within which a latcher counts as attached;
  the slow re-tags and the drain ticks inside it (default 2.0 on
  swarmers, else 0 = never latches)
- `slow_factor` — per-tag speed multiplier compounded onto the player,
  1.0 = no slow (default 0.7 on swarmers, else 1.0)
- `slow_duration` — seconds each slow tag lasts (default 2.0 on
  swarmers, else 0)
- `slow_interval` — re-tag period while latched (default 0.5 on
  swarmers, else 0)

These are LIVE: `ai_config` builds from the resolved switches, drones
scale by the declared curves, and the roster/boss slots drive level
construction — edits here change the game. Shared feel constants
(escape-ladder timings, strafe/retreat speed multipliers, damping)
are engine tuning, not per-enemy balance — they stay code.

```toml
[[boss_slot]]
at = { relative = N }          # planet-relative level, 1..=6
boss = \"<enemy key>\"           # the def it fights as (size on the def)
escorts = { enemy = \"<enemy key>\", trigger = \"on_engage\", count = N }
track = N                      # boss music index
reward = \"hull_container\"      # or \"consolation_pile\"
```

Escort count is the slot's declared call — at least 1; there is no formula.
";
