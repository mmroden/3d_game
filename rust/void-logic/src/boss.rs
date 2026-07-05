//! Boss scheduling. Every planet stages two set-piece fights on a fixed
//! rhythm (owner's call 2026-07-04, superseding the chance-based roll):
//! the mid-planet boss drops a blue/green consolation pile, the
//! planet-final boss drops the red hull container and its portal carries
//! the player to the next planet.

use crate::planet::planet_relative;

/// The two staged fights. Brute = the evil_mech_03 bruiser (death-spawns
/// drones); Latcher = the evil_mech_01 drainer with a circling escort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BossKind {
    Brute,
    Latcher,
}

/// Planet-relative level of the mid-planet boss (consolation pile).
pub const MID_BOSS_AT: u32 = 3;
/// Planet-relative level of the planet-final boss (red hull container +
/// planet transition). Equals PLANET_LENGTH by design: the boss IS the exit.
pub const FINAL_BOSS_AT: u32 = 6;

/// Which boss, if any, a level stages. Deterministic — no seed: the rhythm
/// itself is the design (players learn to bank blues before a rel-3 level).
pub fn boss_for_level(level: u32) -> Option<BossKind> {
    match planet_relative(level) {
        MID_BOSS_AT => Some(BossKind::Brute),
        FINAL_BOSS_AT => Some(BossKind::Latcher),
        _ => None,
    }
}

/// Whether this level's boss guards the planet exit (drops the red
/// container while unowned hulls remain; the portal beyond changes planet).
pub fn is_planet_final(level: u32) -> bool {
    planet_relative(level) == FINAL_BOSS_AT
}

impl BossKind {
    /// The roster entry staged for this boss (stats, model, archetype,
    /// minion kind — all live on `EnemyType` like every other enemy).
    pub fn enemy_type(self) -> crate::enemy_type::EnemyType {
        match self {
            Self::Brute => crate::enemy_type::EnemyType::BossBrute,
            Self::Latcher => crate::enemy_type::EnemyType::BossLatcher,
        }
    }

    /// What this boss fields and when — total by construction, so the
    /// manifest never probes `escorts()`/`death_spawn()` and panics on a
    /// mismatch. Pinned against the `EnemyType` tables in tests.
    pub fn minions(self) -> (crate::enemy_type::EnemyType, crate::level_assembly::MinionTrigger) {
        use crate::level_assembly::MinionTrigger;
        match self {
            Self::Brute => (crate::enemy_type::EnemyType::SpawnDrone, MinionTrigger::OnDeath),
            Self::Latcher => (crate::enemy_type::EnemyType::SpawnDrone, MinionTrigger::OnEngage),
        }
    }
}

/// How many minions a boss fields at this level: the base trio grows by one
/// per planet past the first, capped — fights escalate without drowning the
/// arena. Applies to whichever trigger the boss kind uses.
pub fn boss_adds(level: u32) -> u8 {
    const BASE: u32 = 3;
    const CAP: u32 = 6;
    (BASE + (crate::planet::planet_of(level) - 1)).min(CAP) as u8
}

/// The hull a planet-final boss's red container grants: a seed-deterministic
/// pick among the purchasable hulls not yet owned. `None` once the fleet is
/// complete — the container falls back to a components payout (see
/// `CurrencyKind::HullReward`).
pub fn roll_hull_reward(
    seed: crate::seed::Seed,
    level: u32,
    unlocks: &crate::unlocks::PermanentUnlocks,
) -> Option<crate::ship_type::ShipType> {
    use rand::rngs::SmallRng;
    use rand::{RngExt, SeedableRng};

    let candidates: Vec<_> = crate::ship_type::ShipType::ALL
        .iter()
        .copied()
        .filter(|s| s.spec().organic_cost.is_some() && !unlocks.owns_ship(*s))
        .collect();
    if candidates.is_empty() {
        return None;
    }
    // Salted off the level seed so the pick never correlates with the
    // manifest rolls made from the same run seed.
    const HULL_SALT: u64 = 0x0b05_5000_4001;
    let mut rng = SmallRng::seed_from_u64(seed.for_level(level).value() ^ HULL_SALT);
    Some(candidates[rng.random_range(0..candidates.len())])
}

