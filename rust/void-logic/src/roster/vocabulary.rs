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

use crate::asset_catalog::schema::KitParadigm;

use super::schema::{BossRewardPolicy, CurveAnchor, CurveKind};

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
            "a timed emitter: a batch of `count` rises every N seconds while \
             the parent lives. `cap` is REQUIRED and is the size of the \
             pre-built ring the batches draw from — at most `cap` are afield \
             at once, dead ring members recycle back in, so the pressure \
             never runs dry (the linker requires cap >= count). One-shot \
             triggers take no cap: their `count` IS the whole brood",
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
        KitParadigm::Fixed => (
            "fixed",
            "a pre-modeled environment scene installed whole — the kit \
             declares `environment` (its authored zone map) and `scale` \
             (world units per model meter) instead of deriving a grid",
        ),
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
        &[
            paradigm_line(KitParadigm::Layered),
            paradigm_line(KitParadigm::Panel),
            paradigm_line(KitParadigm::Fixed),
        ],
    );
    out.push_str(SHAPES);
    render_recipes(&mut out);
    out.push_str(LAYERS);
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
`muzzles` (optional barrel tips, aim-frame metres [x right, y up, z toward
the player]; shots round-robin the list, omitted/empty = fire from the
hull centre), `ai`, `reward`,
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
paradigm DERIVE from the kit), `rooms = { base, per_level }` (GENERATED
planets only — a fixed planet's room count is its environment's zone count,
and it declares exactly one kit plus a boss slot at EVERY level), one
`[[level]]` block per relative level — coverage must match `levels`
EXACTLY, both directions — and `[[boss_slot]]`:

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
boss = \"<enemy key>\"           # the def it fights as (size on the def)
escorts = { enemy = \"<enemy key>\", trigger = \"on_engage\", count = N }
track = N                      # boss music index
reward = \"hull_container\"      # or \"consolation_pile\"
```

Escort count is the slot's declared call — at least 1; there is no formula.
";

/// One worked recipe: a named point in the switch space, rendered into the
/// vocabulary AND proven against the real linker by `every_recipe_links` —
/// the examples cannot drift from the grammar (a broken recipe fails the
/// build, not a reader).
struct Recipe {
    name: &'static str,
    /// One-line design intent — which player resource it attacks.
    what: &'static str,
    /// `[[enemy]]`/`[[swarm]]` fragment, complete and linkable.
    enemies: &'static str,
    /// `[[level]]`/`[[boss_slot]]` staging for the wrapper; rendered into
    /// the doc only when it teaches (boss slots, swarms).
    planet: &'static str,
}

fn render_recipes(out: &mut String) {
    out.push_str(
        "## Recipes — worked examples (linker-proven)\n\n\
         The switches form a large space; these are the named points in it\n\
         that make fights. Every block below is COMPILED: a test wraps each\n\
         in a standard preamble and feeds it through the real linker against\n\
         the shipped model catalog, so a recipe that stops linking fails the\n\
         build — the examples cannot go stale. Stats are starting points,\n\
         not law; scaling is omitted (defs fall back to [scaling_defaults]).\n\
         Copy, reskin, retune.\n\n",
    );
    for r in RECIPES {
        out.push_str(&format!("### {}\n\n{}\n\n```toml\n{}```\n", r.name, r.what, r.enemies));
        if r.planet.contains("boss_slot") || r.planet.contains("swarms") {
            out.push_str(&format!("\nStaging:\n\n```toml\n{}```\n", r.planet));
        }
        out.push('\n');
    }
}

const RECIPES: &[Recipe] = &[
    Recipe {
        name: "Drift mine",
        what: "Area denial — owns a doorway instead of chasing. Kill it at \
               range or route around; the lingering cloud denies the corridor \
               even after it pops.",
        enemies: "[[enemy]]\nkey = \"recipe_drift_mine\"\nname = \"Drift Mine\"\n\
            blurb = \"A slow charge that owns a doorway.\"\nmodel = \"Enemy_QuadOrb\"\n\
            size = 0.8\nyaw_offset_deg = 0\nai = \"bomber\"\nreward = 900\n\
            blast_frac = 3.0\nfuse_seconds = 2.0\ncloud_seconds = 5.0\n\
            [enemy.stats]\nhp = 2.0\nspeed = 2.0\ndamage = 25.0\ndetection = 20.0\nattack_range = 5.0\ncooldown = 1.0\n",
        planet: "[[level]]\nrelative = 1\nenemies = [\"recipe_drift_mine\"]\n",
    },
    Recipe {
        name: "Firecracker pack",
        what: "Death by a dozen pops — tiny, fast, short-fused bombers placed \
               as ONE unit. Each blast is survivable; the pack is the threat.",
        enemies: "[[enemy]]\nkey = \"recipe_firecracker\"\nname = \"Firecracker\"\n\
            blurb = \"A fast little charge that hunts in packs.\"\nmodel = \"Enemy_QuadOrb\"\n\
            size = 0.4\nyaw_offset_deg = 0\nai = \"bomber\"\nreward = 300\n\
            blast_frac = 0.8\nfuse_seconds = 0.3\n\
            [enemy.stats]\nhp = 1.0\nspeed = 14.0\ndamage = 6.0\ndetection = 25.0\nattack_range = 4.0\ncooldown = 1.0\n\n\
            [[swarm]]\nkey = \"recipe_firecracker_pack\"\n\
            members = [{ enemy = \"recipe_firecracker\", count = 4 }]\n",
        planet: "[[level]]\nrelative = 1\nenemies = [\"recipe_firecracker\"]\nswarms = [\"recipe_firecracker_pack\"]\n",
    },
    Recipe {
        name: "Tar pit",
        what: "Attacks THRUST, not hull — near-zero damage, heavy compounding \
               slow. It holds you still for whatever else is in the room; \
               alone it is almost harmless, which is the trap.",
        enemies: "[[enemy]]\nkey = \"recipe_tar_pit\"\nname = \"Tar Pit\"\n\
            blurb = \"It doesn't bite; it holds you for the ones that do.\"\nmodel = \"alien_troop_01\"\n\
            size = 1.0\nyaw_offset_deg = 180\nai = \"swarmer\"\nreward = 800\n\
            slow_factor = 0.5\nslow_duration = 3.0\n\
            [enemy.stats]\nhp = 4.0\nspeed = 12.0\ndamage = 0.5\ndetection = 25.0\nattack_range = 3.0\ncooldown = 1.0\n",
        planet: "[[level]]\nrelative = 1\nenemies = [\"recipe_tar_pit\"]\n",
    },
    Recipe {
        name: "Dust bunny",
        what: "A splitter — shoot the ball, it bursts into biting motes \
               (minions bind ONE level deep, so it splits exactly once).",
        enemies: "[[enemy]]\nkey = \"recipe_dust_bunny\"\nname = \"Dust Bunny\"\n\
            blurb = \"A drifting clump that comes apart angry.\"\nmodel = \"sphere_drone_03\"\n\
            size = 1.6\nyaw_offset_deg = 180\nai = \"tank\"\nreward = 1200\n\
            minions = [{ enemy = \"recipe_dust_mote\", count = 2, trigger = \"on_death\" }]\n\
            [enemy.stats]\nhp = 10.0\nspeed = 5.0\ndamage = 4.0\ndetection = 25.0\nattack_range = 6.0\ncooldown = 1.2\n\n\
            [[enemy]]\nkey = \"recipe_dust_mote\"\nname = \"Dust Mote\"\n\
            blurb = \"A biting fragment of the bunny.\"\nmodel = \"sphere_drone_01\"\n\
            size = 0.5\nyaw_offset_deg = 180\nai = \"swarmer\"\nreward = 200\n\
            spawns_directly = false\n\
            [enemy.stats]\nhp = 1.0\nspeed = 14.0\ndamage = 2.0\ndetection = 25.0\nattack_range = 3.0\ncooldown = 1.0\n",
        planet: "[[level]]\nrelative = 1\nenemies = [\"recipe_dust_bunny\"]\n",
    },
    Recipe {
        name: "Glass cannon",
        what: "A long grind then an instant pop — the shield IS the hull \
               (shield_frac ≈ 1, tiny hp). The inverse feel of a standard tank.",
        enemies: "[[enemy]]\nkey = \"recipe_glass_cannon\"\nname = \"Glass Cannon\"\n\
            blurb = \"All shell, no meat — crack it and it's over.\"\nmodel = \"sphere_ship_03\"\n\
            size = 1.4\nyaw_offset_deg = 180\nai = \"tank\"\nreward = 2000\n\
            shield_frac = 0.9\n\
            [enemy.stats]\nhp = 12.0\nspeed = 6.0\ndamage = 12.0\ndetection = 25.0\nattack_range = 10.0\ncooldown = 1.0\n",
        planet: "[[level]]\nrelative = 1\nenemies = [\"recipe_glass_cannon\"]\n",
    },
    Recipe {
        name: "Machine-gun stalker",
        what: "The brrrt-pause-brrrt rhythm: the cooldown gates BURSTS, the \
               in-burst gap spaces the bolts, and speed 32 stretches them \
               into tracers. Damage is per bolt — keep it small.",
        enemies: "[[enemy]]\nkey = \"recipe_mg_stalker\"\nname = \"Machine-Gun Stalker\"\n\
            blurb = \"It stitches the room in threes and fives.\"\nmodel = \"sphere_drone_02\"\n\
            size = 1.2\nyaw_offset_deg = 180\nai = \"shooter\"\nreward = 1600\n\
            burst_count = 5\nburst_seconds = 0.08\nbolt_speed = 32.0\n\
            [enemy.stats]\nhp = 8.0\nspeed = 11.0\ndamage = 3.0\ndetection = 35.0\nattack_range = 18.0\ncooldown = 1.4\n",
        planet: "[[level]]\nrelative = 1\nenemies = [\"recipe_mg_stalker\"]\n",
    },
    Recipe {
        name: "Scattergun warden",
        what: "A doorway blaster — a fan of pellets across a cone. Damage is \
               PER PELLET; point-blank it all lands, at range it's chip \
               damage. Pellets fly ballistic (never combine with homing).",
        enemies: "[[enemy]]\nkey = \"recipe_scattergun\"\nname = \"Scattergun Warden\"\n\
            blurb = \"Cross its doorway at speed or eat the whole fan.\"\nmodel = \"sphere_drone_03\"\n\
            size = 1.8\nyaw_offset_deg = 180\nai = \"tank\"\nreward = 2200\n\
            pellet_count = 6\nspread_deg = 24.0\nbolt_speed = 20.0\n\
            [enemy.stats]\nhp = 18.0\nspeed = 7.0\ndamage = 3.0\ndetection = 30.0\nattack_range = 12.0\ncooldown = 1.6\n",
        planet: "[[level]]\nrelative = 1\nenemies = [\"recipe_scattergun\"]\n",
    },
    Recipe {
        name: "Tracking sniper",
        what: "Slow, curving bolts that follow you — dodge by breaking the \
               chase geometry, not by strafing. Long cooldown is the mercy.",
        enemies: "[[enemy]]\nkey = \"recipe_tracker\"\nname = \"Tracking Sniper\"\n\
            blurb = \"Its bolts don't miss; they arrive late.\"\nmodel = \"sphere_ship_02\"\n\
            size = 1.2\nyaw_offset_deg = 180\nai = \"kiter\"\nreward = 1800\n\
            bolt_turn_deg = 120.0\nbolt_speed = 7.0\n\
            [enemy.stats]\nhp = 5.0\nspeed = 10.0\ndamage = 10.0\ndetection = 35.0\nattack_range = 20.0\ncooldown = 2.5\n",
        planet: "[[level]]\nrelative = 1\nenemies = [\"recipe_tracker\"]\n",
    },
    Recipe {
        name: "Herald (alarm)",
        what: "Attacks target PRIORITY — barely fights, but while it lives \
               and is engaged, everything near it force-engages you. Silence \
               it first or fight the room.",
        enemies: "[[enemy]]\nkey = \"recipe_herald\"\nname = \"Herald\"\n\
            blurb = \"A siren with a popgun.\"\nmodel = \"sphere_ship_01\"\n\
            size = 0.9\nyaw_offset_deg = 180\nai = \"kiter\"\nreward = 1800\n\
            alert_radius = 25.0\n\
            [enemy.stats]\nhp = 4.0\nspeed = 13.0\ndamage = 2.0\ndetection = 30.0\nattack_range = 14.0\ncooldown = 1.5\n",
        planet: "[[level]]\nrelative = 1\nenemies = [\"recipe_herald\"]\n",
    },
    Recipe {
        name: "Aegis (guardian)",
        what: "Kill-order puzzle — allies inside its radius drink from ITS \
               shield first. Declare shield_frac or it guards nothing. Break \
               the guardian, then the guarded.",
        enemies: "[[enemy]]\nkey = \"recipe_aegis\"\nname = \"Aegis\"\n\
            blurb = \"The others don't bleed until it does.\"\nmodel = \"sphere_ship_03\"\n\
            size = 1.4\nyaw_offset_deg = 180\nai = \"tank\"\nreward = 2600\n\
            shield_frac = 0.9\nguard_radius = 12.0\n\
            [enemy.stats]\nhp = 8.0\nspeed = 6.0\ndamage = 6.0\ndetection = 30.0\nattack_range = 9.0\ncooldown = 1.3\n",
        planet: "[[level]]\nrelative = 1\nenemies = [\"recipe_aegis\"]\n",
    },
    Recipe {
        name: "Erratic striker",
        what: "A dogfighter that refuses straight lines — jink re-rolls its \
               strafe on a varied cadence. Straight bolts whiff it; the \
               Valkyrie's tracking is the counter.",
        enemies: "[[enemy]]\nkey = \"recipe_erratic\"\nname = \"Erratic Striker\"\n\
            blurb = \"It flies like it's guilty of something.\"\nmodel = \"sphere_ship_02\"\n\
            size = 1.0\nyaw_offset_deg = 180\nai = \"kiter\"\nreward = 1400\n\
            jink_seconds = 0.4\n\
            [enemy.stats]\nhp = 3.0\nspeed = 14.0\ndamage = 5.0\ndetection = 30.0\nattack_range = 14.0\ncooldown = 0.9\n",
        planet: "[[level]]\nrelative = 1\nenemies = [\"recipe_erratic\"]\n",
    },
    Recipe {
        name: "Fan boss (tractor)",
        what: "The forced-movement boss: a SIGNED pull field drags you toward \
               the gun while bursts stitch the gap — you fight it with \
               thrust. Staged by a slot; its timed emitter is legal (one-shot \
               broods on slot bosses are not — escorts are the slot's call).",
        enemies: "[[enemy]]\nkey = \"recipe_fan_boss\"\nname = \"Fan Boss\"\n\
            blurb = \"Thrust is the only thing between you and the intake.\"\nmodel = \"apartment_boss\"\n\
            size = 5.0\nyaw_offset_deg = 180\nai = \"kiter\"\nreward = 14000\n\
            spawns_directly = false\n\
            pull_accel = 7.0\nburst_count = 3\nburst_seconds = 0.12\nbolt_speed = 30.0\n\
            minions = [{ enemy = \"recipe_boss_picket\", count = 2, trigger = { every_seconds = 5.0 }, cap = 6 }]\n\
            [enemy.stats]\nhp = 130.0\nspeed = 8.0\ndamage = 9.0\ndetection = 60.0\nattack_range = 18.0\ncooldown = 1.5\n\n\
            [[enemy]]\nkey = \"recipe_boss_picket\"\nname = \"Picket\"\n\
            blurb = \"Escort stock for the staged fight.\"\nmodel = \"umsfd_02\"\n\
            size = 0.8\nyaw_offset_deg = 180\nai = \"kiter\"\nreward = 400\n\
            spawns_directly = false\n\
            [enemy.stats]\nhp = 3.0\nspeed = 14.0\ndamage = 4.0\ndetection = 30.0\nattack_range = 9.0\ncooldown = 1.0\n\n\
            [[enemy]]\nkey = \"recipe_fan_patrol\"\nname = \"Patrol\"\n\
            blurb = \"The level's line roster.\"\nmodel = \"sphere_ship_01\"\n\
            size = 1.0\nyaw_offset_deg = 180\nai = \"kiter\"\nreward = 800\n\
            [enemy.stats]\nhp = 3.0\nspeed = 12.0\ndamage = 5.0\ndetection = 25.0\nattack_range = 10.0\ncooldown = 1.0\n",
        planet: "[[level]]\nrelative = 1\nenemies = [\"recipe_fan_patrol\"]\n\n\
            [[boss_slot]]\nat = { relative = 1 }\nboss = \"recipe_fan_boss\"\n\
            escorts = { enemy = \"recipe_boss_picket\", trigger = \"on_engage\", count = 2 }\n\
            track = 1\nreward = \"hull_container\"\n",
    },
    Recipe {
        name: "Grapple boss (lamprey)",
        what: "The latch-and-drain warden: it clamps on, compounds the slow, \
               and drinks hull by the second while its brood rises — break \
               its grip or die by inches.",
        enemies: "[[enemy]]\nkey = \"recipe_grapple_boss\"\nname = \"Grapple Boss\"\n\
            blurb = \"Break its grip before it breaks you.\"\nmodel = \"alien_troop_01\"\n\
            size = 3.0\nyaw_offset_deg = 180\nai = \"swarmer\"\nreward = 11000\n\
            spawns_directly = false\n\
            drain_dps = 5.0\nslow_factor = 0.6\n\
            minions = [{ enemy = \"recipe_grapple_picket\", count = 2, trigger = { every_seconds = 6.0 }, cap = 4 }]\n\
            [enemy.stats]\nhp = 70.0\nspeed = 11.0\ndamage = 6.0\ndetection = 45.0\nattack_range = 3.0\ncooldown = 1.0\n\n\
            [[enemy]]\nkey = \"recipe_grapple_picket\"\nname = \"Picket\"\n\
            blurb = \"Escort stock for the staged fight.\"\nmodel = \"Enemy_GunDrone\"\n\
            size = 0.75\nyaw_offset_deg = 180\nai = \"kiter\"\nreward = 400\n\
            spawns_directly = false\n\
            [enemy.stats]\nhp = 2.0\nspeed = 11.0\ndamage = 3.0\ndetection = 25.0\nattack_range = 8.0\ncooldown = 1.2\n\n\
            [[enemy]]\nkey = \"recipe_grapple_patrol\"\nname = \"Patrol\"\n\
            blurb = \"The level's line roster.\"\nmodel = \"sphere_ship_01\"\n\
            size = 1.0\nyaw_offset_deg = 180\nai = \"kiter\"\nreward = 800\n\
            [enemy.stats]\nhp = 3.0\nspeed = 12.0\ndamage = 5.0\ndetection = 25.0\nattack_range = 10.0\ncooldown = 1.0\n",
        planet: "[[level]]\nrelative = 1\nenemies = [\"recipe_grapple_patrol\"]\n\n\
            [[boss_slot]]\nat = { relative = 1 }\nboss = \"recipe_grapple_boss\"\n\
            escorts = { enemy = \"recipe_grapple_picket\", trigger = \"on_death\", count = 3 }\n\
            track = 2\nreward = \"consolation_pile\"\n",
    },
];

#[cfg(test)]
mod tests {
    /// Inert curves + defaults every recipe links inside — hermetic, so no
    /// shipped curve name can couple the doc to tuning.
    const PREAMBLE: &str = "[curves.flat]\nkind = \"flat\"\n\n\
        [scaling_defaults]\nspeed = \"flat\"\ncooldown = \"flat\"\nhp = \"flat\"\n\
        damage = \"flat\"\ndetection = \"flat\"\nattack_range = \"flat\"\n\n";
    const PLANET_HEADER: &str = "planet = 1\nlevels = 1\nkits = [\"recipe_kit\"]\n\
        rooms = { base = 6, per_level = 2 }\n\n";

    #[test]
    fn every_recipe_links_against_the_real_grammar() {
        // The recipes are DOCUMENTATION THAT COMPILES: each block feeds
        // through the real linker against the shipped model catalog. A
        // switch rename or grammar change that breaks a recipe fails here,
        // not in a reader's copy-paste.
        for r in super::RECIPES {
            let enemies = format!("{PREAMBLE}{}", r.enemies);
            let planet = format!("{PLANET_HEADER}{}", r.planet);
            let kits = "[kits.recipe_kit]\nparadigm = \"panel\"\n\
                install_dir = \"godot/addons/walls\"\nwall_coverage = 0.9\n";
            let grid = &format!(
                "[kits.recipe_kit]\ntile = 3.0\nstory = 3.0\n{}",
                crate::roster::TEST_CENSUS_ONE_PLATE,
            );
            let catalog = crate::asset_catalog::AssetCatalog::load(
                kits,
                grid,
                crate::asset_catalog::MODELS_TOML,
                crate::asset_catalog::ENVIRONMENTS_GENERATED_TOML,
            )
            .unwrap_or_else(|e| {
                panic!(
                    "recipe '{}' catalog no longer links — the doc has drifted:\n{}",
                    r.name,
                    e.join("\n")
                )
            });
            if let Err(e) = super::super::load_from(
                &enemies,
                &[planet.as_str()],
                super::super::EnvSources::generated_only(),
                std::sync::Arc::new(catalog),
            ) {
                panic!("recipe '{}' no longer links — the doc has drifted:\n{e}", r.name);
            }
        }
    }
}

const LAYERS: &str = "\
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
   habit (\"prints pickets every 5s\") is character; WHICH picket is a
   binding, and bindings live on defs and slots.
3. THE RATIO TEST — fractions of other stats (shield_frac, blast_frac,
   slow_factor) are character shapes; absolute magnitudes the war
   inflates (hp, damage, reward) are tier facts. The shipped
   scaling_defaults agree: ramped stats are tier, flat ones character.
";
