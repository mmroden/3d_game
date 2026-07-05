//! The bestiary: a catalog of the hazards the player has encountered. The two
//! currency pickups always lead (they teach the run-vs-permanent economy),
//! followed by every enemy seen so far, in roster declaration order. Enemy
//! identity is the grammar's append-only crossing id — an id whose def has
//! been REMOVED from the roster simply stops appearing (old saves tolerate
//! retired enemies; nothing crashes).

use crate::roster::{roster, EnemyId};

/// The set of enemy crossing ids the player has encountered. Permanent
/// across runs: marked the first time one spawns, catalogued forever.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SeenEnemies {
    seen: std::collections::HashSet<u16>,
}

impl SeenEnemies {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a crossing id as encountered. Returns `true` on first sighting
    /// (so the caller can flag "new entry" / trigger a save).
    pub fn mark(&mut self, crossing_id: u16) -> bool {
        self.seen.insert(crossing_id)
    }

    pub fn contains(&self, crossing_id: u16) -> bool {
        self.seen.contains(&crossing_id)
    }

    pub fn count(&self) -> usize {
        self.seen.len()
    }

    /// Seen enemies in roster declaration order, independent of encounter
    /// order. Ids without a def (removed from the grammar) are skipped.
    pub fn in_roster_order(&self) -> Vec<EnemyId> {
        let r = roster();
        r.enemy_ids()
            .filter(|id| self.seen.contains(&r.enemy(*id).crossing_id))
            .collect()
    }
}

/// What a briefing entry spins in the room — a currency pickup or an enemy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BestiaryKind {
    /// Green cache: the permanent currency (organics).
    OrganicCache,
    /// Blue cache: upgrades for the current run only (components).
    ComponentCache,
    /// A catalogued enemy.
    Enemy(EnemyId),
}

/// A fully-resolved catalog entry: what to display, its title, and its lore.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BestiaryEntry {
    pub kind: BestiaryKind,
    pub title: &'static str,
    pub blurb: &'static str,
}

const ORGANIC_TITLE: &str = "Organic Cache";
const ORGANIC_BLURB: &str = "A green-glowing canister of something the brass want very badly. \
Bank it and it stays with you, run after run, buying the upgrades that ride home in your hull. \
Nobody briefs you on what the green stuff actually is — only that people are paying real money for it.";

const COMPONENT_TITLE: &str = "Component Cache";
const COMPONENT_BLURB: &str = "A blue salvage cache of spare parts, shaken loose when a machine \
dies — fly it down before you move on, because nothing is credited from a distance. Useful now, \
worthless later: its components buy upgrades between levels, this run only, and burn up with you \
if you don't come home.";

/// Build the ordered briefing entries: the two pickups first (always — they
/// teach the economy), then each seen enemy in roster order, titles and
/// lore straight from the grammar.
pub fn entries(seen: &SeenEnemies) -> Vec<BestiaryEntry> {
    let mut out = vec![
        BestiaryEntry {
            kind: BestiaryKind::OrganicCache,
            title: ORGANIC_TITLE,
            blurb: ORGANIC_BLURB,
        },
        BestiaryEntry {
            kind: BestiaryKind::ComponentCache,
            title: COMPONENT_TITLE,
            blurb: COMPONENT_BLURB,
        },
    ];
    let r = roster();
    for id in seen.in_roster_order() {
        let def = r.enemy(id);
        out.push(BestiaryEntry {
            kind: BestiaryKind::Enemy(id),
            title: def.name.as_str(),
            blurb: def.blurb.as_str(),
        });
    }
    out
}

/// Move a catalog cursor by `delta` (-1 prev, +1 next), clamped to
/// `[0, total-1]` — no wrap. Drives the bestiary's left-stick browsing. A
/// `total` of 0 leaves the cursor where it is.
pub fn paged_index(current: usize, delta: i32, total: usize) -> usize {
    if total == 0 {
        return current;
    }
    let last = (total - 1) as i64;
    (current as i64 + delta as i64).clamp(0, last) as usize
}

