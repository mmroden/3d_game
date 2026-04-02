use crate::credits::CreditAccount;
use crate::enemy_type::EnemyType;
use crate::kill_tracker::KillTracker;
use crate::laser::LaserLevel;
use crate::loadout::Loadout;
use crate::newtypes::{Health, Damage};

/// Tracks the state of a single roguelike run.
#[derive(Debug)]
pub struct RunState {
    pub loadout: Loadout,
    pub current_room: usize,
    pub rooms_cleared: Vec<usize>,
    pub health: Health,
    pub score: u32,
    pub run_seed: u64,
    pub credits: CreditAccount,
    pub kills: KillTracker,
    pub laser_level: LaserLevel,
    pub current_level: u32,
}

impl RunState {
    pub fn new(seed: u64) -> Self {
        let loadout = Loadout::new();
        let health = loadout.max_health();
        Self {
            loadout,
            current_room: 0,
            rooms_cleared: Vec::new(),
            health,
            score: 0,
            run_seed: seed,
            credits: CreditAccount::new(),
            kills: KillTracker::new(),
            laser_level: LaserLevel::Red,
            current_level: 1,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.health.is_alive()
    }

    pub fn take_damage(&mut self, amount: Damage) {
        self.health = self.health.take(amount);
    }

    pub fn clear_room(&mut self, room_index: usize) {
        if !self.rooms_cleared.contains(&room_index) {
            self.rooms_cleared.push(room_index);
            self.score += 100;
        }
    }

    /// Record an enemy kill: track it and earn credits.
    pub fn record_kill(&mut self, enemy_type: EnemyType) {
        let credits = enemy_type.stats().credits;
        self.kills.record_kill(enemy_type);
        self.credits.earn(credits);
    }

    /// Current laser damage per beam.
    pub fn laser_damage(&self) -> Damage {
        Damage::new(self.laser_level.damage())
    }

    /// Apply death penalty: halve laser level, reset credits and kills.
    pub fn apply_death_penalty(&mut self) {
        self.laser_level = self.laser_level.downgrade();
        self.credits = CreditAccount::new();
        self.kills.reset();
        self.current_level = 1;
        self.rooms_cleared.clear();
        self.current_room = 0;
        self.health = self.loadout.max_health();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_run_starts_alive() {
        let run = RunState::new(42);
        assert!(run.is_alive());
        assert_eq!(run.health, Health::new(100.0));
        assert_eq!(run.score, 0);
    }

    #[test]
    fn damage_reduces_health() {
        let mut run = RunState::new(42);
        run.take_damage(Damage::new(30.0));
        assert_eq!(run.health, Health::new(70.0));
        assert!(run.is_alive());
    }

    #[test]
    fn lethal_damage_kills() {
        let mut run = RunState::new(42);
        run.take_damage(Damage::new(150.0));
        assert_eq!(run.health, Health::new(0.0));
        assert!(!run.is_alive());
    }

    #[test]
    fn clear_room_scores_once() {
        let mut run = RunState::new(42);
        run.clear_room(0);
        run.clear_room(0); // duplicate
        assert_eq!(run.score, 100);
        assert_eq!(run.rooms_cleared.len(), 1);
    }

    #[test]
    fn starts_with_red_laser() {
        let run = RunState::new(42);
        assert_eq!(run.laser_level, LaserLevel::Red);
        assert_eq!(run.laser_damage(), Damage::new(1.0));
    }

    #[test]
    fn starts_at_level_1() {
        let run = RunState::new(42);
        assert_eq!(run.current_level, 1);
    }

    #[test]
    fn starts_with_zero_credits() {
        let run = RunState::new(42);
        assert_eq!(run.credits.balance, 0);
    }

    #[test]
    fn record_kill_earns_credits() {
        let mut run = RunState::new(42);
        run.record_kill(EnemyType::GunDrone);
        assert_eq!(run.credits.balance, 1_000);
        assert_eq!(run.kills.count(EnemyType::GunDrone), 1);
    }

    #[test]
    fn record_multiple_kills() {
        let mut run = RunState::new(42);
        run.record_kill(EnemyType::GunDrone);
        run.record_kill(EnemyType::GunDrone);
        run.record_kill(EnemyType::Dragon);
        assert_eq!(run.credits.balance, 3_000);
        assert_eq!(run.kills.total_kills(), 3);
    }

    #[test]
    fn death_penalty_halves_laser() {
        let mut run = RunState::new(42);
        run.laser_level = LaserLevel::Green; // level 4
        run.credits.earn(50_000);
        run.record_kill(EnemyType::GunDrone);
        run.current_level = 5;

        run.apply_death_penalty();

        assert_eq!(run.laser_level, LaserLevel::Orange); // 4/2=2
        assert_eq!(run.credits.balance, 0);
        assert_eq!(run.kills.total_kills(), 0);
        assert_eq!(run.current_level, 1);
    }

    #[test]
    fn death_penalty_min_red() {
        let mut run = RunState::new(42);
        run.laser_level = LaserLevel::Red;
        run.apply_death_penalty();
        assert_eq!(run.laser_level, LaserLevel::Red);
    }
}
