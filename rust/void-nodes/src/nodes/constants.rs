//! Compile-time string constants for all Godot identifiers.
//!
//! Every signal name, method name, input action, group, meta key,
//! theme override, node path, and scene path used across the Godot
//! integration layer is defined here as a typed constant.
//! This ensures typos are caught at compile time rather than
//! causing silent runtime failures.

// ── Signal names ──────────────────────────────────────────────────────

pub mod signals {
    pub const ENEMY_KILLED: &str = "enemy_killed";
    pub const PORTAL_ENTERED: &str = "portal_entered";
    pub const PHASE_CHANGED: &str = "phase_changed";
    pub const OPTIONS_CHANGED: &str = "options_changed";
    pub const CONTINUE_PRESSED: &str = "continue_pressed";
    pub const BUY_PRESSED: &str = "buy_pressed";
    pub const RETURN_PRESSED: &str = "return_pressed";
    pub const NEW_GAME_SELECTED: &str = "new_game_selected";
    pub const CONTINUE_SELECTED: &str = "continue_selected";
    pub const SBS_TOGGLED: &str = "sbs_toggled";
    pub const MSAA_TOGGLED: &str = "msaa_toggled";
    pub const EXIT_SELECTED: &str = "exit_selected";
    pub const RESUME_SELECTED: &str = "resume_selected";
    pub const QUIT_SELECTED: &str = "quit_selected";
    pub const BODY_ENTERED: &str = "body_entered";
    pub const SIZE_CHANGED: &str = "size_changed";
    pub const PLAYER_DAMAGED: &str = "player_damaged";
    pub const PLAYER_SLOWED: &str = "player_slowed";
    pub const POWER_MODE_CHANGED: &str = "power_mode_changed";
    pub const UPGRADE_COLLECTED: &str = "upgrade_collected";
    pub const ORGANICS_COLLECTED: &str = "organics_collected";
    pub const SHIP_COLOR_SELECTED: &str = "ship_color_selected";
    pub const BESTIARY_PAGED: &str = "bestiary_paged";
    pub const RENDER_VIEWPORTS_CHANGED: &str = "render_viewports_changed";
}

// ── Callable method names ─────────────────────────────────────────────

pub mod methods {
    pub const START_NEW_GAME: &str = "start_new_game";
    pub const CONTINUE_GAME: &str = "continue_game";
    pub const ON_ENEMY_KILLED: &str = "on_enemy_killed";
    pub const ON_PORTAL_ENTERED: &str = "on_portal_entered";
    pub const ON_SBS_TOGGLED: &str = "on_sbs_toggled";
    pub const ON_MSAA_TOGGLED: &str = "on_msaa_toggled";
    pub const ON_OPTIONS_CHANGED: &str = "on_options_changed";
    pub const BROADCAST_OPTIONS: &str = "broadcast_options";
    pub const ON_RENDER_VIEWPORTS_CHANGED: &str = "on_render_viewports_changed";
    pub const ON_BODY_ENTERED: &str = "on_body_entered";
    pub const ADVANCE_TO_SHOP: &str = "advance_to_shop";
    pub const ADVANCE_TO_NEXT_LEVEL: &str = "advance_to_next_level";
    pub const BUY_LASER_UPGRADE: &str = "buy_laser_upgrade";
    pub const RETURN_TO_MENU: &str = "return_to_menu";
    pub const RESUME_GAME: &str = "resume_game";
    pub const QUIT_TO_MENU: &str = "quit_to_menu";
    pub const ON_WINDOW_SIZE_CHANGED: &str = "on_window_size_changed";
    pub const TAKE_DAMAGE: &str = "take_damage";
    pub const APPLY_UPGRADE: &str = "apply_upgrade";
    pub const SHOW_SUMMARY: &str = "show_summary";
    pub const SHOW_DEATH: &str = "show_death";
    pub const SHOW_SHOP: &str = "show_shop";
    pub const SHOW_SHOWCASE: &str = "show_showcase";
    pub const HIDE_SHOWCASE: &str = "hide_showcase";
    pub const RESET_LOADOUT: &str = "reset_loadout";
    pub const SET_LASER_LEVEL: &str = "set_laser_level";
    pub const UPDATE_HEALTH: &str = "update_health";
    pub const UPDATE_COMPONENTS: &str = "update_components";
    pub const UPDATE_ORGANICS: &str = "update_organics";
    pub const ON_ORGANICS_COLLECTED: &str = "on_organics_collected";
    pub const ON_SHIP_COLOR_SELECTED: &str = "on_ship_color_selected";
    pub const ADVANCE_FROM_SHIP_SELECT: &str = "advance_from_ship_select";
    pub const SHOW_SHIP_SELECT: &str = "show_ship_select";
    pub const SHOW_BESTIARY: &str = "show_bestiary";
    pub const ADVANCE_FROM_BESTIARY: &str = "advance_from_bestiary";
    pub const ON_BESTIARY_PAGED: &str = "on_bestiary_paged";
    pub const BEGIN_BRIEFING: &str = "begin_briefing";
    pub const SHOW_ENTRY: &str = "show_entry";
    pub const HIDE_DISPLAY: &str = "hide_display";
    pub const CONFIGURE_SHIP: &str = "configure_ship";
    pub const SET_CONTROLS_ENABLED: &str = "set_controls_enabled";
    pub const UPDATE_LASER: &str = "update_laser";
    pub const UPDATE_LEVEL: &str = "update_level";
    pub const UPDATE_SHIELD: &str = "update_shield";
    pub const UPDATE_POWER_MODE: &str = "update_power_mode";
    pub const GENERATE_LEVEL: &str = "generate_level";
    pub const GENERATE_BACKDROP: &str = "generate_backdrop";
    pub const ROOM_FLOOR_CENTER: &str = "room_floor_center";
    pub const ON_PLAYER_DAMAGED: &str = "on_player_damaged";
    pub const ON_PLAYER_SLOWED: &str = "on_player_slowed";
    pub const APPLY_SLOW: &str = "apply_slow";
    pub const UPDATE_SLOW: &str = "update_slow";
    pub const ON_POWER_MODE_CHANGED: &str = "on_power_mode_changed";
    pub const ON_UPGRADE_COLLECTED: &str = "on_upgrade_collected";
    pub const ENTER_INITIAL_PHASE: &str = "enter_initial_phase";
    pub const ON_PHASE_CHANGED_AUDIO: &str = "on_phase_changed_audio";
    pub const ON_MUSIC_FINISHED: &str = "on_music_finished";
    pub const ON_SFX_FINISHED: &str = "on_sfx_finished";
}

