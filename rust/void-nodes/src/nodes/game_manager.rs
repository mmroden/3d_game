use godot::prelude::*;
use godot::classes::{
    Node, INode, Engine, CanvasLayer, Input, InputEvent,
    input::MouseMode,
};

use super::level_manager::LevelManager;
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

use super::constants::{actions, groups, map_flags, signals, methods, nodes, properties};
use super::godot_util;
use void_logic::audio_catalog::{self, MusicBed, SfxEvent};
use void_logic::bestiary::{self, BestiaryKind};
use void_logic::boss_fight::BossFight;
use void_logic::game_options::GameOptions;
use void_logic::game_phase::GamePhase;
use void_logic::input_method::InputMethod;
use void_logic::level_map;
use void_logic::level_spec::LevelSpec;
use void_logic::newtypes::Damage;
use void_logic::power_routing::PowerMode;
use void_logic::currency::CurrencyKind;
use void_logic::run_state::{RunState, DamageOutcome};
use void_logic::save_game::SaveGame;
use void_logic::seed::Seed;
use void_logic::ship::ShipColor;
use void_logic::ship_type::ShipType;
use void_logic::shop::{self, Receipt, ShopItemId};
use void_logic::unlocks::Unlock;

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
    /// Countdown (process ticks) to the deferred sector build — see
    /// `transition_to`'s Playing arm. 0 = nothing pending.
    pending_level_build: u8,
    current_power_mode: i32,
    /// Which bestiary entry the pre-level briefing is currently showing.
    bestiary_index: usize,
    /// Set when the shop was entered by losing a life: its Continue restarts
    /// the current level (same seed) instead of advancing to the next.
    pending_respawn: bool,
    /// The staged fight on a boss level (`boss::boss_for_level`), `None`
    /// elsewhere. Reset on every level entry; GameManager alone advances it
    /// and fans the beats out to gate, escorts, music, and portal.
    boss_fight: Option<BossFight>,
    /// The typed description of the current level — constructed once per
    /// level entry (THE door for its attributes) and retained for its
    /// lifetime; the same value is handed to LevelManager's build.
    level_spec: Option<LevelSpec>,

    /// Pins the run seed for reproducible runs when nonzero (0 = random).
    /// Configuration, not build flavor: set it in the editor or a test
    /// scene to replay a run.
    #[export]
    fixed_seed: i64,

    /// Dev knob (same family as `fixed_seed`): when nonzero, a new game
    /// starts at this level — for inspecting how a level renders and what
    /// it fields, without walking the run. `make run LEVEL=7` sets it from
    /// the command line.
    #[export]
    start_level: i32,

    /// Edge detection for the F9/F10 level-hop cheat (held-state latch).
    debug_hop_held: (bool, bool),
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
            pending_level_build: 0,
            current_power_mode: 0,
            bestiary_index: 0,
            pending_respawn: false,
            boss_fight: None,
            level_spec: None,
            start_level: 0,
            debug_hop_held: (false, false),
            fixed_seed: 0,
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }

        // GameManager must process even when tree is paused (for pause toggle)
        self.base_mut().set_process_mode(godot::classes::node::ProcessMode::ALWAYS);

        // Dev knobs from the command line (`make run LEVEL=7 SEED=1` passes
        // `-- --level=7 --seed=1`): same knobs as the editor exports, one
        // more way to set them. Explicit args beat exported defaults.
        for arg in godot::classes::Os::singleton()
            .get_cmdline_user_args()
            .to_vec()
        {
            let arg = arg.to_string();
            if let Some(v) = arg.strip_prefix("--level=") {
                if let Ok(level) = v.parse::<i32>() {
                    self.start_level = level.max(0);
                }
            } else if let Some(v) = arg.strip_prefix("--seed=") {
                if let Ok(seed) = v.parse::<i64>() {
                    self.fixed_seed = seed;
                }
            }
        }

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
        // Dev level-hop (owner's ask 2026-07-04): F10 next / F9 previous
        // while flying. Obscure keys, dev-phase game; revisit before any
        // release build. Edge-latched so a held key hops once.
        if self.phase == GamePhase::Playing {
            let input = Input::singleton();
            let f9 = input.is_key_pressed(godot::global::Key::F9);
            let f10 = input.is_key_pressed(godot::global::Key::F10);
            if f9 && !self.debug_hop_held.0 {
                self.debug_jump_level(-1);
            }
            if f10 && !self.debug_hop_held.1 {
                self.debug_jump_level(1);
            }
            self.debug_hop_held = (f9, f10);
        }

        // A pending sector build waits out one rendered frame so the loading
        // veil actually paints (a synchronous build in the entry frame reads
        // as a freeze), then lands here.
        if self.pending_level_build > 0 {
            self.pending_level_build -= 1;
            if self.pending_level_build == 0 {
                if self.phase == GamePhase::Playing {
                    self.regenerate_level();
                }
                self.push_loading_veil(false);
            }
        }

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

        // Signal wiring is no longer a per-frame tax: the Faucet pools build the
        // whole level roster during the load, and `regenerate_level` wires it
        // once at the end of the build (see `wire_level_signals`).

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
        // A new game is the explicit clean slate: the WHOLE save goes,
        // profile included — organics, unlocks, bestiary (owner's call,
        // 2026-07-03). Run-over is different: the roguelite loop keeps the
        // profile there; choosing New Game from the menu does not.
        self.pending_respawn = false;
        self.wipe_save();
        self.run_state = RunState::new(self.fresh_run_seed());
        // Dev knob: drop the new run straight at the level under inspection
        // (renders, roster, and boss staging all follow from the level
        // number through the one build pathway).
        if self.start_level > 0 {
            self.run_state.current_level = self.start_level as u32;
        }
        self.sync_player_state();
        self.transition_to(GamePhase::ShipSelect);
        self.show_ship_select_ui();
    }

    /// Dev cheat: rebuild the sector at `current_level + delta` in place
    /// (clamped to level 1), skipping the shop loop — F9/F10 in flight, or
    /// callable directly. Rides the same deferred build as a real level
    /// entry, so boss staging, red containers, and pools all arrive true.
    #[func]
    pub fn debug_jump_level(&mut self, delta: i32) {
        if self.phase != GamePhase::Playing {
            return;
        }
        let current = self.run_state.current_level as i32;
        let target = (current + delta).max(1) as u32;
        if target == self.run_state.current_level {
            return;
        }
        self.run_state.current_level = target;
        godot_print!("DEBUG JUMP: rebuilding at level {target}");
        self.mark_level_enemies_seen();
        self.push_loading_veil(true);
        self.pending_level_build = 2;
    }

    /// Called from UI: continue from saved game. Only a run that finished a
    /// level is continuable (the menu hides Continue otherwise; this guard
    /// backs it).
    #[func]
    pub fn continue_game(&mut self) {
        if !self.has_continuable_run() {
            return;
        }
        // Don't touch the run unless the phase machine can enter Playing.
        if !self.phase.can_transition_to(GamePhase::Playing) {
            return;
        }
        if let Some(save) = &self.save_game {
            // The snapshot's own seed restores its layouts; without a
            // continuable run, only the profile applies to a fresh seed.
            let seed = save.run.as_ref().map(|r| r.run_seed)
                .unwrap_or_else(|| self.fresh_run_seed());
            let mut run = RunState::new(seed);
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
        // A kill comes off a LIVE drone, so the id must resolve — the
        // demand door panics on an undeclared one. Retired-id tolerance
        // belongs to history readers (kill summary, bestiary), not here.
        let grammar = void_logic::roster::roster();
        let id = grammar.expect_enemy_by_crossing_id(type_id as u16);
        let def = grammar.enemy(id);
        self.run_state.record_kill(def.crossing_id);
        godot_print!(
            "Kill: {} | Cache dropped: {} components",
            def.name, def.reward,
        );
        // A boss death advances the fight — but opens NOTHING: the gate
        // stays sealed until the reward is taken (see on_cache_collected).
        // "Is this the boss" is the STAGING's call, not a type check:
        // the fight advances only for the enemy the slot staged.
        let staged_boss = self
            .level_spec
            .as_ref()
            .and_then(|s| s.boss.as_ref())
            .map(|b| b.boss);
        if staged_boss == Some(id) {
            let drops = self
                .level_spec
                .as_ref()
                .and_then(|s| s.boss.as_ref())
                .map(|b| b.drop_count())
                .unwrap_or(0);
            if let Some(fight) = &mut self.boss_fight {
                if fight.defeat(drops) {
                    godot_print!(
                        "Boss down — gather all {drops} drops to open the arena"
                    );
                }
            }
        }
        self.queue_music_push();
    }

    /// The staged fight's current beat for the GDScript crossing:
    /// -1 = no boss on this level, else `BossFightState::id()` (0-3).
    #[func]
    pub fn boss_fight_state(&self) -> i32 {
        self.boss_fight.map(|f| f.state().id()).unwrap_or(-1)
    }

    /// How many purchasable hulls the profile owns (the starter never
    /// counts) — red containers are the only source now.
    #[func]
    pub fn owned_hull_count(&self) -> i32 {
        ShipType::ALL
            .iter()
            .filter(|s| {
                s.spec().organic_cost.is_some() && self.run_state.profile.unlocks.owns_ship(**s)
            })
            .count() as i32
    }

    /// The arena entry trigger fired: the player crossed into the boss room.
    /// Legal only from Dormant (the FSM refuses repeats): seal the gate
    /// behind the player, rise the OnEngage escorts, switch the music.
    #[func]
    pub fn on_boss_arena_entered(&mut self) {
        let engaged = self
            .boss_fight
            .as_mut()
            .map(|f| f.engage())
            .unwrap_or(false);
        if !engaged {
            return;
        }
        godot_print!("Boss fight engaged — the arena seals");
        self.set_boss_seal(true);
        self.queue_music_push();
        // Every enemy gets the engage fan-out; `activate_escorts` is a no-op
        // for drones without OnEngage minions, so no type filtering here.
        let tree = self.base().get_tree();
        for node in tree.get_nodes_in_group(groups::ENEMIES).iter_shared() {
            let mut node = node;
            if node.has_method(methods::ACTIVATE_ESCORTS) {
                node.call(methods::ACTIVATE_ESCORTS, &[]);
            }
        }
    }

    /// Fan a gate flip out to the LevelManager.
    fn set_boss_seal(&self, sealed: bool) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut lm) = parent.try_get_node_as::<Node>(nodes::LEVEL_MANAGER) {
            lm.call(methods::SEAL_BOSS_GATE, &[Variant::from(sealed)]);
        }
    }

    /// Fan a portal flip out to the LevelManager.
    fn set_boss_portal(&self, active: bool) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut lm) = parent.try_get_node_as::<Node>(nodes::LEVEL_MANAGER) {
            lm.call(methods::SET_BOSS_PORTAL_ACTIVE, &[Variant::from(active)]);
        }
    }

    /// The music conductor: derive the bed from the FULL input set (phase,
    /// fight beat, live enemies in the player's room — see
    /// `audio_catalog::music_bed`) and push it to the AudioManager. ONE
    /// derivation, re-run at every input change; the AudioManager holds no
    /// music lifecycle of its own (the old boss_mode flag rotted across
    /// death-with-life and save-and-exit precisely because it did).
    #[func]
    fn push_music_bed(&mut self) {
        let boss = self.boss_fight.map(|f| f.state());
        let enemies = self
            .base()
            .get_parent()
            .and_then(|p| p.try_get_node_as::<LevelManager>(nodes::LEVEL_MANAGER))
            .map(|lm| lm.bind().live_enemies_in_current_room())
            .unwrap_or(false);
        let bed = audio_catalog::music_bed(self.phase, boss, enemies);
        let track = match bed {
            MusicBed::Menu => audio_catalog::menu_track().to_string(),
            MusicBed::Combat => String::new(), // the AudioManager rolls the stinger
            MusicBed::Level => match &self.level_spec {
                Some(spec) => spec.background.clone(),
                None => return, // pre-spec boot frame: nothing to play yet
            },
            MusicBed::Boss => match self.level_spec.as_ref().and_then(|s| s.boss.as_ref()) {
                Some(staging) => staging.track.clone(),
                None => return,
            },
        };
        if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
            audio
                .bind_mut()
                .set_music_bed(bed.id(), GString::from(track.as_str()));
        }
    }

    /// Queue a bed re-derivation for after the message-queue flush: the
    /// inputs settle through that same queue (corpse frees, dormancy
    /// flips), so the conductor must read the world AFTER they land.
    fn queue_music_push(&mut self) {
        self.base_mut()
            .call_deferred(methods::PUSH_MUSIC_BED, &[]);
    }

    /// Called when player enters the portal. The continuable-run snapshot is
    /// NOT written here — it lands at the start of the next level, after the
    /// shop; see `transition_to`.
    #[func]
    pub fn on_portal_entered(&mut self) {
        if self.phase == GamePhase::Playing {
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
            self.transition_to(GamePhase::Shop);
            self.show_shop_ui();
        }
    }

    /// Called from shop UI: buy the item with this typed id. Validation,
    /// pricing, and mutation all live in `void_logic::shop::purchase`; this
    /// only routes the receipt to consumers and refreshes the screen.
    #[func]
    pub fn buy_shop_item(&mut self, item_id: i32) -> bool {
        let Some(id) = ShopItemId::from_id(item_id) else {
            godot_warn!("buy_shop_item: unknown item id {item_id}");
            return false;
        };
        match shop::purchase(&mut self.run_state, id) {
            Ok(receipt) => {
                // No disk write here: the save IS the level start. Purchases
                // persist through the next level-start snapshot; quitting
                // mid-shop rewinds to the level start, purchase and price
                // alike (nothing is lost, nothing is ratcheted).
                match &receipt {
                    Receipt::UpgradeAdded(kind) => {
                        // Push to ShipController's cache, like any loadout sync.
                        if let Some(parent) = self.base().get_parent() {
                            if let Some(mut player) = parent.try_get_node_as::<Node>(nodes::PLAYER) {
                                player.call(methods::APPLY_UPGRADE, &[
                                    Variant::from(kind.id()),
                                ]);
                            }
                        }
                    }
                    Receipt::LaserChanged(next) => {
                        godot_print!("Laser upgraded to {} (damage: {})",
                            next.display_name(), next.damage());
                        self.update_player_laser();
                    }
                    Receipt::LifeAdded(lives) => {
                        godot_print!("Extra life bought ({} total)", lives);
                    }
                    Receipt::ChargeAdded(charges) => {
                        godot_print!("Surge charge bought ({charges} in the rack)");
                    }
                    Receipt::UnlockGranted(unlock) => {
                        godot_print!("Permanent unlock bought: {}", unlock.display_name());
                        // A permanent purchase must never be lostable —
                        // profile to disk immediately, run snapshot untouched.
                        self.persist_profile();
                        if *unlock == Unlock::Valkyrie {
                            self.push_valkyrie_owned(true);
                        }
                    }
                }
                self.refresh_shop_ui();
                true
            }
            Err(refusal) => {
                godot_print!("Purchase refused: {refusal:?}");
                false
            }
        }
    }

    /// Called from the ship's item trigger: spend a Shield Surge charge for
    /// instant shields. A no-op with an empty rack.
    #[func]
    pub fn on_shield_burst_requested(&mut self) {
        if self.phase != GamePhase::Playing {
            return;
        }
        if self.run_state.use_shield_burst() {
            self.update_hud();
        }
    }

    /// Called from the shop UI's Save & Exit row: bank everything up to this
    /// point and return to the menu. The save IS the level start, so a
    /// between-levels exit advances first (Continue resumes at the next
    /// level, purchases included); a between-lives exit keeps the current
    /// level (the life was already spent, the level restarts on Continue).
    #[func]
    pub fn save_and_exit(&mut self) {
        if self.phase != GamePhase::Shop {
            return;
        }
        if self.pending_respawn {
            self.pending_respawn = false;
        } else {
            self.run_state.advance_level();
        }
        self.save_game = Some(SaveGame::from_run_state(&self.run_state));
        self.save_run();
        self.transition_to(GamePhase::MainMenu);
    }

    /// Called from shop UI: continue out of the shop — to the loadout screen
    /// and the next level normally, or back into the current level (same
    /// seed) when the shop was entered by losing a life.
    #[func]
    pub fn advance_to_next_level(&mut self) {
        if self.phase != GamePhase::Shop {
            return;
        }
        if self.pending_respawn {
            // The life was already spent (apply_life_loss); the ship is
            // restored — re-enter the same level, which regenerates from the
            // unchanged level seed.
            self.pending_respawn = false;
            self.transition_to(GamePhase::Playing);
            return;
        }
        self.run_state.advance_level();
        self.transition_to(GamePhase::ShipSelect);
        self.show_ship_select_ui();
    }

    /// Called from the ship-select UI: a color was chosen — apply it live.
    #[func]
    pub fn on_ship_color_selected(&mut self, color_id: i32) {
        let Some(color) = ShipColor::from_id(color_id) else { return };
        self.run_state.set_ship_color(color);
        self.sync_player_state();
        self.update_hud();
        self.refresh_showcase_ship();
    }

    /// Called from the ship-select UI: a hull was chosen. Ownership is
    /// validated here (the UI greys locked hulls, the authority backs it).
    #[func]
    pub fn on_ship_type_selected(&mut self, ship_type_id: i32) {
        let Some(ship) = ShipType::from_id(ship_type_id) else { return };
        if !self.run_state.profile.unlocks.owns_ship(ship) {
            // Expected guard: the UI greys locked hulls; the authority backs it.
            godot_print!("on_ship_type_selected: {ship:?} is not owned — ignored");
            return;
        }
        self.run_state.set_ship_type(ship);
        self.sync_player_state();
        self.update_hud();
        self.refresh_showcase_ship();
    }

    /// Re-model/re-skin the showcase ship behind the loadout screens to the
    /// current hull and trim.
    fn refresh_showcase_ship(&self) {
        if let Some(parent) = self.base().get_parent() {
            if let Some(mut turntable) = parent.try_get_node_as::<Node>(nodes::TURNTABLE) {
                turntable.call(methods::SHOW_SHIP, &[
                    Variant::from(self.run_state.ship_type.id()),
                    Variant::from(self.run_state.ship_color.id()),
                ]);
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

    /// Called from the bestiary UI's back press (circle is always back):
    /// return to the loadout screen. The two share one backdrop, so this is
    /// a content flip, not a rebuild.
    #[func]
    pub fn back_from_bestiary(&mut self) {
        if self.phase == GamePhase::Bestiary {
            self.transition_to(GamePhase::ShipSelect);
            self.show_ship_select_ui();
        }
    }

    /// Called from the bestiary UI's menu up/down: browse the catalog by `delta`
    /// (-1 prev, +1 next), clamped to the ends — no wrap, no auto-begin.
    #[func]
    pub fn on_bestiary_paged(&mut self, delta: i32) {
        if self.phase != GamePhase::Bestiary {
            return;
        }
        let total = bestiary::entries(&self.run_state.profile.seen_enemies).len();
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
        let entries = bestiary::entries(&self.run_state.profile.seen_enemies);
        let Some(entry) = entries.get(self.bestiary_index) else { return };
        let Some(parent) = self.base().get_parent() else { return };

        let (kind_id, enemy_id): (i32, i32) = match entry.kind {
            BestiaryKind::OrganicCache => (0, -1),
            BestiaryKind::ComponentCache => (1, -1),
            BestiaryKind::Enemy(t) => {
                (2, void_logic::roster::roster().enemy(t).crossing_id as i32)
            }
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

    /// Populate and show the ship-select UI: current hull and color marked,
    /// per-hull ownership (from the permanent unlocks) gating selection.
    fn show_ship_select_ui(&self) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut ui) = Self::find_ui_node(&parent, nodes::SHIP_SELECT_UI) {
            let owned: PackedByteArray = ShipType::ALL.iter()
                .map(|ship| self.run_state.profile.unlocks.owns_ship(*ship) as u8)
                .collect();
            ui.call(methods::SHOW_SHIP_SELECT, &[
                Variant::from(self.run_state.ship_type.id()),
                Variant::from(self.run_state.ship_color.id()),
                Variant::from(owned),
            ]);
        }
    }

    /// Called when player dies: a spare life keeps the run alive (through the
    /// shop, back into the same level); the last life ends it.
    #[func]
    pub fn on_player_death(&mut self) {
        if self.phase != GamePhase::Playing {
            return;
        }
        let level_reached = self.run_state.current_level as i32;

        if self.run_state.has_spare_life() {
            self.run_state.apply_life_loss();
            self.transition_to(GamePhase::Death);
            let Some(parent) = self.base().get_parent() else { return };
            if let Some(mut death_ui) = Self::find_ui_node(&parent, nodes::DEATH_SCREEN_UI) {
                death_ui.call(methods::SHOW_LIFE_LOST, &[
                    Variant::from(self.run_state.lives as i32),
                    Variant::from(level_reached),
                ]);
            }
            return;
        }

        let old_laser = self.run_state.laser_level.display_name().to_string();
        self.run_state.apply_death_penalty();
        // Run over: the snapshot goes (no more Continue), the profile stays.
        match &mut self.save_game {
            Some(save) => {
                save.clear_run();
                save.update_profile(&self.run_state);
            }
            None => self.save_game = Some(SaveGame::profile_only(&self.run_state)),
        }
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

    /// Called when the player's room changes (LevelManager's culling
    /// detection): visit it in RunState and, on a first visit, refresh the
    /// recon map from the retained graph.
    #[func]
    pub fn on_room_changed(&mut self, room: i64) {
        if room < 0 {
            return;
        }
        // Every room change re-pushes the map: the fog only lifts on first
        // visits (visit_room feeds map_rects), but the current marker must
        // follow the player through KNOWN rooms too (playtest 2026-07-04:
        // it stuck on the last new room). Still per room change, never per
        // frame.
        self.run_state.visit_room(room as usize);
        self.push_map_view();
        self.push_radar_contacts();
        self.queue_music_push();
    }

    /// Scope the radar to the lit neighborhood: pull the contact set from
    /// LevelManager (the cull-visibility authority at RADAR_ROOM_DEPTH) and
    /// push it to the HUD. Runs on every room change — the set follows the
    /// player through the level.
    fn push_radar_contacts(&self) {
        let Some(parent) = self.base().get_parent() else { return };
        let Some(level_mgr) = parent.try_get_node_as::<LevelManager>(nodes::LEVEL_MANAGER) else {
            return;
        };
        let contacts = level_mgr.bind().radar_contacts();
        if let Some(mut hud) = Self::find_ui_node(&parent, nodes::HUD) {
            hud.call(methods::SET_RADAR_CONTACTS, &[Variant::from(contacts)]);
        }
    }

    /// Derive the recon-map view (void-logic, on the retained LevelGraph)
    /// and push it to the HUD's corner widget as packed arrays.
    fn push_map_view(&self) {
        let Some(parent) = self.base().get_parent() else { return };
        let Some(level_mgr) = parent.try_get_node_as::<LevelManager>(nodes::LEVEL_MANAGER) else {
            return;
        };
        let (view, projection) = {
            let lm = level_mgr.bind();
            (
                level_map::map_rects(
                    lm.graph(),
                    &self.run_state.rooms_visited,
                    self.run_state.current_room,
                    self.run_state.profile.unlocks.contains(Unlock::RouteScanner),
                ),
                level_map::map_projection(
                    lm.graph(),
                    self.level_spec.as_ref().map(|s| s.pitch).unwrap_or_else(
                        || void_logic::planet::Pitch::for_level(self.run_state.current_level),
                    ),
                ),
            )
        };

        let mut rects = PackedFloat32Array::new();
        let mut flags = PackedByteArray::new();
        for r in &view {
            for v in r.rect {
                rects.push(v);
            }
            let mut flag = 0u8;
            if r.current { flag |= map_flags::CURRENT; }
            if r.corridor { flag |= map_flags::CORRIDOR; }
            if r.frontier { flag |= map_flags::FRONTIER; }
            flags.push(flag);
        }

        // The world→unit projection rides along so the panel can place the
        // LIVE player marker between room-change pushes.
        let proj = PackedFloat32Array::from(&[
            projection.scale,
            projection.offset[0],
            projection.offset[1],
        ][..]);

        if let Some(mut hud) = Self::find_ui_node(&parent, nodes::HUD) {
            hud.call(methods::UPDATE_MAP, &[
                rects.to_variant(),
                flags.to_variant(),
                proj.to_variant(),
            ]);
        }
    }

    /// Called from the death screen after a life loss: re-arm at the shop,
    /// whose Continue then restarts the current level.
    #[func]
    pub fn on_respawn_pressed(&mut self) {
        if self.phase == GamePhase::Death {
            self.pending_respawn = true;
            self.transition_to(GamePhase::Shop);
            self.show_shop_ui();
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

    /// Test seam (mirrors `reload_options_from_disk`): forget the persisted
    /// save, memory and disk — a fresh install. Real persistence otherwise
    /// leaks profiles (organics, unlocks) across tests within one run and
    /// makes them order-dependent. The GUT Makefile target runs under an
    /// isolated HOME, so this can never touch a developer's real profile.
    #[func]
    fn clear_save_for_tests(&mut self) {
        self.wipe_save();
    }

    /// Forget the save, memory and disk. New Game rides this (the explicit
    /// clean slate); tests reuse it via `clear_save_for_tests`.
    fn wipe_save(&mut self) {
        self.save_game = None;
        godot::classes::DirAccess::remove_absolute(SAVE_FILE);
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

    /// The player's ship rammed static geometry. Costs a flat point off the top
    /// (shield, then hull) so careless careening isn't consequence-free, and the
    /// layer that took it picks the sound: a cushioned clang while the shield
    /// holds, bare metal once it's down.
    #[func]
    pub fn on_player_collided(&mut self) {
        if self.phase != GamePhase::Playing {
            return;
        }
        let outcome = self.run_state.take_collision_damage();
        let event = match outcome {
            DamageOutcome::ShieldHeld => SfxEvent::CollisionShielded,
            DamageOutcome::HullHit => SfxEvent::CollisionBare,
        };
        if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
            audio.bind_mut().play_event(event);
        }
        if !self.run_state.is_alive() {
            self.on_player_death();
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

    /// Called when a currency cache is collected — credit the matching account
    /// and refresh the HUD. The only reward path in a level: nothing credits
    /// either account without this pickup. Green is permanent the moment it
    /// is banked, so it writes the profile immediately.
    #[func]
    pub fn on_cache_collected(&mut self, kind_id: i32, amount: i64, boss_loot: bool) {
        let Some(kind) = CurrencyKind::from_id(kind_id) else { return };
        if kind == CurrencyKind::HullReward {
            // The red container: a hull, not a balance. The hull was rolled
            // ONCE, at spec construction — the grant reads that staging; the
            // components fallback is a never-expected safety arm.
            match self
                .level_spec
                .as_ref()
                .and_then(|s| s.boss.as_ref())
                .and_then(|b| b.hull_reward)
            {
                Some(hull) => {
                    self.run_state.profile.unlocks.grant(Unlock::Ship(hull));
                    self.persist_profile();
                    godot_print!(
                        "RED CONTAINER: the {} joins the fleet",
                        hull.spec().display_name
                    );
                }
                None => self.run_state.collect_cache(kind, amount.max(0) as u32),
            }
        } else {
            self.run_state.collect_cache(kind, amount.max(0) as u32);
        }
        if kind == CurrencyKind::Organics {
            self.persist_profile();
        }
        // Boss loot: the fight closes only when the COMPLETE drop set is
        // gathered (the FSM counts what `defeat` staged) — an ordinary cache
        // never advances it. The stamp crosses with the signal.
        let collected = boss_loot
            && self
                .boss_fight
                .as_mut()
                .map(|f| f.collect_boss_loot())
                .unwrap_or(false);
        if collected {
            godot_print!("Boss reward collected — the arena opens");
            self.set_boss_seal(false);
            self.set_boss_portal(true);
            self.queue_music_push();
        }
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
            // Defensive: a respawn armed but never taken must not leak into
            // the next run's first shop visit.
            self.pending_respawn = false;
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
        self.run_state.profile.organics.balance as i64
    }

    #[func]
    pub fn get_shield_charges(&self) -> i32 {
        self.run_state.shield_charges as i32
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
        let grammar = void_logic::roster::roster();
        for (crossing_id, count) in self.run_state.kills.summary() {
            // summary() lists only ids the grammar declares, so the def
            // resolves; a retired id counts toward totals but is unlisted.
            if let Some(id) = grammar.enemy_by_crossing_id(crossing_id) {
                dict.set(grammar.enemy(id).name.as_str(), count as i32);
            }
        }
        dict
    }

    #[func]
    pub fn get_lives(&self) -> i32 {
        self.run_state.lives as i32
    }

    /// Whether the menu offers Continue at all: a live snapshot resumes;
    /// a profile-only save (post-run-over) restarts sector 1 with the
    /// profile applied (B8). Only a truly fresh install offers nothing.
    #[func]
    pub fn has_continuable_run(&self) -> bool {
        self.save_game.is_some()
    }

    /// Whether Continue would RESTART sector 1 (profile-only save — the
    /// dead run's snapshot is gone) rather than resume a live snapshot.
    #[func]
    pub fn continue_restarts_run(&self) -> bool {
        self.save_game.as_ref().is_some_and(|save| !save.has_run())
    }

    /// Whether the permanent unlock with this id is owned (Unlock::id).
    #[func]
    pub fn has_unlock(&self, unlock_id: i32) -> bool {
        Unlock::from_id(unlock_id)
            .is_some_and(|unlock| self.run_state.profile.unlocks.contains(unlock))
    }

    /// The chosen hull's id (ShipType::id).
    #[func]
    pub fn get_ship_type_id(&self) -> i32 {
        self.run_state.ship_type.id()
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
        self.queue_music_push();

        // The phase machine owns lifecycle effects: entering Playing
        // means a fresh level for the current RunState — except when
        // resuming from pause, which returns to the level in progress.
        // The build itself defers to `process` (two ticks: the first may
        // land in the entry frame) so the loading veil gets a rendered
        // frame — one choke point covers new game, Continue, the next
        // level, and respawns alike.
        if next == GamePhase::Playing && prev != GamePhase::Paused {
            self.mark_level_enemies_seen();
            self.push_loading_veil(true);
            self.pending_level_build = 2;
            // A run becomes continuable at the start of level 2 — it has
            // finished a level, the definition of continuable. The snapshot
            // freezes the level start (post-shop, purchases included); a
            // mid-level quit resumes here. An unfinished first level never
            // writes one.
            if self.run_state.current_level >= 2 {
                self.save_game = Some(SaveGame::from_run_state(&self.run_state));
                self.save_run();
            }
        } else if self.pending_level_build > 0 && next != GamePhase::Playing {
            // Left Playing before the deferred build landed — the sector is
            // no longer wanted; drop the veil with it.
            self.pending_level_build = 0;
            self.push_loading_veil(false);
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
        // Coverage, not the direct roster: it includes death-spawn-only types
        // (the SpawnDrone an EyeDrone drops), so the briefing lists a type the
        // moment the level can produce it. `enemies_for_level` filtered on
        // `spawns_directly()` and could never surface a death-only enemy.
        let coverage = self
            .level_spec
            .as_ref()
            .map(|s| s.coverage.clone())
            .unwrap_or_default();
        let grammar = void_logic::roster::roster();
        for enemy in coverage {
            if self.run_state.mark_enemy_seen(grammar.enemy(enemy).crossing_id) {
                grew = true;
            }
        }
        if grew {
            self.persist_profile();
        }
    }

    /// Permanent state changed (green banked, bestiary grew): write the
    /// profile now, leaving any run snapshot frozen at its level start.
    fn persist_profile(&mut self) {
        match &mut self.save_game {
            Some(save) => save.update_profile(&self.run_state),
            None => self.save_game = Some(SaveGame::profile_only(&self.run_state)),
        }
        self.save_run();
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
        // The menu never reads disk: GameManager pushes whether a continuable
        // run exists every time the menu is shown (broadcast discipline).
        if menu_vis {
            if let Some(mut menu) = Self::find_ui_node(&parent, nodes::MAIN_MENU_UI) {
                menu.call(methods::SET_CONTINUE_AVAILABLE, &[
                    Variant::from(self.has_continuable_run()),
                    Variant::from(self.continue_restarts_run()),
                ]);
            }
        }
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
                turntable.call(methods::SHOW_SHIP, &[
                    Variant::from(self.run_state.ship_type.id()),
                    Variant::from(self.run_state.ship_color.id()),
                ]);
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
            for kind in &self.run_state.loadout.upgrades {
                player.call(methods::APPLY_UPGRADE, &[Variant::from(kind.id())]);
            }
            // Push the chosen hull and trim (model, body-style texture,
            // accent) and the combined thrust tradeoff.
            let thrust_mul = self.run_state.ship_type.spec().thrust_mul
                * self.run_state.ship_color.thrust_mul();
            player.call(methods::CONFIGURE_SHIP, &[
                Variant::from(self.run_state.ship_type.id()),
                Variant::from(self.run_state.ship_color.id()),
                Variant::from(thrust_mul),
            ]);
            // Arm (or disarm) the Valkyrie from the profile — RESET_LOADOUT
            // above wiped the node's cache, and ownership is green state the
            // node must never read from disk itself.
            let valkyrie = self.run_state.profile.unlocks.contains(Unlock::Valkyrie);
            player.call(methods::SET_VALKYRIE_OWNED, &[Variant::from(valkyrie)]);
        }
    }


    /// Push Valkyrie ownership to the flying ship (purchase receipt path —
    /// the full sync also carries it, but a mid-shop grant must arm the
    /// cannon without waiting for the next rebuild).
    fn push_valkyrie_owned(&self, owned: bool) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut player) = parent.try_get_node_as::<Node>(nodes::PLAYER) {
            player.call(methods::SET_VALKYRIE_OWNED, &[Variant::from(owned)]);
        }
    }


    /// Raise or drop the loading veil around a deferred sector build.
    fn push_loading_veil(&self, up: bool) {
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut ui) = Self::find_ui_node(&parent, nodes::LOADING_UI) {
            if up {
                let level = self.run_state.current_level as i32;
                ui.call(methods::SHOW_LOADING, &[Variant::from(level)]);
            } else {
                ui.call(methods::HIDE_LOADING, &[]);
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

    /// Enter the shop screen (cursor reset, opening press swallowed).
    fn show_shop_ui(&self) {
        self.push_shop_catalog(methods::SHOW_SHOP);
    }

    /// Re-price the open shop after a buy (cursor stays on its row).
    fn refresh_shop_ui(&self) {
        self.push_shop_catalog(methods::REFRESH_SHOP);
    }

    /// Price the catalog in void-logic and push it to ShopUI as parallel
    /// packed arrays (typed id, label, cost, flag bits per row — the bit
    /// protocol lives in `constants::shop_flags`, shared with ShopUI).
    /// `method` picks enter-vs-refresh; the distinction is explicit because
    /// ShopUI cannot infer it (the phase machine shows the layer before
    /// this populates it).
    fn push_shop_catalog(&self, method: &str) {
        use super::constants::shop_flags;

        let Some(parent) = self.base().get_parent() else { return };
        if let Some(mut shop) = Self::find_ui_node(&parent, nodes::SHOP_UI) {
            let offers = shop::offers(&self.run_state);
            let ids: PackedInt32Array = offers.iter().map(|o| o.id.id()).collect();
            let labels: PackedStringArray = offers.iter().map(|o| GString::from(&o.label)).collect();
            let details: PackedStringArray = offers.iter().map(|o| GString::from(&o.detail)).collect();
            let costs: PackedInt64Array = offers.iter().map(|o| o.cost as i64).collect();
            let flags: PackedByteArray = offers.iter().map(|o| {
                let mut flag = 0u8;
                if o.affordable { flag |= shop_flags::AFFORDABLE; }
                if o.purchasable { flag |= shop_flags::PURCHASABLE; }
                if o.currency == CurrencyKind::Organics { flag |= shop_flags::GREEN; }
                flag
            }).collect();

            shop.call(method, &[
                Variant::from(self.run_state.components.balance as i64),
                Variant::from(self.run_state.profile.organics.balance as i64),
                Variant::from(ids),
                Variant::from(labels),
                Variant::from(details),
                Variant::from(costs),
                Variant::from(flags),
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
            let buy_callable = self.base().callable(methods::BUY_SHOP_ITEM);
            let continue_callable = self.base().callable(methods::ADVANCE_TO_NEXT_LEVEL);
            if !shop.is_connected(signals::BUY_PRESSED, &buy_callable) {
                let mut shop = shop;
                shop.connect(signals::BUY_PRESSED, &buy_callable);
                shop.connect(signals::CONTINUE_PRESSED, &continue_callable);
                let save_exit_callable = self.base().callable(methods::SAVE_AND_EXIT);
                shop.connect(signals::SAVE_EXIT_PRESSED, &save_exit_callable);
            }
        }

        // Connect ShipSelectUI
        if let Some(ship_select) = Self::find_ui_node(&parent, nodes::SHIP_SELECT_UI) {
            let color_callable = self.base().callable(methods::ON_SHIP_COLOR_SELECTED);
            let type_callable = self.base().callable(methods::ON_SHIP_TYPE_SELECTED);
            let continue_callable = self.base().callable(methods::ADVANCE_FROM_SHIP_SELECT);
            if !ship_select.is_connected(signals::SHIP_COLOR_SELECTED, &color_callable) {
                let mut ship_select = ship_select;
                ship_select.connect(signals::SHIP_COLOR_SELECTED, &color_callable);
                ship_select.connect(signals::SHIP_TYPE_SELECTED, &type_callable);
                ship_select.connect(signals::CONTINUE_PRESSED, &continue_callable);
            }
        }

        // Connect BestiaryUI: menu up/down browses the catalog, Select begins.
        if let Some(bestiary) = Self::find_ui_node(&parent, nodes::BESTIARY_UI) {
            let begin_callable = self.base().callable(methods::ADVANCE_FROM_BESTIARY);
            if !bestiary.is_connected(signals::CONTINUE_PRESSED, &begin_callable) {
                let mut bestiary = bestiary;
                bestiary.connect(signals::CONTINUE_PRESSED, &begin_callable);
                let paged_callable = self.base().callable(methods::ON_BESTIARY_PAGED);
                bestiary.connect(signals::BESTIARY_PAGED, &paged_callable);
                let back_callable = self.base().callable(methods::BACK_FROM_BESTIARY);
                bestiary.connect(signals::BACK_PRESSED, &back_callable);
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
                let burst_callable = self.base().callable(methods::ON_SHIELD_BURST_REQUESTED);
                player.connect(signals::SHIELD_BURST_REQUESTED, &burst_callable);
            }
        }

        // Connect DeathScreenUI: run over returns to the menu, a lost life
        // re-arms at the shop.
        if let Some(death) = Self::find_ui_node(&parent, nodes::DEATH_SCREEN_UI) {
            let callable = self.base().callable(methods::RETURN_TO_MENU);
            if !death.is_connected(signals::RETURN_PRESSED, &callable) {
                let mut death = death;
                death.connect(signals::RETURN_PRESSED, &callable);
                let respawn = self.base().callable(methods::ON_RESPAWN_PRESSED);
                death.connect(signals::RESPAWN_PRESSED, &respawn);
            }
        }
    }

    /// Connect a single entity's gameplay signals to the matching GameManager
    /// handlers, idempotently (skips if already wired). A node only reacts to
    /// the signals it actually has — a currency cache has `cache_collected`, an
    /// enemy has `enemy_killed` — so this is safe to call on every node,
    /// including room-geometry meshes that carry none.
    fn connect_entity_signals(
        node: &Gd<Node>,
        kill: &Callable,
        portal: &Callable,
        cache: &Callable,
        arena: &Callable,
    ) {
        if node.has_signal(signals::ENEMY_KILLED) && !node.is_connected(signals::ENEMY_KILLED, kill) {
            node.clone().connect(signals::ENEMY_KILLED, kill);
        }
        if node.has_signal(signals::PORTAL_ENTERED) && !node.is_connected(signals::PORTAL_ENTERED, portal) {
            node.clone().connect(signals::PORTAL_ENTERED, portal);
        }
        if node.has_signal(signals::CACHE_COLLECTED) && !node.is_connected(signals::CACHE_COLLECTED, cache) {
            node.clone().connect(signals::CACHE_COLLECTED, cache);
        }
        if node.has_signal(signals::BOSS_ARENA_ENTERED)
            && !node.is_connected(signals::BOSS_ARENA_ENTERED, arena)
        {
            node.clone().connect(signals::BOSS_ARENA_ENTERED, arena);
        }
    }

    /// Wire every gameplay-signal emitter in a freshly built level to the
    /// mediator, ONCE, at the end of the build — not every frame (Faucet
    /// Principle: the per-frame `connect_spawned_entities` tree scan is deleted).
    /// Because the Faucet pools pre-instantiate every enemy, death-spawn minion,
    /// and currency cache during the load, the whole roster exists under
    /// `LevelManager` by the time `build_level` returns; a single recursive pass
    /// covers them all (minions and caches live under containers/the level, not
    /// the scene root). Idempotent, so re-running on a rebuild rewires nothing
    /// already set.
    fn wire_level_signals(&self) {
        let Some(parent) = self.base().get_parent() else { return };
        let Some(level_mgr) = parent.try_get_node_as::<Node>(nodes::LEVEL_MANAGER) else { return };

        // The LevelManager itself reports room changes (recon map). Deferred
        // subscription: the handler binds back into LevelManager for the
        // graph, and the signal fires inside its &mut culling pass — the
        // deferral lives here at the subscription, not at the emit.
        let room = self.base().callable(methods::ON_ROOM_CHANGED);
        if !level_mgr.is_connected(signals::ROOM_CHANGED, &room) {
            level_mgr.clone().connect_flags(
                signals::ROOM_CHANGED,
                &room,
                godot::classes::object::ConnectFlags::DEFERRED,
            );
        }

        let kill = self.base().callable(methods::ON_ENEMY_KILLED);
        let portal = self.base().callable(methods::ON_PORTAL_ENTERED);
        let cache = self.base().callable(methods::ON_CACHE_COLLECTED);
        let arena = self.base().callable(methods::ON_BOSS_ARENA_ENTERED);

        Self::wire_subtree(&level_mgr.upcast(), &kill, &portal, &cache, &arena);
    }

    /// Depth-first wire of `node` and every descendant — enemies and their
    /// death-spawn minions sit several levels deep (under room containers), so
    /// the pass must recurse, not just scan two levels as the old per-frame scan
    /// did.
    fn wire_subtree(
        node: &Gd<Node>,
        kill: &Callable,
        portal: &Callable,
        cache: &Callable,
        arena: &Callable,
    ) {
        Self::connect_entity_signals(node, kill, portal, cache, arena);
        for child in node.get_children().iter_shared() {
            Self::wire_subtree(&child, kill, portal, cache, arena);
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
            hud.call(methods::UPDATE_LIVES, &[
                Variant::from(self.run_state.lives as i32),
            ]);
            hud.call(methods::UPDATE_CHARGES, &[
                Variant::from(self.run_state.shield_charges as i32),
            ]);
            hud.call(methods::UPDATE_ORGANICS, &[
                Variant::from(self.run_state.profile.organics.balance as i64),
            ]);
            hud.call(methods::UPDATE_LASER, &[
                Variant::from(GString::from(self.run_state.laser_level.display_name())),
                Variant::from(color),
            ]);
            hud.call(methods::UPDATE_LEVEL, &[
                Variant::from(self.run_state.current_level as i32),
            ]);
            hud.call(methods::SET_UNLOCK_FLAGS, &[
                Variant::from(self.run_state.profile.unlocks.contains(Unlock::Radar)),
                Variant::from(self.run_state.profile.unlocks.contains(Unlock::FogMap)),
                Variant::from(self.run_state.profile.unlocks.contains(Unlock::ThreatTracker)),
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

    fn regenerate_level(&mut self) {
        // ONE spec construction per level entry: every attribute (pitch,
        // paradigm, roster, boss staging incl. the red container and the
        // rolled hull) resolves here, against the profile, and this same
        // value drives the build and every later mediator decision.
        let spec = LevelSpec::for_level(
            self.run_state.run_seed,
            self.run_state.current_level,
            &self.run_state.profile.unlocks,
        );
        self.boss_fight = spec.boss.as_ref().map(|_| BossFight::new());
        self.level_spec = Some(spec.clone());
        let Some(parent) = self.base().get_parent() else { return };
        if let Some(level_mgr) = parent.try_get_node_as::<LevelManager>(nodes::LEVEL_MANAGER) {
            let mut level_mgr = level_mgr;
            // Clear old children
            for mut child in level_mgr.get_children().iter_shared() {
                child.queue_free();
            }
            // Typed crossing — no stringly staging pre-calls, no property
            // pushes syncing duplicate state.
            let seed = self.run_state.level_seed();
            level_mgr
                .bind_mut()
                .build_from_spec(spec, seed.as_i64(), false);
        }
        // `generate_level` builds synchronously (add_child is synchronous), so
        // the whole pre-instantiated roster — enemies, dormant minions, currency
        // caches, portal — now exists under LevelManager. Wire every
        // emitter to the mediator once, here, replacing the deleted per-frame
        // scan. Idempotent, so a later rebuild rewires nothing already connected.
        self.wire_level_signals();
        self.queue_music_push();
    }
}
