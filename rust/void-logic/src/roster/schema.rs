//! Raw serde schema for the roster grammar — the ONLY layer where the TOML's
//! strings exist. `deny_unknown_fields` everywhere: a typo'd field name is a
//! parse error, never a silently-ignored key. Closed vocabularies deserialize
//! into the EXISTING game enums ([`Archetype`], [`MinionTrigger`]) — the game
//! enum IS the grammar. Open references (enemy/swarm/kit/curve keys) stay
//! strings here and die in the linker (`super::link`), which resolves every
//! one into a typed id or reports ALL failures in one error.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::enemy_ai::Archetype;
use crate::level_assembly::MinionTrigger;

// ── rosters/enemies.toml ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemiesFile {
    pub curves: BTreeMap<String, CurveRaw>,
    pub scaling_defaults: ScalingRaw,
    #[serde(rename = "enemy", default)]
    pub enemies: Vec<EnemyRaw>,
    #[serde(rename = "swarm", default)]
    pub swarms: Vec<SwarmRaw>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurveRaw {
    pub kind: CurveKind,
    pub at_level_1: Option<f32>,
    pub at_peak: Option<f32>,
    pub peak_level: Option<u32>,
    #[serde(default)]
    pub anchor: CurveAnchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveKind {
    /// Constant 1.0 — the stat does not scale with level.
    Flat,
    /// Linear from `at_level_1` to `at_peak` (`peak_level`), flat past.
    Ramp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveAnchor {
    /// Curves ride the absolute game level (today's behaviour).
    #[default]
    Absolute,
    /// Curves restart from 1 at the enemy's fleet entry level.
    Entry,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScalingRaw {
    pub speed: Option<String>,
    pub cooldown: Option<String>,
    pub hp: Option<String>,
    pub damage: Option<String>,
    pub detection: Option<String>,
    pub attack_range: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemyRaw {
    pub key: String,
    pub name: String,
    /// Bestiary lore — every enemy is catalogued.
    pub blurb: String,
    pub model: String,
    pub size: f32,
    pub yaw_offset_deg: f32,
    /// Barrel tips in the model's aim frame (x right, y up, z toward the
    /// player), metres at the def's size. Omitted/empty = fire from centre.
    pub muzzles: Option<Vec<[f32; 3]>>,
    pub ai: Archetype,
    pub reward: u32,
    /// False for retired/death-only/boss-staged entries (default true).
    #[serde(default = "default_true")]
    pub spawns_directly: bool,
    /// Pre-staged bound minions and when they rise (`on_death` = from the
    /// corpse, `on_engage` = the moment the parent's fight starts).
    #[serde(default)]
    pub minions: Vec<MinionRaw>,
    pub stats: StatsRaw,
    pub scaling: Option<ScalingRaw>,
    /// Behaviour switches — every derived archetype parameter is openable
    /// per enemy (owner 2026-07-05: no hidden switches). Omitted = the
    /// archetype's default derivation.
    pub standoff_frac: Option<f32>,
    pub fuse_seconds: Option<f32>,
    pub blast_frac: Option<f32>,
    /// Detonation cloud (seconds): the blast leaves an occluding dust
    /// cloud at blast radius for this long. 0 = today's instant blast.
    pub cloud_seconds: Option<f32>,
    pub shield_frac: Option<f32>,
    pub disengage_frac: Option<f32>,
    pub drain_dps: Option<f32>,
    pub bolt_speed: Option<f32>,
    /// Weapon-pattern switches (docs/design/enemy_verbs.md): bolts per
    /// trigger pull, in-burst gap, pellets per bolt, fan cone, homing turn
    /// rate. Omitted = today's single straight shot.
    pub burst_count: Option<u8>,
    pub burst_seconds: Option<f32>,
    pub pellet_count: Option<u8>,
    pub spread_deg: Option<f32>,
    pub bolt_turn_deg: Option<f32>,
    /// Tractor/repulsor field (SIGNED, m/s²): positive drags the player
    /// toward this enemy, negative shoves away; linear falloff to zero at
    /// attack_range. The one switch where negative is legal config.
    pub pull_accel: Option<f32>,
    /// Alarm aura (metres): while this enemy is engaged, machines within
    /// the radius of IT force-engage the player. 0 = no klaxon.
    pub alert_radius: Option<f32>,
    /// Guardian link (metres): damage to machines within the radius drinks
    /// into this enemy's shield first (declare shield_frac too). 0 = none.
    pub guard_radius: Option<f32>,
    /// Erratic dodge (seconds): mean time between strafe re-rolls while
    /// engaged. 0 = the smooth orbit. Homing bolts counter jinkers.
    pub jink_seconds: Option<f32>,
    pub latch_range: Option<f32>,
    pub slow_factor: Option<f32>,
    pub slow_duration: Option<f32>,
    pub slow_interval: Option<f32>,
    /// While this enemy lives, its room's exits seal red and the boss
    /// bed plays — death re-opens them (design 2026-07-06). Default off.
    pub miniboss: Option<bool>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MinionRaw {
    pub enemy: String,
    /// One-shot triggers: the whole brood. Timed triggers: the BATCH per
    /// interval (owner 2026-07-05: "6 minions every 10 seconds").
    pub count: u8,
    pub trigger: TriggerRaw,
    /// Timed triggers only (required there, rejected elsewhere): the ring
    /// size — the most minions from this entry alive at once. Pre-built at
    /// level creation; dead ones return to the ring (Faucet).
    pub cap: Option<u8>,
}

/// `trigger = "on_death"` / `"on_engage"` / `{ every_seconds = 10.0 }`.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(untagged)]
pub enum TriggerRaw {
    Named(MinionTrigger),
    Timed { every_seconds: f32 },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatsRaw {
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub detection: f32,
    pub attack_range: f32,
    pub cooldown: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwarmRaw {
    pub key: String,
    pub members: Vec<SwarmMemberRaw>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SwarmMemberRaw {
    pub enemy: String,
    pub count: u8,
}



// ── rosters/planets/planet_N.toml ────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanetFile {
    pub planet: u32,
    /// Declared length — the [[level]] blocks must cover exactly
    /// `1..=levels` (a gap or an extra is a link error, both directions).
    pub levels: u32,
    pub kits: Vec<String>,
    /// Room-count growth for GENERATED planets. Required for layered/panel
    /// kits; a fixed planet must omit it — its room count is the authored
    /// zone count (both directions are link errors).
    #[serde(default)]
    pub rooms: Option<RoomGrowthRaw>,
    #[serde(rename = "level", default)]
    pub level_rosters: Vec<LevelRosterRaw>,
    #[serde(rename = "boss_slot", default)]
    pub boss_slots: Vec<BossSlotRaw>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoomGrowthRaw {
    pub base: u32,
    pub per_level: u32,
}

/// What one planet-relative level actually fields — the complete list,
/// nothing inherited (owner 2026-07-05: no cumulative add-only fleets).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelRosterRaw {
    pub relative: u32,
    #[serde(default)]
    pub enemies: Vec<String>,
    #[serde(default)]
    pub swarms: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BossSlotRaw {
    pub at: BossAtRaw,
    pub boss: String,
    pub escorts: EscortRaw,
    pub track: u8,
    pub reward: BossRewardPolicy,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BossAtRaw {
    pub relative: u32,
}

// ── Generated catalogs (written by the roster probe via `make assets`;
//    Serialize so emission round-trips through the exact reading schema) ──

/// rosters/environments/<key>.toml: the HAND-AUTHORED zone map over a fixed
/// environment scene. Coordinates are model-space meters on the 1-meter
/// authoring grid (integer box corners); the kit's declared `scale` is the
/// only bridge to world units.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentFile {
    pub environment: EnvironmentHeaderRaw,
    /// Authored sunlight through the scene's window openings — a fixed
    /// environment's daylight is part of its identity, not a shell default.
    #[serde(default)]
    pub sun: Option<SunRaw>,
    /// The window openings as PANEL LIGHT SOURCES (owner 2026-07-12: all
    /// of an interior's light comes from outside; the sheers are the
    /// diffusers). Real-time has no area lights, so the shell renders each
    /// as a wide shadowless spot shining inward; the lightmap bake later
    /// upgrades them to true emissive panels.
    #[serde(rename = "window", default)]
    pub windows: Vec<WindowRaw>,
    /// The bounce stand-in: real daylight interiors are lit everywhere by
    /// light the windows already poured in (GI); real-time has no bounce,
    /// so the environment authors the ambient term that approximates it.
    /// Applied as a world-environment override for this level, restored on
    /// exit. The lightmap bake eventually replaces it with real bounce.
    #[serde(default)]
    pub ambient: Option<AmbientRaw>,
    #[serde(rename = "zone", default)]
    pub zones: Vec<ZoneRaw>,
}

/// One window panel: where it sits on the shell, which way it shines,
/// and how hard. Model-space meters.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowRaw {
    /// Panel center on the opening's plane.
    pub center: [f32; 3],
    /// Panel width and height (along the wall, then vertical), meters.
    pub size: [f32; 2],
    /// The direction the light shines INTO the room (the opening's inward
    /// normal).
    pub facing: WindowFacingRaw,
    pub energy: f32,
    /// Throw distance into the room, model meters (scaled like everything
    /// else).
    #[serde(default = "default_window_range")]
    pub range: f32,
}

pub(crate) fn default_window_range() -> f32 {
    4.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowFacingRaw {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

/// The environment's ambient fill (see [`EnvironmentFile::ambient`]).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmbientRaw {
    pub energy: f32,
    /// Linear RGB; omitted = warm daylight white.
    #[serde(default = "default_ambient_color")]
    pub color: [f32; 3],
}

pub(crate) fn default_ambient_color() -> [f32; 3] {
    [1.0, 0.97, 0.92]
}

impl WindowFacingRaw {
    /// The inward unit direction this window shines along.
    pub fn direction(self) -> [f32; 3] {
        match self {
            Self::PosX => [1.0, 0.0, 0.0],
            Self::NegX => [-1.0, 0.0, 0.0],
            Self::PosY => [0.0, 1.0, 0.0],
            Self::NegY => [0.0, -1.0, 0.0],
            Self::PosZ => [0.0, 0.0, 1.0],
            Self::NegZ => [0.0, 0.0, -1.0],
        }
    }
}

/// The environment's sun: where it sits and how hard it drives.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SunRaw {
    /// Compass bearing of the sun, degrees: 0 = model north (-z), 90 = east
    /// (+x). Light travels FROM this bearing into the scene.
    pub azimuth_deg: f32,
    /// Degrees above the horizon (0 exclusive .. 90 inclusive).
    pub elevation_deg: f32,
    pub energy: f32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentHeaderRaw {
    pub key: String,
    /// The installed scene, a key into environments.generated.toml.
    pub model: String,
    /// Light energy per square meter of DERIVED window pane (the
    /// rosters/windows/ file carries geometry only; brightness is a look
    /// knob and stays authored). Bigger panes pour in more light.
    #[serde(default = "default_window_energy_per_m2")]
    pub window_energy_per_m2: f32,
}

pub(crate) fn default_window_energy_per_m2() -> f32 {
    4.0
}

/// rosters/windows/<env-key>.toml — GENERATED by the asset pipeline
/// (apartment.py): window panes derived from the scene's glass materials.
/// Geometry only; never hand-edited.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowsFile {
    pub environment: String,
    #[serde(rename = "window", default)]
    pub windows: Vec<WindowGenRaw>,
}

/// One derived pane: like [`WindowRaw`] but without light tuning.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowGenRaw {
    pub center: [f32; 3],
    pub size: [f32; 2],
    pub facing: WindowFacingRaw,
}

/// A coarse gameplay volume over the fixed geometry (culling, spawn
/// grouping, boss/portal placement) — not a walls-accurate room.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneRaw {
    pub key: String,
    #[serde(rename = "box")]
    pub bounds: ZoneBoxRaw,
    /// The player-spawn zone (exactly one per environment, enemy-free).
    #[serde(default)]
    pub start: bool,
    /// The staged-fight arena (exactly one per environment, exactly one
    /// enemy spawn — the anchor the boss rises from).
    #[serde(default)]
    pub boss: bool,
    /// Adjacent zone keys (undirected; declare each opening once).
    #[serde(default)]
    pub links: Vec<String>,
    /// Model-space points inside the box the manifest resolves enemy TYPES
    /// onto — positions are authored, never invented.
    #[serde(default)]
    pub enemy_spawns: Vec<[f32; 3]>,
    #[serde(default)]
    pub loot_spawns: Vec<[f32; 3]>,
}

/// Integer corners on the 1-meter authoring grid: min inclusive,
/// min + extents exclusive.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneBoxRaw {
    pub min: [i32; 3],
    pub extents: [u32; 3],
}


#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscortRaw {
    pub enemy: String,
    pub trigger: MinionTrigger,
    pub count: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BossRewardPolicy {
    /// Blue/green pile — mid-planet bosses.
    ConsolationPile,
    /// The red container (random unowned hull) — planet finals.
    HullContainer,
}
