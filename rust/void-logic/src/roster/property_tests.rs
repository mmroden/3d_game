//! Property tests over the roster grammar (owner 2026-07-05: "show that all
//! permutations are actually buildable" and hunt "corners in this grammar we
//! haven't thought about").
//!
//! A seeded generator emits random VALID grammars as TOML text (exercising
//! the full serde + link path, exactly what hand-authored files travel).
//! Three properties:
//!   1. Every generated grammar parses and links.
//!   2. Every linked grammar is TOTAL: for any level, the roster is
//!      non-empty, the planet resolves, boss slots stay in range, and every
//!      curve evaluates finite — the queries a level build will make.
//!   3. Every mutation from the violation catalog is REJECTED, with the
//!      error naming the culprit — one broken rule can't hide behind a
//!      generated grammar's noise.
//!
//! Designing the generator surfaced three corners that became linker rules
//! before this file first ran: swarm members must spawn directly, minions
//! must not nest (the engine binds one level deep), and a slot's boss must
//! not declare one-shot broods beside the slot's escorts (timed emitters
//! are the def's own character and may ride it — owner 2026-07-05).

use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};

use super::load_from;

const SEEDS: u64 = 48;

struct Generated {
    enemies_toml: String,
    kits_toml: String,
    kit_grids_toml: String,
    models_toml: String,
    planet_tomls: Vec<String>,
    /// Facts the mutation catalog needs: keys of directly-spawning enemies,
    /// keys of the rest, and per-planet declared lengths.
    spawner_keys: Vec<String>,
    other_keys: Vec<String>,
    planet_levels: Vec<u32>,
}

