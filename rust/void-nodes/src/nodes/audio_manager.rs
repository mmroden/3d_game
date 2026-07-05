use godot::prelude::*;
use godot::classes::{
    AudioStreamPlayer, AudioStreamPlayer3D,
    Node, INode, Engine, ResourceLoader,
};
use rand::seq::IndexedRandom;

use super::constants::{signals, methods, nodes};
use super::live_handle::{LiveOpt, LiveRef};
use void_logic::audio_catalog::{
    combat_pool, menu_track, MusicBed, SfxEvent,
    CROSSFADE_SECS, GAMEPLAY_MUSIC_VOL, MENU_MUSIC_VOL, TRANSITION_MUSIC_VOL,
    DEATH_MUSIC_VOL, MAX_SFX_POLYPHONY, COLLISION_SFX_COOLDOWN,
};
use void_logic::game_phase::GamePhase;

/// Which of the two crossfade players is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivePlayer { A, B }

impl ActivePlayer {
    fn flip(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }
}

/// Manages all audio playback: music crossfading between game phases
/// and positional/non-positional SFX triggered by gameplay events.
#[derive(GodotClass)]
#[class(base=Node)]
pub struct AudioManager {
    base: Base<Node>,
    music_player_a: Option<LiveRef<AudioStreamPlayer>>,
    music_player_b: Option<LiveRef<AudioStreamPlayer>>,
    active_player: ActivePlayer,
    /// The bed GameManager last pushed (ONE derivation, pushed at every
    /// input change — this node holds no music lifecycle of its own; see
    /// audio_catalog::music_bed) and the track it named.
    current_bed: MusicBed,
    current_track: String,
    /// What is actually audible right now — diverges from `current_track`
    /// when a bed's continuation swaps the stream (Boss → combat stinger).
    audible_track: String,
    crossfade_timer: f32,
    crossfade_target_vol: f32,
    is_crossfading: bool,
    current_phase: GamePhase,
    active_sfx_count: u32,
    collision_cooldown: f32,
}

#[godot_api]
impl INode for AudioManager {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            music_player_a: None,
            music_player_b: None,
            active_player: ActivePlayer::A,
            current_bed: MusicBed::Menu,
            current_track: menu_track().to_string(),
            audible_track: String::new(),
            crossfade_timer: 0.0,
            crossfade_target_vol: MENU_MUSIC_VOL,
            is_crossfading: false,
            current_phase: GamePhase::MainMenu,
            active_sfx_count: 0,
            collision_cooldown: 0.0,
        }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }

        let mut player_a = AudioStreamPlayer::new_alloc();
        player_a.set_bus("Music");
        player_a.set_volume_db(linear_to_db(MENU_MUSIC_VOL));
        self.base_mut().add_child(&player_a);

        let mut player_b = AudioStreamPlayer::new_alloc();
        player_b.set_bus("Music");
        player_b.set_volume_db(linear_to_db(0.0));
        self.base_mut().add_child(&player_b);

        player_a.connect("finished", &self.base().callable(methods::ON_MUSIC_FINISHED));
        player_b.connect("finished", &self.base().callable(methods::ON_MUSIC_FINISHED));

        self.music_player_a = Some(LiveRef::new(&player_a));
        self.music_player_b = Some(LiveRef::new(&player_b));

        // Connect to GameManager's phase_changed signal
        if let Some(parent) = self.base().get_parent() {
            if let Some(mut game_mgr) = parent.try_get_node_as::<Node>(nodes::GAME_MANAGER) {
                let callable = self.base().callable(methods::ON_PHASE_CHANGED_AUDIO);
                game_mgr.connect(signals::PHASE_CHANGED, &callable);
            }
        }

        self.start_music(menu_track(), MENU_MUSIC_VOL);
    }

    fn process(&mut self, delta: f64) {
        self.collision_cooldown = (self.collision_cooldown - delta as f32).max(0.0);

        if !self.is_crossfading {
            return;
        }

        self.crossfade_timer += delta as f32;
        let t = (self.crossfade_timer / CROSSFADE_SECS).clamp(0.0, 1.0);
        let target_vol = self.crossfade_target_vol;

        let (active, inactive) = match self.active_player {
            ActivePlayer::A => (&self.music_player_a, &self.music_player_b),
            ActivePlayer::B => (&self.music_player_b, &self.music_player_a),
        };
        active.with(|a| a.set_volume_db(linear_to_db(t * target_vol)));
        inactive.with(|i| {
            let cur = db_to_linear(i.get_volume_db());
            i.set_volume_db(linear_to_db(cur * (1.0 - t)));
            if t >= 1.0 {
                i.stop();
            }
        });

        if t >= 1.0 {
            self.is_crossfading = false;
        }
    }
}