// ── Input actions ─────────────────────────────────────────────────────

pub mod actions {
    pub const MOVE_FORWARD: &str = "move_forward";
    pub const MOVE_BACK: &str = "move_back";
    pub const MOVE_LEFT: &str = "move_left";
    pub const MOVE_RIGHT: &str = "move_right";
    pub const MOVE_UP: &str = "move_up";
    pub const MOVE_DOWN: &str = "move_down";
    pub const LOOK_UP: &str = "look_up";
    pub const LOOK_DOWN: &str = "look_down";
    pub const LOOK_LEFT: &str = "look_left";
    pub const LOOK_RIGHT: &str = "look_right";
    pub const ROLL_LEFT: &str = "roll_left";
    pub const ROLL_RIGHT: &str = "roll_right";
    pub const FIRE: &str = "fire";
    pub const OPEN_MENU: &str = "open_menu";
    pub const MENU_UP: &str = "menu_up";
    pub const MENU_DOWN: &str = "menu_down";
    pub const MENU_SELECT: &str = "menu_select";
    pub const MENU_BACK: &str = "menu_back";
    pub const ROUTE_SHIELDS: &str = "route_shields";
    pub const ROUTE_WEAPONS: &str = "route_weapons";
    pub const STABILIZE: &str = "stabilize";
    pub const TOGGLE_VIEW: &str = "toggle_view";
}

// ── Group names ───────────────────────────────────────────────────────

pub mod groups {
    pub const PLAYER: &str = "player";
    pub const ENEMIES: &str = "enemies";
}

// ── Meta keys ─────────────────────────────────────────────────────────

pub mod meta_keys {
    pub const BEAM_AGE: &str = "beam_age";
}

// ── Theme override keys ───────────────────────────────────────────────

pub mod theme {
    pub const FONT_SIZE: &str = "font_size";
    pub const FONT_COLOR: &str = "font_color";
}

// ── Node paths ────────────────────────────────────────────────────────

