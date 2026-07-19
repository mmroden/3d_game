// ── Structural triple ──────────────────────────────────────────────────

/// Enforces that any structural piece set provides floor, wall, and ceiling.
pub trait StructuralTriple {
    fn floor(&self) -> &'static str;
    fn wall(&self) -> &'static str;
    fn ceiling(&self) -> &'static str;
}

/// A concrete floor + wall + ceiling mesh triple.
#[derive(Debug, Clone, Copy)]
pub struct Triple {
    pub floor: &'static str,
    pub wall: &'static str,
    pub ceiling: &'static str,
}

impl StructuralTriple for Triple {
    fn floor(&self) -> &'static str { self.floor }
    fn wall(&self) -> &'static str { self.wall }
    fn ceiling(&self) -> &'static str { self.ceiling }
}

// ── Layer set (straight + corner variants) ─────────────────────────────

/// A single mesh layer with straight, inner-corner, and outer-corner variants.
#[derive(Debug, Clone, Copy)]
pub struct LayerSet {
    pub straight: &'static str,
    pub corner_inner: &'static str,
    pub corner_outer: &'static str,
}

// ── Wall sets ───────────────────────────────────────────────────────────

/// A themed group of matching structural assets organized as triples,
/// plus ShortWall and Bottom layers for gap-free wall stacks.
#[derive(Debug, Clone, Copy)]
pub struct WallSet {
    pub id: &'static str,
    pub straight: Triple,
    pub corner_inner: Triple,
    pub corner_outer: Triple,
    /// Lower wall section (y ≈ 0–1m), fills the gap between Bottom and Wall.
    pub short_wall: LayerSet,
    /// Baseboard decorative trim (y ≈ 0–0.02m), at the base of every wall.
    pub bottom: LayerSet,
}

pub const WALL_SET_ASTRA: WallSet = WallSet {
    id: "astra",
    straight: Triple {
        floor: megakit_platform!("Platform_Simple.gltf"),
        wall: megakit_wall!("WallAstra_Straight.gltf"),
        ceiling: megakit_wall!("TopAstra_Straight.gltf"),
    },
    corner_inner: Triple {
        floor: megakit_platform!("Platform_Simple_Curve.gltf"),
        wall: megakit_wall!("WallAstra_Corner_Round_Inner.gltf"),
        ceiling: megakit_wall!("TopSimple_Corner_Round_Inner.gltf"),
    },
    corner_outer: Triple {
        floor: megakit_platform!("Platform_Simple_Curve.gltf"),
        wall: megakit_wall!("WallAstra_Corner_Round_Outer.gltf"),
        ceiling: megakit_wall!("TopAstra_Curve_Round_Outer.gltf"),
    },
    short_wall: LayerSet {
        straight: megakit_wall!("ShortWall_AccentStrip_Straight.gltf"),
        corner_inner: megakit_wall!("ShortWall_AccentStrip_Corner_Inner.gltf"),
        corner_outer: megakit_wall!("ShortWall_AccentStrip_Corner_Outer.gltf"),
    },
    bottom: LayerSet {
        straight: megakit_wall!("BottomAccent_Straight.gltf"),
        corner_inner: megakit_wall!("BottomAccent_Corner_Round_Inner.gltf"),
        corner_outer: megakit_wall!("BottomAccent_Corner_Round_Outer.gltf"),
    },
};

pub const WALL_SET_BAND: WallSet = WallSet {
    id: "band",
    straight: Triple {
        floor: megakit_platform!("Platform_Metal.gltf"),
        wall: megakit_wall!("WallBand_Straight.gltf"),
        ceiling: megakit_wall!("TopAstra_Straight.gltf"),
    },
    corner_inner: Triple {
        floor: megakit_platform!("Platform_Metal_Curve.gltf"),
        wall: megakit_wall!("WallBand_Corner_Round_Inner.gltf"),
        ceiling: megakit_wall!("TopSimple_Corner_Round_Inner.gltf"),
    },
    corner_outer: Triple {
        floor: megakit_platform!("Platform_Metal_Curve.gltf"),
        wall: megakit_wall!("WallBand_Corner_Round_Outer.gltf"),
        ceiling: megakit_wall!("TopAstra_Curve_Round_Outer.gltf"),
    },
    short_wall: LayerSet {
        straight: megakit_wall!("ShortWall_Band2_Straight.gltf"),
        corner_inner: megakit_wall!("ShortWall_Band2_Corner_Inner.gltf"),
        corner_outer: megakit_wall!("ShortWall_Band2_Corner_Outer.gltf"),
    },
    bottom: LayerSet {
        straight: megakit_wall!("BottomSimple_Straight.gltf"),
        corner_inner: megakit_wall!("BottomSimple_Corner_Round_Inner.gltf"),
        corner_outer: megakit_wall!("BottomSimple_Corner_Round_Outer.gltf"),
    },
};

