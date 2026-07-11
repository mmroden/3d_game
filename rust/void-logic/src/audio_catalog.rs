//! Type-safe audio asset catalog.
//!
//! All audio selection goes through enums — callers never touch path strings.
//! `SfxEvent` selects a sound effect (with random variant picking);
//! `MusicBed` + `music_bed` derive which music should be playing.

// ── Path macros (module-private) ─────────────────────────────────────

macro_rules! music {
    ($name:expr) => {
        concat!("res://addons/audio/music/", $name)
    };
}
macro_rules! sfx {
    ($name:expr) => {
        concat!("res://addons/audio/sfx/", $name)
    };
}

// ── Music ────────────────────────────────────────────────────────────
//
// Four beds (owner's design 2026-07-05): the menu ambient, a loopable
// per-level background, random combat stingers while the current room
// holds live enemies, and the boss track from arena entry (never
// relooped — a fight outlasting its track queues combat stingers).
// `music_bed` is THE derivation: one pure function of (phase, fight
// state, enemies-present); the shell evaluates it at every input change
// and pushes the result, so no shell lifecycle bool can drift.

/// Which bed should be playing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicBed {
    Menu,
    /// The level's own loopable background.
    Level,
    /// Random combat stinger — live enemies share the player's room.
    Combat,
    /// The staged fight's track, from arena entry until the loot closes it.
    Boss,
}

/// Fight-music loudness over the exploration baseline (owner's call,
/// playtest 2026-07-05: "combat music should be played louder — maybe 20%").
pub const FIGHT_MUSIC_GAIN: f32 = 1.2;

impl MusicBed {
    /// Loudness multiplier layered on the phase volume: fight beds (combat
    /// stingers AND the boss track — whose continuations ARE combat
    /// stingers) ride above the exploration baseline.
    pub fn gain(self) -> f32 {
        match self {
            Self::Combat | Self::Boss => FIGHT_MUSIC_GAIN,
            Self::Menu | Self::Level => 1.0,
        }
    }

    /// GDScript crossing (append-only law).
    pub fn id(self) -> i32 {
        match self {
            Self::Menu => 0,
            Self::Level => 1,
            Self::Combat => 2,
            Self::Boss => 3,
        }
    }
}

/// THE bed derivation. Boss outranks combat (the arena has enemies in it
/// by definition); combat outranks the level bed; everything outside a
/// run is the menu. Non-Playing in-run phases (shop, summary, briefing,
/// pause, death) keep the level bed — the existing per-phase volume
/// ducking does the rest.
pub fn music_bed(
    phase: crate::game_phase::GamePhase,
    fight: Option<crate::boss_fight::BossFightState>,
    enemies_in_room: bool,
) -> MusicBed {
    use crate::boss_fight::BossFightState;
    use crate::game_phase::GamePhase;

    if phase == GamePhase::MainMenu {
        return MusicBed::Menu;
    }
    // The room seal owns the bed while the fight is on — a staged boss and a
    // miniboss ride the SAME fight FSM (one seal per level), so one input
    // covers both. Note the asymmetry past the kill: a miniboss resolves AT
    // the kill (defeat(0) lands on RewardCollected — bed drops immediately);
    // a staged boss holds Defeated, and the bed, until the loot closes it.
    if matches!(
        fight,
        Some(BossFightState::Engaged) | Some(BossFightState::Defeated)
    ) {
        return MusicBed::Boss;
    }
    if enemies_in_room {
        return MusicBed::Combat;
    }
    MusicBed::Level
}

/// Written level backgrounds so far (levels past the table reuse the
/// last track until more arrive — owner supplies in batches).
pub const LEVEL_BACKGROUND_COUNT: u32 = 30;
const COMBAT_TRACKS: [u32; 9] = [11, 12, 13, 14, 15, 16, 17, 18, 19];

pub fn menu_track() -> &'static str {
    music!("ambient/frozen_whispers.wav")
}

/// The level's loopable background: `levels/level_NN.mp3`.
pub fn level_background(level: u32) -> String {
    let n = level.clamp(1, LEVEL_BACKGROUND_COUNT);
    format!("res://addons/audio/music/levels/level_{n:02}.mp3")
}

/// The combat stinger pool — the shell picks randomly per fight
/// (unseeded: cosmetic, the accepted entropy exception).
pub fn combat_pool() -> Vec<String> {
    COMBAT_TRACKS
        .iter()
        .map(|n| format!("res://addons/audio/music/combat/combat_{n}.mp3"))
        .collect()
}

