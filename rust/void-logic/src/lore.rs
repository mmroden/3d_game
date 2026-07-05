//! The narrative module: every player-facing story word lives here, keyed
//! by PLANET — the campaign's one world-partition (planets of six levels;
//! the old 4-chapters-of-10 partition died with the planet redesign,
//! audit 2026-07-05). `planet::arrival_banner` asks this module for its
//! flavor line; future beats (retrieval story, stay-or-go epilogue) land
//! here too. All copy is PLACEHOLDER until the owner writes the story.

/// The interstitial flavor line for arriving at `planet` (2+). One line,
/// Hades-style sparse — the banner's title carries the where, this
/// carries the why-it-feels-different.
pub fn arrival_flavor(planet: u32) -> &'static str {
    match planet {
        2 => "The wreckage changes here. Something else built this.",
        3 => "Deeper. Older. The panels do not remember floors.",
        4 => "No signal reaches this far. Keep what you can carry.",
        _ => "Further than anyone has salvaged.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_planet_has_a_distinct_arrival_line() {
        // Consumed by planet::arrival_banner — the production caller that
        // makes this a contract rather than a self-referential pin.
        let lines: Vec<&str> = (2..=5).map(arrival_flavor).collect();
        for line in &lines {
            assert!(!line.is_empty());
        }
        assert_ne!(lines[0], lines[1], "planets read differently");
        assert_ne!(lines[1], lines[2]);
    }

    #[test]
    fn planets_beyond_the_written_table_still_get_a_line() {
        assert!(!arrival_flavor(99).is_empty(), "no planet arrives silent");
    }
}
