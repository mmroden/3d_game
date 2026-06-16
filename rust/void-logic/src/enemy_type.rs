//! Enemy type taxonomy with stats, behavioural archetype, display names, and scene paths.

use crate::enemy_ai::{Archetype, DroneConfig};
use crate::newtypes::{Damage, Health, Shield};

/// All enemy types in the game, ordered by difficulty tier. Every enemy is a
/// mechanical defense system; their behaviour is set by [`Archetype`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EnemyType {
    GunDrone,
    QuadOrb,
    Bomber,
    EyeDrone,
    QuadShell,
}

impl EnemyType {
    pub const ALL: &[EnemyType] = &[
        EnemyType::GunDrone,
        EnemyType::QuadOrb,
        EnemyType::Bomber,
        EnemyType::EyeDrone,
        EnemyType::QuadShell,
    ];

    /// Compile-time stat table indexed by variant order in `ALL`.
    const STATS: &[EnemyStats] = &[
        // GunDrone — ranged kiter: holds distance and fires.
        EnemyStats { hp: Health::new(3.0),  speed: 8.0,  damage: Damage::new(5.0),  detection_range: 25.0, attack_range: 10.0, attack_cooldown: 1.0, archetype: Archetype::Kiter,   reward: 1_000 },
        // QuadOrb — swarmer: fast, fragile, four-legged; slows the player on contact.
        EnemyStats { hp: Health::new(3.0),  speed: 12.0, damage: Damage::new(4.0),  detection_range: 25.0, attack_range: 3.0,  attack_cooldown: 1.0, archetype: Archetype::Swarmer, reward: 1_000 },
        // Bomber — suicide: charges, fuses, then detonates for area damage.
        EnemyStats { hp: Health::new(4.0),  speed: 9.0,  damage: Damage::new(16.0), detection_range: 25.0, attack_range: 5.0,  attack_cooldown: 1.0, archetype: Archetype::Bomber,  reward: 1_000 },
        // EyeDrone — ranged kiter that spawns a GunDrone on death.
        EnemyStats { hp: Health::new(5.0),  speed: 7.0,  damage: Damage::new(6.0),  detection_range: 30.0, attack_range: 10.0, attack_cooldown: 1.2, archetype: Archetype::Kiter,   reward: 1_000 },
        // QuadShell — shielded tank: slow, durable, fires.
        EnemyStats { hp: Health::new(12.0), speed: 6.0,  damage: Damage::new(7.0),  detection_range: 25.0, attack_range: 6.0,  attack_cooldown: 1.0, archetype: Archetype::Tank,    reward: 1_000 },
    ];

    pub fn stats(&self) -> EnemyStats {
        Self::STATS[Self::ALL.iter().position(|e| e == self)
            .expect("EnemyType::ALL must contain every variant")]
    }

    /// Build the AI configuration for this enemy: shared ranges from `stats()`
    /// plus archetype-specific parameters (kiter stand-off, bomber fuse/blast,
    /// tank shield). Single source of truth for enemy behaviour tuning.
    pub fn ai_config(&self) -> DroneConfig {
        let s = self.stats();
        let mut config = DroneConfig {
            archetype: s.archetype,
            detection_range: s.detection_range,
            attack_range: s.attack_range,
            disengage_range: s.detection_range * 1.2,
            health: s.hp,
            attack_cooldown: s.attack_cooldown,
            ..DroneConfig::default()
        };
        match s.archetype {
            Archetype::Kiter => config.standoff_range = s.attack_range * 0.6,
            Archetype::Bomber => {
                config.fuse_seconds = 1.0;
                config.blast_radius = s.attack_range * 1.5;
            }
            Archetype::Tank => config.shield = Some(Shield::new(s.hp.as_f32() * 0.5)),
            Archetype::Shooter | Archetype::Swarmer => {}
        }
        config
    }

    /// Enemies this type spawns when it dies (the "subsidiary drone" mechanic).
    pub fn death_spawn(&self) -> Option<(EnemyType, u8)> {
        match self {
            Self::EyeDrone => Some((Self::GunDrone, 1)),
            Self::GunDrone | Self::QuadOrb | Self::Bomber | Self::QuadShell => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::GunDrone => "Gun Drone",
            Self::QuadOrb => "Quad Orb",
            Self::Bomber => "Bomber",
            Self::EyeDrone => "Eye Drone",
            Self::QuadShell => "Quad Shell",
        }
    }