fn generate(seed: u64) -> Generated {
    let mut rng = SmallRng::seed_from_u64(seed);

    // ── Enemies: 2..=12, at least one direct spawner ──
    let enemy_count = rng.random_range(2..=12usize);
    let archetypes = ["shooter", "kiter", "swarmer", "tank", "bomber"];
    let mut spawner_keys = Vec::new();
    let mut other_keys = Vec::new();
    // The generated model catalog: one entry per enemy, referenced by key
    // (the real catalog is probed from disk; the shape is identical).
    let mut models_toml = String::from("[models]\n");
    for i in 0..enemy_count {
        models_toml.push_str(&format!("m{i} = \"res://generated/m{i}.glb\"\n"));
    }
    let mut enemies_toml = String::from(
        "[curves.flat]\nkind = \"flat\"\n\n\
         [curves.main]\nkind = \"ramp\"\nat_level_1 = 0.5\nat_peak = 1.2\npeak_level = 8\n\n\
         [scaling_defaults]\nspeed = \"main\"\ncooldown = \"main\"\nhp = \"flat\"\n\
         damage = \"flat\"\ndetection = \"flat\"\nattack_range = \"main\"\n\n",
    );
    for i in 0..enemy_count {
        let key = format!("enemy_{i}");
        // The first enemy always spawns directly (rosters need at least one);
        // later ones flip a coin.
        let spawns = i == 0 || rng.random_range(0..4u32) > 0;
        if spawns {
            spawner_keys.push(key.clone());
        } else {
            other_keys.push(key.clone());
        }
        enemies_toml.push_str(&format!(
            "[[enemy]]\nkey = \"{key}\"\nname = \"Enemy {i}\"\n\
             blurb = \"Generated hazard {i}.\"\n\
             model = \"m{i}\"\nsize = {:.1}\n\
             yaw_offset_deg = {}\nai = \"{}\"\nreward = {}\n",
            rng.random_range(0.5..4.0f32),
            if rng.random_range(0..2u32) == 0 { 0 } else { 180 },
            archetypes[rng.random_range(0..archetypes.len())],
            rng.random_range(100..20_000u32),
        ));
        if !spawns {
            enemies_toml.push_str("spawns_directly = false\n");
        }
        // Minions: only enemies past the first half may CARRY them, and only
        // enemies from the first half may BE them — structurally acyclic and
        // never nested (carriers and targets are disjoint).
        if i >= enemy_count / 2 && enemy_count >= 2 && rng.random_range(0..3u32) == 0 {
            let target = rng.random_range(0..enemy_count / 2);
            if target != i {
                // One-shot, or a timed emitter with its ring cap — the full
                // trigger vocabulary gets generated coverage.
                let count = rng.random_range(1..4u32);
                let entry = match rng.random_range(0..3u32) {
                    0 => format!(
                        "{{ enemy = \"enemy_{target}\", count = {count}, trigger = \"on_death\" }}"
                    ),
                    1 => format!(
                        "{{ enemy = \"enemy_{target}\", count = {count}, trigger = \"on_engage\" }}"
                    ),
                    _ => format!(
                        "{{ enemy = \"enemy_{target}\", count = {count}, \
                         trigger = {{ every_seconds = {:.1} }}, cap = {} }}",
                        rng.random_range(2.0..30.0f32),
                        count + rng.random_range(0..6u32),
                    ),
                };
                enemies_toml.push_str(&format!("minions = [{entry}]\n"));
            }
        }
        enemies_toml.push_str(&format!(
            "[enemy.stats]\nhp = {:.1}\nspeed = {:.1}\ndamage = {:.1}\n\
             detection = {:.1}\nattack_range = {:.1}\ncooldown = {:.1}\n\n",
            rng.random_range(1.0..100.0f32),
            rng.random_range(4.0..15.0f32),
            rng.random_range(1.0..20.0f32),
            rng.random_range(15.0..50.0f32),
            rng.random_range(2.0..15.0f32),
            rng.random_range(0.5..2.0f32),
        ));
    }

    // ── Swarms: 0..=2, members drawn from the direct spawners ──
    let swarm_count = rng.random_range(0..=2usize);
    let mut swarm_keys = Vec::new();
    for s in 0..swarm_count {
        let key = format!("swarm_{s}");
        let members: Vec<String> = (0..rng.random_range(1..=3usize))
            .map(|_| {
                let m = &spawner_keys[rng.random_range(0..spawner_keys.len())];
                format!("{{ enemy = \"{m}\", count = {} }}", rng.random_range(1..3u32))
            })
            .collect();
        enemies_toml.push_str(&format!(
            "[[swarm]]\nkey = \"{key}\"\nmembers = [{}]\n\n",
            members.join(", ")
        ));
        swarm_keys.push(key);
    }

    // ── Kits: the authored manifest plus the probe-derived grid file ──
    let kit_count = rng.random_range(1..=3usize);
    let mut kits_toml = String::new();
    let mut kit_grids_toml = String::new();
    for k in 0..kit_count {
        let cubic = rng.random_range(0..2u32) == 0;
        let tile = rng.random_range(2.0..6.0f32);
        kits_toml.push_str(&format!(
            "[kits.kit_{k}]\nparadigm = \"{}\"\n\
             install_dir = \"godot/addons/kit_{k}\"\n\n",
            if cubic { "panel" } else { "layered" },
        ));
        kit_grids_toml.push_str(&format!(
            "[kits.kit_{k}]\ntile = {tile:.1}\nstory = {:.1}\n\n",
            if cubic { tile } else { tile + rng.random_range(0.5..2.0f32) },
        ));
    }

    // ── Planets: 1..=4, contiguous, random lengths, full coverage ──
    let planet_count = rng.random_range(1..=4usize);
    let mut planet_tomls = Vec::new();
    let mut planet_levels = Vec::new();
    for p in 0..planet_count {
        let levels = rng.random_range(1..=9u32);
        planet_levels.push(levels);
        let mut t = format!(
            "planet = {}\nlevels = {levels}\nkits = [\"kit_{}\"]\n\
             rooms = {{ base = {}, per_level = {} }}\n\n",
            p + 1,
            rng.random_range(0..kit_count),
            rng.random_range(4..10u32),
            rng.random_range(1..4u32),
        );
        for relative in 1..=levels {
            let mut enemies: Vec<String> = (0..rng.random_range(1..=4usize))
                .map(|_| format!("\"{}\"", spawner_keys[rng.random_range(0..spawner_keys.len())]))
                .collect();
            enemies.dedup();
            let swarms: Vec<String> = swarm_keys
                .iter()
                .filter(|_| rng.random_range(0..3u32) == 0)
                .map(|k| format!("\"{k}\""))
                .collect();
            t.push_str(&format!(
                "[[level]]\nrelative = {relative}\nenemies = [{}]\n",
                enemies.join(", ")
            ));
            if !swarms.is_empty() {
                t.push_str(&format!("swarms = [{}]\n", swarms.join(", ")));
            }
            t.push('\n');
        }
        // Boss slots at distinct relatives; bosses/escorts = minion-less
        // enemies (the generator's first half never carries minions).
        let mut used: Vec<u32> = Vec::new();
        for _ in 0..rng.random_range(0..=2usize) {
            let rel = rng.random_range(1..=levels);
            if used.contains(&rel) {
                continue;
            }
            used.push(rel);
            let pool_max = (enemy_count / 2).max(1);
            t.push_str(&format!(
                "[[boss_slot]]\nat = {{ relative = {rel} }}\nboss = \"enemy_{}\"\n\
                 escorts = {{ enemy = \"enemy_{}\", trigger = \"on_engage\", count = {} }}\n\
                 track = {}\nreward = \"{}\"\n\n",
                rng.random_range(0..pool_max),
                rng.random_range(0..pool_max),
                rng.random_range(1..=6u8),
                rng.random_range(1..=6u32),
                if rng.random_range(0..2u32) == 0 { "consolation_pile" } else { "hull_container" },
            ));
        }
        planet_tomls.push(t);
    }

    Generated {
        enemies_toml,
        kits_toml,
        kit_grids_toml,
        models_toml,
        planet_tomls,
        spawner_keys,
        other_keys,
        planet_levels,
    }
}

