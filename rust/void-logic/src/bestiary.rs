//! The bestiary: a catalog of the hazards the player has encountered. The two
//! currency pickups always lead (they teach the run-vs-permanent economy),
//! followed by every enemy type seen so far, in roster order. This drives the
//! between-level briefing screen — on the first level, before any enemy is met,
//! it shows only the green barrel and the blue cache.

use crate::enemy_type::EnemyType;

/// The set of enemy types the player has encountered. Permanent across runs:
/// an enemy is marked the first time it spawns, and stays catalogued forever.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SeenEnemies {
    seen: std::collections::HashSet<EnemyType>,
}

impl SeenEnemies {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark `enemy` as encountered. Returns `true` if this is the first sighting
    /// (so the caller can flag "new entry" / trigger a save).
    pub fn mark(&mut self, enemy: EnemyType) -> bool {
        self.seen.insert(enemy)
    }

    pub fn contains(&self, enemy: EnemyType) -> bool {
        self.seen.contains(&enemy)
    }

    pub fn count(&self) -> usize {
        self.seen.len()
    }

    /// Seen enemies in stable roster order (`EnemyType::ALL`), independent of the
    /// order they were actually encountered, so the catalog reads consistently.
    pub fn in_roster_order(&self) -> Vec<EnemyType> {
        EnemyType::ALL
            .iter()
            .copied()
            .filter(|e| self.seen.contains(e))
            .collect()
    }
}

/// What a briefing entry spins in the room — a currency pickup or an enemy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BestiaryKind {
    /// Green barrel: permanent, run-to-run ship upgrades (organics).
    OrganicBarrel,
    /// Blue cache: upgrades for the current run only (components).
    ComponentCache,
    /// A catalogued enemy.
    Enemy(EnemyType),
}

/// A fully-resolved catalog entry: what to display, its title, and its lore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BestiaryEntry {
    pub kind: BestiaryKind,
    pub title: &'static str,
    pub blurb: &'static str,
}

const ORGANIC_TITLE: &str = "Organic Barrel";
const ORGANIC_BLURB: &str = "A green-glowing canister of something the brass want very badly. \
Bank it and it stays with you, run after run, buying the upgrades that ride home in your hull. \
Nobody briefs you on what the green stuff actually is — only that people are paying real money for it.";

const COMPONENT_TITLE: &str = "Component Cache";
const COMPONENT_BLURB: &str = "A blue salvage cache of spare parts. Useful now, worthless later: \
its components buy upgrades for this run only, and burn up with you if you don't come home.";

/// Build the ordered briefing entries: the two pickups first (always — they
/// teach the economy), then each seen enemy in roster order.
pub fn entries(seen: &SeenEnemies) -> Vec<BestiaryEntry> {
    let mut out = vec![
        BestiaryEntry {
            kind: BestiaryKind::OrganicBarrel,
            title: ORGANIC_TITLE,
            blurb: ORGANIC_BLURB,
        },
        BestiaryEntry {
            kind: BestiaryKind::ComponentCache,
            title: COMPONENT_TITLE,
            blurb: COMPONENT_BLURB,
        },
    ];
    for enemy in seen.in_roster_order() {
        out.push(BestiaryEntry {
            kind: BestiaryKind::Enemy(enemy),
            title: enemy.display_name(),
            blurb: enemy_blurb(enemy),
        });
    }
    out
}

/// Lore for a catalogued enemy. Every variant must return non-empty text.
pub fn enemy_blurb(enemy: EnemyType) -> &'static str {
    match enemy {
        EnemyType::GunDrone => "A lone picket gun. It hangs back at standoff range and strafes, \
peppering you with bolts while it keeps the gap open.",
        EnemyType::QuadOrb => "A four-legged swarm unit. It doesn't shoot — it closes and clamps on, \
dragging your thrust down so its friends get clean shots.",
        EnemyType::Bomber => "A walking charge. It fuses up the moment it's in range and detonates, \
trading itself for a hole in your shields. Kill it early or get clear.",
        EnemyType::EyeDrone => "An optical sentry. It takes a few shots, then breaks off to round up \
other machines and herd them onto you — death by overwhelming odds.",
        EnemyType::QuadShell => "A shielded tank. Its plating soaks damage before its hull ever feels it; \
patient fire, or a flank while it's busy, is the only way through.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newly_seen_enemy_reports_first_sighting() {
        let mut seen = SeenEnemies::new();
        assert!(seen.mark(EnemyType::GunDrone), "first sighting is new");
        assert!(!seen.mark(EnemyType::GunDrone), "second sighting is not new");
        assert!(seen.contains(EnemyType::GunDrone));
        assert_eq!(seen.count(), 1);
    }

    #[test]
    fn seen_enemies_iterate_in_roster_order_not_encounter_order() {
        let mut seen = SeenEnemies::new();
        // Encountered out of roster order.
        seen.mark(EnemyType::QuadShell);
        seen.mark(EnemyType::GunDrone);
        seen.mark(EnemyType::Bomber);
        assert_eq!(
            seen.in_roster_order(),
            vec![EnemyType::GunDrone, EnemyType::Bomber, EnemyType::QuadShell]
        );
    }

    #[test]
    fn empty_bestiary_shows_only_the_two_pickups() {
        let entries = entries(&SeenEnemies::new());
        assert_eq!(entries.len(), 2, "level 1, nothing seen → just the pickups");
        assert_eq!(entries[0].kind, BestiaryKind::OrganicBarrel);
        assert_eq!(entries[1].kind, BestiaryKind::ComponentCache);
    }

    #[test]
    fn pickups_always_lead_then_seen_enemies_in_order() {
        let mut seen = SeenEnemies::new();
        seen.mark(EnemyType::QuadShell);
        seen.mark(EnemyType::GunDrone);
        let entries = entries(&seen);
        let kinds: Vec<BestiaryKind> = entries.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                BestiaryKind::OrganicBarrel,
                BestiaryKind::ComponentCache,
                BestiaryKind::Enemy(EnemyType::GunDrone),
                BestiaryKind::Enemy(EnemyType::QuadShell),
            ]
        );
    }

    #[test]
    fn every_entry_has_a_title_and_a_blurb() {
        let mut seen = SeenEnemies::new();
        for e in EnemyType::ALL {
            seen.mark(*e);
        }
        for entry in entries(&seen) {
            assert!(!entry.title.is_empty(), "{:?} title empty", entry.kind);
            assert!(!entry.blurb.is_empty(), "{:?} blurb empty", entry.kind);
        }
    }
}
