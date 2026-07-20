//! The roster grammar (B13): enemies, swarms, curves, kits, and the planet
//! cascade, loaded from `rosters/*.toml` and linked into a typed IR.
//!
//! Strings exist only at the boundaries — the TOML serde ([`schema`]) and the
//! Godot/save crossing. The linker resolves every cross-reference into a typed
//! identity ([`EnemyKey`], [`SwarmId`], [`KitId`], [`CurveId`]) and collects
//! ALL violations — unresolved references, duplicate keys, planet gaps,
//! boss-slot collisions — into one error. The runtime
//! model this module exposes contains no reference strings.
//!
//! The TOML is THE game data — hand-authored, machinery never writes it.
//! The generated artifacts beside it (`TEMPLATE.toml`, `VOCABULARY.md`)
//! are the opposite: never hand-edited, re-rendered by `make build`.

pub mod schema;

/// The minimum census a PANEL kit links with (append after tile/story):
/// a baked filler variant in each surface role — pools derive from
/// variants alone, and a kit without pooled variants is a link error by
/// design (v1 retired 2026-07-19). Inline table so header-renaming
/// doctor tests carry it along.
#[cfg(test)]
pub(crate) fn test_census_variants(tile: f32) -> String {
    let v = |stem: &str, role: &str, axis: &str, detail: &str| {
        format!(
            "{stem} = {{ sources = [\"plate\"], role = \"{role}\", \
             face = [{tile:.1}, {tile:.1}], thick = 0.2, axis = \"{axis}\", \
             detail = \"{detail}\", coverage = 1.0, stretch = 0.0, \
             tris = 100, textures = 3 }}"
        )
    };
    format!(
        "variants = {{ {}, {}, {} }}\n",
        v("plate_floor", "floor", "y", "pos_y"),
        v("plate_ceiling", "ceiling", "y", "neg_y"),
        v("plate_wall", "wall", "z", "pos_z"),
    )
}
pub mod template;
pub mod vocabulary;

use crate::asset_catalog::KitKind;
use schema::{
    BossRewardPolicy, CurveAnchor, CurveKind, EnemiesFile, EnvironmentFile,
    PlanetFile,
    ScalingRaw,
};

use crate::enemy_ai::Archetype;
use crate::level_assembly::MinionTrigger;

const ENEMIES_TOML: &str = include_str!("../../../../rosters/enemies.toml");
// PLANET_TOMLS / ENVIRONMENT_TOMLS: every rosters/planets/*.toml and
// rosters/environments/*.toml, embedded by build.rs — a new file wires
// itself in by existing.
include!(concat!(env!("OUT_DIR"), "/planet_tomls.rs"));

// ── Typed ids: arena indices, no reference strings past the linker ──────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwarmId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurveId(pub usize);

// Kit identity/defs live in the CATALOG (Amendment A); re-exported so
// planet defs and consumers keep one obvious path.
pub use crate::asset_catalog::{KitDef, KitId};

// ── The one enemy identity ───────────────────────────────────────────────

/// The single enemy identity: the key string from `enemies.toml`, interned to
/// a `'static` handle. Strongly typed — an `EnemyKey` exists only for a key
/// some grammar has declared (built at a boundary via [`Roster::enemy_key`]),
/// so anywhere it appears in Rust it is proof the enemy exists. Strings live
/// ONLY at the two boundaries — the TOML link and the Godot/save crossing;
/// this is the identity everywhere else. Interned, so it is `Copy` and
/// independent of which `Roster` produced it (the shipped singleton or a
/// test fixture).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnemyKey(&'static str);

impl EnemyKey {
    /// The underlying key string — for the boundaries ONLY (the Godot
    /// crossing, save serialization). Internal code passes the `EnemyKey`.
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

// Serde crosses the ONE save boundary as the key string — never a number.
// Deserialize interns; a key no live grammar declares still round-trips (an
// old save's retired enemy), and display filters it against the roster.
impl serde::Serialize for EnemyKey {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for EnemyKey {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(EnemyKey(intern_key(&s)))
    }
}

/// Intern a key/path string to a `'static` — the handful of enemy keys and
/// environment scene paths are permanent, so a leak is the correct
/// lifetime. One canonical pointer per distinct string, shared across every
/// `Roster` (singleton and fixtures).
fn intern_key(s: &str) -> &'static str {
    use std::sync::{Mutex, OnceLock};
    static POOL: OnceLock<Mutex<std::collections::HashSet<&'static str>>> = OnceLock::new();
    let pool = POOL.get_or_init(|| Mutex::new(std::collections::HashSet::new()));
    let mut pool = pool.lock().expect("key interner poisoned");
    if let Some(&existing) = pool.get(s) {
        return existing;
    }
    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
    pool.insert(leaked);
    leaked
}

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
    /// Detonation cloud (seconds): the blast leaves an occluding dust
    /// cloud at blast radius for this long. 0 = instant blast only —
    /// the cloud attacks VISION, not hull.
    pub cloud_seconds: f32,
    pub shield: f32,
    pub disengage: f32,
    pub drain_dps: f32,
    /// Projectile speed (m/s) for firing archetypes.
    pub bolt_speed: f32,
    /// Bolts per trigger pull (1 = single shot); the cooldown gates BURSTS.
    pub burst_count: u8,
    /// In-burst gap (s) between a burst's bolts.
    pub burst_seconds: f32,
    /// Pellets fanned per bolt (1 = no fan; a shotgun declares more).
    pub pellet_count: u8,
    /// Total fan cone (degrees) the pellets spread across.
    pub spread_deg: f32,
    /// Max steering rate (deg/s) for homing bolts; 0 = ballistic.
    pub bolt_turn_deg: f32,
    /// Tractor/repulsor field (SIGNED, m/s²): positive drags the player
    /// toward this enemy, negative shoves away; linear falloff to zero at
    /// attack_range (void_logic::tractor). 0 = no field.
    pub pull_accel: f32,
    /// Alarm aura (metres): while this enemy is engaged, machines within
    /// the radius of IT force-engage the player. 0 = no klaxon.
    pub alert_radius: f32,
    /// Guardian link (metres): damage to machines within the radius drinks
    /// into this enemy's shield first; overflow stays with the victim.
    /// 0 = guards nothing.
    pub guard_radius: f32,
    /// Erratic dodge (seconds): mean time between strafe re-rolls while
    /// engaged. 0 = the smooth orbit.
    pub jink_seconds: f32,
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
            cloud_seconds: 0.0,
            shield: 0.0,
            disengage: 0.0,
            drain_dps: 0.0,
            bolt_speed: 0.0,
            burst_count: 1,
            burst_seconds: 0.0,
            pellet_count: 1,
            spread_deg: 0.0,
            bolt_turn_deg: 0.0,
            pull_accel: 0.0,
            alert_radius: 0.0,
            guard_radius: 0.0,
            jink_seconds: 0.0,
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
    pub enemy: EnemyKey,
    pub count: u8,
    pub trigger: MinionTrigger,
    pub cap: u8,
}

#[derive(Debug, Clone)]
pub struct EnemyDef {
    pub key: String,
    pub name: String,
    /// The installed model scene, as the catalog minted it at link —
    /// proof the authored model KEY resolved. The shell turns it back
    /// into a path at its one documented crossing.
    pub model: crate::asset_catalog::SceneId,
    pub size: f32,
    pub yaw_offset_deg: f32,
    /// Barrel tips in the model's aim frame (x right, y up, z toward the
    /// player; horizontal, matching the yaw-only billboard), metres at the
    /// def's size. The fire site round-robins them shot by shot; empty =
    /// fire from the hull centre plus clearance (the legacy convention).
    pub muzzles: Vec<[f32; 3]>,
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
    pub fn stats_at(&self, key: EnemyKey, level: u32) -> Stats {
        let def = self.enemy(key);
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
    pub fn ai_config_at(&self, key: EnemyKey, level: u32) -> crate::enemy_ai::DroneConfig {
        let def = self.enemy(key);
        let stats = self.stats_at(key, level);
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
            burst_count: def.behavior.burst_count,
            burst_seconds: def.behavior.burst_seconds,
            jink_seconds: def.behavior.jink_seconds,
            // Determinism seed: the SHELL stamps its instance id after
            // construction — the grammar has no per-instance identity.
            seed: 0,
            standoff_range: def.behavior.standoff * attack_mul,
            fuse_seconds: def.behavior.fuse_seconds,
            blast_radius: def.behavior.blast_radius * attack_mul,
            shield: (def.behavior.shield > 0.0)
                .then(|| crate::newtypes::Shield::new(def.behavior.shield * hp_mul)),
        }
    }

}

#[derive(Debug, Clone)]
pub struct SwarmDef {
    pub key: String,
    pub members: Vec<(EnemyKey, u8)>,
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


/// A coarse authored volume over a fixed environment's geometry — the
/// LevelGraph node, spawn group, and culling unit of a fixed level.
/// Coordinates are model-space: integer meter boxes, float spawn points;
/// the kit's scale is the only bridge to world units.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneDef {
    pub key: String,
    /// Box min corner, meters (min-inclusive).
    pub min: [i32; 3],
    /// Box extents, meters (min + extents exclusive).
    pub extents: [u32; 3],
    /// Adjacent zones (indices into the environment's zone list),
    /// undirected and deduplicated.
    pub links: Vec<usize>,
    pub enemy_spawns: Vec<[f32; 3]>,
    pub loot_spawns: Vec<[f32; 3]>,
    pub start: bool,
    pub boss: bool,
}

/// Authored sunlight for a fixed environment (validated degrees/energy).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunDef {
    /// Compass bearing, degrees: 0 = model north (-z), 90 = east (+x).
    pub azimuth_deg: f32,
    /// Degrees above the horizon.
    pub elevation_deg: f32,
    pub energy: f32,
}

/// One authored window panel, linked: the opening as a light source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowDef {
    /// Panel center on the opening's plane, model meters.
    pub center: [f32; 3],
    /// Panel width and height, model meters.
    pub size: [f32; 2],
    /// Unit inward direction the light shines along.
    pub inward: [f32; 3],
    pub energy: f32,
    /// Throw distance into the room, model meters.
    pub range: f32,
}

/// The bounce stand-in: the ambient term approximating the window light's
/// missing GI (validated energy; linear RGB color).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmbientDef {
    pub energy: f32,
    pub color: [f32; 3],
}

/// A linked fixed environment: the installed scene plus its authored zones.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvironmentDef {
    pub key: String,
    /// Sunlight through the window openings, when the file authors one.
    pub sun: Option<SunDef>,
    /// The window openings as panel light sources (daylight comes from
    /// outside; these are the diffusers).
    pub windows: Vec<WindowDef>,
    /// The world-environment ambient override while this level runs.
    pub ambient: Option<AmbientDef>,
    /// The installed scene, as the catalog minted it at link (resolved
    /// from environments.generated.toml — consumers never see keys, and
    /// placements carry the id unchanged).
    pub model: crate::asset_catalog::SceneId,
    pub zones: Vec<ZoneDef>,
    /// Index of the `start = true` zone (linker-enforced: exactly one).
    pub start_zone: usize,
    /// Index of the `boss = true` zone (linker-enforced: exactly one,
    /// farthest-by-graph from the start).
    pub boss_zone: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnRef {
    Enemy(EnemyKey),
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
    pub boss: EnemyKey,
    pub escorts: EscortDef,
    pub track: u8,
    pub reward: BossRewardPolicy,
}

/// The escort declaration on a boss slot: the boss's minions are the slot's
/// call, count included — there is no formula behind it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EscortDef {
    pub enemy: EnemyKey,
    pub trigger: MinionTrigger,
    pub count: u8,
}

#[derive(Debug)]
pub struct Roster {
    pub enemies: Vec<EnemyDef>,
    pub swarms: Vec<SwarmDef>,
    pub curves: Vec<(String, CurveDef)>,
    /// The linked asset catalog this grammar resolved against — kits,
    /// models, environment scenes. Shared (Arc): the production roster
    /// holds the process catalog; fixtures hold their own.
    pub catalog: std::sync::Arc<crate::asset_catalog::AssetCatalog>,
    /// Fixed-kit environment KEY → index into `environments` (the
    /// authored zone maps): the one cross-boundary join, resolved and
    /// validated at link.
    kit_environments: std::collections::BTreeMap<String, usize>,
    /// Sorted by planet number, contiguous from 1.
    pub planets: Vec<PlanetDef>,
    /// Fixed-level environments, in embedding order; fixed kits index in.
    pub environments: Vec<EnvironmentDef>,
}

