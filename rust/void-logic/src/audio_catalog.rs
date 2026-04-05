//! Type-safe audio asset catalog.
//!
//! All audio selection goes through enums — callers never touch path strings.
//! `SfxEvent` selects a sound effect (with random variant picking),
//! `MusicContext` selects menu vs. gameplay music pools.

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

/// Which music context is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicContext {
    Menu,
    Gameplay,
}

const MENU_TRACK: &str = music!("frozen_whispers.wav");

const GAMEPLAY_TRACKS: &[&str] = &[
    music!("days_became_years.wav"),
    music!("cosmic_research_facility.wav"),
    music!("askaris_v.wav"),
    music!("bad_omen.wav"),
    music!("tumor.wav"),
    music!("mist_of_aeons.wav"),
    music!("last_light.wav"),
    music!("departure.wav"),
    music!("sacrifice.wav"),
    music!("erythion_rift.wav"),
];

impl MusicContext {
    /// The single track for this context (Menu) or the first track in the pool.
    pub fn track_path(self) -> &'static str {
        match self {
            Self::Menu => MENU_TRACK,
            Self::Gameplay => GAMEPLAY_TRACKS[0],
        }
    }

    /// The full pool of tracks for rotation. Menu returns a single-element slice.
    pub fn track_pool(self) -> &'static [&'static str] {
        match self {
            Self::Menu => std::slice::from_ref(&MENU_TRACK),
            Self::Gameplay => GAMEPLAY_TRACKS,
        }
    }
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
    /// Laser hits wall or enemy hull.
    ImpactMetal,
    /// Laser hits shield.
    ImpactShield,
    /// Ram collision or enemy death explosion.
    ImpactHeavy,
    /// Player enters portal.
    PortalEnter,
    /// Lootbox collected.
    LootPickup,
    /// Health below critical threshold.
    LowHealthAlert,
    /// Game start / new level boot-up.
    WeaponBoot,
}

/// All `SfxEvent` variants, for exhaustive iteration in tests.
const ALL_SFX_EVENTS: &[SfxEvent] = &[
    SfxEvent::LaserFire,
    SfxEvent::EnemyFire,
    SfxEvent::ImpactMetal,
    SfxEvent::ImpactShield,
    SfxEvent::ImpactHeavy,
    SfxEvent::PortalEnter,
    SfxEvent::LootPickup,
    SfxEvent::LowHealthAlert,
    SfxEvent::WeaponBoot,
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
            Self::ImpactMetal => &[
                sfx!("Impacts/impact_kinetic_light_metal_01.wav"),
                sfx!("Impacts/impact_kinetic_light_metal_02.wav"),
                sfx!("Impacts/impact_kinetic_light_metal_03.wav"),
            ],
            Self::ImpactShield => &[
                sfx!("Impacts/impact_kinetic_heavy_shield_01.wav"),
                sfx!("Impacts/impact_kinetic_heavy_shield_02.wav"),
                sfx!("Impacts/impact_kinetic_heavy_shield_03.wav"),
            ],
            Self::ImpactHeavy => &[
                sfx!("Impacts/impact_kinematic_heavy_metal_01.wav"),
                sfx!("Impacts/impact_kinematic_heavy_metal_02.wav"),
                sfx!("Impacts/impact_kinematic_heavy_metal_03.wav"),
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
pub fn all_audio_paths() -> Vec<&'static str> {
    let mut paths = Vec::new();

    // Music
    paths.push(MENU_TRACK);
    paths.extend_from_slice(GAMEPLAY_TRACKS);

    // SFX
    for event in ALL_SFX_EVENTS {
        paths.extend_from_slice(event.variants());
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
        for path in [MENU_TRACK].iter().chain(GAMEPLAY_TRACKS.iter()) {
            assert!(
                path.starts_with("res://"),
                "music path should start with res://: {path}"
            );
            assert!(
                path.ends_with(".ogg") || path.ends_with(".wav"),
                "music path should end with .ogg or .wav: {path}"
            );
        }
    }

    #[test]
    fn gameplay_has_10_tracks() {
        assert_eq!(
            MusicContext::Gameplay.track_pool().len(),
            10,
            "expected 10 gameplay tracks"
        );
    }

    #[test]
    fn no_duplicate_gameplay_tracks() {
        let mut tracks: Vec<&str> = GAMEPLAY_TRACKS.to_vec();
        tracks.sort();
        for pair in tracks.windows(2) {
            assert_ne!(pair[0], pair[1], "duplicate gameplay track: {}", pair[0]);
        }
    }

    #[test]
    fn menu_track_not_in_gameplay() {
        assert!(
            !GAMEPLAY_TRACKS.contains(&MENU_TRACK),
            "menu track should not be in gameplay rotation"
        );
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
    fn menu_context_returns_single_track() {
        assert_eq!(MusicContext::Menu.track_pool().len(), 1);
        assert_eq!(MusicContext::Menu.track_path(), MENU_TRACK);
    }

    #[test]
    fn no_duplicate_audio_paths() {
        let paths = all_audio_paths(); // already sorted + deduped
        let before_dedup = {
            let mut p = Vec::new();
            p.push(MENU_TRACK);
            p.extend_from_slice(GAMEPLAY_TRACKS);
            for event in ALL_SFX_EVENTS {
                p.extend_from_slice(event.variants());
            }
            p
        };
        assert_eq!(
            paths.len(),
            before_dedup.len(),
            "found duplicate audio paths"
        );
    }
}