#[godot_api]
impl AudioManager {
    /// Called when GameManager emits phase_changed(phase_name: GString).
    #[func]
    fn on_phase_changed_audio(&mut self, phase_name: GString) {
        let Some(phase) = GamePhase::from_name(&phase_name.to_string()) else {
            return;
        };

        self.current_phase = phase;
        // Volume ducking only — WHICH bed plays is GameManager's single
        // derivation (set_music_bed), never inferred from phases here.
        let vol = self.volume_for_phase();
        if self.is_crossfading {
            self.crossfade_target_vol = vol;
        } else {
            self.set_active_volume(vol);
        }
    }

    /// GameManager pushes the derived bed at every input change (phase,
    /// room, kill, fight beat, rebuild). An empty `track` lets this node
    /// pick — used for Combat, where the stinger is random (cosmetic
    /// entropy, the accepted exception).
    #[func]
    pub fn set_music_bed(&mut self, bed_id: i32, track: GString) {
        let bed = match bed_id {
            0 => MusicBed::Menu,
            1 => MusicBed::Level,
            2 => MusicBed::Combat,
            3 => MusicBed::Boss,
            _ => return,
        };
        let mut track = track.to_string();
        if bed == self.current_bed
            && (bed == MusicBed::Combat || track == self.current_track)
        {
            return; // already on this bed — a re-push is a no-op
        }
        if bed == MusicBed::Combat && track.is_empty() {
            track = Self::random_combat_track();
        }
        if track.is_empty() {
            return;
        }
        self.current_bed = bed;
        self.current_track = track.clone();
        let vol = self.volume_for_phase();
        self.crossfade_to(&track, vol);
    }

    /// Track-end continuation, per bed (owner's design 2026-07-05): menu
    /// and level backgrounds loop; a finished combat stinger draws another;
    /// a boss fight outlasting its track continues on combat stingers —
    /// the boss track never reloops.
    #[func]
    fn on_music_finished(&mut self) {
        let vol = self.volume_for_phase();
        match self.current_bed {
            MusicBed::Menu | MusicBed::Level => {
                let track = self.current_track.clone();
                self.start_music(&track, vol);
            }
            MusicBed::Combat | MusicBed::Boss => {
                // current_bed/current_track stay as pushed, so GameManager
                // re-pushes remain no-ops; only the audible track changes.
                let track = Self::random_combat_track();
                self.start_music(&track, vol);
            }
        }
    }

    fn random_combat_track() -> String {
        let pool = combat_pool();
        pool.choose(&mut rand::rng()).cloned().unwrap_or_default()
    }

    /// The bed as last pushed — the GDScript-facing observability door.
    #[func]
    pub fn music_bed_id(&self) -> i32 {
        self.current_bed.id()
    }

    /// The track actually audible (a Boss bed may be playing a combat
    /// stinger continuation — this reports the stinger).
    #[func]
    pub fn music_track(&self) -> GString {
        GString::from(self.audible_track.as_str())
    }

    /// Called when any ephemeral SFX node finishes playback.
    /// Decrements the polyphony counter so new SFX can spawn.
    #[func]
    fn on_sfx_finished(&mut self) {
        self.active_sfx_count = self.active_sfx_count.saturating_sub(1);
    }

    /// Play a typed SFX event non-positionally. Called from Godot via callable.
    #[func]
    fn play_sfx_event(&mut self, event_id: i32) {
        if let Some(event) = SfxEvent::from_id(event_id) {
            self.play_event(event);
        }
    }

    /// Play a typed SFX event at a 3D position. Called from Godot via callable.
    #[func]
    fn play_sfx_event_at(&mut self, event_id: i32, position: Vector3) {
        if let Some(event) = SfxEvent::from_id(event_id) {
            self.play_event_at(event, position);
        }
    }
}

// ── Public typed API (called directly from other Rust nodes) ─────────

impl AudioManager {
    /// Play a typed SFX event non-positionally (player feedback).
    pub fn play_event(&mut self, event: SfxEvent) {
        if self.active_sfx_count >= MAX_SFX_POLYPHONY {
            return;
        }
        if Self::is_collision_event(event) && !self.try_collision_cooldown() {
            return;
        }
        let path = Self::pick_variant(event);
        self.spawn_sfx_player(path);
    }

    /// Play a typed SFX event at a 3D position.
    pub fn play_event_at(&mut self, event: SfxEvent, position: Vector3) {
        if self.active_sfx_count >= MAX_SFX_POLYPHONY {
            return;
        }
        if Self::is_collision_event(event) && !self.try_collision_cooldown() {
            return;
        }
        let path = Self::pick_variant(event);
        self.spawn_sfx_3d_player(path, position);
    }
}