pub mod nodes {
    pub const GAME_MANAGER: &str = "GameManager";
    pub const LEVEL_MANAGER: &str = "LevelManager";
    pub const PLAYER: &str = "Player";
    pub const PLAYER_CAMERA: &str = "Player/Camera3D";
    pub const SHIP_SHOWCASE: &str = "ShipShowcase";
    pub const MAIN_MENU_UI: &str = "MainMenuUI";
    pub const HUD: &str = "HUD";
    pub const KILL_SUMMARY_UI: &str = "KillSummaryUI";
    pub const SHOP_UI: &str = "ShopUI";
    pub const SHIP_SELECT_UI: &str = "ShipSelectUI";
    pub const BESTIARY_UI: &str = "BestiaryUI";
    pub const BESTIARY_DISPLAY: &str = "BestiaryDisplay";
    pub const DEATH_SCREEN_UI: &str = "DeathScreenUI";
    pub const PAUSE_MENU_UI: &str = "PauseMenuUI";
    pub const STEREO_CANVAS: &str = "StereoCanvas";
    pub const MONO_UI_LAYER: &str = "MonoUILayer";
    pub const UI_VIEWPORT: &str = "UIViewport";
    pub const LEFT_CAMERA: &str = "StereoCanvas/LeftContainer/LeftViewport/LeftCamera";
    pub const RIGHT_CAMERA: &str = "StereoCanvas/RightContainer/RightViewport/RightCamera";
    pub const LEFT_CONTAINER: &str = "StereoCanvas/LeftContainer";
    pub const RIGHT_CONTAINER: &str = "StereoCanvas/RightContainer";
    pub const VIEW_MANAGER: &str = "ViewManager";
    pub const LEFT_VIEWPORT: &str = "StereoCanvas/LeftContainer/LeftViewport";
    pub const RIGHT_VIEWPORT: &str = "StereoCanvas/RightContainer/RightViewport";
    pub const LEFT_UI_OVERLAY: &str = "StereoCanvas/LeftContainer/LeftUIOverlay";
    pub const RIGHT_UI_OVERLAY: &str = "StereoCanvas/RightContainer/RightUIOverlay";
    pub const UI_PLANE: &str = "UIPlane";
    pub const AUDIO_MANAGER: &str = "AudioManager";
}


/// Property names set across node boundaries via `Node::set`.
pub mod properties {
    pub const CURRENT_LEVEL: &str = "current_level";
}

// ── Scene paths ───────────────────────────────────────────────────────

pub mod scenes {
    pub const LOOTBOX: &str = "res://scenes/items/lootbox.tscn";
    pub const ORGANIC_BARREL: &str = "res://scenes/items/organic_barrel.tscn";
    pub const PORTAL: &str = "res://scenes/items/portal.tscn";
    /// Player ship model (CGTrader, installed via `make assets`).
    pub const SHIP_MODEL: &str = "res://addons/ships/Spacecraft_1.glb";
    pub const ENEMY_DRONE_FALLBACK: &str = "res://scenes/enemies/enemy_drone.tscn";
    /// Bestiary pickup models — the same GLTFs the in-level pickups wear, spun
    /// in the briefing room (without the pickups' collision/collect behavior).
    pub const BARREL_MODEL: &str = "res://addons/quaternius/essentials/props/Prop_Barrel1.gltf";
    pub const CRATE_MODEL: &str = "res://addons/quaternius/modularscifimegakit/props/Prop_Crate1.gltf";
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Signal names must match the #[signal] fn names in their respective classes.
    /// The godot-rust macro generates signals from fn names using snake_case.
    #[test]
    fn signal_names_are_valid_snake_case() {
        let all_signals = [
            signals::ENEMY_KILLED,
            signals::PORTAL_ENTERED,
            signals::PHASE_CHANGED,
            signals::OPTIONS_CHANGED,
            signals::CONTINUE_PRESSED,
            signals::BUY_PRESSED,
            signals::RETURN_PRESSED,
            signals::NEW_GAME_SELECTED,
            signals::CONTINUE_SELECTED,
            signals::SBS_TOGGLED,
            signals::MSAA_TOGGLED,
            signals::EXIT_SELECTED,
            signals::RESUME_SELECTED,
            signals::QUIT_SELECTED,
            signals::BODY_ENTERED,
            signals::SIZE_CHANGED,
            signals::PLAYER_DAMAGED,
            signals::PLAYER_SLOWED,
            signals::POWER_MODE_CHANGED,
            signals::UPGRADE_COLLECTED,
            signals::ORGANICS_COLLECTED,
            signals::SHIP_COLOR_SELECTED,
            signals::RENDER_VIEWPORTS_CHANGED,
        ];
        for sig in &all_signals {
            assert!(
                sig.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "signal '{}' should be snake_case",
                sig
            );
            assert!(!sig.is_empty(), "signal name must not be empty");
        }
    }

