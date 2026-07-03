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
    /// Subsidiary drone an EyeDrone coughs up on death — a distinct, weaker,
    /// faster harasser (wears the old GunDrone model) so a respawn reads as a
    /// *new* enemy, not a clone of the one you just killed. Never placed
    /// directly; only spawned (see `spawns_directly`).
    SpawnDrone,
}

impl EnemyType {
    // SpawnDrone is appended last so the original variants keep their
    // `enemy_type_id` (the `.tscn`s and save data depend on those indices).
    pub const ALL: &[EnemyType] = &[
        EnemyType::GunDrone,
        EnemyType::QuadOrb,
        EnemyType::Bomber,
        EnemyType::EyeDrone,
        EnemyType::QuadShell,
        EnemyType::SpawnDrone,
    ];

    /// Compile-time stat table indexed by variant order in `ALL`.
    const STATS: &[EnemyStats] = &[
        // GunDrone — ranged kiter: holds distance and fires. Nimble (not a
        // battleship), but the SpawnDrone it can drop stays the faster harasser.
        EnemyStats { hp: Health::new(3.0),  speed: 10.0, damage: Damage::new(5.0),  detection_range: 25.0, attack_range: 10.0, attack_cooldown: 1.0, archetype: Archetype::Kiter,   reward: 1_000 },
        // QuadOrb — swarmer: fast, fragile, four-legged; slows the player on contact.
        EnemyStats { hp: Health::new(3.0),  speed: 12.0, damage: Damage::new(4.0),  detection_range: 25.0, attack_range: 3.0,  attack_cooldown: 1.0, archetype: Archetype::Swarmer, reward: 1_000 },
        // Bomber — suicide: charges, fuses, then detonates for area damage.
        EnemyStats { hp: Health::new(4.0),  speed: 9.0,  damage: Damage::new(16.0), detection_range: 25.0, attack_range: 5.0,  attack_cooldown: 1.0, archetype: Archetype::Bomber,  reward: 1_000 },
        // EyeDrone — ranged kiter that spawns a SpawnDrone on death.
        EnemyStats { hp: Health::new(5.0),  speed: 7.0,  damage: Damage::new(6.0),  detection_range: 30.0, attack_range: 10.0, attack_cooldown: 1.2, archetype: Archetype::Kiter,   reward: 1_000 },
        // QuadShell — shielded tank: slow, durable, fires.
        EnemyStats { hp: Health::new(12.0), speed: 6.0,  damage: Damage::new(7.0),  detection_range: 25.0, attack_range: 6.0,  attack_cooldown: 1.0, archetype: Archetype::Tank,    reward: 1_000 },
        // SpawnDrone — weaker, faster harasser; only ever EyeDrone-spawned.
        EnemyStats { hp: Health::new(2.0),  speed: 11.0, damage: Damage::new(3.0),  detection_range: 25.0, attack_range: 8.0,  attack_cooldown: 1.2, archetype: Archetype::Kiter,   reward: 1_000 },
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
            Self::EyeDrone => Some((Self::SpawnDrone, 1)),
            Self::GunDrone | Self::QuadOrb | Self::Bomber | Self::QuadShell
            | Self::SpawnDrone => None,
        }
    }

    /// Whether this type is placed directly into a room's spawn list. Spawn-only
    /// types (the SpawnDrone) appear solely as another drone's death spawn.
    pub fn spawns_directly(&self) -> bool {
        !matches!(self, Self::SpawnDrone)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::GunDrone => "Gun Drone",
            Self::QuadOrb => "Quad Orb",
            Self::Bomber => "Bomber",
            Self::EyeDrone => "Eye Drone",
            Self::QuadShell => "Quad Shell",
            Self::SpawnDrone => "Spawn Drone",
        }
    }

    /// Every enemy is the same node + collider scene; the type (set at spawn)
    /// drives stats, model, and collider size. One scene, not one per variant.
    pub fn scene_path(&self) -> &'static str {
        "res://scenes/enemies/enemy.tscn"
    }


    /// The bare visual model (no AI/collision) each enemy wears — for the
    /// bestiary briefing, which spins the model without the gameplay node.
    /// The Bomber reuses the QuadOrb model (a placeholder until it has its own).
    pub fn model_path(&self) -> &'static str {
        match self {
            // The two front-line drones wear the cgtrader evil-mech models;
            // the SpawnDrone inherits the GunDrone's old Quaternius model so a
            // respawn is visibly a different, lesser machine.
            Self::GunDrone =>  "res://addons/enemies/evil_mech_03.glb",
            Self::QuadOrb =>   "res://addons/enemies/evil_mech_01.glb",
            Self::Bomber =>    "res://addons/quaternius/essentials/enemies/Enemy_QuadOrb.gltf",
            Self::EyeDrone =>  "res://addons/quaternius/essentials/enemies/Enemy_EyeDrone.gltf",
            Self::QuadShell => "res://addons/quaternius/essentials/enemies/Enemy_QuadShell.gltf",
            Self::SpawnDrone => "res://addons/quaternius/essentials/enemies/Enemy_GunDrone.gltf",
        }
    }

    /// Longest-edge target the model is fit-scaled to (meters). The GunDrone
    /// mech is the bruiser at 2 m, the QuadOrb mech at 1 m; the rest keep the
    /// ~0.5 m drone size.
    pub fn model_size(&self) -> f32 {
        match self {
            Self::GunDrone => 2.0,
            Self::QuadOrb => 1.0,
            Self::Bomber | Self::EyeDrone | Self::QuadShell | Self::SpawnDrone => 0.5,
        }
    }

    /// Extra yaw (radians) layered on top of "face the player", correcting for
    /// the model's imported front axis. The cgtrader mechs import facing their
    /// own +X (their flank points down -Z), so they need a quarter turn to put
    /// their nose on the player; the Quaternius drones already front along -Z.
    /// This is the single knob to tune if a model ends up facing askew in-game.
    pub fn model_yaw_offset(&self) -> f32 {
        match self {
            // The cgtrader mechs front along +Z, so look_at (which aims -Z at the
            // player) leaves them facing exactly backwards — a half turn fixes it.
            Self::GunDrone | Self::QuadOrb => std::f32::consts::PI,
            Self::Bomber | Self::EyeDrone | Self::QuadShell | Self::SpawnDrone => 0.0,
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
            // Appears (via EyeDrone death) from level 3; never placed directly.
            Self::SpawnDrone => 3,
            Self::QuadShell => 4,
        }
    }
}

