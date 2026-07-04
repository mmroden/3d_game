//! Permanent unlocks — the green (organics) purchases that survive
//! everything: run-over, quits, new games. Bought once at the shop, owned
//! forever, persisted in the [`Profile`](crate::save_game::Profile).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::ship_type::ShipType;

/// Everything organics can permanently unlock. Order in `ALL` is the green
/// progression ladder — a tutorial spine where each capability opens the
/// next (see [`PermanentUnlocks::available`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Unlock {
    /// HUD arrows pointing at off-screen enemies.
    Radar,
    /// The fog-of-war room map: visited rooms and unexplored corridors.
    FogMap,
    /// The Vanguard's heavy center cannon — the keystone purchase that
    /// opens the fleet.
    Valkyrie,
    /// A purchasable hull (the starter is always owned and never listed).
    Ship(ShipType),
    /// Map upgrade: unexplored connectors draw on the recon map.
    RouteScanner,
    /// Map upgrade: nearby enemies draw on the recon map.
    ThreatTracker,
    /// One-press shield surge: comes with three charges, refills are blue
    /// (`ShopItemId::ShieldCharge`).
    ShieldBurst,
}

impl Unlock {
    // The map upgrades append AFTER the hulls: ids are positions in ALL,
    // and the GDScript crossings pin them — append-only, never reorder.
    pub const ALL: &[Unlock] = &[
        Unlock::Radar,
        Unlock::FogMap,
        Unlock::Valkyrie,
        Unlock::Ship(ShipType::Talon),
        Unlock::Ship(ShipType::Hive),
        Unlock::Ship(ShipType::Reaver),
        Unlock::RouteScanner,
        Unlock::ThreatTracker,
        Unlock::ShieldBurst,
    ];

    /// Stable id for GDScript crossings (position in `ALL`).
    pub fn id(&self) -> i32 {
        Self::ALL.iter().position(|u| u == self)
            .expect("Unlock::ALL must contain every variant") as i32
    }

    pub fn from_id(id: i32) -> Option<Unlock> {
        Self::ALL.get(id as usize).copied()
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Radar => "Enemy Radar",
            Self::FogMap => "Recon Map",
            Self::Valkyrie => "Valkyrie Cannon",
            Self::Ship(ship) => ship.spec().display_name,
            Self::RouteScanner => "Route Scanner",
            Self::ThreatTracker => "Threat Tracker",
            Self::ShieldBurst => "Shield Surge",
        }
    }

    /// How the player USES what they bought — shown before every green
    /// purchase so nobody buys a mystery (owner's ask, 2026-07-04).
    pub fn trigger_hint(&self) -> &'static str {
        match self {
            Self::Radar | Self::FogMap | Self::RouteScanner | Self::ThreatTracker => {
                "Passive — always on once bought"
            }
            Self::Valkyrie => "Fires with your lasers — hold FIRE",
            Self::Ship(_) => "Choose it at the loadout screen",
            Self::ShieldBurst => "Press L1 (pad) / C (keys) — one charge per press",
        }
    }

    /// One line of what owning this actually gets you — shop-row detail.
    pub fn blurb(&self) -> String {
        match self {
            Self::Radar => "HUD arrows point at off-screen enemies".to_string(),
            Self::FogMap => "Charts visited rooms and unexplored corridors".to_string(),
            Self::Valkyrie => "Heavy second cannon on the fire trigger".to_string(),
            Self::Ship(ship) => format!("New hull — {}", ship.spec().weapon.display_name()),
            Self::RouteScanner => "Unexplored routes glow on the recon map".to_string(),
            Self::ThreatTracker => "Nearby enemies mark on the recon map".to_string(),
            Self::ShieldBurst => "Instant +50 shields, three charges; refills are components".to_string(),
        }
    }

    /// Price in organics. Green income is slow by design (~50 per cache,
    /// a few caches per level), so these are multi-level commitments —
    /// the hulls especially are long-arc purchases.
    pub fn organic_cost(&self) -> u32 {
        match self {
            Self::Radar => 300,
            Self::FogMap => 500,
            Self::Valkyrie => 800,
            Self::Ship(ship) => ship.spec().organic_cost.unwrap_or(0),
            Self::RouteScanner => 600,
            Self::ThreatTracker => 1_200,
            Self::ShieldBurst => 1_000,
        }
    }
}

/// The set of unlocks owned. Ordered (BTreeSet) so serialization — and
/// therefore the save file — is deterministic.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermanentUnlocks {
    owned: BTreeSet<Unlock>,
}

