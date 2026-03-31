//! ROYGBIV laser level progression system.

/// Laser upgrade levels following ROYGBIV color progression.
/// Each level increases damage by 1 (Red=1 through Violet=7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LaserLevel {
    Red = 1,
    Orange = 2,
    Yellow = 3,
    Green = 4,
    Blue = 5,
    Indigo = 6,
    Violet = 7,
}

impl LaserLevel {
    pub fn damage(&self) -> f32 {
        *self as u32 as f32
    }

    /// RGBA color for beam rendering.
    pub fn color(&self) -> [f32; 4] {
        match self {
            Self::Red => [1.0, 0.2, 0.2, 1.0],
            Self::Orange => [1.0, 0.5, 0.0, 1.0],
            Self::Yellow => [1.0, 1.0, 0.2, 1.0],
            Self::Green => [0.2, 1.0, 0.2, 1.0],
            Self::Blue => [0.3, 0.3, 1.0, 1.0],
            Self::Indigo => [0.4, 0.0, 0.8, 1.0],
            Self::Violet => [0.8, 0.2, 1.0, 1.0],
        }
    }

    /// Cost to upgrade TO this level. None for Red (starting level).
    pub fn upgrade_cost(&self) -> Option<u32> {
        match self {
            Self::Red => None,
            Self::Orange => Some(10_000),
            Self::Yellow => Some(20_000),
            Self::Green => Some(30_000),
            Self::Blue => Some(50_000),
            Self::Indigo => Some(80_000),
            Self::Violet => Some(130_000),
        }
    }

    /// Next level, or None if already at Violet.
    pub fn next(&self) -> Option<LaserLevel> {
        match self {
            Self::Red => Some(Self::Orange),
            Self::Orange => Some(Self::Yellow),
            Self::Yellow => Some(Self::Green),
            Self::Green => Some(Self::Blue),
            Self::Blue => Some(Self::Indigo),
            Self::Indigo => Some(Self::Violet),
            Self::Violet => None,
        }
    }

    /// Death penalty: halve the level (round down), minimum Red.
    /// Level 4→2, Level 3→1, Level 7→3, Level 2→1, Level 1→1.
    pub fn downgrade(&self) -> LaserLevel {
        let current = *self as u32;
        let new_level = (current / 2).max(1);
        Self::from_level(new_level).unwrap_or(Self::Red)
    }

    /// Construct from numeric level (1-7).
    pub fn from_level(n: u32) -> Option<LaserLevel> {
        match n {
            1 => Some(Self::Red),
            2 => Some(Self::Orange),
            3 => Some(Self::Yellow),
            4 => Some(Self::Green),
            5 => Some(Self::Blue),
            6 => Some(Self::Indigo),
            7 => Some(Self::Violet),
            _ => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Indigo => "Indigo",
            Self::Violet => "Violet",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_matches_level_number() {
        assert_eq!(LaserLevel::Red.damage(), 1.0);
        assert_eq!(LaserLevel::Orange.damage(), 2.0);
        assert_eq!(LaserLevel::Yellow.damage(), 3.0);
        assert_eq!(LaserLevel::Green.damage(), 4.0);
        assert_eq!(LaserLevel::Blue.damage(), 5.0);
        assert_eq!(LaserLevel::Indigo.damage(), 6.0);
        assert_eq!(LaserLevel::Violet.damage(), 7.0);
    }

    #[test]
    fn colors_are_distinct() {
        let colors: Vec<[f32; 4]> = [
            LaserLevel::Red, LaserLevel::Orange, LaserLevel::Yellow,
            LaserLevel::Green, LaserLevel::Blue, LaserLevel::Indigo, LaserLevel::Violet,
        ].iter().map(|l| l.color()).collect();
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(colors[i], colors[j], "levels {} and {} have same color", i, j);
            }
        }
    }

    #[test]
    fn red_has_no_upgrade_cost() {
        assert_eq!(LaserLevel::Red.upgrade_cost(), None);
    }

    #[test]
    fn upgrade_costs_increase() {
        let costs: Vec<u32> = [
            LaserLevel::Orange, LaserLevel::Yellow, LaserLevel::Green,
            LaserLevel::Blue, LaserLevel::Indigo, LaserLevel::Violet,
        ].iter().filter_map(|l| l.upgrade_cost()).collect();
        assert_eq!(costs, vec![10_000, 20_000, 30_000, 50_000, 80_000, 130_000]);
        for w in costs.windows(2) {
            assert!(w[1] > w[0], "costs should increase: {} > {}", w[1], w[0]);
        }
    }

    #[test]
    fn next_level_progression() {
        assert_eq!(LaserLevel::Red.next(), Some(LaserLevel::Orange));
        assert_eq!(LaserLevel::Indigo.next(), Some(LaserLevel::Violet));
        assert_eq!(LaserLevel::Violet.next(), None);
    }

    #[test]
    fn downgrade_halves_level() {
        assert_eq!(LaserLevel::Red.downgrade(), LaserLevel::Red);       // 1/2=0 -> clamped to 1
        assert_eq!(LaserLevel::Orange.downgrade(), LaserLevel::Red);    // 2/2=1
        assert_eq!(LaserLevel::Yellow.downgrade(), LaserLevel::Red);    // 3/2=1
        assert_eq!(LaserLevel::Green.downgrade(), LaserLevel::Orange);  // 4/2=2
        assert_eq!(LaserLevel::Blue.downgrade(), LaserLevel::Orange);   // 5/2=2
        assert_eq!(LaserLevel::Indigo.downgrade(), LaserLevel::Yellow); // 6/2=3
        assert_eq!(LaserLevel::Violet.downgrade(), LaserLevel::Yellow); // 7/2=3
    }

    #[test]
    fn from_level_valid() {
        assert_eq!(LaserLevel::from_level(1), Some(LaserLevel::Red));
        assert_eq!(LaserLevel::from_level(7), Some(LaserLevel::Violet));
    }

    #[test]
    fn from_level_invalid() {
        assert_eq!(LaserLevel::from_level(0), None);
        assert_eq!(LaserLevel::from_level(8), None);
    }

    #[test]
    fn display_names() {
        assert_eq!(LaserLevel::Red.display_name(), "Red");
        assert_eq!(LaserLevel::Violet.display_name(), "Violet");
    }
}
