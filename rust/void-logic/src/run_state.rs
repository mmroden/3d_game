use crate::bestiary::SeenEnemies;
use crate::currency::{ComponentAccount, CurrencyKind, OrganicAccount};
use crate::enemy_type::EnemyType;
use crate::kill_tracker::KillTracker;
use crate::laser::LaserLevel;
use crate::loadout::Loadout;
use crate::upgrade::UpgradeKind;
use crate::newtypes::{Health, Damage, Shield};
use crate::seed::Seed;
use crate::shield::ShieldState;
use crate::ship::ShipColor;
use crate::ship_type::ShipType;
use crate::unlocks::PermanentUnlocks;

/// Which defensive layer absorbed a hit. Drives impact SFX: a held shield
/// plays the energy zap, a hull hit plays the heavy metal clang.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageOutcome {
    /// The shield absorbed the whole hit; the hull was untouched.
    ShieldHeld,
    /// Damage overflowed the shield and reached the hull.
    HullHit,
}

/// Shields the Shield Surge restores per charge.
pub const SHIELD_BURST_AMOUNT: f32 = 50.0;
/// Charges the Surge item carries when handed over fresh.
pub const SHIELD_BURST_STARTING_CHARGES: u32 = 3;

/// Tracks the state of a single roguelike run.
#[derive(Debug)]
pub struct RunState {
    pub loadout: Loadout,
    pub current_room: usize,
    pub rooms_cleared: Vec<usize>,
    /// Rooms the player has entered this level — the recon map's reveal set.
    pub rooms_visited: Vec<usize>,
    pub health: Health,
    pub shield: ShieldState,
    pub score: u32,
    pub run_seed: Seed,
    /// In-run currency from mechanical kills; lost on death.
    pub components: ComponentAccount,
    /// Permanent currency from barrels; survives death.
    pub organics: OrganicAccount,
    pub kills: KillTracker,
    pub laser_level: LaserLevel,
    pub current_level: u32,
    /// Chosen hull — stats, weapon, and model (permanent ownership lives in
    /// `unlocks`; the selection itself is per-run state).
    pub ship_type: ShipType,
    /// Chosen ship color — the trim tradeoff of hulls with painted styles.
    pub ship_color: ShipColor,
    /// Lives: the run survives death while more than one remains.
    pub lives: u32,
    /// Lives bought this run — the shop's price-ratchet key, so dying (which
    /// lowers `lives`) never discounts the next life.
    pub lives_purchased: u32,
    /// Enemy types catalogued in the bestiary. Permanent: an enemy is marked on
    /// first sighting and survives death, like organics.
    pub seen_enemies: SeenEnemies,
    /// Permanent unlocks bought with organics (radar, map, …). Survive
    /// run-over like the organics that paid for them.
    pub unlocks: PermanentUnlocks,
    /// Shield Surge charges in the rack. The ITEM is green-permanent; the
    /// charges ride the run (snapshot-saved, refilled blue at the shop, and
    /// the rack restocks to its starting three wherever the item would be
    /// handed over fresh — grant, run-over, profile-only load).
    pub shield_charges: u32,
}

impl RunState {
    /// Default shield: 50 capacity, 2/sec regen, 3s delay after hit.
    const DEFAULT_SHIELD_CAPACITY: f32 = 50.0;
    // Slower regen and a longer post-hit delay so sustained fire actually
    // wears the player down instead of the shield topping back up between shots.
    const DEFAULT_SHIELD_REGEN: f32 = 1.5;
    const DEFAULT_SHIELD_DELAY: f32 = 5.0;
    /// Flat tax for ramming geometry, paid by shield-then-hull like any hit.
    const COLLISION_DAMAGE: f32 = 1.0;

    /// Derive the seed for the current level from the run seed.
    pub fn level_seed(&self) -> Seed {
        self.run_seed.for_level(self.current_level)
    }

    /// Build a shield sized and tuned for the given hull × trim combination.
    fn shield_for(ship_type: ShipType, color: ShipColor) -> ShieldState {
        let spec = ship_type.spec();
        ShieldState::new(
            Shield::new(Self::DEFAULT_SHIELD_CAPACITY * spec.shield_capacity_mul * color.shield_capacity_mul()),
            Self::DEFAULT_SHIELD_REGEN * spec.shield_regen_mul * color.shield_regen_mul(),
            Self::DEFAULT_SHIELD_DELAY,
        )
    }