// ── Private helpers ──────────────────────────────────────────────────

impl AudioManager {
    fn is_collision_event(event: SfxEvent) -> bool {
        matches!(event, SfxEvent::CollisionShielded | SfxEvent::CollisionBare)
    }

    /// Returns true if the collision cooldown has expired, and resets it.
    fn try_collision_cooldown(&mut self) -> bool {
        if self.collision_cooldown > 0.0 {
            return false;
        }
        self.collision_cooldown = COLLISION_SFX_COOLDOWN;
        true
    }

    fn pick_variant(event: SfxEvent) -> &'static str {
        let variants = event.variants();
        variants.choose(&mut rand::rng()).copied().unwrap_or(variants[0])
    }

    fn volume_for_phase(&self) -> f32 {
        match self.current_phase {
            GamePhase::MainMenu => MENU_MUSIC_VOL,
            GamePhase::Playing => GAMEPLAY_MUSIC_VOL,
            GamePhase::Death => DEATH_MUSIC_VOL,
            GamePhase::KillSummary | GamePhase::Shop | GamePhase::ShipSelect
            | GamePhase::Bestiary | GamePhase::Paused | GamePhase::LevelComplete => {
                TRANSITION_MUSIC_VOL
            }
        }
    }

    fn start_music(&mut self, path: &str, volume: f32) {
        self.audible_track = path.to_string();
        if let Some(stream) = Self::load_audio_stream(path) {
            self.active_music_player().with(|p| {
                p.set_stream(&stream);
                p.set_volume_db(linear_to_db(volume));
                p.play();
            });
        }
    }

    fn crossfade_to(&mut self, path: &str, target_vol: f32) {
        self.audible_track = path.to_string();
        self.active_player = self.active_player.flip();

        if let Some(stream) = Self::load_audio_stream(path) {
            self.active_music_player().with(|p| {
                p.set_stream(&stream);
                p.set_volume_db(linear_to_db(0.0));
                p.play();
            });
        }

        self.crossfade_target_vol = target_vol;
        self.crossfade_timer = 0.0;
        self.is_crossfading = true;
    }

    fn set_active_volume(&mut self, volume: f32) {
        self.active_music_player()
            .with(|p| p.set_volume_db(linear_to_db(volume)));
    }

    fn active_music_player(&self) -> &Option<LiveRef<AudioStreamPlayer>> {
        match self.active_player {
            ActivePlayer::A => &self.music_player_a,
            ActivePlayer::B => &self.music_player_b,
        }
    }

    fn load_audio_stream(path: &str) -> Option<Gd<godot::classes::AudioStream>> {
        let mut loader = ResourceLoader::singleton();
        if loader.exists(path) {
            loader.load(path).map(|r| r.cast::<godot::classes::AudioStream>())
        } else {
            godot_warn!("AudioManager: audio file not found: {path}");
            None
        }
    }

    fn spawn_sfx_player(&mut self, path: &str) {
        let Some(stream) = Self::load_audio_stream(path) else { return };

        let mut player = AudioStreamPlayer::new_alloc();
        player.set_bus("SFX");
        player.set_stream(&stream);

        self.base_mut().add_child(&player);
        self.active_sfx_count += 1;
        // On finished: decrement polyphony counter, then free the node
        player.connect("finished", &self.base().callable(methods::ON_SFX_FINISHED));
        let free_callable = player.callable("queue_free");
        player.connect("finished", &free_callable);
        player.play();
    }

    fn spawn_sfx_3d_player(&mut self, path: &str, position: Vector3) {
        let Some(stream) = Self::load_audio_stream(path) else { return };

        let mut player = AudioStreamPlayer3D::new_alloc();
        player.set_bus("SFX");
        player.set_stream(&stream);
        player.set_position(position);

        // Add to scene root so position is in world space
        if let Some(mut root) = super::godot_util::scene_root(self.base().get_tree()) {
            root.add_child(&player);
        } else {
            self.base_mut().add_child(&player);
        }

        self.active_sfx_count += 1;
        // On finished: decrement polyphony counter, then free the node
        player.connect("finished", &self.base().callable(methods::ON_SFX_FINISHED));
        let free_callable = player.callable("queue_free");
        player.connect("finished", &free_callable);
        player.play();
    }
}

// ── Audio math ───────────────────────────────────────────────────────

fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0001 {
        -80.0
    } else {
        20.0 * linear.log10()
    }
}

fn db_to_linear(db: f32) -> f32 {
    if db <= -80.0 {
        0.0
    } else {
        10.0_f32.powf(db / 20.0)
    }
}
