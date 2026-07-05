//! Planet structure: the campaign is divided into planets of six levels
//! each. Bosses are scheduled deterministically within every planet — a
//! mid-planet boss and a planet-final boss (owner's call 2026-07-04,
//! superseding the earlier chance-based roll): predictable rhythm is the
//! world-building, and the reward split (consolation pile vs red hull
//! container) hangs off the same schedule.

/// Levels per planet. The portal after each planet's final boss carries the
/// player to the next planet (new wall palette, new interstitial — B10).
pub const PLANET_LENGTH: u32 = 6;

/// 1-based planet index for a 1-based level: levels 1-6 → planet 1,
/// 7-12 → planet 2, and so on.
pub fn planet_of(level: u32) -> u32 {
    (level.saturating_sub(1)) / PLANET_LENGTH + 1
}

/// 1-based position of a level within its planet (1..=PLANET_LENGTH).
pub fn planet_relative(level: u32) -> u32 {
    (level.saturating_sub(1)) % PLANET_LENGTH + 1
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
    pub fn for_level(level: u32) -> Self {
        if panel_world(level) {
            // Cubic cells: tile == story == the panel pitch — no
            // distinguished axis (the 6DOF principle made physical).
            let p = crate::asset_catalog::PANEL_SET_VOL01.pitch;
            Self { tile: p, story: p }
        } else {
            Self {
                tile: crate::asset_catalog::WALL_SET_ASTRA.tile_width,
                story: crate::asset_catalog::WALL_SET_ASTRA.story_height,
            }
        }
    }
}

/// Whether this level is built in the panel paradigm (planet 2+): cubic
/// cells skinned from one panel pool, megakit left behind on planet 1.
pub fn panel_world(level: u32) -> bool {
    planet_of(level) >= 2
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
    fn the_pitch_is_single_sourced_and_matches_the_megakit_on_planet_one() {
        // Planet 1 rooms are ASSEMBLED from megakit pieces authored at
        // 4 m × 5 m — the pitch and the wall set must agree or every wall
        // gaps. Cross-pinned here so neither can drift alone.
        let astra = crate::asset_catalog::WALL_SET_ASTRA;
        for level in 1..=6 {
            let p = Pitch::for_level(level);
            assert_eq!(p.tile, astra.tile_width, "level {level} tile");
            assert_eq!(p.story, astra.story_height, "level {level} story");
        }
        // Planet 2+: cubic panel cells — tile == story, pinned against the
        // panel set so the plates and the cells can never drift apart.
        let panel = crate::asset_catalog::PANEL_SET_VOL01;
        let p2 = Pitch::for_level(7);
        assert_eq!((p2.tile, p2.story), (panel.pitch, panel.pitch),
            "planet 2 is cubic at the panel pitch");
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
        // planet_of and planet_relative are a proper quotient/remainder pair.
        for level in 1..=36 {
            let rebuilt = (planet_of(level) - 1) * PLANET_LENGTH + planet_relative(level);
            assert_eq!(rebuilt, level);
        }
    }
}
