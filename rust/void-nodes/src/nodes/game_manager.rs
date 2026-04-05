use godot::prelude::*;
use godot::classes::{
    Node, INode, Engine, CanvasLayer, Input, InputEvent,
    input::MouseMode,
    viewport::Msaa,
};

use super::constants::{actions, signals, methods, nodes};
use void_logic::enemy_type::EnemyType;
use void_logic::game_options::GameOptions;
use void_logic::game_phase::GamePhase;
use void_logic::input_method::InputMethod;
use void_logic::newtypes::Damage;
use void_logic::power_routing::PowerMode;
use void_logic::run_state::RunState;
use void_logic::save_game::SaveGame;

/// Central orchestrator: owns RunState, manages game phase transitions,
/// shows/hides UI screens, and connects signals from enemies/portal.
#[derive(GodotClass)]
#[class(base=Node)]
pub struct GameManager {
    base: Base<Node>,
    run_state: RunState,
    phase: GamePhase,
    game_options: GameOptions,
    save_game: Option<SaveGame>,
    active_input: InputMethod,
    current_power_mode: i32,
}

#[godot_api]
impl INode for GameManager {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            run_state: RunState::new(42),
            phase: GamePhase::MainMenu,
            game_options: GameOptions::new(),
            save_game: None,
            active_input: InputMethod::Keyboard,
            current_power_mode: 0,
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }

        // GameManager must process even when tree is paused (for pause toggle)
        self.base_mut().set_process_mode(godot::classes::node::ProcessMode::ALWAYS);

        // Detect controller on startup
        let input = Input::singleton();
        if !input.get_connected_joypads().is_empty() {
            self.active_input = InputMethod::Controller;
        }

        // Connect UI signals
        self.connect_ui_signals();
        self.show_phase(GamePhase::MainMenu);
    }

    fn input(&mut self, event: Gd<InputEvent>) {
        // Live-switch active input method based on last event type
        let class = event.get_class().to_string();
        if class.starts_with("InputEventJoypad") {
            self.active_input = InputMethod::Controller;
        } else if class.starts_with("InputEventKey")
            || class.starts_with("InputEventMouse")
        {
            self.active_input = InputMethod::Keyboard;
        }
    }

    fn process(&mut self, delta: f64) {
        // Check for pause toggle
        let input = Input::singleton();
        if input.is_action_just_pressed(actions::OPEN_MENU)
            && self.phase == GamePhase::Playing
        {
            self.transition_to(GamePhase::Paused);
            self.set_scene_paused(true);
            return;
        }

        if self.phase != GamePhase::Playing {
            return;
        }

        // Tick shield regeneration
        self.run_state.tick_shield(delta as f32);

        // Connect to any newly-spawned enemies and portal
        self.connect_spawned_entities();

        // Update HUD
        self.update_hud();
    }
}

#[godot_api]
impl GameManager {
    #[signal]
    fn phase_changed(phase_name: GString);

    #[signal]
    fn options_changed(sbs_enabled: bool, msaa_enabled: bool);

    /// Called from UI: start a fresh new game.
    #[func]
    pub fn start_new_game(&mut self) {
        // If paused, unpause and go to menu first
        if self.phase == GamePhase::Paused {
            self.set_scene_paused(false);
            self.transition_to(GamePhase::MainMenu);
        }
        self.run_state = RunState::new(42);
        self.save_game = None;
        self.transition_to(GamePhase::Playing);
    }

    /// Called from UI: continue from saved game.
    #[func]
    pub fn continue_game(&mut self) {
        if let Some(save) = &self.save_game {
            let mut run = RunState::new(save.run_seed);
            save.apply_to(&mut run);
            self.run_state = run;
        } else {
            self.run_state = RunState::new(42);
        }
        self.transition_to(GamePhase::Playing);
    }

    /// Called when an enemy dies (connected to enemy_killed signal).
    #[func]
    pub fn on_enemy_killed(&mut self, type_id: i32) {
        if let Some(enemy_type) = EnemyType::from_id(type_id) {
            self.run_state.record_kill(enemy_type);
            godot_print!(
                "Kill: {} | Credits: {}",
                enemy_type.display_name(),
                self.run_state.credits.balance,
            );
        }
    }

