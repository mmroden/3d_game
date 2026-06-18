use godot::prelude::*;
use godot::classes::{
    Node, INode, Engine, CanvasLayer, Input, InputEvent,
    input::MouseMode,
};

use super::persistence;

/// Persisted display/render preferences — the single GameOptions is
/// loaded from here at startup and rewritten on every change.
const OPTIONS_FILE: &str = "user://options.cfg";
const OPTIONS_SECTION: &str = "display";
/// Persisted run snapshot — the in-memory SaveGame is written here so
/// "Continue" survives a quit.
const SAVE_FILE: &str = "user://savegame.cfg";
const SAVE_SECTION: &str = "run";

use rand::RngExt;

use super::constants::{actions, signals, methods, nodes, properties};
use super::godot_util;
use void_logic::audio_catalog::SfxEvent;
use void_logic::bestiary::{self, BestiaryKind};
use void_logic::enemy_type::{self, EnemyType};
use void_logic::game_options::GameOptions;
use void_logic::game_phase::GamePhase;
use void_logic::generator::rooms_for_level;
use void_logic::input_method::InputMethod;
use void_logic::newtypes::Damage;
use void_logic::power_routing::PowerMode;
use void_logic::run_state::{RunState, DamageOutcome};
use void_logic::save_game::SaveGame;
use void_logic::seed::Seed;
use void_logic::ship::ShipColor;
use void_logic::upgrade::{Upgrade, UpgradeKind};

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
    /// Which bestiary entry the pre-level briefing is currently showing.
    bestiary_index: usize,

    /// Pins the run seed for reproducible runs when nonzero (0 = random).
    /// Configuration, not build flavor: set it in the editor or a test
    /// scene to replay a run.
    #[export]
    fixed_seed: i64,
}

#[godot_api]
impl INode for GameManager {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            // Placeholder until a run starts: the phase machine only
            // enters Playing through start_new_game/continue_game,
            // which assign the real seed via fresh_run_seed().
            run_state: RunState::new(Seed::new(0)),
            phase: GamePhase::MainMenu,
            game_options: GameOptions::new(),
            save_game: None,
            active_input: InputMethod::Keyboard,
            current_power_mode: 0,
            bestiary_index: 0,
            fixed_seed: 0,
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
        // Load remembered preferences into the one model object first,
        // so the broadcast below seeds consumers from the saved values.
        self.load_options();
        // Load any persisted run so "Continue" survives a quit.
        self.load_run();
        self.connect_ui_signals();
        // Seed every options consumer (menus, ViewManager) from the one
        // authoritative GameOptions. Deferred so it fires after all
        // sibling nodes have run ready() and connected their listeners.
        self.base_mut().call_deferred(methods::BROADCAST_OPTIONS, &[]);
        // Apply the opening screen deferred, NOT inline: sibling nodes (the
        // showcase, player, UI) run their own ready() after ours, and a node
        // that defaults itself hidden in ready() would clobber an inline
        // show_phase — that was the black-menu regression. Deferred runs once
        // every sibling is ready, so GameManager's visibility wins.
        self.base_mut().call_deferred(methods::ENTER_INITIAL_PHASE, &[]);
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

    /// Deferred from `ready()`: show the opening screen once every sibling node
    /// has finished its own `ready()`. See the call site for why it can't be
    /// inline.
    #[func]
    fn enter_initial_phase(&mut self) {
        self.show_phase(self.phase);
    }

    /// Called from UI: start a fresh new game.
    #[func]
    pub fn start_new_game(&mut self) {
        // If paused, unpause and go to menu first
        if self.phase == GamePhase::Paused {
            self.set_scene_paused(false);
            self.transition_to(GamePhase::MainMenu);
        }
        // New game starts at the loadout (ship-color) screen.
        if !self.phase.can_transition_to(GamePhase::ShipSelect) {
            return;
        }
        self.run_state = RunState::new(self.fresh_run_seed());
        self.save_game = None;
        self.sync_player_state();
        self.transition_to(GamePhase::ShipSelect);
        self.show_ship_select_ui();
    }

