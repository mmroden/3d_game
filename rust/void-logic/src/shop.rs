//! The between-level (and between-lives) shop: the purchase authority.
//!
//! Everything the player buys goes through [`purchase`]: it validates,
//! spends through the typed [`Account`](crate::currency::Account)s, mutates
//! [`RunState`], and returns a [`Receipt`] telling the shell what to push
//! (an upgrade to the ship, a laser change, a life). The shell never mutates
//! prices or balances itself.
//!
//! Pricing shape (tests pin the shape, not the absolute balance):
//! - Stat upgrades grant a fixed +10% and cost `BASE × 1.6^owned` — the
//!   growth curve is the soft cap.
//! - The laser delegates to [`LaserLevel::upgrade_cost`].
//! - Extra lives double per purchase, keyed to lives *bought* this run (not
//!   current lives), so losing a life never discounts the next one.

use crate::currency::{CurrencyKind, NotEnough};
use crate::laser::LaserLevel;
use crate::run_state::RunState;
use crate::unlocks::Unlock;
use crate::upgrade::{Upgrade, UpgradeKind};

/// Everything the shop can sell. Ids cross to GDScript: stat kinds map
/// through [`UpgradeKind::id`] (0..=4), then Laser and ExtraLife, then the
/// permanent unlocks (green) continue the id space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShopItemId {
    Stat(UpgradeKind),
    Laser,
    ExtraLife,
    /// A permanent unlock, priced in organics.
    Unlock(Unlock),
    /// One Shield Surge refill (blue) — stocked only once the green item
    /// is owned. Its id sits AFTER the unlocks so their ids never shift.
    ShieldCharge,
}

/// Number of stat ids; Laser, ExtraLife, then the unlocks follow.
const STAT_IDS: i32 = UpgradeKind::ALL.len() as i32;

impl ShopItemId {
    pub fn id(&self) -> i32 {
        match self {
            Self::Stat(kind) => kind.id(),
            Self::Laser => STAT_IDS,
            Self::ExtraLife => STAT_IDS + 1,
            Self::Unlock(unlock) => STAT_IDS + 2 + unlock.id(),
            Self::ShieldCharge => STAT_IDS + 2 + Unlock::ALL.len() as i32,
        }
    }

    pub fn from_id(id: i32) -> Option<ShopItemId> {
        if let Some(kind) = UpgradeKind::from_id(id) {
            return Some(Self::Stat(kind));
        }
        match id - STAT_IDS {
            0 => Some(Self::Laser),
            1 => Some(Self::ExtraLife),
            n if n - 2 == Unlock::ALL.len() as i32 => Some(Self::ShieldCharge),
            n => Unlock::from_id(n - 2).map(Self::Unlock),
        }
    }
}

/// One row of the shop screen, fully priced against the current run.
#[derive(Debug, Clone, PartialEq)]
pub struct ShopOffer {
    pub id: ShopItemId,
    pub label: String,
    /// What the purchase actually does to THIS run — the now→next stat
    /// line, the laser damage jump, the capability blurb. Derived here so
    /// the shell only renders (playtest 2026-07-03: "no sense of what
    /// buying an upgrade gets you").
    pub detail: String,
    pub cost: u32,
    pub currency: CurrencyKind,
    /// The matching account covers the cost right now.
    pub affordable: bool,
    /// The item can be bought at all (a maxed laser cannot).
    pub purchasable: bool,
}

/// What a successful purchase changed — the shell pushes this to consumers.
#[derive(Debug, Clone, PartialEq)]
pub enum Receipt {
    UpgradeAdded(Upgrade),
    LaserChanged(LaserLevel),
    /// Lives after the purchase.
    LifeAdded(u32),
    /// A permanent unlock was granted — persist the profile immediately.
    UnlockGranted(Unlock),
    /// Shield Surge charges after the refill.
    ChargeAdded(u32),
}

