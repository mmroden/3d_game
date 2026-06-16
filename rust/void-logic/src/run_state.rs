use crate::currency::{ComponentAccount, OrganicAccount};
use crate::enemy_type::EnemyType;
use crate::kill_tracker::KillTracker;
use crate::laser::LaserLevel;
use crate::loadout::Loadout;
use crate::newtypes::{Health, Damage, Shield};
use crate::seed::Seed;
use crate::shield::ShieldState;

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
}

impl RunState {
    /// Default shield: 50 capacity, 2/sec regen, 3s delay after hit.
    const DEFAULT_SHIELD_CAPACITY: f32 = 50.0;
    // Slower regen and a longer post-hit delay so sustained fire actually
    // wears the player down instead of the shield topping back up between shots.
    const DEFAULT_SHIELD_REGEN: f32 = 1.5;
    const DEFAULT_SHIELD_DELAY: f32 = 5.0;

    /// Derive the seed for the current level from the run seed.
    pub fn level_seed(&self) -> Seed {
        self.run_seed.for_level(self.current_level)
    }

    pub fn new(seed: Seed) -> Self {
        let loadout = Loadout::new();
        let health = loadout.max_health();
        let shield = ShieldState::new(
            Shield::new(Self::DEFAULT_SHIELD_CAPACITY),
            Self::DEFAULT_SHIELD_REGEN,
            Self::DEFAULT_SHIELD_DELAY,
        );
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
        }
    }

    pub fn is_alive(&self) -> bool {
        self.health.is_alive()
    }

    pub fn take_damage(&mut self, amount: Damage) {
        let overflow = self.shield.take_hit(amount);
        self.health = self.health.take(overflow);
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
        run.take_damage(Damage::new(30.0));
        // Shield absorbs 30 of 50, health untouched
        assert_eq!(run.shield.current, Shield::new(20.0));
        assert_eq!(run.health, Health::new(100.0));
        assert!(run.is_alive());
    }

    #[test]
    fn damage_overflows_shield_to_health() {
        let mut run = RunState::new(Seed::new(42));
        run.take_damage(Damage::new(70.0));
        // Shield absorbs 50, health takes 20
        assert_eq!(run.shield.current, Shield::new(0.0));
        assert_eq!(run.health, Health::new(80.0));
        assert!(run.is_alive());
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
