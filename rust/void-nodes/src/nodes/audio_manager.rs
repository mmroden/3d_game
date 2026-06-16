use godot::prelude::*;
use godot::classes::{
    AudioStreamPlayer, AudioStreamPlayer3D,
    Node, INode, Engine, ResourceLoader,
};
use rand::seq::{IndexedRandom, SliceRandom};

use super::constants::{signals, methods, nodes};
use super::live_handle::{LiveOpt, LiveRef};
use void_logic::audio_catalog::{
    MusicContext, SfxEvent,
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
    crossfade_timer: f32,
    crossfade_target_vol: f32,
    is_crossfading: bool,
    gameplay_tracks: Vec<&'static str>,
    gameplay_track_index: usize,
    current_phase: GamePhase,
    active_sfx_count: u32,
    collision_cooldown: f32,
}

#[godot_api]
impl INode for AudioManager {
    fn init(base: Base<Node>) -> Self {
        let mut tracks: Vec<&str> = MusicContext::Gameplay.track_pool().to_vec();
        tracks.shuffle(&mut rand::rng());

        Self {
            base,
            music_player_a: None,
            music_player_b: None,
            active_player: ActivePlayer::A,
            crossfade_timer: 0.0,
            crossfade_target_vol: MENU_MUSIC_VOL,
            is_crossfading: false,
            gameplay_tracks: tracks,
            gameplay_track_index: 0,
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

        self.start_music(MusicContext::Menu.track_path(), MENU_MUSIC_VOL);
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

        let prev = self.current_phase;
        self.current_phase = phase;

        match phase {
            GamePhase::MainMenu => {
                self.crossfade_to(MusicContext::Menu.track_path(), MENU_MUSIC_VOL);
            }
            GamePhase::Playing => {
                if prev == GamePhase::MainMenu
                    || prev == GamePhase::Shop
                    || prev == GamePhase::ShipSelect
                    || prev == GamePhase::Bestiary
                {
                    let track = self.next_gameplay_track();
                    self.crossfade_to(track, GAMEPLAY_MUSIC_VOL);
                } else {
                    self.set_active_volume(GAMEPLAY_MUSIC_VOL);
                }
            }
            GamePhase::Death => {
                self.set_active_volume(DEATH_MUSIC_VOL);
            }
            // Transition/menu screens all reduce volume.
            GamePhase::KillSummary | GamePhase::Shop | GamePhase::ShipSelect
            | GamePhase::Bestiary | GamePhase::Paused | GamePhase::LevelComplete => {
                self.set_active_volume(TRANSITION_MUSIC_VOL);
            }
        }
    }

    /// Called when the active music player finishes a track.
    #[func]
    fn on_music_finished(&mut self) {
        if matches!(
            self.current_phase,
            GamePhase::Playing
                | GamePhase::LevelComplete
                | GamePhase::KillSummary
                | GamePhase::Shop
        ) {
            let track = self.next_gameplay_track();
            let vol = self.volume_for_phase();
            self.start_music(track, vol);
        }
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
        matches!(event, SfxEvent::ImpactMetal | SfxEvent::ImpactShield | SfxEvent::ImpactHeavy)
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

    fn next_gameplay_track(&mut self) -> &'static str {
        let track = self.gameplay_tracks[self.gameplay_track_index];
        self.gameplay_track_index = (self.gameplay_track_index + 1) % self.gameplay_tracks.len();
        if self.gameplay_track_index == 0 {
            self.gameplay_tracks.shuffle(&mut rand::rng());
        }
        track
    }

    fn start_music(&mut self, path: &str, volume: f32) {
        if let Some(stream) = Self::load_audio_stream(path) {
            self.active_music_player().with(|p| {
                p.set_stream(&stream);
                p.set_volume_db(linear_to_db(volume));
                p.play();
            });
        }
    }

    fn crossfade_to(&mut self, path: &str, target_vol: f32) {
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
