//! The roster grammar (B13): enemies, swarms, curves, kits, and the planet
//! cascade, loaded from `rosters/*.toml` and linked into a typed IR.
//!
//! Strings exist only at the serde boundary ([`schema`]). The linker resolves
//! every cross-reference into a typed id ([`EnemyId`], [`SwarmId`], [`KitId`],
//! [`CurveId`]) and collects ALL violations — unresolved references, duplicate
//! keys/ids, planet gaps, boss-slot collisions — into one error. The runtime
//! model this module exposes contains no reference strings.
//!
//! The TOML is THE game data — hand-authored, machinery never writes it.
//! The generated artifacts beside it (`TEMPLATE.toml`, `VOCABULARY.md`)
//! are the opposite: never hand-edited, re-rendered by `make build`.

pub mod schema;
#[cfg(test)]
mod probe;
pub mod template;
pub mod vocabulary;

use schema::{
    BossRewardPolicy, CurveAnchor, CurveKind, EnemiesFile, KitParadigm, KitsFile, PlanetFile,
    ScalingRaw,
};

use crate::enemy_ai::Archetype;
use crate::level_assembly::MinionTrigger;

const ENEMIES_TOML: &str = include_str!("../../../../rosters/enemies.toml");
const KITS_TOML: &str = include_str!("../../../../rosters/kits.toml");
const MODELS_TOML: &str = include_str!("../../../../rosters/models.generated.toml");
const KITS_GENERATED_TOML: &str = include_str!("../../../../rosters/kits.generated.toml");
// PLANET_TOMLS: every rosters/planets/*.toml, embedded by build.rs — a new
// planet file wires itself in by existing.
include!(concat!(env!("OUT_DIR"), "/planet_tomls.rs"));

// ── Typed ids: arena indices, no reference strings past the linker ──────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnemyId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwarmId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KitId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurveId(pub usize);

// ── Linked IR ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub detection: f32,
    pub attack_range: f32,
    pub cooldown: f32,
}

/// Resolved behaviour parameters — every derived archetype default is an
/// openable per-enemy switch (owner 2026-07-05); these are the values
/// after resolution (declared switch, or the archetype's derivation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Behavior {
    pub standoff: f32,
    pub fuse_seconds: f32,
    pub blast_radius: f32,
    pub shield: f32,
    pub disengage: f32,
    pub drain_dps: f32,
    /// Projectile speed (m/s) for firing archetypes.
    pub bolt_speed: f32,
    /// Within this range (m) a latcher counts as attached: the slow re-tags
    /// and the drain ticks. 0 = never latches.
    pub latch_range: f32,
    /// Per-tag speed multiplier compounded onto the player (1.0 = no slow).
    pub slow_factor: f32,
    /// Seconds each slow tag lasts.
    pub slow_duration: f32,
    /// Re-tag period while latched.
    pub slow_interval: f32,
    /// While this enemy lives, its room's exits seal red and the boss
    /// bed plays — death re-opens them (design 2026-07-06).
    pub miniboss: bool,
}