/// Why a purchase was refused. The run is untouched in every refusal.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// The item cannot be bought (maxed laser, unknown id).
    NotPurchasable,
    NotEnough(NotEnough),
}

/// Fixed multiplier every stat purchase grants.
const STAT_UPGRADE_MULTIPLIER: f32 = 1.10;
/// Cost growth per already-owned upgrade of the same kind.
const STAT_COST_GROWTH: f32 = 1.6;
/// First extra life; each purchase adds a flat step (linear, not
/// exponential — owner's call 2026-07-04: a life stays reachable).
const LIFE_BASE_COST: u32 = 10_000;
/// Added to the life price per life already bought this run.
const LIFE_STEP_COST: u32 = 5_000;
/// One Shield Surge refill (flat — the rack is the ratchet).
const SHIELD_CHARGE_COST: u32 = 5_000;
/// Rack capacity — the HUD counter's ceiling.
const SHIELD_CHARGE_CAP: u32 = 9;

/// Base price per stat kind — the catalog's relative value ordering.
fn stat_base_cost(kind: UpgradeKind) -> u32 {
    match kind {
        UpgradeKind::ProjectileDamage => 5_000,
        UpgradeKind::FireRate => 4_000,
        UpgradeKind::MaxHealth => 3_000,
        UpgradeKind::ShieldCapacity => 3_500,
        UpgradeKind::Thrust => 2_000,
        UpgradeKind::RotationSpeed => 1_500,
    }
}

/// Current price of the next `kind` upgrade: base × 1.6^owned, rounded to
/// the nearest hundred. The growth curve is the soft cap — no hard limit.
fn stat_cost(run: &RunState, kind: UpgradeKind) -> u32 {
    let owned = run.loadout.upgrades.iter().filter(|u| u.kind == kind).count() as i32;
    let raw = stat_base_cost(kind) as f32 * STAT_COST_GROWTH.powi(owned);
    ((raw / 100.0).round() as u32).saturating_mul(100)
}

/// Current price of the next extra life: a flat step per life *bought*
/// this run (losing one never discounts the next).
fn life_cost(run: &RunState) -> u32 {
    LIFE_BASE_COST.saturating_add(LIFE_STEP_COST.saturating_mul(run.lives_purchased))
}

