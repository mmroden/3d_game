use crate::bestiary::SeenEnemies;
use crate::currency::{ComponentAccount, OrganicAccount};
use crate::enemy_type::EnemyType;
use crate::kill_tracker::KillTracker;
use crate::laser::LaserLevel;
use crate::loadout::Loadout;
use crate::newtypes::{Health, Damage, Shield};
use crate::seed::Seed;
use crate::shield::ShieldState;
use crate::ship::ShipColor;

/// Which defensive layer absorbed a hit. Drives impact SFX: a held shield
/// plays the energy zap, a hull hit plays the heavy metal clang.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageOutcome {
    /// The shield absorbed the whole hit; the hull was untouched.
    ShieldHeld,
    /// Damage overflowed the shield and reached the hull.
    HullHit,
}

/// Tracks the state of a single roguelike run.
#[derive(Debug)]
pub struct RunState {
    pub loadout: Loadout,
    pub current_room: usize,
    pub rooms_cleared: Vec<usize>,
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
    /// Chosen ship color — a loadout tradeoff that shapes shields and thrust.
    pub ship_color: ShipColor,
    /// Enemy types catalogued in the bestiary. Permanent: an enemy is marked on
    /// first sighting and survives death, like organics.
    pub seen_enemies: SeenEnemies,
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

    /// Build a shield sized and tuned for the given ship color.
    fn shield_for(color: ShipColor) -> ShieldState {
        ShieldState::new(
            Shield::new(Self::DEFAULT_SHIELD_CAPACITY * color.shield_capacity_mul()),
            Self::DEFAULT_SHIELD_REGEN * color.shield_regen_mul(),
            Self::DEFAULT_SHIELD_DELAY,
        )
    }

    pub fn new(seed: Seed) -> Self {
        let loadout = Loadout::new();
        let health = loadout.max_health();
        let ship_color = ShipColor::default();
        let shield = Self::shield_for(ship_color);
        Self {
            loadout,
            current_room: 0,
            rooms_cleared: Vec::new(),
            health,
            shield,
            score: 0,
            run_seed: seed,
            components: ComponentAccount::new(),
            organics: OrganicAccount::new(),
            kills: KillTracker::new(),
            laser_level: LaserLevel::Red,
            current_level: 1,
            ship_color,
            seen_enemies: SeenEnemies::new(),
        }
    }

    /// Choose a ship color, rebuilding the shield to its capacity/regen.
    pub fn set_ship_color(&mut self, color: ShipColor) {
        self.ship_color = color;
        self.shield = Self::shield_for(color);
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

    pub fn clear_room(&mut self, room_index: usize) {
        if !self.rooms_cleared.contains(&room_index) {
            self.rooms_cleared.push(room_index);
            self.score += 100;
        }
    }

    /// Record an enemy kill: track it and earn components (all enemies are mechanical).
    pub fn record_kill(&mut self, enemy_type: EnemyType) {
        let reward = enemy_type.stats().reward;
        self.kills.record_kill(enemy_type);
        self.components.earn(reward);
    }

    /// Collect organics from a barrel pickup. Permanent currency.
    pub fn collect_organics(&mut self, amount: u32) {
        self.organics.earn(amount);
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

    /// Apply death penalty: halve laser level, reset components and kills.
    /// Organics are permanent and deliberately preserved.
    pub fn apply_death_penalty(&mut self) {
        self.laser_level = self.laser_level.downgrade();
        self.components = ComponentAccount::new();
        self.kills.reset();
        self.current_level = 1;
        self.rooms_cleared.clear();
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
    fn record_kill_earns_components() {
        let mut run = RunState::new(Seed::new(42));
        run.record_kill(EnemyType::GunDrone);
        assert_eq!(run.components.balance, 1_000);
        assert_eq!(run.kills.count(EnemyType::GunDrone), 1);
    }

    #[test]
    fn record_multiple_kills() {
        let mut run = RunState::new(Seed::new(42));
        run.record_kill(EnemyType::GunDrone);
        run.record_kill(EnemyType::GunDrone);
        run.record_kill(EnemyType::QuadShell);
        assert_eq!(run.components.balance, 3_000);
        assert_eq!(run.kills.total_kills(), 3);
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
    fn bestiary_is_permanent_across_death() {
        let mut run = RunState::new(Seed::new(42));
        run.mark_enemy_seen(EnemyType::QuadShell);
        run.apply_death_penalty();
        assert!(run.seen_enemies.contains(EnemyType::QuadShell),
            "the bestiary survives death, like organics");
    }

    #[test]
    fn collect_organics_accumulates() {
        let mut run = RunState::new(Seed::new(42));
        run.collect_organics(50);
        run.collect_organics(25);
        assert_eq!(run.organics.balance, 75);
    }

    #[test]
    fn death_preserves_organics_but_clears_components() {
        let mut run = RunState::new(Seed::new(42));
        run.record_kill(EnemyType::GunDrone); // components
        run.collect_organics(40); // organics
        run.apply_death_penalty();
        assert_eq!(run.components.balance, 0, "components are lost on death");
        assert_eq!(run.organics.balance, 40, "organics are permanent");
    }

    #[test]
    fn death_penalty_halves_laser() {
        let mut run = RunState::new(Seed::new(42));
        run.laser_level = LaserLevel::Green; // level 4
        run.components.earn(50_000);
        run.record_kill(EnemyType::GunDrone);
        run.current_level = 5;

        run.apply_death_penalty();

        assert_eq!(run.laser_level, LaserLevel::Orange); // 4/2=2
        assert_eq!(run.components.balance, 0);
        assert_eq!(run.kills.total_kills(), 0);
        assert_eq!(run.current_level, 1);
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
            seed: RunState::new(Seed::new(42)).level_seed(),
            max_rooms: 10, min_room_xz: 3, max_room_xz: 6,
            min_room_y: 1, max_room_y: 6,
        };
        let config_b = GeneratorConfig {
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
