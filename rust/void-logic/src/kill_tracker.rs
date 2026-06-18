//! Tracks enemy kills by type for the level summary screen.

use std::collections::HashMap;
use crate::enemy_type::EnemyType;

#[derive(Debug, Clone)]
pub struct KillTracker {
    kills: HashMap<EnemyType, u32>,
}

impl KillTracker {
    pub fn new() -> Self {
        Self { kills: HashMap::new() }
    }

    pub fn record_kill(&mut self, enemy_type: EnemyType) {
        *self.kills.entry(enemy_type).or_insert(0) += 1;
    }

    pub fn count(&self, enemy_type: EnemyType) -> u32 {
        self.kills.get(&enemy_type).copied().unwrap_or(0)
    }

    pub fn total_kills(&self) -> u32 {
        self.kills.values().sum()
    }

    /// Total component reward earned (1,000 per kill).
    pub fn total_reward(&self) -> u32 {
        self.total_kills() * 1_000
    }

    /// Summary sorted by enemy type ordering (weakest to strongest), skipping zeros.
    pub fn summary(&self) -> Vec<(EnemyType, u32)> {
        EnemyType::ALL
            .iter()
            .filter_map(|e| {
                let count = self.count(*e);
                if count > 0 { Some((*e, count)) } else { None }
            })
            .collect()
    }

    pub fn reset(&mut self) {
        self.kills.clear();
    }
}

impl Default for KillTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty() {
        let tracker = KillTracker::new();
        assert_eq!(tracker.total_kills(), 0);
        assert_eq!(tracker.total_reward(), 0);
        assert!(tracker.summary().is_empty());
    }

    #[test]
    fn record_and_count() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(EnemyType::GunDrone);
        tracker.record_kill(EnemyType::GunDrone);
        tracker.record_kill(EnemyType::QuadShell);
        assert_eq!(tracker.count(EnemyType::GunDrone), 2);
        assert_eq!(tracker.count(EnemyType::QuadShell), 1);
        assert_eq!(tracker.count(EnemyType::QuadOrb), 0);
    }

    #[test]
    fn total_kills() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(EnemyType::GunDrone);
        tracker.record_kill(EnemyType::GunDrone);
        tracker.record_kill(EnemyType::QuadOrb);
        assert_eq!(tracker.total_kills(), 3);
    }

    #[test]
    fn total_reward_1000_per_kill() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(EnemyType::GunDrone);
        tracker.record_kill(EnemyType::QuadShell);
        assert_eq!(tracker.total_reward(), 2_000);
    }

    #[test]
    fn summary_skips_zeros() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(EnemyType::QuadShell);
        tracker.record_kill(EnemyType::QuadShell);
        let summary = tracker.summary();
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0], (EnemyType::QuadShell, 2));
    }

    #[test]
    fn summary_ordered_by_enemy_type() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(EnemyType::QuadShell);
        tracker.record_kill(EnemyType::GunDrone);
        tracker.record_kill(EnemyType::Bomber);
        let summary = tracker.summary();
        assert_eq!(summary[0].0, EnemyType::GunDrone);
        assert_eq!(summary[1].0, EnemyType::Bomber);
        assert_eq!(summary[2].0, EnemyType::QuadShell);
    }

    #[test]
    fn reset_clears_all() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(EnemyType::GunDrone);
        tracker.reset();
        assert_eq!(tracker.total_kills(), 0);
        assert!(tracker.summary().is_empty());
    }
}
