//! The roster grammar (B13): enemies, swarms, curves, kits, and the planet
//! cascade, loaded from `rosters/*.toml` and linked into a typed IR.
//!
//! Strings exist only at the serde boundary ([`schema`]). The linker resolves
//! every cross-reference into a typed id ([`EnemyId`], [`SwarmId`], [`KitId`],
//! [`CurveId`]) and collects ALL violations — unresolved references, duplicate
//! keys/ids, planet gaps, boss-slot collisions — into one error. The runtime
//! model this module exposes contains no reference strings.
//!
//! Stage 1 (foundational): the TOML transcribes the Rust tables and the
//! golden-equivalence tests below hold them identical. Stage 2 swaps
//! consumers to this door and deletes the enum tables.

pub mod schema;
pub mod vocabulary;

use schema::{
    BossRewardPolicy, CurveAnchor, CurveKind, EnemiesFile, KitParadigm, KitsFile, PlanetFile,
    ScalingRaw,
};

use crate::enemy_ai::Archetype;
use crate::level_assembly::MinionTrigger;

const ENEMIES_TOML: &str = include_str!("../../../../rosters/enemies.toml");
const KITS_TOML: &str = include_str!("../../../../rosters/kits.toml");
const PLANET_TOMLS: &[&str] = &[
    include_str!("../../../../rosters/planets/planet_1.toml"),
    include_str!("../../../../rosters/planets/planet_2.toml"),
];

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
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scaling {
    pub speed: CurveId,
    pub cooldown: CurveId,
    pub hp: CurveId,
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
    /// Pre-staged bound minions: (kind, count, when they rise).
    pub minions: Vec<(EnemyId, u8, MinionTrigger)>,
    pub stats: Stats,
    pub scaling: Scaling,
    pub behavior: Behavior,
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
    pub escorts: (EnemyId, MinionTrigger),
    pub track: u8,
    pub reward: BossRewardPolicy,
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
    load_from(ENEMIES_TOML, KITS_TOML, PLANET_TOMLS)
}

/// The one shared instance (parsed on first use; a bad grammar panics with
/// the full violation list — the parse pin fails long before a run does).
pub fn roster() -> &'static Roster {
    static ROSTER: std::sync::OnceLock<Roster> = std::sync::OnceLock::new();
    ROSTER.get_or_init(|| load().expect("rosters/ must parse and link"))
}

fn load_from(enemies: &str, kits: &str, planets: &[&str]) -> Result<Roster, String> {
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

    let (Some(enemies_file), Some(kits_file)) = (enemies_file, kits_file) else {
        return Err(errors.join("\n"));
    };

    link(enemies_file, kits_file, planet_files, errors)
}

fn link(
    enemies_file: EnemiesFile,
    kits_file: KitsFile,
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
        let none = ScalingRaw { speed: None, cooldown: None, hp: None };
        let own = raw.unwrap_or(&none);
        Scaling {
            speed: pick("speed", &own.speed, &defaults.speed, errors),
            cooldown: pick("cooldown", &own.cooldown, &defaults.cooldown, errors),
            hp: pick("hp", &own.hp, &defaults.hp, errors),
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
                }
            };
            EnemyDef {
                key: e.key.clone(),
                crossing_id: e.id,
                name: e.name.clone(),
                model: e.model.clone(),
                size: e.size,
                yaw_offset_deg: e.yaw_offset_deg,
                ai: e.ai,
                reward: e.reward,
                spawns_directly: e.spawns_directly,
                minions: e
                    .minions
                    .iter()
                    .map(|m| {
                        (enemy_by_key(&m.enemy, &mut errors, &at), m.count, m.trigger)
                    })
                    .collect(),
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

    // Kits (BTreeMap: TOML rejects re-declared kit tables).
    let kits: Vec<KitDef> = kits_file
        .kits
        .iter()
        .map(|(key, raw)| KitDef {
            key: key.clone(),
            paradigm: raw.paradigm,
            tile: raw.tile,
            story: raw.story,
            install_dir: raw.install_dir.clone(),
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
            if !raw_enemies[boss_id.0].minions.is_empty() {
                errors.push(format!(
                    "{at}: boss '{}' declares def-level minions — a staged \
                     boss's minions are the slot's escorts (one door)",
                    slot.boss
                ));
            }
            let escort_id = enemy_by_key(&slot.escorts.enemy, &mut errors, &at);
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
                escorts: (escort_id, slot.escorts.trigger),
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

// ── Grammar tests: parse/link/validation, generated-reference currency,
//    kit-install pins, and (in property_tests) the generative sweep. The
//    golden-equivalence suite retired WITH the enum tables it mirrored —
//    the game reads this grammar now; behavior pins live with consumers. ──

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
        let err = load_from(&doctored, KITS_TOML, PLANET_TOMLS).unwrap_err();
        assert!(err.contains("drain_dps must be finite"), "{err}");
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
    fn the_committed_vocabulary_reference_is_current() {
        // The closed vocabularies self-document: enum → renderer (exhaustive
        // match, so a new variant refuses to compile undescribed) → this
        // golden file. Regenerate with `make roster-vocab`.
        assert_eq!(
            include_str!("../../../../rosters/VOCABULARY.md"),
            super::vocabulary::vocabulary(),
            "rosters/VOCABULARY.md is stale — run `make roster-vocab`"
        );
    }

    #[test]
    #[ignore = "writes rosters/VOCABULARY.md — run via make roster-vocab"]
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
        let err = load_from(&doctored, KITS_TOML, PLANET_TOMLS).unwrap_err();
        assert!(err.contains("yaw_offest_deg"), "typo'd field must be named: {err}");
    }

    #[test]
    fn an_unresolved_reference_is_a_link_error() {
        let doctored = ENEMIES_TOML.replace(
            "minions = [{ enemy = \"spawn_drone\", count = 1, trigger = \"on_death\" }]",
            "minions = [{ enemy = \"spwan_drone\", count = 1, trigger = \"on_death\" }]",
        );
        let err = load_from(&doctored, KITS_TOML, PLANET_TOMLS).unwrap_err();
        assert!(err.contains("spwan_drone"), "bad ref must be named: {err}");
    }

    #[test]
    fn a_duplicated_crossing_id_is_a_link_error() {
        let doctored = ENEMIES_TOML.replace("id = 10", "id = 9");
        let err = load_from(&doctored, KITS_TOML, PLANET_TOMLS).unwrap_err();
        assert!(err.contains("duplicate enemy id 9"), "{err}");
    }

    #[test]
    fn a_declared_length_without_a_roster_is_a_link_error() {
        // Owner 2026-07-05: "if I ask for 7 levels but only populate 6 …
        // that's a problem" — both directions.
        let p2 = PLANET_TOMLS[1].replace("levels = 6", "levels = 7");
        let err = load_from(ENEMIES_TOML, KITS_TOML, &[PLANET_TOMLS[0], &p2]).unwrap_err();
        assert!(
            err.contains("relative 7 has no"),
            "the unpopulated level must be named: {err}"
        );
    }

    #[test]
    fn a_roster_outside_the_declared_length_is_a_link_error() {
        let p2 = PLANET_TOMLS[1].replacen("relative = 6", "relative = 8", 1);
        let err = load_from(ENEMIES_TOML, KITS_TOML, &[PLANET_TOMLS[0], &p2]).unwrap_err();
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
        let err = load_from(ENEMIES_TOML, KITS_TOML, &[&p1, PLANET_TOMLS[1]]).unwrap_err();
        assert!(err.contains("never spawns directly"), "{err}");
    }
}