impl Default for Behavior {
    /// Pre-`ready` placeholder for shell nodes — inert on every axis; a
    /// live drone overwrites it from its def before the first tick.
    fn default() -> Self {
        Behavior {
            standoff: 0.0,
            fuse_seconds: 0.0,
            blast_radius: 0.0,
            shield: 0.0,
            disengage: 0.0,
            drain_dps: 0.0,
            bolt_speed: 0.0,
            latch_range: 0.0,
            slow_factor: 1.0,
            slow_duration: 0.0,
            slow_interval: 0.0,
            miniboss: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scaling {
    pub speed: CurveId,
    pub cooldown: CurveId,
    pub hp: CurveId,
    pub damage: CurveId,
    pub detection: CurveId,
    pub attack_range: CurveId,
}

/// One bound-minion entry: for one-shot triggers `count` is the whole
/// brood (`cap == count`); for timed triggers `count` is the batch per
/// interval and `cap` the pre-built ring it draws from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinionDef {
    pub enemy: EnemyId,
    pub count: u8,
    pub trigger: MinionTrigger,
    pub cap: u8,
}

#[derive(Debug, Clone)]
pub struct EnemyDef {
    pub key: String,
    /// The GDScript/save crossing — append-only.
    pub crossing_id: u16,
    pub name: String,
    pub model: String,
    pub size: f32,
    pub yaw_offset_deg: f32,
    pub ai: Archetype,
    pub reward: u32,
    pub spawns_directly: bool,
    /// Pre-staged bound minions and their rise rules.
    pub minions: Vec<MinionDef>,
    /// Bestiary lore.
    pub blurb: String,
    pub stats: Stats,
    pub scaling: Scaling,
    pub behavior: Behavior,
}

impl Roster {
    /// A def's stats at a level: every one of the six baselines multiplied
    /// by its DECLARED curve — the one door level scaling flows through.
    pub fn stats_at(&self, id: EnemyId, level: u32) -> Stats {
        let def = self.enemy(id);
        let mul = |c: CurveId| self.curve(c).eval(level);
        Stats {
            hp: def.stats.hp * mul(def.scaling.hp),
            speed: def.stats.speed * mul(def.scaling.speed),
            damage: def.stats.damage * mul(def.scaling.damage),
            detection: def.stats.detection * mul(def.scaling.detection),
            attack_range: def.stats.attack_range * mul(def.scaling.attack_range),
            cooldown: def.stats.cooldown * mul(def.scaling.cooldown),
        }
    }

    /// The AI configuration at a level: ranges from the scaled stats,
    /// archetype parameters from the resolved behaviour switches, with
    /// every derived absolute riding its base stat's curve (standoff and
    /// blast follow attack_range, disengage follows detection, shield
    /// follows hp) — a declared ramp moves the whole machine.
    pub fn ai_config_at(&self, id: EnemyId, level: u32) -> crate::enemy_ai::DroneConfig {
        let def = self.enemy(id);
        let stats = self.stats_at(id, level);
        let mul = |c: CurveId| self.curve(c).eval(level);
        let hp_mul = mul(def.scaling.hp);
        let detection_mul = mul(def.scaling.detection);
        let attack_mul = mul(def.scaling.attack_range);
        crate::enemy_ai::DroneConfig {
            archetype: def.ai,
            detection_range: stats.detection,
            attack_range: stats.attack_range,
            disengage_range: def.behavior.disengage * detection_mul,
            health: crate::newtypes::Health::new(stats.hp),
            attack_cooldown: stats.cooldown,
            standoff_range: def.behavior.standoff * attack_mul,
            fuse_seconds: def.behavior.fuse_seconds,
            blast_radius: def.behavior.blast_radius * attack_mul,
            shield: (def.behavior.shield > 0.0)
                .then(|| crate::newtypes::Shield::new(def.behavior.shield * hp_mul)),
        }
    }

    /// Every enemy id in declaration order — THE catalog order.
    pub fn enemy_ids(&self) -> impl Iterator<Item = EnemyId> + '_ {
        (0..self.enemies.len()).map(EnemyId)
    }
}

#[derive(Debug, Clone)]
pub struct SwarmDef {
    pub key: String,
    pub members: Vec<(EnemyId, u8)>,
}

#[derive(Debug, Clone, Copy)]
pub struct CurveDef {
    pub kind: CurveKind,
    pub at_level_1: f32,
    pub at_peak: f32,
    pub peak_level: u32,
    pub anchor: CurveAnchor,
}

impl CurveDef {
    /// Evaluate at a game level (`Entry`-anchored curves are evaluated by the
    /// caller passing a level already rebased to the enemy's entry).
    pub fn eval(&self, level: u32) -> f32 {
        match self.kind {
            CurveKind::Flat => 1.0,
            CurveKind::Ramp => {
                let level = level.clamp(1, self.peak_level);
                let t = (level - 1) as f32 / (self.peak_level - 1).max(1) as f32;
                self.at_level_1 + (self.at_peak - self.at_level_1) * t
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct KitDef {
    pub key: String,
    pub paradigm: KitParadigm,
    pub tile: f32,
    pub story: f32,
    /// Repo-relative directory `make assets` populates for this kit.
    pub install_dir: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnRef {
    Enemy(EnemyId),
    Swarm(SwarmId),
}

#[derive(Debug, Clone)]
pub struct PlanetDef {
    pub planet: u32,
    /// Declared length; `rosters` covers exactly `1..=levels`.
    pub levels: u32,
    pub kits: Vec<KitId>,
    pub rooms_base: u32,
    pub rooms_per_level: u32,
    /// What each planet-relative level fields — index `relative - 1`.
    /// Complete lists, nothing inherited between levels.
    pub rosters: Vec<Vec<SpawnRef>>,
    pub boss_slots: Vec<BossSlot>,
}

#[derive(Debug, Clone)]
pub struct BossSlot {
    pub at_relative: u32,
    pub boss: EnemyId,
    pub escorts: EscortDef,
    pub track: u8,
    pub reward: BossRewardPolicy,
}

/// The escort declaration on a boss slot: the boss's minions are the slot's
/// call, count included — there is no formula behind it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EscortDef {
    pub enemy: EnemyId,
    pub trigger: MinionTrigger,
    pub count: u8,
}

#[derive(Debug)]
pub struct Roster {
    pub enemies: Vec<EnemyDef>,
    pub swarms: Vec<SwarmDef>,
    pub curves: Vec<(String, CurveDef)>,
    pub kits: Vec<KitDef>,
    /// Sorted by planet number, contiguous from 1.
    pub planets: Vec<PlanetDef>,
}

impl Roster {
    pub fn enemy(&self, id: EnemyId) -> &EnemyDef {
        &self.enemies[id.0]
    }

    pub fn enemy_by_key(&self, key: &str) -> Option<EnemyId> {
        self.enemies.iter().position(|e| e.key == key).map(EnemyId)
    }

    pub fn enemy_by_crossing_id(&self, crossing_id: u16) -> Option<EnemyId> {
        self.enemies
            .iter()
            .position(|e| e.crossing_id == crossing_id)
            .map(EnemyId)
    }

    /// The demanding door for LIVE references — spawners stamping drones,
    /// kill events off living enemies: an id the grammar doesn't declare is
    /// a bad config and dies here, loudly. History readers (saves, bestiary
    /// summaries) use `enemy_by_crossing_id` and tolerate retired ids.
    pub fn expect_enemy_by_crossing_id(&self, crossing_id: u16) -> EnemyId {
        self.enemy_by_crossing_id(crossing_id).unwrap_or_else(|| {
            panic!(
                "crossing id {crossing_id} is not in rosters/enemies.toml — \
                 a scene or spawner stamped an enemy the grammar doesn't declare"
            )
        })
    }

    pub fn curve(&self, id: CurveId) -> &CurveDef {
        &self.curves[id.0].1
    }

    /// Resolve a game level against the DECLARED planet lengths: the planet
    /// def serving it, the planet-relative level, and the planet NUMBER
    /// (virtual past the table: the newest planet's shape repeats — its
    /// length, slots, kits — while the number keeps counting, so
    /// escalation formulas keep escalating).
    fn locate(&self, level: u32) -> (&PlanetDef, u32, u32) {
        let mut start = 1u32;
        for p in &self.planets {
            if level < start + p.levels {
                return (p, level - start + 1, p.planet);
            }
            start += p.levels;
        }
        let last = self.planets.last().expect("linker guarantees planets");
        let overflow = level - start;
        (
            last,
            overflow % last.levels + 1,
            last.planet + 1 + overflow / last.levels,
        )
    }

    /// The planet def serving a game level.
    pub fn planet_for_level(&self, level: u32) -> &PlanetDef {
        self.locate(level).0
    }

    /// The planet NUMBER a level belongs to (counts past the declared
    /// table) and the planet-relative level — the game's level algebra,
    /// sourced from the declared lengths.
    pub fn planet_number_and_relative(&self, level: u32) -> (u32, u32) {
        let (_, relative, number) = self.locate(level);
        (number, relative)
    }

    /// The enemy ids the given level fields (its declared list, swarm
    /// members expanded, deduplicated in declaration order). Beyond the
    /// declared planets, every level fields the newest planet's FINAL
    /// roster — the endgame holds its hardest mix until new files arrive.
    pub fn roster_for_level(&self, level: u32) -> Vec<EnemyId> {
        let (def, relative, number) = self.locate(level);
        let beyond = number != def.planet;
        let list = if beyond {
            def.rosters.last().expect("linker guarantees coverage")
        } else {
            &def.rosters[(relative - 1) as usize]
        };
        let mut out: Vec<EnemyId> = Vec::new();
        let push = |id: EnemyId, out: &mut Vec<EnemyId>| {
            if !out.contains(&id) {
                out.push(id);
            }
        };
        for unit in list {
            match unit {
                SpawnRef::Enemy(id) => push(*id, &mut out),
                SpawnRef::Swarm(sid) => {
                    for (id, _) in &self.swarms[sid.0].members {
                        push(*id, &mut out);
                    }
                }
            }
        }
        out
    }

    /// The boss slot staged at this level, if any (beyond the declared
    /// planets the newest planet's slot schedule repeats).
    pub fn boss_slot_for_level(&self, level: u32) -> Option<&BossSlot> {
        let (def, relative, _) = self.locate(level);
        def.boss_slots.iter().find(|s| s.at_relative == relative)
    }
}

// ── Load + link ──────────────────────────────────────────────────────────

/// Parse and link the embedded roster files. Every violation — parse errors,
/// unresolved references, duplicate keys/ids, planet gaps, slot collisions —
/// is collected; the Err carries them all.
pub fn load() -> Result<Roster, String> {
    load_from(ENEMIES_TOML, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, PLANET_TOMLS)
}

/// The one shared instance (parsed on first use; a bad grammar panics with
/// the full violation list — the parse pin fails long before a run does).
pub fn roster() -> &'static Roster {
    static ROSTER: std::sync::OnceLock<Roster> = std::sync::OnceLock::new();
    ROSTER.get_or_init(|| load().expect("rosters/ must parse and link"))
}

fn load_from(
    enemies: &str,
    kits: &str,
    kit_grids: &str,
    models: &str,
    planets: &[&str],
) -> Result<Roster, String> {
    let mut errors: Vec<String> = Vec::new();

    let enemies_file: Option<EnemiesFile> = match toml::from_str(enemies) {
        Ok(f) => Some(f),
        Err(e) => {
            errors.push(format!("enemies.toml: {e}"));
            None
        }
    };
    let kits_file: Option<KitsFile> = match toml::from_str(kits) {
        Ok(f) => Some(f),
        Err(e) => {
            errors.push(format!("kits.toml: {e}"));
            None
        }
    };
    let models_file: Option<schema::ModelsFile> = match toml::from_str(models) {
        Ok(f) => Some(f),
        Err(e) => {
            errors.push(format!("models.generated.toml: {e}"));
            None
        }
    };
    let kit_grids_file: Option<schema::GeneratedKitsFile> = match toml::from_str(kit_grids) {
        Ok(f) => Some(f),
        Err(e) => {
            errors.push(format!("kits.generated.toml: {e}"));
            None
        }
    };
    let planet_files: Vec<PlanetFile> = planets
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match toml::from_str(s) {
            Ok(f) => Some(f),
            Err(e) => {
                errors.push(format!("planets file #{}: {e}", i + 1));
                None
            }
        })
        .collect();

    let (Some(enemies_file), Some(kits_file), Some(kit_grids_file), Some(models_file)) =
        (enemies_file, kits_file, kit_grids_file, models_file)
    else {
        return Err(errors.join("\n"));
    };

    link(enemies_file, kits_file, kit_grids_file, models_file, planet_files, errors)
}

fn link(
    enemies_file: EnemiesFile,
    kits_file: KitsFile,
    kit_grids_file: schema::GeneratedKitsFile,
    models_file: schema::ModelsFile,
    planet_files: Vec<PlanetFile>,
    mut errors: Vec<String>,
) -> Result<Roster, String> {
    // Curves (BTreeMap: TOML itself rejects a re-declared [curves.X] table).
    let curves: Vec<(String, CurveDef)> = enemies_file
        .curves
        .iter()
        .filter_map(|(name, raw)| {
            let def = match raw.kind {
                CurveKind::Flat => {
                    for (field, present) in [
                        ("at_level_1", raw.at_level_1.is_some()),
                        ("at_peak", raw.at_peak.is_some()),
                        ("peak_level", raw.peak_level.is_some()),
                    ] {
                        if present {
                            errors.push(format!(
                                "curve '{name}': flat curves take no '{field}'"
                            ));
                        }
                    }
                    CurveDef {
                        kind: CurveKind::Flat,
                        at_level_1: 1.0,
                        at_peak: 1.0,
                        peak_level: 1,
                        anchor: raw.anchor,
                    }
                }
                CurveKind::Ramp => {
                    let (Some(at_level_1), Some(at_peak), Some(peak_level)) =
                        (raw.at_level_1, raw.at_peak, raw.peak_level)
                    else {
                        errors.push(format!(
                            "curve '{name}': ramp needs at_level_1, at_peak, peak_level"
                        ));
                        return None;
                    };
                    if peak_level < 2 {
                        errors.push(format!("curve '{name}': peak_level must be ≥ 2"));
                    }
                    CurveDef {
                        kind: CurveKind::Ramp,
                        at_level_1,
                        at_peak,
                        peak_level,
                        anchor: raw.anchor,
                    }
                }
            };
            Some((name.clone(), def))
        })
        .collect();
    let curve_by_name = |name: &str, errors: &mut Vec<String>, at: &str| -> CurveId {
        match curves.iter().position(|(n, _)| n == name) {
            Some(i) => CurveId(i),
            None => {
                errors.push(format!("{at}: unknown curve '{name}'"));
                CurveId(0)
            }
        }
    };

    // Enemy identity pass: keys/ids first so references can resolve.
    let raw_enemies = enemies_file.enemies;
    for (i, e) in raw_enemies.iter().enumerate() {
        if raw_enemies[..i].iter().any(|p| p.key == e.key) {
            errors.push(format!("duplicate enemy key '{}'", e.key));
        }
        if raw_enemies[..i].iter().any(|p| p.id == e.id) {
            errors.push(format!(
                "duplicate enemy id {} ('{}') — ids are the append-only crossing",
                e.id, e.key
            ));
        }
    }
    let enemy_by_key = |key: &str, errors: &mut Vec<String>, at: &str| -> EnemyId {
        match raw_enemies.iter().position(|e| e.key == key) {
            Some(i) => EnemyId(i),
            None => {
                errors.push(format!("{at}: unknown enemy '{key}'"));
                EnemyId(0)
            }
        }
    };

    let resolve_scaling = |raw: Option<&ScalingRaw>,
                           defaults: &ScalingRaw,
                           errors: &mut Vec<String>,
                           at: &str|
     -> Scaling {
        let pick = |field: &str,
                    own: &Option<String>,
                    default: &Option<String>,
                    errors: &mut Vec<String>| {
            match own.as_deref().or(default.as_deref()) {
                Some(name) => curve_by_name(name, errors, at),
                None => {
                    errors.push(format!(
                        "{at}: no '{field}' curve (neither declared nor in scaling_defaults)"
                    ));
                    CurveId(0)
                }
            }
        };
        let none = ScalingRaw {
            speed: None,
            cooldown: None,
            hp: None,
            damage: None,
            detection: None,
            attack_range: None,
        };
        let own = raw.unwrap_or(&none);
        Scaling {
            speed: pick("speed", &own.speed, &defaults.speed, errors),
            cooldown: pick("cooldown", &own.cooldown, &defaults.cooldown, errors),
            hp: pick("hp", &own.hp, &defaults.hp, errors),
            damage: pick("damage", &own.damage, &defaults.damage, errors),
            detection: pick("detection", &own.detection, &defaults.detection, errors),
            attack_range: pick(
                "attack_range",
                &own.attack_range,
                &defaults.attack_range,
                errors,
            ),
        }
    };

    let defaults = enemies_file.scaling_defaults;
    let enemies: Vec<EnemyDef> = raw_enemies
        .iter()
        .map(|e| {
            let at = format!("enemy '{}'", e.key);
            for (field, value) in [
                ("standoff_frac", e.standoff_frac),
                ("fuse_seconds", e.fuse_seconds),
                ("blast_frac", e.blast_frac),
                ("shield_frac", e.shield_frac),
                ("disengage_frac", e.disengage_frac),
                ("drain_dps", e.drain_dps),
                ("bolt_speed", e.bolt_speed),
                ("latch_range", e.latch_range),
                ("slow_factor", e.slow_factor),
                ("slow_duration", e.slow_duration),
                ("slow_interval", e.slow_interval),
            ] {
                if let Some(v) = value {
                    if !v.is_finite() || v < 0.0 {
                        errors.push(format!("{at}: {field} must be finite and ≥ 0, got {v}"));
                    }
                }
            }
            let behavior = {
                use crate::enemy_ai::Archetype::*;
                let standoff_default = match e.ai {
                    Shooter | Kiter | Tank => 0.6,
                    Swarmer | Bomber => 0.0,
                };
                Behavior {
                    standoff: e.standoff_frac.unwrap_or(standoff_default) * e.stats.attack_range,
                    fuse_seconds: e
                        .fuse_seconds
                        .unwrap_or(if e.ai == Bomber { 1.0 } else { 0.0 }),
                    blast_radius: e.blast_frac.unwrap_or(if e.ai == Bomber { 1.5 } else { 0.0 })
                        * e.stats.attack_range,
                    shield: e.shield_frac.unwrap_or(if e.ai == Tank { 0.5 } else { 0.0 })
                        * e.stats.hp,
                    disengage: e.disengage_frac.unwrap_or(1.2) * e.stats.detection,
                    drain_dps: e.drain_dps.unwrap_or(0.0),
                    bolt_speed: e.bolt_speed.unwrap_or(13.0),
                    // The latch set: live on swarmers, inert elsewhere.
                    latch_range: e
                        .latch_range
                        .unwrap_or(if e.ai == Swarmer { 2.0 } else { 0.0 }),
                    slow_factor: e
                        .slow_factor
                        .unwrap_or(if e.ai == Swarmer { 0.7 } else { 1.0 }),
                    slow_duration: e
                        .slow_duration
                        .unwrap_or(if e.ai == Swarmer { 2.0 } else { 0.0 }),
                    slow_interval: e
                        .slow_interval
                        .unwrap_or(if e.ai == Swarmer { 0.5 } else { 0.0 }),
                    miniboss: e.miniboss.unwrap_or(false),
                }
            };
            EnemyDef {
                key: e.key.clone(),
                crossing_id: e.id,
                name: e.name.clone(),
                // The def declares a model KEY; the linked def carries the
                // catalog's res:// path, so consumers never see keys.
                model: match models_file.models.get(&e.model) {
                    Some(path) => path.clone(),
                    None => {
                        errors.push(format!(
                            "{at}: unknown model '{}' — not in \
                             rosters/models.generated.toml (run `make assets`)",
                            e.model
                        ));
                        String::new()
                    }
                },
                size: e.size,
                yaw_offset_deg: e.yaw_offset_deg,
                ai: e.ai,
                reward: e.reward,
                spawns_directly: e.spawns_directly,
                minions: e
                    .minions
                    .iter()
                    .map(|m| {
                        let enemy = enemy_by_key(&m.enemy, &mut errors, &at);
                        let (trigger, cap) = match (m.trigger, m.cap) {
                            (schema::TriggerRaw::Named(t), None) => (t, m.count),
                            (schema::TriggerRaw::Named(t), Some(_)) => {
                                errors.push(format!(
                                    "{at}: '{}' — cap is a TIMED-trigger field \
                                     (one-shot broods are just count)",
                                    m.enemy
                                ));
                                (t, m.count)
                            }
                            (schema::TriggerRaw::Timed { every_seconds }, Some(cap)) => {
                                if every_seconds <= 0.0 || !every_seconds.is_finite() {
                                    errors.push(format!(
                                        "{at}: '{}' — every_seconds must be finite and > 0",
                                        m.enemy
                                    ));
                                }
                                if cap < m.count {
                                    errors.push(format!(
                                        "{at}: '{}' — cap {cap} can't hold a batch of {}",
                                        m.enemy, m.count
                                    ));
                                }
                                (MinionTrigger::Every(every_seconds), cap)
                            }
                            (schema::TriggerRaw::Timed { .. }, None) => {
                                errors.push(format!(
                                    "{at}: '{}' — a timed emitter needs a cap (the \
                                     pre-built ring it draws from)",
                                    m.enemy
                                ));
                                (MinionTrigger::Every(1.0), m.count)
                            }
                        };
                        if m.count == 0 {
                            errors.push(format!("{at}: '{}' — a zero-count entry", m.enemy));
                        }
                        MinionDef { enemy, count: m.count, trigger, cap }
                    })
                    .collect(),
                blurb: {
                    if e.blurb.trim().is_empty() {
                        errors.push(format!("{at}: empty blurb — every enemy is catalogued"));
                    }
                    e.blurb.clone()
                },
                stats: Stats {
                    hp: e.stats.hp,
                    speed: e.stats.speed,
                    damage: e.stats.damage,
                    detection: e.stats.detection,
                    attack_range: e.stats.attack_range,
                    cooldown: e.stats.cooldown,
                },
                scaling: resolve_scaling(e.scaling.as_ref(), &defaults, &mut errors, &at),
                behavior,
            }
        })
        .collect();

    // Swarms.
    let raw_swarms = enemies_file.swarms;
    for (i, s) in raw_swarms.iter().enumerate() {
        if raw_swarms[..i].iter().any(|p| p.key == s.key) {
            errors.push(format!("duplicate swarm key '{}'", s.key));
        }
        if s.members.is_empty() {
            errors.push(format!("swarm '{}' has no members", s.key));
        }
    }
    let swarms: Vec<SwarmDef> = raw_swarms
        .iter()
        .map(|s| SwarmDef {
            key: s.key.clone(),
            members: s
                .members
                .iter()
                .map(|m| {
                    let at = format!("swarm '{}'", s.key);
                    let id = enemy_by_key(&m.enemy, &mut errors, &at);
                    if !raw_enemies[id.0].spawns_directly {
                        errors.push(format!(
                            "{at}: member '{}' never spawns directly — swarms \
                             place live roamers",
                            m.enemy
                        ));
                    }
                    (id, m.count)
                })
                .collect(),
        })
        .collect();

    // The engine binds minions ONE level deep (a parent activates its own
    // reserve; nothing recurses). The grammar must not express more.
    for e in &raw_enemies {
        for m in &e.minions {
            if let Some(target) = raw_enemies.iter().find(|t| t.key == m.enemy) {
                if !target.minions.is_empty() {
                    errors.push(format!(
                        "enemy '{}': minion '{}' has minions of its own — \
                         nesting is unsupported (one level deep)",
                        e.key, m.enemy
                    ));
                }
            }
        }
    }

    // Kits (BTreeMap: TOML rejects re-declared kit tables). The grid joins
    // in from the probe's derived measurements — never authored.
    let kits: Vec<KitDef> = kits_file
        .kits
        .iter()
        .map(|(key, raw)| {
            let grid = match kit_grids_file.kits.get(key) {
                Some(g) => g.clone(),
                None => {
                    errors.push(format!(
                        "kit '{key}': no derived grid in \
                         rosters/kits.generated.toml (run `make assets`)"
                    ));
                    schema::GeneratedKitRaw { tile: 0.0, story: 0.0 }
                }
            };
            KitDef {
                key: key.clone(),
                paradigm: raw.paradigm,
                tile: grid.tile,
                story: grid.story,
                install_dir: raw.install_dir.clone(),
            }
        })
        .collect();
    let kit_by_key = |key: &str, errors: &mut Vec<String>, at: &str| -> KitId {
        match kits.iter().position(|k| k.key == key) {
            Some(i) => KitId(i),
            None => {
                errors.push(format!("{at}: unknown kit '{key}'"));
                KitId(0)
            }
        }
    };

    // Planets: contiguous from 1, one file per planet, fleets in-band.
    let mut planets: Vec<PlanetDef> = Vec::new();
    for f in &planet_files {
        let at = format!("planet {}", f.planet);
        if planets.iter().any(|p| p.planet == f.planet) {
            errors.push(format!("{at}: declared twice"));
        }
        if f.kits.is_empty() {
            errors.push(format!("{at}: no kits"));
        }
        if f.levels == 0 {
            errors.push(format!("{at}: declares zero levels"));
        }
        // Coverage is exact, both directions (owner 2026-07-05): a declared
        // length without a roster, or a roster outside the declared length,
        // are both link errors.
        for block in &f.level_rosters {
            if block.relative == 0 || block.relative > f.levels {
                errors.push(format!(
                    "{at}: [[level]] relative {} is outside the declared 1..={}",
                    block.relative, f.levels
                ));
            }
            if f
                .level_rosters
                .iter()
                .filter(|b| b.relative == block.relative)
                .count()
                > 1
            {
                errors.push(format!(
                    "{at}: [[level]] relative {} declared more than once",
                    block.relative
                ));
            }
        }
        let mut rosters: Vec<Vec<SpawnRef>> = Vec::new();
        for relative in 1..=f.levels {
            let Some(block) = f.level_rosters.iter().find(|b| b.relative == relative)
            else {
                errors.push(format!(
                    "{at}: declares {} levels but relative {relative} has no \
                     [[level]] roster",
                    f.levels
                ));
                rosters.push(Vec::new());
                continue;
            };
            let mut list: Vec<SpawnRef> = Vec::new();
            for key in &block.enemies {
                let id = enemy_by_key(key, &mut errors, &at);
                if !raw_enemies[id.0].spawns_directly {
                    errors.push(format!(
                        "{at}: relative {relative} lists '{key}', which never \
                         spawns directly (retired / death-spawned / boss-staged)"
                    ));
                }
                list.push(SpawnRef::Enemy(id));
            }
            for key in &block.swarms {
                match raw_swarms.iter().position(|w| w.key == *key) {
                    Some(i) => list.push(SpawnRef::Swarm(SwarmId(i))),
                    None => errors.push(format!("{at}: unknown swarm '{key}'")),
                }
            }
            if list.is_empty() {
                errors.push(format!(
                    "{at}: relative {relative} fields nothing — an enemy-less \
                     level must be an explicit design decision, not a default"
                ));
            }
            rosters.push(list);
        }

        let mut boss_slots: Vec<BossSlot> = Vec::new();
        for slot in &f.boss_slots {
            let rel = slot.at.relative;
            if rel == 0 || rel > f.levels {
                errors.push(format!(
                    "{at}: boss slot at relative {rel} is outside the planet (1..={})",
                    f.levels
                ));
            }
            if boss_slots.iter().any(|s| s.at_relative == rel) {
                errors.push(format!("{at}: two boss slots at relative {rel}"));
            }
            if slot.track == 0 {
                errors.push(format!("{at}: boss track indices start at 1"));
            }
            let boss_id = enemy_by_key(&slot.boss, &mut errors, &at);
            // A slot-staged boss may field TIMED emitters — sustained
            // pressure is the def's own character (owner 2026-07-05). One-
            // shot broods stay forbidden: they'd double-dip against the
            // slot's declared escorts (one door).
            if raw_enemies[boss_id.0]
                .minions
                .iter()
                .any(|m| matches!(m.trigger, schema::TriggerRaw::Named(_)))
            {
                errors.push(format!(
                    "{at}: boss '{}' declares a one-shot brood — a staged \
                     boss's on_death/on_engage minions are the slot's \
                     escorts (one door); only timed emitters may ride the def",
                    slot.boss
                ));
            }
            let escort_id = enemy_by_key(&slot.escorts.enemy, &mut errors, &at);
            if slot.escorts.count == 0 {
                errors.push(format!(
                    "{at}: escort count must be at least 1 — the count is \
                     the slot's declared call, and zero declares no fight"
                ));
            }
            if !raw_enemies[escort_id.0].minions.is_empty() {
                errors.push(format!(
                    "{at}: escort '{}' has minions of its own — nesting is \
                     unsupported (one level deep)",
                    slot.escorts.enemy
                ));
            }
            boss_slots.push(BossSlot {
                at_relative: rel,
                boss: boss_id,
                escorts: EscortDef {
                    enemy: escort_id,
                    trigger: slot.escorts.trigger,
                    count: slot.escorts.count,
                },
                track: slot.track,
                reward: slot.reward,
            });
        }

        planets.push(PlanetDef {
            planet: f.planet,
            levels: f.levels,
            kits: f.kits.iter().map(|k| kit_by_key(k, &mut errors, &at)).collect(),
            rooms_base: f.rooms.base,
            rooms_per_level: f.rooms.per_level,
            rosters,
            boss_slots,
        });
    }
    planets.sort_by_key(|p| p.planet);
    for (i, p) in planets.iter().enumerate() {
        if p.planet != i as u32 + 1 {
            errors.push(format!(
                "planet files must be contiguous from 1: found planet {} at position {}",
                p.planet,
                i + 1
            ));
        }
    }
    if planets.is_empty() {
        errors.push("no planet files".into());
    }

    if errors.is_empty() {
        Ok(Roster { enemies, swarms, curves, kits, planets })
    } else {
        Err(errors.join("\n"))
    }
}

#[cfg(test)]
mod property_tests;

// ── Grammar tests: parse/link/validation, the leveled-door scaling
//    contracts, kit-install pins, and (in property_tests) the generative
//    sweep. No goldens: the generated artifacts (TEMPLATE.toml,
//    VOCABULARY.md) are re-rendered by `make build`, never pinned —
//    behavior pins live with consumers. ──

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded() -> Roster {
        load().expect("rosters/ must parse and link")
    }

    #[test]
    fn the_foundational_roster_parses_and_links() {
        if let Err(e) = load() {
            panic!("roster load failed:\n{e}");
        }
    }

    #[test]
    fn a_negative_switch_is_a_link_error() {
        let doctored = ENEMIES_TOML.replace("drain_dps = 6.0", "drain_dps = -6.0");
        let err = load_from(&doctored, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, PLANET_TOMLS).unwrap_err();
        assert!(err.contains("drain_dps must be finite"), "{err}");
    }

    #[test]
    fn a_kit_without_a_derived_grid_is_a_link_error() {
        let doctored = KITS_GENERATED_TOML.replace("[kits.quaternius_megakit]", "[kits.renamed]");
        let err =
            load_from(ENEMIES_TOML, KITS_TOML, &doctored, MODELS_TOML, PLANET_TOMLS).unwrap_err();
        assert!(
            err.contains("kit 'quaternius_megakit': no derived grid"),
            "the linker must name the unmeasured kit: {err}"
        );
    }

    #[test]
    fn the_derived_grids_match_the_recipes() {
        // The committed grid file against a fresh derivation from the
        // installed meshes — a re-authored asset or stale probe run
        // surfaces here (run `make assets`).
        assert_eq!(
            KITS_GENERATED_TOML,
            super::probe::kit_grids(),
            "kits.generated.toml is stale — run `make assets`"
        );
    }

    #[test]
    fn an_unknown_model_key_is_a_link_error() {
        // Doctor the FIRST model line, whatever key it names — anchoring
        // on a specific model broke the moment the owner retuned the TOML
        // (feedback 2026-07-06: tuning must never break tests).
        let model_line = ENEMIES_TOML
            .lines()
            .find(|l| l.trim_start().starts_with("model = "))
            .expect("the shipped grammar declares models");
        let doctored = ENEMIES_TOML.replacen(model_line, "model = \"no_such_model\"", 1);
        let err = load_from(&doctored, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, PLANET_TOMLS).unwrap_err();
        assert!(
            err.contains("unknown model 'no_such_model'"),
            "the linker must name the missing catalog key: {err}"
        );
    }

    #[test]
    fn every_catalog_model_is_installed_on_disk() {
        // The committed catalog and the installed files must agree in both
        // directions — a probe that ran against a different install, or a
        // model deleted since, surfaces here (run `make assets`).
        let repo = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let catalog: schema::ModelsFile =
            toml::from_str(MODELS_TOML).expect("models.generated.toml parses");
        for (key, res) in &catalog.models {
            let disk = format!("{repo}/godot/{}", res.trim_start_matches("res://"));
            assert!(
                std::path::Path::new(&disk).is_file(),
                "catalog model '{key}' missing on disk at {disk}"
            );
        }
    }

    #[test]
    fn every_planet_file_on_disk_is_embedded() {
        // build.rs globs rosters/planets/ — this pin proves a dropped-in
        // planet_N.toml can never be silently absent from the build (the
        // hand-listed array this replaced could forget one).
        let repo = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let on_disk = std::fs::read_dir(format!("{repo}/rosters/planets"))
            .expect("rosters/planets/ exists")
            .filter(|e| {
                e.as_ref().is_ok_and(|e| {
                    e.path().extension().is_some_and(|x| x == "toml")
                })
            })
            .count();
        assert_eq!(PLANET_TOMLS.len(), on_disk,
            "every planet file on disk is embedded");
    }

    #[test]
    fn every_stat_rides_its_declared_curve() {
        // The PRODUCTION declarations at the peak level (owner's tuning):
        // speed 1.15, cooldown 0.85, hp 3.0, damage 1.5; detection and
        // attack_range declared flat.
        let roster = loaded();
        let id = roster.enemy_by_key("sentry_drone").unwrap();
        let base = roster.enemy(id).stats;
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        let peak = roster.stats_at(id, 10);
        assert!(close(peak.speed, base.speed * 1.15), "speed rides its ramp");
        assert!(close(peak.cooldown, base.cooldown * 0.85), "cooldown rides");
        assert!(close(peak.hp, base.hp * 3.0), "hp toughens on hp_standard");
        assert!(close(peak.damage, base.damage * 1.5), "damage rides");
        assert!(close(peak.detection, base.detection), "detection: flat by call");
        assert!(close(peak.attack_range, base.attack_range), "range: flat by call");
        let l1 = roster.stats_at(id, 1);
        assert!(close(l1.speed, base.speed * 0.6), "level 1 crawls");
        assert!(close(l1.cooldown, base.cooldown * 1.4), "level 1 shoots slow");
        assert!(close(l1.hp, base.hp), "hp_standard starts at baseline");
    }

    #[test]
    fn derived_ranges_ride_their_base_stats_curve() {
        // detection/attack_range pointed at the speed ramp: the AI's derived
        // absolutes (disengage, standoff) must follow their base stat, and
        // shield follows the production hp ramp — or a declared curve is a
        // lever that only half-moves the machine.
        let doctored = ENEMIES_TOML
            .replace("detection = \"flat\"", "detection = \"speed_standard\"")
            .replace("attack_range = \"flat\"", "attack_range = \"speed_standard\"");
        let roster =
            load_from(&doctored, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, PLANET_TOMLS)
                .unwrap();
        let id = roster.enemy_by_key("quad_shell").unwrap(); // tank: shielded
        let def = roster.enemy(id);
        let cfg = roster.ai_config_at(id, 10); // peak: speed ramp 1.15, hp 3.0
        let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
        assert!(close(cfg.detection_range, def.stats.detection * 1.15));
        assert!(close(cfg.attack_range, def.stats.attack_range * 1.15));
        assert!(close(cfg.disengage_range, def.behavior.disengage * 1.15),
            "disengage follows detection");
        assert!(close(cfg.standoff_range, def.behavior.standoff * 1.15),
            "standoff follows attack_range");
        assert!(close(cfg.health.as_f32(), def.stats.hp * 3.0));
        assert!(close(cfg.shield.expect("tanks are shielded").as_f32(),
            def.behavior.shield * 3.0), "shield follows hp");
    }

    #[test]
    fn scaling_defaults_must_cover_every_stat() {
        let doctored = ENEMIES_TOML.replace("detection = \"flat\"\n", "");
        let err = load_from(&doctored, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, PLANET_TOMLS)
            .unwrap_err();
        assert!(err.contains("no 'detection' curve"), "{err}");
    }

    #[test]
    fn latch_and_bolt_switches_default_by_archetype() {
        let roster = loaded();
        let swarmer = roster.enemy(roster.enemy_by_key("quad_orb").unwrap());
        assert_eq!(swarmer.behavior.latch_range, 2.0, "swarmers latch at 2 m");
        assert_eq!(swarmer.behavior.slow_factor, 0.7, "each tag compounds 0.7");
        assert_eq!(swarmer.behavior.slow_duration, 2.0);
        assert_eq!(swarmer.behavior.slow_interval, 0.5);
        let shooter = roster.enemy(roster.enemy_by_key("sphere_gunner").unwrap());
        assert_eq!(shooter.behavior.bolt_speed, 13.0, "bolts fly 13 m/s by default");
        assert_eq!(shooter.behavior.latch_range, 0.0, "non-swarmers never latch");
        assert_eq!(shooter.behavior.slow_factor, 1.0, "1.0 = no slow per tag");
        assert_eq!(shooter.behavior.slow_duration, 0.0);
        assert_eq!(shooter.behavior.slow_interval, 0.0);
    }

    #[test]
    fn a_declared_latch_switch_beats_its_archetype_default() {
        let doctored = ENEMIES_TOML.replace(
            "drain_dps = 6.0",
            "drain_dps = 6.0\nlatch_range = 4.5\nbolt_speed = 20.0",
        );
        let roster = load_from(&doctored, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, PLANET_TOMLS).unwrap();
        let latcher = roster.enemy(roster.enemy_by_key("boss_latcher").unwrap());
        assert_eq!(latcher.behavior.latch_range, 4.5, "the declared reach wins");
        assert_eq!(latcher.behavior.bolt_speed, 20.0, "declared even when unused");
    }

    #[test]
    fn a_declared_miniboss_switch_marks_the_room_sealer() {
        // The miniboss grammar (design 2026-07-06): any enemy may declare
        // `miniboss = true` — while it lives, its room's exits seal red
        // and the boss bed plays; death re-opens them. Off by default for
        // every archetype: a miniboss is a declared switch, not an engine
        // concept.
        let roster = loaded();
        for id in roster.enemy_ids() {
            assert!(
                !roster.enemy(id).behavior.miniboss,
                "no shipped enemy declares the miniboss switch yet: {}",
                roster.enemy(id).key
            );
        }
        // Doctor the FIRST enemy block, whatever def it is — never a named
        // anchor (feedback 2026-07-06).
        let doctored = ENEMIES_TOML.replacen("ai = ", "miniboss = true\nai = ", 1);
        let roster =
            load_from(&doctored, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, PLANET_TOMLS)
                .unwrap();
        assert!(
            roster.enemy_ids().any(|id| roster.enemy(id).behavior.miniboss),
            "the declared switch reads back"
        );
    }

    #[test]
    fn a_negative_latch_switch_is_a_link_error() {
        let doctored = ENEMIES_TOML.replace(
            "drain_dps = 6.0",
            "drain_dps = 6.0\nslow_factor = -0.5",
        );
        let err = load_from(&doctored, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, PLANET_TOMLS).unwrap_err();
        assert!(err.contains("slow_factor must be finite"), "{err}");
    }

    #[test]
    fn a_slot_boss_may_field_timed_emitters_but_never_broods() {
        // Sustained pressure is the def's own character (owner 2026-07-05:
        // the Brute fields a spawn ring); one-shot broods stay forbidden —
        // they'd double-dip against the slot's declared escorts.
        let emitter = ENEMIES_TOML.replace(
            "spawns_directly = false # staged by boss slots, never rolled into rooms\nminions = [{ enemy = \"spawn_drone\", count = 1, trigger = { every_seconds = 5.0 }, cap = 3 }]",
            "spawns_directly = false\nminions = [{ enemy = \"spawn_drone\", count = 1, trigger = { every_seconds = 5.0 }, cap = 3 }]",
        );
        assert!(
            load_from(&emitter, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, PLANET_TOMLS).is_ok(),
            "a timed emitter on a slot-staged boss must link"
        );
        let brood = ENEMIES_TOML.replace(
            "trigger = { every_seconds = 5.0 }, cap = 3",
            "trigger = \"on_death\"",
        );
        let err = load_from(&brood, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, PLANET_TOMLS)
            .unwrap_err();
        assert!(
            err.contains("one-shot brood"),
            "a brood on a slot-staged boss must be a link error naming the rule: {err}"
        );
    }

    #[test]
    fn a_zero_escort_count_is_a_link_error() {
        let doctored = PLANET_TOMLS[0].replace(
            "trigger = \"on_death\", count = 3",
            "trigger = \"on_death\", count = 0",
        );
        let planets = [doctored.as_str(), PLANET_TOMLS[1]];
        let err = load_from(ENEMIES_TOML, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, &planets).unwrap_err();
        assert!(
            err.contains("escort count must be at least 1"),
            "the linker must name the zero-count rule: {err}"
        );
    }

    #[test]
    #[should_panic(expected = "crossing id 9999 is not in rosters/enemies.toml")]
    fn an_undeclared_crossing_id_dies_at_the_demand_door_naming_the_culprit() {
        loaded().expect_enemy_by_crossing_id(9999);
    }

    #[test]
    fn every_declared_crossing_id_resolves_at_the_demand_door() {
        let roster = loaded();
        for def in &roster.enemies {
            assert_eq!(
                roster.expect_enemy_by_crossing_id(def.crossing_id),
                roster.enemy_by_crossing_id(def.crossing_id).unwrap(),
                "'{}' must resolve identically through both doors",
                def.key
            );
        }
    }

    #[test]
    fn every_kit_install_dir_is_populated() {
        // `make assets` must populate what kits.toml claims (owner
        // 2026-07-05) — run it first on a fresh checkout. Stage 3's probe
        // deepens this from "directory has content" to measured dimensions.
        let repo = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        for kit in &loaded().kits {
            let dir = format!("{repo}/{}", kit.install_dir);
            let entries = std::fs::read_dir(&dir).unwrap_or_else(|e| {
                panic!(
                    "kit '{}': install dir {} unreadable ({e}) — run `make assets`",
                    kit.key, kit.install_dir
                )
            });
            assert!(
                entries.count() > 0,
                "kit '{}': install dir {} is EMPTY — the pipeline did not \
                 populate it",
                kit.key,
                kit.install_dir
            );
        }
    }

    // ── Grammar enforcement: what serde/toml reject natively, and what the
    //    linker adds on top. ─────────────────────────────────────────────

    #[test]
    #[ignore = "writes rosters/VOCABULARY.md — `make build` runs it"]
    fn regenerate_vocabulary_reference() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../rosters/VOCABULARY.md");
        std::fs::write(path, super::vocabulary::vocabulary())
            .expect("write rosters/VOCABULARY.md");
    }