    pub fn new(seed: Seed) -> Self {
        let loadout = Loadout::new();
        let health = loadout.max_health();
        let ship_type = ShipType::default();
        let ship_color = ShipColor::default();
        let shield = Self::shield_for(ship_type, ship_color);
        Self {
            loadout,
            current_room: 0,
            rooms_cleared: Vec::new(),
            rooms_visited: Vec::new(),
            health,
            shield,
            score: 0,
            run_seed: seed,
            components: ComponentAccount::new(),
            organics: OrganicAccount::new(),
            kills: KillTracker::new(),
            laser_level: LaserLevel::Red,
            current_level: 1,
            ship_type,
            ship_color,
            lives: 1,
            lives_purchased: 0,
            seen_enemies: SeenEnemies::new(),
            unlocks: PermanentUnlocks::new(),
            shield_charges: 0,
        }
    }

    /// Spend one Shield Surge charge: +[`SHIELD_BURST_AMOUNT`] shields on
    /// the spot, clamped to capacity. False (and no effect) with an empty
    /// rack.
    pub fn use_shield_burst(&mut self) -> bool {
        if self.shield_charges == 0 {
            return false;
        }
        self.shield_charges -= 1;
        let boosted = (self.shield.current.as_f32() + SHIELD_BURST_AMOUNT)
            .min(self.shield.max_capacity.as_f32());
        self.shield.current = Shield::new(boosted);
        true
    }

    /// Whether death costs a life instead of the run.
    pub fn has_spare_life(&self) -> bool {
        self.lives > 1
    }

    /// Spend a spare life: the ship is restored, everything earned is kept
    /// (components, upgrades, laser, level) — the level restart around this
    /// is the shell's job. Never call without a spare life.
    pub fn apply_life_loss(&mut self) {
        debug_assert!(self.has_spare_life(), "life loss requires a spare life");
        self.lives = self.lives.saturating_sub(1).max(1);
        self.health = self.loadout.max_health();
        self.shield.reset();
        // The level restarts around this: it is unexplored again.
        self.rooms_visited.clear();
        self.current_room = 0;
    }

    /// Re-derive the shield envelope from hull × trim × owned upgrades,
    /// preserving nothing (the rebuild recharges — shields are topped off
    /// wherever this is called: purchases, hull/trim changes, level starts).
    pub fn refresh_shield(&mut self) {
        let mut shield = Self::shield_for(self.ship_type, self.ship_color);
        let upgrade_mul = self.loadout.stat_multiplier(UpgradeKind::ShieldCapacity);
        shield.max_capacity = Shield::new(shield.max_capacity.as_f32() * upgrade_mul);
        shield.reset();
        self.shield = shield;
    }

    /// Choose a ship color, rebuilding the shield to its capacity/regen.
    pub fn set_ship_color(&mut self, color: ShipColor) {
        self.ship_color = color;
        self.refresh_shield();
    }

    /// Choose a hull, rebuilding the shield to its capacity/regen.
    /// Ownership validation is the caller's job (`unlocks.owns_ship`).
    pub fn set_ship_type(&mut self, ship_type: ShipType) {
        self.ship_type = ship_type;
        self.refresh_shield();
    }

    pub fn is_alive(&self) -> bool {
        self.health.is_alive()
    }

    /// Apply damage: the shield absorbs first, any overflow hits health.
    /// Returns which layer took the hit so the caller picks the right impact SFX.
    pub fn take_damage(&mut self, amount: Damage) -> DamageOutcome {
        let overflow = self.shield.take_hit(amount);
        self.health = self.health.take(overflow);
        if overflow > Damage::new(0.0) {
            DamageOutcome::HullHit
        } else {
            DamageOutcome::ShieldHeld
        }
    }

    /// Ramming any geometry costs a flat point off the top — the shield while
    /// it holds, the hull once it's down. Careening carelessly down a hallway
    /// should hurt, not be consequence-free.
    pub fn take_collision_damage(&mut self) -> DamageOutcome {
        self.take_damage(Damage::new(Self::COLLISION_DAMAGE))
    }

    pub fn tick_shield(&mut self, delta: f32) {
        self.shield.tick(delta);
    }