    /// Called from UI: continue from saved game.
    #[func]
    pub fn continue_game(&mut self) {
        // Don't touch the run unless the phase machine can enter Playing.
        if !self.phase.can_transition_to(GamePhase::Playing) {
            return;
        }
        if let Some(save) = &self.save_game {
            let mut run = RunState::new(save.run_seed);
            save.apply_to(&mut run);
            self.run_state = run;
        } else {
            self.run_state = RunState::new(self.fresh_run_seed());
        }
        self.sync_player_state();
        self.transition_to(GamePhase::Playing);
    }

    /// Called when an enemy dies (connected to enemy_killed signal).
    #[func]
    pub fn on_enemy_killed(&mut self, type_id: i32) {
        if let Some(enemy_type) = EnemyType::from_id(type_id) {
            self.run_state.record_kill(enemy_type);
            godot_print!(
                "Kill: {} | Components: {}",
                enemy_type.display_name(),
                self.run_state.components.balance,
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
                    Variant::from(self.run_state.components.balance as i64),
                    Variant::from(self.run_state.current_level as i32),
                ]);
            }
        }
    }

    /// Called from kill summary UI: proceed to shop.
    #[func]
    pub fn advance_to_shop(&mut self) {
        if self.phase == GamePhase::KillSummary {
            // End of level: snapshot and persist so the cleared-level
            // progress survives a quit (the doc's "saved at end-of-level").
            self.save_game = Some(SaveGame::from_run_state(&self.run_state));
            self.save_run();
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
        if self.run_state.components.spend(cost).is_ok() {
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

    /// Called from shop UI: proceed to the loadout screen, then the next level.
    #[func]
    pub fn advance_to_next_level(&mut self) {
        if self.phase == GamePhase::Shop {
            self.run_state.current_level += 1;
            self.run_state.kills.reset();
            self.run_state.health = self.run_state.loadout.max_health();
            self.run_state.shield.reset();
            self.transition_to(GamePhase::ShipSelect);
            self.show_ship_select_ui();
        }
    }

    /// Called from the ship-select UI: a color was chosen — apply it live.
    #[func]
    pub fn on_ship_color_selected(&mut self, color_id: i32) {
        let Some(color) = ShipColor::from_id(color_id) else { return };
        self.run_state.set_ship_color(color);
        self.sync_player_state();
        self.update_hud();
        // Re-skin the showcase ship behind the loadout screen to the new style.
        if let Some(parent) = self.base().get_parent() {
            if let Some(mut turntable) = parent.try_get_node_as::<Node>(nodes::TURNTABLE) {
                turntable.call(methods::SHOW_SHIP, &[Variant::from(color_id)]);
            }
        }
    }

    /// Called from the ship-select UI's Continue: into the pre-level briefing.
    #[func]
    pub fn advance_from_ship_select(&mut self) {
        if self.phase == GamePhase::ShipSelect {
            self.sync_player_state();
            self.transition_to(GamePhase::Bestiary);
        }
    }

    /// Called from the bestiary UI's Select/Fire: begin the mission. Browsing is
    /// separate (see `on_bestiary_paged`), so this always drops into the level.
    #[func]
    pub fn advance_from_bestiary(&mut self) {
        if self.phase == GamePhase::Bestiary {
            self.transition_to(GamePhase::Playing);
        }
    }

    /// Called from the bestiary UI's left stick: browse the catalog by `delta`
    /// (-1 prev, +1 next), clamped to the ends — no wrap, no auto-begin.
    #[func]
    pub fn on_bestiary_paged(&mut self, delta: i32) {
        if self.phase != GamePhase::Bestiary {
            return;
        }
        let total = bestiary::entries(&self.run_state.seen_enemies).len();
        self.bestiary_index = bestiary::paged_index(self.bestiary_index, delta, total);
        self.refresh_bestiary();
    }

    /// Reset the briefing to its first entry and show it, then lock the UI's
    /// input for a beat so the ship-select press that opened this screen can't
    /// bleed through and instantly begin the mission.
    fn enter_bestiary(&mut self) {
        self.bestiary_index = 0;
        self.refresh_bestiary();
        if let Some(parent) = self.base().get_parent() {
            if let Some(mut ui) = Self::find_ui_node(&parent, nodes::BESTIARY_UI) {
                ui.call(methods::BEGIN_BRIEFING, &[]);
            }
        }
    }

    /// Push the current briefing entry to the panel (name + lore) and to the
    /// turntable (the model to spin). GameManager owns the paging so the UI and
    /// the 3D display never drift apart.
    fn refresh_bestiary(&self) {
        let entries = bestiary::entries(&self.run_state.seen_enemies);
        let Some(entry) = entries.get(self.bestiary_index) else { return };
        let Some(parent) = self.base().get_parent() else { return };

        let (kind_id, enemy_id): (i32, i32) = match entry.kind {
            BestiaryKind::OrganicBarrel => (0, -1),
            BestiaryKind::ComponentCache => (1, -1),
            BestiaryKind::Enemy(t) => (2, t.id()),
        };
        if let Some(mut turntable) = parent.try_get_node_as::<Node>(nodes::TURNTABLE) {
            turntable.call(
                methods::SHOW_ENTRY,
                &[Variant::from(kind_id), Variant::from(enemy_id)],
            );
        }

        // Left stick browses; Ⓧ begins. The hint (and whether it offers next/prev)
        // is owned by void-logic.
        let hint = bestiary::briefing_hint(entries.len());
        let position = format!("{} / {}", self.bestiary_index + 1, entries.len());
        if let Some(mut ui) = Self::find_ui_node(&parent, nodes::BESTIARY_UI) {
            ui.call(methods::SHOW_BESTIARY, &[
                Variant::from(GString::from(entry.title)),
                Variant::from(GString::from(entry.blurb)),
                Variant::from(GString::from(position.as_str())),
                Variant::from(GString::from(hint)),
            ]);
        }
    }

    /// Populate and show the ship-select UI with the current color marked.
    fn show_ship_select_ui(&self) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut ui) = Self::find_ui_node(&parent, nodes::SHIP_SELECT_UI) {
            ui.call(methods::SHOW_SHIP_SELECT, &[Variant::from(self.run_state.ship_color.id())]);
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
            self.save_run();
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

    /// Broadcast the authoritative options to every consumer (menus,
    /// ViewManager). The single source of truth is `self.game_options`;
    /// listeners cache a copy but never invent their own default.
    #[func]
    fn broadcast_options(&mut self) {
        let sbs = self.game_options.sbs_enabled;
        let msaa = self.game_options.msaa_enabled;
        self.base_mut()
            .emit_signal(signals::OPTIONS_CHANGED, &[sbs.to_variant(), msaa.to_variant()]);
    }

    /// The authoritative MSAA option (for tests / inspection).
    #[func]
    fn msaa_enabled(&self) -> bool {
        self.game_options.msaa_enabled
    }

    /// The authoritative SBS option (for tests / inspection).
    #[func]
    fn sbs_enabled(&self) -> bool {
        self.game_options.sbs_enabled
    }

    /// Test seam: discard in-memory options and reload from disk, as a
    /// fresh launch would.
    #[func]
    fn reload_options_from_disk(&mut self) {
        self.game_options = GameOptions::default();
        self.load_options();
    }

    /// Called from main menu: toggle SBS stereo.
    #[func]
    pub fn on_sbs_toggled(&mut self) {
        let sbs = self.game_options.toggle_sbs();
        let msaa = self.game_options.msaa_enabled;
        self.save_options();
        self.base_mut().emit_signal(signals::OPTIONS_CHANGED, &[sbs.to_variant(), msaa.to_variant()]);
    }

    /// Called from main menu: toggle MSAA. Controller-only — flip the
    /// option and announce it; ViewManager (the view) applies it to the
    /// actual viewports.
    #[func]
    pub fn on_msaa_toggled(&mut self) {
        let msaa = self.game_options.toggle_msaa();
        let sbs = self.game_options.sbs_enabled;
        self.save_options();
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
    pub fn on_player_damaged(&mut self, amount: f32, hit_position: Vector3) {
        if self.phase != GamePhase::Playing {
            return;
        }
        let outcome = self.run_state.take_damage(Damage::new(amount));
        // Pick the hit sound from the layer that took it: a held shield gives a
        // light deflection, a hull breach a full explosion. Play it at the
        // ship-localized source point so it points at the attacker.
        let event = match outcome {
            DamageOutcome::ShieldHeld => SfxEvent::HitShielded,
            DamageOutcome::HullHit => SfxEvent::Explosion,
        };
        if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
            audio.bind_mut().play_event_at(event, hit_position);
        }
        if !self.run_state.is_alive() {
            self.on_player_death();
        }
    }

    /// The player's ship rammed static geometry. Pick the collision sound from
    /// the shield: a cushioned clang while it holds, bare metal once it's down.
    #[func]
    pub fn on_player_collided(&mut self) {
        if self.phase != GamePhase::Playing {
            return;
        }
        let event = if self.run_state.shield.is_up() {
            SfxEvent::CollisionShielded
        } else {
            SfxEvent::CollisionBare
        };
        if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
            audio.bind_mut().play_event(event);
        }
    }

    /// Relay the player's slow state to the HUD indicator.
    #[func]
    pub fn on_player_slowed(&mut self, active: bool) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut hud) = Self::find_ui_node(&parent, nodes::HUD) {
            hud.call(methods::UPDATE_SLOW, &[Variant::from(active)]);
        }
    }

    /// Called when a lootbox is collected — update RunState then push to ShipController.
    #[func]
    pub fn on_upgrade_collected(&mut self, name: GString, kind_id: i32, multiplier: f32) {
        let kind = match kind_id {
            0 => UpgradeKind::Thrust,
            1 => UpgradeKind::RotationSpeed,
            2 => UpgradeKind::Damping,
            3 => UpgradeKind::MaxHealth,
            4 => UpgradeKind::FireRate,
            5 => UpgradeKind::ProjectileSpeed,
            6 => UpgradeKind::ProjectileDamage,
            _ => return,
        };
        let upgrade = Upgrade {
            name: name.to_string(),
            kind,
            multiplier,
        };
        // Update authority
        self.run_state.loadout.add_upgrade(upgrade);

        // Push to ShipController cache
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut player) = parent.try_get_node_as::<Node>(nodes::PLAYER) {
            player.call(methods::APPLY_UPGRADE, &[
                Variant::from(name),
                Variant::from(kind_id),
                Variant::from(multiplier),
            ]);
        }
    }

    /// Called when an organics barrel is collected — adds to the permanent
    /// organics account and refreshes the HUD.
    #[func]
    pub fn on_organics_collected(&mut self, amount: i32) {
        self.run_state.collect_organics(amount.max(0) as u32);
        self.update_hud();
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
    pub fn get_components(&self) -> i64 {
        self.run_state.components.balance as i64
    }

    #[func]
    pub fn get_organics(&self) -> i64 {
        self.run_state.organics.balance as i64
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
                return self.run_state.components.can_afford(cost);
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
        let prev = self.phase;
        self.phase = next;

        // The loadout and briefing screens SHARE one backdrop room. Build it
        // once when entering that flow from outside it; flipping between
        // ShipSelect and Bestiary keeps the same room (and the parked camera)
        // and only swaps what the turntable shows — no teardown, no regenerate.
        let entering_backdrop = next == GamePhase::ShipSelect || next == GamePhase::Bestiary;
        let leaving_backdrop = prev == GamePhase::ShipSelect || prev == GamePhase::Bestiary;
        if entering_backdrop && !leaving_backdrop {
            self.generate_backdrop();
        }

        self.show_phase(next);

        let phase_name: GString = GString::from(format!("{:?}", next).as_str());
        self.base_mut().emit_signal(signals::PHASE_CHANGED, &[phase_name.to_variant()]);

        // The phase machine owns lifecycle effects: entering Playing
        // means a fresh level for the current RunState — except when
        // resuming from pause, which returns to the level in progress.
        if next == GamePhase::Playing && prev != GamePhase::Paused {
            self.mark_level_enemies_seen();
            self.regenerate_level();
        }

        // Entering the briefing: catalog stands at whatever's been seen so far,
        // start it at the first entry.
        if next == GamePhase::Bestiary {
            self.enter_bestiary();
        }
    }

    /// Catalog every enemy type this level will contain, so the *next* briefing
    /// lists them, and persist the bestiary if it grew (it is permanent).
    fn mark_level_enemies_seen(&mut self) {
        let mut grew = false;
        for enemy in enemy_type::enemies_for_level(self.run_state.current_level) {
            if self.run_state.mark_enemy_seen(enemy) {
                grew = true;
            }
        }
        if grew {
            self.save_game = Some(SaveGame::from_run_state(&self.run_state));
            self.save_run();
        }
    }

    /// Build the quiet one-room backdrop shown behind the loadout screen.
    fn generate_backdrop(&self) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut level_mgr) = parent.try_get_node_as::<Node>(nodes::LEVEL_MANAGER) {
            for mut child in level_mgr.get_children().iter_shared() {
                child.queue_free();
            }
            level_mgr.set(properties::CURRENT_LEVEL, &Variant::from(self.run_state.current_level as i32));
            let seed = self.run_state.level_seed();
            level_mgr.call(methods::GENERATE_BACKDROP, &[Variant::from(seed.as_i64())]);
            // Park the player (and its camera) on the room FLOOR at eye height,
            // centered — not at room_center, whose Y is the vertical midpoint and
            // left the camera up by the ceiling looking out into the void (black).
            let spawn = level_mgr
                .call(methods::ROOM_FLOOR_CENTER, &[Variant::from(0_i64)])
                .to::<Vector3>();
            if let Some(mut player) = parent.try_get_node_as::<Node3D>(nodes::PLAYER) {
                // Reset orientation too, not just position: the player is a
                // RigidBody that keeps whatever rotation it tumbled into, so
                // without this the camera faces a random direction.
                player.set_global_transform(Transform3D::new(Basis::IDENTITY, spawn));
                player.reset_physics_interpolation();
            }
        }
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
        let ship_select_vis = phase == GamePhase::ShipSelect;
        let bestiary_vis = phase == GamePhase::Bestiary;
        let death_vis = phase == GamePhase::Death;

        Self::set_ui_visible(&parent, nodes::MAIN_MENU_UI, menu_vis);
        Self::set_ui_visible(&parent, nodes::HUD, hud_vis);
        Self::set_ui_visible(&parent, nodes::PAUSE_MENU_UI, pause_vis);
        Self::set_ui_visible(&parent, nodes::KILL_SUMMARY_UI, summary_vis);
        Self::set_ui_visible(&parent, nodes::SHOP_UI, shop_vis);
        Self::set_ui_visible(&parent, nodes::SHIP_SELECT_UI, ship_select_vis);
        Self::set_ui_visible(&parent, nodes::BESTIARY_UI, bestiary_vis);
        Self::set_ui_visible(&parent, nodes::DEATH_SCREEN_UI, death_vis);

        // Show/hide gameplay elements (keep visible when paused). The player's
        // own ship stays hidden on the loadout/briefing screens (the showcase or
        // the bestiary turntable stands in front), but the room is the backdrop.
        let gameplay_vis = phase == GamePhase::Playing || phase == GamePhase::Paused;
        let level_vis = gameplay_vis || ship_select_vis || bestiary_vis;
        if let Some(mut player) = parent.try_get_node_as::<Node3D>(nodes::PLAYER) {
            player.set_visible(gameplay_vis);
            // Pilot input is live only in the flying phase (the policy lives in
            // void-logic) — otherwise the stick would rotate the camera on the
            // menu/showcase/bestiary screens.
            player.call(
                methods::SET_CONTROLS_ENABLED,
                &[Variant::from(phase.allows_piloting())],
            );
        }
        if let Some(mut level) = parent.try_get_node_as::<Node3D>(nodes::LEVEL_MANAGER) {
            level.set_visible(level_vis);
        }

        // One turntable serves both the ship "hero" shot (menu + loadout +
        // end-of-level screens) and the bestiary briefing. It self-positions in
        // front of the camera each frame, so we only choose its content here:
        // ship mode on the showcase screens; on the briefing the bestiary drives
        // show_entry itself (via refresh_bestiary); hidden everywhere else.
        let showcase_vis = menu_vis || ship_select_vis || summary_vis || shop_vis || death_vis;
        if let Some(mut turntable) = parent.try_get_node_as::<Node>(nodes::TURNTABLE) {
            if showcase_vis {
                turntable.call(
                    methods::SHOW_SHIP,
                    &[Variant::from(self.run_state.ship_color.id())],
                );
            } else if !bestiary_vis {
                turntable.call(methods::HIDE_TURNTABLE, &[]);
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

    /// Push RunState's loadout and laser to ShipController after a reset/restore.
    fn sync_player_state(&self) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut player) = parent.try_get_node_as::<Node>(nodes::PLAYER) {
            // Reset ShipController's local cache
            player.call(methods::RESET_LOADOUT, &[]);
            // Push current laser level
            let level = self.run_state.laser_level as i32;
            player.call(methods::SET_LASER_LEVEL, &[Variant::from(level)]);
            // Re-apply all upgrades from RunState's loadout
            for upgrade in &self.run_state.loadout.upgrades {
                player.call(methods::APPLY_UPGRADE, &[
                    Variant::from(GString::from(&upgrade.name)),
                    Variant::from(upgrade.kind as i32),
                    Variant::from(upgrade.multiplier),
                ]);
            }
            // Push the chosen ship color (drives body-style texture + accent) and
            // its thrust tradeoff.
            player.call(methods::CONFIGURE_SHIP, &[
                Variant::from(self.run_state.ship_color.id()),
                Variant::from(self.run_state.ship_color.thrust_mul()),
            ]);
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
                Variant::from(self.run_state.components.balance as i64),
                Variant::from(GString::from(self.run_state.laser_level.display_name())),
                Variant::from(color),
                Variant::from(self.run_state.laser_damage().as_f32()),
                Variant::from(self.get_next_upgrade_cost()),
                Variant::from(self.can_afford_upgrade()),
                Variant::from(self.is_max_laser()),
            ]);
        }
    }

    /// Load persisted display options into the single GameOptions.
    /// A missing file leaves the defaults in place. Called before the
    /// startup broadcast so every consumer is seeded from the remembered
    /// preference.
    fn load_options(&mut self) {
        let Some(cfg) = persistence::load(OPTIONS_FILE) else {
            return; // no saved preferences yet
        };
        self.game_options.sbs_enabled = cfg
            .get_value_ex(OPTIONS_SECTION, "sbs")
            .default(&self.game_options.sbs_enabled.to_variant())
            .done()
            .to();
        self.game_options.msaa_enabled = cfg
            .get_value_ex(OPTIONS_SECTION, "msaa")
            .default(&self.game_options.msaa_enabled.to_variant())
            .done()
            .to();
    }

    /// Persist the current options so they are remembered next launch.
    fn save_options(&self) {
        persistence::save(
            OPTIONS_FILE,
            OPTIONS_SECTION,
            &[
                ("sbs", self.game_options.sbs_enabled.to_variant()),
                ("msaa", self.game_options.msaa_enabled.to_variant()),
            ],
        );
    }

    /// Persist the current run snapshot (serialized via serde) so
    /// "Continue" survives a quit. Reuses the same ConfigFile path as
    /// options — one persistence mechanism, two files.
    fn save_run(&self) {
        if let Some(save) = &self.save_game {
            persistence::save(
                SAVE_FILE,
                SAVE_SECTION,
                &[("json", save.to_json().to_variant())],
            );
        }
    }

    /// Load a persisted run into the in-memory SaveGame at startup.
    fn load_run(&mut self) {
        let Some(cfg) = persistence::load(SAVE_FILE) else {
            return;
        };
        let json: GString = cfg
            .get_value_ex(SAVE_SECTION, "json")
            .default(&GString::new().to_variant())
            .done()
            .to();
        self.save_game = SaveGame::from_json(&json.to_string());
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

        // Connect ShipSelectUI
        if let Some(ship_select) = Self::find_ui_node(&parent, nodes::SHIP_SELECT_UI) {
            let color_callable = self.base().callable(methods::ON_SHIP_COLOR_SELECTED);
            let continue_callable = self.base().callable(methods::ADVANCE_FROM_SHIP_SELECT);
            if !ship_select.is_connected(signals::SHIP_COLOR_SELECTED, &color_callable) {
                let mut ship_select = ship_select;
                ship_select.connect(signals::SHIP_COLOR_SELECTED, &color_callable);
                ship_select.connect(signals::CONTINUE_PRESSED, &continue_callable);
            }
        }

        // Connect BestiaryUI: left stick browses the catalog, Select begins.
        if let Some(bestiary) = Self::find_ui_node(&parent, nodes::BESTIARY_UI) {
            let begin_callable = self.base().callable(methods::ADVANCE_FROM_BESTIARY);
            if !bestiary.is_connected(signals::CONTINUE_PRESSED, &begin_callable) {
                let mut bestiary = bestiary;
                bestiary.connect(signals::CONTINUE_PRESSED, &begin_callable);
                let paged_callable = self.base().callable(methods::ON_BESTIARY_PAGED);
                bestiary.connect(signals::BESTIARY_PAGED, &paged_callable);
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
                let slow_callable = self.base().callable(methods::ON_PLAYER_SLOWED);
                player.connect(signals::PLAYER_SLOWED, &slow_callable);
                let collide_callable = self.base().callable(methods::ON_PLAYER_COLLIDED);
                player.connect(signals::PLAYER_COLLIDED, &collide_callable);
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

        let kill_callable = self.base().callable(methods::ON_ENEMY_KILLED);
        let portal_callable = self.base().callable(methods::ON_PORTAL_ENTERED);
        let upgrade_callable = self.base().callable(methods::ON_UPGRADE_COLLECTED);
        let organics_callable = self.base().callable(methods::ON_ORGANICS_COLLECTED);

        // Scan LevelManager children (enemies, portals, organics barrels)
        for child in level_mgr.get_children().iter_shared() {
            if child.has_signal(signals::ENEMY_KILLED) && !child.is_connected(signals::ENEMY_KILLED, &kill_callable) {
                let mut c = child.clone();
                c.connect(signals::ENEMY_KILLED, &kill_callable);
            }
            if child.has_signal(signals::PORTAL_ENTERED) && !child.is_connected(signals::PORTAL_ENTERED, &portal_callable) {
                let mut c = child.clone();
                c.connect(signals::PORTAL_ENTERED, &portal_callable);
            }
            if child.has_signal(signals::ORGANICS_COLLECTED) && !child.is_connected(signals::ORGANICS_COLLECTED, &organics_callable) {
                let mut c = child;
                c.connect(signals::ORGANICS_COLLECTED, &organics_callable);
            }
        }

        // Scan scene root children for lootboxes (spawned by enemy death, added to root)
        if let Some(root) = self.base().get_tree().get_root() {
            for child in root.get_children().iter_shared() {
                if child.has_signal(signals::UPGRADE_COLLECTED) && !child.is_connected(signals::UPGRADE_COLLECTED, &upgrade_callable) {
                    let mut c = child;
                    c.connect(signals::UPGRADE_COLLECTED, &upgrade_callable);
                }
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
            hud.call(methods::UPDATE_COMPONENTS, &[
                Variant::from(self.run_state.components.balance as i64),
            ]);
            hud.call(methods::UPDATE_ORGANICS, &[
                Variant::from(self.run_state.organics.balance as i64),
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


    /// Seed policy for a new run: pinned by `fixed_seed` for
    /// reproducible runs, otherwise drawn from OS entropy. Policy
    /// lives here in the shell; void-logic stays deterministic.
    fn fresh_run_seed(&self) -> Seed {
        if self.fixed_seed != 0 {
            Seed::from_i64(self.fixed_seed)
        } else {
            Seed::new(rand::rng().random())
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
            level_mgr.set(properties::CURRENT_LEVEL, &Variant::from(self.run_state.current_level as i32));
            // Generate with the seed derived from the run seed and level.
            let seed = self.run_state.level_seed();
            let target_rooms = rooms_for_level(self.run_state.current_level) as u32;
            level_mgr.call(
                methods::GENERATE_LEVEL,
                &[Variant::from(seed.as_i64()), Variant::from(target_rooms)],
            );
        }
    }
}