    /// Called when player enters the portal.
    #[func]
    pub fn on_portal_entered(&mut self) {
        if self.phase == GamePhase::Playing {
            self.save_game = Some(SaveGame::from_run_state(&self.run_state));
            self.transition_to(GamePhase::LevelComplete);
            self.transition_to(GamePhase::KillSummary);

            // Populate kill summary UI
            let Some(parent) = self.base().get_parent() else { return };
            if let Some(mut summary) = Self::find_ui_node(&parent, nodes::KILL_SUMMARY_UI) {
                let kill_data = self.get_kill_summary();
                summary.call(methods::SHOW_SUMMARY, &[
                    Variant::from(kill_data),
                    Variant::from(self.run_state.credits.balance as i64),
                    Variant::from(self.run_state.current_level as i32),
                ]);
            }
        }
    }

    /// Called from kill summary UI: proceed to shop.
    #[func]
    pub fn advance_to_shop(&mut self) {
        if self.phase == GamePhase::KillSummary {
            self.transition_to(GamePhase::Shop);
            self.show_shop_ui();
        }
    }

    /// Called from shop UI: buy laser upgrade.
    #[func]
    pub fn buy_laser_upgrade(&mut self) -> bool {
        let next = match self.run_state.laser_level.next() {
            Some(n) => n,
            None => return false,
        };
        let cost = match next.upgrade_cost() {
            Some(c) => c,
            None => return false,
        };
        if self.run_state.credits.spend(cost).is_ok() {
            self.run_state.laser_level = next;
            godot_print!(
                "Laser upgraded to {} (damage: {})",
                next.display_name(),
                next.damage(),
            );
            self.update_player_laser();
            // Refresh shop UI
            self.show_shop_ui();
            true
        } else {
            false
        }
    }

    /// Called from shop UI: proceed to next level.
    #[func]
    pub fn advance_to_next_level(&mut self) {
        if self.phase == GamePhase::Shop {
            self.run_state.current_level += 1;
            self.run_state.kills.reset();
            self.run_state.health = self.run_state.loadout.max_health();
            self.run_state.shield.reset();
            self.transition_to(GamePhase::Playing);
            self.regenerate_level();
        }
    }

    /// Called when player dies.
    #[func]
    pub fn on_player_death(&mut self) {
        if self.phase == GamePhase::Playing {
            let old_laser = self.run_state.laser_level.display_name().to_string();
            let level_reached = self.run_state.current_level as i32;
            self.run_state.apply_death_penalty();
            self.save_game = Some(SaveGame::from_run_state(&self.run_state));
            let new_laser = self.run_state.laser_level.display_name().to_string();
            self.transition_to(GamePhase::Death);

            // Show death screen
            let Some(parent) = self.base().get_parent() else { return };
            if let Some(mut death_ui) = Self::find_ui_node(&parent, nodes::DEATH_SCREEN_UI) {
                death_ui.call(methods::SHOW_DEATH, &[
                    Variant::from(GString::from(old_laser.as_str())),
                    Variant::from(GString::from(new_laser.as_str())),
                    Variant::from(level_reached),
                ]);
            }
        }
    }

    /// Called from main menu: toggle SBS stereo.
    #[func]
    pub fn on_sbs_toggled(&mut self) {
        let sbs = self.game_options.toggle_sbs();
        let msaa = self.game_options.msaa_enabled;
        self.base_mut().emit_signal(signals::OPTIONS_CHANGED, &[sbs.to_variant(), msaa.to_variant()]);
    }

    /// Called from main menu: toggle MSAA.
    #[func]
    pub fn on_msaa_toggled(&mut self) {
        let msaa = self.game_options.toggle_msaa();
        // Apply MSAA to viewport
        if let Some(mut viewport) = self.base().get_viewport() {
            viewport.set_msaa_3d(if msaa { Msaa::MSAA_4X } else { Msaa::DISABLED });
        }
        let sbs = self.game_options.sbs_enabled;
        self.base_mut().emit_signal(signals::OPTIONS_CHANGED, &[sbs.to_variant(), msaa.to_variant()]);
    }