    #[test]
    fn a_redeclared_key_is_rejected_by_the_toml_crate() {
        // Free enforcement (TOML spec): no need to write this ourselves.
        let doubled = "value = 1\nvalue = 2\n";
        assert!(toml::from_str::<toml::Value>(doubled).is_err());
    }

    #[test]
    fn an_unknown_field_is_a_parse_error() {
        let doctored = ENEMIES_TOML.replace("yaw_offset_deg", "yaw_offest_deg");
        let err = load_from(&doctored, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, PLANET_TOMLS).unwrap_err();
        assert!(err.contains("yaw_offest_deg"), "typo'd field must be named: {err}");
    }

    #[test]
    fn an_unresolved_reference_is_a_link_error() {
        let doctored = ENEMIES_TOML.replace(
            "minions = [{ enemy = \"spawn_drone\", count = 1, trigger = \"on_death\" }]",
            "minions = [{ enemy = \"spwan_drone\", count = 1, trigger = \"on_death\" }]",
        );
        let err = load_from(&doctored, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, PLANET_TOMLS).unwrap_err();
        assert!(err.contains("spwan_drone"), "bad ref must be named: {err}");
    }

    #[test]
    fn a_duplicated_crossing_id_is_a_link_error() {
        let doctored = ENEMIES_TOML.replace("id = 10", "id = 9");
        let err = load_from(&doctored, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, PLANET_TOMLS).unwrap_err();
        assert!(err.contains("duplicate enemy id 9"), "{err}");
    }

