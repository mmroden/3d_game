//! The CATALOG's TOML schema: the hand-authored kit manifest
//! (catalog/kits.toml) and the three probe-generated catalogs
//! (kits/models/environments .generated.toml). Moved out of the roster
//! grammar 2026-07-18 (Amendment A): the roster is the bestiary; assets
//! are the catalog's domain. The roster references catalog entries by
//! KEY — planets name kits, enemies name models — and the linker joins
//! across that boundary, never through shared types beyond these.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]

pub struct KitsFile {
    pub kits: BTreeMap<String, KitRaw>,
    /// The panoramas a fixed environment's openings look out on, by
    /// key; a fixed kit names one in `sky`. Optional: a catalog without
    /// skies leaves the shell's authored environment in place.
    #[serde(default)]
    pub skies: BTreeMap<String, SkyRaw>,
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
    /// PANEL kits only: the wall-pool policy — the minimum probe-measured
    /// face coverage for a piece to serve as a WALL (a solid plate rates
    /// 1.0, a truss frame well under 0.5). Membership itself derives from
    /// the census in kits.generated.toml; only this bar is authored.
    /// Required iff `paradigm = "panel"`.
    pub wall_coverage: Option<f32>,
    /// PANEL kits only: PIECES whose artist front is NOT their
    /// relief-heavy side — the authored escape hatch for the one case
    /// the relief measure is structurally blind to. Per piece, never
    /// per kit: a kit-wide bit silently inverts the exceptions (the
    /// marble backs of 2026-07-19).
    #[serde(default)]
    pub detail_flip: Vec<String>,
    /// FIXED kits only: world units per authored model meter — the pitch of
    /// the environment's 1-meter zone grid. The one deliberate exception to
    /// "the probe derives all grids": a fixed scene has no recipe to derive
    /// from; scale is a design knob. Required iff `paradigm = "fixed"`.
    #[serde(default)]
    pub scale: Option<f32>,
    /// FIXED kits only: the environment this kit builds, a key into BOTH
    /// rosters/environments/*.toml (the authored zones) and
    /// environments.generated.toml (the installed scene). Required iff
    /// `paradigm = "fixed"`.
    #[serde(default)]
    pub environment: Option<String>,
    /// FIXED kits only, optional: the sky its openings look out on — a
    /// key into `[skies]`. Absent, the shell keeps the environment
    /// main.tscn authored.
    #[serde(default)]
    pub sky: Option<String>,
}


/// One sky: an equirectangular panorama the fixed levels' world
/// environment shows behind every opening (owner 2026-09-08: "the
/// exterior being a skyscape goes with the story"; the NASA Deep Star
/// Maps 2020, galactic frame). `texture` is the installed res:// path
/// (`make assets` copies assets/sky/ into godot/addons/sky/);
/// `attribution` joins catalog/attributions.toml, and the asset audit
/// holds every sky to a credited, checksum-pinned source.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkyRaw {
    pub texture: String,
    #[serde(default)]
    pub attribution: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KitParadigm {
    Layered,
    Panel,
    /// A pre-modeled environment scene installed whole (planet 3's
    /// apartment): no grid derivation, no per-cell skinning — the kit
    /// declares its `environment` and `scale` instead.
    Fixed,
}

/// catalog/models.generated.toml: model key (file stem) → res:// path.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelsFile {
    pub models: BTreeMap<String, String>,
}

/// catalog/environments.generated.toml: environment key (file stem) →
/// res:// path. A separate namespace from [`ModelsFile`] on purpose: that
/// catalog's contract is "every installed ENEMY model", and enemy defs
/// resolve against it — mixing the two would let an enemy reference the
/// planet-3 house as its body.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentsFile {
    pub environments: BTreeMap<String, String>,
}

/// The grid snap tolerance, metres: a measured extent within this of the
/// module IS the module (split panels decimate a hair under; wall sets
/// carry cm-scale decorative lips). The ONE home — pool derivation, the
/// bake qualifier, and the probe's grid agreement all snap with it.
/// scripts/split-panels.py mirrors the value (Python can't import it);
/// the probe's bake contract audits the mirror at `make assets`.
pub const GRID_SNAP: f32 = 0.05;

/// catalog/kits.generated.toml: each kit's grid, DERIVED by the probe from
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
    /// Panel kits, the per-piece census: the probe's raw measurement of
    /// each authored solid. Links nothing (v1 retired 2026-07-19) but
    /// deliberately survives — pitch derivation and bake qualification
    /// read it, and `a_pieces_census_beside_variants_changes_nothing`
    /// pins that it stays inert at link.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pieces: BTreeMap<String, GeneratedPieceRaw>,
    /// Panel kits, census v2 (see [`GeneratedVariantRaw`]): the BAKED
    /// role variants the linker derives role pools from. MEASURED and
    /// declared by the pipeline, never authored.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variants: BTreeMap<String, GeneratedVariantRaw>,
}

/// A world axis — which one a censused plate is THIN along, as authored.
/// The panel assembler's rotations turn ONE native pose (plate in XZ,
/// thin along `y`) into all six cell faces; a plate authored thin along
/// any other axis places 90° off under those same rotations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinAxis {
    X,
    Y,
    Z,
}