pub const WALL_SET_PIPE: WallSet = WallSet {
    id: "pipe",
    straight: Triple {
        floor: megakit_platform!("Platform_DarkPlates.gltf"),
        wall: megakit_wall!("WallPipe_Straight.gltf"),
        ceiling: megakit_wall!("TopPlates_Straight.gltf"),
    },
    corner_inner: Triple {
        floor: megakit_platform!("Platform_DarkPlates_Curves.gltf"),
        wall: megakit_wall!("WallPipe_Corner_Round_Inner.gltf"),
        ceiling: megakit_wall!("TopPlates_Corner_Round_Inner.gltf"),
    },
    corner_outer: Triple {
        floor: megakit_platform!("Platform_DarkPlates_Curves.gltf"),
        wall: megakit_wall!("WallPipe_Corner_Round_Outer.gltf"),
        ceiling: megakit_wall!("TopPlates_Corner_Round_Outer.gltf"),
    },
    short_wall: LayerSet {
        straight: megakit_wall!("ShortWall_MetalPlates_Straight.gltf"),
        corner_inner: megakit_wall!("ShortWall_MetalPlates_Corner_Inner.gltf"),
        corner_outer: megakit_wall!("ShortWall_MetalPlates_Corner_Outer.gltf"),
    },
    bottom: LayerSet {
        straight: megakit_wall!("BottomMetal_Straight.gltf"),
        corner_inner: megakit_wall!("BottomMetal_Corner_Round_Inner.gltf"),
        corner_outer: megakit_wall!("BottomMetal_Corner_Round_Outer.gltf"),
    },
};

pub const WALL_SET_WIDEBAND: WallSet = WallSet {
    id: "wideband",
    straight: Triple {
        floor: megakit_platform!("Platform_CenterPlate.gltf"),
        wall: megakit_wall!("WallWideBand_Straight.gltf"),
        ceiling: megakit_wall!("TopSimple_Straight.gltf"),
    },
    corner_inner: Triple {
        floor: megakit_platform!("Platform_CenterPlate_Curve.gltf"),
        wall: megakit_wall!("WallWideBand_Corner_Round_Inner.gltf"),
        ceiling: megakit_wall!("TopSimple_Corner_Round_Inner.gltf"),
    },
    corner_outer: Triple {
        floor: megakit_platform!("Platform_CenterPlate_Curve.gltf"),
        wall: megakit_wall!("WallWideBand_Corner_Round_Outer.gltf"),
        ceiling: megakit_wall!("TopSimple_Corner_Round_Outer.gltf"),
    },
    short_wall: LayerSet {
        straight: megakit_wall!("ShortWall_WhitePlate2_Straight.gltf"),
        corner_inner: megakit_wall!("ShortWall_WhitePlate2_Corner_Inner.gltf"),
        corner_outer: megakit_wall!("ShortWall_WhitePlate2_Corner_Outer.gltf"),
    },
    bottom: LayerSet {
        straight: megakit_wall!("BottomSimple_Straight.gltf"),
        corner_inner: megakit_wall!("BottomSimple_Corner_Round_Inner.gltf"),
        corner_outer: megakit_wall!("BottomSimple_Corner_Round_Outer.gltf"),
    },
};

pub const WALL_SET_WINDOW: WallSet = WallSet {
    id: "window",
    straight: Triple {
        floor: megakit_platform!("Platform_Squares.gltf"),
        wall: megakit_wall!("WallWindow_Straight.gltf"),
        ceiling: megakit_wall!("TopWindow_Straight.gltf"),
    },
    corner_inner: Triple {
        floor: megakit_platform!("Platform_Squares_Curve.gltf"),
        wall: megakit_wall!("WallWindow_Corner_Round_Inner.gltf"),
        ceiling: megakit_wall!("TopWindow_Corner_Curve_Inner.gltf"),
    },
    corner_outer: Triple {
        floor: megakit_platform!("Platform_Squares_Curve.gltf"),
        wall: megakit_wall!("WallWindow_Corner_Round_Outer.gltf"),
        ceiling: megakit_wall!("TopSimple_Corner_Round_Outer.gltf"),
    },
    short_wall: LayerSet {
        straight: megakit_wall!("ShortWall_Simple1_Straight.gltf"),
        corner_inner: megakit_wall!("ShortWall_Simple1_Corner_Inner.gltf"),
        corner_outer: megakit_wall!("ShortWall_Simple1_Corner_Outer.gltf"),
    },
    bottom: LayerSet {
        straight: megakit_wall!("BottomSimple_Straight.gltf"),
        corner_inner: megakit_wall!("BottomSimple_Corner_Round_Inner.gltf"),
        corner_outer: megakit_wall!("BottomSimple_Corner_Round_Outer.gltf"),
    },
};

