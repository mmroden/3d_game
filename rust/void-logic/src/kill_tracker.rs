//! Tracks enemy kills for the level summary screen, keyed by the grammar's
//! append-only crossing id (open-set identity: the tracker never needs to
//! know the full roster, only what actually died).

use std::collections::HashMap;

use crate::roster::EnemyKey;

#[derive(Debug, Clone, Default)]
pub struct KillTracker {
    kills: HashMap<EnemyKey, u32>,
}

impl KillTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_kill(&mut self, key: EnemyKey) {
        *self.kills.entry(key).or_insert(0) += 1;
    }

    pub fn count(&self, key: EnemyKey) -> u32 {
        self.kills.get(&key).copied().unwrap_or(0)
    }

    pub fn total_kills(&self) -> u32 {
        self.kills.values().sum()
    }

    /// Total component reward earned (1,000 per kill).
    pub fn total_reward(&self) -> u32 {
        self.total_kills() * 1_000
    }

    /// Summary in roster declaration order, skipping zeros. Kills of keys the
    /// grammar no longer declares are counted in totals but not listed.
    pub fn summary(&self) -> Vec<(EnemyKey, u32)> {
        crate::roster::roster()
            .enemy_keys()
            .filter_map(|key| {
                let count = self.count(key);
                if count > 0 { Some((key, count)) } else { None }
            })
            .collect()
    }

    pub fn reset(&mut self) {
        self.kills.clear();
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// The n-th declared enemy, by roster position — these tests need SOME
    /// distinct enemies to tally, never a particular one. The grammar
    /// declares which exist; retuning the roster can't touch this.
    fn nth_enemy(n: usize) -> crate::roster::EnemyKey {
        crate::roster::roster()
            .enemy_keys()
            .nth(n)
            .expect("the grammar declares enough enemies")
    }

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
        tracker.record_kill(nth_enemy(0));
        tracker.record_kill(nth_enemy(0));
        tracker.record_kill(nth_enemy(1));
        assert_eq!(tracker.count(nth_enemy(0)), 2);
        assert_eq!(tracker.count(nth_enemy(1)), 1);
        assert_eq!(tracker.count(nth_enemy(2)), 0);
    }

    #[test]
    fn total_kills() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(nth_enemy(0));
        tracker.record_kill(nth_enemy(0));
        tracker.record_kill(nth_enemy(2));
        assert_eq!(tracker.total_kills(), 3);
    }

    #[test]
    fn total_reward_1000_per_kill() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(nth_enemy(0));
        tracker.record_kill(nth_enemy(1));
        assert_eq!(tracker.total_reward(), 2_000);
    }

    #[test]
    fn summary_skips_zeros() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(nth_enemy(1));
        tracker.record_kill(nth_enemy(1));
        let summary = tracker.summary();
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0], (nth_enemy(1), 2));
    }

    #[test]
    fn reset_clears_all() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(nth_enemy(0));
        tracker.reset();
        assert_eq!(tracker.total_kills(), 0);
        assert!(tracker.summary().is_empty());
    }
}
