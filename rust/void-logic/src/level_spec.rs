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
use crate::roster::EnemyId;
use crate::planet::Pitch;
use crate::seed::Seed;
use crate::ship_type::ShipType;
use crate::unlocks::PermanentUnlocks;

/// How a level's rooms are skinned: the megakit's layered wall stacks
/// (planet 1, themed per room) or a panel pool over cubic cells (planet 2+).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Paradigm {
    Layered,
    Panel(&'static PanelSet),
}

/// This level's staged fight, resolved against the profile at construction:
/// which boss, whether its drop is the red hull container (and which hull),
/// and how many caches the arena stays sealed for.
#[derive(Debug, Clone, PartialEq)]
pub struct BossStaging {
    /// The enemy def this boss fights as (declared on the boss slot).
    pub boss: EnemyId,
    /// The escort kind and its rise trigger (declared on the boss slot).
    pub escorts: (EnemyId, MinionTrigger),
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
    pub roster: Vec<EnemyId>,
    /// The bestiary horizon: the roster closed over bound minions.
    pub coverage: Vec<EnemyId>,
    pub boss: Option<BossStaging>,
    /// The level's loopable background music.
    pub background: String,
    /// The planet-arrival interstitial, on planet boundaries only.
    pub banner: Option<(String, String)>,
}

