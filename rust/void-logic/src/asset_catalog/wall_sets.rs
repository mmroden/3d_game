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
    /// Width of one tile in meters, derived from mesh z-range.
    pub tile_width: f32,
    /// Height of one story in meters, derived from top-layer y-max.
    pub story_height: f32,
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
    tile_width: 4.0,
    story_height: 5.0,
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
    tile_width: 4.0,
    story_height: 5.0,
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
    tile_width: 4.0,
    story_height: 5.0,
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
    tile_width: 4.0,
    story_height: 5.0,
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
    tile_width: 4.0,
    story_height: 5.0,
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
    tile_width: 4.0,
    story_height: 5.0,
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
/// are flat plates authored `pitch × pitch` in XZ, thin in Y, split from
/// the source kit by `scripts/split-panels.py`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelSet {
    pub id: &'static str,
    /// Every panel spans exactly one cell face at this pitch (meters).
    pub pitch: f32,
    pub panels: &'static [&'static str],
}

macro_rules! wall_panel {
    ($name:expr) => {
        concat!("res://addons/walls/", $name)
    };
}

/// Planet 2's kit: the 3 m plates of cgtrader Sci-Fi Parts Kit Vol 01
/// (wider plates in the kit are reserved for props / multi-cell faces).
pub const PANEL_SET_VOL01: PanelSet = PanelSet {
    id: "vol01",
    pitch: 3.0,
    panels: &[
        wall_panel!("sf_pp01_a_001.glb"),
        wall_panel!("sf_pp01_a_006.glb"),
        wall_panel!("sf_pp01_b_001.glb"),
        wall_panel!("sf_pp01_b_002.glb"),
        wall_panel!("sf_pp01_c_001.glb"),
        wall_panel!("sf_pp01_c_002.glb"),
        wall_panel!("sf_pp01_c_003.glb"),
        wall_panel!("sf_pp01_d_001.glb"),
        wall_panel!("sf_pp01_f_001.glb"),
    ],
};
