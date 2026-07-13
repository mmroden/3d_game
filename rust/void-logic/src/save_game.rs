//! Persistence, split by lifetime: the [`Profile`] (permanent — organics,
//! bestiary) always survives; the [`RunSnapshot`] exists only while there is
//! a continuable run, written at level start once at least one level has
//! been finished, and cleared on run-over. "Continue" on the main menu is
//! gated on the snapshot's presence — a run that never finished a level is
//! not continuable.

use crate::currency::ComponentAccount;
use crate::laser::LaserLevel;
use crate::loadout::Loadout;
use crate::newtypes::Health;
use crate::run_state::{Profile, RunState};
use crate::seed::Seed;
use crate::shield::ShieldState;
use crate::ship::ShipColor;
use crate::ship_type::ShipType;
use serde::{Deserialize, Serialize};

/// A continuable run, frozen at the start of its current level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunSnapshot {
    pub laser_level: LaserLevel,
    pub loadout: Loadout,
    pub score: u32,
    pub current_level: u32,
    pub run_seed: Seed,
    pub components: ComponentAccount,
    pub health: Health,
    pub shield: ShieldState,
    pub ship_color: ShipColor,
    /// Chosen hull.
    pub ship_type: ShipType,
    pub lives: u32,
    pub lives_purchased: u32,
    pub shield_charges: u32,
    /// Valkyrie charge-row upgrades (blue, playtest 2026-07-06).
    pub valkyrie_bars_bought: u32,
    pub valkyrie_refill_level: u32,
}

/// Everything persisted to disk: the permanent profile plus, while a run is
/// continuable, its snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveGame {
    pub profile: Profile,
    pub run: Option<RunSnapshot>,
}

impl SaveGame {
    /// Serialize to JSON for persistence (one string, stored via the
    /// same ConfigFile mechanism as options).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Reconstruct from a persisted JSON string; `None` if it does not parse
    /// as the CURRENT shape. There is no legacy migration by design (owner
    /// decision, 2026-07-03): an unparseable save means a fresh start.
    pub fn from_json(s: &str) -> Option<Self> {
        serde_json::from_str(s).ok()
    }

    /// Snapshot a run in full: profile and run snapshot together. Used at
    /// level start once the run is continuable.
    pub fn from_run_state(run: &RunState) -> Self {
        Self {
            profile: run.profile.clone(),
            run: Some(RunSnapshot {
                laser_level: run.laser_level,
                loadout: run.loadout.clone(),
                score: run.score,
                current_level: run.current_level,
                run_seed: run.run_seed,
                components: run.components,
                health: run.health,
                shield: run.shield.clone(),
                ship_color: run.ship_color,
                ship_type: run.ship_type,
                lives: run.lives,
                lives_purchased: run.lives_purchased,
                shield_charges: run.shield_charges,
                valkyrie_bars_bought: run.valkyrie_bars_bought,
                valkyrie_refill_level: run.valkyrie_refill_level,
            }),
        }
    }

    /// A profile with no continuable run (fresh install, or after run-over).
    pub fn profile_only(run: &RunState) -> Self {
        Self {
            profile: run.profile.clone(),
            run: None,
        }
    }

    /// Whether a continuable run exists — the main menu's Continue gate.
    pub fn has_run(&self) -> bool {
        self.run.is_some()
    }

    /// Refresh the permanent profile from the live run WITHOUT touching the
    /// stored run snapshot: green pickups and bestiary growth are permanent
    /// the moment they happen, but the run stays frozen at its level start.
    pub fn update_profile(&mut self, run: &RunState) {
        self.profile = run.profile.clone();
    }

    /// Drop the run snapshot (run-over): the profile survives, Continue goes.
    pub fn clear_run(&mut self) {
        self.run = None;
    }

