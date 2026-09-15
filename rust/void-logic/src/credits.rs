//! The credits, rendered from `catalog/attributions.toml` — the one place
//! every third-party source's license and credit line live and, since
//! 2026-09-09, the team. Embedded at build time like the asset catalog;
//! `scripts/credits.py` renders `docs/CREDITS.md` from the same file, so
//! the crawl in the menu and the page in the repository are two views of
//! one truth. A source we cannot credit refuses to load: we cannot ship
//! what we cannot credit.
//!
//! The crawl is pairs (owner 2026-09-09: "each line should be
//! contribution then person on the next line in a rolling scroll"): the
//! team's roles and names, then every shipped source's `contribution`
//! and author, in the catalog's own order — a curated roll, not an index.

use serde::Deserialize;

/// The catalog's embedded source (catalog/ on disk).
const ATTRIBUTIONS_TOML: &str = include_str!("../../../catalog/attributions.toml");

/// One line of the team roll.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Team {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub role: String,
}

/// One third-party source: what it is, who made it, the license it came
/// under, the credit line it asks for, and `contribution` — the one short
/// line the crawl shows above the author. `ships = false` marks a pack in
/// the repository that nothing installs from — credited on the
/// repository page, off the crawl. Every field defaults so the load, not
/// the parser, can name the source a hole belongs to.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Source {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub credit: String,
    #[serde(default)]
    pub used_for: String,
    #[serde(default)]
    pub contribution: String,
    #[serde(default = "ships_by_default")]
    pub ships: bool,
}

fn ships_by_default() -> bool {
    true
}

/// The catalog as loaded: the team and every source.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct Credits {
    #[serde(default)]
    pub team: Vec<Team>,
    #[serde(default)]
    pub source: Vec<Source>,
}

/// One pair of the crawl: what was contributed, then by whom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollEntry {
    pub contribution: String,
    pub person: String,
}

impl Credits {
    /// Parse the catalog text. A source missing any field a credit needs
    /// (name, author, license, the credit line, what it was used for — and,
    /// for one that ships, its crawl line) or a team member without a name
    /// or role is a hole the load refuses, named.
    pub fn load(text: &str) -> Result<Credits, String> {
        let credits: Credits = toml::from_str(text).map_err(|e| e.to_string())?;
        for member in &credits.team {
            if member.name.trim().is_empty() || member.role.trim().is_empty() {
                let who = if member.name.trim().is_empty() { &member.role } else { &member.name };
                return Err(format!(
                    "team entry {who:?} lacks a name or a role — the crawl names everyone and what they did"
                ));
            }
        }
        for source in &credits.source {
            let mut required = vec![
                ("name", &source.name),
                ("author", &source.author),
                ("license", &source.license),
                ("credit", &source.credit),
                ("used_for", &source.used_for),
            ];
            if source.ships {
                required.push(("contribution", &source.contribution));
            }
            let missing: Vec<&str> = required
                .into_iter()
                .filter(|(_, value)| value.trim().is_empty())
                .map(|(field, _)| field)
                .collect();
            if !missing.is_empty() {
                let key = if source.key.is_empty() { "?" } else { source.key.as_str() };
                return Err(format!(
                    "attribution {key:?} lacks {} — a third-party source we cannot credit is one we cannot ship",
                    missing.join(", ")
                ));
            }
        }
        Ok(credits)
    }

    /// The crawl, top to bottom: the team (role, then name) and every
    /// shipped source (contribution, then author) in the catalog's order.
    pub fn roll(&self) -> Vec<RollEntry> {
        self.team
            .iter()
            .map(|member| RollEntry { contribution: member.role.clone(), person: member.name.clone() })
            .chain(self.source.iter().filter(|s| s.ships).map(|source| RollEntry {
                contribution: source.contribution.clone(),
                person: source.author.clone(),
            }))
            .collect()
    }
}

/// The one shipped catalog (parsed on first use; a bad catalog panics
/// with the violation, exactly like the asset catalog).
pub fn credits() -> &'static Credits {
    static CREDITS: std::sync::OnceLock<Credits> = std::sync::OnceLock::new();
    CREDITS.get_or_init(|| {
        Credits::load(ATTRIBUTIONS_TOML)
            .unwrap_or_else(|e| panic!("catalog/attributions.toml must load:\n{e}"))
    })
}