    /// Called from pause menu: resume gameplay.
    #[func]
    pub fn resume_game(&mut self) {
        if self.phase == GamePhase::Paused {
            self.set_scene_paused(false);
            self.transition_to(GamePhase::Playing);
        }
    }

    /// Called from pause menu: quit to main menu.
    #[func]
    pub fn quit_to_menu(&mut self) {
        if self.phase == GamePhase::Paused {
            self.set_scene_paused(false);
            self.transition_to(GamePhase::MainMenu);
        }
    }

    /// Called when the player takes damage (from projectile hit).
    #[func]
    pub fn on_player_damaged(&mut self, amount: f32) {
        if self.phase != GamePhase::Playing {
            return;
        }
        self.run_state.take_damage(Damage::new(amount));
        if !self.run_state.is_alive() {
            self.on_player_death();
        }
    }

    /// Called when the player changes power routing mode.
    #[func]
    pub fn on_power_mode_changed(&mut self, mode: i32) {
        self.current_power_mode = mode;
        let power_mode = match mode {
            0 => PowerMode::Balanced,
            1 => PowerMode::ShieldBoost,
            2 => PowerMode::WeaponBoost,
            _ => PowerMode::Balanced,
        };
        let boosted = power_mode == PowerMode::ShieldBoost;
        self.run_state.shield.set_boosted(boosted);
    }

    /// Called from death screen: return to main menu.
    #[func]
    pub fn return_to_menu(&mut self) {
        if self.phase == GamePhase::Death {
            self.transition_to(GamePhase::MainMenu);
        }
    }

    // --- Getters for UI ---

    #[func]
    pub fn get_credits(&self) -> i64 {
        self.run_state.credits.balance as i64
    }

    #[func]
    pub fn get_laser_level(&self) -> i32 {
        self.run_state.laser_level as i32
    }

    #[func]
    pub fn get_laser_name(&self) -> GString {
        self.run_state.laser_level.display_name().into()
    }

    #[func]
    pub fn get_laser_damage(&self) -> f32 {
        self.run_state.laser_damage().as_f32()
    }

    #[func]
    pub fn get_current_level(&self) -> i32 {
        self.run_state.current_level as i32
    }

    #[func]
    pub fn get_health(&self) -> f32 {
        self.run_state.health.as_f32()
    }

    #[func]
    pub fn get_max_health(&self) -> f32 {
        self.run_state.loadout.max_health().as_f32()
    }

    #[func]
    pub fn get_shield(&self) -> f32 {
        self.run_state.shield.current.as_f32()
    }

    #[func]
    pub fn get_max_shield(&self) -> f32 {
        self.run_state.shield.max_capacity.as_f32()
    }

    #[func]
    pub fn get_phase_name(&self) -> GString {
        GString::from(format!("{:?}", self.phase).as_str())
    }

    #[func]
    pub fn get_kill_summary(&self) -> Dictionary<GString, i32> {
        let mut dict = Dictionary::new();
        for (enemy_type, count) in self.run_state.kills.summary() {
            dict.set(enemy_type.display_name(), count as i32);
        }
        dict
    }

    #[func]
    pub fn get_next_upgrade_cost(&self) -> i64 {
        self.run_state.laser_level
            .next()
            .and_then(|n| n.upgrade_cost())
            .unwrap_or(0) as i64
    }

    #[func]
    pub fn can_afford_upgrade(&self) -> bool {
        if let Some(next) = self.run_state.laser_level.next() {
            if let Some(cost) = next.upgrade_cost() {
                return self.run_state.credits.can_afford(cost);
            }
        }
        false
    }

    #[func]
    pub fn is_max_laser(&self) -> bool {
        self.run_state.laser_level.next().is_none()
    }

    #[func]
    pub fn get_laser_color(&self) -> Color {
        let c = self.run_state.laser_level.color();
        Color::from_rgba(c[0], c[1], c[2], c[3])
    }
}

