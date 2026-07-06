//! Boss reward formulas. WHICH boss a level stages, its escorts (kind,
//! trigger, and count), track, and reward policy are DECLARED per boss
//! slot in the roster grammar (rosters/planets/*.toml); this module keeps
//! the level-scaled reward formulas the slots reference: the hull-reward
//! roll and the consolation-pile economy.

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
    let mut rng =
        SmallRng::seed_from_u64(seed.for_level(level).value() ^ crate::seed::salt::HULL);
    Some(candidates[rng.random_range(0..candidates.len())])
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
}
