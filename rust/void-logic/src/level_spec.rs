//! The typed level description — THE aggregate of every level attribute
//! (owner's design, 2026-07-05). One truth, one door: `LevelSpec::for_level`
//! is the only constructor; every build stage consumes `&LevelSpec`; no
//! consumer re-derives an attribute from a bare level number.
//!
//! Ownership rule: intrinsic facts live on the enemy defs, scheduling and
//! composition facts on the planet files — ALL in rosters/ (the grammar).
//! This constructor is the join point: it reads the linked roster plus the
//! profile and produces the immutable per-level value every stage consumes.

use crate::asset_catalog::PanelSet;
use crate::level_assembly::MinionTrigger;
use crate::roster::EnemyKey;
use crate::planet::Pitch;
use crate::seed::Seed;
use crate::ship_type::ShipType;
use crate::unlocks::PermanentUnlocks;

/// How a level's rooms are skinned: the megakit's layered wall stacks
/// (planet 1, themed per room), a panel pool over cubic cells (planet 2+),
/// or a FIXED pre-modeled environment installed whole (planet 3's
/// apartment) — the spec carries the environment's authored zones, per its
/// own doctrine ("no consumer re-derives an attribute").
#[derive(Debug, Clone, PartialEq)]
pub enum Paradigm {
    Layered,
    Panel(&'static PanelSet),
    Fixed(crate::roster::EnvironmentDef),
}

/// This level's staged fight, resolved against the profile at construction:
/// which boss, whether its drop is the red hull container (and which hull),
/// and how many caches the arena stays sealed for.
#[derive(Debug, Clone, PartialEq)]
pub struct BossStaging {
    /// The enemy def this boss fights as (declared on the boss slot).
    pub boss: EnemyKey,
    /// The escort kind and its rise trigger (declared on the boss slot).
    pub escorts: (EnemyKey, MinionTrigger),
    /// The fight's music track (never relooped — a fight outlasting it
    /// continues on combat stingers).
    pub track: String,
    /// `Some(hull)` = the drop is the red container granting this hull;
    /// `None` = the consolation pile drops.
    pub hull_reward: Option<ShipType>,
    /// The consolation pile (kind, amount) — empty when the red container
    /// is staged.
    pub pile: Vec<(crate::currency::CurrencyKind, u32)>,
    /// Minions the boss fields — the slot's declared escort count.
    pub adds: u8,
}

impl BossStaging {
    /// The complete drop set the arena stays sealed for
    /// (`BossFight::defeat` stages this count): the boss's bound cache,
    /// plus the pile when no red container is staged. Derived, never
    /// stored — one truth.
    pub fn drop_count(&self) -> u8 {
        1 + self.pile.len() as u8
    }
}

/// Everything a level IS, in one place. Retained for the level's lifetime
/// beside the `LevelGraph` (shell) — never reconstructed mid-level.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelSpec {
    pub level: u32,
    pub planet: u32,
    pub pitch: Pitch,
    pub paradigm: Paradigm,
    /// Target room count for generation.
    pub room_budget: usize,
    /// The direct-spawn pool: exactly what this level's declared list fields.
    pub roster: Vec<EnemyKey>,
    /// The bestiary horizon: the roster closed over bound minions.
    pub coverage: Vec<EnemyKey>,
    pub boss: Option<BossStaging>,
    /// The room-seal elite this level fields, when its declared list names a
    /// `miniboss` def: the manifest places it in ONE seed-chosen room (never
    /// rolled), whose connectors seal on entry and re-open on its death.
    /// Always `None` on staged-boss levels — one seal per level, and the
    /// staged fight owns it.
    pub miniboss: Option<EnemyKey>,
    /// The level's loopable background music.
    pub background: String,
    /// The planet-arrival interstitial, on planet boundaries only.
    pub banner: Option<(String, String)>,
}

