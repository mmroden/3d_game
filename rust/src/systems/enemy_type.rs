//! Enemy type taxonomy with stats, display names, and scene paths.

/// All enemy types in the game, ordered by difficulty tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnemyType {
    Slime,
    GunDrone,
    Bat,
    EyeDrone,
    QuadOrb,
    Shark,
    QuadShell,
    Raptor,
    Skeleton,
    Trilobite,
    Dragon,
}

impl EnemyType {
    pub const ALL: &[EnemyType] = &[
        EnemyType::Slime,
        EnemyType::GunDrone,
        EnemyType::Bat,
        EnemyType::EyeDrone,
        EnemyType::QuadOrb,
        EnemyType::Shark,
        EnemyType::QuadShell,
        EnemyType::Raptor,
        EnemyType::Skeleton,
        EnemyType::Trilobite,
        EnemyType::Dragon,
    ];

    pub fn stats(&self) -> EnemyStats {
        match self {
            Self::Slime =>     EnemyStats { hp: 2.0,  speed: 4.0,  damage: 2.0,  detection_range: 20.0, attack_range: 4.0, attack_cooldown: 1.5, credits: 1_000 },
            Self::GunDrone =>  EnemyStats { hp: 3.0,  speed: 8.0,  damage: 3.0,  detection_range: 25.0, attack_range: 5.0, attack_cooldown: 1.0, credits: 1_000 },
            Self::Bat =>       EnemyStats { hp: 3.0,  speed: 10.0, damage: 2.0,  detection_range: 22.0, attack_range: 3.0, attack_cooldown: 0.8, credits: 1_000 },
            Self::EyeDrone =>  EnemyStats { hp: 5.0,  speed: 7.0,  damage: 4.0,  detection_range: 30.0, attack_range: 8.0, attack_cooldown: 1.2, credits: 1_000 },
            Self::QuadOrb =>   EnemyStats { hp: 8.0,  speed: 6.0,  damage: 5.0,  detection_range: 25.0, attack_range: 6.0, attack_cooldown: 1.0, credits: 1_000 },
            Self::Shark =>     EnemyStats { hp: 10.0, speed: 9.0,  damage: 6.0,  detection_range: 28.0, attack_range: 4.0, attack_cooldown: 0.7, credits: 1_000 },
            Self::QuadShell => EnemyStats { hp: 12.0, speed: 6.0,  damage: 5.0,  detection_range: 25.0, attack_range: 6.0, attack_cooldown: 1.0, credits: 1_000 },
            Self::Raptor =>    EnemyStats { hp: 18.0, speed: 11.0, damage: 7.0,  detection_range: 30.0, attack_range: 5.0, attack_cooldown: 0.6, credits: 1_000 },
            Self::Skeleton =>  EnemyStats { hp: 22.0, speed: 7.0,  damage: 8.0,  detection_range: 28.0, attack_range: 7.0, attack_cooldown: 0.9, credits: 1_000 },
            Self::Trilobite => EnemyStats { hp: 30.0, speed: 5.0,  damage: 10.0, detection_range: 20.0, attack_range: 5.0, attack_cooldown: 1.2, credits: 1_000 },
            Self::Dragon =>    EnemyStats { hp: 50.0, speed: 8.0,  damage: 15.0, detection_range: 35.0, attack_range: 10.0, attack_cooldown: 0.5, credits: 1_000 },
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Slime => "Slime",
            Self::GunDrone => "Gun Drone",
            Self::Bat => "Bat",
            Self::EyeDrone => "Eye Drone",
            Self::QuadOrb => "Quad Orb",
            Self::Shark => "Shark",
            Self::QuadShell => "Quad Shell",
            Self::Raptor => "Raptor",
            Self::Skeleton => "Skeleton",
            Self::Trilobite => "Trilobite",
            Self::Dragon => "Dragon",
        }
    }

