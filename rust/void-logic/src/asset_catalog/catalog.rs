//! The linked ASSET CATALOG (Amendment A): kits joined to their probed
//! grids and pools, plus the installed model and environment-scene
//! catalogs. Owns every string it serves. The roster (bestiary/planets)
//! references catalog entries by KEY — planets name kits, enemies name
//! models, authored environments name scenes — and resolves those keys
//! against this API at ITS link. Nothing crosses the boundary except
//! keys in and lookups out.

use std::collections::BTreeMap;

use super::schema::{self, KitParadigm};
use super::RolePools;

/// Index of a kit in the catalog — the roster's planets store these
/// after resolving their declared kit keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KitId(pub usize);

/// An interned scene in the catalog. 4 bytes, Copy, Eq, Hash. The ONLY
/// minting path is the catalog's linker — no public constructor, no
/// `From<&str>`: a SceneId in hand PROVES the scene resolved against the
/// catalog at link time. Resolve back with [`AssetCatalog::path`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneId(u32);

/// Every scene GAME CODE must name (as opposed to the open-set pools and
/// model maps, which code reaches by role or key). Closed set: adding a
/// fixture is a compile-visible event, and the catalog's one internal
/// table maps each to its installed path — audited by a probe contract,
/// never folklore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fixture {
    DoorFrame,
}

/// The one place a LOGIC-placed code-known scene path is written.
/// Audited by the probe's `fixture_scenes_are_installed` contract:
/// every entry must resolve to an installed file, or `make assets`
/// fails. (The shell's cosmetic scene consts — jump gate, portal,
/// bestiary models — live in void-nodes `scenes::*` and never flow
/// through placements; folding them in here is an OPEN scope call,
/// not assumed.)
pub(super) const FIXTURE_PATHS: &[(Fixture, &str)] = &[(Fixture::DoorFrame, super::DOOR)];

/// One linked kit: the authored manifest joined to the probe's derived
/// grid and plate library.
#[derive(Debug)]
pub struct KitDef {
    pub key: String,
    pub paradigm: KitParadigm,
    /// The grid: probed for layered/panel kits; a fixed kit's declared
    /// scale IS its tile/story (no recipe to derive from).
    pub tile: f32,
    pub story: f32,
    pub install_dir: String,
    /// FIXED kits: the authored environment KEY. The catalog does not
    /// know the roster's authored zone maps — the roster resolves this
    /// key at its own link (the one deliberate cross-boundary join).
    pub environment: Option<String>,
    /// PANEL kits: role-typed pools from the baked variants — the one
    /// plate library. Owned; plates carry interned ids.
    pub role_pools: Option<RolePools>,
}

/// The load-time scene interner: builds the arena the catalog serves
/// from. Private — ids only ever come out of a finished catalog.
struct Interner {
    paths: Vec<String>,
    index: BTreeMap<String, u32>,
}

impl Interner {
    fn new() -> Self {
        Self { paths: Vec::new(), index: BTreeMap::new() }
    }

    fn intern(&mut self, s: &str) -> SceneId {
        if let Some(&i) = self.index.get(s) {
            return SceneId(i);
        }
        let i = u32::try_from(self.paths.len()).expect("scene arena fits u32");
        self.paths.push(s.to_string());
        self.index.insert(s.to_string(), i);
        SceneId(i)
    }
}

/// The linked catalog. One instance per process in production (see
/// [`crate::asset_catalog::catalog()`]); fixtures build their own.
#[derive(Debug)]
pub struct AssetCatalog {
    kits: Vec<KitDef>,
    kit_index: BTreeMap<String, usize>,
    /// Enemy-model key → installed res:// path.
    models: BTreeMap<String, String>,
    /// Environment key → installed scene res:// path. A separate
    /// namespace from `models` on purpose: that catalog's contract is
    /// "every installed ENEMY model" — mixing the two would let an enemy
    /// reference the planet-3 house as its body.
    environments: BTreeMap<String, String>,
    /// The scene-path arena: every string a [`SceneId`] resolves to.
    /// Owned — consumers borrow at spawn and let go; nothing leaks.
    paths: Vec<String>,
    scene_index: BTreeMap<String, u32>,
    fixtures: Vec<(Fixture, SceneId)>,
}