/// The staged fight's track by index — declared per boss slot in the
/// roster grammar (owner: "tracks 1 and 2, up to 6" — all six installed).
pub fn boss_track(index: u32) -> String {
    let n = index.clamp(1, 6);
    format!("res://addons/audio/music/boss/boss_{n}.mp3")
}

// ── Sound effects ────────────────────────────────────────────────────

/// A typed sound effect event. Each variant maps to one or more .wav files.
/// AudioManager matches on this enum — callers never touch path strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfxEvent {
    /// Player dual-laser fire.
    LaserFire,
    /// Enemy blaster fire.
    EnemyFire,
    /// Ship rams static geometry while the shield is up — a cushioned clang.
    CollisionShielded,
    /// Ship rams static geometry with the shield down — bare metal on metal.
    CollisionBare,
    /// An enemy hit lands while the shield holds — a light energy deflection.
    HitShielded,
    /// A full explosion: an enemy dying, or an enemy shot breaching the bare hull.
    Explosion,
    /// A swarmer drone latches onto the ship (floor 2+).
    SwarmerLatch,
    /// A subsidiary drone arms up from a dying drone's corpse (e.g. EyeDrone
    /// releasing its SpawnDrone, floor 3+).
    DroneSpawn,
    /// Player enters portal.
    PortalEnter,
    /// Currency cache collected.
    LootPickup,
    /// Health below critical threshold.
    LowHealthAlert,
    /// Game start / new level boot-up.
    WeaponBoot,
    /// A Shield Surge charge spent — the emergency recharge landing.
    ShieldBurst,
    /// The Valkyrie dumps its stored bolts (playtest 2026-07-06 redesign).
    ValkyrieFire,
    /// A Valkyrie charge bar completes — played pitched-up per bar, so
    /// the fill reads as a rising scale.
    ValkyrieBarReady,
}

/// All `SfxEvent` variants, for exhaustive iteration in tests.
const ALL_SFX_EVENTS: &[SfxEvent] = &[
    SfxEvent::LaserFire,
    SfxEvent::EnemyFire,
    SfxEvent::CollisionShielded,
    SfxEvent::CollisionBare,
    SfxEvent::HitShielded,
    SfxEvent::Explosion,
    SfxEvent::SwarmerLatch,
    SfxEvent::DroneSpawn,
    SfxEvent::PortalEnter,
    SfxEvent::LootPickup,
    SfxEvent::LowHealthAlert,
    SfxEvent::WeaponBoot,
    SfxEvent::ShieldBurst,
    SfxEvent::ValkyrieFire,
    SfxEvent::ValkyrieBarReady,
];