pub const WALL_SET_PADDED: WallSet = WallSet {
    id: "padded",
    straight: Triple {
        floor: megakit_platform!("Platform_Padded.gltf"),
        wall: megakit_wall!("WallPadded_Straight.gltf"),
        ceiling: megakit_wall!("TopPadded_Flat_Straight.gltf"),
    },
    corner_inner: Triple {
        floor: megakit_platform!("Platform_Padded.gltf"),
        wall: megakit_wall!("WallPadded_Curve_Round_Inner.gltf"),
        ceiling: megakit_wall!("TopPadded_Flat_Curve_Round_Inner.gltf"),
    },
    corner_outer: Triple {
        floor: megakit_platform!("Platform_Padded.gltf"),
        wall: megakit_wall!("WallPadded_Curve_Round_Outer.gltf"),
        ceiling: megakit_wall!("TopPadded_Flat_Curve_Round_Outer.gltf"),
    },
    short_wall: LayerSet {
        straight: megakit_wall!("ShortWall_DarkPlastic_Straight.gltf"),
        corner_inner: megakit_wall!("ShortWall_DarkPlastic_Corner_Inner.gltf"),
        corner_outer: megakit_wall!("ShortWall_DarkPlastic_Corner_Outer.gltf"),
    },
    bottom: LayerSet {
        straight: megakit_wall!("BottomMetal_Straight.gltf"),
        corner_inner: megakit_wall!("BottomMetal_Corner_Round_Inner.gltf"),
        corner_outer: megakit_wall!("BottomMetal_Corner_Round_Outer.gltf"),
    },
};

pub const ALL_WALL_SETS: &[WallSet] = &[
    WALL_SET_ASTRA,
    WALL_SET_BAND,
    WALL_SET_PIPE,
    WALL_SET_WIDEBAND,
    WALL_SET_WINDOW,
    WALL_SET_PADDED,
];

/// The door frame asset — structural, not themed.
pub const DOOR: &str = megakit_platform!("Door_Frame_Square.gltf");

// ── Panel sets (B11 cubic-cell panel worlds) ────────────────────────────

/// A panel world's face pool: ONE list serving every cell face — floor,
/// wall, ceiling are terrestrial words with no meaning here; a face is a
/// face and rotation is the only difference (the 6DOF principle). Panels
/// are flat plates spanning exactly one cell face, split from the source
/// kit by `scripts/split-panels.py`; the pitch derives from the pool's
/// measured extent (the probe, `make assets`) — never authored.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelSet {
    pub id: String,
    pub plates: Vec<PanelPlate>,
}

/// One wall plate: its scene and its censused thickness. The assembler
/// seats each plate's BACK on the cell-face plane (half a thickness
/// inward) — centered plates leave a void slit at every corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelPlate {
    pub scene: super::catalog::SceneId,
    pub thick: f32,
}


/// One baked library plate as pooled by ROLE (census v2): scene, censused
/// thickness, and face extents — walls normalized to [width, height]
/// (height == the story module), floors/ceilings larger-first.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RolePlate {
    pub scene: super::catalog::SceneId,
    pub thick: f32,
    pub face: [f32; 2],
}

/// A panel kit's role-typed pools, derived by the linker from the baked
/// variants — never authored. The constructor grabs floor pieces for -Y
/// faces, ceiling pieces for +Y, wall pieces for XZ faces (yaw at
/// placement), and layers decorations/add-ons colliderlessly. Replaces
/// the one-pool-rotated-six-ways [`PanelSet`], whose native-pose axiom
/// failed (2026-07-18); that type dies with the legacy assembler path.
#[derive(Debug, Clone, PartialEq)]
pub struct RolePools {
    pub id: String,
    pub floor: Vec<RolePlate>,
    pub ceiling: Vec<RolePlate>,
    pub wall: Vec<RolePlate>,
    pub decoration: Vec<RolePlate>,
    pub addon: Vec<RolePlate>,
}

// No authored panel list lives here (the old PANEL_SET_VOL01 hand-picked
// nine plates — seven of which the census later measured as see-through
// trusses). Wall pools DERIVE: the probe censuses each installed piece
// into kits.generated.toml, and the CATALOG's linker filters the census
// against the kit's authored `wall_coverage` policy into owned,
// SceneId-carrying pools.

impl Triple {
    /// Every scene this triple references (the catalog interns them all
    /// at load, so placements can carry ids).
    pub(super) fn scenes(&self) -> [&'static str; 3] {
        [self.floor, self.wall, self.ceiling]
    }
}

impl LayerSet {
    pub(super) fn scenes(&self) -> [&'static str; 3] {
        [self.straight, self.corner_inner, self.corner_outer]
    }
}

impl WallSet {
    /// Every scene in the set, for load-time interning.
    pub(super) fn scenes(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.straight
            .scenes()
            .into_iter()
            .chain(self.corner_inner.scenes())
            .chain(self.corner_outer.scenes())
            .chain(self.short_wall.scenes())
            .chain(self.bottom.scenes())
    }
}