    /// Mark the player's presence in a room. Returns `true` on the FIRST
    /// visit this level — the shell redraws the recon map only then.
    pub fn visit_room(&mut self, room: usize) -> bool {
        self.current_room = room;
        if self.rooms_visited.contains(&room) {
            return false;
        }
        self.rooms_visited.push(room);
        true
    }

    /// Advance to the next level: everything per-level resets (kill tally,
    /// cleared/visited rooms, ship condition); the run itself — currency,
    /// upgrades, laser, lives — persists.
    pub fn advance_level(&mut self) {
        self.current_level += 1;
        self.kills.reset();
        self.rooms_cleared.clear();
        self.rooms_visited.clear();
        self.current_room = 0;
        self.health = self.loadout.max_health();
        self.shield.reset();
    }

    pub fn clear_room(&mut self, room_index: usize) {
        if !self.rooms_cleared.contains(&room_index) {
            self.rooms_cleared.push(room_index);
            self.score += 100;
        }
    }

    /// Record an enemy kill for the tally. Pays nothing: the kill's reward
    /// rides the cache the enemy drops, credited only via [`Self::collect_cache`].
    pub fn record_kill(&mut self, enemy_type: EnemyType) {
        self.kills.record_kill(enemy_type);
    }

    /// Credit a collected currency cache to the matching account. The only
    /// reward path in a level — nothing is credited without a pickup.
    pub fn collect_cache(&mut self, kind: CurrencyKind, amount: u32) {
        match kind {
            CurrencyKind::Components => self.components.earn(amount),
            CurrencyKind::Organics => self.organics.earn(amount),
            // The hull container is not a balance: GameManager routes it to
            // `boss::roll_hull_reward` BEFORE this call and only forwards the
            // components fallback when every hull is already owned.
            CurrencyKind::HullReward => self.components.earn(amount),
        }
    }

    /// Catalogue an enemy on sighting. Returns `true` the first time this type
    /// is seen, so the caller can persist the freshly-grown bestiary.
    pub fn mark_enemy_seen(&mut self, enemy_type: EnemyType) -> bool {
        self.seen_enemies.mark(enemy_type)
    }

    /// Current laser damage per beam.
    pub fn laser_damage(&self) -> Damage {
        Damage::new(self.laser_level.damage())
    }