/// The full catalog priced against `run`, in display order: the five stat
/// upgrades, the laser, then the extra life. Unpurchasable stock (a maxed
/// laser) stays listed so the menu never reshuffles under the cursor.
pub fn offers(run: &RunState) -> Vec<ShopOffer> {
    let mut out: Vec<ShopOffer> = UpgradeKind::ALL.iter().map(|kind| {
        let cost = stat_cost(run, *kind);
        let now = run.loadout.stat_multiplier(*kind);
        ShopOffer {
            id: ShopItemId::Stat(*kind),
            label: format!("{} +10%", kind.label()),
            detail: format!("now ×{:.2} → ×{:.2}", now, now * STAT_UPGRADE_MULTIPLIER),
            cost,
            currency: CurrencyKind::Components,
            affordable: run.components.can_afford(cost),
            purchasable: true,
        }
    }).collect();

    let (laser_label, laser_detail, laser_cost, laser_purchasable) = match run.laser_level.next() {
        Some(next) => (
            format!("Laser: {}", next.display_name()),
            format!("damage {} → {}", run.laser_level.damage(), next.damage()),
            next.upgrade_cost().unwrap_or(0),
            true,
        ),
        None => (
            "Laser: MAXED".to_string(),
            "the spectrum ends at Violet".to_string(),
            0,
            false,
        ),
    };
    out.push(ShopOffer {
        id: ShopItemId::Laser,
        label: laser_label,
        detail: laser_detail,
        cost: laser_cost,
        currency: CurrencyKind::Components,
        affordable: laser_purchasable && run.components.can_afford(laser_cost),
        purchasable: laser_purchasable,
    });

    let cost = life_cost(run);
    out.push(ShopOffer {
        id: ShopItemId::ExtraLife,
        label: "Extra Life".to_string(),
        detail: format!("lives {} → {}", run.lives, run.lives + 1),
        cost,
        currency: CurrencyKind::Components,
        affordable: run.components.can_afford(cost),
        purchasable: true,
    });

    // Shield Surge refills: blue, and only stocked once the green item is
    // owned (the item explains the trigger at purchase; refills need no
    // second tutorial).
    if run.unlocks.contains(Unlock::ShieldBurst) {
        let full = run.shield_charges >= SHIELD_CHARGE_CAP;
        out.push(ShopOffer {
            id: ShopItemId::ShieldCharge,
            label: "Surge Charge".to_string(),
            detail: format!("charges {} → {}", run.shield_charges,
                (run.shield_charges + 1).min(SHIELD_CHARGE_CAP)),
            cost: SHIELD_CHARGE_COST,
            currency: CurrencyKind::Components,
            affordable: !full && run.components.can_afford(SHIELD_CHARGE_COST),
            purchasable: !full,
        });
    }

    // The green section: permanent unlocks, priced in organics. Owned
    // unlocks leave the catalog for good — unlike the maxed laser, they can
    // never come back on offer, so there is no row to keep stable.
    for unlock in Unlock::ALL {
        if run.unlocks.contains(*unlock) || !run.unlocks.available(*unlock) {
            continue;
        }
        let cost = unlock.organic_cost();
        out.push(ShopOffer {
            id: ShopItemId::Unlock(*unlock),
            label: unlock.display_name().to_string(),
            // Blurb + trigger: the confirm screen shows exactly this, so a
            // green purchase always says what it does AND how to use it.
            detail: format!("{} | {}", unlock.blurb(), unlock.trigger_hint()),
            cost,
            currency: CurrencyKind::Organics,
            affordable: run.organics.can_afford(cost),
            purchasable: true,
        });
    }

    out
}