impl GameManager {
    fn set_scene_paused(&self, paused: bool) {
        let mut tree = self.base().get_tree();
        tree.set_pause(paused);
    }

    fn transition_to(&mut self, next: GamePhase) {
        if !self.phase.can_transition_to(next) {
            godot_warn!(
                "Invalid phase transition: {:?} -> {:?}",
                self.phase, next
            );
            return;
        }
        self.phase = next;
        self.show_phase(next);

        let phase_name: GString = GString::from(format!("{:?}", next).as_str());
        self.base_mut().emit_signal(signals::PHASE_CHANGED, &[phase_name.to_variant()]);
    }

    fn show_phase(&self, phase: GamePhase) {
        // Mouse always visible — controller handles all gameplay input
        let mut input = Input::singleton();
        input.set_mouse_mode(MouseMode::VISIBLE);

        // Show/hide UI layers by calling into the tree
        let Some(parent) = self.base().get_parent() else { return };

        let menu_vis = phase == GamePhase::MainMenu;
        let hud_vis = phase == GamePhase::Playing || phase == GamePhase::Paused;
        let pause_vis = phase == GamePhase::Paused;
        let summary_vis = phase == GamePhase::KillSummary;
        let shop_vis = phase == GamePhase::Shop;
        let death_vis = phase == GamePhase::Death;

        Self::set_ui_visible(&parent, nodes::MAIN_MENU_UI, menu_vis);
        Self::set_ui_visible(&parent, nodes::HUD, hud_vis);
        Self::set_ui_visible(&parent, nodes::PAUSE_MENU_UI, pause_vis);
        Self::set_ui_visible(&parent, nodes::KILL_SUMMARY_UI, summary_vis);
        Self::set_ui_visible(&parent, nodes::SHOP_UI, shop_vis);
        Self::set_ui_visible(&parent, nodes::DEATH_SCREEN_UI, death_vis);

        // Show/hide gameplay elements (keep visible when paused)
        let gameplay_vis = phase == GamePhase::Playing || phase == GamePhase::Paused;
        if let Some(mut player) = parent.try_get_node_as::<Node3D>(nodes::PLAYER) {
            player.set_visible(gameplay_vis);
        }
        if let Some(mut level) = parent.try_get_node_as::<Node3D>(nodes::LEVEL_MANAGER) {
            level.set_visible(gameplay_vis);
        }

        // Show/hide ship showcase for end-of-level screens
        let showcase_vis = summary_vis || shop_vis || death_vis;
        if let Some(mut showcase) = parent.try_get_node_as::<Node>(nodes::SHIP_SHOWCASE) {
            if showcase_vis {
                let c = self.run_state.laser_level.color();
                let color = Color::from_rgba(c[0], c[1], c[2], c[3]);
                showcase.call(methods::SHOW_SHOWCASE, &[Variant::from(color)]);

                // Position showcase in front of camera
                if let Some(camera) = parent.try_get_node_as::<Node3D>(nodes::PLAYER_CAMERA) {
                    let cam_transform = camera.get_global_transform();
                    let forward = -cam_transform.basis.col_c();
                    let showcase_pos = cam_transform.origin + forward * 3.0;
                    if let Some(mut showcase_3d) = parent.try_get_node_as::<Node3D>(nodes::SHIP_SHOWCASE) {
                        showcase_3d.set_global_position(showcase_pos);
                    }
                }
            } else {
                showcase.call(methods::HIDE_SHOWCASE, &[]);
            }
        }
    }

    /// Find a UI node by name. UI nodes are always direct children of Main
    /// (never reparented — SBS uses custom_viewport to redirect rendering).
    fn find_ui_node(parent: &Gd<Node>, name: &str) -> Option<Gd<Node>> {
        if let Some(node) = parent.try_get_node_as::<Node>(name) {
            return Some(node);
        }
        godot_warn!("find_ui_node: '{}' not found", name);
        None
    }