impl LevelSpec {
    /// THE constructor — the one place level attributes are resolved: the
    /// linked roster grammar (WHAT exists and WHERE it appears) joined with
    /// the profile (red-container staging needs to know which hulls
    /// remain). Immutable for the level's lifetime. `grammar` is the linked
    /// grammar to resolve against — production passes the [`roster()`]
    /// singleton; tests may pass a fixture, so tuning the shipped TOML can
    /// never break them.
    pub fn for_level(
        grammar: &crate::roster::Roster,
        run_seed: Seed,
        level: u32,
        unlocks: &PermanentUnlocks,
    ) -> Self {
        let roster: Vec<EnemyKey> = grammar.roster_for_level(level);

        // The bestiary horizon: the roster closed over bound minions, in
        // declaration order, deduplicated.
        let mut seen = std::collections::HashSet::new();
        for key in &roster {
            seen.insert(*key);
            for m in &grammar.enemy(*key).minions {
                seen.insert(m.enemy);
            }
        }
        let coverage: Vec<EnemyKey> = grammar
            .enemy_keys()
            .filter(|k| seen.contains(k))
            .collect();

        let boss = grammar.boss_slot_for_level(level).map(|slot| {
            // The red container hangs off the slot's DECLARED reward
            // policy — a hull while unowned ones remain, else the pile.
            let hull_reward = if slot.reward
                == crate::roster::schema::BossRewardPolicy::HullContainer
            {
                crate::boss::roll_hull_reward(run_seed, level, unlocks)
            } else {
                None
            };
            let pile = if hull_reward.is_some() {
                Vec::new()
            } else {
                crate::boss::consolation_pile(level)
            };
            BossStaging {
                boss: slot.boss,
                escorts: (slot.escorts.enemy, slot.escorts.trigger),
                track: crate::audio_catalog::boss_track(slot.track as u32),
                hull_reward,
                pile,
                adds: slot.escorts.count,
            }
        });

        // The room-seal elite: a `miniboss` def in the declared list, placed
        // (never rolled) by the manifest. A staged-boss level never also
        // fields one — the level has ONE seal, and the staged fight owns it.
        let miniboss = if boss.is_none() {
            roster
                .iter()
                .copied()
                .find(|k| grammar.enemy(*k).behavior.miniboss)
        } else {
            None
        };

        let planet_def = grammar.planet_for_level(level);
        Self {
            level,
            planet: grammar.planet_number_and_relative(level).0,
            pitch: grammar.pitch_for_level(level),
            paradigm: if let Some(env) = grammar.environment_for_level(level) {
                Paradigm::Fixed(env.clone())
            } else if grammar.panel_world(level) {
                Paradigm::Panel(&crate::asset_catalog::PANEL_SET_VOL01)
            } else {
                Paradigm::Layered
            },
            room_budget: (planet_def.rooms_base + level * planet_def.rooms_per_level)
                as usize,
            roster,
            coverage,
            boss,
            miniboss,
            background: crate::audio_catalog::level_background(level),
            banner: crate::planet::arrival_banner(level),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(level: u32) -> LevelSpec {
        LevelSpec::for_level(crate::roster::roster(), Seed::new(1), level, &PermanentUnlocks::new())
    }

    #[test]
    fn the_reserves_never_enter_any_roster() {
        // Anything the grammar keeps off the direct line (spawns_directly
        // = false) stays out of every roster — derived from the grammar,
        // never a named list (feedback 2026-07-06).
        use crate::roster::roster;
        for level in 1..=24 {
            let spec = fresh(level);
            for key in roster().enemy_keys() {
                if !roster().enemy(key).spawns_directly {
                    assert!(!spec.roster.contains(&key),
                        "level {level}: {} is not pool stock", roster().enemy(key).key);
                }
            }
        }
    }

    #[test]
    fn coverage_closes_the_roster_over_death_spawns() {
        // The spec's coverage is its roster closed over declared minions —
        // the bestiary horizon. Derived from the grammar, never from named
        // defs (feedback 2026-07-06). Order is just whatever the file
        // declares — not a contract; only membership and no-duplicates are.
        use crate::roster::roster;
        for level in 1..=12u32 {
            let spec = fresh(level);
            for direct in &spec.roster {
                assert!(spec.coverage.contains(direct),
                    "level {level}: the roster is inside its own horizon");
                for d in &roster().enemy(*direct).minions {
                    assert!(spec.coverage.contains(&d.enemy),
                        "level {level}: a roster member's declared minion joins the horizon");
                }
            }
            let unique: std::collections::HashSet<_> = spec.coverage.iter().collect();
            assert_eq!(unique.len(), spec.coverage.len(),
                "level {level}: coverage lists each enemy once");
        }
    }

    #[test]
    fn boss_staging_resolves_drop_and_hull_from_the_slot() {
        // The staging RESOLUTION, derived by scanning for staged fights —
        // never which boss or which level: the drop is the bound cache plus
        // whatever pile stages; a hull container rides a slot that asks for it
        // and replaces the pile; a pile fight drops more than the lone bound
        // cache. Retuning the campaign can't move this.
        let mut saw_pile = false;
        let mut saw_container = false;
        for level in 1..=24u32 {
            let Some(staging) = fresh(level).boss else { continue };
            assert_eq!(staging.drop_count() as usize, 1 + staging.pile.len(),
                "level {level}: the drop is the bound cache plus the pile");
            assert!(!staging.track.is_empty(), "level {level}: a staged fight names its music");
            match staging.hull_reward {
                Some(_) => {
                    saw_container = true;
                    assert!(staging.pile.is_empty(), "level {level}: the container replaces the pile");
                    assert_eq!(staging.drop_count(), 1, "level {level}: the container is the whole drop");
                }
                None => {
                    saw_pile = true;
                    assert!(!staging.pile.is_empty(),
                        "level {level}: a pile fight drops more than the bound cache");
                }
            }
        }
        assert!(saw_pile && saw_container,
            "the grammar exercises both a pile fight and a hull-container fight");
    }

    #[test]
    fn a_complete_fleet_downgrades_the_container_final_to_a_pile() {
        // The reward MECHANISM, derived: a fight that would drop a hull
        // container drops the pile instead once every hull is owned. The
        // container fight is FOUND, never pinned to a level.
        let mut full = PermanentUnlocks::new();
        for ship in [ShipType::Talon, ShipType::Hive, ShipType::Reaver] {
            full.grant(crate::unlocks::Unlock::Ship(ship));
        }
        let container_level = (1..=24u32)
            .find(|l| fresh(*l).boss.is_some_and(|b| b.hull_reward.is_some()))
            .expect("the campaign stages a hull-container fight");
        let downgraded = LevelSpec::for_level(crate::roster::roster(), Seed::new(1), container_level, &full)
            .boss
            .expect("the fight still stages");
        assert_eq!(downgraded.hull_reward, None, "no hulls left to win");
        assert!(downgraded.drop_count() > 1, "so the pile drops instead");
    }

    #[test]
    fn the_spec_is_a_pure_function_of_its_inputs() {
        let a = fresh(6);
        let b = fresh(6);
        assert_eq!(a, b, "same inputs, same level — bit for bit");
    }

    #[test]
    fn fixed_levels_resolve_the_fixed_paradigm() {
        // Every level of a fixed planet resolves Paradigm::Fixed carrying
        // the SAME environment; pitch is the declared scale; the room
        // budget is the authored zone count. Derived by walking the fixture
        // planet's declared length — never level-number-pinned.
        let grammar = crate::test_fixtures::fixed_fixture_grammar();
        let levels = grammar.planet_for_level(1).levels;
        assert!(levels > 1, "the fixture exercises more than one level");
        for level in 1..=levels {
            let spec =
                LevelSpec::for_level(&grammar, Seed::new(1), level, &PermanentUnlocks::new());
            let Paradigm::Fixed(env) = &spec.paradigm else {
                panic!("level {level}: a fixed planet resolves the fixed paradigm");
            };
            assert_eq!(env.key, "fx_house", "one environment across the planet");
            assert_eq!(
                spec.room_budget,
                env.zones.len(),
                "level {level}: the room budget is the authored zone count"
            );
            let kit_scale = grammar.pitch_for_level(level);
            assert_eq!(
                (spec.pitch.tile, spec.pitch.story),
                (kit_scale.tile, kit_scale.story),
                "level {level}: pitch is the kit's declared scale"
            );
        }
    }

    #[test]
    fn fixed_levels_always_stage_a_boss_and_never_a_miniboss() {
        // The staged fight owns the level's seal — linker-guaranteed on
        // fixed planets, resolved here.
        let grammar = crate::test_fixtures::fixed_fixture_grammar();
        for level in 1..=grammar.planet_for_level(1).levels {
            let spec =
                LevelSpec::for_level(&grammar, Seed::new(1), level, &PermanentUnlocks::new());
            assert!(spec.boss.is_some(), "level {level}: the fight is staged");
            assert_eq!(spec.miniboss, None, "level {level}: no second seal");
        }
    }
}
