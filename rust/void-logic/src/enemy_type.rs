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

    /// This variant's roster def — THE data source (rosters/enemies.toml).
    /// The enum survives as the typed handle (saves, GDScript crossings,
    /// exhaustive matches); every VALUE lives in the grammar.
    fn def(&self) -> &'static crate::roster::EnemyDef {
        let roster = crate::roster::roster();
        let id = roster
            .enemy_by_crossing_id(self.id() as u16)
            .expect("every EnemyType variant has a roster def (linker-checked)");
        roster.enemy(id)
    }

    pub fn stats(&self) -> EnemyStats {
        let d = self.def();
        EnemyStats {
            hp: Health::new(d.stats.hp),
            speed: d.stats.speed,
            damage: Damage::new(d.stats.damage),
            detection_range: d.stats.detection,
            attack_range: d.stats.attack_range,
            attack_cooldown: d.stats.cooldown,
            archetype: d.ai,
            reward: d.reward,
        }
    }

    /// Components carried by the cache this enemy drops on death. The reward is
    /// only ever credited through that pickup, never directly on the kill.
    pub fn reward(&self) -> u32 {
        self.def().reward
    }

    /// The AI configuration: ranges from the def's stats, archetype
    /// parameters from its behaviour switches (declared or derived —
    /// see rosters/VOCABULARY.md "Behaviour switches").
    pub fn ai_config(&self) -> DroneConfig {
        let d = self.def();
        DroneConfig {
            archetype: d.ai,
            detection_range: d.stats.detection,
            attack_range: d.stats.attack_range,
            disengage_range: d.behavior.disengage,
            health: Health::new(d.stats.hp),
            attack_cooldown: d.stats.cooldown,
            standoff_range: d.behavior.standoff,
            fuse_seconds: d.behavior.fuse_seconds,
            blast_radius: d.behavior.blast_radius,
            shield: (d.behavior.shield > 0.0).then(|| Shield::new(d.behavior.shield)),
        }
    }

    /// Hull drain per second while latched (0 for non-drainers) — the
    /// `drain_dps` switch on the def.
    pub fn drain_dps(&self) -> f32 {
        self.def().behavior.drain_dps
    }

    /// Pre-staged bound minions and their rise triggers, straight from the
    /// def's `minions` list (the general form of the old death-spawn).
    pub fn minions(&self) -> Vec<(EnemyType, u8, crate::level_assembly::MinionTrigger)> {
        let roster = crate::roster::roster();
        self.def()
            .minions
            .iter()
            .map(|(id, count, trigger)| {
                let minion = EnemyType::from_id(roster.enemy(*id).crossing_id as i32)
                    .expect("crossing ids round-trip");
                (minion, *count, *trigger)
            })
            .collect()
    }

    /// Whether this type is placed directly into a room's spawn list
    /// (`spawns_directly` on the def; the linker refuses rosters that
    /// list a false one).
    pub fn spawns_directly(&self) -> bool {
        self.def().spawns_directly
    }

    pub fn display_name(&self) -> &'static str {
        self.def().name.as_str()
    }

    /// Every enemy is the same node + collider scene; the type (set at spawn)
    /// drives stats, model, and collider size. One scene, not one per variant.
    pub fn scene_path(&self) -> &'static str {
        "res://scenes/enemies/enemy.tscn"
    }

    /// The bare visual model (no AI/collision) this enemy wears.
    pub fn model_path(&self) -> &'static str {
        self.def().model.as_str()
    }

    /// Longest-edge target the model is fit-scaled to (meters).
    pub fn model_size(&self) -> f32 {
        self.def().size
    }

    /// Extra yaw (radians) layered on top of "face the player", correcting
    /// for the model's imported front axis (declared in degrees on the def).
    pub fn model_yaw_offset(&self) -> f32 {
        self.def().yaw_offset_deg.to_radians()
    }

    /// Per-level stat multipliers — the def's declared curves. These replace
    /// the old global `difficulty` module: scaling is per-enemy grammar.
    pub fn speed_multiplier(&self, level: u32) -> f32 {
        crate::roster::roster().curve(self.def().scaling.speed).eval(level)
    }

    pub fn cooldown_multiplier(&self, level: u32) -> f32 {
        crate::roster::roster().curve(self.def().scaling.cooldown).eval(level)
    }

    pub fn hp_multiplier(&self, level: u32) -> f32 {
        crate::roster::roster().curve(self.def().scaling.hp).eval(level)
    }

    pub fn from_id(id: i32) -> Option<EnemyType> {
        Self::ALL.get(id as usize).copied()
    }

    pub fn id(&self) -> i32 {
        Self::ALL.iter().position(|e| e == self)
            .expect("EnemyType::ALL must contain every variant") as i32
    }

    // Scheduling facts (WHEN a type appears) live on the level side —
    // the planet files' [[level]] rosters — not here (one truth, one door;
    // owner's call 2026-07-05). This type carries only identity.
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
    use crate::level_assembly::MinionTrigger;

    // Every VALUE lives in rosters/enemies.toml now — tests here pin SHAPES
    // (balance relationships, identity law), never restate the data: exact
    // numbers are the owner's tuning surface, not a regression target.

    #[test]
    fn all_enemies_have_positive_hp() {
        for enemy in EnemyType::ALL {
            assert!(enemy.stats().hp.as_f32() > 0.0, "{:?} has non-positive hp", enemy);
        }
    }

    #[test]
    fn rewards_scale_with_toughness() {
        assert!(EnemyType::QuadShell.reward() > EnemyType::SphereGunner.reward(),
            "the tank out-pays the basic gunner");
        assert!(EnemyType::SphereCarrier.reward() > EnemyType::SphereGunner.reward(),
            "the carrier out-pays the basic gunner");
        assert!(EnemyType::SphereCarrier.reward() > EnemyType::AlienTroop.reward(),
            "the carrier out-pays the latcher");
    }

    #[test]
    fn spawn_drone_reward_is_the_smallest() {
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
    fn hp_scales_with_toughness() {
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
        assert_eq!(EnemyType::SphereCarrier.minions(),
            vec![(EnemyType::SpawnDrone, 2, MinionTrigger::OnDeath)],
            "later levels' spawn-drone pressure comes from the carrier");
    }

    #[test]
    fn eye_drone_spawns_a_spawn_drone_on_death() {
        assert_eq!(EnemyType::EyeDrone.minions(),
            vec![(EnemyType::SpawnDrone, 1, MinionTrigger::OnDeath)]);
    }

    #[test]
    fn most_enemies_carry_no_minions() {
        assert!(EnemyType::GunDrone.minions().is_empty());
        assert!(EnemyType::QuadShell.minions().is_empty());
        assert!(EnemyType::SpawnDrone.minions().is_empty());
    }

    #[test]
    fn spawn_drone_is_never_placed_directly() {
        assert!(!EnemyType::SpawnDrone.spawns_directly(),
            "the SpawnDrone exists only as another machine's death spawn");
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

    #[test]
    fn boss_stats_dwarf_the_line_roster() {
        let toughest_regular = EnemyType::QuadShell.stats();
        for boss in [EnemyType::BossBrute, EnemyType::BossLatcher] {
            let s = boss.stats();
            assert!(s.hp.as_f32() >= toughest_regular.hp.as_f32() * 4.0,
                "{boss:?} hp {} must dwarf the QuadShell's {}",
                s.hp.as_f32(), toughest_regular.hp.as_f32());
            assert!(boss.model_size() > EnemyType::QuadShell.model_size() * 2.0,
                "{boss:?} reads at arena scale");
        }
    }

    #[test]
    fn only_the_latcher_drains_today() {
        assert!(EnemyType::BossLatcher.drain_dps() > 0.0,
            "the planet warden siphons hulls");
        for enemy in EnemyType::ALL {
            if *enemy != EnemyType::BossLatcher {
                assert_eq!(enemy.drain_dps(), 0.0, "{enemy:?} must not drain");
            }
        }
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
    fn scaling_curves_ease_the_early_game() {
        // Shape, not values: level 1 is gentler than the peak on every
        // enemy's declared curves; HP curves stay within sane bounds.
        for enemy in EnemyType::ALL {
            assert!(enemy.speed_multiplier(1) <= enemy.speed_multiplier(30),
                "{enemy:?} speed must not shrink with level");
            assert!(enemy.cooldown_multiplier(1) >= enemy.cooldown_multiplier(30),
                "{enemy:?} fire must not slow with level");
            assert!(enemy.hp_multiplier(1) > 0.0);
        }
    }
}
