use godot::prelude::*;
use godot::classes::{Node, INode, Engine, CanvasLayer, viewport::Msaa};

use void_logic::enemy_type::EnemyType;
use void_logic::game_options::GameOptions;
use void_logic::game_phase::GamePhase;
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
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }

        // Connect UI signals
        self.connect_ui_signals();
        self.show_phase(GamePhase::MainMenu);
    }

    fn process(&mut self, _delta: f64) {
        if self.phase != GamePhase::Playing {
            return;
        }

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
            if let Some(mut summary) = Self::find_ui_node(&parent, "KillSummaryUI") {
                let kill_data = self.get_kill_summary();
                summary.call("show_summary", &[
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
            if let Some(mut death_ui) = Self::find_ui_node(&parent, "DeathScreenUI") {
                death_ui.call("show_death", &[
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
        self.base_mut().emit_signal("options_changed", &[sbs.to_variant(), msaa.to_variant()]);
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
        self.base_mut().emit_signal("options_changed", &[sbs.to_variant(), msaa.to_variant()]);
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
        self.run_state.laser_damage()
    }

    #[func]
    pub fn get_current_level(&self) -> i32 {
        self.run_state.current_level as i32
    }

    #[func]
    pub fn get_health(&self) -> f32 {
        self.run_state.health
    }

    #[func]
    pub fn get_max_health(&self) -> f32 {
        self.run_state.loadout.max_health()
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
        self.base_mut().emit_signal("phase_changed", &[phase_name.to_variant()]);
    }

    fn show_phase(&self, phase: GamePhase) {
        // Show/hide UI layers by calling into the tree
        let Some(parent) = self.base().get_parent() else { return };

        let menu_vis = phase == GamePhase::MainMenu;
        let hud_vis = phase == GamePhase::Playing;
        let summary_vis = phase == GamePhase::KillSummary;
        let shop_vis = phase == GamePhase::Shop;
        let death_vis = phase == GamePhase::Death;

        Self::set_ui_visible(&parent, "MainMenuUI", menu_vis);
        Self::set_ui_visible(&parent, "HUD", hud_vis);
        Self::set_ui_visible(&parent, "KillSummaryUI", summary_vis);
        Self::set_ui_visible(&parent, "ShopUI", shop_vis);
        Self::set_ui_visible(&parent, "DeathScreenUI", death_vis);

        // Show/hide gameplay elements
        if let Some(mut player) = parent.try_get_node_as::<Node3D>("Player") {
            player.set_visible(hud_vis);
        }
        if let Some(mut level) = parent.try_get_node_as::<Node3D>("LevelManager") {
            level.set_visible(hud_vis);
        }

        // Show/hide ship showcase for end-of-level screens
        let showcase_vis = summary_vis || shop_vis || death_vis;
        if let Some(mut showcase) = parent.try_get_node_as::<Node>("ShipShowcase") {
            if showcase_vis {
                let c = self.run_state.laser_level.color();
                let color = Color::from_rgba(c[0], c[1], c[2], c[3]);
                showcase.call("show_showcase", &[Variant::from(color)]);

                // Position showcase in front of camera
                if let Some(camera) = parent.try_get_node_as::<Node3D>("Player/Camera3D") {
                    let cam_transform = camera.get_global_transform();
                    let forward = -cam_transform.basis.col_c();
                    let showcase_pos = cam_transform.origin + forward * 3.0;
                    if let Some(mut showcase_3d) = parent.try_get_node_as::<Node3D>("ShipShowcase") {
                        showcase_3d.set_global_position(showcase_pos);
                    }
                }
            } else {
                showcase.call("hide_showcase", &[]);
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
        if let Some(mut player) = parent.try_get_node_as::<Node>("Player") {
            let level = self.run_state.laser_level as i32;
            player.call("set_laser_level", &[Variant::from(level)]);
        }
    }

    fn show_shop_ui(&self) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut shop) = Self::find_ui_node(&parent, "ShopUI") {
            let c = self.run_state.laser_level.color();
            let color = Color::from_rgba(c[0], c[1], c[2], c[3]);
            shop.call("show_shop", &[
                Variant::from(self.run_state.credits.balance as i64),
                Variant::from(GString::from(self.run_state.laser_level.display_name())),
                Variant::from(color),
                Variant::from(self.run_state.laser_damage()),
                Variant::from(self.get_next_upgrade_cost()),
                Variant::from(self.can_afford_upgrade()),
                Variant::from(self.is_max_laser()),
            ]);
        }
    }

    fn connect_ui_signals(&mut self) {
        let Some(parent) = self.base().get_parent() else { return };

        // Connect MainMenuUI signals
        if let Some(menu) = Self::find_ui_node(&parent, "MainMenuUI") {
            let new_game = self.base().callable("start_new_game");
            if !menu.is_connected("new_game_selected", &new_game) {
                let mut menu = menu;
                menu.connect("new_game_selected", &new_game);
                let continue_game = self.base().callable("continue_game");
                menu.connect("continue_selected", &continue_game);
                let sbs = self.base().callable("on_sbs_toggled");
                menu.connect("sbs_toggled", &sbs);
                let msaa = self.base().callable("on_msaa_toggled");
                menu.connect("msaa_toggled", &msaa);
            }
        }

        // Connect KillSummaryUI
        if let Some(summary) = Self::find_ui_node(&parent, "KillSummaryUI") {
            let callable = self.base().callable("advance_to_shop");
            if !summary.is_connected("continue_pressed", &callable) {
                let mut summary = summary;
                summary.connect("continue_pressed", &callable);
            }
        }

        // Connect ShopUI
        if let Some(shop) = Self::find_ui_node(&parent, "ShopUI") {
            let buy_callable = self.base().callable("buy_laser_upgrade");
            let continue_callable = self.base().callable("advance_to_next_level");
            if !shop.is_connected("buy_pressed", &buy_callable) {
                let mut shop = shop;
                shop.connect("buy_pressed", &buy_callable);
                shop.connect("continue_pressed", &continue_callable);
            }
        }

        // Connect DeathScreenUI
        if let Some(death) = Self::find_ui_node(&parent, "DeathScreenUI") {
            let callable = self.base().callable("return_to_menu");
            if !death.is_connected("return_pressed", &callable) {
                let mut death = death;
                death.connect("return_pressed", &callable);
            }
        }
    }

    fn connect_spawned_entities(&mut self) {
        let Some(parent) = self.base().get_parent() else { return };
        let Some(level_mgr) = parent.try_get_node_as::<Node>("LevelManager") else { return };

        let callable = self.base().callable("on_enemy_killed");
        let portal_callable = self.base().callable("on_portal_entered");

        for child in level_mgr.get_children().iter_shared() {
            // Connect enemy signals
            if child.has_signal("enemy_killed") && !child.is_connected("enemy_killed", &callable) {
                let mut c = child.clone();
                c.connect("enemy_killed", &callable);
            }
            // Connect portal signal
            if child.has_signal("portal_entered") && !child.is_connected("portal_entered", &portal_callable) {
                let mut c = child;
                c.connect("portal_entered", &portal_callable);
            }
        }
    }

    fn update_hud(&self) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut hud) = Self::find_ui_node(&parent, "HUD") {
            let c = self.run_state.laser_level.color();
            let color = Color::from_rgba(c[0], c[1], c[2], c[3]);
            hud.call("update_health", &[
                Variant::from(self.run_state.health),
                Variant::from(self.run_state.loadout.max_health()),
            ]);
            hud.call("update_credits", &[
                Variant::from(self.run_state.credits.balance as i64),
            ]);
            hud.call("update_laser", &[
                Variant::from(GString::from(self.run_state.laser_level.display_name())),
                Variant::from(color),
            ]);
            hud.call("update_level", &[
                Variant::from(self.run_state.current_level as i32),
            ]);
        }
    }


    fn regenerate_level(&self) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut level_mgr) = parent.try_get_node_as::<Node>("LevelManager") {
            // Clear old children
            for mut child in level_mgr.get_children().iter_shared() {
                child.queue_free();
            }
            // Set current level for enemy scaling
            level_mgr.set("current_level", &Variant::from(self.run_state.current_level as i32));
            // Generate with new seed based on level number
            let seed = 42_u64.wrapping_add(self.run_state.current_level as u64 * 7919);
            level_mgr.call(
                "generate_level",
                &[Variant::from(seed as i64), Variant::from(5_i32)],
            );
        }
    }
}
