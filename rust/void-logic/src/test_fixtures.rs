//! Test-owned fixture grammars. `rosters/` is the OWNER'S tuning data —
//! tests that need guaranteed content load the fixture through the same
//! [`load_from`](crate::roster::load_from) path instead, so retuning the
//! shipped TOML can never break them (owner 2026-07-09). ONE canonical
//! fixture set, shared with the GUT suite (which installs it through the
//! shell's grammar-override test door): `godot/tests/fixtures/grammar/`.
//! It links against the REAL model catalog so the shell can build it.

use crate::roster::{load_from, Roster, MODELS_TOML};

pub const FX_ENEMIES: &str = include_str!("../../../godot/tests/fixtures/grammar/enemies.toml");
pub const FX_KITS: &str = include_str!("../../../godot/tests/fixtures/grammar/kits.toml");
pub const FX_KIT_GRIDS: &str =
    include_str!("../../../godot/tests/fixtures/grammar/kits.generated.toml");
pub const FX_PLANET: &str = include_str!("../../../godot/tests/fixtures/grammar/planet_1.toml");

/// The fixture grammar: LEVEL 1 fields a plain regular (`fx_grunt`), an
/// on-death-brood parent (`fx_brood_parent`), and a miniboss
/// (`fx_miniboss`) — every mechanism subject, guaranteed at any seed.
pub fn fixture_grammar() -> Roster {
    load_from(FX_ENEMIES, FX_KITS, FX_KIT_GRIDS, MODELS_TOML, &[FX_PLANET])
        .expect("the fixture grammar links")
}