impl AssetCatalog {
    /// Parse and link the four catalog sources. Errors collect — a bad
    /// catalog reports every violation at once, like the roster's linker.
    pub fn load(
        kits_toml: &str,
        kit_grids_toml: &str,
        models_toml: &str,
        environments_toml: &str,
    ) -> Result<Self, Vec<String>> {
        let mut errors: Vec<String> = Vec::new();

        let kits_file: Option<schema::KitsFile> = match toml::from_str(kits_toml) {
            Ok(f) => Some(f),
            Err(e) => {
                errors.push(format!("kits.toml: {e}"));
                None
            }
        };
        let kit_grids_file: Option<schema::GeneratedKitsFile> =
            match toml::from_str(kit_grids_toml) {
                Ok(f) => Some(f),
                Err(e) => {
                    errors.push(format!("kits.generated.toml: {e}"));
                    None
                }
            };
        let models_file: Option<schema::ModelsFile> = match toml::from_str(models_toml) {
            Ok(f) => Some(f),
            Err(e) => {
                errors.push(format!("models.generated.toml: {e}"));
                None
            }
        };
        let environments_file: Option<schema::EnvironmentsFile> =
            match toml::from_str(environments_toml) {
                Ok(f) => Some(f),
                Err(e) => {
                    errors.push(format!("environments.generated.toml: {e}"));
                    None
                }
            };

        let (Some(kits_file), Some(kit_grids_file), Some(models_file), Some(environments_file)) =
            (kits_file, kit_grids_file, models_file, environments_file)
        else {
            return Err(errors);
        };

        // The arena starts with every CONST scene the game can place —
        // wall sets, the door fixture, props, lights — so `id_of` is
        // total over authored tables, then grows with each pool.
        let mut interner = Interner::new();
        let fixtures: Vec<(Fixture, SceneId)> = FIXTURE_PATHS
            .iter()
            .map(|&(f, p)| (f, interner.intern(p)))
            .collect();
        for ws in super::ALL_WALL_SETS {
            for s in ws.scenes() {
                interner.intern(s);
            }
        }
        for s in super::props::all_scenes().chain(super::lights::all_scenes()) {
            interner.intern(s);
        }
        for path in environments_file.environments.values() {
            interner.intern(path);
        }

        // Kits (BTreeMap: TOML rejects re-declared kit tables). The grid
        // joins in from the probe's derived measurements — never authored.
        // FIXED kits are the one deliberate exception: a fixed scene has
        // no recipe to derive a grid from, so its declared scale IS its
        // tile/story, and it names an authored environment instead.
        let kits: Vec<KitDef> = kits_file
            .kits
            .iter()
            .map(|(key, raw)| {
                if raw.paradigm == KitParadigm::Fixed {
                    if raw.wall_coverage.is_some() {
                        errors.push(format!("kit '{key}': wall_coverage is a panel-kit knob"));
                    }
                    let scale = match raw.scale {
                        Some(s) if s.is_finite() && s > 0.0 => s,
                        Some(s) => {
                            errors.push(format!(
                                "kit '{key}': scale must be finite and > 0, got {s}"
                            ));
                            1.0
                        }
                        None => {
                            errors.push(format!(
                                "kit '{key}': a fixed kit must declare scale \
                                 (world units per model meter)"
                            ));
                            1.0
                        }
                    };
                    if raw.environment.is_none() {
                        errors.push(format!(
                            "kit '{key}': a fixed kit must declare its environment \
                             (the authored zone map it builds)"
                        ));
                    }
                    return KitDef {
                        key: key.clone(),
                        paradigm: raw.paradigm,
                        tile: scale,
                        story: scale,
                        install_dir: raw.install_dir.clone(),
                        environment: raw.environment.clone(),
                        role_pools: None,
                    };
                }
                for (field, present) in [
                    ("scale", raw.scale.is_some()),
                    ("environment", raw.environment.is_some()),
                ] {
                    if present {
                        errors.push(format!(
                            "kit '{key}': {field} is a fixed-kit knob — a generated \
                             kit derives its grid from the probe"
                        ));
                    }
                }
                let grid = match kit_grids_file.kits.get(key) {
                    Some(g) => g.clone(),
                    None => {
                        errors.push(format!(
                            "kit '{key}': no derived grid in \
                             catalog/kits.generated.toml (run `make assets`)"
                        ));
                        schema::GeneratedKitRaw {
                            tile: 0.0,
                            story: 0.0,
                            pieces: Default::default(),
                            variants: Default::default(),
                        }
                    }
                };
                // The kit's plate library: the baked variants and only
                // them. The pieces census survives for the probe's own
                // uses (pitch derivation, bake qualification) but links
                // nothing — v1 retired 2026-07-19.
                let role_pools = if grid.variants.is_empty() {
                    None
                } else if raw.paradigm != KitParadigm::Panel {
                    errors.push(format!(
                        "kit '{key}': a variants census is a panel-kit artifact"
                    ));
                    None
                } else {
                    derive_role_pools(
                        key,
                        &raw.install_dir,
                        grid.tile,
                        grid.story,
                        raw.wall_coverage.unwrap_or(1.0),
                        &grid.variants,
                        &mut interner,
                        &mut errors,
                    )
                };
                match (raw.paradigm, raw.wall_coverage) {
                    (KitParadigm::Panel, Some(bar))
                        if bar.is_finite() && (0.0..=1.0).contains(&bar) =>
                    {
                        // The retirement contract: a panel kit links
                        // through its baked role pools alone — a census
                        // without pooled variants is a dead kit, loudly.
                        if role_pools.is_none() {
                            errors.push(format!(
                                "kit '{key}': no pooled role variants — the \
                                 Transform bakes them (`make assets`)"
                            ));
                        }
                    }
                    (KitParadigm::Panel, Some(bar)) => {
                        errors.push(format!(
                            "kit '{key}': wall_coverage must be a fraction in 0..=1, got {bar}"
                        ));
                    }
                    (KitParadigm::Panel, None) => {
                        errors.push(format!(
                            "kit '{key}': a panel kit must declare wall_coverage \
                             (the wall pool's minimum censused face coverage)"
                        ));
                    }
                    (_, Some(_)) => {
                        errors.push(format!("kit '{key}': wall_coverage is a panel-kit knob"));
                    }
                    (_, None) => {}
                }
                KitDef {
                    key: key.clone(),
                    paradigm: raw.paradigm,
                    tile: grid.tile,
                    story: grid.story,
                    install_dir: raw.install_dir.clone(),
                    environment: None,
                    role_pools,
                }
            })
            .collect();

        if !errors.is_empty() {
            return Err(errors);
        }
        let kit_index = kits
            .iter()
            .enumerate()
            .map(|(i, k)| (k.key.clone(), i))
            .collect();

        let Interner { paths, index: scene_index } = interner;
        Ok(Self {
            kits,
            kit_index,
            models: models_file.models,
            environments: environments_file.environments,
            paths,
            scene_index,
            fixtures,
        })
    }

