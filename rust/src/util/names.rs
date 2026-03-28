use rand::prelude::IndexedRandom;

const ADJECTIVES: &[&str] = &[
    "Wobbling", "Cursed", "Magnificent", "Dubious", "Throbbing",
    "Reluctant", "Forbidden", "Moist", "Aggressive", "Whispering",
    "Haunted", "Caffeinated", "Sentient", "Belligerent", "Ominous",
    "Sparkling", "Deflated", "Suspicious", "Righteous", "Quantum",
];

const NOUNS: &[&str] = &[
    "Catheter", "Tribunal", "Waffle", "Prophecy", "Accordion",
    "Sausage", "Abyss", "Biscuit", "Carbuncle", "Dumpster",
    "Gazebo", "Hamster", "Invoice", "Kebab", "Mortgage",
    "Pancake", "Spatula", "Teapot", "Vortex", "Zamboni",
];

const SUFFIXES: &[&str] = &[
    "of Regret", "of Doom", "of Mild Inconvenience", "of Chaos",
    "of Questionable Origin", "of Tuesday", "of the Ancients",
    "of Unreasonable Size", "MK II", "Deluxe", "Supreme",
    "of Infinite Sadness", "the Unforgivable", "of Science",
    "of Beef", "Prime", "of Moderate Peril", "Classic",
];

const STATION_PREFIXES: &[&str] = &[
    "Station", "Outpost", "Platform", "Sector", "Facility",
    "Complex", "Installation", "Hub", "Depot", "Nexus",
];

/// Generate a silly name for a weapon or item.
pub fn item_name(rng: &mut impl rand::Rng) -> String {
    let adj = ADJECTIVES.choose(rng).unwrap_or(&"Broken");
    let noun = NOUNS.choose(rng).unwrap_or(&"Thing");
    let suffix = SUFFIXES.choose(rng).unwrap_or(&"");
    format!("The {adj} {noun} {suffix}")
}

/// Generate a silly name for a station or level.
pub fn station_name(rng: &mut impl rand::Rng) -> String {
    let prefix = STATION_PREFIXES.choose(rng).unwrap_or(&"Station");
    let adj = ADJECTIVES.choose(rng).unwrap_or(&"Unknown");
    let noun = NOUNS.choose(rng).unwrap_or(&"Place");
    format!("{prefix} {adj} {noun}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn item_name_is_nonempty() {
        let mut rng = SmallRng::seed_from_u64(42);
        let name = item_name(&mut rng);
        assert!(!name.is_empty());
        assert!(name.starts_with("The "));
    }

    #[test]
    fn station_name_is_nonempty() {
        let mut rng = SmallRng::seed_from_u64(42);
        let name = station_name(&mut rng);
        assert!(!name.is_empty());
    }

    #[test]
    fn names_vary_with_seed() {
        let mut rng1 = SmallRng::seed_from_u64(1);
        let mut rng2 = SmallRng::seed_from_u64(2);
        let name1 = item_name(&mut rng1);
        let name2 = item_name(&mut rng2);
        // Not guaranteed but extremely likely with different seeds
        assert_ne!(name1, name2);
    }
}
