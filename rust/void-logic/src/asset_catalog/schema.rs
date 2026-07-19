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
    /// Panel kits, census v1 (MIGRATION — dies with Transform v2): the
    /// per-piece census the legacy one-pool assembler links from.
    /// `variants` wins when present.
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
    pub tris: u32,
    pub textures: u8,
}

impl GeneratedPieceRaw {
    /// Does this piece qualify for a panel kit's WALL POOL? Square
    /// (≤1%), cell-sized (within the probe's 5 cm snap of `tile`), and
    /// covered to at least the kit's authored `wall_coverage` bar —
    /// solid plates wall, truss frames and strips furnish. The one
    /// membership filter: the linker derives pools with it and the
    /// probe's contract tests audit against it.
    pub fn qualifies_as_wall(&self, tile: f32, bar: f32) -> bool {
        let square = (self.face[0] - self.face[1]) <= 0.01 * self.face[0];
        let cell_sized = (self.face[0] - tile).abs() <= 0.05;
        square && cell_sized && self.coverage >= bar
    }

    /// Does this piece qualify for the v2 ROLE BAKE? Solid, with its
    /// SHORTER face extent on the module — the plate then serves as a
    /// course segment in any role (the free extent is the width the
    /// coverer partitions with). Wider than v1's unitary filter above,
    /// which dies with it. The Transform mirrors this to decide what to
    /// bake; the probe's bake contract audits the mirror.
    pub fn qualifies_for_role_bake(&self, module: f32, bar: f32) -> bool {
        (self.face[1] - module).abs() <= 0.05 && self.coverage >= bar
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