impl PermanentUnlocks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn contains(&self, unlock: Unlock) -> bool {
        self.owned.contains(&unlock)
    }

    pub fn grant(&mut self, unlock: Unlock) {
        self.owned.insert(unlock);
    }

    /// Whether a hull can be flown: the starter always, others once bought.
    pub fn owns_ship(&self, ship: ShipType) -> bool {
        ship.spec().organic_cost.is_none() || self.contains(Unlock::Ship(ship))
    }

    /// Whether `unlock` may be OFFERED, given what's owned — the green
    /// ladder is a tutorial spine: Radar → Recon Map → Valkyrie → the
    /// fleet. Early on the shop's green section shows exactly one next
    /// capability; at endgame everything unowned is available.
    pub fn available(&self, unlock: Unlock) -> bool {
        match unlock {
            Unlock::Radar => true,
            Unlock::FogMap => self.contains(Unlock::Radar),
            Unlock::Valkyrie => self.contains(Unlock::FogMap),
            Unlock::Ship(_) => self.contains(Unlock::Valkyrie),
            // Map upgrades BRANCH off the map — they never gate the spine.
            Unlock::RouteScanner | Unlock::ThreatTracker => self.contains(Unlock::FogMap),
            // The surge branches off the radar (the defensive intro).
            Unlock::ShieldBurst => self.contains(Unlock::Radar),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_owned_until_granted() {
        let mut unlocks = PermanentUnlocks::new();
        for unlock in Unlock::ALL {
            assert!(!unlocks.contains(*unlock), "{unlock:?} must start locked");
        }
        unlocks.grant(Unlock::Radar);
        assert!(unlocks.contains(Unlock::Radar), "granted is owned");
        assert!(!unlocks.contains(Unlock::FogMap), "granting one unlocks only that one");
    }

    #[test]
    fn granting_twice_is_idempotent() {
        let mut unlocks = PermanentUnlocks::new();
        unlocks.grant(Unlock::FogMap);
        unlocks.grant(Unlock::FogMap);
        assert!(unlocks.contains(Unlock::FogMap));
    }

    #[test]
    fn serde_round_trips_the_owned_set() {
        let mut unlocks = PermanentUnlocks::new();
        unlocks.grant(Unlock::Radar);
        let json = serde_json::to_string(&unlocks).unwrap();
        let back: PermanentUnlocks = serde_json::from_str(&json).unwrap();
        assert_eq!(back, unlocks);
        assert!(back.contains(Unlock::Radar));
        assert!(!back.contains(Unlock::FogMap));
    }

    #[test]
    fn ids_round_trip() {
        for unlock in Unlock::ALL {
            assert_eq!(Unlock::from_id(unlock.id()), Some(*unlock));
        }
        assert_eq!(Unlock::from_id(-1), None);
        assert_eq!(Unlock::from_id(99), None);
    }

    #[test]
    fn every_unlock_is_named_and_priced() {
        for unlock in Unlock::ALL {
            assert!(!unlock.display_name().is_empty(), "{unlock:?} needs a name");
            assert!(unlock.organic_cost() > 0, "{unlock:?} must cost organics");
            assert!(!unlock.blurb().is_empty(),
                "{unlock:?} needs a blurb — the shop row must say what it does");
            assert!(!unlock.trigger_hint().is_empty(),
                "{unlock:?} needs a trigger hint — nobody buys a mystery");
        }
    }

    #[test]
    fn the_surge_branches_off_the_radar() {
        let mut unlocks = PermanentUnlocks::new();
        assert!(!unlocks.available(Unlock::ShieldBurst), "no radar, no surge");
        unlocks.grant(Unlock::Radar);
        assert!(unlocks.available(Unlock::ShieldBurst));
        // A branch, never a gate: the map doesn't wait for it.
        assert!(unlocks.available(Unlock::FogMap));
    }

    #[test]
    fn every_purchasable_hull_is_an_unlock_and_the_starter_is_not() {
        for ship in ShipType::ALL {
            let listed = Unlock::ALL.contains(&Unlock::Ship(*ship));
            match ship.spec().organic_cost {
                Some(_) => assert!(listed, "{ship:?} is purchasable, it must be an unlock"),
                None => assert!(!listed, "the starter is always owned, never listed"),
            }
        }
    }

    #[test]
    fn the_green_ladder_opens_one_capability_at_a_time() {
        let mut unlocks = PermanentUnlocks::new();
        assert!(unlocks.available(Unlock::Radar), "the radar is the first rung");
        assert!(!unlocks.available(Unlock::FogMap), "the map needs the radar");
        assert!(!unlocks.available(Unlock::Valkyrie), "the cannon needs the map");
        assert!(!unlocks.available(Unlock::Ship(ShipType::Talon)),
            "no hull is reachable before the Valkyrie");

        unlocks.grant(Unlock::Radar);
        assert!(unlocks.available(Unlock::FogMap), "owning the radar opens the map");
        assert!(!unlocks.available(Unlock::Valkyrie), "still one rung at a time");

        unlocks.grant(Unlock::FogMap);
        assert!(unlocks.available(Unlock::Valkyrie));
        assert!(!unlocks.available(Unlock::Ship(ShipType::Hive)));

        unlocks.grant(Unlock::Valkyrie);
        for ship in [ShipType::Talon, ShipType::Hive, ShipType::Reaver] {
            assert!(unlocks.available(Unlock::Ship(ship)),
                "the Valkyrie opens the whole fleet ({ship:?})");
        }
    }

    #[test]
    fn map_upgrades_branch_off_the_map_without_gating_the_spine() {
        let mut unlocks = PermanentUnlocks::new();
        assert!(!unlocks.available(Unlock::RouteScanner), "no map, no routes");
        assert!(!unlocks.available(Unlock::ThreatTracker), "no map, no tracker");

        unlocks.grant(Unlock::Radar);
        unlocks.grant(Unlock::FogMap);
        assert!(unlocks.available(Unlock::RouteScanner), "the map opens its upgrades");
        assert!(unlocks.available(Unlock::ThreatTracker));
        // The spine doesn't wait for the branches.
        assert!(unlocks.available(Unlock::Valkyrie),
            "the Valkyrie needs only the map, never the map's extras");
    }

    #[test]
    fn ship_ownership_is_starter_or_bought() {
        let mut unlocks = PermanentUnlocks::new();
        assert!(unlocks.owns_ship(ShipType::Vanguard), "the starter is always owned");
        assert!(!unlocks.owns_ship(ShipType::Talon), "unbought hulls are locked");
        unlocks.grant(Unlock::Ship(ShipType::Talon));
        assert!(unlocks.owns_ship(ShipType::Talon), "buying the hull unlocks it");
        assert!(!unlocks.owns_ship(ShipType::Hive), "each hull is bought separately");
    }
}