    /// Run over (death with no spare life): halve the laser, zero the
    /// components, clear the bought upgrades ("salvage lost"), reset lives
    /// and the life-price ratchet. Organics are permanent and deliberately
    /// preserved, like the bestiary.
    pub fn apply_death_penalty(&mut self) {
        // Run-over is a full blue reset (owner's call 2026-07-04): the
        // laser drops to Red outright — greens/reds are what carry forward.
        self.laser_level = LaserLevel::Red;
        self.components = ComponentAccount::new();
        self.loadout.upgrades.clear();
        self.lives = 1;
        self.lives_purchased = 0;
        // The Surge item is green-permanent and arrives stocked on a fresh
        // run; unbought, the rack stays empty.
        self.shield_charges = if self.unlocks.contains(crate::unlocks::Unlock::ShieldBurst) {
            SHIELD_BURST_STARTING_CHARGES
        } else {
            0
        };
        self.kills.reset();
        self.current_level = 1;
        self.rooms_cleared.clear();
        self.rooms_visited.clear();
        self.current_room = 0;
        self.health = self.loadout.max_health();
        self.shield.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_run_starts_alive() {
        let run = RunState::new(Seed::new(42));
        assert!(run.is_alive());
        assert_eq!(run.health, Health::new(100.0));
        assert_eq!(run.score, 0);
    }

    #[test]
    fn damage_hits_shield_first() {
        let mut run = RunState::new(Seed::new(42));
        let outcome = run.take_damage(Damage::new(30.0));
        // Shield absorbs 30 of 50, health untouched
        assert_eq!(outcome, DamageOutcome::ShieldHeld);
        assert_eq!(run.shield.current, Shield::new(20.0));
        assert_eq!(run.health, Health::new(100.0));
        assert!(run.is_alive());
    }

    #[test]
    fn damage_overflows_shield_to_health() {
        let mut run = RunState::new(Seed::new(42));
        let outcome = run.take_damage(Damage::new(70.0));
        // Shield absorbs 50, health takes 20
        assert_eq!(outcome, DamageOutcome::HullHit);
        assert_eq!(run.shield.current, Shield::new(0.0));
        assert_eq!(run.health, Health::new(80.0));
        assert!(run.is_alive());
    }

    #[test]
    fn exact_shield_depletion_still_holds_the_hull() {
        // Draining the shield to exactly zero with no overflow is a held shield,
        // not a hull hit — the boundary that picks zap vs clang.
        let mut run = RunState::new(Seed::new(42));
        let outcome = run.take_damage(Damage::new(50.0));
        assert_eq!(outcome, DamageOutcome::ShieldHeld);
        assert_eq!(run.shield.current, Shield::new(0.0));
        assert_eq!(run.health, Health::new(100.0));
    }

    #[test]
    fn lethal_damage_kills() {
        let mut run = RunState::new(Seed::new(42));
        // Must overwhelm shield (50) + health (100)
        run.take_damage(Damage::new(200.0));
        assert_eq!(run.health, Health::new(0.0));
        assert!(!run.is_alive());
    }

    #[test]
    fn collision_costs_one_shield_point() {
        // A careless clang against a wall taxes one point off the shield.
        let mut run = RunState::new(Seed::new(42));
        let outcome = run.take_collision_damage();
        assert_eq!(outcome, DamageOutcome::ShieldHeld);
        assert_eq!(run.shield.current, Shield::new(49.0));
        assert_eq!(run.health, Health::new(100.0));
    }

    #[test]
    fn collision_bites_the_hull_once_the_shield_is_down() {
        // Shield drained to zero, the next collision costs a hull point.
        let mut run = RunState::new(Seed::new(42));
        run.take_damage(Damage::new(50.0)); // drain the shield exactly
        let outcome = run.take_collision_damage();
        assert_eq!(outcome, DamageOutcome::HullHit);
        assert_eq!(run.health, Health::new(99.0));
    }

    #[test]
    fn visit_room_reports_only_the_first_visit() {
        let mut run = RunState::new(Seed::new(42));
        assert!(run.visit_room(3), "first visit is new — the map redraws");
        assert_eq!(run.current_room, 3, "visiting tracks the current room");
        assert!(!run.visit_room(3), "re-entering is not new");
        assert!(run.visit_room(5), "another room is new again");
        assert_eq!(run.rooms_visited, vec![3, 5]);
    }

    #[test]
    fn advance_level_resets_per_level_state_and_keeps_the_run() {
        let mut run = RunState::new(Seed::new(42));
        run.collect_cache(CurrencyKind::Components, 7_000);
        run.record_kill(EnemyType::GunDrone);
        run.clear_room(2);
        run.visit_room(2);
        run.take_damage(Damage::new(70.0)); // through the shield into the hull

        run.advance_level();

        assert_eq!(run.current_level, 2);
        assert_eq!(run.kills.total_kills(), 0, "the kill tally is per-level");
        assert!(run.rooms_cleared.is_empty(), "cleared rooms are per-level");
        assert!(run.rooms_visited.is_empty(), "the recon map resets per level");
        assert_eq!(run.current_room, 0);
        assert_eq!(run.health, run.loadout.max_health(), "patched up between levels");
        assert_eq!(run.shield.current, run.shield.max_capacity, "shield recharged");
        assert_eq!(run.components.balance, 7_000, "the bank persists");
    }

    #[test]
    fn the_shield_surge_spends_charges_for_instant_shields() {
        let mut run = RunState::new(Seed::new(42));
        assert!(!run.use_shield_burst(), "an empty rack does nothing");

        run.shield_charges = 3;
        run.take_damage(Damage::new(45.0)); // 50-cap shield down to 5
        let before = run.shield.current.as_f32();
        assert!(run.use_shield_burst(), "a stocked rack fires");
        assert_eq!(run.shield_charges, 2, "one charge spent");
        assert!(run.shield.current.as_f32() >= before + 44.0,
            "the surge lands its ~50 on the spot (got {} from {before})",
            run.shield.current.as_f32());

        // Clamped to capacity: a full shield wastes the overflow, not the rack.
        assert!(run.use_shield_burst());
        assert!(run.shield.current <= run.shield.max_capacity);
        assert_eq!(run.shield_charges, 1);
    }

    #[test]
    fn run_over_restocks_the_owned_rack() {
        let mut run = RunState::new(Seed::new(42));
        run.unlocks.grant(crate::unlocks::Unlock::ShieldBurst);
        run.shield_charges = 0;
        run.apply_death_penalty();
        assert_eq!(run.shield_charges, 3,
            "the owned item comes with three charges on a fresh run");

        let mut never_bought = RunState::new(Seed::new(42));
        never_bought.apply_death_penalty();
        assert_eq!(never_bought.shield_charges, 0, "no item, no charges");
    }

    #[test]
    fn shield_upgrades_raise_the_envelope() {
        let mut run = RunState::new(Seed::new(42));
        let base_cap = run.shield.max_capacity.as_f32();
        run.loadout.add_upgrade(crate::upgrade::Upgrade {
            name: "Shields +10%".to_string(),
            kind: crate::upgrade::UpgradeKind::ShieldCapacity,
            multiplier: 1.10,
        });
        run.refresh_shield();
        assert!((run.shield.max_capacity.as_f32() - base_cap * 1.10).abs() < 0.01,
            "one shield upgrade is +10% capacity, got {} from {}",
            run.shield.max_capacity.as_f32(), base_cap);
        assert_eq!(run.shield.current, run.shield.max_capacity,
            "the rebuilt shield is topped off");

        // The envelope composes hull × trim × upgrades: a trim change must
        // keep the upgrade multiplier.
        run.set_ship_color(ShipColor::Armored);
        assert!(run.shield.max_capacity.as_f32() > base_cap * 1.10,
            "Armored trim on top of the upgrade beats the upgrade alone");
    }

    #[test]
    fn life_loss_resets_the_recon_map() {
        let mut run = RunState::new(Seed::new(42));
        run.lives = 2;
        run.visit_room(4);
        run.apply_life_loss();
        assert!(run.rooms_visited.is_empty(),
            "the restarted level is unexplored again");
    }

    #[test]
    fn clear_room_scores_once() {
        let mut run = RunState::new(Seed::new(42));
        run.clear_room(0);
        run.clear_room(0); // duplicate
        assert_eq!(run.score, 100);
        assert_eq!(run.rooms_cleared.len(), 1);
    }

    #[test]
    fn starts_with_red_laser() {
        let run = RunState::new(Seed::new(42));
        assert_eq!(run.laser_level, LaserLevel::Red);
        assert_eq!(run.laser_damage(), Damage::new(1.0));
    }

    #[test]
    fn starts_at_level_1() {
        let run = RunState::new(Seed::new(42));
        assert_eq!(run.current_level, 1);
    }

    #[test]
    fn starts_with_zero_components_and_organics() {
        let run = RunState::new(Seed::new(42));
        assert_eq!(run.components.balance, 0);
        assert_eq!(run.organics.balance, 0);
    }

    #[test]
    fn record_kill_tracks_but_pays_nothing() {
        // The kill's reward rides the dropped cache — nothing is credited
        // without a pickup.
        let mut run = RunState::new(Seed::new(42));
        run.record_kill(EnemyType::GunDrone);
        assert_eq!(run.kills.count(EnemyType::GunDrone), 1);
        assert_eq!(run.components.balance, 0,
            "kills pay nothing directly; the reward is in the cache");
    }

    #[test]
    fn record_multiple_kills() {
        let mut run = RunState::new(Seed::new(42));
        run.record_kill(EnemyType::GunDrone);
        run.record_kill(EnemyType::GunDrone);
        run.record_kill(EnemyType::QuadShell);
        assert_eq!(run.kills.total_kills(), 3);
        assert_eq!(run.components.balance, 0, "no kill is auto-credited");
    }

    #[test]
    fn collect_cache_credits_the_matching_account() {
        let mut run = RunState::new(Seed::new(42));
        run.collect_cache(CurrencyKind::Components, 800);
        assert_eq!(run.components.balance, 800);
        assert_eq!(run.organics.balance, 0, "a blue cache never credits organics");
        run.collect_cache(CurrencyKind::Organics, 50);
        assert_eq!(run.organics.balance, 50);
        assert_eq!(run.components.balance, 800, "a green cache never credits components");
    }

    #[test]
    fn new_run_flies_the_starter_hull() {
        let run = RunState::new(Seed::new(42));
        assert_eq!(run.ship_type, ShipType::Vanguard);
    }

    #[test]
    fn choosing_a_hull_rebuilds_the_shield_from_its_spec() {
        let mut run = RunState::new(Seed::new(42));
        run.set_ship_type(ShipType::Hive);
        assert_eq!(run.ship_type, ShipType::Hive);
        let expected = 50.0 * ShipType::Hive.spec().shield_capacity_mul;
        assert_eq!(run.shield.max_capacity, Shield::new(expected),
            "the Hive's plating multiplies the base shield");
        assert_eq!(run.shield.current, run.shield.max_capacity, "rebuilt shields start full");

        // Trim styles stack on hulls that support them (the starter).
        run.set_ship_type(ShipType::Vanguard);
        run.set_ship_color(ShipColor::Armored);
        assert_eq!(run.shield.max_capacity, Shield::new(50.0 * 1.4),
            "hull x trim multiply together");
    }

    #[test]
    fn run_over_keeps_the_chosen_hull() {
        let mut run = RunState::new(Seed::new(42));
        run.set_ship_type(ShipType::Talon);
        run.apply_death_penalty();
        assert_eq!(run.ship_type, ShipType::Talon,
            "the hull is bought with green — it survives the run, like the color choice");
    }

    #[test]
    fn new_run_is_standard_ship() {
        let run = RunState::new(Seed::new(42));
        assert_eq!(run.ship_color, ShipColor::Standard);
        assert_eq!(run.shield.max_capacity, Shield::new(50.0));
    }

    #[test]
    fn armored_ship_has_a_bigger_shield() {
        let mut run = RunState::new(Seed::new(42));
        run.set_ship_color(ShipColor::Armored);
        assert_eq!(run.ship_color, ShipColor::Armored);
        assert_eq!(run.shield.max_capacity, Shield::new(50.0 * 1.4));
        assert_eq!(run.shield.current, Shield::new(50.0 * 1.4), "rebuilt shield starts full");
    }

    #[test]
    fn death_keeps_the_chosen_ship_color() {
        let mut run = RunState::new(Seed::new(42));
        run.set_ship_color(ShipColor::Swift);
        run.apply_death_penalty();
        assert_eq!(run.ship_color, ShipColor::Swift);
    }

    #[test]
    fn marking_an_enemy_seen_reports_first_sighting() {
        let mut run = RunState::new(Seed::new(42));
        assert!(run.mark_enemy_seen(EnemyType::GunDrone), "first sighting is new");
        assert!(!run.mark_enemy_seen(EnemyType::GunDrone), "repeat sighting is not new");
        assert!(run.seen_enemies.contains(EnemyType::GunDrone));
    }

    #[test]
    fn unlocks_survive_run_over() {
        use crate::unlocks::Unlock;
        let mut run = RunState::new(Seed::new(42));
        run.unlocks.grant(Unlock::Radar);
        run.apply_death_penalty();
        assert!(run.unlocks.contains(Unlock::Radar),
            "permanent unlocks survive run-over, like the organics that paid for them");
    }

    #[test]
    fn bestiary_is_permanent_across_death() {
        let mut run = RunState::new(Seed::new(42));
        run.mark_enemy_seen(EnemyType::QuadShell);
        run.apply_death_penalty();
        assert!(run.seen_enemies.contains(EnemyType::QuadShell),
            "the bestiary survives death, like organics");
    }

    #[test]
    fn death_preserves_organics_but_clears_components() {
        let mut run = RunState::new(Seed::new(42));
        run.collect_cache(CurrencyKind::Components, 800);
        run.collect_cache(CurrencyKind::Organics, 40);
        run.apply_death_penalty();
        assert_eq!(run.components.balance, 0, "components are lost on death");
        assert_eq!(run.organics.balance, 40, "organics are permanent");
    }

    #[test]
    fn run_over_resets_the_laser_fully_to_red() {
        // Owner's call (2026-07-04): run-over is a full blue reset — the
        // laser drops to Red outright, not one step. Greens/reds survive.
        let mut run = RunState::new(Seed::new(42));
        run.laser_level = LaserLevel::Violet; // the very top
        run.components.earn(50_000);
        run.record_kill(EnemyType::GunDrone);
        run.current_level = 5;

        run.apply_death_penalty();

        assert_eq!(run.laser_level, LaserLevel::Red, "all the way down");
        assert_eq!(run.components.balance, 0);
        assert_eq!(run.kills.total_kills(), 0);
        assert_eq!(run.current_level, 1);
    }

    #[test]
    fn new_run_has_one_life_and_none_purchased() {
        let run = RunState::new(Seed::new(42));
        assert_eq!(run.lives, 1);
        assert_eq!(run.lives_purchased, 0);
        assert!(!run.has_spare_life(), "the starting life is not spare");
    }

    #[test]
    fn life_loss_restores_ship_and_keeps_progress() {
        let mut run = RunState::new(Seed::new(42));
        run.lives = 3;
        run.collect_cache(CurrencyKind::Components, 12_000);
        run.loadout.add_upgrade(crate::upgrade::Upgrade {
            name: "Thrust +10%".to_string(),
            kind: crate::upgrade::UpgradeKind::Thrust,
            multiplier: 1.10,
        });
        run.laser_level = LaserLevel::Green;
        run.current_level = 5;
        run.take_damage(Damage::new(120.0)); // through the shield into the hull

        run.apply_life_loss();

        assert_eq!(run.lives, 2, "a life was spent");
        assert!(run.has_spare_life());
        assert_eq!(run.health, run.loadout.max_health(), "hull restored");
        assert_eq!(run.shield.current, run.shield.max_capacity, "shield restored");
        assert_eq!(run.components.balance, 12_000, "banked components keep");
        assert_eq!(run.loadout.upgrades.len(), 1, "bought upgrades keep");
        assert_eq!(run.laser_level, LaserLevel::Green, "no laser penalty on a life");
        assert_eq!(run.current_level, 5, "the run stays at its level");
    }

    #[test]
    fn run_over_clears_bought_upgrades() {
        let mut run = RunState::new(Seed::new(42));
        run.loadout.add_upgrade(crate::upgrade::Upgrade {
            name: "Armor +10%".to_string(),
            kind: crate::upgrade::UpgradeKind::MaxHealth,
            multiplier: 1.10,
        });
        run.apply_death_penalty();
        assert!(run.loadout.upgrades.is_empty(),
            "blue purchases die with the run — the salvage is lost");
    }

    #[test]
    fn run_over_resets_lives_and_the_price_ratchet() {
        let mut run = RunState::new(Seed::new(42));
        run.lives = 4;
        run.lives_purchased = 3;
        run.apply_death_penalty();
        assert_eq!(run.lives, 1, "a new run starts with the one life");
        assert_eq!(run.lives_purchased, 0, "the price ratchet resets with the run");
    }

    #[test]
    fn death_penalty_min_red() {
        let mut run = RunState::new(Seed::new(42));
        run.laser_level = LaserLevel::Red;
        run.apply_death_penalty();
        assert_eq!(run.laser_level, LaserLevel::Red);
    }

    #[test]
    fn level_seed_varies_with_run_seed() {
        let a = RunState::new(Seed::new(42));
        let b = RunState::new(Seed::new(999));
        assert_ne!(a.level_seed(), b.level_seed(),
            "different run seeds must produce different level seeds");
    }

    #[test]
    fn level_seed_varies_with_level() {
        let mut run = RunState::new(Seed::new(42));
        let seed1 = run.level_seed();
        run.current_level = 2;
        let seed2 = run.level_seed();
        assert_ne!(seed1, seed2,
            "different levels must produce different seeds");
    }

    #[test]
    fn different_seeds_produce_different_levels() {
        use crate::generator::{generate, GeneratorConfig};

        let config_a = GeneratorConfig {
            pitch: crate::planet::Pitch { tile: 4.0, story: 5.0 },
            seed: RunState::new(Seed::new(42)).level_seed(),
            max_rooms: 10, min_room_xz: 3, max_room_xz: 6,
            min_room_y: 1, max_room_y: 6,
        };
        let config_b = GeneratorConfig {
            pitch: crate::planet::Pitch { tile: 4.0, story: 5.0 },
            seed: RunState::new(Seed::new(999)).level_seed(),
            max_rooms: 10, min_room_xz: 3, max_room_xz: 6,
            min_room_y: 1, max_room_y: 6,
        };

        let graph_a = generate(&config_a).unwrap();
        let graph_b = generate(&config_b).unwrap();

        // Collect room grid positions for each graph.
        let positions = |g: &crate::level_graph::LevelGraph| -> Vec<[i32; 3]> {
            g.room_indices()
                .filter_map(|idx| g.room(idx))
                .map(|r| r.grid_pos)
                .collect()
        };
        assert_ne!(positions(&graph_a), positions(&graph_b),
            "different run seeds must produce different level layouts");
    }
}
