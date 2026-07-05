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
    /// Basic sphere gunner — the front-line shooter from level 1 (the
    /// evil-mech models graduated to bosses, 2026-07-04).
    SphereGunner,
    /// Strafing sphere — the kiting standoff role the GunDrone mech held.
    SphereStriker,
    /// Shielded sphere carrier — a tank that releases SpawnDrones on death
    /// (later levels' spawn pressure).
    SphereCarrier,
    /// Alien latcher — the swarming slow-tagger role the QuadOrb mech held.
    AlienTroop,
    /// Planet 1's basic front-line gunner — the Quaternius fleet keeps the
    /// early game (owner's call 2026-07-04); the white spheres are planet-2
    /// machines. Wears the previously unused Raptor model.
    SentryDrone,
    /// Mid-planet boss: the evil_mech_03 bruiser at arena scale. Kites,
    /// hits like a siege engine, and coughs up a drone trio on death.
    /// Placed only by the boss schedule (planet-relative level 3).
    BossBrute,
    /// Planet-final boss: the evil_mech_01 drainer. Latches on with a hull
    /// siphon and fights behind a circling SpawnDrone escort that rises the
    /// moment the fight starts. Placed only at planet-relative level 6.
    BossLatcher,
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
        EnemyType::SphereGunner,
        EnemyType::SphereStriker,
        EnemyType::SphereCarrier,
        EnemyType::AlienTroop,
        EnemyType::SentryDrone,
        EnemyType::BossBrute,
        EnemyType::BossLatcher,
    ];

    /// Compile-time stat table indexed by variant order in `ALL`.
    const STATS: &[EnemyStats] = &[
        // GunDrone — ranged kiter: holds distance and fires. Nimble (not a
        // battleship), but the SpawnDrone it can drop stays the faster harasser.
        EnemyStats { hp: Health::new(3.0),  speed: 10.0, damage: Damage::new(5.0),  detection_range: 25.0, attack_range: 10.0, attack_cooldown: 1.0, archetype: Archetype::Kiter,   reward: 800 },
        // QuadOrb — swarmer: fast, fragile, four-legged; latches within 2m and
        // re-tags a compounding slow while it stays close (no contact needed).
        EnemyStats { hp: Health::new(3.0),  speed: 12.0, damage: Damage::new(4.0),  detection_range: 25.0, attack_range: 3.0,  attack_cooldown: 1.0, archetype: Archetype::Swarmer, reward: 900 },
        // Bomber — suicide: charges, fuses, then detonates for area damage.
        EnemyStats { hp: Health::new(4.0),  speed: 9.0,  damage: Damage::new(16.0), detection_range: 25.0, attack_range: 5.0,  attack_cooldown: 1.0, archetype: Archetype::Bomber,  reward: 1_200 },
        // EyeDrone — ranged kiter that spawns a SpawnDrone on death.
        EnemyStats { hp: Health::new(5.0),  speed: 7.0,  damage: Damage::new(6.0),  detection_range: 30.0, attack_range: 10.0, attack_cooldown: 1.2, archetype: Archetype::Kiter,   reward: 1_500 },
        // QuadShell — shielded tank: slow, durable, fires.
        EnemyStats { hp: Health::new(12.0), speed: 6.0,  damage: Damage::new(7.0),  detection_range: 25.0, attack_range: 6.0,  attack_cooldown: 1.0, archetype: Archetype::Tank,    reward: 2_500 },
        // SpawnDrone — weaker, faster harasser; only ever death-spawned.
        EnemyStats { hp: Health::new(2.0),  speed: 11.0, damage: Damage::new(3.0),  detection_range: 25.0, attack_range: 8.0,  attack_cooldown: 1.2, archetype: Archetype::Kiter,   reward: 400 },
        // SphereGunner — the basic front-line shooter, level 1 on.
        EnemyStats { hp: Health::new(3.0),  speed: 9.0,  damage: Damage::new(5.0),  detection_range: 25.0, attack_range: 10.0, attack_cooldown: 1.0, archetype: Archetype::Shooter, reward: 800 },
        // SphereStriker — nimble standoff kiter (the old GunDrone role).
        EnemyStats { hp: Health::new(3.0),  speed: 12.0, damage: Damage::new(4.0),  detection_range: 25.0, attack_range: 10.0, attack_cooldown: 1.0, archetype: Archetype::Kiter,   reward: 900 },
        // SphereCarrier — shielded tank; releases SpawnDrones on death.
        EnemyStats { hp: Health::new(10.0), speed: 6.0,  damage: Damage::new(6.0),  detection_range: 25.0, attack_range: 8.0,  attack_cooldown: 1.2, archetype: Archetype::Tank,    reward: 2_200 },
        // AlienTroop — swarming latcher (the old QuadOrb role).
        EnemyStats { hp: Health::new(3.0),  speed: 12.0, damage: Damage::new(4.0),  detection_range: 25.0, attack_range: 3.0,  attack_cooldown: 1.0, archetype: Archetype::Swarmer, reward: 1_000 },
        // SentryDrone — planet 1's basic front-line shooter.
        EnemyStats { hp: Health::new(3.0),  speed: 9.0,  damage: Damage::new(5.0),  detection_range: 25.0, attack_range: 10.0, attack_cooldown: 1.0, archetype: Archetype::Shooter, reward: 800 },
        // BossBrute — the siege engine: kites at range, huge pool, crushing
        // shots. Detection spans the whole arena; there is no sneaking past.
        EnemyStats { hp: Health::new(60.0), speed: 8.0,  damage: Damage::new(16.0), detection_range: 45.0, attack_range: 14.0, attack_cooldown: 1.4, archetype: Archetype::Kiter,   reward: 10_000 },
        // BossLatcher — the planet warden: even deeper pool, closes and
        // clamps; its real damage is the DrainDebuff wired through the
        // swarm-latch check, so contact damage stays modest.
        EnemyStats { hp: Health::new(80.0), speed: 10.0, damage: Damage::new(6.0),  detection_range: 45.0, attack_range: 3.0,  attack_cooldown: 1.0, archetype: Archetype::Swarmer, reward: 12_000 },
    ];

    pub fn stats(&self) -> EnemyStats {
        Self::STATS[Self::ALL.iter().position(|e| e == self)
            .expect("EnemyType::ALL must contain every variant")]
    }

    /// Components carried by the cache this enemy drops on death. The reward is
    /// only ever credited through that pickup, never directly on the kill.
    pub fn reward(&self) -> u32 {
        self.stats().reward
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
            Archetype::Tank => {
                config.shield = Some(Shield::new(s.hp.as_f32() * 0.5));
                // Tanks and Shooters strafe when sight-blocked; the standoff
                // keeps that orbit at firing distance instead of spiraling
                // into the player's face (the 2026-07-04 hugging regression).
                config.standoff_range = s.attack_range * 0.6;
            }
            Archetype::Shooter => config.standoff_range = s.attack_range * 0.6,
            Archetype::Swarmer => {}
        }
        config
    }

    /// Enemies this type spawns when it dies (the "subsidiary drone" mechanic).
    pub fn death_spawn(&self) -> Option<(EnemyType, u8)> {
        match self {
            Self::EyeDrone => Some((Self::SpawnDrone, 1)),
            Self::SphereCarrier => Some((Self::SpawnDrone, 2)),
            // Boss composition (what they field, when, how many) is FIGHT
            // design, not an intrinsic fact — it lives on `BossKind::minions`
            // + `BossStaging.adds` (audit 2026-07-05), never here.
            Self::GunDrone | Self::QuadOrb | Self::Bomber | Self::QuadShell
            | Self::SpawnDrone | Self::SphereGunner | Self::SphereStriker
            | Self::AlienTroop | Self::SentryDrone
            | Self::BossBrute | Self::BossLatcher => None,
        }
    }

    /// Whether this type is placed directly into a room's spawn list.
    /// Spawn-only types (the SpawnDrone) appear solely as another drone's
    /// death spawn; the evil-mech pair keep their ids but graduated to boss
    /// duty (owner's call 2026-07-04) and never place directly again.
    pub fn spawns_directly(&self) -> bool {
        !matches!(
            self,
            Self::SpawnDrone | Self::GunDrone | Self::QuadOrb
                | Self::BossBrute | Self::BossLatcher
        )
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::GunDrone => "Gun Drone",
            Self::QuadOrb => "Quad Orb",
            Self::Bomber => "Bomber",
            Self::EyeDrone => "Eye Drone",
            Self::QuadShell => "Quad Shell",
            Self::SpawnDrone => "Spawn Drone",
            Self::SphereGunner => "Sphere Gunner",
            Self::SphereStriker => "Sphere Striker",
            Self::SphereCarrier => "Sphere Carrier",
            Self::AlienTroop => "Alien Troop",
            Self::SentryDrone => "Sentry Drone",
            Self::BossBrute => "Siege Mech",
            Self::BossLatcher => "Lamprey Mech",
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
            // The sphere fleet: cgtrader FBX through the same decimation
            // pipeline as the mechs (install-addons.sh spheres pass).
            Self::SphereGunner => "res://addons/enemies/sphere_ship_01.glb",
            Self::SphereStriker => "res://addons/enemies/sphere_ship_02.glb",
            Self::SphereCarrier => "res://addons/enemies/sphere_ship_03.glb",
            Self::AlienTroop => "res://addons/enemies/alien_troop_01.glb",
            // Planet 1's basic gunner: the previously unused Quaternius
            // Raptor, so it never reads as a SpawnDrone clone.
            Self::SentryDrone => "res://addons/quaternius/essentials/enemies/Enemy_Raptor.gltf",
            // The bosses inherit the mechs' models at arena scale.
            Self::BossBrute => "res://addons/enemies/evil_mech_03.glb",
            Self::BossLatcher => "res://addons/enemies/evil_mech_01.glb",
        }
    }

    /// Longest-edge target the model is fit-scaled to (meters). The GunDrone
    /// mech is the bruiser at 2 m, the QuadOrb mech at 1 m; the rest keep the
    /// ~0.5 m drone size.
    pub fn model_size(&self) -> f32 {
        match self {
            Self::GunDrone => 2.0,
            Self::QuadOrb => 1.0,
            Self::Bomber | Self::EyeDrone | Self::QuadShell | Self::SpawnDrone
            | Self::SentryDrone => 0.5,
            Self::SphereGunner | Self::SphereStriker | Self::AlienTroop => 1.0,
            Self::SphereCarrier => 1.4,
            // Arena scale: the Brute doubles the old GunDrone mech's 2m.
            Self::BossBrute => 4.0,
            Self::BossLatcher => 3.0,
        }
    }

    /// Extra yaw (radians) layered on top of "face the player", correcting for
    /// the model's imported front axis. The cgtrader mechs import fronting
    /// along +Z, so `look_at` (which aims -Z at the player) leaves them facing
    /// exactly backwards — a half turn fixes it; the Quaternius drones already
    /// front along -Z and need nothing.
    /// This is the single knob to tune if a model ends up facing askew in-game.
    pub fn model_yaw_offset(&self) -> f32 {
        match self {
            // The cgtrader mechs front along +Z, so look_at (which aims -Z at the
            // player) leaves them facing exactly backwards — a half turn fixes it.
            Self::GunDrone | Self::QuadOrb
            | Self::BossBrute | Self::BossLatcher => std::f32::consts::PI,
            Self::Bomber | Self::EyeDrone | Self::QuadShell | Self::SpawnDrone
            | Self::SentryDrone => 0.0,
            // cgtrader imports front along +Z like the mechs — the single
            // knob to tune on the first visual pass if any face askew.
            Self::SphereGunner | Self::SphereStriker | Self::SphereCarrier
            | Self::AlienTroop => std::f32::consts::PI,
        }
    }

    pub fn from_id(id: i32) -> Option<EnemyType> {
        Self::ALL.get(id as usize).copied()
    }

    pub fn id(&self) -> i32 {
        Self::ALL.iter().position(|e| e == self)
            .expect("EnemyType::ALL must contain every variant") as i32
    }

    // Scheduling facts (WHEN a type appears) live on the level side —
    // `level_spec::ROSTER_SCHEDULE` — not here (one truth, one door;
    // owner's call 2026-07-05). This type carries only intrinsic facts.
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
    fn rewards_scale_with_toughness() {
        // Ids are append-only, so ALL is no longer tier-ordered — the pin is
        // per-pair: tougher kills drop richer caches.
        assert!(EnemyType::QuadShell.reward() > EnemyType::SphereGunner.reward(),
            "the tank out-pays the basic gunner");
        assert!(EnemyType::SphereCarrier.reward() > EnemyType::SphereGunner.reward(),
            "the carrier out-pays the basic gunner");
        assert!(EnemyType::SphereCarrier.reward() > EnemyType::AlienTroop.reward(),
            "the carrier out-pays the latcher");
    }

    #[test]
    fn spawn_drone_reward_is_the_smallest() {
        // The spawn-only harasser is a lesser machine; its cache pays least.
        for enemy in EnemyType::ALL {
            if *enemy != EnemyType::SpawnDrone {
                assert!(EnemyType::SpawnDrone.reward() < enemy.reward(),
                    "SpawnDrone must pay less than {enemy:?}");
            }
        }
    }

    #[test]
    fn rewards_are_positive() {
        for enemy in EnemyType::ALL {
            assert!(enemy.reward() > 0, "{enemy:?} must drop a non-empty cache");
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
    fn hp_scales_with_toughness() {
        // Per-pair pins (ALL is id-stable, not tier-ordered).
        assert!(EnemyType::SphereCarrier.stats().hp.as_f32()
            > EnemyType::SphereGunner.stats().hp.as_f32(),
            "the carrier tank outlasts the basic gunner");
        assert!(EnemyType::QuadShell.stats().hp.as_f32()
            > EnemyType::SphereStriker.stats().hp.as_f32());
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
    fn the_carrier_releases_two_spawn_drones_on_death() {
        assert_eq!(EnemyType::SphereCarrier.death_spawn(),
            Some((EnemyType::SpawnDrone, 2)),
            "later levels' spawn-drone pressure comes from the carrier");
    }

    #[test]
    fn spawn_drone_is_never_placed_directly() {
        assert!(!EnemyType::SpawnDrone.spawns_directly(),
            "the SpawnDrone exists only as another machine's death spawn");
    }

    #[test]
    fn spawn_drone_is_weaker_and_faster_than_gun_drone() {
        let spawn = EnemyType::SpawnDrone.stats();
        let gun = EnemyType::GunDrone.stats();
        assert!(spawn.hp.as_f32() < gun.hp.as_f32(), "SpawnDrone is weaker");
        assert!(spawn.speed > gun.speed, "SpawnDrone is faster");
    }

    #[test]
    fn roster_is_eight_direct_enemies_plus_the_reserves() {
        // 13 variants: 8 direct (SentryDrone, Bomber, EyeDrone, QuadShell,
        // the three spheres, the alien), the spawn-only SpawnDrone, the two
        // retired mech ids, and the two schedule-placed bosses.
        assert_eq!(EnemyType::ALL.len(), 13, "thirteen types total");
        let direct = EnemyType::ALL.iter().filter(|e| e.spawns_directly()).count();
        assert_eq!(direct, 8, "eight place directly");
    }

    #[test]
    fn legacy_ids_are_stable() {
        // Append-only law: ids never move (saves, bestiary, and the .tscn
        // crossing depend on them). Every append extends this pin.
        assert_eq!(EnemyType::GunDrone.id(), 0);
        assert_eq!(EnemyType::QuadOrb.id(), 1);
        assert_eq!(EnemyType::Bomber.id(), 2);
        assert_eq!(EnemyType::EyeDrone.id(), 3);
        assert_eq!(EnemyType::QuadShell.id(), 4);
        assert_eq!(EnemyType::SpawnDrone.id(), 5);
        assert_eq!(EnemyType::SphereGunner.id(), 6);
        assert_eq!(EnemyType::SphereStriker.id(), 7);
        assert_eq!(EnemyType::SphereCarrier.id(), 8);
        assert_eq!(EnemyType::AlienTroop.id(), 9);
        assert_eq!(EnemyType::SentryDrone.id(), 10);
        assert_eq!(EnemyType::BossBrute.id(), 11);
        assert_eq!(EnemyType::BossLatcher.id(), 12);
    }

    // --- Bosses (B5) ---

    #[test]
    fn bosses_wear_the_evil_mechs_at_boss_scale() {
        // The mechs were "way too cool" for the line — they ARE the bosses.
        assert_eq!(EnemyType::BossBrute.model_path(),
            "res://addons/enemies/evil_mech_03.glb");
        assert_eq!(EnemyType::BossLatcher.model_path(),
            "res://addons/enemies/evil_mech_01.glb");
        assert_eq!(EnemyType::BossBrute.model_size(), 4.0,
            "twice the old GunDrone mech's 2m — the arena sells the scale");
        assert!(EnemyType::BossLatcher.model_size() >= 2.5);
    }

    #[test]
    fn boss_stats_dwarf_the_line_roster() {
        let toughest_regular = EnemyType::QuadShell.stats();
        for boss in [EnemyType::BossBrute, EnemyType::BossLatcher] {
            let s = boss.stats();
            assert!(s.hp.as_f32() >= toughest_regular.hp.as_f32() * 4.0,
                "{boss:?} hp {} must dwarf the QuadShell's {}",
                s.hp.as_f32(), toughest_regular.hp.as_f32());
        }
        assert!(EnemyType::BossBrute.stats().damage.as_f32()
            >= toughest_regular.damage.as_f32() * 2.0,
            "the Brute hits like a siege engine");
    }

    #[test]
    fn boss_intrinsics_stay_but_composition_lives_on_the_fight_side() {
        // Archetypes are intrinsic; what a boss FIELDS is fight design
        // (`BossKind::minions` + staging) — the tables here stay silent.
        assert_eq!(EnemyType::BossBrute.stats().archetype, Archetype::Kiter);
        assert_eq!(EnemyType::BossLatcher.stats().archetype, Archetype::Swarmer);
        assert_eq!(EnemyType::BossBrute.death_spawn(), None);
        assert_eq!(EnemyType::BossLatcher.death_spawn(), None);
    }

    // --- Archetype + behaviour config ---

    #[test]
    fn archetypes_match_roster() {
        assert_eq!(EnemyType::GunDrone.stats().archetype, Archetype::Kiter);
        assert_eq!(EnemyType::QuadOrb.stats().archetype, Archetype::Swarmer);
        assert_eq!(EnemyType::Bomber.stats().archetype, Archetype::Bomber);
        assert_eq!(EnemyType::EyeDrone.stats().archetype, Archetype::Kiter);
        assert_eq!(EnemyType::QuadShell.stats().archetype, Archetype::Tank);
        assert_eq!(EnemyType::SphereGunner.stats().archetype, Archetype::Shooter);
        assert_eq!(EnemyType::SphereStriker.stats().archetype, Archetype::Kiter);
        assert_eq!(EnemyType::SphereCarrier.stats().archetype, Archetype::Tank);
        assert_eq!(EnemyType::AlienTroop.stats().archetype, Archetype::Swarmer);
        assert_eq!(EnemyType::SentryDrone.stats().archetype, Archetype::Shooter);
    }

    #[test]
    fn the_sentry_wears_the_unused_quaternius_raptor() {
        // Planet 1's basic gunner reuses the Quaternius fleet (owner's call
        // 2026-07-04) — the Raptor, so it never reads as a SpawnDrone clone.
        assert_eq!(EnemyType::SentryDrone.model_path(),
            "res://addons/quaternius/essentials/enemies/Enemy_Raptor.gltf");
    }

    #[test]
    fn sphere_models_come_from_the_decimation_pipeline() {
        assert_eq!(EnemyType::SphereGunner.model_path(),
            "res://addons/enemies/sphere_ship_01.glb");
        assert_eq!(EnemyType::SphereStriker.model_path(),
            "res://addons/enemies/sphere_ship_02.glb");
        assert_eq!(EnemyType::SphereCarrier.model_path(),
            "res://addons/enemies/sphere_ship_03.glb");
        assert_eq!(EnemyType::AlienTroop.model_path(),
            "res://addons/enemies/alien_troop_01.glb");
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

    // --- Level coverage (bestiary: direct + death-spawn types) ---

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
