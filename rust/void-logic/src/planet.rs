//! Planet structure: the campaign is divided into planets of six levels
//! each. Bosses are scheduled deterministically within every planet — a
//! mid-planet boss and a planet-final boss (owner's call 2026-07-04,
//! superseding the earlier chance-based roll): predictable rhythm is the
//! world-building, and the reward split (consolation pile vs red hull
//! container) hangs off the same schedule.

/// 1-based planet index for a 1-based level. Planet lengths are DECLARED
/// per planet in rosters/planets/ (owner 2026-07-05); past the declared
/// table the number keeps counting while the newest planet's shape repeats.
pub fn planet_of(level: u32) -> u32 {
    crate::roster::roster()
        .planet_number_and_relative(level.max(1))
        .0
}

/// 1-based position of a level within its planet (1..=its declared length).
pub fn planet_relative(level: u32) -> u32 {
    crate::roster::roster()
        .planet_number_and_relative(level.max(1))
        .1
}

/// World-space quantization of a level: meters per grid tile and meters
/// per story. THE single pitch source (B11): every world-space conversion
/// takes a `Pitch` — no consumer holds its own 4.0/5.0 literal. Planet 1
/// is the megakit's terrestrial 4×5; planet 2+ flips to 3 m CUBES
/// (tile == story, no distinguished axis) when the panel assembler lands
/// (B11 step 3) — the plumbing is planet-aware now, the values flip then.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pitch {
    pub tile: f32,
    pub story: f32,
}

impl Pitch {
    /// The pitch DERIVES from the planet's declared kit (catalog/kits.toml,
    /// measured by the make-assets probe once it lands) — nobody authors a
    /// cell dimension anywhere else. Shipped-grammar door for
    /// [`Roster::pitch_for_level`].
    pub fn for_level(level: u32, run_seed: crate::seed::Seed) -> Self {
        crate::roster::roster().pitch_for_level(level, run_seed)
    }
}

/// Whether this level is built in the panel paradigm: the planet's declared
/// kit decides (cubic cells skinned from one panel pool vs layered megakit).
pub fn panel_world(level: u32) -> bool {
    crate::roster::roster().panel_world(level)
}

/// The interstitial banner for a level entry: `Some((title, flavor))` when
/// this level is the first step onto a NEW planet (planet 2+), `None` for
/// every ordinary sector entry. The flavor line is the narrative channel —
/// the story beats land here when the owner picks them (B10 note).
pub fn arrival_banner(level: u32) -> Option<(String, String)> {
    let planet = planet_of(level);
    if planet < 2 || planet_relative(level) != 1 {
        return None;
    }
    // The schedule (WHEN a banner shows) lives here; the words live in
    // the narrative module (lore owns every story string).
    Some((
        format!("PLANET {planet}"),
        crate::lore::arrival_flavor(planet).to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_levels_per_planet() {
        for level in 1..=6 {
            assert_eq!(planet_of(level), 1, "level {level} is planet 1");
        }
        for level in 7..=12 {
            assert_eq!(planet_of(level), 2, "level {level} is planet 2");
        }
        assert_eq!(planet_of(13), 3);
    }

    #[test]
    fn planet_relative_cycles_one_through_six() {
        assert_eq!(planet_relative(1), 1);
        assert_eq!(planet_relative(3), 3);
        assert_eq!(planet_relative(6), 6);
        assert_eq!(planet_relative(7), 1, "planet 2 restarts the cycle");
        assert_eq!(planet_relative(9), 3);
        assert_eq!(planet_relative(12), 6);
    }

    #[test]
    fn the_pitch_is_single_sourced_and_derives_from_the_kit_meshes() {
        // The pitch's ONE source is the probe-derived kit grid
        // (kits.generated.toml, measured from the recipe meshes) — there is
        // no second constant to drift against. These literals pin what the
        // megakit and panel meshes measure (tests may hold literals).
        for level in 1..=6 {
            let p = Pitch::for_level(level, crate::seed::Seed::new(1));
            assert_eq!((p.tile, p.story), (4.0, 5.0), "level {level}: megakit grid");
        }
        // Planet 2+: cubic panel cells — tile == story at the panel extent.
        let p2 = Pitch::for_level(7, crate::seed::Seed::new(1));
        assert_eq!((p2.tile, p2.story), (3.0, 3.0), "planet 2 is cubic at 3 m");
    }

    #[test]
    fn the_banner_greets_each_new_planet_and_only_then() {
        for level in 1..=6 {
            assert_eq!(arrival_banner(level), None,
                "planet 1 needs no introduction (level {level})");
        }
        let (title, flavor) = arrival_banner(7).expect("planet 2 announces itself");
        assert!(title.contains("PLANET 2"), "got {title:?}");
        assert!(!flavor.is_empty(), "the flavor line is the story channel");
        for level in 8..=12 {
            assert_eq!(arrival_banner(level), None,
                "mid-planet sectors are ordinary entries (level {level})");
        }
        let (title, _) = arrival_banner(13).expect("planet 3 announces itself");
        assert!(title.contains("PLANET 3"));
        assert!(arrival_banner(25).is_some(),
            "planets past the flavor table still get a banner");
    }

    #[test]
    fn the_algebra_reconstructs_the_level() {
        // planet_of and planet_relative are a proper quotient/remainder
        // pair over the DECLARED per-planet strides (planets need not share
        // a length — planet 3 is shorter than 1 and 2); past the declared
        // table the newest planet's stride repeats. Reconstruction walks
        // the same declarations, so retuning lengths can't break this.
        let declared: Vec<(u32, u32)> = crate::roster::roster()
            .planets
            .iter()
            .map(|p| (p.planet, p.levels))
            .collect();
        let level_base = |planet: u32| -> u32 {
            let mut base = 0;
            for (number, levels) in &declared {
                if *number < planet {
                    base += levels;
                }
            }
            let last = declared.last().expect("linker guarantees planets");
            // Virtual planets past the table repeat the newest stride.
            base + planet.saturating_sub(last.0 + 1) * last.1
        };
        for level in 1..=36 {
            let rebuilt = level_base(planet_of(level)) + planet_relative(level);
            assert_eq!(rebuilt, level, "level {level} reconstructs from its planet algebra");
        }
    }
}
