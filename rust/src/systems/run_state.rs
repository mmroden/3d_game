use crate::systems::loadout::Loadout;

/// Tracks the state of a single roguelike run.
#[derive(Debug)]
pub struct RunState {
    pub loadout: Loadout,
    pub current_room: usize,
    pub rooms_cleared: Vec<usize>,
    pub health: f32,
    pub score: u32,
    pub run_seed: u64,
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
        }
    }

    pub fn is_alive(&self) -> bool {
        self.health > 0.0
    }

    pub fn take_damage(&mut self, amount: f32) {
        self.health = (self.health - amount).max(0.0);
    }

    pub fn clear_room(&mut self, room_index: usize) {
        if !self.rooms_cleared.contains(&room_index) {
            self.rooms_cleared.push(room_index);
            self.score += 100;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_run_starts_alive() {
        let run = RunState::new(42);
        assert!(run.is_alive());
        assert_eq!(run.health, 100.0);
        assert_eq!(run.score, 0);
    }

    #[test]
    fn damage_reduces_health() {
        let mut run = RunState::new(42);
        run.take_damage(30.0);
        assert_eq!(run.health, 70.0);
        assert!(run.is_alive());
    }

    #[test]
    fn lethal_damage_kills() {
        let mut run = RunState::new(42);
        run.take_damage(150.0);
        assert_eq!(run.health, 0.0);
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
}