    /// Resolve an id this catalog minted to its res:// path. Borrow —
    /// the string stays owned here.
    pub fn path(&self, id: SceneId) -> &str {
        &self.paths[id.0 as usize]
    }

    /// The scene behind a code-known fixture. Infallible: the table is
    /// closed and interned at load.
    pub fn fixture(&self, f: Fixture) -> SceneId {
        self.fixtures
            .iter()
            .find(|&&(x, _)| x == f)
            .map(|&(_, id)| id)
            .expect("every Fixture has a table entry")
    }

    /// Look up an already-interned path.
    pub(crate) fn id_of(&self, path: &str) -> Option<SceneId> {
        self.scene_index.get(path).map(|&i| SceneId(i))
    }

    /// The id of an authored CONST scene (wall sets, props, lights).
    /// Total by construction: load walks every authored table into the
    /// arena — a miss means a table was added without joining the walk,
    /// and the panic names it.
    pub(crate) fn const_scene(&self, path: &'static str) -> SceneId {
        self.id_of(path).unwrap_or_else(|| {
            panic!("scene '{path}' not interned — add its table to the catalog's load walk")
        })
    }

    /// Resolve a kit KEY (a planet's declaration) to its id + def.
    pub fn kit(&self, key: &str) -> Option<(KitId, &KitDef)> {
        self.kit_index.get(key).map(|&i| (KitId(i), &self.kits[i]))
    }

    /// The kit behind an id this catalog minted.
    pub fn kit_def(&self, id: KitId) -> &KitDef {
        &self.kits[id.0]
    }

    /// Every linked kit, in manifest order (install pins and contract
    /// tests audit the full set).
    pub fn kits(&self) -> impl Iterator<Item = &KitDef> {
        self.kits.iter()
    }

    /// Resolve an enemy-model KEY to its installed res:// path.
    pub fn model(&self, key: &str) -> Option<&str> {
        self.models.get(key).map(String::as_str)
    }

