//! Game phase state machine governing screen transitions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    MainMenu,
    Playing,
    Paused,
    LevelComplete,
    KillSummary,
    Shop,
    Death,
}

impl GamePhase {
    /// Returns whether transitioning from self to `next` is valid.
    pub fn can_transition_to(&self, next: GamePhase) -> bool {
        matches!(
            (self, next),
            (GamePhase::MainMenu, GamePhase::Playing)
                | (GamePhase::Playing, GamePhase::Paused)
                | (GamePhase::Paused, GamePhase::Playing)
                | (GamePhase::Paused, GamePhase::MainMenu)
                | (GamePhase::Playing, GamePhase::LevelComplete)
                | (GamePhase::Playing, GamePhase::Death)
                | (GamePhase::LevelComplete, GamePhase::KillSummary)
                | (GamePhase::KillSummary, GamePhase::Shop)
                | (GamePhase::Shop, GamePhase::Playing)
                | (GamePhase::Death, GamePhase::MainMenu)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_menu_to_playing() {
        assert!(GamePhase::MainMenu.can_transition_to(GamePhase::Playing));
    }

    #[test]
    fn playing_to_level_complete() {
        assert!(GamePhase::Playing.can_transition_to(GamePhase::LevelComplete));
    }

    #[test]
    fn playing_to_death() {
        assert!(GamePhase::Playing.can_transition_to(GamePhase::Death));
    }

    #[test]
    fn level_complete_to_kill_summary() {
        assert!(GamePhase::LevelComplete.can_transition_to(GamePhase::KillSummary));
    }

    #[test]
    fn kill_summary_to_shop() {
        assert!(GamePhase::KillSummary.can_transition_to(GamePhase::Shop));
    }

    #[test]
    fn shop_to_playing() {
        assert!(GamePhase::Shop.can_transition_to(GamePhase::Playing));
    }

    #[test]
    fn death_to_main_menu() {
        assert!(GamePhase::Death.can_transition_to(GamePhase::MainMenu));
    }

    #[test]
    fn playing_to_paused() {
        assert!(GamePhase::Playing.can_transition_to(GamePhase::Paused));
    }

    #[test]
    fn paused_to_playing() {
        assert!(GamePhase::Paused.can_transition_to(GamePhase::Playing));
    }

    #[test]
    fn paused_to_main_menu() {
        assert!(GamePhase::Paused.can_transition_to(GamePhase::MainMenu));
    }

    #[test]
    fn cannot_pause_from_menu() {
        assert!(!GamePhase::MainMenu.can_transition_to(GamePhase::Paused));
    }

    #[test]
    fn cannot_skip_phases() {
        assert!(!GamePhase::MainMenu.can_transition_to(GamePhase::Shop));
        assert!(!GamePhase::Playing.can_transition_to(GamePhase::Shop));
        assert!(!GamePhase::KillSummary.can_transition_to(GamePhase::Playing));
        assert!(!GamePhase::Death.can_transition_to(GamePhase::Playing));
    }

    #[test]
    fn cannot_self_transition() {
        assert!(!GamePhase::Playing.can_transition_to(GamePhase::Playing));
        assert!(!GamePhase::MainMenu.can_transition_to(GamePhase::MainMenu));
    }
}
