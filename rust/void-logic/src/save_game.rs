use crate::credits::CreditAccount;
use crate::laser::LaserLevel;
use crate::loadout::Loadout;
use crate::run_state::RunState;

/// A snapshot of game state, saved at end-of-level and on death.
#[derive(Debug, Clone, PartialEq)]
pub struct SaveGame {
    pub laser_level: LaserLevel,
    pub loadout: Loadout,
    pub score: u32,
    pub current_level: u32,
    pub run_seed: u64,
    pub credits: CreditAccount,
    pub health: f32,
}

impl SaveGame {
    pub fn from_run_state(run: &RunState) -> Self {
        Self {
            laser_level: run.laser_level,
            loadout: run.loadout.clone(),
            score: run.score,
            current_level: run.current_level,
            run_seed: run.run_seed,
            credits: run.credits.clone(),
            health: run.health,
        }
    }

    pub fn apply_to(&self, run: &mut RunState) {
        run.laser_level = self.laser_level;
        run.loadout = self.loadout.clone();
        run.score = self.score;
        run.current_level = self.current_level;
        run.run_seed = self.run_seed;
        run.credits = self.credits.clone();
        run.health = self.health;
        // Reset ephemeral state
        run.kills.reset();
        run.rooms_cleared.clear();
        run.current_room = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_captures_laser_level() {
        let mut run = RunState::new(42);
        run.laser_level = LaserLevel::Green;
        let save = SaveGame::from_run_state(&run);
        assert_eq!(save.laser_level, LaserLevel::Green);
    }

    #[test]
    fn snapshot_captures_current_level() {
        let mut run = RunState::new(42);
        run.current_level = 4;
        let save = SaveGame::from_run_state(&run);
        assert_eq!(save.current_level, 4);
    }

    #[test]
    fn snapshot_captures_credits() {
        let mut run = RunState::new(42);
        run.credits.earn(5_000);
        let save = SaveGame::from_run_state(&run);
        assert_eq!(save.credits.balance, 5_000);
    }

    #[test]
    fn snapshot_captures_score() {
        let mut run = RunState::new(42);
        run.score = 300;
        let save = SaveGame::from_run_state(&run);
        assert_eq!(save.score, 300);
    }

    #[test]
    fn snapshot_captures_health() {
        let mut run = RunState::new(42);
        run.take_damage(25.0);
        let save = SaveGame::from_run_state(&run);
        assert_eq!(save.health, 75.0);
    }

    #[test]
    fn apply_restores_laser_level() {
        let mut run = RunState::new(42);
        run.laser_level = LaserLevel::Green;
        let save = SaveGame::from_run_state(&run);

        let mut fresh = RunState::new(99);
        save.apply_to(&mut fresh);
        assert_eq!(fresh.laser_level, LaserLevel::Green);
    }

    #[test]
    fn apply_restores_current_level() {
        let mut run = RunState::new(42);
        run.current_level = 4;
        let save = SaveGame::from_run_state(&run);

        let mut fresh = RunState::new(99);
        save.apply_to(&mut fresh);
        assert_eq!(fresh.current_level, 4);
    }

    #[test]
    fn apply_restores_credits() {
        let mut run = RunState::new(42);
        run.credits.earn(5_000);
        let save = SaveGame::from_run_state(&run);

        let mut fresh = RunState::new(99);
        save.apply_to(&mut fresh);
        assert_eq!(fresh.credits.balance, 5_000);
    }

    #[test]
    fn apply_resets_ephemeral_state() {
        let mut run = RunState::new(42);
        run.current_level = 3;
        run.record_kill(crate::enemy_type::EnemyType::GunDrone);
        run.clear_room(0);
        let save = SaveGame::from_run_state(&run);

        let mut fresh = RunState::new(99);
        fresh.record_kill(crate::enemy_type::EnemyType::Bat);
        fresh.clear_room(1);
        save.apply_to(&mut fresh);

        assert_eq!(fresh.kills.total_kills(), 0);
        assert!(fresh.rooms_cleared.is_empty());
        assert_eq!(fresh.current_room, 0);
    }

    #[test]
    fn continue_from_level_4_save() {
        let mut run = RunState::new(42);
        run.laser_level = LaserLevel::Green;
        run.current_level = 4;
        run.credits.earn(5_000);
        let save = SaveGame::from_run_state(&run);

        let mut fresh = RunState::new(99);
        save.apply_to(&mut fresh);

        assert_eq!(fresh.current_level, 4);
        assert_eq!(fresh.laser_level, LaserLevel::Green);
        assert_eq!(fresh.credits.balance, 5_000);
    }

    #[test]
    fn continue_from_death_save() {
        let mut run = RunState::new(42);
        run.laser_level = LaserLevel::Green; // level 4
        run.current_level = 4;
        run.credits.earn(10_000);
        run.apply_death_penalty();
        let save = SaveGame::from_run_state(&run);

        let mut fresh = RunState::new(99);
        save.apply_to(&mut fresh);

        assert_eq!(fresh.current_level, 1);
        assert_eq!(fresh.laser_level, LaserLevel::Orange); // Green(4) halved = 2 = Orange
        assert_eq!(fresh.credits.balance, 0);
    }
}
