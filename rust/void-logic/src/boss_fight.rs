//! Boss-fight sub-state: pure FSM owned by GameManager, driven by shell
//! triggers (arena entry, boss death, reward pickup). Not a `GamePhase` —
//! the game stays in `Playing` throughout; this machine only decides the
//! gate, the music context, and the portal.

/// The four beats of a staged fight. Transitions are one-way and strictly
/// ordered — a boss fight never rewinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossFightState {
    /// Boss level built, arena sealed off ahead, gate open inbound.
    Dormant,
    /// The player crossed the corridor mouth: gate solid behind them, boss
    /// and OnEngage escorts live, boss music context active.
    Engaged,
    /// Boss down, reward cache live — the gate STAYS sealed until the
    /// reward is taken; the fight ends at the pickup, not the kill.
    Defeated,
    /// Reward collected: gate open, portal active, music back to gameplay.
    RewardCollected,
}

/// The fight machine. Every mutator returns whether the transition was
/// legal; illegal calls are refused no-ops so a stray shell signal (a
/// second trigger volume touch, a duplicate death report) can never skip
/// or rewind a beat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BossFight {
    state: BossFightState,
}

impl BossFight {
    pub fn new() -> Self {
        Self { state: BossFightState::Dormant }
    }

    pub fn state(&self) -> BossFightState {
        self.state
    }

    /// The player crossed into the arena approach.
    pub fn engage(&mut self) -> bool {
        self.advance(BossFightState::Dormant, BossFightState::Engaged)
    }

    /// The boss died.
    pub fn defeat(&mut self) -> bool {
        self.advance(BossFightState::Engaged, BossFightState::Defeated)
    }

    /// The reward cache was picked up.
    pub fn collect_reward(&mut self) -> bool {
        self.advance(BossFightState::Defeated, BossFightState::RewardCollected)
    }

    /// Whether the arena gate is solid right now: from the moment the fight
    /// starts until the reward is taken — the kill alone opens nothing.
    pub fn is_sealed(&self) -> bool {
        matches!(
            self.state,
            BossFightState::Engaged | BossFightState::Defeated
        )
    }

    /// Whether the exit portal is live right now.
    pub fn portal_active(&self) -> bool {
        self.state == BossFightState::RewardCollected
    }

    fn advance(&mut self, from: BossFightState, to: BossFightState) -> bool {
        if self.state == from {
            self.state = to;
            true
        } else {
            false
        }
    }
}

impl BossFightState {
    /// GDScript crossing (append-only law): -1 is reserved by convention for
    /// "no boss fight on this level" on the shell side.
    pub fn id(self) -> i32 {
        match self {
            Self::Dormant => 0,
            Self::Engaged => 1,
            Self::Defeated => 2,
            Self::RewardCollected => 3,
        }
    }
}

impl Default for BossFight {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_dormant_with_the_gate_open_and_the_portal_dark() {
        let f = BossFight::new();
        assert_eq!(f.state(), BossFightState::Dormant);
        assert!(!f.is_sealed(), "the approach is passable inbound");
        assert!(!f.portal_active(), "no exit until the fight is done");
    }

    #[test]
    fn the_fight_walks_its_only_legal_path() {
        let mut f = BossFight::new();

        assert!(f.engage(), "entry engages");
        assert_eq!(f.state(), BossFightState::Engaged);
        assert!(f.is_sealed(), "the gate slams behind the player");
        assert!(!f.portal_active());

        assert!(f.defeat(), "the boss falls");
        assert_eq!(f.state(), BossFightState::Defeated);
        assert!(f.is_sealed(), "the kill is not the end — the pickup is");
        assert!(!f.portal_active());

        assert!(f.collect_reward(), "the reward closes the fight");
        assert_eq!(f.state(), BossFightState::RewardCollected);
        assert!(!f.is_sealed(), "the arena opens");
        assert!(f.portal_active(), "the way to the next level appears");
    }

    #[test]
    fn state_ids_are_pinned_for_the_gdscript_crossing() {
        assert_eq!(BossFightState::Dormant.id(), 0);
        assert_eq!(BossFightState::Engaged.id(), 1);
        assert_eq!(BossFightState::Defeated.id(), 2);
        assert_eq!(BossFightState::RewardCollected.id(), 3);
    }

    #[test]
    fn illegal_transitions_are_refused_and_preserve_state() {
        let mut f = BossFight::new();
        assert!(!f.defeat(), "nothing to defeat before engagement");
        assert!(!f.collect_reward(), "nothing to collect before engagement");
        assert_eq!(f.state(), BossFightState::Dormant);

        f.engage();
        assert!(!f.engage(), "a second trigger touch is a no-op");
        assert!(!f.collect_reward(), "no reward exists while the boss lives");
        assert_eq!(f.state(), BossFightState::Engaged);

        f.defeat();
        assert!(!f.engage(), "the fight never rewinds");
        assert!(!f.defeat(), "a duplicate death report is a no-op");
        assert_eq!(f.state(), BossFightState::Defeated);

        f.collect_reward();
        assert!(!f.engage());
        assert!(!f.defeat());
        assert!(!f.collect_reward());
        assert_eq!(f.state(), BossFightState::RewardCollected);
    }
}