/// What drives the crawl this frame: its own motion, the player holding a
/// direction, or a hold (a capture parks it and reads it still).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollDrive {
    Auto,
    Forward,
    Back,
    Hold,
}

/// The crawl's motion: an offset in pixels, how far the column has risen
/// from below the window's bottom edge, over a travel of `extent` pixels
/// (the column's height plus the window's — bottom edge to past the top).
/// It rises on its own at [`Roll::SPEED`], [`Roll::BOOST`] times faster
/// under a held Down, backwards under Up, and clamps at both ends; the
/// shell feeds the extent from its layout each frame and applies the
/// offset it gets back, and leaves when the crawl is [`Roll::finished`].
#[derive(Debug, Clone, PartialEq)]
pub struct Roll {
    offset: f32,
    extent: f32,
}

impl Roll {
    /// Pixels per second the crawl rises on its own — a reading pace for
    /// a line of body text every half second or so.
    pub const SPEED: f32 = 60.0;
    /// Multiplier under a held direction.
    pub const BOOST: f32 = 6.0;

    pub fn new() -> Self {
        Self { offset: 0.0, extent: 0.0 }
    }

    pub fn offset(&self) -> f32 {
        self.offset
    }

    pub fn extent(&self) -> f32 {
        self.extent
    }

    /// The crawl's full travel, from the shell's layout (the column's
    /// height plus the window's); an offset past a shrunken extent is
    /// pulled back to the end.
    pub fn set_extent(&mut self, extent: f32) {
        self.extent = extent.max(0.0);
        self.offset = self.offset.clamp(0.0, self.extent);
    }

    /// Jump to `offset`, clamped to the roll.
    pub fn park(&mut self, offset: f32) {
        self.offset = offset.clamp(0.0, self.extent);
    }

    /// Move by `dt` seconds under `drive`; returns the new offset.
    pub fn advance(&mut self, dt: f32, drive: RollDrive) -> f32 {
        let velocity = match drive {
            RollDrive::Auto => Self::SPEED,
            RollDrive::Forward => Self::SPEED * Self::BOOST,
            RollDrive::Back => -Self::SPEED * Self::BOOST,
            RollDrive::Hold => 0.0,
        };
        self.park(self.offset + velocity * dt);
        self.offset
    }

    /// The last line has cleared the top: the crawl is over.
    pub fn finished(&self) -> bool {
        self.offset >= self.extent
    }
}

impl Default for Roll {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
[[team]]
name = "A. Director"
role = "Design, direction"

[[team]]
name = "B. Builder"
role = "Code"

[[source]]
key = "zeta"
name = "Zeta star map"
author = "Zeta Observatory"
url = "https://zeta.example"
license = "public domain"
credit = "Zeta Observatory"
used_for = "the sky"
contribution = "Star field"

[[source]]
key = "alpha"
name = "Alpha kits"
author = "Alpha"
license = "CC0 1.0"
credit = "Models by Alpha"
used_for = "the panels"
contribution = "Wall panel kits"

[[source]]
key = "parked"
name = "Parked textures"
author = "Parked"
license = "free"
credit = "Parked"
used_for = "nothing yet"
ships = false
"#;

    fn pairs(entries: &[RollEntry]) -> Vec<(&str, &str)> {
        entries.iter().map(|e| (e.contribution.as_str(), e.person.as_str())).collect()
    }

    #[test]
    fn the_crawl_is_contribution_then_person_team_first_then_sources_in_catalog_order() {
        let roll = Credits::load(FIXTURE).unwrap().roll();
        assert_eq!(pairs(&roll), vec![
            ("Design, direction", "A. Director"),
            ("Code", "B. Builder"),
            ("Star field", "Zeta Observatory"),
            ("Wall panel kits", "Alpha"),
        ], "the catalog's own order, not alphabetical — Zeta was declared before Alpha");
    }

    #[test]
    fn a_source_that_does_not_ship_is_off_the_crawl() {
        let roll = Credits::load(FIXTURE).unwrap().roll();
        assert!(roll.iter().all(|e| e.person != "Parked"));
    }