/// The call-to-action under the briefing panel. The Ⓧ marks the begin button
/// (gamepad X / keyboard Enter). The next/prev hint only appears when there's
/// more than one entry to move between; its up/down arrows match the menu_up /
/// menu_down paging input (the same navigation every other menu uses).
pub fn briefing_hint(total: usize) -> &'static str {
    if total > 1 {
        "\u{25B2} next \u{25BC}     \u{2022}     \u{24CD} Begin mission"
    } else {
        "\u{24CD} Begin mission"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cid(key: &str) -> u16 {
        let r = roster();
        r.enemy(r.enemy_by_key(key).expect(key)).crossing_id
    }

    #[test]
    fn newly_seen_enemy_reports_first_sighting() {
        let mut seen = SeenEnemies::new();
        assert!(seen.mark(cid("gun_drone")), "first sighting is new");
        assert!(!seen.mark(cid("gun_drone")), "second sighting is not new");
        assert!(seen.contains(cid("gun_drone")));
        assert_eq!(seen.count(), 1);
    }

    #[test]
    fn seen_enemies_iterate_in_roster_order_not_encounter_order() {
        let mut seen = SeenEnemies::new();
        // Encountered out of declaration order.
        seen.mark(cid("quad_shell"));
        seen.mark(cid("gun_drone"));
        seen.mark(cid("bomber"));
        let keys: Vec<&str> = seen
            .in_roster_order()
            .iter()
            .map(|id| roster().enemy(*id).key.as_str())
            .collect();
        assert_eq!(keys, vec!["gun_drone", "bomber", "quad_shell"]);
    }

    #[test]
    fn a_removed_enemys_save_flag_is_tolerated() {
        // Open-set identity (owner 2026-07-05): an old save may reference an
        // enemy the grammar no longer declares — it silently stops appearing.
        let mut seen = SeenEnemies::new();
        seen.mark(9999);
        seen.mark(cid("bomber"));
        let listed = seen.in_roster_order();
        assert_eq!(listed.len(), 1, "the ghost id is skipped, nothing crashes");
        assert_eq!(entries(&seen).len(), 3, "pickups + the one real enemy");
    }

    #[test]
    fn empty_bestiary_shows_only_the_two_pickups() {
        let entries = entries(&SeenEnemies::new());
        assert_eq!(entries.len(), 2, "level 1, nothing seen → just the pickups");
        assert_eq!(entries[0].kind, BestiaryKind::OrganicCache);
        assert_eq!(entries[1].kind, BestiaryKind::ComponentCache);
    }

    #[test]
    fn pickups_always_lead_then_seen_enemies_in_order() {
        let mut seen = SeenEnemies::new();
        seen.mark(cid("quad_shell"));
        seen.mark(cid("gun_drone"));
        let entries = entries(&seen);
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].kind, BestiaryKind::OrganicCache);
        assert_eq!(entries[1].kind, BestiaryKind::ComponentCache);
        assert_eq!(entries[2].title, "Gun Drone");
        assert_eq!(entries[3].title, "Quad Shell");
    }

    #[test]
    fn briefing_hint_shows_next_only_with_more_than_one_entry() {
        // Single subject: nothing to page to, just the begin prompt.
        assert!(briefing_hint(1).contains("Begin mission"));
        assert!(!briefing_hint(1).contains("next"), "one entry can't be paged");
        // Multiple subjects: offer next/prev as well as begin.
        let multi = briefing_hint(3);
        assert!(multi.contains("next"), "multiple entries show the next/prev hint");
        assert!(multi.contains("Begin mission"));
        // Paging is bound to menu up/down, so the glyphs must read up/down —
        // not the left/right arrows that imply a horizontal control.
        assert!(multi.contains('\u{25B2}'), "up arrow ▲ matches the menu_up paging input");
        assert!(multi.contains('\u{25BC}'), "down arrow ▼ matches the menu_down paging input");
        assert!(!multi.contains('\u{25C0}'), "no left arrow ◀ — paging isn't horizontal");
        assert!(!multi.contains('\u{25B6}'), "no right arrow ▶ — paging isn't horizontal");
    }

    #[test]
    fn paging_steps_and_clamps_without_wrapping() {
        assert_eq!(paged_index(0, 1, 3), 1, "step forward");
        assert_eq!(paged_index(2, -1, 3), 1, "step back");
        assert_eq!(paged_index(0, -1, 3), 0, "clamp at the start — no wrap to the end");
        assert_eq!(paged_index(2, 1, 3), 2, "clamp at the end — no wrap to the start");
        assert_eq!(paged_index(0, 1, 0), 0, "empty catalog leaves the cursor put");
        assert_eq!(paged_index(0, 1, 1), 0, "a single entry can't be paged off");
    }

    #[test]
    fn every_entry_has_a_title_and_a_blurb() {
        let mut seen = SeenEnemies::new();
        for id in roster().enemy_ids() {
            seen.mark(roster().enemy(id).crossing_id);
        }
        for entry in entries(&seen) {
            assert!(!entry.title.is_empty(), "{:?} title empty", entry.kind);
            assert!(!entry.blurb.is_empty(), "{:?} blurb empty", entry.kind);
        }
    }
}