impl Roster {
    /// The one enemy accessor: infallible, because an [`EnemyKey`] is proof
    /// the enemy exists. Panics only on misuse — a key from a DIFFERENT
    /// grammar than `self` — the single demand door.
    pub fn enemy(&self, key: EnemyKey) -> &EnemyDef {
        self.enemies
            .iter()
            .find(|e| e.key == key.0)
            .unwrap_or_else(|| panic!("EnemyKey '{}' is not in this grammar", key.0))
    }

    /// The boundary parser: a raw key string (from the TOML link or the
    /// Godot/save crossing) to the typed identity. `None` for a key this
    /// grammar does not declare — a retired save entry, a typo caught at
    /// link. This is the ONLY function that turns a string into an identity.
    pub fn enemy_key(&self, s: &str) -> Option<EnemyKey> {
        self.enemies
            .iter()
            .find(|e| e.key == s)
            .map(|e| EnemyKey(intern_key(&e.key)))
    }

    /// Every declared enemy as its key, in declaration order.
    pub fn enemy_keys(&self) -> impl Iterator<Item = EnemyKey> + '_ {
        self.enemies.iter().map(|e| EnemyKey(intern_key(&e.key)))
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

    /// A level's world-space quantization — the tile/story of its planet's
    /// declared kit (measured by the make-assets probe). Nobody authors a
    /// cell dimension anywhere else.
    pub fn pitch_for_level(&self, level: u32) -> crate::planet::Pitch {
        let def = self.planet_for_level(level.max(1));
        let kit = self.catalog.kit_def(def.kits[0]);
        crate::planet::Pitch { tile: kit.tile, story: kit.story }
    }

    /// Whether a level's planet builds from cubic panel cells (vs the layered
    /// megakit) — its declared kit's paradigm decides.
    pub fn panel_world(&self, level: u32) -> bool {
        let def = self.planet_for_level(level.max(1));
        matches!(self.catalog.kit_def(def.kits[0]).kind, KitKind::Panel(_))
    }

    /// The wall pools of a level's declared PANEL kits, in declaration
    /// order (empty for layered/fixed planets). IDS, not pools — the
    /// catalog owns the plates; consumers resolve per room at assembly.
    pub fn panel_kits_for_level(&self, level: u32) -> Vec<crate::asset_catalog::KitId> {
        let def = self.planet_for_level(level.max(1));
        def.kits
            .iter()
            .filter(|&&id| matches!(self.catalog.kit_def(id).kind, KitKind::Panel(_)))
            .copied()
            .collect()
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
    pub fn roster_for_level(&self, level: u32) -> Vec<EnemyKey> {
        let (def, relative, number) = self.locate(level);
        let beyond = number != def.planet;
        let list = if beyond {
            def.rosters.last().expect("linker guarantees coverage")
        } else {
            &def.rosters[(relative - 1) as usize]
        };
        let mut out: Vec<EnemyKey> = Vec::new();
        let push = |id: EnemyKey, out: &mut Vec<EnemyKey>| {
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

    /// The fixed environment a level builds, when its planet's declared kit
    /// is fixed-paradigm (planet 3's apartment). `None` = generated level.
    pub fn environment_for_level(&self, level: u32) -> Option<&EnvironmentDef> {
        let def = self.planet_for_level(level.max(1));
        let kit = self.catalog.kit_def(def.kits[0]);
        kit.environment()
            .and_then(|k| self.kit_environments.get(k))
            .map(|&i| &self.environments[i])
    }
}

// ── Load + link ──────────────────────────────────────────────────────────

/// Parse and link the embedded roster files. Every violation — parse errors,
/// unresolved references, duplicate keys/ids, planet gaps, slot collisions —
/// is collected; the Err carries them all.
pub fn load() -> Result<Roster, String> {
    load_from(
        ENEMIES_TOML,
        PLANET_TOMLS,
        EnvSources::shipped(),
        crate::asset_catalog::catalog().clone(),
    )
}

/// The environment grammar's source bundle: authored zone maps, the
/// generated environment catalog, and the generated window sets. One
/// argument because they link as one unit — the fixed paradigm's inputs.
pub(crate) struct EnvSources<'a> {
    pub authored: &'a [&'a str],
    pub windows: &'a [&'a str],
}

impl EnvSources<'static> {
    /// The shipped bundle (the embedded rosters/ files).
    pub(crate) fn shipped() -> Self {
        Self {
            authored: ENVIRONMENT_TOMLS,
            windows: WINDOW_TOMLS,
        }
    }

    /// No authored environments (grid-paradigm grammars); scene lookups
    /// resolve against the catalog the caller supplies.
    #[cfg(test)]
    pub(crate) fn generated_only() -> Self {
        Self { authored: &[], windows: &[] }
    }
}

/// TEST SEAM (planned with the identity unification, Phase 5): the GUT
/// harness swaps THE grammar for a test-owned fixture through the shell's
/// test door, so shell scenarios never depend on the owner's rosters/
/// tuning. Production never writes this; each install leaks one Roster —
/// a per-test-file cost, not a play-path one.
static OVERRIDE: std::sync::RwLock<Option<&'static Roster>> = std::sync::RwLock::new(None);

/// The one shared instance (parsed on first use; a bad grammar panics with
/// the full violation list — the parse pin fails long before a run does).
/// A test-installed override, when present, IS the grammar — every reader,
/// model and shell alike, resolves it until cleared.
pub fn roster() -> &'static Roster {
    static ROSTER: std::sync::OnceLock<Roster> = std::sync::OnceLock::new();
    if let Some(r) = *OVERRIDE.read().expect("grammar override lock") {
        return r;
    }
    ROSTER.get_or_init(|| load().expect("rosters/ must parse and link"))
}

/// Install a fixture grammar as THE grammar (see [`OVERRIDE`]). Links the
/// fixture against the REAL model catalog — fixture enemies wear installed
/// models, exactly like the template — so the shell can build them.
pub fn override_grammar_from(
    enemies: &str,
    kits: &str,
    kit_grids: &str,
    planets: &[&str],
    environments: &[&str],
) -> Result<(), String> {
    let catalog = std::sync::Arc::new(
        crate::asset_catalog::AssetCatalog::load(
            kits,
            kit_grids,
            crate::asset_catalog::MODELS_TOML,
            crate::asset_catalog::ENVIRONMENTS_GENERATED_TOML,
        )
        .map_err(|e| e.join("\n"))?,
    );
    let g = load_from(
        enemies,
        planets,
        EnvSources { authored: environments, windows: WINDOW_TOMLS },
        catalog,
    )?;
    *OVERRIDE.write().expect("grammar override lock") = Some(Box::leak(Box::new(g)));
    Ok(())
}

/// Drop the fixture: the next [`roster()`] read resolves the shipped grammar.
pub fn clear_grammar_override() {
    *OVERRIDE.write().expect("grammar override lock") = None;
}

