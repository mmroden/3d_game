// ── Light fixtures ──────────────────────────────────────────────────────

/// A light fixture mesh with its co-located light source parameters.
/// The `light_offset` is relative to the fixture mesh origin, keeping the
/// light source physically inside the fixture geometry.
#[derive(Debug, Clone, Copy)]
pub struct LightFixture {
    pub scene: &'static str,
    /// Offset from fixture mesh origin to the light emitter point.
    pub light_offset: [f32; 3],
    /// Approximate half-extents of the fixture mesh (for bounds checking).
    pub fixture_bounds: [f32; 3],
    pub range: f32,
    pub energy: f32,
}

pub const LIGHT_CEILING_WIDE: LightFixture = LightFixture {
    scene: megakit_prop!("Prop_Light_Wide.gltf"),
    light_offset: [0.0, -0.3, 0.0],
    fixture_bounds: [1.0, 0.4, 0.5],
    range: 14.0,
    energy: 4.5,
};

pub const LIGHT_CEILING_SMALL: LightFixture = LightFixture {
    scene: megakit_prop!("Prop_Light_Small.gltf"),
    light_offset: [0.0, -0.2, 0.0],
    fixture_bounds: [0.3, 0.3, 0.3],
    range: 12.0,
    energy: 3.75,
};

pub const LIGHT_CORNER: LightFixture = LightFixture {
    scene: megakit_prop!("Prop_Light_Corner.gltf"),
    light_offset: [0.0, -0.2, 0.0],
    fixture_bounds: [0.4, 0.3, 0.4],
    range: 10.0,
    energy: 3.0,
};

pub const LIGHT_FLOOR: LightFixture = LightFixture {
    scene: megakit_prop!("Prop_Light_Floor.gltf"),
    light_offset: [0.0, 1.0, 0.0],
    fixture_bounds: [0.3, 1.2, 0.3],
    range: 10.0,
    energy: 3.0,
};

pub const CEILING_LIGHTS: &[LightFixture] = &[LIGHT_CEILING_WIDE, LIGHT_CEILING_SMALL];
pub const ALL_LIGHTS: &[LightFixture] = &[LIGHT_CEILING_WIDE, LIGHT_CEILING_SMALL, LIGHT_CORNER, LIGHT_FLOOR];