/// How many caches this level's boss sheds in total — the complete drop
/// set the arena stays sealed for (owner's call 2026-07-05: ALL boss loot
/// gathered opens the room, not any one pickup). Planet-final with hulls
/// left = the red container alone; otherwise the bound blue cache plus the
/// consolation pile.
pub fn boss_drop_count(
    seed: crate::seed::Seed,
    level: u32,
    unlocks: &crate::unlocks::PermanentUnlocks,
) -> u8 {
    if is_planet_final(level) && roll_hull_reward(seed, level, unlocks).is_some() {
        1
    } else {
        1 + consolation_pile(level).len() as u8
    }
}

/// The mid-boss drop (and the planet-final drop once every hull is owned):
/// a pile of blue and green caches, richer each planet. Pre-built dormant
/// beside the boss (Faucet) and scattered on its death.
pub fn consolation_pile(level: u32) -> Vec<(crate::currency::CurrencyKind, u32)> {
    use crate::currency::CurrencyKind;

    // Per-planet scaling knobs (B9 tunes from play).
    const PILE_COMPONENTS_BASE: u32 = 4_000;
    const PILE_ORGANICS_BASE: u32 = 100;
    const PILE_ORGANIC_CACHES: usize = 2;

    let planet = crate::planet::planet_of(level);
    let mut pile = vec![(CurrencyKind::Components, PILE_COMPONENTS_BASE * planet)];
    pile.extend(
        std::iter::repeat_n(
            (CurrencyKind::Organics, PILE_ORGANICS_BASE * planet),
            PILE_ORGANIC_CACHES,
        ),
    );
    pile
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planet::PLANET_LENGTH;

    #[test]
    fn no_boss_before_level_three() {
        assert_eq!(boss_for_level(1), None);
        assert_eq!(boss_for_level(2), None);
    }

    #[test]
    fn every_planet_stages_brute_then_latcher() {
        for planet in 0..4u32 {
            let base = planet * PLANET_LENGTH;
            for rel in 1..=PLANET_LENGTH {
                let expected = match rel {
                    MID_BOSS_AT => Some(BossKind::Brute),
                    FINAL_BOSS_AT => Some(BossKind::Latcher),
                    _ => None,
                };
                assert_eq!(boss_for_level(base + rel), expected,
                    "level {} (planet {}, rel {})", base + rel, planet + 1, rel);
            }
        }
    }

    #[test]
    fn the_final_boss_guards_the_planet_exit() {
        assert!(!is_planet_final(3), "the mid-boss is not the exit");
        assert!(is_planet_final(6));
        assert!(!is_planet_final(7));
        assert!(is_planet_final(12));
    }

    #[test]
    fn boss_kinds_map_to_their_roster_entries() {
        use crate::enemy_type::EnemyType;
        assert_eq!(BossKind::Brute.enemy_type(), EnemyType::BossBrute);
        assert_eq!(BossKind::Latcher.enemy_type(), EnemyType::BossLatcher);
    }

    #[test]
    fn boss_minions_agree_with_the_roster_tables() {
        use crate::level_assembly::MinionTrigger;
        // Consistency pin: the total mapping and the EnemyType tables can
        // never drift apart.
        let (brute_kind, brute_trigger) = BossKind::Brute.minions();
        assert_eq!(Some((brute_kind, 3)),
            BossKind::Brute.enemy_type().death_spawn().map(|(t, _)| (t, 3)));
        assert_eq!(brute_trigger, MinionTrigger::OnDeath);

        let (latcher_kind, latcher_trigger) = BossKind::Latcher.minions();
        assert_eq!(Some(latcher_kind),
            BossKind::Latcher.enemy_type().escorts().map(|(t, _)| t));
        assert_eq!(latcher_trigger, MinionTrigger::OnEngage);
    }

    #[test]
    fn the_red_container_grants_only_unowned_hulls() {
        use crate::seed::Seed;
        use crate::ship_type::ShipType;
        use crate::unlocks::{PermanentUnlocks, Unlock};

        let mut owned = PermanentUnlocks::new();
        let mut granted = Vec::new();
        // Collect every red container of a long run: each grant must be a
        // purchasable hull not yet owned, and the fleet completes exactly.
        for round in 0..ShipType::ALL.len() + 2 {
            let level = 6 * (round as u32 + 1); // successive planet-finals
            match roll_hull_reward(Seed::new(1), level, &owned) {
                Some(hull) => {
                    assert!(hull.spec().organic_cost.is_some(),
                        "{hull:?} — the starter is never a boss reward");
                    assert!(!owned.owns_ship(hull),
                        "{hull:?} was already owned — a dead grant");
                    owned.grant(Unlock::Ship(hull));
                    granted.push(hull);
                }
                None => break,
            }
        }
        assert_eq!(granted.len(), 3, "the three purchasable hulls all arrive");
        assert_eq!(roll_hull_reward(Seed::new(1), 24, &owned), None,
            "a complete fleet means no further hull grants");
    }

    #[test]
    fn the_hull_roll_is_seed_deterministic() {
        use crate::seed::Seed;
        use crate::unlocks::PermanentUnlocks;
        let owned = PermanentUnlocks::new();
        let a = roll_hull_reward(Seed::new(7), 6, &owned);
        let b = roll_hull_reward(Seed::new(7), 6, &owned);
        assert_eq!(a, b, "same run, same level, same hull");
        assert!(a.is_some(), "an empty fleet always yields a hull");
    }

    #[test]
    fn the_drop_count_is_the_container_alone_or_the_whole_pile() {
        use crate::seed::Seed;
        use crate::ship_type::ShipType;
        use crate::unlocks::{PermanentUnlocks, Unlock};

        let fresh = PermanentUnlocks::new();
        assert_eq!(boss_drop_count(Seed::new(1), 6, &fresh), 1,
            "a planet final with hulls left sheds exactly the red container");
        assert_eq!(
            boss_drop_count(Seed::new(1), 3, &fresh) as usize,
            1 + consolation_pile(3).len(),
            "a mid-boss sheds its bound cache plus the whole pile"
        );

        let mut full = PermanentUnlocks::new();
        for ship in [ShipType::Talon, ShipType::Hive, ShipType::Reaver] {
            full.grant(Unlock::Ship(ship));
        }
        assert_eq!(
            boss_drop_count(Seed::new(1), 6, &full) as usize,
            1 + consolation_pile(6).len(),
            "a complete fleet turns the final drop into the pile too"
        );
    }

    #[test]
    fn the_consolation_pile_pays_both_currencies_and_scales_by_planet() {
        use crate::currency::CurrencyKind;
        let pile = consolation_pile(3);
        assert!(!pile.is_empty(), "a boss never drops nothing");
        assert!(pile.iter().any(|(k, _)| *k == CurrencyKind::Components),
            "the pile pays blues");
        assert!(pile.iter().any(|(k, _)| *k == CurrencyKind::Organics),
            "the pile pays greens");
        assert!(pile.iter().all(|(_, amount)| *amount > 0));

        let p1: u32 = consolation_pile(3).iter().map(|(_, a)| a).sum();
        let p2: u32 = consolation_pile(9).iter().map(|(_, a)| a).sum();
        assert!(p2 > p1, "planet 2's bosses pay better than planet 1's");
    }

    #[test]
    fn boss_adds_start_at_three_and_grow_one_per_planet_capped() {
        assert_eq!(boss_adds(3), 3, "planet 1: the base trio");
        assert_eq!(boss_adds(6), 3, "same planet, same adds");
        assert_eq!(boss_adds(9), 4, "planet 2 adds one");
        assert_eq!(boss_adds(15), 5, "planet 3");
        assert_eq!(boss_adds(21), 6, "planet 4");
        assert_eq!(boss_adds(27), 6, "capped — escalation, not a drowning");
    }
}
