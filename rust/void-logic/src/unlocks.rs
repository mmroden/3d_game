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
}

impl Unlock {
    pub const ALL: &[Unlock] = &[
        Unlock::Radar,
        Unlock::FogMap,
        Unlock::Valkyrie,
        Unlock::Ship(ShipType::Talon),
        Unlock::Ship(ShipType::Hive),
        Unlock::Ship(ShipType::Reaver),
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
        }
    }

    /// One line of what owning this actually gets you — shop-row detail.
    pub fn blurb(&self) -> String {
        match self {
            Self::Radar => "HUD arrows point at off-screen enemies".to_string(),
            Self::FogMap => "Charts visited rooms and unexplored corridors".to_string(),
            Self::Valkyrie => "Heavy second cannon on the fire trigger".to_string(),
            Self::Ship(ship) => format!("New hull — {}", ship.spec().weapon.display_name()),
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
        }
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
    fn ship_ownership_is_starter_or_bought() {
        let mut unlocks = PermanentUnlocks::new();
        assert!(unlocks.owns_ship(ShipType::Vanguard), "the starter is always owned");
        assert!(!unlocks.owns_ship(ShipType::Talon), "unbought hulls are locked");
        unlocks.grant(Unlock::Ship(ShipType::Talon));
        assert!(unlocks.owns_ship(ShipType::Talon), "buying the hull unlocks it");
        assert!(!unlocks.owns_ship(ShipType::Hive), "each hull is bought separately");
    }
}