    fn set_ui_visible(parent: &Gd<Node>, name: &str, visible: bool) {
        if let Some(node) = Self::find_ui_node(parent, name) {
            if let Ok(mut canvas) = node.try_cast::<CanvasLayer>() {
                canvas.set_layer(if visible { 1 } else { 128 });
                canvas.set_visible(visible);
                canvas.set_process_input(visible);
            } else {
                godot_warn!("set_ui_visible: '{}' failed to cast to CanvasLayer", name);
            }
        }
    }

    fn update_player_laser(&self) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut player) = parent.try_get_node_as::<Node>(nodes::PLAYER) {
            let level = self.run_state.laser_level as i32;
            player.call(methods::SET_LASER_LEVEL, &[Variant::from(level)]);
        }
    }

    fn show_shop_ui(&self) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut shop) = Self::find_ui_node(&parent, nodes::SHOP_UI) {
            let c = self.run_state.laser_level.color();
            let color = Color::from_rgba(c[0], c[1], c[2], c[3]);
            shop.call(methods::SHOW_SHOP, &[
                Variant::from(self.run_state.credits.balance as i64),
                Variant::from(GString::from(self.run_state.laser_level.display_name())),
                Variant::from(color),
                Variant::from(self.run_state.laser_damage().as_f32()),
                Variant::from(self.get_next_upgrade_cost()),
                Variant::from(self.can_afford_upgrade()),
                Variant::from(self.is_max_laser()),
            ]);
        }
    }

    fn connect_ui_signals(&mut self) {
        let Some(parent) = self.base().get_parent() else { return };

        // Connect MainMenuUI signals
        if let Some(menu) = Self::find_ui_node(&parent, nodes::MAIN_MENU_UI) {
            let new_game = self.base().callable(methods::START_NEW_GAME);
            if !menu.is_connected(signals::NEW_GAME_SELECTED, &new_game) {
                let mut menu = menu;
                menu.connect(signals::NEW_GAME_SELECTED, &new_game);
                let continue_game = self.base().callable(methods::CONTINUE_GAME);
                menu.connect(signals::CONTINUE_SELECTED, &continue_game);
                let sbs = self.base().callable(methods::ON_SBS_TOGGLED);
                menu.connect(signals::SBS_TOGGLED, &sbs);
                let msaa = self.base().callable(methods::ON_MSAA_TOGGLED);
                menu.connect(signals::MSAA_TOGGLED, &msaa);
            }
        }

        // Connect PauseMenuUI signals
        if let Some(pause_ui) = Self::find_ui_node(&parent, nodes::PAUSE_MENU_UI) {
            let resume = self.base().callable(methods::RESUME_GAME);
            if !pause_ui.is_connected(signals::RESUME_SELECTED, &resume) {
                let mut pause_ui = pause_ui;
                pause_ui.connect(signals::RESUME_SELECTED, &resume);
                let new_game = self.base().callable(methods::START_NEW_GAME);
                pause_ui.connect(signals::NEW_GAME_SELECTED, &new_game);
                let quit = self.base().callable(methods::QUIT_TO_MENU);
                pause_ui.connect(signals::QUIT_SELECTED, &quit);
                let sbs = self.base().callable(methods::ON_SBS_TOGGLED);
                pause_ui.connect(signals::SBS_TOGGLED, &sbs);
                let msaa = self.base().callable(methods::ON_MSAA_TOGGLED);
                pause_ui.connect(signals::MSAA_TOGGLED, &msaa);
            }
        }

        // Connect KillSummaryUI
        if let Some(summary) = Self::find_ui_node(&parent, nodes::KILL_SUMMARY_UI) {
            let callable = self.base().callable(methods::ADVANCE_TO_SHOP);
            if !summary.is_connected(signals::CONTINUE_PRESSED, &callable) {
                let mut summary = summary;
                summary.connect(signals::CONTINUE_PRESSED, &callable);
            }
        }

        // Connect ShopUI
        if let Some(shop) = Self::find_ui_node(&parent, nodes::SHOP_UI) {
            let buy_callable = self.base().callable(methods::BUY_LASER_UPGRADE);
            let continue_callable = self.base().callable(methods::ADVANCE_TO_NEXT_LEVEL);
            if !shop.is_connected(signals::BUY_PRESSED, &buy_callable) {
                let mut shop = shop;
                shop.connect(signals::BUY_PRESSED, &buy_callable);
                shop.connect(signals::CONTINUE_PRESSED, &continue_callable);
            }
        }

        // Connect Player signals
        if let Some(player) = parent.try_get_node_as::<Node>(nodes::PLAYER) {
            let damage_callable = self.base().callable(methods::ON_PLAYER_DAMAGED);
            if !player.is_connected(signals::PLAYER_DAMAGED, &damage_callable) {
                let mut player = player;
                player.connect(signals::PLAYER_DAMAGED, &damage_callable);
                let power_callable = self.base().callable(methods::ON_POWER_MODE_CHANGED);
                player.connect(signals::POWER_MODE_CHANGED, &power_callable);
            }
        }

        // Connect DeathScreenUI
        if let Some(death) = Self::find_ui_node(&parent, nodes::DEATH_SCREEN_UI) {
            let callable = self.base().callable(methods::RETURN_TO_MENU);
            if !death.is_connected(signals::RETURN_PRESSED, &callable) {
                let mut death = death;
                death.connect(signals::RETURN_PRESSED, &callable);
            }
        }
    }

    fn connect_spawned_entities(&mut self) {
        let Some(parent) = self.base().get_parent() else { return };
        let Some(level_mgr) = parent.try_get_node_as::<Node>(nodes::LEVEL_MANAGER) else { return };

        let callable = self.base().callable(methods::ON_ENEMY_KILLED);
        let portal_callable = self.base().callable(methods::ON_PORTAL_ENTERED);

        for child in level_mgr.get_children().iter_shared() {
            // Connect enemy signals
            if child.has_signal(signals::ENEMY_KILLED) && !child.is_connected(signals::ENEMY_KILLED, &callable) {
                let mut c = child.clone();
                c.connect(signals::ENEMY_KILLED, &callable);
            }
            // Connect portal signal
            if child.has_signal(signals::PORTAL_ENTERED) && !child.is_connected(signals::PORTAL_ENTERED, &portal_callable) {
                let mut c = child;
                c.connect(signals::PORTAL_ENTERED, &portal_callable);
            }
        }
    }

    fn update_hud(&self) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut hud) = Self::find_ui_node(&parent, nodes::HUD) {
            let c = self.run_state.laser_level.color();
            let color = Color::from_rgba(c[0], c[1], c[2], c[3]);
            hud.call(methods::UPDATE_HEALTH, &[
                Variant::from(self.run_state.health.as_f32()),
                Variant::from(self.run_state.loadout.max_health().as_f32()),
            ]);
            hud.call(methods::UPDATE_SHIELD, &[
                Variant::from(self.run_state.shield.current.as_f32()),
                Variant::from(self.run_state.shield.max_capacity.as_f32()),
            ]);
            hud.call(methods::UPDATE_POWER_MODE, &[
                Variant::from(self.current_power_mode),
            ]);
            hud.call(methods::UPDATE_CREDITS, &[
                Variant::from(self.run_state.credits.balance as i64),
            ]);
            hud.call(methods::UPDATE_LASER, &[
                Variant::from(GString::from(self.run_state.laser_level.display_name())),
                Variant::from(color),
            ]);
            hud.call(methods::UPDATE_LEVEL, &[
                Variant::from(self.run_state.current_level as i32),
            ]);
        }
    }


    fn regenerate_level(&self) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut level_mgr) = parent.try_get_node_as::<Node>(nodes::LEVEL_MANAGER) {
            // Clear old children
            for mut child in level_mgr.get_children().iter_shared() {
                child.queue_free();
            }
            // Set current level for enemy scaling
            level_mgr.set("current_level", &Variant::from(self.run_state.current_level as i32));
            // Generate with new seed based on level number
            let seed = 42_u64.wrapping_add(self.run_state.current_level as u64 * 7919);
            level_mgr.call(
                methods::GENERATE_LEVEL,
                &[Variant::from(seed as i64), Variant::from(5_i32)],
            );
        }
    }
}