    #[test]
    fn a_shipped_source_without_a_crawl_line_refuses_to_load() {
        let broken = FIXTURE.replace("contribution = \"Wall panel kits\"\n", "");
        let err = Credits::load(&broken).expect_err("a source with no line for the crawl must not load");
        assert!(err.contains("alpha") && err.contains("contribution"), "names the source and the hole: {err}");
    }

    #[test]
    fn a_parked_source_needs_no_crawl_line() {
        let credits = Credits::load(FIXTURE).unwrap();
        let parked = credits.source.iter().find(|s| s.key == "parked").unwrap();
        assert!(!parked.ships && parked.contribution.is_empty(), "off the crawl, nothing to say on it");
    }

    #[test]
    fn a_source_without_a_credit_line_refuses_to_load() {
        let broken = FIXTURE.replace("credit = \"Models by Alpha\"\n", "");
        let err = Credits::load(&broken).expect_err("a source we cannot credit must not load");
        assert!(err.contains("alpha"), "the refusal names the source: {err}");
    }

    #[test]
    fn a_team_member_without_a_role_refuses_to_load() {
        let broken = FIXTURE.replace("role = \"Code\"\n", "");
        let err = Credits::load(&broken).expect_err("a member without a role must not load");
        assert!(err.contains("B. Builder"), "the refusal names the member: {err}");
    }

    #[test]
    fn the_shipped_catalog_loads_and_opens_on_the_team() {
        let credits = credits();
        let roll = credits.roll();
        let team = credits.team.first().expect("the shipped catalog names a team");
        assert_eq!(roll.first().map(|e| e.person.as_str()), Some(team.name.as_str()),
            "the crawl opens on the first team member");
        assert!(roll.len() > credits.team.len(), "at least one shipped source is credited");
    }

    // ---- the crawl's motion ----

    fn roll_of(extent: f32) -> Roll {
        let mut roll = Roll::new();
        roll.set_extent(extent);
        roll
    }

    #[test]
    fn the_roll_advances_on_its_own_at_its_speed() {
        let mut roll = roll_of(1000.0);
        let after = roll.advance(0.5, RollDrive::Auto);
        assert!((after - Roll::SPEED * 0.5).abs() < 1e-4, "half a second at SPEED: {after}");
        assert_eq!(roll.offset(), after);
        assert!(!roll.finished());
    }

    #[test]
    fn the_roll_stops_at_its_end_and_says_so() {
        let mut roll = roll_of(100.0);
        let after = roll.advance(1000.0, RollDrive::Auto);
        assert_eq!(after, 100.0, "clamped to the extent");
        assert!(roll.finished());
    }

    #[test]
    fn a_held_direction_boosts_forward_or_rewinds_and_clamps_at_the_top() {
        let mut roll = roll_of(1000.0);
        let forward = roll.advance(1.0, RollDrive::Forward);
        assert!((forward - Roll::SPEED * Roll::BOOST).abs() < 1e-4, "{forward}");
        let back = roll.advance(0.5, RollDrive::Back);
        assert!((back - Roll::SPEED * Roll::BOOST * 0.5).abs() < 1e-4, "rewinds at the same boost: {back}");
        assert_eq!(roll.advance(100.0, RollDrive::Back), 0.0, "clamped at the top");
    }

    #[test]
    fn a_hold_keeps_the_roll_still() {
        let mut roll = roll_of(1000.0);
        roll.park(300.0);
        assert_eq!(roll.advance(5.0, RollDrive::Hold), 300.0);
    }

    #[test]
    fn parking_and_a_shrinking_extent_both_clamp() {
        let mut roll = roll_of(500.0);
        roll.park(900.0);
        assert_eq!(roll.offset(), 500.0, "park past the end lands at the end");
        roll.park(-10.0);
        assert_eq!(roll.offset(), 0.0, "park above the top lands at the top");
        roll.park(400.0);
        roll.set_extent(250.0);
        assert_eq!(roll.offset(), 250.0, "content re-laid out shorter pulls the offset back");
        assert_eq!(roll.extent(), 250.0);
    }
}
