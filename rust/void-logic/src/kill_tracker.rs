//! Tracks enemy kills for the level summary screen, keyed by the grammar's
//! append-only crossing id (open-set identity: the tracker never needs to
//! know the full roster, only what actually died).

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct KillTracker {
    kills: HashMap<u16, u32>,
}

impl KillTracker {
    pub fn new() -> Self {
        Self { kills: HashMap::new() }
    }

    pub fn record_kill(&mut self, crossing_id: u16) {
        *self.kills.entry(crossing_id).or_insert(0) += 1;
    }

    pub fn count(&self, crossing_id: u16) -> u32 {
        self.kills.get(&crossing_id).copied().unwrap_or(0)
    }

    pub fn total_kills(&self) -> u32 {
        self.kills.values().sum()
    }

    /// Total component reward earned (1,000 per kill).
    pub fn total_reward(&self) -> u32 {
        self.total_kills() * 1_000
    }

    /// Summary in roster declaration order, skipping zeros. Kills of ids the
    /// grammar no longer declares are counted in totals but not listed.
    pub fn summary(&self) -> Vec<(u16, u32)> {
        crate::roster::roster()
            .enemy_ids()
            .filter_map(|id| {
                let crossing = crate::roster::roster().enemy(id).crossing_id;
                let count = self.count(crossing);
                if count > 0 { Some((crossing, count)) } else { None }
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

    fn cid(key: &str) -> u16 {
        let r = crate::roster::roster();
        r.enemy(r.enemy_by_key(key).expect(key)).crossing_id
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
        tracker.record_kill(cid("gun_drone"));
        tracker.record_kill(cid("gun_drone"));
        tracker.record_kill(cid("quad_shell"));
        assert_eq!(tracker.count(cid("gun_drone")), 2);
        assert_eq!(tracker.count(cid("quad_shell")), 1);
        assert_eq!(tracker.count(cid("quad_orb")), 0);
    }

    #[test]
    fn total_kills() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(cid("gun_drone"));
        tracker.record_kill(cid("gun_drone"));
        tracker.record_kill(cid("quad_orb"));
        assert_eq!(tracker.total_kills(), 3);
    }

    #[test]
    fn total_reward_1000_per_kill() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(cid("gun_drone"));
        tracker.record_kill(cid("quad_shell"));
        assert_eq!(tracker.total_reward(), 2_000);
    }

    #[test]
    fn summary_skips_zeros() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(cid("quad_shell"));
        tracker.record_kill(cid("quad_shell"));
        let summary = tracker.summary();
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0], (cid("quad_shell"), 2));
    }

    #[test]
    fn summary_ordered_by_roster_declaration() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(cid("quad_shell"));
        tracker.record_kill(cid("gun_drone"));
        tracker.record_kill(cid("bomber"));
        let summary = tracker.summary();
        assert_eq!(summary[0].0, cid("gun_drone"));
        assert_eq!(summary[1].0, cid("bomber"));
        assert_eq!(summary[2].0, cid("quad_shell"));
    }

    #[test]
    fn a_retired_id_still_counts_toward_totals() {
        // Open-set tolerance: mid-run roster edits (dev flow) or a stale
        // id can't crash the summary — unlisted, but the kill happened.
        let mut tracker = KillTracker::new();
        tracker.record_kill(9999);
        tracker.record_kill(cid("bomber"));
        assert_eq!(tracker.total_kills(), 2);
        assert_eq!(tracker.summary().len(), 1);
    }

    #[test]
    fn reset_clears_all() {
        let mut tracker = KillTracker::new();
        tracker.record_kill(cid("gun_drone"));
        tracker.reset();
        assert_eq!(tracker.total_kills(), 0);
        assert!(tracker.summary().is_empty());
    }
}
