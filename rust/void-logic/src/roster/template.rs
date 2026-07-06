//! The authoring scaffold (owner 2026-07-05): a COMPLETE annotated example
//! of every entry kind the grammar accepts, rendered to
//! `rosters/TEMPLATE.toml` by `make roster-template`.
//!
//! Copy the relevant block into `enemies.toml` / a new
//! `planets/planet_N.toml` and fill it in — every field is present with
//! its default spelled out, so nothing is discoverable only by reading
//! Rust. `make build` regenerates the committed file mechanically (no
//! golden pin to trip), and `the_template_is_a_valid_grammar` PARSES AND
//! LINKS the template's sections — the scaffold can never drift into an
//! example the loader would reject.

/// Section separator: the validity test splits the rendered template on
/// these to feed each part through the real loader.
pub const CUT: &str = "# ================= 8< =================";

pub fn template() -> String {
    format!(
        "{ENEMIES}\n{CUT}\n{KITS}\n{CUT}\n{PLANET}"
    )
}

const ENEMIES: &str = r#"# TEMPLATE — GENERATED, do not edit; `make build` re-renders this file.
# Copy blocks into rosters/*.toml and fill them in. Every field appears
# here with its default; the linker names anything you get wrong. The
# closed vocabularies are listed in rosters/VOCABULARY.md.

# ── Section: enemies.toml ────────────────────────────────────────────────

# A curve multiplies a baseline stat by level. kind = "flat" | "ramp".
[curves.flat]
kind = "flat"

[curves.example_ramp]
kind = "ramp"
at_level_1 = 0.6   # multiplier at game level 1
at_peak = 1.15     # multiplier at peak_level (flat past it)
peak_level = 10
anchor = "absolute" # or "entry": restart from 1 at the fleet entry level

# Fallback curve refs for any enemy that doesn't declare its own. All six
# stats are declared — flat is a config call, not an absence.
[scaling_defaults]
speed = "example_ramp"
cooldown = "example_ramp"
hp = "flat"
damage = "flat"
detection = "flat"
attack_range = "flat"

[[enemy]]
key = "template_enemy"   # stable identity (rosters, saves, linker refs)
id = 1000                # GDScript/save crossing — append-only, never reuse
name = "Template Enemy"
blurb = "Bestiary lore — every enemy is catalogued."
model = "sphere_ship_01"    # a KEY in models.generated.toml (make assets)
size = 1.0               # metres, longest edge (fit-scaled)
yaw_offset_deg = 180     # imported front-axis correction (0 or 180 so far)
ai = "shooter"           # shooter | kiter | swarmer | tank | bomber
reward = 800             # components in its death cache
spawns_directly = true   # false = minion/escort/boss-staged only
# Behaviour switches — omit for the archetype's default (shown):
standoff_frac = 0.6      # fighting ring, fraction of attack_range
                         # (default 0.6 shooter/kiter/tank, 0 swarmer/bomber)
fuse_seconds = 0.0       # bomber fuse (default 1.0 on bombers, else 0)
blast_frac = 0.0         # bomber blast radius, fraction of attack_range
                         # (default 1.5 on bombers, else 0)
shield_frac = 0.0        # shield pool, fraction of hp (default 0.5 tanks)
disengage_frac = 1.2     # give-up-the-chase, fraction of detection
drain_dps = 0.0          # hull drain per second while latched (swarmers)
bolt_speed = 13.0        # projectile speed, m/s (firing archetypes)
latch_range = 0.0        # attached within this range: slow re-tags, drain
                         # ticks (default 2.0 swarmers, else 0 = never)
slow_factor = 1.0        # per-tag speed multiplier, compounds; 1.0 = none
                         # (default 0.7 swarmers, else 1.0)
