//! Game phase state machine governing screen transitions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    MainMenu,
    Playing,
    Paused,
    LevelComplete,
    KillSummary,
    Shop,
    /// Between-level (and new-game) screen to pick the ship color/loadout.
    ShipSelect,
    /// Pre-level briefing: the bestiary of pickups and enemies seen so far,
    /// shown in the loadout room while the next level is readied.
    Bestiary,
    Death,
}

impl GamePhase {
    /// Parse from the `Debug` format name (e.g. `"MainMenu"`, `"Playing"`).
    /// This is the format emitted by `GameManager::phase_changed`.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "MainMenu" => Some(Self::MainMenu),
            "Playing" => Some(Self::Playing),
            "Paused" => Some(Self::Paused),
            "LevelComplete" => Some(Self::LevelComplete),
            "KillSummary" => Some(Self::KillSummary),
            "Shop" => Some(Self::Shop),
            "ShipSelect" => Some(Self::ShipSelect),
            "Bestiary" => Some(Self::Bestiary),
            "Death" => Some(Self::Death),
            _ => None,
        }
    }

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
                // Loadout / ship-color screen, reached on new game and between levels.
                | (GamePhase::MainMenu, GamePhase::ShipSelect)
                | (GamePhase::Shop, GamePhase::ShipSelect)
                // Ship-select → bestiary briefing → into the level.
                | (GamePhase::ShipSelect, GamePhase::Bestiary)
                | (GamePhase::Bestiary, GamePhase::Playing)
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
    fn ship_select_flow() {
        assert!(GamePhase::MainMenu.can_transition_to(GamePhase::ShipSelect));
        assert!(GamePhase::Shop.can_transition_to(GamePhase::ShipSelect));
        // Ship-select hands off to the bestiary briefing, not straight to play.
        assert!(!GamePhase::ShipSelect.can_transition_to(GamePhase::Playing));
        // Not a free-for-all.
        assert!(!GamePhase::ShipSelect.can_transition_to(GamePhase::Shop));
        assert!(!GamePhase::Playing.can_transition_to(GamePhase::ShipSelect));
    }

    #[test]
    fn bestiary_briefing_sits_between_ship_select_and_play() {
        assert!(GamePhase::ShipSelect.can_transition_to(GamePhase::Bestiary));
        assert!(GamePhase::Bestiary.can_transition_to(GamePhase::Playing));
        // The briefing is a one-way gate into the level, nothing else.
        assert!(!GamePhase::Bestiary.can_transition_to(GamePhase::Shop));
        assert!(!GamePhase::Bestiary.can_transition_to(GamePhase::ShipSelect));
        assert!(!GamePhase::Playing.can_transition_to(GamePhase::Bestiary));
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
    fn from_name_round_trips_all_variants() {
        let all = [
            GamePhase::MainMenu, GamePhase::Playing, GamePhase::Paused,
            GamePhase::LevelComplete, GamePhase::KillSummary,
            GamePhase::Shop, GamePhase::ShipSelect, GamePhase::Bestiary,
            GamePhase::Death,
        ];
        for phase in all {
            let name = format!("{phase:?}");
            assert_eq!(
                GamePhase::from_name(&name),
                Some(phase),
                "{name} should round-trip through from_name"
            );
        }
    }

    #[test]
    fn from_name_returns_none_for_garbage() {
        assert_eq!(GamePhase::from_name("Bogus"), None);
        assert_eq!(GamePhase::from_name(""), None);
    }

    #[test]
    fn cannot_self_transition() {
        assert!(!GamePhase::Playing.can_transition_to(GamePhase::Playing));
        assert!(!GamePhase::MainMenu.can_transition_to(GamePhase::MainMenu));
    }
}