pub(crate) fn load_from(
    enemies: &str,
    planets: &[&str],
    env: EnvSources,
    catalog: std::sync::Arc<crate::asset_catalog::AssetCatalog>,
) -> Result<Roster, String> {
    let EnvSources { authored: environments, windows } = env;
    let mut errors: Vec<String> = Vec::new();

    let enemies_file: Option<EnemiesFile> = match toml::from_str(enemies) {
        Ok(f) => Some(f),
        Err(e) => {
            errors.push(format!("enemies.toml: {e}"));
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
    let environment_files: Vec<EnvironmentFile> = environments
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match toml::from_str(s) {
            Ok(f) => Some(f),
            Err(e) => {
                errors.push(format!("environments file #{}: {e}", i + 1));
                None
            }
        })
        .collect();

    let windows_files: Vec<schema::WindowsFile> = windows
        .iter()
        .enumerate()
        .filter_map(|(i, s)| match toml::from_str(s) {
            Ok(f) => Some(f),
            Err(e) => {
                errors.push(format!("windows file #{}: {e}", i + 1));
                None
            }
        })
        .collect();

    let Some(enemies_file) = enemies_file else {
        return Err(errors.join("\n"));
    };

    link(
        enemies_file,
        planet_files,
        ParsedEnvFiles {
            authored: environment_files,
            windows: windows_files,
        },
        catalog,
        errors,
    )
}

/// The parsed counterpart of [`EnvSources`]: one linker argument.
struct ParsedEnvFiles {
    authored: Vec<EnvironmentFile>,
    windows: Vec<schema::WindowsFile>,
}

fn link(
    enemies_file: EnemiesFile,
    planet_files: Vec<PlanetFile>,
    env: ParsedEnvFiles,
    catalog: std::sync::Arc<crate::asset_catalog::AssetCatalog>,
    mut errors: Vec<String>,
) -> Result<Roster, String> {
    let ParsedEnvFiles {
        authored: environment_files,
        windows: windows_files,
    } = env;
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
    }
    // Link a key reference (minion, planet roster, boss slot) to the typed
    // identity. An unknown key is a link error; the returned handle is a
    // placeholder the failed load never surfaces.
    // The one link-boundary resolver: a key reference to its typed identity,
    // recording an "unknown enemy" error if the grammar doesn't declare it.
    // Link-time property checks below read the raw def with an inline
    // `raw_enemies.iter().any(|e| e.key == K && …)` — no second accessor.
    let enemy_by_key = |key: &str, errors: &mut Vec<String>, at: &str| -> EnemyKey {
        if !raw_enemies.iter().any(|e| e.key == key) {
            errors.push(format!("{at}: unknown enemy '{key}'"));
        }
        EnemyKey(intern_key(key))
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
        .filter_map(|e| {
            let at = format!("enemy '{}'", e.key);
            for (field, value) in [
                ("standoff_frac", e.standoff_frac),
                ("fuse_seconds", e.fuse_seconds),
                ("blast_frac", e.blast_frac),
                ("cloud_seconds", e.cloud_seconds),
                ("shield_frac", e.shield_frac),
                ("disengage_frac", e.disengage_frac),
                ("drain_dps", e.drain_dps),
                ("bolt_speed", e.bolt_speed),
                ("burst_seconds", e.burst_seconds),
                ("spread_deg", e.spread_deg),
                ("bolt_turn_deg", e.bolt_turn_deg),
                ("alert_radius", e.alert_radius),
                ("guard_radius", e.guard_radius),
                ("jink_seconds", e.jink_seconds),
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
            // Counts say how MANY bolts, never whether the enemy fires —
            // that is the archetype's call. Zero is a config bug.
            for (field, value) in [
                ("burst_count", e.burst_count),
                ("pellet_count", e.pellet_count),
            ] {
                if value == Some(0) {
                    errors.push(format!("{at}: {field} must be ≥ 1"));
                }
            }
            // Muzzles are aim-frame offsets, so components are signed; a
            // tip reaching past the def's size is a unit/frame mistake.
            if let Some(ms) = &e.muzzles {
                for (i, m) in ms.iter().enumerate() {
                    let reach = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt();
                    if !reach.is_finite() {
                        errors.push(format!("{at}: muzzle {i} must be finite"));
                    } else if reach > e.size {
                        errors.push(format!(
                            "{at}: muzzle {i} reaches {reach:.2} m — beyond the \
                             def's size ({} m); offsets are metres in the aim frame",
                            e.size
                        ));
                    }
                }
            }
            // pull_accel is SIGNED — a repulsor is legal config — so it
            // skips the ≥ 0 loop; only non-finite dies.
            if let Some(v) = e.pull_accel {
                if !v.is_finite() {
                    errors.push(format!("{at}: pull_accel must be finite, got {v}"));
                }
            }
            if e.pellet_count.unwrap_or(1) > 1 && e.bolt_turn_deg.unwrap_or(0.0) > 0.0 {
                errors.push(format!(
                    "{at}: pellets fly ballistic — declare pellet_count or \
                     bolt_turn_deg, not both"
                ));
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
                    cloud_seconds: e.cloud_seconds.unwrap_or(0.0),
                    shield: e.shield_frac.unwrap_or(if e.ai == Tank { 0.5 } else { 0.0 })
                        * e.stats.hp,
                    disengage: e.disengage_frac.unwrap_or(1.2) * e.stats.detection,
                    drain_dps: e.drain_dps.unwrap_or(0.0),
                    bolt_speed: e.bolt_speed.unwrap_or(13.0),
                    burst_count: e.burst_count.unwrap_or(1),
                    burst_seconds: e.burst_seconds.unwrap_or(0.1),
                    pellet_count: e.pellet_count.unwrap_or(1),
                    spread_deg: e.spread_deg.unwrap_or(0.0),
                    bolt_turn_deg: e.bolt_turn_deg.unwrap_or(0.0),
                    pull_accel: e.pull_accel.unwrap_or(0.0),
                    alert_radius: e.alert_radius.unwrap_or(0.0),
                    guard_radius: e.guard_radius.unwrap_or(0.0),
                    jink_seconds: e.jink_seconds.unwrap_or(0.0),
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
            Some(EnemyDef {
                key: e.key.clone(),
                name: e.name.clone(),
                size: e.size,
                yaw_offset_deg: e.yaw_offset_deg,
                muzzles: e.muzzles.clone().unwrap_or_default(),
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
                // The def declares a model KEY; the linked def carries the
                // catalog's minted id, so consumers never see keys or
                // paths. LAST field on purpose: the bail-out skips this
                // def only after every validation above pushed its errors.
                model: match catalog.model_scene_id(&e.model) {
                    Some(id) => id,
                    None => {
                        errors.push(format!(
                            "{at}: unknown model '{}' — not in \
                             catalog/models.generated.toml (run `make assets`)",
                            e.model
                        ));
                        return None;
                    }
                },
            })
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
                    if raw_enemies.iter().any(|e| e.key == m.enemy && !e.spawns_directly) {
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

    // Environments: authored zone maps joined to the installed-scene
    // catalog. Zone links resolve to indices (undirected at use).
    let environments: Vec<EnvironmentDef> = environment_files
        .iter()
        .filter_map(|f| {
            let at = format!("environment '{}'", f.environment.key);
            let zones: Vec<ZoneDef> = f
                .zones
                .iter()
                .map(|z| ZoneDef {
                    key: z.key.clone(),
                    min: z.bounds.min,
                    extents: z.bounds.extents,
                    links: z
                        .links
                        .iter()
                        .filter_map(|k| f.zones.iter().position(|o| o.key == *k))
                        .collect(),
                    enemy_spawns: z.enemy_spawns.clone(),
                    loot_spawns: z.loot_spawns.clone(),
                    start: z.start,
                    boss: z.boss,
                })
                .collect();
            let sun = f.sun.as_ref().map(|s| {
                if !(s.elevation_deg > 0.0 && s.elevation_deg <= 90.0) {
                    errors.push(format!(
                        "{at}: sun elevation_deg must be in (0, 90], got {}",
                        s.elevation_deg
                    ));
                }
                if !(s.energy.is_finite() && s.energy > 0.0) {
                    errors.push(format!(
                        "{at}: sun energy must be finite and > 0, got {}",
                        s.energy
                    ));
                }
                SunDef {
                    azimuth_deg: s.azimuth_deg,
                    elevation_deg: s.elevation_deg,
                    energy: s.energy,
                }
            });
            let mut windows: Vec<WindowDef> = f
                .windows
                .iter()
                .enumerate()
                .map(|(wi, w)| {
                    for (field, v) in [
                        ("energy", w.energy),
                        ("range", w.range),
                        ("width", w.size[0]),
                        ("height", w.size[1]),
                    ] {
                        if !(v.is_finite() && v > 0.0) {
                            errors.push(format!(
                                "{at}: window #{}: {field} must be finite and > 0, got {v}",
                                wi + 1
                            ));
                        }
                    }
                    WindowDef {
                        center: w.center,
                        size: w.size,
                        inward: w.facing.direction(),
                        energy: w.energy,
                        range: w.range,
                    }
                })
                .collect();
            // Join the GENERATED panes (rosters/windows/, derived from the
            // scene's glass): geometry from the model, energy from the
            // environment's authored per-m² knob — bigger panes pour more.
            for wf in windows_files.iter().filter(|wf| wf.environment == f.environment.key) {
                for w in &wf.windows {
                    let area = w.size[0] * w.size[1];
                    windows.push(WindowDef {
                        center: w.center,
                        size: w.size,
                        inward: w.facing.direction(),
                        energy: area * f.environment.window_energy_per_m2,
                        range: schema::default_window_range(),
                    });
                }
            }
            let ambient = f.ambient.as_ref().map(|a| {
                if !(a.energy.is_finite() && a.energy > 0.0) {
                    errors.push(format!(
                        "{at}: ambient energy must be finite and > 0, got {}",
                        a.energy
                    ));
                }
                AmbientDef { energy: a.energy, color: a.color }
            });
            Some(EnvironmentDef {
                key: f.environment.key.clone(),
                sun,
                windows,
                ambient,
                start_zone: zones.iter().position(|z| z.start).unwrap_or(0),
                boss_zone: zones.iter().position(|z| z.boss).unwrap_or(0),
                zones,
                // LAST field on purpose: an unknown model bails only
                // after the authored lighting/zone fields validated.
                model: match catalog.environment_scene_id(&f.environment.model) {
                    Some(id) => id,
                    None => {
                        errors.push(format!(
                            "{at}: unknown model '{}' — not in \
                             catalog/environments.generated.toml (run `make assets`)",
                            f.environment.model
                        ));
                        return None;
                    }
                },
            })
        })
        .collect();

    // Environment validation: identity, volumes, spawns, and the zone
    // graph — the SAME petgraph shape the runtime builds, validated at
    // link time so a bad authoring is a load failure, never a broken run.
    for (i, e) in environments.iter().enumerate() {
        if environments[..i].iter().any(|p| p.key == e.key) {
            errors.push(format!("duplicate environment key '{}'", e.key));
        }
    }
    for (f, env) in environment_files.iter().zip(&environments) {
        let at = format!("environment '{}'", env.key);
        for (i, z) in env.zones.iter().enumerate() {
            if env.zones[..i].iter().any(|p| p.key == z.key) {
                errors.push(format!("{at}: duplicate zone key '{}'", z.key));
            }
        }
        // Dangling links check the RAW side — resolution dropped them.
        for z in &f.zones {
            for k in &z.links {
                if !f.zones.iter().any(|o| o.key == *k) {
                    errors.push(format!(
                        "{at}: zone '{}' links to unknown zone '{k}'",
                        z.key
                    ));
                }
            }
        }
        let starts = env.zones.iter().filter(|z| z.start).count();
        if starts != 1 {
            errors.push(format!(
                "{at}: exactly one zone must declare start = true (found {starts})"
            ));
        }
        let bosses = env.zones.iter().filter(|z| z.boss).count();
        if bosses != 1 {
            errors.push(format!(
                "{at}: exactly one zone must declare boss = true (found {bosses})"
            ));
        }
        for z in &env.zones {
            let zat = format!("{at}: zone '{}'", z.key);
            if z.extents.contains(&0) {
                errors.push(format!("{zat}: box extents must all be at least 1"));
            }
            let inside = |p: &[f32; 3]| {
                (0..3).all(|i| {
                    p[i] >= z.min[i] as f32
                        && p[i] <= (z.min[i] + z.extents[i] as i32) as f32
                })
            };
            for p in z.enemy_spawns.iter().chain(&z.loot_spawns) {
                if !inside(p) {
                    errors.push(format!("{zat}: spawn {p:?} lies outside the zone box"));
                }
            }
            if z.start && !z.enemy_spawns.is_empty() {
                errors.push(format!(
                    "{zat}: the start zone declares enemy spawns — the start \
                     room stays clear (room-0 rule)"
                ));
            }
            if z.boss && z.enemy_spawns.len() != 1 {
                errors.push(format!(
                    "{zat}: the boss zone must author exactly one enemy spawn \
                     (the arena anchor the staged fight rises from), found {}",
                    z.enemy_spawns.len()
                ));
            }
        }
        // Integer AABBs, min-inclusive max-exclusive: shared faces touch,
        // shared volume is an authoring error.
        for (i, a) in env.zones.iter().enumerate() {
            for b in &env.zones[i + 1..] {
                let overlaps = (0..3).all(|k| {
                    a.min[k] < b.min[k] + b.extents[k] as i32
                        && b.min[k] < a.min[k] + a.extents[k] as i32
                });
                if overlaps {
                    errors.push(format!(
                        "{at}: zone boxes '{}' and '{}' overlap",
                        a.key, b.key
                    ));
                }
            }
        }
        // Linked zones must share a face (touching on one axis, positive
        // overlap on the other two): fixed_layout synthesizes the doorway
        // connector on that face, and a link without one has nowhere to go.
        for z in &env.zones {
            for &l in &z.links {
                let o = &env.zones[l];
                let touch_axis = (0..3).find(|&k| {
                    z.min[k] + z.extents[k] as i32 == o.min[k]
                        || o.min[k] + o.extents[k] as i32 == z.min[k]
                });
                let shares_face = touch_axis.is_some_and(|k| {
                    (0..3).filter(|&j| j != k).all(|j| {
                        z.min[j] < o.min[j] + o.extents[j] as i32
                            && o.min[j] < z.min[j] + z.extents[j] as i32
                    })
                });
                if !shares_face {
                    errors.push(format!(
                        "{at}: linked zones '{}' and '{}' share no face — a \
                         doorway connector has nowhere to go",
                        z.key, o.key
                    ));
                }
            }
        }
        // The zone graph: every zone reachable, and the boss zone farthest
        // from the start — portal placement and the arena ride the graph.
        if starts == 1 && bosses == 1 {
            let mut g: petgraph::graph::UnGraph<(), ()> = petgraph::graph::UnGraph::default();
            let nodes: Vec<_> = env.zones.iter().map(|_| g.add_node(())).collect();
            for (zi, z) in env.zones.iter().enumerate() {
                for &l in &z.links {
                    g.update_edge(nodes[zi], nodes[l], ());
                }
            }
            let dist = petgraph::algo::dijkstra(&g, nodes[env.start_zone], None, |_| 1u32);
            for (zi, z) in env.zones.iter().enumerate() {
                if !dist.contains_key(&nodes[zi]) {
                    errors.push(format!(
                        "{at}: zone '{}' is unreachable from the start zone",
                        z.key
                    ));
                }
            }
            if let Some(max) = dist.values().max().copied() {
                if dist.get(&nodes[env.boss_zone]) != Some(&max) {
                    errors.push(format!(
                        "{at}: the boss zone '{}' must be farthest from the \
                         start (portal and arena placement ride the graph)",
                        env.zones[env.boss_zone].key
                    ));
                }
            }
        }
    }

    // Kits live in the CATALOG (Amendment A): planets declare kit KEYS
    // and resolve them here — the one sanctioned join. An unresolved key
    // is a link error naming the planet.
    let kit_by_key = |key: &str, errors: &mut Vec<String>, at: &str| -> Option<KitId> {
        match catalog.kit(key) {
            Some((id, _)) => Some(id),
            None => {
                errors.push(format!("{at}: unknown kit '{key}'"));
                None
            }
        }
    };

    // Planets: contiguous from 1, one file per planet, fleets in-band.
    let mut planets: Vec<PlanetDef> = Vec::new();
    let mut kit_environments: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
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
                if raw_enemies.iter().any(|e| e.key == *key && !e.spawns_directly) {
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
            if raw_enemies.iter().any(|e| {
                e.key == slot.boss
                    && e.minions
                        .iter()
                        .any(|m| matches!(m.trigger, schema::TriggerRaw::Named(_)))
            }) {
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
            if raw_enemies.iter().any(|e| e.key == slot.escorts.enemy && !e.minions.is_empty()) {
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

        // Room growth: a GENERATED planet declares it; a FIXED planet's
        // room count is its environment's authored zone count — the file
        // must not also state one (one truth).
        let first_kit = f.kits.first().and_then(|k| catalog.kit(k)).map(|(_, kd)| kd);
        if first_kit.is_some_and(|kd| matches!(kd.kind, KitKind::Fixed { .. })) {
            if f.kits.len() > 1 {
                errors.push(format!(
                    "{at}: a fixed planet declares exactly one kit — its \
                     environment is the whole level"
                ));
            }
            // v1 fixed levels have no connector seal, so the miniboss
            // fallback can't gate — every fixed level stages its fight
            // explicitly.
            for relative in 1..=f.levels {
                if !f.boss_slots.iter().any(|s| s.at.relative == relative) {
                    errors.push(format!(
                        "{at}: relative {relative} has no boss slot — every \
                         level of a fixed planet stages its boss (an open \
                         house has no room seal for a miniboss)"
                    ));
                }
            }
        }
        let first_kit_zone_count = first_kit
            .and_then(|kd| kd.environment())
            .and_then(|envk| environments.iter().position(|e| e.key == envk))
            .map(|ei| environments[ei].zones.len() as u32);
        let (rooms_base, rooms_per_level) = match (&f.rooms, first_kit_zone_count) {
            (Some(r), None) => (r.base, r.per_level),
            (None, Some(zones)) => (zones, 0),
            (Some(_), Some(_)) => {
                errors.push(format!(
                    "{at}: declares rooms, but its fixed kit's environment \
                     authors the room count — remove the rooms field"
                ));
                (0, 0)
            }
            (None, None) => {
                errors.push(format!(
                    "{at}: a generated planet must declare rooms = {{ base, per_level }}"
                ));
                (0, 0)
            }
        };
        let kit_ids: Vec<KitId> =
            f.kits.iter().filter_map(|k| kit_by_key(k, &mut errors, &at)).collect();
        // A planet mixing panel kits mixes them per ROOM on one cell grid,
        // so every panel kit it declares must derive the same grid as its
        // first — kits with different pitches belong to different planets.
        // The cross-boundary env join: a fixed kit's declared environment
        // KEY must name an authored zone map this grammar embeds.
        for &id in &kit_ids {
            let kit = catalog.kit_def(id);
            if let Some(envk) = kit.environment() {
                if !kit_environments.contains_key(envk) {
                    match environments.iter().position(|e| e.key == envk) {
                        Some(i) => {
                            kit_environments.insert(envk.to_string(), i);
                        }
                        None => errors.push(format!(
                            "kit '{}': unknown environment '{envk}'",
                            kit.key
                        )),
                    }
                }
            }
        }
        if let Some(first) = kit_ids.first().map(|&id| catalog.kit_def(id)) {
            for &id in &kit_ids[1..] {
                let kit = catalog.kit_def(id);
                if matches!(kit.kind, KitKind::Panel(_))
                    && (kit.tile != first.tile || kit.story != first.story)
                {
                    errors.push(format!(
                        "{at}: kit '{}' derives grid {}/{} but '{}' derives \
                         {}/{} — one planet, one grid",
                        kit.key, kit.tile, kit.story, first.key, first.tile, first.story
                    ));
                }
            }
        }
        planets.push(PlanetDef {
            planet: f.planet,
            levels: f.levels,
            kits: kit_ids,
            rooms_base,
            rooms_per_level,
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
        Ok(Roster { enemies, swarms, curves, catalog, kit_environments, planets, environments })
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

    /// The stand-in for the probe's derivation: grid + the filler-trio
    /// census a panel kit needs to pool (see [`test_census_variants`]).
    fn template_grid() -> String {
        format!(
            "[kits.template_kit]\ntile = 3.0\nstory = 3.0\n{}",
            test_census_variants(3.0)
        )
    }

    /// The template scaffold — every construct the grammar accepts, with its
    /// defaults — split into loadable sections. Linker-rule and mechanism
    /// tests doctor the relevant section, so a RULE or SHAPE is what's tested,
    /// never the shipped roster's tuning. The grid stands in for the probe's.
    fn template_parts() -> (String, String, String, String) {
        let rendered = template::template();
        let mut parts = rendered.split(template::CUT);
        let enemies = parts.next().expect("enemies section").to_string();
        let kits = parts.next().expect("kits section").to_string();
        let planet = parts.next().expect("planet section").to_string();
        (enemies, kits, template_grid(), planet)
    }

    use crate::asset_catalog::{
        ENVIRONMENTS_GENERATED_TOML, KITS_GENERATED_TOML, KITS_TOML, MODELS_TOML,
    };

    /// Test shim over the SPLIT load: builds a fixture catalog, then
    /// links the grammar against it — link-rule tests keep one entry and
    /// their original shape, whichever side of the boundary a rule
    /// lives on.
    fn load_split(
        enemies: &str,
        kits: &str,
        kit_grids: &str,
        models: &str,
        planets: &[&str],
        env: EnvSources,
    ) -> Result<Roster, String> {
        let catalog = crate::asset_catalog::AssetCatalog::load(
            kits,
            kit_grids,
            models,
            ENVIRONMENTS_GENERATED_TOML,
        )
        .map_err(|e| e.join("\n"))?;
        load_from(enemies, planets, env, std::sync::Arc::new(catalog))
    }

    fn load_template(enemies: &str, kits: &str, grid: &str, planet: &str) -> Result<Roster, String> {
        load_split(enemies, kits, grid, MODELS_TOML, &[planet], EnvSources::generated_only())
    }

    fn template_roster() -> Roster {
        let (enemies, kits, grid, planet) = template_parts();
        load_template(&enemies, &kits, &grid, &planet).expect("the template links")
    }

    // ── Fixed-paradigm fixtures: a small house environment over the real
    //    installed catalog. Violation tests doctor ONE aspect each, so the
    //    RULE is what's tested, never the shipped apartment's authoring. ──

    /// A valid three-zone environment: porch (start) → hall → den (boss,
    /// farthest). Baseline for every doctoring test.
    const FX_ENV: &str = r#"
[environment]
key = "fx_house_env"
model = "apartment"

[[zone]]
key = "porch"
box = { min = [0, 0, 0], extents = [2, 2, 2] }
start = true
links = ["hall"]

[[zone]]
key = "hall"
box = { min = [2, 0, 0], extents = [2, 2, 2] }
links = ["den"]
enemy_spawns = [[3.0, 1.0, 1.0]]

[[zone]]
key = "den"
box = { min = [4, 0, 0], extents = [2, 2, 2] }
boss = true
enemy_spawns = [[5.0, 1.0, 1.0]]
"#;

    const FX_FIXED_KIT: &str = "[kits.fx_house]\nparadigm = \"fixed\"\n\
        install_dir = \"godot/addons/environments\"\nscale = 5.0\n\
        environment = \"fx_house_env\"\n";

    /// A one-level fixed planet riding the template's enemy defs. No
    /// `rooms` — a fixed planet's room count is the environment's — and a
    /// staged boss at its only level (fixed levels always stage the fight).
    const FX_FIXED_PLANET: &str = "planet = 1\nlevels = 1\nkits = [\"fx_house\"]\n\
        [[level]]\nrelative = 1\nenemies = [\"template_enemy\"]\n\
        [[boss_slot]]\nat = { relative = 1 }\nboss = \"template_boss\"\n\
        escorts = { enemy = \"template_minion\", trigger = \"on_engage\", count = 2 }\n\
        track = 1\nreward = \"consolation_pile\"\n";

    /// Load the fixed fixture with a doctored environment/kit/planet.
    fn load_fixed(env: &str, kits: &str, planet: &str) -> Result<Roster, String> {
        let (enemies, _, _, _) = template_parts();
        load_split(
            &enemies,
            kits,
            "[kits]\n",
            MODELS_TOML,
            &[planet],
            EnvSources { authored: &[env], windows: &[] },
        )
    }

    // ── Panel-kit wall pools: census (probe pieces) × authored policy ──

    /// A panel kit with an authored coverage policy.
    const FX_PANEL_KIT: &str = "[kits.fx_panels]\nparadigm = \"panel\"\n\
        install_dir = \"godot/addons/fx_walls\"\nwall_coverage = 0.9\n";

    /// The probe's census for it: two wall-worthy squares (one exactly on
    /// pitch, one within tolerance), a truss square, a solid strip, and a
    /// small solid tile — only the first two may pool.
    const FX_PANEL_GRID: &str = r#"
[kits.fx_panels]
tile = 3.0
story = 3.0

[kits.fx_panels.pieces.solid_a]
face = [3.0, 3.0]
thick = 0.2
axis = "y"
coverage = 1.0
tris = 100
textures = 3

[kits.fx_panels.pieces.solid_b]
face = [2.999, 2.998]
thick = 0.2
axis = "y"
coverage = 0.97
tris = 100
textures = 3

[kits.fx_panels.pieces.truss_a]
face = [3.0, 3.0]
thick = 0.2
axis = "y"
coverage = 0.2
tris = 100
textures = 3

[kits.fx_panels.pieces.strip_a]
face = [5.0, 3.0]
thick = 0.2
axis = "y"
coverage = 1.0
tris = 100
textures = 3

[kits.fx_panels.pieces.tile_a]
face = [1.0, 1.0]
thick = 0.1
axis = "y"
coverage = 1.0
tris = 100
textures = 3
"#;

    const FX_PANEL_PLANET: &str = "planet = 1\nlevels = 1\nkits = [\"fx_panels\"]\n\
        rooms = { base = 6, per_level = 2 }\n\
        [[level]]\nrelative = 1\nenemies = [\"template_enemy\"]\n";

    fn load_panel_fixture(kits: &str, grid: &str) -> Result<Roster, String> {
        let (enemies, _, _, _) = template_parts();
        load_split(
            &enemies,
            kits,
            grid,
            MODELS_TOML,
            &[FX_PANEL_PLANET],
            EnvSources::generated_only(),
        )
    }


    /// One census-v2 variant record — tests compose grids from these,
    /// doctoring only what the RULE under test needs (never shipped data).
    fn fx_variant(
        stem: &str,
        role: &str,
        face: [f32; 2],
        axis: &str,
        detail: &str,
        stretch: f32,
    ) -> String {
        format!(
            "[kits.fx_panels.variants.{stem}]\n\
             sources = [\"{stem}_src\"]\nrole = \"{role}\"\n\
             face = [{}, {}]\nthick = 0.2\naxis = \"{axis}\"\n\
             detail = \"{detail}\"\ncoverage = 1.0\nstretch = {stretch:?}\n\
             tris = 100\ntextures = 3\n",
            face[0], face[1],
        )
    }

    fn fx_variant_grid(variants: &[String]) -> String {
        format!(
            "[kits.fx_panels]\ntile = 3.0\nstory = 3.0\n\n{}",
            variants.join("\n")
        )
    }

    /// The healthy baked set: a 1-module filler in each surface role,
    /// one wide wall, one decoration (with a preassembly-style stretch).
    fn fx_variants_healthy() -> Vec<String> {
        vec![
            fx_variant("base_floor", "floor", [3.0, 3.0], "y", "pos_y", 0.0),
            fx_variant("base_ceiling", "ceiling", [3.0, 3.0], "y", "neg_y", 0.0),
            fx_variant("base_wall", "wall", [3.0, 3.0], "z", "pos_z", 0.0),
            fx_variant("wide_wall", "wall", [5.0, 3.0], "z", "pos_z", 0.0),
            fx_variant("greeble", "decoration", [1.0, 0.9], "z", "pos_z", 0.07),
        ]
    }

    #[test]
    fn variants_derive_role_pools() {
        let grid = fx_variant_grid(&fx_variants_healthy());
        let roster = load_panel_fixture(FX_PANEL_KIT, &grid).expect("variants census links");
        let (_, kit) = roster.catalog.kit("fx_panels").unwrap();
        let pools = kit
            .role_pools()
            .expect("a variants census derives role pools");
        assert_eq!(pools.floor.len(), 1);
        assert_eq!(pools.ceiling.len(), 1);
        assert_eq!(pools.wall.len(), 2);
        assert_eq!(pools.decoration.len(), 1);
        assert!(pools.addon.is_empty());
        let wide = pools
            .wall
            .iter()
            .find(|p| p.face[0] > 4.0)
            .expect("the wide wall pools");
        assert_eq!(wide.face, [5.0, 3.0], "wall faces read [width, height]");
        assert!(
            roster.catalog.path(wide.scene).starts_with("res://"),
            "pool ids resolve to res:// scenes, got {}",
            roster.catalog.path(wide.scene)
        );
        assert!((wide.thick - 0.2).abs() < 1e-6, "censused thickness carries");
    }

    #[test]
    fn a_pieces_census_beside_variants_changes_nothing() {
        // The pieces census SURVIVES v1 (pitch derivation, bake
        // qualification, the probe's bake contract) — but it links
        // nothing anymore: a kit carrying both censuses derives exactly
        // the pools its variants alone would.
        let grid = format!(
            "{FX_PANEL_GRID}\n{}",
            fx_variants_healthy().join("\n")
        );
        let roster = load_panel_fixture(FX_PANEL_KIT, &grid).expect("census links");
        let (_, kit) = roster.catalog.kit("fx_panels").unwrap();
        let pools = kit.role_pools().expect("role pools derive from variants");
        assert_eq!(pools.wall.len(), 2, "pieces contribute no plates");
    }

    #[test]
    fn a_missing_surface_filler_is_a_link_error() {
        // Every surface role must keep a 1-module filler — the coverer's
        // completion guarantee. The wall case still has the wide plate,
        // so the error is specifically about the FILLER, not emptiness.
        for role in ["floor", "ceiling", "wall"] {
            let variants: Vec<String> = fx_variants_healthy()
                .into_iter()
                .filter(|v| !v.starts_with(&format!("[kits.fx_panels.variants.base_{role}]")))
                .collect();
            let err = load_panel_fixture(FX_PANEL_KIT, &fx_variant_grid(&variants))
                .unwrap_err();
            assert!(
                err.contains("fx_panels") && err.contains(role) && err.contains("filler"),
                "{role}: {err}"
            );
        }
    }

    #[test]
    fn a_mis_baked_variant_is_not_pooled() {
        // Census × policy: a variant whose measured pose disagrees with
        // its role is SKIPPED, never pooled and never a link error on
        // its own — the game must not be hostage to a Transform bug
        // (the loud gate is the probe's bake contract at `make assets`).
        // Corrupting a FILLER therefore surfaces as the filler rule:
        let mut variants = fx_variants_healthy();
        variants[0] = fx_variant("base_floor", "floor", [3.0, 3.0], "z", "pos_y", 0.0);
        let err = load_panel_fixture(FX_PANEL_KIT, &fx_variant_grid(&variants)).unwrap_err();
        assert!(err.contains("floor") && err.contains("filler"), "{err}");

        // Corrupting a NON-filler links fine — the plate is just absent.
        let mut variants = fx_variants_healthy();
        variants[3] = fx_variant("wide_wall", "wall", [6.0, 3.0], "z", "neg_z", 0.0);
        let roster = load_panel_fixture(FX_PANEL_KIT, &fx_variant_grid(&variants))
            .expect("a skipped non-filler is not a link error");
        let (_, kit) = roster.catalog.kit("fx_panels").unwrap();
        let pools = kit.role_pools().expect("pools derive");
        assert_eq!(pools.wall.len(), 1, "the mis-baked wide wall is not pooled");
    }

    #[test]
    fn a_wall_off_the_story_module_is_a_link_error() {
        let mut variants = fx_variants_healthy();
        variants.push(fx_variant("odd_wall", "wall", [4.0, 2.5], "z", "pos_z", 0.0));
        let err = load_panel_fixture(FX_PANEL_KIT, &fx_variant_grid(&variants)).unwrap_err();
        assert!(err.contains("odd_wall") && err.contains("story"), "{err}");
    }

    #[test]
    fn variant_stretch_beyond_unit_is_a_link_error() {
        let mut variants = fx_variants_healthy();
        variants.push(fx_variant("taffy", "decoration", [1.0, 1.0], "z", "pos_z", 1.5));
        let err = load_panel_fixture(FX_PANEL_KIT, &fx_variant_grid(&variants)).unwrap_err();
        assert!(err.contains("taffy") && err.contains("stretch"), "{err}");
    }

    #[test]
    fn a_panel_kit_without_baked_variants_is_a_link_error() {
        // v1 is retired: a pieces-only census carries no kit. A panel
        // kit links through its baked role pools alone, and the error
        // names the healer.
        let err = load_panel_fixture(FX_PANEL_KIT, FX_PANEL_GRID).unwrap_err();
        assert!(
            err.contains("fx_panels") && err.contains("variants"),
            "the error names the kit and the missing bake: {err}"
        );
    }

    #[test]
    fn a_panel_level_spec_carries_its_declared_pools() {
        let roster = load_panel_fixture(FX_PANEL_KIT, &fx_variant_grid(&fx_variants_healthy()))
            .expect("the panel fixture links");
        let spec = crate::level_spec::LevelSpec::for_level(
            &roster,
            crate::seed::Seed::new(1),
            1,
            &crate::unlocks::PermanentUnlocks::new(),
        );
        match &spec.paradigm {
            crate::level_spec::Paradigm::Panel(kits) => {
                assert_eq!(kits.len(), 1, "one declared panel kit");
                let pools = roster
                    .catalog
                    .kit_def(kits[0])
                    .role_pools()
                    .expect("the spec's kit ids resolve to derived role pools");
                assert_eq!(
                    pools
                        .wall
                        .iter()
                        .map(|p| roster.catalog.path(p.scene))
                        .collect::<Vec<_>>(),
                    [
                        "res://addons/fx_walls/base_wall.glb",
                        "res://addons/fx_walls/wide_wall.glb",
                    ],
                    "the spec speaks the grammar's derived pools"
                );
            }
            other => panic!("a panel planet's spec is Panel, got {other:?}"),
        }
    }

    /// A second panel kit; its census tile is a fixture knob so agreement
    /// tests can doctor it.
    fn fx_panel_kit2(tile: f32) -> (String, String) {
        let variant = |stem: &str, role: &str, axis: &str, detail: &str| {
            format!(
                "[kits.fx_panels2.variants.{stem}]\n\
                 sources = [\"{stem}_src\"]\nrole = \"{role}\"\n\
                 face = [{tile}, {tile}]\nthick = 0.2\naxis = \"{axis}\"\n\
                 detail = \"{detail}\"\ncoverage = 1.0\nstretch = 0.0\n\
                 tris = 100\ntextures = 3\n"
            )
        };
        (
            "[kits.fx_panels2]\nparadigm = \"panel\"\n\
             install_dir = \"godot/addons/fx_walls2\"\nwall_coverage = 0.9\n"
                .to_string(),
            format!(
                "[kits.fx_panels2]\ntile = {tile}\nstory = {tile}\n\n{}{}{}",
                variant("floor2", "floor", "y", "pos_y"),
                variant("ceiling2", "ceiling", "y", "neg_y"),
                variant("wall2", "wall", "z", "pos_z"),
            ),
        )
    }

    const FX_TWO_KIT_PLANET: &str = "planet = 1\nlevels = 1\n\
        kits = [\"fx_panels\", \"fx_panels2\"]\n\
        rooms = { base = 6, per_level = 2 }\n\
        [[level]]\nrelative = 1\nenemies = [\"template_enemy\"]\n";

    fn load_two_kit_fixture(tile2: f32) -> Result<Roster, String> {
        let (enemies, _, _, _) = template_parts();
        let (kit2, grid2) = fx_panel_kit2(tile2);
        load_split(
            &enemies,
            &format!("{FX_PANEL_KIT}{kit2}"),
            &format!("{}\n{grid2}", fx_variant_grid(&fx_variants_healthy())),
            MODELS_TOML,
            &[FX_TWO_KIT_PLANET],
            EnvSources::generated_only(),
        )
    }

    #[test]
    fn a_planet_mixing_panel_kits_pools_them_all() {
        let roster = load_two_kit_fixture(3.0).expect("agreeing kits link");
        let kits = roster.panel_kits_for_level(1);
        assert_eq!(
            kits.iter()
                .map(|&id| roster.catalog.kit_def(id).key.as_str())
                .collect::<Vec<_>>(),
            ["fx_panels", "fx_panels2"],
            "both declared kits pool, in declaration order"
        );
    }

    #[test]
    fn panel_kits_of_one_planet_must_agree_on_the_grid() {
        let err = load_two_kit_fixture(2.0).unwrap_err();
        assert!(
            err.contains("fx_panels2") && err.contains("grid"),
            "the error names the disagreeing kit: {err}"
        );
    }

    #[test]
    fn a_panel_kit_must_declare_its_coverage_policy() {
        let kit = "[kits.fx_panels]\nparadigm = \"panel\"\n\
            install_dir = \"godot/addons/fx_walls\"\n";
        let err =
            load_panel_fixture(kit, &fx_variant_grid(&fx_variants_healthy())).unwrap_err();
        assert!(
            err.contains("fx_panels") && err.contains("wall_coverage"),
            "the error names the kit and the missing policy: {err}"
        );
    }

    #[test]
    fn coverage_policy_on_a_non_panel_kit_is_a_link_error() {
        let doctored = FX_FIXED_KIT.replace("scale = 5.0", "scale = 5.0\nwall_coverage = 0.9");
        assert_ne!(doctored, FX_FIXED_KIT, "the fixture kit is fixed");
        let err = load_fixed(FX_ENV, &doctored, FX_FIXED_PLANET).unwrap_err();
        assert!(
            err.contains("wall_coverage") && err.contains("panel-kit knob"),
            "the error names the misplaced policy: {err}"
        );
    }

    #[test]
    fn a_valid_fixed_fixture_links() {
        let roster =
            load_fixed(FX_ENV, FX_FIXED_KIT, FX_FIXED_PLANET).expect("the fixed fixture links");
        let env = roster.environment_for_level(1).expect("level 1 is fixed");
        assert_eq!(env.key, "fx_house_env");
        assert_eq!(env.zones.len(), 3);
        assert_eq!(env.zones[env.start_zone].key, "porch");
        assert_eq!(env.zones[env.boss_zone].key, "den");
        let pitch = roster.pitch_for_level(1);
        assert_eq!((pitch.tile, pitch.story), (5.0, 5.0), "pitch IS the declared scale");
        let planet = roster.planet_for_level(1);
        assert_eq!(
            (planet.rooms_base, planet.rooms_per_level),
            (3, 0),
            "a fixed planet's room count is the authored zone count"
        );
    }

    #[test]
    fn an_authored_sun_links_and_a_bad_one_is_a_link_error() {
        let roster = load_fixed(FX_ENV, FX_FIXED_KIT, FX_FIXED_PLANET)
            .expect("the fixed fixture links");
        let env = roster.environment_for_level(1).expect("level 1 is fixed");
        assert_eq!(env.sun, None, "the inline fixture authors no sun");
        let with_sun = FX_ENV.replace(
            "model = \"apartment\"",
            "model = \"apartment\"\n[sun]\nazimuth_deg = 135.0\nelevation_deg = 40.0\nenergy = 1.5",
        );
        let roster = load_fixed(&with_sun, FX_FIXED_KIT, FX_FIXED_PLANET)
            .expect("a sunlit fixture links");
        let sun = roster
            .environment_for_level(1)
            .and_then(|e| e.sun)
            .expect("the authored sun links");
        assert_eq!(
            (sun.azimuth_deg, sun.elevation_deg, sun.energy),
            (135.0, 40.0, 1.5)
        );
        let doctored = with_sun.replace("elevation_deg = 40.0", "elevation_deg = 120.0");
        let err = load_fixed(&doctored, FX_FIXED_KIT, FX_FIXED_PLANET).unwrap_err();
        assert!(err.contains("elevation"), "names the horizon rule: {err}");
    }

    #[test]
    fn an_authored_ambient_links_and_a_dead_one_is_a_link_error() {
        let with_ambient = FX_ENV.replace(
            "model = \"apartment\"",
            "model = \"apartment\"\n[ambient]\nenergy = 0.8\ncolor = [1.0, 0.95, 0.9]",
        );
        let roster = load_fixed(&with_ambient, FX_FIXED_KIT, FX_FIXED_PLANET)
            .expect("an ambient fixture links");
        let ambient = roster
            .environment_for_level(1)
            .and_then(|e| e.ambient)
            .expect("the authored ambient links");
        assert_eq!((ambient.energy, ambient.color), (0.8, [1.0, 0.95, 0.9]));
        let doctored = with_ambient.replace("energy = 0.8", "energy = 0.0");
        let err = load_fixed(&doctored, FX_FIXED_KIT, FX_FIXED_PLANET).unwrap_err();
        assert!(err.contains("ambient"), "names the dead-fill rule: {err}");
    }

    #[test]
    fn a_window_with_a_bad_dimension_is_a_link_error() {
        let with_window = FX_ENV.replace(
            "model = \"apartment\"",
            "model = \"apartment\"\n[[window]]\ncenter = [6.0, 1.0, 1.0]\n\
             size = [1.5, 1.2]\nfacing = \"neg_x\"\nenergy = 3.0",
        );
        let roster = load_fixed(&with_window, FX_FIXED_KIT, FX_FIXED_PLANET)
            .expect("a windowed fixture links");
        let env = roster.environment_for_level(1).expect("level 1 is fixed");
        assert_eq!(env.windows.len(), 1);
        assert_eq!(env.windows[0].inward, [-1.0, 0.0, 0.0], "neg_x shines west");
        let doctored = with_window.replace("energy = 3.0", "energy = 0.0");
        let err = load_fixed(&doctored, FX_FIXED_KIT, FX_FIXED_PLANET).unwrap_err();
        assert!(err.contains("energy"), "names the dead-panel rule: {err}");
    }

    #[test]
    fn a_second_start_zone_is_a_link_error() {
        let doctored = FX_ENV.replace("key = \"hall\"", "key = \"hall\"\nstart = true");
        let err = load_fixed(&doctored, FX_FIXED_KIT, FX_FIXED_PLANET).unwrap_err();
        assert!(err.contains("start"), "names the start-zone rule: {err}");
    }

    #[test]
    fn a_missing_boss_zone_is_a_link_error() {
        let doctored = FX_ENV.replace("boss = true\n", "");
        let err = load_fixed(&doctored, FX_FIXED_KIT, FX_FIXED_PLANET).unwrap_err();
        assert!(err.contains("boss"), "names the boss-zone rule: {err}");
    }

    #[test]
    fn a_dangling_zone_link_is_a_link_error() {
        let doctored = FX_ENV.replace("links = [\"den\"]", "links = [\"nowhere\"]");
        let err = load_fixed(&doctored, FX_FIXED_KIT, FX_FIXED_PLANET).unwrap_err();
        assert!(err.contains("nowhere"), "names the dangling key: {err}");
    }

    #[test]
    fn a_disconnected_zone_is_a_link_error() {
        let doctored = FX_ENV.replace("links = [\"den\"]\n", "");
        let err = load_fixed(&doctored, FX_FIXED_KIT, FX_FIXED_PLANET).unwrap_err();
        assert!(
            err.contains("unreachable") || err.contains("connected"),
            "names the connectivity rule: {err}"
        );
    }

    #[test]
    fn a_spawn_outside_its_zone_box_is_a_link_error() {
        let doctored = FX_ENV.replace("[[3.0, 1.0, 1.0]]", "[[30.0, 1.0, 1.0]]");
        let err = load_fixed(&doctored, FX_FIXED_KIT, FX_FIXED_PLANET).unwrap_err();
        assert!(err.contains("outside"), "names the in-box rule: {err}");
    }

    #[test]
    fn a_link_between_zones_that_share_no_face_is_a_link_error() {
        // fixed_layout synthesizes a doorway connector on the shared face
        // of every linked pair — a link with no shared face has nowhere to
        // put one. Doctor the den out of touching range of the hall.
        let doctored = FX_ENV.replace("min = [4, 0, 0]", "min = [5, 0, 0]");
        let err = load_fixed(&doctored, FX_FIXED_KIT, FX_FIXED_PLANET).unwrap_err();
        assert!(
            err.contains("share") || err.contains("adjacent"),
            "names the shared-face rule: {err}"
        );
    }

    #[test]
    fn a_boss_zone_that_is_not_farthest_is_a_link_error() {
        let doctored = FX_ENV
            .replace("boss = true\n", "")
            .replace("key = \"hall\"", "key = \"hall\"\nboss = true");
        let err = load_fixed(&doctored, FX_FIXED_KIT, FX_FIXED_PLANET).unwrap_err();
        assert!(err.contains("farthest"), "names the farthest rule: {err}");
    }

    #[test]
    fn a_boss_zone_without_exactly_one_spawn_is_a_link_error() {
        let doctored =
            FX_ENV.replace("[[5.0, 1.0, 1.0]]", "[[5.0, 1.0, 1.0], [4.5, 1.0, 1.0]]");
        let err = load_fixed(&doctored, FX_FIXED_KIT, FX_FIXED_PLANET).unwrap_err();
        assert!(err.contains("exactly one"), "names the arena-anchor rule: {err}");
    }

    #[test]
    fn a_start_zone_with_enemy_spawns_is_a_link_error() {
        let doctored = FX_ENV.replace(
            "start = true",
            "start = true\nenemy_spawns = [[1.0, 1.0, 1.0]]",
        );
        let err = load_fixed(&doctored, FX_FIXED_KIT, FX_FIXED_PLANET).unwrap_err();
        assert!(
            err.contains("start") && err.contains("enemy"),
            "names the clear-start rule: {err}"
        );
    }

    #[test]
    fn overlapping_zone_boxes_are_a_link_error() {
        let doctored = FX_ENV.replace("min = [2, 0, 0]", "min = [1, 0, 0]");
        let err = load_fixed(&doctored, FX_FIXED_KIT, FX_FIXED_PLANET).unwrap_err();
        assert!(err.contains("overlap"), "names the overlap rule: {err}");
    }

    #[test]
    fn a_fixed_kit_without_scale_is_a_link_error() {
        let doctored = FX_FIXED_KIT.replace("scale = 5.0\n", "");
        let err = load_fixed(FX_ENV, &doctored, FX_FIXED_PLANET).unwrap_err();
        assert!(err.contains("scale"), "names the missing knob: {err}");
    }

    #[test]
    fn a_fixed_kit_without_an_environment_is_a_link_error() {
        let doctored = FX_FIXED_KIT.replace("environment = \"fx_house_env\"\n", "");
        let err = load_fixed(FX_ENV, &doctored, FX_FIXED_PLANET).unwrap_err();
        assert!(err.contains("environment"), "names the missing join: {err}");
    }

    #[test]
    fn a_fixed_kit_with_an_unknown_environment_is_a_link_error() {
        let doctored = FX_FIXED_KIT.replace("fx_house_env", "no_such_env");
        let err = load_fixed(FX_ENV, &doctored, FX_FIXED_PLANET).unwrap_err();
        assert!(err.contains("no_such_env"), "names the dangling key: {err}");
    }

    #[test]
    fn an_environment_with_an_uninstalled_model_is_a_link_error() {
        let doctored = FX_ENV.replace("model = \"apartment\"", "model = \"no_such_scene\"");
        let err = load_fixed(&doctored, FX_FIXED_KIT, FX_FIXED_PLANET).unwrap_err();
        assert!(err.contains("no_such_scene"), "names the missing install: {err}");
    }

    #[test]
    fn a_generated_kit_declaring_fixed_knobs_is_a_link_error() {
        let (enemies, kits, grid, planet) = template_parts();
        let doctored = kits.replace("install_dir", "scale = 5.0\ninstall_dir");
        assert_ne!(kits, doctored, "the template declares a generated kit");
        let err = load_split(
            &enemies,
            &doctored,
            &grid,
            MODELS_TOML,
            &[&planet],
            EnvSources::generated_only(),
        )
        .unwrap_err();
        assert!(err.contains("scale"), "names the misplaced knob: {err}");
    }

    #[test]
    fn a_fixed_planet_declaring_rooms_is_a_link_error() {
        let doctored = FX_FIXED_PLANET.replace(
            "[[level]]",
            "rooms = { base = 4, per_level = 1 }\n[[level]]",
        );
        let err = load_fixed(FX_ENV, FX_FIXED_KIT, &doctored).unwrap_err();
        assert!(err.contains("rooms"), "names the one-truth rule: {err}");
    }

    #[test]
    fn a_generated_planet_without_rooms_is_a_link_error() {
        let (enemies, kits, grid, planet) = template_parts();
        let doctored: String = planet
            .lines()
            .filter(|l| !l.starts_with("rooms"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_ne!(planet, doctored, "the template declares rooms");
        let err = load_split(
            &enemies,
            &kits,
            &grid,
            MODELS_TOML,
            &[&doctored],
            EnvSources::generated_only(),
        )
        .unwrap_err();
        assert!(err.contains("rooms"), "names the missing growth: {err}");
    }

    #[test]
    fn a_fixed_planet_level_without_a_boss_slot_is_a_link_error() {
        // v1 fixed levels have no connector seal, so the miniboss fallback
        // can't gate — every fixed level must stage its fight explicitly.
        let (planet, slot) = FX_FIXED_PLANET
            .split_once("[[boss_slot]]")
            .expect("the fixture stages a boss");
        assert!(!slot.is_empty());
        let err = load_fixed(FX_ENV, FX_FIXED_KIT, planet).unwrap_err();
        assert!(
            err.contains("boss slot"),
            "names the staged-fight rule: {err}"
        );
    }

    #[test]
    fn a_fixed_planet_with_more_than_one_kit_is_a_link_error() {
        let (enemies, kits, grid, _) = template_parts();
        let two_kits = format!("{FX_FIXED_KIT}{kits}");
        let doctored = FX_FIXED_PLANET
            .replace("kits = [\"fx_house\"]", "kits = [\"fx_house\", \"template_kit\"]");
        let err = load_split(
            &enemies,
            &two_kits,
            &grid,
            MODELS_TOML,
            &[&doctored],
            EnvSources { authored: &[FX_ENV], windows: &[] },
        )
        .unwrap_err();
        assert!(
            err.contains("one kit") || err.contains("exactly one"),
            "names the single-kit rule: {err}"
        );
    }

    #[test]
    fn the_foundational_roster_parses_and_links() {
        if let Err(e) = load() {
            panic!("roster load failed:\n{e}");
        }
    }

    #[test]
    fn enemy_key_is_the_one_identity() {
        let r = loaded();
        // The boundary parser is the ONLY string→identity door: a declared
        // key resolves, an undeclared one is rejected (no panic).
        let k = r.enemy_keys().next().expect("the grammar declares an enemy");
        assert!(
            r.enemy_key("pretty_pretty_princess").is_none(),
            "an undeclared key is rejected at the boundary"
        );
        // The identity resolves to its def, infallibly, and round-trips.
        assert_eq!(r.enemy_key(r.enemy(k).key.as_str()), Some(k));
        assert_eq!(
            r.enemy_keys().count(),
            r.enemies.len(),
            "enemy_keys covers every declared enemy in declaration order"
        );
        for key in r.enemy_keys() {
            assert_eq!(
                r.enemy_key(r.enemy(key).key.as_str()),
                Some(key),
                "every declared key round-trips through the parser"
            );
        }
    }

    #[test]
    fn a_negative_switch_is_a_link_error() {
        let (enemies, kits, grid, planet) = template_parts();
        let enemies = enemies.replace("drain_dps = 0.0", "drain_dps = -6.0");
        let err = load_template(&enemies, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("drain_dps must be finite"), "{err}");
    }

    #[test]
    fn a_kit_without_a_derived_grid_is_a_link_error() {
        let (enemies, kits, grid, planet) = template_parts();
        let grid = grid.replace("[kits.template_kit]", "[kits.renamed]");
        let err = load_template(&enemies, &kits, &grid, &planet).unwrap_err();
        assert!(
            err.contains("kit 'template_kit': no derived grid"),
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
            crate::asset_catalog::probe::kit_grids(),
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
        let err = load_split(&doctored, KITS_TOML, KITS_GENERATED_TOML, MODELS_TOML, PLANET_TOMLS, EnvSources::shipped()).unwrap_err();
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
        let catalog: crate::asset_catalog::schema::ModelsFile =
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
        // The MECHANISM, not any shipped number: a stat pointed at a ramp
        // curve moves with level (below baseline at level 1, climbing toward
        // its peak); a flat curve holds constant. Proven on the template's own
        // curves (template_enemy: speed/cooldown ramp, hp/damage/ranges flat).
        let roster = template_roster();
        let id = roster.enemy_keys().next().unwrap();
        let base = roster.enemy(id).stats;
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        let l1 = roster.stats_at(id, 1);
        let peak = roster.stats_at(id, 10);
        assert!(l1.speed < base.speed, "the ramp starts below baseline at level 1");
        assert!(peak.speed > l1.speed, "the ramp climbs with level");
        assert!(peak.cooldown > l1.cooldown, "cooldown rides the same ramp");
        assert!(close(l1.hp, base.hp) && close(peak.hp, base.hp), "a flat curve holds hp");
        assert!(close(peak.detection, base.detection), "flat holds detection");
        assert!(close(peak.attack_range, base.attack_range), "flat holds attack_range");
    }

    #[test]
    fn derived_ranges_ride_their_base_stats_curve() {
        // The AI's derived absolutes ride the SAME curve as the base stat they
        // derive from: disengage↔detection, standoff↔attack_range, shield↔hp.
        // Pointed at the template's ramp with a shield declared, so the SHAPE
        // is tested and the factor is DERIVED, never a shipped number.
        let (enemies, kits, grid, planet) = template_parts();
        let enemies = enemies
            .replace("detection = \"flat\"", "detection = \"example_ramp\"")
            .replace("attack_range = \"flat\"", "attack_range = \"example_ramp\"")
            .replace("hp = \"flat\"", "hp = \"example_ramp\"")
            .replace("shield_frac = 0.0", "shield_frac = 0.5");
        let roster = load_template(&enemies, &kits, &grid, &planet).unwrap();
        let id = roster.enemy_keys().next().unwrap();
        let def = roster.enemy(id);
        // The ramp's peak factor, DERIVED from a base stat that rides it.
        let f = roster.stats_at(id, 10).speed / def.stats.speed;
        assert!(f > 1.0, "sanity: the ramp lifts at peak");
        let cfg = roster.ai_config_at(id, 10);
        let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
        assert!(close(cfg.detection_range, def.stats.detection * f));
        assert!(close(cfg.attack_range, def.stats.attack_range * f));
        assert!(close(cfg.disengage_range, def.behavior.disengage * f),
            "disengage follows detection");
        assert!(close(cfg.standoff_range, def.behavior.standoff * f),
            "standoff follows attack_range");
        assert!(close(cfg.health.as_f32(), def.stats.hp * f), "health rides hp");
        assert!(close(cfg.shield.expect("a shield was declared").as_f32(),
            def.behavior.shield * f), "shield follows hp");
    }

    #[test]
    fn scaling_defaults_must_cover_every_stat() {
        let (enemies, kits, grid, planet) = template_parts();
        let enemies = enemies.replace("detection = \"flat\"\n", "");
        let err = load_template(&enemies, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("no 'detection' curve"), "{err}");
    }

    #[test]
    fn latch_and_bolt_switches_default_by_archetype() {
        // Omitting a switch takes the ARCHETYPE's default: swarmers latch and
        // slow, non-swarmers never latch, firing archetypes get a bolt speed.
        // The MECHANISM (which default applies), not any number — on a fixture
        // whose enemies declare NO switches, so shipped tuning can't move it.
        let enemies = "[curves.flat]\nkind = \"flat\"\n\n\
            [scaling_defaults]\nspeed = \"flat\"\ncooldown = \"flat\"\nhp = \"flat\"\n\
            damage = \"flat\"\ndetection = \"flat\"\nattack_range = \"flat\"\n\n\
            [[enemy]]\nkey = \"fx_swarmer\"\nname = \"S\"\nblurb = \"b\"\nmodel = \"m0\"\n\
            size = 1.0\nyaw_offset_deg = 0\nai = \"swarmer\"\nreward = 100\nspawns_directly = true\n\
            [enemy.stats]\nhp = 5.0\nspeed = 9.0\ndamage = 3.0\ndetection = 25.0\nattack_range = 8.0\ncooldown = 1.0\n\n\
            [[enemy]]\nkey = \"fx_shooter\"\nname = \"H\"\nblurb = \"b\"\nmodel = \"m1\"\n\
            size = 1.0\nyaw_offset_deg = 0\nai = \"shooter\"\nreward = 100\nspawns_directly = true\n\
            [enemy.stats]\nhp = 5.0\nspeed = 9.0\ndamage = 3.0\ndetection = 25.0\nattack_range = 10.0\ncooldown = 1.0\n";
        let models = "[models]\nm0 = \"res://x/m0.glb\"\nm1 = \"res://x/m1.glb\"\n";
        let kits = "[kits.k]\nparadigm = \"panel\"\ninstall_dir = \"godot/addons/walls\"\n\
            wall_coverage = 0.9\n";
        let grid = &format!("[kits.k]\ntile = 3.0\nstory = 3.0\n{}", test_census_variants(3.0));
        let planet = "planet = 1\nlevels = 1\nkits = [\"k\"]\nrooms = { base = 6, per_level = 2 }\n\n\
            [[level]]\nrelative = 1\nenemies = [\"fx_swarmer\", \"fx_shooter\"]\n";
        let roster = load_split(enemies, kits, grid, models, &[planet], EnvSources::generated_only()).unwrap();
        let sw = roster.enemy(roster.enemy_key("fx_swarmer").unwrap());
        let sh = roster.enemy(roster.enemy_key("fx_shooter").unwrap());
        assert!(sw.behavior.latch_range > 0.0, "a swarmer latches by default");
        assert!(sw.behavior.slow_factor < 1.0, "a swarmer's tag slows");
        assert!(sw.behavior.slow_duration > 0.0 && sw.behavior.slow_interval > 0.0,
            "a swarmer's tags tick");
        assert_eq!(sh.behavior.latch_range, 0.0, "a non-swarmer never latches");
        assert_eq!(sh.behavior.slow_factor, 1.0, "no slow without latching");
        assert!(sh.behavior.bolt_speed > 0.0, "a firing archetype gets a bolt speed");
    }

    #[test]
    fn a_declared_latch_switch_beats_its_archetype_default() {
        let (enemies, kits, grid, planet) = template_parts();
        let enemies = enemies
            .replace("latch_range = 0.0", "latch_range = 4.5")
            .replace("bolt_speed = 13.0", "bolt_speed = 20.0");
        let roster = load_template(&enemies, &kits, &grid, &planet).unwrap();
        let e = roster.enemy(roster.enemy_keys().next().unwrap());
        assert_eq!(e.behavior.latch_range, 4.5, "the declared reach wins");
        assert_eq!(e.behavior.bolt_speed, 20.0, "declared even when unused");
    }

    #[test]
    fn a_declared_miniboss_switch_marks_the_room_sealer() {
        // The miniboss grammar (design 2026-07-06): any enemy may declare
        // `miniboss = true` — while it lives, its room's exits seal red and
        // the boss bed plays; death re-opens them. It is a declared switch,
        // off by default, not an engine concept. Proven against the template
        // fixture so shipped tuning — which defs ARE minibosses — can never
        // move this contract.
        let rendered = template::template();
        let mut parts = rendered.split(template::CUT);
        let enemies = parts.next().expect("enemies section");
        let kits = parts.next().expect("kits section");
        let planet = parts.next().expect("planet section");
        let grid = &template_grid();

        // Off by default: the template declares no miniboss.
        let roster = load_split(enemies, kits, grid, MODELS_TOML, &[planet], EnvSources::generated_only()).unwrap();
        for key in roster.enemy_keys() {
            assert!(
                !roster.enemy(key).behavior.miniboss,
                "the template declares no miniboss: {}",
                roster.enemy(key).key
            );
        }

        // Declared, it reads back: flip the switch on the first enemy block.
        let doctored = enemies.replacen("ai = ", "miniboss = true\nai = ", 1);
        let roster = load_split(&doctored, kits, grid, MODELS_TOML, &[planet], EnvSources::generated_only()).unwrap();
        assert!(
            roster.enemy_keys().any(|key| roster.enemy(key).behavior.miniboss),
            "the declared switch reads back"
        );
    }

    #[test]
    fn a_negative_latch_switch_is_a_link_error() {
        let (enemies, kits, grid, planet) = template_parts();
        let enemies = enemies.replace("slow_factor = 1.0", "slow_factor = -0.5");
        let err = load_template(&enemies, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("slow_factor must be finite"), "{err}");
    }

    #[test]
    fn muzzles_default_to_centre_fire_and_read_back_in_order() {
        // No muzzles = the legacy centre-spawn convention. Declared barrel
        // tips read back in declaration order — the fire site round-robins
        // them, so order IS the firing pattern.
        let (enemies, kits, grid, planet) = template_parts();
        let roster = load_template(&enemies, &kits, &grid, &planet).unwrap();
        for key in roster.enemy_keys() {
            assert!(
                roster.enemy(key).muzzles.is_empty(),
                "the template declares no barrels: {}",
                roster.enemy(key).key
            );
        }

        let doctored = enemies.replace(
            "muzzles = []",
            "muzzles = [[0.0, 0.3, 0.8], [0.2, -0.1, 0.6]]",
        );
        let roster = load_template(&doctored, &kits, &grid, &planet).unwrap();
        let e = roster.enemy(roster.enemy_keys().next().unwrap());
        assert_eq!(
            e.muzzles,
            vec![[0.0, 0.3, 0.8], [0.2, -0.1, 0.6]],
            "declared barrels read back in order"
        );
    }

    #[test]
    fn a_degenerate_muzzle_is_a_link_error() {
        let (enemies, kits, grid, planet) = template_parts();
        // A barrel tip farther out than the def's size is a unit mistake
        // (centimetres, wrong frame) — named loudly. template_enemy: size 1.0.
        let far = enemies.replacen("muzzles = []", "muzzles = [[0.0, 0.0, 5.0]]", 1);
        let err = load_template(&far, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("beyond the def's size"), "{err}");

        let broken = enemies.replacen("muzzles = []", "muzzles = [[nan, 0.0, 0.5]]", 1);
        let err = load_template(&broken, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("must be finite"), "{err}");
    }

    #[test]
    fn the_tractor_switch_is_signed_and_defaults_inert() {
        // pull_accel is the one SIGNED switch: positive drags the player
        // toward the enemy, negative shoves away — a repulsor is legal
        // config, so the ≥ 0 rule must not apply. Non-finite still dies.
        let (enemies, kits, grid, planet) = template_parts();

        // Off by default: the template's declared 0.0 resolves to inert.
        let roster = load_template(&enemies, &kits, &grid, &planet).unwrap();
        for key in roster.enemy_keys() {
            assert_eq!(
                roster.enemy(key).behavior.pull_accel, 0.0,
                "no field without a declaration: {}",
                roster.enemy(key).key
            );
        }

        // Declared positive (tractor) and negative (repulsor) both link.
        let pulls = enemies.replace("pull_accel = 0.0", "pull_accel = 6.0");
        let roster = load_template(&pulls, &kits, &grid, &planet).unwrap();
        let e = roster.enemy(roster.enemy_keys().next().unwrap());
        assert_eq!(e.behavior.pull_accel, 6.0, "the declared pull wins");

        let pushes = enemies.replace("pull_accel = 0.0", "pull_accel = -6.0");
        let roster = load_template(&pushes, &kits, &grid, &planet).unwrap();
        let e = roster.enemy(roster.enemy_keys().next().unwrap());
        assert_eq!(e.behavior.pull_accel, -6.0, "a repulsor is legal config");

        // Non-finite is still a config bug.
        let broken = enemies.replace("pull_accel = 0.0", "pull_accel = nan");
        let err = load_template(&broken, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("pull_accel must be finite"), "{err}");
    }

    #[test]
    fn the_alarm_switch_defaults_inert_and_reads_back() {
        let (enemies, kits, grid, planet) = template_parts();
        let roster = load_template(&enemies, &kits, &grid, &planet).unwrap();
        for key in roster.enemy_keys() {
            assert_eq!(
                roster.enemy(key).behavior.alert_radius, 0.0,
                "no klaxon without a declaration: {}",
                roster.enemy(key).key
            );
        }
        let doctored = enemies.replace("alert_radius = 0.0", "alert_radius = 20.0");
        let roster = load_template(&doctored, &kits, &grid, &planet).unwrap();
        let e = roster.enemy(roster.enemy_keys().next().unwrap());
        assert_eq!(e.behavior.alert_radius, 20.0, "the declared radius wins");
    }

    #[test]
    fn the_jink_switch_defaults_inert_and_reads_back() {
        let (enemies, kits, grid, planet) = template_parts();
        let roster = load_template(&enemies, &kits, &grid, &planet).unwrap();
        for key in roster.enemy_keys() {
            assert_eq!(
                roster.enemy(key).behavior.jink_seconds, 0.0,
                "the orbit stays smooth without a declaration: {}",
                roster.enemy(key).key
            );
        }
        let doctored = enemies.replace("jink_seconds = 0.0", "jink_seconds = 0.4");
        let roster = load_template(&doctored, &kits, &grid, &planet).unwrap();
        let e = roster.enemy(roster.enemy_keys().next().unwrap());
        assert_eq!(e.behavior.jink_seconds, 0.4, "the declared cadence wins");
    }

    #[test]
    fn the_cloud_switch_defaults_inert_and_reads_back() {
        let (enemies, kits, grid, planet) = template_parts();
        let roster = load_template(&enemies, &kits, &grid, &planet).unwrap();
        for key in roster.enemy_keys() {
            assert_eq!(
                roster.enemy(key).behavior.cloud_seconds, 0.0,
                "no cloud without a declaration: {}",
                roster.enemy(key).key
            );
        }
        let doctored = enemies.replace("cloud_seconds = 0.0", "cloud_seconds = 6.0");
        let roster = load_template(&doctored, &kits, &grid, &planet).unwrap();
        let e = roster.enemy(roster.enemy_keys().next().unwrap());
        assert_eq!(e.behavior.cloud_seconds, 6.0, "the declared linger wins");
    }

    #[test]
    fn the_guardian_switch_defaults_inert_and_reads_back() {
        let (enemies, kits, grid, planet) = template_parts();
        let roster = load_template(&enemies, &kits, &grid, &planet).unwrap();
        for key in roster.enemy_keys() {
            assert_eq!(
                roster.enemy(key).behavior.guard_radius, 0.0,
                "guards nothing without a declaration: {}",
                roster.enemy(key).key
            );
        }
        let doctored = enemies.replace("guard_radius = 0.0", "guard_radius = 12.0");
        let roster = load_template(&doctored, &kits, &grid, &planet).unwrap();
        let e = roster.enemy(roster.enemy_keys().next().unwrap());
        assert_eq!(e.behavior.guard_radius, 12.0, "the declared radius wins");
    }

    #[test]
    fn weapon_switches_default_to_a_single_ballistic_shot() {
        // Omitting the weapon switches keeps today's exact firing shape —
        // one bolt per cooldown, no fan, no steering. Mechanism on a
        // switch-free fixture, so shipped tuning can't move it.
        let enemies = "[curves.flat]\nkind = \"flat\"\n\n\
            [scaling_defaults]\nspeed = \"flat\"\ncooldown = \"flat\"\nhp = \"flat\"\n\
            damage = \"flat\"\ndetection = \"flat\"\nattack_range = \"flat\"\n\n\
            [[enemy]]\nkey = \"fx_shooter\"\nname = \"H\"\nblurb = \"b\"\nmodel = \"m0\"\n\
            size = 1.0\nyaw_offset_deg = 0\nai = \"shooter\"\nreward = 100\nspawns_directly = true\n\
            [enemy.stats]\nhp = 5.0\nspeed = 9.0\ndamage = 3.0\ndetection = 25.0\nattack_range = 10.0\ncooldown = 1.0\n";
        let models = "[models]\nm0 = \"res://x/m0.glb\"\n";
        let kits = "[kits.k]\nparadigm = \"panel\"\ninstall_dir = \"godot/addons/walls\"\n\
            wall_coverage = 0.9\n";
        let grid = &format!("[kits.k]\ntile = 3.0\nstory = 3.0\n{}", test_census_variants(3.0));
        let planet = "planet = 1\nlevels = 1\nkits = [\"k\"]\nrooms = { base = 6, per_level = 2 }\n\n\
            [[level]]\nrelative = 1\nenemies = [\"fx_shooter\"]\n";
        let roster = load_split(enemies, kits, grid, models, &[planet], EnvSources::generated_only()).unwrap();
        let b = roster.enemy(roster.enemy_key("fx_shooter").unwrap()).behavior;
        assert_eq!(b.burst_count, 1, "one bolt per trigger pull by default");
        assert!(b.burst_seconds > 0.0, "the in-burst gap is a real interval");
        assert_eq!(b.pellet_count, 1, "no fan by default");
        assert_eq!(b.spread_deg, 0.0, "no spread by default");
        assert_eq!(b.bolt_turn_deg, 0.0, "ballistic by default");
    }

    #[test]
    fn a_declared_weapon_switch_beats_its_default() {
        let (enemies, kits, grid, planet) = template_parts();
        // Fan and steer rate are mutually exclusive, so prove them on two
        // separate doctorings of the template.
        let fanned = enemies
            .replace("burst_count = 1", "burst_count = 4")
            .replace("burst_seconds = 0.1", "burst_seconds = 0.05")
            .replace("pellet_count = 1", "pellet_count = 6")
            .replace("spread_deg = 0.0", "spread_deg = 18.0");
        let roster = load_template(&fanned, &kits, &grid, &planet).unwrap();
        let b = roster.enemy(roster.enemy_keys().next().unwrap()).behavior;
        assert_eq!(b.burst_count, 4, "the declared burst wins");
        assert_eq!(b.burst_seconds, 0.05, "the declared gap wins");
        assert_eq!(b.pellet_count, 6, "the declared fan wins");
        assert_eq!(b.spread_deg, 18.0, "the declared cone wins");

        let homing = enemies.replace("bolt_turn_deg = 0.0", "bolt_turn_deg = 120.0");
        let roster = load_template(&homing, &kits, &grid, &planet).unwrap();
        let b = roster.enemy(roster.enemy_keys().next().unwrap()).behavior;
        assert_eq!(b.bolt_turn_deg, 120.0, "the declared steer rate wins");
    }

    #[test]
    fn a_zero_weapon_count_is_a_link_error() {
        // Counts are how MANY bolts exist, not whether the enemy fires —
        // that is the archetype's call. Zero is a config bug, named loudly.
        let (enemies, kits, grid, planet) = template_parts();
        let doctored = enemies.replace("burst_count = 1", "burst_count = 0");
        let err = load_template(&doctored, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("burst_count must be ≥ 1"), "{err}");
        let doctored = enemies.replace("pellet_count = 1", "pellet_count = 0");
        let err = load_template(&doctored, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("pellet_count must be ≥ 1"), "{err}");
    }

    #[test]
    fn a_fanned_homing_bolt_is_a_link_error() {
        // Pellets fly ballistic — a def declares a fan OR a steer rate,
        // never both. The linker names the conflict, not the fire site.
        let (enemies, kits, grid, planet) = template_parts();
        let enemies = enemies
            .replace("pellet_count = 1", "pellet_count = 6")
            .replace("bolt_turn_deg = 0.0", "bolt_turn_deg = 90.0");
        let err = load_template(&enemies, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("pellets fly ballistic"), "{err}");
    }

    #[test]
    fn a_negative_weapon_switch_is_a_link_error() {
        let (enemies, kits, grid, planet) = template_parts();
        let enemies = enemies.replace("bolt_turn_deg = 0.0", "bolt_turn_deg = -90.0");
        let err = load_template(&enemies, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("bolt_turn_deg must be finite"), "{err}");
    }

    #[test]
    fn a_slot_boss_may_field_timed_emitters_but_never_broods() {
        // Sustained pressure is the def's own character (owner 2026-07-05:
        // the Brute fields a spawn ring); one-shot broods stay forbidden —
        // they'd double-dip against the slot's declared escorts.
        let (enemies, kits, grid, planet) = template_parts();
        // A timed emitter on the slot-staged boss links.
        let emitter = enemies.replace(
            "key = \"template_boss\"",
            "key = \"template_boss\"\nminions = [{ enemy = \"template_minion\", count = 1, trigger = { every_seconds = 5.0 }, cap = 3 }]",
        );
        assert!(
            load_template(&emitter, &kits, &grid, &planet).is_ok(),
            "a timed emitter on a slot-staged boss must link"
        );
        // A one-shot brood does not — it would double-dip the slot's escorts.
        let brood = enemies.replace(
            "key = \"template_boss\"",
            "key = \"template_boss\"\nminions = [{ enemy = \"template_minion\", count = 1, trigger = \"on_death\" }]",
        );
        let err = load_template(&brood, &kits, &grid, &planet).unwrap_err();
        assert!(
            err.contains("one-shot brood"),
            "a brood on a slot-staged boss must be a link error naming the rule: {err}"
        );
    }

    #[test]
    fn a_zero_escort_count_is_a_link_error() {
        let (enemies, kits, grid, planet) = template_parts();
        let planet = planet.replace("count = 3", "count = 0");
        let err = load_template(&enemies, &kits, &grid, &planet).unwrap_err();
        assert!(
            err.contains("escort count must be at least 1"),
            "the linker must name the zero-count rule: {err}"
        );
    }

    #[test]
    fn an_unknown_key_is_rejected_at_the_boundary() {
        // The one string→identity door: an undeclared key is `None`, never a
        // panic. (The internal `enemy(EnemyKey)` demand door can't be reached
        // with a key from this grammar — the type is proof of existence.)
        assert!(loaded().enemy_key("pretty_pretty_princess").is_none());
    }

    #[test]
    fn every_kit_install_dir_is_populated() {
        // `make assets` must populate what kits.toml claims (owner
        // 2026-07-05) — run it first on a fresh checkout. Stage 3's probe
        // deepens this from "directory has content" to measured dimensions.
        let repo = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        for kit in loaded().catalog.kits() {
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
        let (enemies, kits, grid, planet) = template_parts();
        let enemies = enemies.replacen("yaw_offset_deg", "yaw_offest_deg", 1);
        let err = load_template(&enemies, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("yaw_offest_deg"), "typo'd field must be named: {err}");
    }

    #[test]
    fn an_unresolved_reference_is_a_link_error() {
        let (enemies, kits, grid, planet) = template_parts();
        let enemies = enemies.replacen(
            "{ enemy = \"template_minion\", count = 2, trigger = \"on_death\" }",
            "{ enemy = \"template_mnion\", count = 2, trigger = \"on_death\" }",
            1,
        );
        let err = load_template(&enemies, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("template_mnion"), "bad ref must be named: {err}");
    }

    #[test]
    fn a_duplicated_key_is_a_link_error() {
        // Two blocks, one key — the loader rejects it (the key is the sole
        // identity; duplicate keys are the only collision that exists).
        let (enemies, kits, grid, planet) = template_parts();
        let enemies = enemies.replacen("key = \"template_minion\"", "key = \"template_enemy\"", 1);
        let err = load_template(&enemies, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("duplicate enemy key"), "{err}");
    }

    #[test]
    fn a_declared_length_without_a_roster_is_a_link_error() {
        // Owner 2026-07-05: declaring more levels than you populate is a link
        // error — both directions.
        let (enemies, kits, grid, planet) = template_parts();
        let planet = planet.replace("levels = 2", "levels = 3");
        let err = load_template(&enemies, &kits, &grid, &planet).unwrap_err();
        assert!(
            err.contains("relative 3 has no"),
            "the unpopulated level must be named: {err}"
        );
    }

    #[test]
    fn a_roster_outside_the_declared_length_is_a_link_error() {
        let (enemies, kits, grid, planet) = template_parts();
        let planet = planet.replacen("relative = 2", "relative = 3", 1);
        let err = load_template(&enemies, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("outside the declared"), "{err}");
        assert!(
            err.contains("relative 2 has no"),
            "the hole it left must be named too: {err}"
        );
    }

    #[test]
    fn a_level_listing_a_non_spawning_enemy_is_a_link_error() {
        // template_minion is minion-only (spawns_directly = false) — a level
        // roster naming it lies.
        let (enemies, kits, grid, planet) = template_parts();
        let planet = planet.replacen(
            "enemies = [\"template_enemy\"]",
            "enemies = [\"template_minion\"]",
            1,
        );
        let err = load_template(&enemies, &kits, &grid, &planet).unwrap_err();
        assert!(err.contains("never spawns directly"), "{err}");
    }
}