impl SfxEvent {
    /// All .wav variants for this event. AudioManager picks one at random.
    pub fn variants(self) -> &'static [&'static str] {
        match self {
            Self::LaserFire => &[
                sfx!("Gunshots/Laser/laser_shoot_01.wav"),
                sfx!("Gunshots/Laser/laser_shoot_02.wav"),
                sfx!("Gunshots/Laser/laser_shoot_03.wav"),
            ],
            Self::EnemyFire => &[
                sfx!("Gunshots/Blaster/blaster_shoot_01.wav"),
                sfx!("Gunshots/Blaster/blaster_shoot_02.wav"),
                sfx!("Gunshots/Blaster/blaster_shoot_03.wav"),
            ],
            Self::CollisionShielded => &[
                sfx!("Impacts/impact_kinetic_heavy_shield_01.wav"),
                sfx!("Impacts/impact_kinetic_heavy_shield_02.wav"),
                sfx!("Impacts/impact_kinetic_heavy_shield_03.wav"),
            ],
            Self::CollisionBare => &[
                sfx!("Impacts/impact_metal_01.wav"),
                sfx!("Impacts/impact_metal_02.wav"),
                sfx!("Impacts/impact_metal_03.wav"),
            ],
            Self::HitShielded => &[
                sfx!("Impacts/impact_kinetic_light_shield_01.wav"),
                sfx!("Impacts/impact_kinetic_light_shield_02.wav"),
                sfx!("Impacts/impact_kinetic_light_shield_03.wav"),
            ],
            Self::SwarmerLatch => &[
                sfx!("AttachmentAttacks/attach_attack_01.wav"),
                sfx!("AttachmentAttacks/attach_attack_02.wav"),
                sfx!("AttachmentAttacks/attach_attack_03.wav"),
            ],
            Self::DroneSpawn => &[
                sfx!("Spawns/spawn_01.wav"),
                sfx!("Spawns/spawn_02.wav"),
                sfx!("Spawns/spawn_03.wav"),
                sfx!("Spawns/spawn_04.wav"),
            ],
            Self::Explosion => &[
                sfx!("Impacts/explosion_01.wav"),
                sfx!("Impacts/explosion_02.wav"),
                sfx!("Impacts/explosion_03.wav"),
                sfx!("Impacts/explosion_04.wav"),
                sfx!("Impacts/explosion_05.wav"),
                sfx!("Impacts/explosion_06.wav"),
            ],
            Self::PortalEnter => &[
                sfx!("WeaponSystems/system_cooling_vent.wav"),
            ],
            Self::LootPickup => &[
                sfx!("WeaponHandle/weapon_handle_pickup_01.wav"),
            ],
            Self::LowHealthAlert => &[
                sfx!("WeaponSystems/system_weapon_alert_01.wav"),
                sfx!("WeaponSystems/system_weapon_alert_02.wav"),
            ],
            Self::WeaponBoot => &[
                sfx!("WeaponSystems/system_weapon_boot_01.wav"),
                sfx!("WeaponSystems/system_weapon_boot_02.wav"),
                sfx!("WeaponSystems/system_weapon_boot_03.wav"),
            ],
            Self::ShieldBurst => &[
                sfx!("WeaponSystems/system_weapon_energy_charge_01.wav"),
                sfx!("WeaponSystems/system_weapon_energy_charge_02.wav"),
            ],
            Self::ValkyrieFire => &[
                sfx!("Valkyrie/LASRGun_Valkyrie_01_SFRMS_SCIWPNS.wav"),
                sfx!("Valkyrie/LASRGun_Valkyrie_02_SFRMS_SCIWPNS.wav"),
                sfx!("Valkyrie/LASRGun_Valkyrie_03_SFRMS_SCIWPNS.wav"),
                sfx!("Valkyrie/LASRGun_Valkyrie_04_SFRMS_SCIWPNS.wav"),
                sfx!("Valkyrie/LASRGun_Valkyrie_05_SFRMS_SCIWPNS.wav"),
                sfx!("Valkyrie/LASRGun_Valkyrie_06_SFRMS_SCIWPNS.wav"),
            ],
            Self::ValkyrieBarReady => &[
                sfx!("WeaponSystems/system_weapon_energy_aim_lock_01.wav"),
                sfx!("WeaponSystems/system_weapon_energy_aim_lock_02.wav"),
            ],
        }
    }

    /// Convert from integer ID (for Godot signal interop).
    pub fn from_id(id: i32) -> Option<Self> {
        ALL_SFX_EVENTS.get(id as usize).copied()
    }

    /// Convert to integer ID (for Godot signal interop).
    pub fn to_id(self) -> i32 {
        ALL_SFX_EVENTS.iter().position(|&e| e == self).unwrap_or(0) as i32
    }
}

// ── Timing constants ─────────────────────────────────────────────────

/// Duration of music crossfade in seconds.
pub const CROSSFADE_SECS: f32 = 2.0;
/// Volume for gameplay music (linear, 0.0–1.0).
pub const GAMEPLAY_MUSIC_VOL: f32 = 0.7;
/// Volume for menu music (linear, 0.0–1.0).
pub const MENU_MUSIC_VOL: f32 = 0.8;
/// Reduced volume during transitions (shop, kill summary).
pub const TRANSITION_MUSIC_VOL: f32 = 0.4;
/// Reduced volume on death screen (near-silent, not zero).
pub const DEATH_MUSIC_VOL: f32 = 0.1;
/// Maximum simultaneous SFX nodes.
pub const MAX_SFX_POLYPHONY: u32 = 8;
/// Minimum interval between collision/impact SFX (seconds).
pub const COLLISION_SFX_COOLDOWN: f32 = 0.3;
/// Minimum speed (m/s) for a physical collision to trigger SFX.
pub const COLLISION_SFX_MIN_SPEED: f32 = 3.0;

// ── Validation helper ────────────────────────────────────────────────

