//! The typed level description — THE aggregate of every level attribute
//! (owner's design, 2026-07-05). One truth, one door: `LevelSpec::for_level`
//! is the only constructor; every build stage consumes `&LevelSpec`; no
//! consumer re-derives an attribute from a bare level number.
//!
//! Ownership rule: intrinsic facts live on the type (`EnemyType` stats,
//! `BossKind::minions`), scheduling and composition facts live HERE — the
//! roster schedule, the boss staging, the pitch, the paradigm. Joins that
//! need both (what does this level's boss drop, given the profile?) are
//! computed once, in the constructor.

use crate::asset_catalog::PanelSet;
use crate::boss::BossKind;
use crate::enemy_type::EnemyType;
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
    pub kind: BossKind,
    /// `Some(hull)` = the drop is the red container granting this hull;
    /// `None` = the consolation pile drops.
    pub hull_reward: Option<ShipType>,
    /// The consolation pile (kind, amount) — empty when the red container
    /// is staged.
    pub pile: Vec<(crate::currency::CurrencyKind, u32)>,
    /// Minions the boss fields (planet-scaled).
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
    /// The direct-spawn pool: every type the schedule admits by this level.
    pub roster: Vec<EnemyType>,
    /// The bestiary horizon: the roster closed over death-spawns.
    pub coverage: Vec<EnemyType>,
    pub boss: Option<BossStaging>,
    /// The planet-arrival interstitial, on planet boundaries only.
    pub banner: Option<(String, String)>,
}

/// WHEN each type joins the direct-spawn pool — the world design on one
/// screen (owner's call: scheduling is a level fact, not an enemy fact).
/// Cumulative: once admitted, never retired. Types absent here never enter
/// pools at all (the boss-duty reserves and the death-spawn-only drone).
const ROSTER_SCHEDULE: &[(u32, EnemyType)] = &[
    // ── Planet 1: the Quaternius fleet ──────────────────────────────
    (1, EnemyType::SentryDrone),
    (2, EnemyType::Bomber),
    (2, EnemyType::EyeDrone),
    (4, EnemyType::QuadShell),
    // ── Planet 2: the white sphere fleet mixes in ───────────────────
    (7, EnemyType::SphereGunner),
    (8, EnemyType::SphereStriker),
    (9, EnemyType::AlienTroop),
    (11, EnemyType::SphereCarrier),
];

