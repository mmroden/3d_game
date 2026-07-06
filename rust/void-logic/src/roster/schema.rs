//! Raw serde schema for the roster grammar — the ONLY layer where the TOML's
//! strings exist. `deny_unknown_fields` everywhere: a typo'd field name is a
//! parse error, never a silently-ignored key. Closed vocabularies deserialize
//! into the EXISTING game enums ([`Archetype`], [`MinionTrigger`]) — the game
//! enum IS the grammar. Open references (enemy/swarm/kit/curve keys) stay
//! strings here and die in the linker (`super::link`), which resolves every
//! one into a typed id or reports ALL failures in one error.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
    /// The GDScript/save crossing — append-only, never renumbered.
    pub id: u16,
    pub name: String,
    /// Bestiary lore — every enemy is catalogued.
    pub blurb: String,
    pub model: String,
    pub size: f32,
    pub yaw_offset_deg: f32,
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
    pub shield_frac: Option<f32>,
    pub disengage_frac: Option<f32>,
    pub drain_dps: Option<f32>,
    pub bolt_speed: Option<f32>,
    pub latch_range: Option<f32>,
    pub slow_factor: Option<f32>,
    pub slow_duration: Option<f32>,
    pub slow_interval: Option<f32>,
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

// ── rosters/kits.toml (interim; stage 3 generates it) ───────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitsFile {
    pub kits: BTreeMap<String, KitRaw>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitRaw {
    pub paradigm: KitParadigm,
    /// Repo-relative directory `make assets` populates for this kit — the
    /// disk pin ties the kit's claim to installed reality. The kit's GRID
    /// (tile/story) is never authored: the probe derives it into
    /// kits.generated.toml and the linker joins the two.
    pub install_dir: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KitParadigm {
    Layered,
    Panel,
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
    pub rooms: RoomGrowthRaw,
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

/// rosters/models.generated.toml: model key (file stem) → res:// path.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelsFile {
    pub models: BTreeMap<String, String>,
}

/// rosters/kits.generated.toml: each kit's grid, DERIVED by the probe from
/// its assembly recipe + installed meshes (no dimension is ever authored).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedKitsFile {
    pub kits: BTreeMap<String, GeneratedKitRaw>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedKitRaw {
    pub tile: f32,
    pub story: f32,
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
