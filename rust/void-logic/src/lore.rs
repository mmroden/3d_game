//! Discovery log: the slowly-unfolding story of excavating the ruins,
//! shown on the level-load screen while shaders warm up.
//!
//! The copy here is PLACEHOLDER — the narrative is written later. What's
//! stable is the contract: every level maps to a blurb, themed by the
//! chapter it falls in (4 chapters of 10 levels — asteroids, Mars,
//! Earth, Venus; see the game-design notes).

/// The chapter (0..=3) a level belongs to. Levels run 1..=40.
pub fn chapter_of(level: u32) -> u32 {
    (level.saturating_sub(1) / 10).min(3)
}

/// A discovery-log blurb for `level`, shown on the load screen.
/// Placeholder copy; the level → blurb mapping is the stable contract.
pub fn level_blurb(level: u32) -> String {
    let (place, note) = match chapter_of(level) {
        0 => (
            "the asteroid aeries",
            "The builders flew. These chambers have no proper floors — \
             only perches, and the long fall between them.",
        ),
        1 => (
            "the Martian bunkers",
            "Buried military tech, hastily sealed. They were preparing \
             for something they did not expect to survive.",
        ),
        2 => (
            "the Yucatan strata",
            "The impact layer. Whatever they feared came here first, and \
             the bunkers below it were already occupied.",
        ),
        _ => (
            "the Venusian back-channels",
            "A peace faction's hidden portals, held open across 65 \
             million years. Someone meant for these to be found.",
        ),
    };
    format!("DISCOVERY LOG — Site {level}, {place}.\n\n[placeholder] {note}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapters_partition_the_forty_levels() {
        assert_eq!(chapter_of(1), 0);
        assert_eq!(chapter_of(10), 0);
        assert_eq!(chapter_of(11), 1);
        assert_eq!(chapter_of(20), 1);
        assert_eq!(chapter_of(21), 2);
        assert_eq!(chapter_of(30), 2);
        assert_eq!(chapter_of(31), 3);
        assert_eq!(chapter_of(40), 3);
        // Out-of-range levels clamp to the last chapter, never panic.
        assert_eq!(chapter_of(99), 3);
        assert_eq!(chapter_of(0), 0);
    }

    #[test]
    fn every_level_has_a_blurb_naming_its_site() {
        for level in 1..=40u32 {
            let blurb = level_blurb(level);
            assert!(!blurb.is_empty());
            assert!(
                blurb.contains(&format!("Site {level}")),
                "blurb for level {level} should name the site"
            );
        }
    }

    #[test]
    fn chapters_have_distinct_settings() {
        // The four chapters read differently, so the load screen isn't
        // the same text for 40 levels.
        let a = level_blurb(1);
        let b = level_blurb(11);
        let c = level_blurb(21);
        let d = level_blurb(31);
        assert!(a.contains("aeries"));
        assert!(b.contains("Martian"));
        assert!(c.contains("Yucatan"));
        assert!(d.contains("Venusian"));
    }
}