slow_duration = 0.0      # seconds a tag lasts (default 2.0 swarmers)
slow_interval = 0.0      # re-tag period while latched (default 0.5 swarmers)
# Bound minions: one-shot broods and/or timed emitters.
minions = [
  { enemy = "template_minion", count = 2, trigger = "on_death" },
  { enemy = "template_minion", count = 1, trigger = { every_seconds = 10.0 }, cap = 4 },
]
[enemy.stats]
hp = 3.0
speed = 9.0
damage = 5.0
detection = 25.0
attack_range = 10.0
cooldown = 1.0
[enemy.scaling]          # optional, per-field — falls back to [scaling_defaults]
speed = "example_ramp"
cooldown = "example_ramp"
hp = "flat"              # point at a ramp and this enemy toughens by level
damage = "flat"          # every stat scales through its declared curve —
detection = "flat"       # the AI's derived ranges (standoff, disengage,
attack_range = "flat"    # blast, shield) follow their base stat's curve

[[enemy]]
key = "template_minion"
id = 1001
name = "Template Minion"
blurb = "A lesser machine the template enemy fields."
model = "sphere_ship_02"
size = 0.5
yaw_offset_deg = 180
ai = "kiter"
reward = 400
spawns_directly = false  # exists only as another machine's minion
[enemy.stats]
hp = 2.0
speed = 11.0
damage = 3.0
detection = 25.0
attack_range = 8.0
cooldown = 1.2

[[enemy]]
key = "template_boss"
id = 1002
name = "Template Boss"
blurb = "A staged fight. Its escorts are declared on the boss slot."
model = "evil_mech_03"
size = 4.0               # arena scale
yaw_offset_deg = 180
ai = "kiter"
reward = 10000
spawns_directly = false  # placed only by a boss slot
# A slot-staged boss must NOT declare def-level minions: the slot's
# escorts ARE its minions (one door).
[enemy.stats]
hp = 60.0
speed = 8.0
damage = 16.0
detection = 45.0
attack_range = 14.0
cooldown = 1.4

# Swarms place as ONE unit: same room, adjacent cells. Members must
# spawn directly.
[[swarm]]
key = "template_swarm"
members = [{ enemy = "template_enemy", count = 2 }]
"#;

const KITS: &str = r#"# ── Section: kits.toml (interim; the make-assets probe will generate) ───

[kits.template_kit]
paradigm = "panel"       # layered (tile+story stacks) | panel (cubic cells)
install_dir = "godot/addons/walls"  # repo-relative; disk-pinned populated
# The kit's grid (tile/story) is NEVER authored: the make-assets probe
# derives it from the assembly recipe into kits.generated.toml.
"#;

const PLANET: &str = r#"# ── Section: planets/planet_N.toml ──────────────────────────────────────

planet = 1               # contiguous from 1; lengths are per-planet
levels = 2               # [[level]] coverage must match EXACTLY
kits = ["template_kit"]
rooms = { base = 6, per_level = 2 }

[[level]]
relative = 1             # 1..=levels, each exactly once
enemies = ["template_enemy"]  # the COMPLETE list — nothing carries over
swarms = ["template_swarm"]   # optional

[[level]]
relative = 2
enemies = ["template_enemy"]

[[boss_slot]]
at = { relative = 2 }    # planet-relative level
boss = "template_boss"   # the def it fights as (size/stats on the def)
escorts = { enemy = "template_minion", trigger = "on_engage", count = 3 }
track = 1                # boss music index (boss_N.mp3)
reward = "hull_container" # or "consolation_pile"
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_template_is_a_valid_grammar() {
        // The scaffold must always LOAD: a schema change that invalidates
        // the template fails here, not in an author's copy-paste.
        let rendered = template();
        let mut parts = rendered.split(CUT);
        let enemies = parts.next().expect("enemies section");
        let kits = parts.next().expect("kits section");
        let planet = parts.next().expect("planet section");
        // The template links against the REAL model catalog: its example
        // enemies wear installed models, so the scaffold stays honest. Its
        // kit grid stands in for what the probe would derive.
        let template_grid = "[kits.template_kit]\ntile = 3.0\nstory = 3.0\n";
        if let Err(e) = super::super::load_from(
            enemies,
            kits,
            template_grid,
            super::super::MODELS_TOML,
            &[planet],
        ) {
            panic!("the template no longer links:\n{e}");
        }
    }

    #[test]
    #[ignore = "writes rosters/TEMPLATE.toml — `make build` runs it"]
    fn regenerate_template() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../rosters/TEMPLATE.toml");
        std::fs::write(path, template()).expect("write rosters/TEMPLATE.toml");
    }
}