impl LevelSpec {
    /// THE constructor — the one place level attributes are resolved.
    /// `unlocks` joins the profile in (red-container staging needs to know
    /// which hulls remain); it cannot change mid-level, so the spec is
    /// immutable for the level's lifetime.
    pub fn for_level(run_seed: Seed, level: u32, unlocks: &PermanentUnlocks) -> Self {
        let roster: Vec<EnemyType> = EnemyType::ALL
            .iter()
            .copied()
            .filter(|t| {
                ROSTER_SCHEDULE
                    .iter()
                    .any(|(entry, scheduled)| scheduled == t && *entry <= level)
            })
            .collect();

        // The bestiary horizon: the roster closed over death-spawns, in
        // ALL order, deduplicated.
        let mut seen = [false; EnemyType::ALL.len()];
        for t in &roster {
            seen[t.id() as usize] = true;
            if let Some((minion, _)) = t.death_spawn() {
                seen[minion.id() as usize] = true;
            }
        }
        let coverage = EnemyType::ALL
            .iter()
            .copied()
            .filter(|t| seen[t.id() as usize])
            .collect();

        let boss = crate::boss::boss_for_level(level).map(|kind| {
            let hull_reward = if crate::boss::is_planet_final(level) {
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
                kind,
                hull_reward,
                pile,
                adds: crate::boss::boss_adds(level),
            }
        });

        let planet = crate::planet::planet_of(level);
        Self {
            level,
            planet,
            pitch: Pitch::for_level(level),
            paradigm: if crate::planet::panel_world(level) {
                Paradigm::Panel(&crate::asset_catalog::PANEL_SET_VOL01)
            } else {
                Paradigm::Layered
            },
            room_budget: crate::generator::rooms_for_level(level),
            roster,
            coverage,
            boss,
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

    #[test]
    fn level_one_is_the_terrestrial_opening() {
        let spec = fresh(1);
        assert_eq!(spec.level, 1);
        assert_eq!(spec.planet, 1);
        assert_eq!((spec.pitch.tile, spec.pitch.story), (4.0, 5.0));
        assert_eq!(spec.paradigm, Paradigm::Layered);
        assert_eq!(spec.room_budget, 8);
        assert_eq!(spec.roster, vec![EnemyType::SentryDrone],
            "level 1 fields the sentry alone");
        assert_eq!(spec.boss, None, "no fight staged");
        assert_eq!(spec.banner, None, "planet 1 needs no introduction");
    }

    #[test]
    fn the_roster_schedule_admits_cumulatively() {
        // The old per-type tier pins, transposed onto the schedule table:
        // planet 1 is the Quaternius fleet; the white spheres mix in on
        // planet 2 without retiring anyone.
        assert!(!fresh(6).roster.contains(&EnemyType::SphereGunner),
            "no white spheres anywhere on planet 1");
        assert!(fresh(7).roster.contains(&EnemyType::SphereGunner));
        assert!(!fresh(7).roster.contains(&EnemyType::SphereStriker));
        assert!(fresh(8).roster.contains(&EnemyType::SphereStriker));
        assert!(!fresh(8).roster.contains(&EnemyType::AlienTroop));
        assert!(fresh(9).roster.contains(&EnemyType::AlienTroop));
        assert!(!fresh(10).roster.contains(&EnemyType::SphereCarrier));
        assert!(fresh(11).roster.contains(&EnemyType::SphereCarrier));
        for veteran in [EnemyType::SentryDrone, EnemyType::Bomber,
                        EnemyType::EyeDrone, EnemyType::QuadShell] {
            assert!(fresh(7).roster.contains(&veteran),
                "{veteran:?} keeps spawning on planet 2");
        }
    }

    #[test]
    fn the_reserves_never_enter_any_roster() {
        for level in 1..=24 {
            let spec = fresh(level);
            for reserve in [EnemyType::GunDrone, EnemyType::QuadOrb,
                            EnemyType::BossBrute, EnemyType::BossLatcher,
                            EnemyType::SpawnDrone] {
                assert!(!spec.roster.contains(&reserve),
                    "level {level}: {reserve:?} is not pool stock");
            }
        }
    }

    #[test]
    fn coverage_closes_the_roster_over_death_spawns() {
        assert!(!fresh(1).coverage.contains(&EnemyType::SpawnDrone),
            "level 1 cannot produce a SpawnDrone");
        assert!(fresh(2).coverage.contains(&EnemyType::SpawnDrone),
            "the EyeDrone's death spawn enters the bestiary horizon with it");
        // Coverage is a superset of the roster …
        let spec = fresh(11);
        for direct in &spec.roster {
            assert!(spec.coverage.contains(direct));
        }
        // … and is ALL-ordered and deduplicated (bestiary contract).
        let coverage = fresh(11).coverage.clone();
        let ids: Vec<i32> = coverage.iter().map(|t| t.id()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, ids);
    }

    #[test]
    fn boss_staging_resolves_kind_drop_and_hull_in_one_place() {
        let spec3 = fresh(3);
        let staging = spec3.boss.expect("rel-3 stages the mid-boss");
        assert_eq!(staging.kind, BossKind::Brute);
        assert_eq!(staging.hull_reward, None, "mid-bosses drop the pile");
        assert_eq!(staging.pile.len(), 3, "the pile of three stages with it");
        assert_eq!(staging.drop_count(), 4, "bound cache + the pile of three");
        assert_eq!(staging.adds, 3, "planet 1 fields the base trio");

        let spec6 = fresh(6);
        let staging = spec6.boss.expect("rel-6 stages the planet final");
        assert_eq!(staging.kind, BossKind::Latcher);
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