    /// Method names must match the #[func] fn names in their respective classes.
    #[test]
    fn method_names_are_valid_snake_case() {
        let all_methods = [
            methods::START_NEW_GAME,
            methods::CONTINUE_GAME,
            methods::ON_ENEMY_KILLED,
            methods::ON_PORTAL_ENTERED,
            methods::ON_SBS_TOGGLED,
            methods::ON_MSAA_TOGGLED,
            methods::ON_OPTIONS_CHANGED,
            methods::BROADCAST_OPTIONS,
            methods::ON_RENDER_VIEWPORTS_CHANGED,
            methods::ON_BODY_ENTERED,
            methods::ADVANCE_TO_SHOP,
            methods::ADVANCE_TO_NEXT_LEVEL,
            methods::BUY_LASER_UPGRADE,
            methods::RETURN_TO_MENU,
            methods::TAKE_DAMAGE,
            methods::APPLY_UPGRADE,
            methods::SHOW_SUMMARY,
            methods::SHOW_DEATH,
            methods::SHOW_SHOP,
            methods::SHOW_SHOWCASE,
            methods::HIDE_SHOWCASE,
            methods::SET_LASER_LEVEL,
            methods::UPDATE_HEALTH,
            methods::UPDATE_COMPONENTS,
            methods::UPDATE_ORGANICS,
            methods::ON_ORGANICS_COLLECTED,
            methods::ON_SHIP_COLOR_SELECTED,
            methods::ADVANCE_FROM_SHIP_SELECT,
            methods::SHOW_SHIP_SELECT,
            methods::CONFIGURE_SHIP,
            methods::UPDATE_LASER,
            methods::UPDATE_LEVEL,
            methods::GENERATE_LEVEL,
            methods::GENERATE_BACKDROP,
            methods::ROOM_FLOOR_CENTER,
            methods::ON_PLAYER_DAMAGED,
            methods::ON_PLAYER_SLOWED,
            methods::APPLY_SLOW,
            methods::UPDATE_SLOW,
            methods::UPDATE_SHIELD,
            methods::ON_POWER_MODE_CHANGED,
            methods::ON_UPGRADE_COLLECTED,
            methods::UPDATE_POWER_MODE,
            methods::RESUME_GAME,
            methods::QUIT_TO_MENU,
            methods::ON_WINDOW_SIZE_CHANGED,
            methods::RESET_LOADOUT,
            methods::ON_PHASE_CHANGED_AUDIO,
            methods::ON_MUSIC_FINISHED,
            methods::ON_SFX_FINISHED,

        ];
        for method in &all_methods {
            assert!(
                method.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "method '{}' should be snake_case",
                method
            );
        }
    }

    /// Input action names must be non-empty snake_case.
    #[test]
    fn action_names_are_valid() {
        let all_actions = [
            actions::MOVE_FORWARD, actions::MOVE_BACK,
            actions::MOVE_LEFT, actions::MOVE_RIGHT,
            actions::MOVE_UP, actions::MOVE_DOWN,
            actions::LOOK_UP, actions::LOOK_DOWN,
            actions::LOOK_LEFT, actions::LOOK_RIGHT,
            actions::ROLL_LEFT, actions::ROLL_RIGHT,
            actions::FIRE,
            actions::OPEN_MENU,
            actions::MENU_UP,
            actions::MENU_DOWN,
            actions::MENU_SELECT,
            actions::MENU_BACK,
            actions::ROUTE_SHIELDS,
            actions::ROUTE_WEAPONS,
            actions::STABILIZE,
            actions::TOGGLE_VIEW,
        ];
        for action in &all_actions {
            assert!(!action.is_empty());
            assert!(
                action.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "action '{}' should be snake_case",
                action
            );
        }
    }

    /// Scene paths must start with res:// and end with a Godot-recognized extension.
    #[test]
    fn scene_paths_are_valid_godot_paths() {
        let all_scenes = [
            scenes::LOOTBOX,
            scenes::ORGANIC_BARREL,
            scenes::PORTAL,
            scenes::ENEMY_DRONE_FALLBACK,
            scenes::SHIP_MODEL,
        ];
        for path in &all_scenes {
            assert!(
                path.starts_with("res://"),
                "scene path '{}' must start with res://",
                path
            );
            assert!(
                path.ends_with(".tscn") || path.ends_with(".gltf") || path.ends_with(".glb"),
                "scene path '{}' must end with .tscn, .gltf, or .glb",
                path
            );
        }
    }