/// Returns which enemy types can be placed directly at a given level. Spawn-only
/// types (the SpawnDrone) are excluded — they appear solely as death spawns.
pub fn enemies_for_level(level: u32) -> Vec<EnemyType> {
    EnemyType::ALL
        .iter()
        .filter(|e| e.spawns_directly() && e.min_level() <= level)
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
        // The directly-spawnable roster is ordered by tier; the spawn-only
        // SpawnDrone sits outside that progression.
        let hps: Vec<f32> = EnemyType::ALL.iter()
            .filter(|e| e.spawns_directly())
            .map(|e| e.stats().hp.as_f32()).collect();
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
    fn model_paths_are_mesh_resources() {
        for enemy in EnemyType::ALL {
            let path = enemy.model_path();
            assert!(path.starts_with("res://"), "{:?} bad model path", enemy);
            assert!(
                path.ends_with(".gltf") || path.ends_with(".glb") || path.ends_with(".fbx"),
                "{:?} model should be a gltf/glb/fbx", enemy
            );
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
    fn enemies_for_level_high_includes_all_directly_spawnable() {
        let enemies = enemies_for_level(8);
        let direct = EnemyType::ALL.iter().filter(|e| e.spawns_directly()).count();
        assert_eq!(enemies.len(), direct);
    }

    #[test]
    fn spawn_drone_is_never_placed_directly() {
        assert!(!EnemyType::SpawnDrone.spawns_directly());
        for level in 1..=10 {
            assert!(
                !enemies_for_level(level).contains(&EnemyType::SpawnDrone),
                "SpawnDrone must never be in the direct spawn list (level {level})"
            );
        }
    }

    #[test]
    fn spawn_drone_is_weaker_and_faster_than_gun_drone() {
        let spawn = EnemyType::SpawnDrone.stats();
        let gun = EnemyType::GunDrone.stats();
        assert!(spawn.hp.as_f32() < gun.hp.as_f32(), "SpawnDrone is weaker");
        assert!(spawn.speed > gun.speed, "SpawnDrone is faster");
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
    fn roster_is_five_direct_enemies_plus_the_spawn_drone() {
        assert_eq!(EnemyType::ALL.len(), 6, "six types total");
        let direct = EnemyType::ALL.iter().filter(|e| e.spawns_directly()).count();
        assert_eq!(direct, 5, "five are placed directly; the SpawnDrone is spawn-only");
    }

    #[test]
    fn min_level_ordering_matches_all_ordering() {
        // Only the directly-spawnable roster is tier-ordered.
        let levels: Vec<u32> = EnemyType::ALL.iter()
            .filter(|e| e.spawns_directly())
            .map(|e| e.min_level()).collect();
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
    fn eye_drone_spawns_a_spawn_drone_on_death() {
        assert_eq!(EnemyType::EyeDrone.death_spawn(), Some((EnemyType::SpawnDrone, 1)));
    }

    #[test]
    fn most_enemies_have_no_death_spawn() {
        assert_eq!(EnemyType::GunDrone.death_spawn(), None);
        assert_eq!(EnemyType::QuadShell.death_spawn(), None);
        assert_eq!(EnemyType::SpawnDrone.death_spawn(), None);
    }
}