fn load_generated(g: &Generated) -> Result<super::Roster, String> {
    let planets: Vec<&str> = g.planet_tomls.iter().map(|s| s.as_str()).collect();
    load_from(
        &g.enemies_toml,
        &g.kits_toml,
        &g.kit_grids_toml,
        &g.models_toml,
        &planets,
    )
}

#[test]
fn every_generated_grammar_links_and_is_total() {
    for seed in 0..SEEDS {
        let g = generate(seed);
        let roster = match load_generated(&g) {
            Ok(r) => r,
            Err(e) => panic!(
                "seed {seed}: a generated-valid grammar failed to link:\n{e}\n\
                 --- enemies.toml ---\n{}\n--- planets ---\n{}",
                g.enemies_toml,
                g.planet_tomls.join("\n====\n")
            ),
        };
        // Totality: the queries a level build makes never panic and never
        // come back empty, far past the declared planets.
        let declared: u32 = g.planet_levels.iter().sum();
        for level in 1..=declared * 3 {
            let ids = roster.roster_for_level(level);
            assert!(
                !ids.is_empty(),
                "seed {seed}: level {level} fields nothing"
            );
            for id in &ids {
                let def = roster.enemy(*id);
                assert!(def.spawns_directly, "seed {seed}: roster lists a non-spawner");
                for (curve, stat) in [
                    (def.scaling.speed, "speed"),
                    (def.scaling.cooldown, "cooldown"),
                    (def.scaling.hp, "hp"),
                ] {
                    let v = roster.curve(curve).eval(level);
                    assert!(
                        v.is_finite() && v > 0.0,
                        "seed {seed}: {stat} curve degenerate at level {level}: {v}"
                    );
                }
            }
            let def = roster.planet_for_level(level);
            if let Some(slot) = roster.boss_slot_for_level(level) {
                assert!(
                    slot.at_relative >= 1 && slot.at_relative <= def.levels,
                    "seed {seed}: boss slot out of the planet band"
                );
            }
            assert!(!def.kits.is_empty(), "seed {seed}: planet without kits");
        }
    }
}