    #[test]
    fn a_declared_length_without_a_roster_is_a_link_error() {
        // Owner 2026-07-05: "if I ask for 7 levels but only populate 6 …
        // that's a problem" — both directions.
        let p2 = PLANET_TOMLS[1].replace("levels = 6", "levels = 7");
        let err = load_from(ENEMIES_TOML, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, &[PLANET_TOMLS[0], &p2]).unwrap_err();
        assert!(
            err.contains("relative 7 has no"),
            "the unpopulated level must be named: {err}"
        );
    }

    #[test]
    fn a_roster_outside_the_declared_length_is_a_link_error() {
        let p2 = PLANET_TOMLS[1].replacen("relative = 6", "relative = 8", 1);
        let err = load_from(ENEMIES_TOML, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, &[PLANET_TOMLS[0], &p2]).unwrap_err();
        assert!(err.contains("outside the declared"), "{err}");
        assert!(
            err.contains("relative 6 has no"),
            "the hole it left must be named too: {err}"
        );
    }

    #[test]
    fn a_level_listing_a_non_spawning_enemy_is_a_link_error() {
        // spawn_drone is death-spawned only — a level roster naming it lies.
        let p1 = PLANET_TOMLS[0].replacen(
            "enemies = [\"sentry_drone\"]",
            "enemies = [\"spawn_drone\"]",
            1,
        );
        let err = load_from(ENEMIES_TOML, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, &[&p1, PLANET_TOMLS[1]]).unwrap_err();
        assert!(err.contains("never spawns directly"), "{err}");
    }
}