/// Every audio path in the catalog, for disk-existence tests.
pub fn all_audio_paths() -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();

    // Music — every bed the conductor can select.
    paths.push(menu_track().to_string());
    for level in 1..=LEVEL_BACKGROUND_COUNT {
        paths.push(level_background(level));
    }
    paths.extend(combat_pool());
    paths.push(boss_track(1));
    paths.push(boss_track(2));

    // SFX
    for event in ALL_SFX_EVENTS {
        paths.extend(event.variants().iter().map(|s| s.to_string()));
    }

    paths.sort();
    paths.dedup();
    paths
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fight_beds_play_louder_than_the_rest() {
        // Owner's call (playtest 2026-07-05): combat music noticeably louder
        // than exploration — about 20%. Boss rides the same fight gain so its
        // combat-stinger continuations hold one loudness.
        assert_eq!(MusicBed::Combat.gain(), FIGHT_MUSIC_GAIN);
        assert_eq!(MusicBed::Boss.gain(), FIGHT_MUSIC_GAIN);
        assert_eq!(MusicBed::Level.gain(), 1.0, "exploration is the baseline");
        assert_eq!(MusicBed::Menu.gain(), 1.0);
        assert!((FIGHT_MUSIC_GAIN - 1.2).abs() < f32::EPSILON);
    }

    #[test]
    fn the_bed_derivation_ranks_the_seal_over_combat_over_level() {
        use crate::boss_fight::BossFightState as B;
        use crate::game_phase::GamePhase as P;

        assert_eq!(music_bed(P::MainMenu, None, false), MusicBed::Menu);
        assert_eq!(music_bed(P::Playing, None, false), MusicBed::Level);
        assert_eq!(music_bed(P::Playing, None, true), MusicBed::Combat,
            "live enemies in the room bring the stinger in");
        // One seal FSM serves the staged boss and the miniboss alike — an
        // engaged fight of EITHER kind owns the bed.
        assert_eq!(music_bed(P::Playing, Some(B::Engaged), true), MusicBed::Boss,
            "the sealed room outranks the stinger");
        assert_eq!(music_bed(P::Playing, Some(B::Defeated), false), MusicBed::Boss,
            "a staged boss holds the bed until the loot closes it");
        assert_eq!(music_bed(P::Playing, Some(B::Dormant), false), MusicBed::Level,
            "a dormant fight is no fight");
        assert_eq!(music_bed(P::Playing, Some(B::RewardCollected), false),
            MusicBed::Level,
            "resolution drops the bed — a miniboss lands here AT the kill");
        // In-run menus keep the level bed (phase volume ducking handles feel).
        assert_eq!(music_bed(P::Shop, None, false), MusicBed::Level);
        assert_eq!(music_bed(P::Paused, Some(B::Engaged), true), MusicBed::Boss,
            "pausing mid-fight doesn't end the fight");
        assert_eq!(music_bed(P::Death, None, false), MusicBed::Level);
    }

    #[test]
    fn level_backgrounds_map_by_number_and_reuse_the_last_past_the_table() {
        assert_eq!(level_background(1), "res://addons/audio/music/levels/level_01.mp3");
        assert_eq!(level_background(7), "res://addons/audio/music/levels/level_07.mp3");
        assert_eq!(level_background(30), "res://addons/audio/music/levels/level_30.mp3");
        assert_eq!(level_background(31), "res://addons/audio/music/levels/level_30.mp3",
            "levels past the written table reuse the last background");
    }

    #[test]
    fn boss_tracks_split_mid_and_final() {
        assert_eq!(boss_track(1), "res://addons/audio/music/boss/boss_1.mp3");
        assert_eq!(boss_track(2), "res://addons/audio/music/boss/boss_2.mp3");
        assert_eq!(boss_track(9), "res://addons/audio/music/boss/boss_6.mp3",
            "indices clamp to the installed set");
    }

    #[test]
    fn the_combat_pool_is_the_nine_stingers() {
        let pool = combat_pool();
        assert_eq!(pool.len(), 9);
        assert!(pool[0].ends_with("combat_11.mp3"));
        assert!(pool[8].ends_with("combat_19.mp3"));
    }

    #[test]
    fn the_shield_burst_speaks_with_the_energy_charge_voice() {
        // Playtest 2026-07-06: the surge needs a sound of its own — the
        // energy-charge pair, distinct from every combat impact.
        let variants = SfxEvent::ShieldBurst.variants();
        assert_eq!(variants.len(), 2, "both energy-charge takes");
        for path in variants {
            assert!(
                path.contains("system_weapon_energy_charge"),
                "the surge voice is the energy charge, got {path}"
            );
        }
    }

    #[test]
    fn the_valkyrie_speaks_with_its_own_voices() {
        // The dump: the six-take Valkyrie gun pack. The bar-ready tick:
        // the aim-lock pair, pitched up per bar by the shell.
        let fire = SfxEvent::ValkyrieFire.variants();
        assert_eq!(fire.len(), 6, "all six gun takes");
        for path in fire {
            assert!(path.contains("Valkyrie/LASRGun_Valkyrie"),
                "the dump speaks with the Valkyrie gun pack, got {path}");
        }
        let ready = SfxEvent::ValkyrieBarReady.variants();
        assert_eq!(ready.len(), 2, "both aim-lock takes");
        for path in ready {
            assert!(path.contains("system_weapon_energy_aim_lock"),
                "the bar tick is the aim-lock blip, got {path}");
        }
    }

    #[test]
    fn sfx_event_has_at_least_one_variant() {
        for event in ALL_SFX_EVENTS {
            assert!(
                !event.variants().is_empty(),
                "{event:?} has no variants"
            );
        }
    }

    #[test]
    fn all_sfx_variants_are_wav() {
        for event in ALL_SFX_EVENTS {
            for path in event.variants() {
                assert!(
                    path.ends_with(".wav"),
                    "{event:?} variant should end with .wav: {path}"
                );
            }
        }
    }

    #[test]
    fn all_sfx_variants_are_valid_res_paths() {
        for event in ALL_SFX_EVENTS {
            for path in event.variants() {
                assert!(
                    path.starts_with("res://"),
                    "{event:?} variant should start with res://: {path}"
                );
            }
        }
    }

    #[test]
    fn all_music_paths_are_valid_res() {
        let mut music: Vec<String> = vec![menu_track().to_string()];
        music.push(level_background(1));
        music.extend(combat_pool());
        music.push(boss_track(2));
        for path in music {
            assert!(path.starts_with("res://"),
                "music path should start with res://: {path}");
            assert!(path.ends_with(".mp3") || path.ends_with(".wav"),
                "music path should be mp3/wav: {path}");
        }
    }

    #[test]
    fn crossfade_in_range() {
        assert!(
            (0.0..=5.0).contains(&CROSSFADE_SECS),
            "crossfade duration out of range: {CROSSFADE_SECS}"
        );
    }

    #[test]
    fn sfx_event_round_trips_through_id() {
        for event in ALL_SFX_EVENTS {
            let id = event.to_id();
            let back = SfxEvent::from_id(id);
            assert_eq!(
                back,
                Some(*event),
                "{event:?} did not round-trip through id {id}"
            );
        }
    }

    #[test]
    fn sfx_event_from_invalid_id_returns_none() {
        assert_eq!(SfxEvent::from_id(-1), None);
        assert_eq!(SfxEvent::from_id(999), None);
    }

    #[test]
    fn all_audio_paths_exist_on_disk() {
        let godot_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("void-logic/ should have a parent dir")
            .parent()
            .expect("rust/ should have a parent dir")
            .join("godot");

        let audio_dir = godot_dir.join("addons/audio");
        if !audio_dir.exists() {
            // Skip if audio assets haven't been installed yet
            return;
        }

        let mut missing = Vec::new();
        for res_path in all_audio_paths() {
            let rel = res_path
                .strip_prefix("res://")
                .unwrap_or_else(|| panic!("path should start with res://: {res_path}"));
            let full = godot_dir.join(rel);
            if !full.exists() {
                missing.push(res_path);
            }
        }

        assert!(
            missing.is_empty(),
            "Audio paths do not exist on disk:\n{}",
            missing
                .iter()
                .map(|p| format!("  - {p}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    #[test]
    fn no_duplicate_audio_paths() {
        let paths = all_audio_paths(); // already sorted + deduped
        let before_dedup = 1 // menu
            + LEVEL_BACKGROUND_COUNT as usize
            + combat_pool().len()
            + 2 // boss mid + final
            + ALL_SFX_EVENTS
                .iter()
                .map(|e| e.variants().len())
                .sum::<usize>();
        assert_eq!(paths.len(), before_dedup, "found duplicate audio paths");
    }
}
