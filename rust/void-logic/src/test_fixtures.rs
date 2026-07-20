//! Test-owned fixture grammars. `rosters/` is the OWNER'S tuning data —
//! tests that need guaranteed content load the fixture through the same
//! [`load_from`](crate::roster::load_from) path instead, so retuning the
//! shipped TOML can never break them (owner 2026-07-09). ONE canonical
//! fixture set, shared with the GUT suite (which installs it through the
//! shell's grammar-override test door): `godot/tests/fixtures/grammar/`.
//! It links against the REAL model catalog so the shell can build it.

use crate::asset_catalog::{ENVIRONMENTS_GENERATED_TOML, MODELS_TOML};
use crate::roster::{load_from, EnvSources, Roster};

/// The SHIPPED catalog — the ONE test-support home for it (helper
/// duplication across test modules is plaque; see design-review §10).
/// Tests that assemble against real assets resolve their placements'
/// ids here; fixture-grammar tests resolve through their own
/// `roster.catalog` instead.
pub fn cat() -> &'static crate::asset_catalog::AssetCatalog {
    crate::asset_catalog::catalog()
}

/// A placement's scene as its res:// path — for FAMILY assertions
/// (`spath(id).contains("Platform")`). Identity assertions compare ids:
/// `p.scene == cat().const_scene(WS.straight.wall)`.
pub fn spath(id: crate::asset_catalog::SceneId) -> &'static str {
    crate::asset_catalog::catalog().path(id)
}

pub const FX_ENEMIES: &str = include_str!("../../../godot/tests/fixtures/grammar/enemies.toml");
pub const FX_KITS: &str = include_str!("../../../godot/tests/fixtures/grammar/kits.toml");
pub const FX_KIT_GRIDS: &str =
    include_str!("../../../godot/tests/fixtures/grammar/kits.generated.toml");
pub const FX_PLANET: &str = include_str!("../../../godot/tests/fixtures/grammar/planet_1.toml");
pub const FX_KITS_FIXED: &str =
    include_str!("../../../godot/tests/fixtures/grammar/kits_fixed.toml");
pub const FX_PLANET_FIXED: &str =
    include_str!("../../../godot/tests/fixtures/grammar/planet_fixed.toml");
pub const FX_ENV_FIXED: &str =
    include_str!("../../../godot/tests/fixtures/grammar/env_fixed.toml");

/// The fixture grammar: LEVEL 1 fields a plain regular (`fx_grunt`), an
/// on-death-brood parent (`fx_brood_parent`), and a miniboss
/// (`fx_miniboss`) — every mechanism subject, guaranteed at any seed.
pub fn fixture_grammar() -> Roster {
    let catalog = crate::asset_catalog::AssetCatalog::load(
        FX_KITS,
        FX_KIT_GRIDS,
        MODELS_TOML,
        ENVIRONMENTS_GENERATED_TOML,
    )
    .unwrap_or_else(|e| panic!("the fixture catalog links: {}", e.join("\n")));
    load_from(
        FX_ENEMIES,
        &[FX_PLANET],
        EnvSources::generated_only(),
        std::sync::Arc::new(catalog),
    )
    .expect("the fixture grammar links")
}

/// The FIXED-paradigm fixture grammar: a two-level fixed planet over the
/// `fx_house` environment (porch start, den arena), staging `fx_boss` at
/// every level. The zone map rides the real installed apartment scene.
pub fn fixed_fixture_grammar() -> Roster {
    let catalog = crate::asset_catalog::AssetCatalog::load(
        FX_KITS_FIXED,
        "[kits]\n",
        MODELS_TOML,
        ENVIRONMENTS_GENERATED_TOML,
    )
    .unwrap_or_else(|e| panic!("the fixed fixture catalog links: {}", e.join("\n")));
    load_from(
        FX_ENEMIES,
        &[FX_PLANET_FIXED],
        EnvSources { authored: &[FX_ENV_FIXED], windows: &[] },
        std::sync::Arc::new(catalog),
    )
    .expect("the fixed fixture grammar links")
}

#[cfg(test)]
mod tests {
    /// ANCHOR for godot/tests/test_fixed_level.gd (the house convention:
    /// GUT never re-derives grammar; its constants trace here). The GUT
    /// file pins the fixture's shape — zone count, scale, the porch start
    /// box, the den arena center — so editing the fixture files means
    /// updating BOTH this anchor and the GUT constants together.
    #[test]
    fn gut_fixed_level_anchors() {
        let grammar = super::fixed_fixture_grammar();
        let env = grammar.environment_for_level(1).expect("level 1 is fixed");
        assert_eq!(env.zones.len(), 4, "GUT ZONES");
        let pitch = grammar.pitch_for_level(1);
        assert_eq!((pitch.tile, pitch.story), (5.0, 5.0), "GUT SCALE");
        let start = &env.zones[env.start_zone];
        assert_eq!((start.key.as_str(), start.min, start.extents),
            ("porch", [0, 0, 0], [2, 2, 2]),
            "GUT start box: world [0,10]^3 at scale 5");
        let boss = &env.zones[env.boss_zone];
        assert_eq!((boss.key.as_str(), boss.min, boss.extents),
            ("den", [4, 0, 0], [2, 2, 2]),
            "GUT arena: world center (25, 5, 5) at scale 5");
        // Containment: one slab per unshared zone-cell face. 28 cells,
        // 52 shared pairs -> 168 - 104 = 64 boundary faces.
        let mut cells = std::collections::HashSet::new();
        for z in &env.zones {
            for x in 0..z.extents[0] as i32 {
                for y in 0..z.extents[1] as i32 {
                    for c in 0..z.extents[2] as i32 {
                        cells.insert([z.min[0] + x, z.min[1] + y, z.min[2] + c]);
                    }
                }
            }
        }
        let boundary: usize = cells
            .iter()
            .map(|c| {
                [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]
                    .iter()
                    .filter(|d| !cells.contains(&[c[0] + d[0], c[1] + d[1], c[2] + d[2]]))
                    .count()
            })
            .sum();
        assert_eq!(boundary, 64, "GUT containment slab count");
    }
}