impl LevelSpec {
    /// THE constructor — the one place level attributes are resolved: the
    /// linked roster grammar (WHAT exists and WHERE it appears) joined with
    /// the profile (red-container staging needs to know which hulls
    /// remain). Immutable for the level's lifetime.
    pub fn for_level(run_seed: Seed, level: u32, unlocks: &PermanentUnlocks) -> Self {
        let grammar = crate::roster::roster();
        let roster: Vec<EnemyId> = grammar.roster_for_level(level);

        // The bestiary horizon: the roster closed over bound minions, in
        // declaration order, deduplicated.
        let mut seen = vec![false; grammar.enemies.len()];
        for id in &roster {
            seen[id.0] = true;
            for m in &grammar.enemy(*id).minions {
                seen[m.enemy.0] = true;
            }
        }
        let coverage: Vec<EnemyId> = grammar
            .enemy_ids()
            .filter(|id| seen[id.0])
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

        let planet_def = grammar.planet_for_level(level);
        Self {
            level,
            planet: crate::planet::planet_of(level),
            pitch: Pitch::for_level(level),
            paradigm: if crate::planet::panel_world(level) {
                Paradigm::Panel(&crate::asset_catalog::PANEL_SET_VOL01)
            } else {
                Paradigm::Layered
            },
            room_budget: (planet_def.rooms_base + level * planet_def.rooms_per_level)
                as usize,
            roster,
            coverage,
            boss,
            background: crate::audio_catalog::level_background(level),
            banner: crate::planet::arrival_banner(level),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_catalog::PANEL_SET_VOL01;

    fn fresh(level: u32) -> LevelSpec {
        LevelSpec::for_level(Seed::new(1), level, &PermanentUnlocks::new())
    }

    fn eid(key: &str) -> crate::roster::EnemyId {
        crate::roster::roster().enemy_by_key(key).expect(key)
    }

    #[test]
    fn level_one_is_the_terrestrial_opening() {
        let spec = fresh(1);
        assert_eq!(spec.level, 1);
        assert_eq!(spec.planet, 1);
        assert_eq!((spec.pitch.tile, spec.pitch.story), (4.0, 5.0));
        assert_eq!(spec.paradigm, Paradigm::Layered);
        assert_eq!(spec.room_budget, 8);
        assert_eq!(spec.roster, vec![eid("sentry_drone")],
            "level 1 fields the sentry alone");
        assert_eq!(spec.boss, None, "no fight staged");
        assert!(spec.background.ends_with("level_01.mp3"),
            "each level carries its own background");
        assert_eq!(spec.banner, None, "planet 1 needs no introduction");
    }

    #[test]
    fn rosters_are_planet_scoped_not_mixed() {
        // Owner's correction (playtest 2026-07-05, twice): planet 2 fields
        // the WHITE SPHERE fleet — the Quaternius machines belong to planet
        // 1 and retire at its final boss. Replacement, never mix-in.
        assert!(!fresh(6).roster.contains(&eid("sphere_gunner")),
            "no white spheres anywhere on planet 1");
        assert!(fresh(7).roster.contains(&eid("sphere_gunner")));
        assert!(!fresh(7).roster.contains(&eid("sphere_striker")));
        assert!(fresh(8).roster.contains(&eid("sphere_striker")));
        assert!(!fresh(8).roster.contains(&eid("alien_troop")));
        assert!(fresh(9).roster.contains(&eid("alien_troop")));
        assert!(!fresh(10).roster.contains(&eid("sphere_carrier")));
        assert!(fresh(11).roster.contains(&eid("sphere_carrier")));
        for veteran in [eid("sentry_drone"), eid("bomber"),
                        eid("eye_drone"), eid("quad_shell")] {
            assert!(!fresh(7).roster.contains(&veteran),
                "{veteran:?} retired with planet 1");
        }
        // Planets past the sphere fleet's home keep fielding it until new
        // kits arrive — a planet is never enemy-less.
        assert!(fresh(13).roster.contains(&eid("sphere_gunner")),
            "planet 3 inherits the newest fleet");
        assert!(!fresh(13).roster.contains(&eid("sentry_drone")));
    }

    #[test]
    fn the_reserves_never_enter_any_roster() {
        for level in 1..=24 {
            let spec = fresh(level);
            for reserve in [eid("gun_drone"), eid("quad_orb"),
                            eid("boss_brute"), eid("boss_latcher"),
                            eid("spawn_drone")] {
                assert!(!spec.roster.contains(&reserve),
                    "level {level}: {reserve:?} is not pool stock");
            }
        }
    }

    #[test]
    fn coverage_closes_the_roster_over_death_spawns() {
        assert!(!fresh(2).coverage.contains(&eid("spawn_drone")),
            "level 2 cannot produce a SpawnDrone");
        assert!(fresh(3).coverage.contains(&eid("spawn_drone")),
            "the EyeDrone's death spawn enters the bestiary horizon with it \
             (the EyeDrone arrives at relative 3, owner's schedule)");
        // Coverage is a superset of the roster …
        let spec = fresh(11);
        for direct in &spec.roster {
            assert!(spec.coverage.contains(direct));
        }
        // … and is declaration-ordered and deduplicated (bestiary contract).
        let coverage = fresh(11).coverage.clone();
        let ids: Vec<usize> = coverage.iter().map(|id| id.0).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, ids);
    }

    #[test]
    fn boss_staging_resolves_kind_drop_and_hull_in_one_place() {
        let spec3 = fresh(3);
        let staging = spec3.boss.expect("rel-3 stages the mid-boss");
        assert_eq!(staging.boss, eid("boss_brute"));
        assert!(staging.track.ends_with("boss_1.mp3"), "mid-boss music");
        assert_eq!(staging.hull_reward, None, "mid-bosses drop the pile");
        assert_eq!(staging.pile.len(), 3, "the pile of three stages with it");
        assert_eq!(staging.drop_count(), 4, "bound cache + the pile of three");
        assert_eq!(staging.adds, 3, "planet 1's slot declares a trio");

        let spec6 = fresh(6);
        let staging = spec6.boss.expect("rel-6 stages the planet final");
        assert_eq!(staging.boss, eid("boss_latcher"));
        assert!(staging.track.ends_with("boss_2.mp3"), "planet-final music");
        assert!(staging.hull_reward.is_some(),
            "a fresh profile's planet final stages the red container");
        assert!(staging.pile.is_empty(), "the container replaces the pile");
        assert_eq!(staging.drop_count(), 1, "the container is the whole drop");

        assert_eq!(fresh(5).boss, None);
    }

    #[test]
    fn a_complete_fleet_downgrades_the_final_to_the_pile() {
        let mut full = PermanentUnlocks::new();
        for ship in [ShipType::Talon, ShipType::Hive, ShipType::Reaver] {
            full.grant(crate::unlocks::Unlock::Ship(ship));
        }
        let spec = LevelSpec::for_level(Seed::new(1), 6, &full);
        let staging = spec.boss.expect("the fight still stages");
        assert_eq!(staging.hull_reward, None, "no hulls left to win");
        assert!(staging.drop_count() > 1, "so the pile drops instead");
    }

    #[test]
    fn planet_two_is_the_cubic_panel_world_with_its_banner() {
        let spec = fresh(7);
        assert_eq!(spec.planet, 2);
        assert_eq!((spec.pitch.tile, spec.pitch.story), (3.0, 3.0));
        assert_eq!(spec.paradigm, Paradigm::Panel(&PANEL_SET_VOL01));
        let (title, flavor) = spec.banner.expect("planet 2 announces itself");
        assert!(title.contains("PLANET 2"));
        assert!(!flavor.is_empty());
        assert_eq!(fresh(8).banner, None, "mid-planet entries are ordinary");
    }

    #[test]
    fn the_spec_is_a_pure_function_of_its_inputs() {
        let a = fresh(6);
        let b = fresh(6);
        assert_eq!(a, b, "same inputs, same level — bit for bit");
    }
}