/// One targeted corruption per rule in the violation catalog: every mutation
/// must be rejected, and the error must name the culprit.
#[test]
fn every_cataloged_violation_is_rejected() {
    for seed in 0..SEEDS {
        let g = generate(seed);
        let mut mutations: Vec<(&str, Generated, &str)> = vec![
            ("duplicate key", duplicate_key(&g), "duplicate enemy key"),
            ("dangling roster ref", dangle_roster_ref(&g), "unknown enemy"),
            ("coverage gap", drop_level_block(&g), "has no"),
            ("declared length overrun", extend_declared_levels(&g), "has no"),
            ("planet numbering gap", renumber_first_planet(&g), "contiguous"),
        ];
        if let Some(m) = zero_track(&g) {
            mutations.push(("zero track", m, "track"));
        }
        for (name, mutated, expect) in mutations {
            match load_generated(&mutated) {
                Err(e) => assert!(
                    e.contains(expect),
                    "seed {seed}, {name}: error must mention '{expect}':\n{e}"
                ),
                Ok(_) => panic!("seed {seed}: {name} was accepted"),
            }
        }
        // Non-spawner listed in a roster — only when the grammar has one.
        if let Some(bad) = g.other_keys.first() {
            let mut m = clone_generated(&g);
            replace_first_roster_key(&mut m.planet_tomls[0], bad);
            match load_generated(&m) {
                Err(e) => assert!(
                    e.contains("never spawns directly"),
                    "seed {seed}: non-spawner roster error missing:\n{e}"
                ),
                Ok(_) => panic!("seed {seed}: non-spawner in roster was accepted"),
            }
        }
    }
}

fn clone_generated(g: &Generated) -> Generated {
    Generated {
        enemies_toml: g.enemies_toml.clone(),
        kits_toml: g.kits_toml.clone(),
        kit_grids_toml: g.kit_grids_toml.clone(),
        models_toml: g.models_toml.clone(),
        planet_tomls: g.planet_tomls.clone(),
        spawner_keys: g.spawner_keys.clone(),
        other_keys: g.other_keys.clone(),
        planet_levels: g.planet_levels.clone(),
    }
}

fn duplicate_key(g: &Generated) -> Generated {
    let mut m = clone_generated(g);
    // Rewrite enemy_1's key to enemy_0's — the key is the sole identity now,
    // so two blocks sharing it is the one collision the linker must catch.
    m.enemies_toml = m
        .enemies_toml
        .replacen("key = \"enemy_1\"", "key = \"enemy_0\"", 1);
    m
}

/// Rewrite the FIRST key of the first roster list — structural, so it can
/// never miss (seed 17 taught us: a key-targeted replace can be a no-op
/// when that enemy happens not to be rostered on planet 1).
fn dangle_roster_ref(g: &Generated) -> Generated {
    let mut m = clone_generated(g);
    replace_first_roster_key(&mut m.planet_tomls[0], "no_such_enemy");
    m
}

fn replace_first_roster_key(planet_toml: &mut String, with: &str) {
    let open = "enemies = [\"";
    let at = planet_toml.find(open).expect("generator layout") + open.len();
    let end = at + planet_toml[at..].find('"').expect("closing quote");
    planet_toml.replace_range(at..end, with);
}

fn drop_level_block(g: &Generated) -> Generated {
    let mut m = clone_generated(g);
    // Remove the relative-1 roster block (every planet has one).
    let start = m.planet_tomls[0].find("[[level]]").expect("generator layout");
    let rest = &m.planet_tomls[0][start + 1..];
    let end = rest
        .find("[[level]]")
        .or_else(|| rest.find("[[boss_slot]]"))
        .map(|i| start + 1 + i)
        .unwrap_or(m.planet_tomls[0].len());
    m.planet_tomls[0].replace_range(start..end, "");
    m
}

fn extend_declared_levels(g: &Generated) -> Generated {
    let mut m = clone_generated(g);
    let levels = g.planet_levels[0];
    m.planet_tomls[0] = m.planet_tomls[0].replacen(
        &format!("levels = {levels}"),
        &format!("levels = {}", levels + 1),
        1,
    );
    m
}

fn renumber_first_planet(g: &Generated) -> Generated {
    let mut m = clone_generated(g);
    m.planet_tomls[0] = m.planet_tomls[0].replacen("planet = 1", "planet = 9", 1);
    m
}

/// `None` when the generated grammar staged no boss slots (nothing to
/// corrupt) — the other mutations still cover the seed.
fn zero_track(g: &Generated) -> Option<Generated> {
    let mut m = clone_generated(g);
    for t in &mut m.planet_tomls {
        if let Some(at) = t.find("track = ") {
            // Track values are 1..=6 single digits in generator layout.
            t.replace_range(at..at + "track = X".len(), "track = 0");
            return Some(m);
        }
    }
    None
}