    /// No duplicate signal names.
    #[test]
    fn no_duplicate_signals() {
        let all = [
            signals::ENEMY_KILLED, signals::PORTAL_ENTERED,
            signals::PHASE_CHANGED, signals::OPTIONS_CHANGED,
            signals::CONTINUE_PRESSED, signals::BUY_PRESSED,
            signals::RETURN_PRESSED, signals::NEW_GAME_SELECTED,
            signals::CONTINUE_SELECTED, signals::SBS_TOGGLED,
            signals::MSAA_TOGGLED, signals::EXIT_SELECTED,
            signals::RESUME_SELECTED, signals::QUIT_SELECTED,
            signals::BODY_ENTERED,
            signals::SIZE_CHANGED,
            signals::PLAYER_DAMAGED,
            signals::PLAYER_SLOWED,
            signals::POWER_MODE_CHANGED,
            signals::UPGRADE_COLLECTED,
            signals::ORGANICS_COLLECTED,
            signals::SHIP_COLOR_SELECTED,
            signals::RENDER_VIEWPORTS_CHANGED,
        ];
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "duplicate signal: '{}'", a);
                }
            }
        }
    }

    /// No duplicate method names.
    #[test]
    fn no_duplicate_methods() {
        let all = [
            methods::START_NEW_GAME, methods::CONTINUE_GAME,
            methods::ON_ENEMY_KILLED, methods::ON_PORTAL_ENTERED,
            methods::ON_SBS_TOGGLED, methods::ON_MSAA_TOGGLED,
            methods::ON_OPTIONS_CHANGED, methods::ON_BODY_ENTERED,
            methods::BROADCAST_OPTIONS,
            methods::ON_RENDER_VIEWPORTS_CHANGED,
            methods::ADVANCE_TO_SHOP, methods::ADVANCE_TO_NEXT_LEVEL,
            methods::BUY_LASER_UPGRADE, methods::RETURN_TO_MENU,
            methods::RESUME_GAME, methods::QUIT_TO_MENU,
            methods::ON_WINDOW_SIZE_CHANGED,
            methods::TAKE_DAMAGE, methods::APPLY_UPGRADE,
            methods::SHOW_SUMMARY, methods::SHOW_DEATH,
            methods::SHOW_SHOP, methods::SHOW_SHOWCASE,
            methods::HIDE_SHOWCASE, methods::SET_LASER_LEVEL,
            methods::UPDATE_HEALTH, methods::UPDATE_COMPONENTS,
            methods::UPDATE_ORGANICS, methods::ON_ORGANICS_COLLECTED,
            methods::UPDATE_LASER, methods::UPDATE_LEVEL,
            methods::GENERATE_LEVEL, methods::ON_PLAYER_DAMAGED,
            methods::ON_PLAYER_SLOWED, methods::APPLY_SLOW, methods::UPDATE_SLOW,
            methods::UPDATE_SHIELD, methods::ON_POWER_MODE_CHANGED,
            methods::ON_UPGRADE_COLLECTED, methods::UPDATE_POWER_MODE,
            methods::ON_PHASE_CHANGED_AUDIO, methods::ON_MUSIC_FINISHED,

            methods::RESET_LOADOUT,
        ];
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "duplicate method: '{}'", a);
                }
            }
        }
    }

    /// Node paths must not be empty and must not start/end with '/'.
    #[test]
    fn node_paths_are_well_formed() {
        let all_nodes = [
            nodes::GAME_MANAGER, nodes::LEVEL_MANAGER,
            nodes::PLAYER, nodes::PLAYER_CAMERA,
            nodes::SHIP_SHOWCASE, nodes::MAIN_MENU_UI,
            nodes::HUD, nodes::KILL_SUMMARY_UI,
            nodes::SHOP_UI, nodes::DEATH_SCREEN_UI,
            nodes::PAUSE_MENU_UI,
            nodes::STEREO_CANVAS, nodes::MONO_UI_LAYER,
            nodes::UI_VIEWPORT, nodes::LEFT_CAMERA,
            nodes::RIGHT_CAMERA, nodes::LEFT_CONTAINER,
            nodes::RIGHT_CONTAINER, nodes::LEFT_UI_OVERLAY,
            nodes::RIGHT_UI_OVERLAY,
            nodes::UI_PLANE,
            nodes::AUDIO_MANAGER,
        ];
        for path in &all_nodes {
            assert!(!path.is_empty(), "node path must not be empty");
            assert!(!path.starts_with('/'), "node path '{}' should be relative", path);
            assert!(!path.ends_with('/'), "node path '{}' must not end with /", path);
        }
    }
}