/// Buy `id` for `run`: validate, spend, mutate, receipt. Any refusal leaves
/// the run untouched — `Account::spend` is atomic and mutation follows it.
pub fn purchase(run: &mut RunState, id: ShopItemId) -> Result<Receipt, Refusal> {
    match id {
        ShopItemId::Stat(kind) => {
            let cost = stat_cost(run, kind);
            run.components.spend(cost).map_err(Refusal::NotEnough)?;
            let upgrade = Upgrade {
                name: format!("{} +10%", kind.label()),
                kind,
                multiplier: STAT_UPGRADE_MULTIPLIER,
            };
            run.loadout.add_upgrade(upgrade.clone());
            if kind == UpgradeKind::ShieldCapacity {
                // The shield envelope lives on RunState, not the loadout —
                // re-derive it so the purchase takes effect immediately.
                run.refresh_shield();
            }
            Ok(Receipt::UpgradeAdded(upgrade))
        }
        ShopItemId::Laser => {
            let next = run.laser_level.next().ok_or(Refusal::NotPurchasable)?;
            let cost = next.upgrade_cost().ok_or(Refusal::NotPurchasable)?;
            run.components.spend(cost).map_err(Refusal::NotEnough)?;
            run.laser_level = next;
            Ok(Receipt::LaserChanged(next))
        }
        ShopItemId::ExtraLife => {
            let cost = life_cost(run);
            run.components.spend(cost).map_err(Refusal::NotEnough)?;
            run.lives += 1;
            run.lives_purchased += 1;
            Ok(Receipt::LifeAdded(run.lives))
        }
        ShopItemId::Unlock(unlock) => {
            if run.unlocks.contains(unlock) || !run.unlocks.available(unlock) {
                return Err(Refusal::NotPurchasable);
            }
            run.organics.spend(unlock.organic_cost()).map_err(Refusal::NotEnough)?;
            run.unlocks.grant(unlock);
            if unlock == Unlock::ShieldBurst {
                // The item arrives stocked.
                run.shield_charges = crate::run_state::SHIELD_BURST_STARTING_CHARGES;
            }
            Ok(Receipt::UnlockGranted(unlock))
        }
        ShopItemId::ShieldCharge => {
            if !run.unlocks.contains(Unlock::ShieldBurst)
                || run.shield_charges >= SHIELD_CHARGE_CAP
            {
                return Err(Refusal::NotPurchasable);
            }
            run.components.spend(SHIELD_CHARGE_COST).map_err(Refusal::NotEnough)?;
            run.shield_charges += 1;
            Ok(Receipt::ChargeAdded(run.shield_charges))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed::Seed;

    fn rich_run() -> RunState {
        let mut run = RunState::new(Seed::new(42));
        run.collect_cache(CurrencyKind::Components, 1_000_000);
        run
    }

    #[test]
    fn item_id_round_trips() {
        let mut all = vec![ShopItemId::Laser, ShopItemId::ExtraLife];
        all.extend(UpgradeKind::ALL.iter().map(|k| ShopItemId::Stat(*k)));
        for item in all {
            assert_eq!(ShopItemId::from_id(item.id()), Some(item),
                "{item:?} must round-trip through its id");
        }
        assert_eq!(ShopItemId::from_id(-1), None);
        assert_eq!(ShopItemId::from_id(99), None);
    }

    #[test]
    fn offers_cover_stats_laser_and_life() {
        let run = RunState::new(Seed::new(42));
        let offers = offers(&run);
        for kind in UpgradeKind::ALL {
            assert!(offers.iter().any(|o| o.id == ShopItemId::Stat(*kind)),
                "catalog must offer {kind:?}");
        }
        assert!(offers.iter().any(|o| o.id == ShopItemId::Laser), "catalog must offer the laser");
        assert!(offers.iter().any(|o| o.id == ShopItemId::ExtraLife), "catalog must offer a life");
        for offer in &offers {
            assert!(!offer.label.is_empty(), "{:?} needs a label", offer.id);
            let expected = match offer.id {
                ShopItemId::Unlock(_) => CurrencyKind::Organics,
                ShopItemId::Stat(_)
                | ShopItemId::Laser
                | ShopItemId::ExtraLife
                | ShopItemId::ShieldCharge => CurrencyKind::Components,
            };
            assert_eq!(offer.currency, expected,
                "{:?} belongs to the {expected:?} section", offer.id);
        }
    }

    #[test]
    fn stat_cost_grows_1_6x_per_owned() {
        let mut run = rich_run();
        let cost_of = |run: &RunState| {
            offers(run).iter()
                .find(|o| o.id == ShopItemId::Stat(UpgradeKind::ProjectileDamage))
                .expect("damage offer").cost
        };
        let first = cost_of(&run);
        assert!(first > 0, "stat upgrades are not free");
        purchase(&mut run, ShopItemId::Stat(UpgradeKind::ProjectileDamage)).expect("rich run affords");
        let second = cost_of(&run);
        purchase(&mut run, ShopItemId::Stat(UpgradeKind::ProjectileDamage)).expect("rich run affords");
        let third = cost_of(&run);
        // Shape: strictly growing, roughly 1.6× per owned (rounded to 100s).
        let ratio2 = second as f32 / first as f32;
        let ratio3 = third as f32 / second as f32;
        for ratio in [ratio2, ratio3] {
            assert!((1.5..=1.7).contains(&ratio),
                "cost growth per owned should be ~1.6x, got {ratio}");
        }
    }

    #[test]
    fn stat_purchase_adds_fixed_ten_percent_upgrade_and_deducts() {
        let mut run = rich_run();
        let before = run.components.balance;
        let thrust_before = run.loadout.thrust_power();
        let receipt = purchase(&mut run, ShopItemId::Stat(UpgradeKind::Thrust)).expect("affordable");
        let Receipt::UpgradeAdded(upgrade) = receipt else {
            panic!("a stat purchase yields an upgrade receipt");
        };
        assert_eq!(upgrade.kind, UpgradeKind::Thrust);
        assert!((upgrade.multiplier - 1.10).abs() < 1e-6, "fixed +10%, no RNG");
        assert!(run.components.balance < before, "the purchase deducts components");
        let thrust_after = run.loadout.thrust_power();
        assert!((thrust_after / thrust_before - 1.10).abs() < 1e-3,
            "the loadout applies the bought upgrade: {thrust_before} -> {thrust_after}");
    }

    #[test]
    fn insufficient_funds_leaves_run_untouched() {
        let mut run = RunState::new(Seed::new(42)); // broke
        let upgrades_before = run.loadout.upgrades.len();
        let result = purchase(&mut run, ShopItemId::Stat(UpgradeKind::Thrust));
        assert!(matches!(result, Err(Refusal::NotEnough(_))),
            "a broke run is refused for funds, got {result:?}");
        assert_eq!(run.loadout.upgrades.len(), upgrades_before, "no upgrade granted");
        assert_eq!(run.components.balance, 0, "nothing deducted");
        assert_eq!(run.lives, 1, "no life granted");
    }

    #[test]
    fn extra_life_price_ratchets_and_ignores_life_loss() {
        let mut run = rich_run();
        let life_cost = |run: &RunState| {
            offers(run).iter().find(|o| o.id == ShopItemId::ExtraLife).expect("life offer").cost
        };
        let first = life_cost(&run);
        assert_eq!(first, 10_000, "the first spare life anchors at 10k");
        let receipt = purchase(&mut run, ShopItemId::ExtraLife).expect("affordable");
        assert_eq!(receipt, Receipt::LifeAdded(2), "one spare life on top of the starting one");
        let second = life_cost(&run);
        // Linear steps, not doubling (owner's call 2026-07-04): 10k, 15k,
        // 20k, 25k — a life stays reachable deep into a run.
        assert_eq!(second, 15_000, "each life adds a flat step");
        purchase(&mut run, ShopItemId::ExtraLife).expect("affordable");
        let third = life_cost(&run);
        assert_eq!(third, 20_000, "10k → 15k → 20k");
        // Losing a bought life must NOT discount the next one: the ratchet
        // keys to lives purchased, not lives held.
        run.apply_life_loss();
        assert_eq!(life_cost(&run), third, "dying never discounts the next life");
    }

    #[test]
    fn laser_offer_unpurchasable_at_violet() {
        let mut run = rich_run();
        run.laser_level = LaserLevel::Violet;
        let offer = offers(&run).into_iter().find(|o| o.id == ShopItemId::Laser).expect("laser offer");
        assert!(!offer.purchasable, "a maxed laser stays listed but dimmed");
        assert!(matches!(purchase(&mut run, ShopItemId::Laser), Err(Refusal::NotPurchasable)));
    }

    #[test]
    fn laser_purchase_delegates_to_cost_table() {
        let mut run = rich_run();
        let before = run.components.balance;
        let receipt = purchase(&mut run, ShopItemId::Laser).expect("affordable");
        assert_eq!(receipt, Receipt::LaserChanged(LaserLevel::Orange));
        assert_eq!(run.laser_level, LaserLevel::Orange);
        let expected = LaserLevel::Orange.upgrade_cost().expect("orange has a cost");
        assert_eq!(before - run.components.balance, expected,
            "the laser price is the LaserLevel table, unchanged");
    }

    #[test]
    fn unlock_offers_are_green_and_vanish_once_owned() {
        let run = RunState::new(Seed::new(42));
        let radar = offers(&run).into_iter()
            .find(|o| o.id == ShopItemId::Unlock(Unlock::Radar))
            .expect("the first rung is on offer from the start");
        assert_eq!(radar.currency, CurrencyKind::Organics, "unlocks are green purchases");
        assert_eq!(radar.cost, Unlock::Radar.organic_cost());
        assert!(radar.purchasable);

        let mut owned = RunState::new(Seed::new(42));
        owned.unlocks.grant(Unlock::Radar);
        let catalog = offers(&owned);
        assert!(!catalog.iter().any(|o| o.id == ShopItemId::Unlock(Unlock::Radar)),
            "an owned unlock leaves the catalog — it cannot be bought twice");
        assert!(catalog.iter().any(|o| o.id == ShopItemId::Unlock(Unlock::FogMap)),
            "owning a rung puts the next one on offer");
    }

    #[test]
    fn unlock_purchase_spends_organics_never_components() {
        let mut run = RunState::new(Seed::new(42));
        run.collect_cache(CurrencyKind::Components, 100_000);
        run.collect_cache(CurrencyKind::Organics, 400);

        let receipt = purchase(&mut run, ShopItemId::Unlock(Unlock::Radar)).expect("affordable");
        assert_eq!(receipt, Receipt::UnlockGranted(Unlock::Radar));
        assert!(run.unlocks.contains(Unlock::Radar));
        assert_eq!(run.organics.balance, 400 - Unlock::Radar.organic_cost(),
            "the radar costs organics");
        assert_eq!(run.components.balance, 100_000, "components are never touched");

        // 100 organics left: FogMap (500) is refused on the green account
        // even though the blue account is rich.
        let result = purchase(&mut run, ShopItemId::Unlock(Unlock::FogMap));
        assert!(matches!(result, Err(Refusal::NotEnough(_))),
            "green purchases are refused on the green balance, got {result:?}");
        assert!(!run.unlocks.contains(Unlock::FogMap));
    }

    #[test]
    fn surge_charges_are_blue_and_wait_for_the_green_item() {
        let mut run = RunState::new(Seed::new(42));
        run.collect_cache(CurrencyKind::Components, 20_000);
        assert!(!offers(&run).iter().any(|o| o.id == ShopItemId::ShieldCharge),
            "no Surge item, no refill row");
        assert!(matches!(purchase(&mut run, ShopItemId::ShieldCharge),
            Err(Refusal::NotPurchasable)));

        run.collect_cache(CurrencyKind::Organics, 2_000);
        run.unlocks.grant(Unlock::Radar);
        purchase(&mut run, ShopItemId::Unlock(Unlock::ShieldBurst))
            .expect("radar owned, 2k affords the 1k surge item");
        assert_eq!(run.shield_charges, 3, "the item arrives stocked");

        let offer = offers(&run).into_iter()
            .find(|o| o.id == ShopItemId::ShieldCharge)
            .expect("owned item stocks the refill row");
        assert_eq!(offer.currency, CurrencyKind::Components, "refills are blue");
        assert_eq!(offer.cost, 5_000);

        let before = run.components.balance;
        let receipt = purchase(&mut run, ShopItemId::ShieldCharge).expect("affordable");
        assert_eq!(receipt, Receipt::ChargeAdded(4));
        assert_eq!(before - run.components.balance, 5_000, "flat price, no ratchet");
    }

    #[test]
    fn green_details_carry_the_trigger_hint() {
        // The confirm screen shows the detail line verbatim — every green
        // row must say what it does AND how to use it.
        let run = RunState::new(Seed::new(42));
        let radar = offers(&run).into_iter()
            .find(|o| o.id == ShopItemId::Unlock(Unlock::Radar))
            .expect("radar on offer");
        assert!(radar.detail.contains('|'),
            "blurb | trigger, got {:?}", radar.detail);
    }

    #[test]
    fn shield_charge_id_sits_after_the_unlocks() {
        // Appended AFTER the unlock block so no GDScript id shifted.
        assert_eq!(ShopItemId::Unlock(Unlock::Radar).id(), 8, "radar keeps its id");
        let charge = ShopItemId::ShieldCharge.id();
        assert_eq!(ShopItemId::from_id(charge), Some(ShopItemId::ShieldCharge));
        assert!(charge > ShopItemId::Unlock(Unlock::ShieldBurst).id());
    }

    #[test]
    fn buying_shields_raises_the_envelope_through_the_shop() {
        let mut run = RunState::new(Seed::new(42));
        run.collect_cache(CurrencyKind::Components, 10_000);
        let before = run.shield.max_capacity;
        purchase(&mut run, ShopItemId::Stat(UpgradeKind::ShieldCapacity))
            .expect("10k affords the 3.5k shield upgrade");
        assert!(run.shield.max_capacity > before,
            "the purchase must reach the shield envelope, not just the loadout");
    }

    #[test]
    fn every_offer_explains_itself() {
        let mut run = RunState::new(Seed::new(42));
        run.unlocks.grant(Unlock::Radar);
        run.unlocks.grant(Unlock::FogMap);
        run.unlocks.grant(Unlock::Valkyrie); // the fleet is on offer too
        for offer in offers(&run) {
            assert!(!offer.detail.is_empty(),
                "{:?} needs a detail line — a row must say what buying it does", offer.id);
        }
    }

    #[test]
    fn stat_detail_walks_now_to_next() {
        let mut run = RunState::new(Seed::new(42));
        let damage_detail = |run: &RunState| offers(run).iter()
            .find(|o| o.id == ShopItemId::Stat(UpgradeKind::ProjectileDamage))
            .expect("damage row is always stocked").detail.clone();

        let fresh = damage_detail(&run);
        assert!(fresh.contains("1.00") && fresh.contains("1.10"),
            "an unupgraded stat reads now ×1.00 → next ×1.10, got {fresh:?}");

        run.collect_cache(CurrencyKind::Components, 100_000);
        purchase(&mut run, ShopItemId::Stat(UpgradeKind::ProjectileDamage)).unwrap();
        let after = damage_detail(&run);
        assert!(after.contains("1.10") && after.contains("1.21"),
            "the detail compounds with what is owned, got {after:?}");
    }

    #[test]
    fn laser_detail_shows_the_damage_jump() {
        let run = RunState::new(Seed::new(42));
        let detail = offers(&run).iter()
            .find(|o| o.id == ShopItemId::Laser)
            .expect("laser row is always stocked").detail.clone();
        let current = run.laser_level;
        let next = current.next().expect("a fresh run is not maxed");
        assert!(detail.contains(&current.damage().to_string()),
            "the jump starts from the current damage, got {detail:?}");
        assert!(detail.contains(&next.damage().to_string()),
            "and lands on the next level's damage, got {detail:?}");
    }

    #[test]
    fn life_detail_counts_lives() {
        let run = RunState::new(Seed::new(42));
        let detail = offers(&run).iter()
            .find(|o| o.id == ShopItemId::ExtraLife)
            .expect("the life row is always stocked").detail.clone();
        assert!(detail.contains("1") && detail.contains("2"),
            "one life becomes two, got {detail:?}");
    }

    #[test]
    fn the_green_section_walks_the_ladder() {
        // Early game: exactly one green capability on offer (the next rung).
        let run = RunState::new(Seed::new(42));
        let green: Vec<_> = offers(&run).iter()
            .filter(|o| o.currency == CurrencyKind::Organics)
            .map(|o| o.id)
            .collect();
        assert_eq!(green, vec![ShopItemId::Unlock(Unlock::Radar)],
            "a fresh profile sees only the radar in the green section");

        // Owning the whole spine opens the branches — but NEVER the hulls:
        // those left the shop for the planet-final bosses' red containers
        // (owner's call 2026-07-04).
        let mut veteran = RunState::new(Seed::new(42));
        veteran.unlocks.grant(Unlock::Radar);
        veteran.unlocks.grant(Unlock::FogMap);
        veteran.unlocks.grant(Unlock::Valkyrie);
        let green: Vec<_> = offers(&veteran).iter()
            .filter(|o| o.currency == CurrencyKind::Organics)
            .map(|o| o.id)
            .collect();
        use crate::ship_type::ShipType;
        assert_eq!(green.len(), 3,
            "the spine done: the two map branches and the surge — no hulls: {green:?}");
        assert!(!green.iter().any(|id| matches!(id, ShopItemId::Unlock(Unlock::Ship(_)))),
            "hulls are boss loot, never shop stock");
        assert!(green.contains(&ShopItemId::Unlock(Unlock::RouteScanner)),
            "owning the map put its upgrades on offer");
        assert!(green.contains(&ShopItemId::Unlock(Unlock::ThreatTracker)));
        assert!(green.contains(&ShopItemId::Unlock(Unlock::ShieldBurst)),
            "owning the radar put the surge on offer");
        let _ = ShipType::Vanguard; // fleet types stay referenced below
    }

    #[test]
    fn unavailable_rungs_cannot_be_bought_even_with_funds() {
        use crate::ship_type::ShipType;
        let mut run = RunState::new(Seed::new(42));
        run.collect_cache(CurrencyKind::Organics, 100_000);
        let result = purchase(&mut run, ShopItemId::Unlock(Unlock::Ship(ShipType::Talon)));
        assert!(matches!(result, Err(Refusal::NotPurchasable)),
            "hulls are gated behind the Valkyrie, got {result:?}");
        assert_eq!(run.organics.balance, 100_000, "nothing deducted");
    }

    #[test]
    fn hulls_never_sell_and_a_hull_purchase_is_refused() {
        use crate::ship_type::ShipType;
        let mut run = RunState::new(Seed::new(42));
        run.collect_cache(CurrencyKind::Organics, 10_000);
        // Even with the whole spine owned and a full purse …
        run.unlocks.grant(Unlock::Radar);
        run.unlocks.grant(Unlock::FogMap);
        run.unlocks.grant(Unlock::Valkyrie);

        let offer_ids: Vec<_> = offers(&run).iter().map(|o| o.id).collect();
        assert!(!offer_ids.iter().any(|id| matches!(id, ShopItemId::Unlock(Unlock::Ship(_)))),
            "… no hull is ever on offer — bosses drop them");

        // A stale/forged purchase id is refused, not honored.
        let result = purchase(&mut run, ShopItemId::Unlock(Unlock::Ship(ShipType::Talon)));
        assert!(result.is_err(), "a hull purchase must be refused");
        assert!(!run.unlocks.owns_ship(ShipType::Talon));
        assert_eq!(run.organics.balance, 10_000, "nothing deducted");
    }

    #[test]
    fn owned_unlock_cannot_be_bought_again() {
        let mut run = RunState::new(Seed::new(42));
        run.collect_cache(CurrencyKind::Organics, 10_000);
        purchase(&mut run, ShopItemId::Unlock(Unlock::Radar)).expect("first buy");
        let balance_after = run.organics.balance;
        let result = purchase(&mut run, ShopItemId::Unlock(Unlock::Radar));
        assert!(matches!(result, Err(Refusal::NotPurchasable)),
            "owning it makes it unpurchasable, got {result:?}");
        assert_eq!(run.organics.balance, balance_after, "no double charge");
    }

    #[test]
    fn offers_mark_affordability() {
        let mut run = RunState::new(Seed::new(42));
        run.collect_cache(CurrencyKind::Components, 2_000); // affords Rotation (1.5k), not Damage (5k)
        let offers = offers(&run);
        let rotation = offers.iter().find(|o| o.id == ShopItemId::Stat(UpgradeKind::RotationSpeed)).expect("offer");
        let damage = offers.iter().find(|o| o.id == ShopItemId::Stat(UpgradeKind::ProjectileDamage)).expect("offer");
        assert!(rotation.affordable, "2k affords the 1.5k rotation upgrade");
        assert!(!damage.affordable, "2k does not afford the 5k damage upgrade");
        assert!(damage.purchasable, "unaffordable is still purchasable stock");
    }
}