    pub fn scene_path(&self) -> &'static str {
        match self {
            Self::Slime =>     "res://scenes/enemies/enemy_slime.tscn",
            Self::GunDrone =>  "res://scenes/enemies/enemy_drone.tscn",
            Self::Bat =>       "res://scenes/enemies/enemy_bat.tscn",
            Self::EyeDrone =>  "res://scenes/enemies/enemy_eye_drone.tscn",
            Self::QuadOrb =>   "res://scenes/enemies/enemy_quad_orb.tscn",
            Self::Shark =>     "res://scenes/enemies/enemy_shark.tscn",
            Self::QuadShell => "res://scenes/enemies/enemy_quad_shell.tscn",
            Self::Raptor =>    "res://scenes/enemies/enemy_raptor.tscn",
            Self::Skeleton =>  "res://scenes/enemies/enemy_skeleton.tscn",
            Self::Trilobite => "res://scenes/enemies/enemy_trilobite.tscn",
            Self::Dragon =>    "res://scenes/enemies/enemy_dragon.tscn",
        }
    }

    pub fn from_id(id: i32) -> Option<EnemyType> {
        Self::ALL.get(id as usize).copied()
    }

    pub fn id(&self) -> i32 {
        Self::ALL.iter().position(|e| e == self).unwrap() as i32
    }

    /// Minimum level at which this enemy type first appears.
    pub fn min_level(&self) -> u32 {
        match self {
            Self::Slime | Self::GunDrone => 1,
            Self::Bat | Self::EyeDrone => 2,
            Self::QuadOrb | Self::Shark => 3,
            Self::QuadShell => 4,
            Self::Raptor => 5,
            Self::Skeleton => 6,
            Self::Trilobite => 7,
            Self::Dragon => 8,
        }
    }
}

/// Returns which enemy types can appear at a given level.
pub fn enemies_for_level(level: u32) -> Vec<EnemyType> {
    EnemyType::ALL
        .iter()
        .filter(|e| e.min_level() <= level)
        .copied()
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnemyStats {
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub detection_range: f32,
    pub attack_range: f32,
    pub attack_cooldown: f32,
    pub credits: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_enemies_have_positive_hp() {
        for enemy in EnemyType::ALL {
            assert!(enemy.stats().hp > 0.0, "{:?} has non-positive hp", enemy);
        }
    }

    #[test]
    fn all_enemies_award_1000_credits() {
        for enemy in EnemyType::ALL {
            assert_eq!(enemy.stats().credits, 1_000, "{:?} doesn't award 1000 credits", enemy);
        }
    }

    #[test]
    fn gun_drone_has_3_hp() {
        assert_eq!(EnemyType::GunDrone.stats().hp, 3.0);
    }

    #[test]
    fn slime_is_weakest() {
        assert_eq!(EnemyType::Slime.stats().hp, 2.0);
    }

    #[test]
    fn dragon_is_strongest() {
        assert_eq!(EnemyType::Dragon.stats().hp, 50.0);
    }

    #[test]
    fn hp_scales_with_tier() {
        let hps: Vec<f32> = EnemyType::ALL.iter().map(|e| e.stats().hp).collect();
        // Each enemy should have HP >= the previous one (monotonically non-decreasing)
        for w in hps.windows(2) {
            assert!(w[1] >= w[0], "hp should scale: {} >= {}", w[1], w[0]);
        }
    }

    #[test]
    fn display_names_non_empty() {
        for enemy in EnemyType::ALL {
            assert!(!enemy.display_name().is_empty(), "{:?} has empty name", enemy);
        }
    }

    #[test]
    fn scene_paths_non_empty() {
        for enemy in EnemyType::ALL {
            assert!(enemy.scene_path().starts_with("res://"), "{:?} bad scene path", enemy);
        }
    }

    #[test]
    fn from_id_roundtrip() {
        for enemy in EnemyType::ALL {
            let id = enemy.id();
            assert_eq!(EnemyType::from_id(id), Some(*enemy));
        }
    }

    #[test]
    fn from_id_invalid() {
        assert_eq!(EnemyType::from_id(-1), None);
        assert_eq!(EnemyType::from_id(99), None);
    }

    #[test]
    fn enemies_for_level_1() {
        let enemies = enemies_for_level(1);
        assert!(enemies.contains(&EnemyType::Slime));
        assert!(enemies.contains(&EnemyType::GunDrone));
        assert!(!enemies.contains(&EnemyType::Dragon));
        assert_eq!(enemies.len(), 2);
    }

    #[test]
    fn enemies_for_level_8_includes_all() {
        let enemies = enemies_for_level(8);
        assert_eq!(enemies.len(), EnemyType::ALL.len());
    }

    #[test]
    fn enemies_for_level_scales() {
        let l1 = enemies_for_level(1).len();
        let l3 = enemies_for_level(3).len();
        let l8 = enemies_for_level(8).len();
        assert!(l3 > l1);
        assert!(l8 > l3);
    }

    #[test]
    fn min_level_ordering_matches_all_ordering() {
        let levels: Vec<u32> = EnemyType::ALL.iter().map(|e| e.min_level()).collect();
        for w in levels.windows(2) {
            assert!(w[1] >= w[0], "min_level should be non-decreasing: {} >= {}", w[1], w[0]);
        }
    }
}