    /// Every installed enemy model (contract tests audit against disk).
    pub fn models(&self) -> impl Iterator<Item = (&str, &str)> {
        self.models.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Resolve an environment KEY to its installed scene res:// path.
    pub fn environment_scene(&self, key: &str) -> Option<&str> {
        self.environments.get(key).map(String::as_str)
    }
}

/// Derive a panel kit's ROLE POOLS from its census-v2 variants: cross-
/// check each variant's declared role against its measured pose (a bake
/// the Transform got wrong must fail the load, not skin a room sideways),
/// normalize wall faces to [width, height] on the story module, and
/// enforce the universal-filler rule per surface role — the coverer's
/// completion guarantee.
fn derive_role_pools(
    key: &str,
    install_dir: &str,
    tile: f32,
    story: f32,
    bar: f32,
    variants: &BTreeMap<String, schema::GeneratedVariantRaw>,
    interner: &mut Interner,
    errors: &mut Vec<String>,
) -> Option<RolePools> {
    use super::RolePlate;
    use crate::room_template::ConnectorFacing;
    use schema::{ThinAxis, VariantRole};

    /// The probe's snap tolerance: extents this close to the module ARE it.
    const SNAP: f32 = 0.05;

    let res_dir = install_dir.trim_start_matches("godot/");
    let (mut floor, mut ceiling, mut wall) = (Vec::new(), Vec::new(), Vec::new());
    let (mut decoration, mut addon) = (Vec::new(), Vec::new());
    let before = errors.len();
    for (stem, v) in variants {
        if v.sources.is_empty() {
            errors.push(format!("kit '{key}' variant '{stem}': sources is empty"));
        }
        if !v.stretch.is_finite() || !(0.0..=1.0).contains(&v.stretch) {
            errors.push(format!(
                "kit '{key}' variant '{stem}': stretch {} outside 0..=1",
                v.stretch
            ));
            continue;
        }
        // Pool membership = census × policy, same principle as v1: the
        // Transform's bake filter is best-effort (its analytic coverage
        // overbaked a truss, 2026-07-19); the probe's rasterizer is the
        // ONE truth, and a variant under the bar is simply not pooled —
        // an unused file, never a see-through wall.
        if matches!(
            v.role,
            VariantRole::Floor | VariantRole::Ceiling | VariantRole::Wall
        ) && v.coverage < bar
        {
            continue;
        }
        // Surface roles have exactly one legal baked pose; decorations
        // and add-ons layer in any pose.
        let expected_pose = match v.role {
            VariantRole::Floor => Some((ThinAxis::Y, ConnectorFacing::PosY)),
            VariantRole::Ceiling => Some((ThinAxis::Y, ConnectorFacing::NegY)),
            VariantRole::Wall => Some((ThinAxis::Z, ConnectorFacing::PosZ)),
            VariantRole::Decoration | VariantRole::Addon => None,
        };
        // A mis-baked pose is NOT POOLED — same census × policy stance
        // as the coverage bar: the game never becomes hostage to a
        // Transform bug (2026-07-19: every variant of a whole bake
        // measured identical, and pose-as-link-error bricked the entire
        // grammar). The LOUD failure lives at `make assets`: the probe's
        // bake contract asserts pose per variant.
        if let Some((axis, detail)) = expected_pose {
            if v.axis != axis || v.detail != detail {
                continue;
            }
        }
        // Surface plates normalize to [width, module]: the module extent
        // (story for walls, tile for floor/ceiling strips) is the course
        // DEPTH; the other is the width the coverer partitions with. A
        // plate with neither extent on its module cannot course.
        let module = match v.role {
            VariantRole::Wall => Some(("story", story)),
            VariantRole::Floor | VariantRole::Ceiling => Some(("tile", tile)),
            VariantRole::Decoration | VariantRole::Addon => None,
        };
        let face = match module {
            None => v.face,
            Some((name, m)) => {
                if (v.face[1] - m).abs() <= SNAP {
                    [v.face[0], v.face[1]]
                } else if (v.face[0] - m).abs() <= SNAP {
                    [v.face[1], v.face[0]]
                } else {
                    errors.push(format!(
                        "kit '{key}' variant '{stem}': {:?} face [{}, {}] has \
                         no extent on the {name} module {m}",
                        v.role, v.face[0], v.face[1]
                    ));
                    continue;
                }
            }
        };
        let plate = RolePlate {
            scene: interner.intern(&format!("res://{res_dir}/{stem}.glb")),
            thick: v.thick,
            face,
        };
        match v.role {
            VariantRole::Floor => floor.push(plate),
            VariantRole::Ceiling => ceiling.push(plate),
            VariantRole::Wall => wall.push(plate),
            VariantRole::Decoration => decoration.push(plate),
            VariantRole::Addon => addon.push(plate),
        }
    }
    if floor.is_empty()
        && ceiling.is_empty()
        && wall.is_empty()
        && decoration.is_empty()
        && addon.is_empty()
    {
        // Nothing survived the census × policy filter: the kit has no v2
        // library (yet). The probe's bake contract at `make assets` is
        // the loud gate for WHY; during migration the v1 pool still
        // carries the kit, so this is not a link error.
        return None;
    }
    // Universal fillers: every surface pool carries a 1-module plate, so
    // the coverer can ALWAYS complete and larger plates stay pure upside.
    for (role, pool, want) in [
        ("floor", &floor, [tile, tile]),
        ("ceiling", &ceiling, [tile, tile]),
        ("wall", &wall, [tile, story]),
    ] {
        if !pool.iter().any(|p| {
            (p.face[0] - want[0]).abs() <= SNAP && (p.face[1] - want[1]).abs() <= SNAP
        }) {
            errors.push(format!(
                "kit '{key}': {role} pool has no 1-module filler (a {}×{} \
                 plate) — the coverer's completion guarantee",
                want[0], want[1]
            ));
        }
    }
    if errors.len() > before {
        return None;
    }
    Some(RolePools { id: key.to_owned(), floor, ceiling, wall, decoration, addon })
}