/// One installed kit piece, as measured (see [`GeneratedKitRaw::pieces`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedPieceRaw {
    /// Plate face extents, larger first (the two largest world extents).
    pub face: [f32; 2],
    /// Extent along the plate's thin axis.
    pub thick: f32,
    /// The world axis the plate is thin along, AS AUTHORED. The panel
    /// assembler poses every plate assuming [`ThinAxis::Y`] (plate in
    /// XZ, normal +Y); the census records the measured fact that axiom
    /// rests on, so it is checkable — never assumed.
    pub axis: ThinAxis,
    /// Fraction of the plate face covered by geometry: 1.0 = a solid
    /// plate, a truss frame reads well below it.
    pub coverage: f32,
    /// Confidence of the detail verdict: the relief-mass asymmetry
    /// about the base slab, 0 = symmetric (facing unmeasurable — author
    /// it in kits.toml `detail_flip` if it matters) to 1 = one-sided.
    /// Absent in pre-relief censuses.
    #[serde(default)]
    pub relief: f32,
    pub tris: u32,
    pub textures: u8,
    /// Strongest metallic factor across the file's materials. With no
    /// metallic-roughness map, factors alone drive shading — the shiny/
    /// matte question is answered here, not in Blender.
    #[serde(default)]
    pub metallic: f32,
    /// Smoothest roughness factor across the file's materials.
    #[serde(default = "one")]
    pub roughness: f32,
    /// Whether any material carries a metallic-roughness texture.
    #[serde(default)]
    pub mr_map: bool,
    /// Mean of the metallic map's blue channel (the shiniest material's;
    /// effective metal = factor × this). 1.0 with no map.
    #[serde(default = "one")]
    pub metallic_px: f32,
    /// Mean of the roughness map's green channel (the smoothest
    /// material's; effective roughness = factor × this). 1.0 with no map.
    #[serde(default = "one")]
    pub rough_px: f32,
}

fn one() -> f32 {
    1.0
}

impl GeneratedPieceRaw {
    /// Does this piece qualify for the v2 ROLE BAKE? Solid, with its
    /// SHORTER face extent on the module — the plate then serves as a
    /// course segment in any role (the free extent is the width the
    /// coverer partitions with). Wider than v1's unitary filter above,
    /// which dies with it. The Transform mirrors this to decide what to
    /// bake; the probe's bake contract audits the mirror.
    pub fn qualifies_for_role_bake(&self, module: f32, bar: f32) -> bool {
        (self.face[1] - module).abs() <= GRID_SNAP && self.coverage >= bar
    }
}

/// A variant's construction ROLE — what the Transform baked it to be.
/// Floor/ceiling/wall variants skin surfaces; decorations and add-ons
/// layer onto them (colliderless). Declared by the conversion, verified
/// against the probe's measurements by the linker and the probe's
/// contract tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VariantRole {
    Floor,
    Ceiling,
    Wall,
    Decoration,
    Addon,
}

/// One BAKED library variant (census v2): a role-oriented export of one
/// or more source pieces (several = a metric-completion preassembly).
/// `face`/`thick`/`axis`/`detail`/`coverage` are probe MEASUREMENTS;
/// `sources`/`role`/`stretch` are conversion DECLARATIONS — the linker
/// cross-checks the two so a mis-baked variant fails the load, loudly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedVariantRaw {
    /// Source piece stems this variant was baked from (>1 = preassembly).
    pub sources: Vec<String>,
    pub role: VariantRole,
    /// Face extents, larger first (the two largest world extents).
    pub face: [f32; 2],
    /// Extent along the thin axis.
    pub thick: f32,
    /// Measured thin axis — floors/ceilings bake to `y`, walls to `z`.
    pub axis: ThinAxis,
    /// Measured direction the authored detail faces — floors bake to
    /// `pos_y`, ceilings to `neg_y`, walls to `pos_z` (yaw at placement).
    pub detail: crate::room_template::ConnectorFacing,
    /// Confidence of the detail verdict (relief-mass asymmetry; see
    /// [`GeneratedPieceRaw::relief`]). Absent in pre-relief censuses.
    #[serde(default)]
    pub relief: f32,
    /// Fraction of the face covered by geometry (1.0 = solid plate).
    pub coverage: f32,
    /// Stretch the conversion applied to reach the module, as a fraction
    /// of the original extent (0 = none). Bounded by authored policy.
    pub stretch: f32,
    pub tris: u32,
    pub textures: u8,
}


/// The Transform's manifest (`<install_dir>/manifest.toml`, written by
/// scripts/split-panels.py): DECLARES each baked variant — role,
/// sources, applied stretch. The probe MEASURES the same files; the
/// census carries declaration + measurement side by side and the linker
/// cross-checks them, so a bake the Transform got wrong fails the load.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransformManifest {
    /// Empty is legal: a kit with nothing on-module bakes nothing.
    #[serde(default)]
    pub variants: BTreeMap<String, ManifestVariantRaw>,
    /// The Transform's PERSISTENT per-source front-flip state — its own
    /// memory across bakes (without it the census-feedback correction
    /// oscillates with period two; bake 8, 2026-07-19). The probe never
    /// reads this; it carries it for the next Transform run.
    #[serde(default)]
    pub flips: BTreeMap<String, bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestVariantRaw {
    pub sources: Vec<String>,
    pub role: VariantRole,
    pub stretch: f32,
}