    pub fn scene_path(&self) -> &'static str {
        match self {
            Self::GunDrone =>  "res://scenes/enemies/enemy_drone.tscn",
            Self::QuadOrb =>   "res://scenes/enemies/enemy_quad_orb.tscn",
            Self::Bomber =>    "res://scenes/enemies/enemy_bomber.tscn",
            Self::EyeDrone =>  "res://scenes/enemies/enemy_eye_drone.tscn",
            Self::QuadShell => "res://scenes/enemies/enemy_quad_shell.tscn",
        }
    }

    /// The bare visual model (no AI/collision) each enemy wears — for the
    /// bestiary briefing, which spins the model without the gameplay node.
    /// The Bomber reuses the QuadOrb model (a placeholder until it has its own).
    pub fn model_path(&self) -> &'static str {
        match self {
            Self::GunDrone =>  "res://addons/quaternius/essentials/enemies/Enemy_GunDrone.gltf",
            Self::QuadOrb =>   "res://addons/quaternius/essentials/enemies/Enemy_QuadOrb.gltf",
            Self::Bomber =>    "res://addons/quaternius/essentials/enemies/Enemy_QuadOrb.gltf",
            Self::EyeDrone =>  "res://addons/quaternius/essentials/enemies/Enemy_EyeDrone.gltf",
            Self::QuadShell => "res://addons/quaternius/essentials/enemies/Enemy_QuadShell.gltf",
        }
    }

    pub fn from_id(id: i32) -> Option<EnemyType> {
        Self::ALL.get(id as usize).copied()
    }

    pub fn id(&self) -> i32 {
        Self::ALL.iter().position(|e| e == self)
            .expect("EnemyType::ALL must contain every variant") as i32
    }

    /// Minimum level at which this enemy type first appears.
    /// Level 1 is shooters only (GunDrone); level 2 introduces the QuadOrb
    /// grabbers that slow the player so the shooters can land hits.
    pub fn min_level(&self) -> u32 {
        match self {
            Self::GunDrone => 1,
            Self::QuadOrb | Self::Bomber | Self::EyeDrone => 2,
            Self::QuadShell => 4,
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
    pub hp: Health,
    pub speed: f32,
    pub damage: Damage,
    pub detection_range: f32,
    pub attack_range: f32,
    pub attack_cooldown: f32,
    pub archetype: Archetype,
    pub(crate) reward: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::newtypes::Health;

    #[test]
    fn stats_table_matches_all_length() {
        assert_eq!(
            EnemyType::ALL.len(), EnemyType::STATS.len(),
            "STATS table ({}) must have same length as ALL ({})",
            EnemyType::STATS.len(), EnemyType::ALL.len()
        );
    }

    #[test]
    fn all_enemies_have_positive_hp() {
        for enemy in EnemyType::ALL {
            assert!(enemy.stats().hp.as_f32() > 0.0, "{:?} has non-positive hp", enemy);
        }
    }

    #[test]
    fn all_enemies_award_1000_reward() {
        for enemy in EnemyType::ALL {
            assert_eq!(enemy.stats().reward, 1_000, "{:?} doesn't award 1000 components", enemy);
        }
    }

    #[test]
    fn gun_drone_has_3_hp() {
        assert_eq!(EnemyType::GunDrone.stats().hp, Health::new(3.0));
    }

    #[test]
    fn gun_drone_is_weakest() {
        assert_eq!(EnemyType::GunDrone.stats().hp, Health::new(3.0));
    }

    #[test]
    fn quad_shell_is_strongest() {
        assert_eq!(EnemyType::QuadShell.stats().hp, Health::new(12.0));
    }

    #[test]
    fn hp_scales_with_tier() {
        let hps: Vec<f32> = EnemyType::ALL.iter().map(|e| e.stats().hp.as_f32()).collect();
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
    fn model_paths_are_gltf_resources() {
        for enemy in EnemyType::ALL {
            let path = enemy.model_path();
            assert!(path.starts_with("res://"), "{:?} bad model path", enemy);
            assert!(path.ends_with(".gltf"), "{:?} model should be a gltf", enemy);
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
    fn level_1_is_shooters_only() {
        let enemies = enemies_for_level(1);
        assert_eq!(enemies, vec![EnemyType::GunDrone],
            "level 1 should spawn only the GunDrone shooter");
    }

    #[test]
    fn level_2_introduces_the_grabber() {
        let enemies = enemies_for_level(2);
        assert!(enemies.contains(&EnemyType::GunDrone));
        assert!(enemies.contains(&EnemyType::QuadOrb), "grabbers arrive at level 2");
        assert!(!enemies.contains(&EnemyType::QuadShell));
    }

    #[test]
    fn enemies_for_level_high_includes_all() {
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
    fn roster_is_the_five_mechanical_enemies() {
        assert_eq!(EnemyType::ALL.len(), 5, "the roster is five enemies");
    }

    #[test]
    fn min_level_ordering_matches_all_ordering() {
        let levels: Vec<u32> = EnemyType::ALL.iter().map(|e| e.min_level()).collect();
        for w in levels.windows(2) {
            assert!(w[1] >= w[0], "min_level should be non-decreasing: {} >= {}", w[1], w[0]);
        }
    }

    // --- Archetype + behaviour config ---

    #[test]
    fn archetypes_match_roster() {
        assert_eq!(EnemyType::GunDrone.stats().archetype, Archetype::Kiter);
        assert_eq!(EnemyType::QuadOrb.stats().archetype, Archetype::Swarmer);
        assert_eq!(EnemyType::Bomber.stats().archetype, Archetype::Bomber);
        assert_eq!(EnemyType::EyeDrone.stats().archetype, Archetype::Kiter);
        assert_eq!(EnemyType::QuadShell.stats().archetype, Archetype::Tank);
    }

    #[test]
    fn kiter_config_has_standoff() {
        let config = EnemyType::GunDrone.ai_config();
        assert_eq!(config.archetype, Archetype::Kiter);
        assert!(config.standoff_range > 0.0);
        assert!(config.standoff_range < config.attack_range);
    }

    #[test]
    fn bomber_config_has_fuse_and_blast() {
        let config = EnemyType::Bomber.ai_config();
        assert!(config.fuse_seconds > 0.0);
        assert!(config.blast_radius > 0.0);
    }

    #[test]
    fn tank_config_has_shield() {
        let config = EnemyType::QuadShell.ai_config();
        assert!(config.shield.is_some());
    }

    #[test]
    fn non_tank_has_no_shield() {
        assert!(EnemyType::GunDrone.ai_config().shield.is_none());
    }

    #[test]
    fn eye_drone_spawns_gun_drone_on_death() {
        assert_eq!(EnemyType::EyeDrone.death_spawn(), Some((EnemyType::GunDrone, 1)));
    }

    #[test]
    fn most_enemies_have_no_death_spawn() {
        assert_eq!(EnemyType::GunDrone.death_spawn(), None);
        assert_eq!(EnemyType::QuadShell.death_spawn(), None);
    }
}