    /// Restore into a run: the profile always applies; the snapshot applies
    /// when present (otherwise the fresh run keeps its defaults). Ephemeral
    /// per-level state resets either way.
    pub fn apply_to(&self, run: &mut RunState) {
        run.profile = self.profile.clone();
        if let Some(snapshot) = &self.run {
            run.set_ship_type(snapshot.ship_type);
            run.laser_level = snapshot.laser_level;
            run.loadout = snapshot.loadout.clone();
            run.score = snapshot.score;
            run.current_level = snapshot.current_level;
            run.run_seed = snapshot.run_seed;
            run.components = snapshot.components;
            run.health = snapshot.health;
            run.shield = snapshot.shield.clone();
            run.ship_color = snapshot.ship_color;
            run.lives = snapshot.lives;
            run.lives_purchased = snapshot.lives_purchased;
            run.shield_charges = snapshot.shield_charges;
            run.valkyrie_bars_bought = snapshot.valkyrie_bars_bought;
            run.valkyrie_refill_level = snapshot.valkyrie_refill_level;
        }
        // A profile-only load hands the owned Surge item over freshly
        // stocked (a snapshot's rack, restored above, wins when present).
        if self.run.is_none() && run.profile.unlocks.contains(crate::unlocks::Unlock::ShieldBurst) {
            run.shield_charges = crate::run_state::SHIELD_BURST_STARTING_CHARGES;
        }
        // Reset ephemeral state
        run.kills.reset();
        run.rooms_cleared.clear();
        run.rooms_visited.clear();
        run.current_room = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currency::CurrencyKind;

    /// The n-th declared enemy, by roster position — these tests need SOME
    /// distinct enemies, never a particular one. The grammar declares which
    /// exist; retuning the roster can't touch this.
    fn nth_enemy(n: usize) -> crate::roster::EnemyKey {
        crate::roster::roster()
            .enemy_keys()
            .nth(n)
            .expect("the grammar declares enough enemies")
    }
    
    fn seasoned_run() -> RunState {
        let mut run = RunState::new(Seed::new(42));
        run.laser_level = LaserLevel::Green;
        run.collect_cache(CurrencyKind::Components, 5_000);
        run.collect_cache(CurrencyKind::Organics, 120);
        run.current_level = 4;
        run.lives = 3;
        run.lives_purchased = 2;
        run.mark_enemy_seen(nth_enemy(0));
        run
    }

    #[test]
    fn from_run_state_snapshots_profile_and_run() {
        let save = SaveGame::from_run_state(&seasoned_run());
        assert!(save.has_run(), "a full snapshot is continuable");
        assert_eq!(save.profile.organics.balance, 120);
        assert!(save.profile.seen_enemies.contains(nth_enemy(0)));
        let run = save.run.as_ref().expect("snapshot present");
        assert_eq!(run.laser_level, LaserLevel::Green);
        assert_eq!(run.components.balance, 5_000);
        assert_eq!(run.current_level, 4);
        assert_eq!(run.lives, 3);
        assert_eq!(run.lives_purchased, 2);
    }

    #[test]
    fn the_valkyrie_charge_upgrades_survive_the_snapshot() {
        // Blue upgrades bought this run must ride save-and-exit like every
        // other blue (playtest 2026-07-06 charge redesign).
        let mut run = seasoned_run();
        run.valkyrie_bars_bought = 2;
        run.valkyrie_refill_level = 3;
        let save = SaveGame::from_run_state(&run);
        let restored = SaveGame::from_json(&save.to_json()).expect("round-trips");
        let mut fresh = RunState::new(Seed::new(1));
        restored.apply_to(&mut fresh);
        assert_eq!(fresh.valkyrie_bars_bought, 2, "bought bars ride the snapshot");
        assert_eq!(fresh.valkyrie_refill_level, 3, "refill levels too");
    }

    #[test]
    fn profile_only_is_not_continuable() {
        let save = SaveGame::profile_only(&seasoned_run());
        assert!(!save.has_run(), "no snapshot, no Continue");
        assert_eq!(save.profile.organics.balance, 120, "the profile still carries the organics");
    }

    #[test]
    fn clear_run_keeps_the_profile() {
        let mut save = SaveGame::from_run_state(&seasoned_run());
        save.clear_run();
        assert!(!save.has_run(), "run-over drops the snapshot");
        assert_eq!(save.profile.organics.balance, 120, "organics survive run-over");
        assert!(save.profile.seen_enemies.contains(nth_enemy(0)),
            "the bestiary survives run-over");
    }

    #[test]
    fn update_profile_refreshes_permanents_without_touching_the_run() {
        let mut run = seasoned_run();
        let mut save = SaveGame::from_run_state(&run);

        // Mid-level: more green banked, more salvage carried.
        run.collect_cache(CurrencyKind::Organics, 80);
        run.collect_cache(CurrencyKind::Components, 9_999);
        save.update_profile(&run);

        assert_eq!(save.profile.organics.balance, 200,
            "green is permanent the moment it is picked up");
        let snapshot = save.run.as_ref().expect("snapshot untouched");
        assert_eq!(snapshot.components.balance, 5_000,
            "the run snapshot stays frozen at its level start");
    }

    #[test]
    fn apply_with_run_restores_everything() {
        let save = SaveGame::from_run_state(&seasoned_run());
        let mut fresh = RunState::new(Seed::new(99));
        save.apply_to(&mut fresh);
        assert_eq!(fresh.laser_level, LaserLevel::Green);
        assert_eq!(fresh.components.balance, 5_000);
        assert_eq!(fresh.profile.organics.balance, 120);
        assert_eq!(fresh.current_level, 4);
        assert_eq!(fresh.run_seed, Seed::new(42), "the seed comes back — same layouts");
        assert_eq!(fresh.lives, 3);
        assert_eq!(fresh.lives_purchased, 2);
        assert!(fresh.profile.seen_enemies.contains(nth_enemy(0)));
    }

    #[test]
    fn apply_without_run_grants_only_the_profile() {
        let mut save = SaveGame::from_run_state(&seasoned_run());
        save.clear_run();
        let mut fresh = RunState::new(Seed::new(99));
        save.apply_to(&mut fresh);
        assert_eq!(fresh.profile.organics.balance, 120, "profile organics apply");
        assert!(fresh.profile.seen_enemies.contains(nth_enemy(0)), "profile bestiary applies");
        assert_eq!(fresh.components.balance, 0, "no run to restore");
        assert_eq!(fresh.current_level, 1, "a fresh run starts at level 1");
        assert_eq!(fresh.run_seed, Seed::new(99), "the fresh run keeps its own seed");
    }

    #[test]
    fn apply_resets_ephemeral_state() {
        let save = SaveGame::from_run_state(&seasoned_run());
        let mut fresh = RunState::new(Seed::new(99));
        fresh.record_kill(nth_enemy(0));
        fresh.clear_room(3);
        save.apply_to(&mut fresh);
        assert_eq!(fresh.kills.total_kills(), 0, "kill tally is per-level");
        assert!(fresh.rooms_cleared.is_empty(), "cleared rooms are per-level");
        assert_eq!(fresh.current_room, 0);
    }

    #[test]
    fn the_chosen_hull_rides_the_snapshot() {
        let mut run = seasoned_run();
        run.profile.unlocks.grant(crate::unlocks::Unlock::Ship(ShipType::Hive));
        run.set_ship_type(ShipType::Hive);
        let save = SaveGame::from_run_state(&run);
        let mut fresh = RunState::new(Seed::new(99));
        save.apply_to(&mut fresh);
        assert_eq!(fresh.ship_type, ShipType::Hive, "the hull selection restores");
    }

    #[test]
    fn unlocks_ride_the_profile() {
        use crate::unlocks::Unlock;
        let mut run = seasoned_run();
        run.profile.unlocks.grant(Unlock::Radar);
        let mut save = SaveGame::from_run_state(&run);
        assert!(save.profile.unlocks.contains(Unlock::Radar), "the snapshot carries unlocks");

        run.profile.unlocks.grant(Unlock::FogMap);
        save.update_profile(&run);
        assert!(save.profile.unlocks.contains(Unlock::FogMap),
            "a profile update carries a freshly bought unlock");

        save.clear_run();
        let mut fresh = RunState::new(Seed::new(99));
        save.apply_to(&mut fresh);
        assert!(fresh.profile.unlocks.contains(Unlock::Radar) && fresh.profile.unlocks.contains(Unlock::FogMap),
            "unlocks restore from the profile even with no continuable run");
    }

    #[test]
    fn json_round_trips_the_split_shape() {
        let save = SaveGame::from_run_state(&seasoned_run());
        let back = SaveGame::from_json(&save.to_json()).expect("round trip parses");
        assert_eq!(back, save);

        let mut profile_only = save.clone();
        profile_only.clear_run();
        let back = SaveGame::from_json(&profile_only.to_json()).expect("round trip parses");
        assert!(!back.has_run());
        assert_eq!(back.profile, profile_only.profile);
    }

    #[test]
    fn unparseable_saves_are_a_fresh_start() {
        // There is no legacy migration by design (owner decision, 2026-07-03):
        // anything that is not the current shape — old layouts included —
        // loads as None, i.e. a fresh install.
        assert_eq!(SaveGame::from_json("not json"), None);
        assert_eq!(SaveGame::from_json("{}"), None);
        // A pre-split flat layout is just another unparseable shape.
        assert_eq!(SaveGame::from_json(r#"{"laser_level":"Red","score":0}"#), None);
    }
}
