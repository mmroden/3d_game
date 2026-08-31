//! The tuning corpus: labeled records precomputed once through the
//! shipped pipeline, and the seeded fold deal every instrument shares.
//!
//! One home for three rules that must not fork: what a finding is (the
//! flattened teacher's own sheet), when a finding hits a record's
//! headline (identity AND tokens, exactly the exam's matching), and how
//! records deal into folds (a seeded splitmix64 shuffle, the harness's
//! only randomness). The greedy tuner and the fitter both read THIS, so
//! a comparison between them compares models, never yardsticks.

use shakmaty::{CastlingMode, Chess, fen::Fen};
use std::collections::BTreeMap;

/// One ranked finding of one record, precomputed: the owning lesson
/// (`None` for a weighed row no lesson owns -- untunable, priced at its
/// flat weight forever), its relevance (loudness under the flattened
/// teacher), its live-rung bits, and whether it satisfies the record's
/// headline label.
pub struct Finding {
    /// The owning lesson's slug, or `None` for the breaks hybrid.
    pub slug: Option<&'static str>,
    /// The row's loudness under the flattened teacher.
    pub relevance: f32,
    /// The six live-rung bits, read through the one curriculum reader.
    pub bits: curriculum::silman::chains::ChainBits,
    /// Whether this finding satisfies the record's headline label.
    pub hits_label: bool,
}

/// One scored record: id, section (the fold stratifier), and its
/// findings in emission order -- the tie order the shipped ranking
/// keeps under every candidate weighting.
pub struct Record {
    /// The record's stable id, `section-index`.
    pub id: String,
    /// The chapter-test section, for stratified fold deals.
    pub section: String,
    /// The record's findings, emission order.
    pub findings: Vec<Finding>,
}

/// The headline label's matching rule, THE one the exam grades with:
/// identity first (the owning lesson's slug), wording as the
/// disambiguator (every token, lowercased containment). A label
/// specifying neither matches nothing, exactly like an absent headline.
pub fn hits_label(
    wanted_lesson: Option<&str>,
    tokens: &[String],
    slug: Option<&str>,
    text: &str,
) -> bool {
    if wanted_lesson.is_none() && tokens.is_empty() {
        return false;
    }
    let lesson_hits = wanted_lesson.is_none_or(|wanted| slug.is_some_and(|s| s == wanted));
    let lowered = text.to_lowercase();
    lesson_hits && tokens.iter().all(|token| lowered.contains(token))
}

/// Load one records file into precomputed findings, through the shipped
/// pipeline under the FLATTENED teacher: every `.base` knob to 1,
/// lesson bases and chain dials alike, so a finding's loudness IS its
/// relevance and the candidate transformation happens in the
/// instruments, on top of the one pipeline. Excluded records are
/// dropped by the exam's own classifier.
pub fn load(path: &std::path::Path) -> Vec<Record> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("{} is missing; nothing to tune against", path.display()));
    let records: serde_json::Value = serde_json::from_str(&text).expect("records JSON parses");
    let records = records.as_array().expect("top level is an array");

    let flat = || {
        use curriculum::knobs::Knobs;
        let mut teacher = curriculum::silman::Silman::landed();
        for name in teacher.names() {
            if name.ends_with(".base") {
                assert!(teacher.set(name, 1.0), "a named knob turns");
            }
        }
        teacher
    };
    let lessons = curriculum::silman::Lessons::landed();
    // Registry-driven slug lookup: the census of lessons is the
    // registry itself, so this cannot drift from it.
    let id_of: BTreeMap<&'static str, curriculum::silman::LessonId> = lessons
        .all()
        .iter()
        .map(|lesson| (lesson.id().slug(), lesson.id()))
        .collect();

    records
        .iter()
        .filter(|record| {
            match crate::exclusion::classify(record) {
                Ok(class) => class.is_none(),
                Err(why) => panic!("{}: {why}", record["id"].as_str().unwrap_or("?")),
            }
        })
        .map(|record| {
            let id = record["id"].as_str().unwrap_or("?").to_string();
            let section = record["section"].as_str().unwrap_or("?").to_string();
            // The id format "section-index" is the exam schema's pin,
            // and fold stratification reads the section straight off
            // the id -- this assert keeps the two from drifting apart.
            assert!(
                id.starts_with(&format!("{section}-")),
                "{id}: id does not carry its section prefix {section:?}"
            );
            let fen = record["fen"]
                .as_str()
                .unwrap_or_else(|| panic!("{id}: fen"));
            let position: Chess = fen
                .parse::<Fen>()
                .unwrap_or_else(|e| panic!("{id}: FEN does not parse: {e}"))
                .into_position(CastlingMode::Standard)
                .unwrap_or_else(|e| panic!("{id}: position is illegal: {e}"));
            let wanted_lesson = record["headline"]["lesson"].as_str();
            let tokens: Vec<String> = record["headline"]["contains"]
                .as_array()
                .map(|tokens| {
                    tokens
                        .iter()
                        .filter_map(|t| t.as_str())
                        .map(|t| t.to_lowercase())
                        .collect()
                })
                .unwrap_or_default();
            let sheet =
                curriculum::Teaching::assemble_with(&crate::game_record(&position), flat());
            let context =
                curriculum::silman::chains::ChainContext::of_sheet(sheet.facts(), &lessons);
            // Findings keep the sheet's walk order: instruments re-sort
            // with a stable sort under every candidate, so this order IS
            // the tie order, exactly as the shipped ranking breaks ties.
            let findings = sheet
                .facts()
                .iter()
                .filter_map(|fact| {
                    let relevance = fact.loudness()?;
                    let slug = match fact.kind() {
                        curriculum::ReadingKind::Lesson(slug) => Some(slug),
                        _ => None,
                    };
                    let bits = slug
                        .and_then(|slug| id_of.get(slug).copied())
                        .map(|lesson| context.bits(&lessons, lesson))
                        .unwrap_or_default();
                    Some(Finding {
                        slug,
                        relevance,
                        bits,
                        hits_label: hits_label(wanted_lesson, &tokens, slug, fact.text()),
                    })
                })
                .collect();
            Record {
                id,
                section,
                findings,
            }
        })
        .collect()
}

/// splitmix64: the whole of the harnesses' randomness, seeded and
/// stated.
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Deterministic STRATIFIED fold assignment, per the plan of record:
/// ids are grouped by section (the "section-index" id format the exam
/// schema pins; [`load`] asserts it against the record's own section
/// field), each section is shuffled by one seeded Fisher-Yates stream
/// in section order, and the round-robin deal carries its counter
/// across sections -- so every section spreads across folds within one
/// record of even, and total fold sizes stay within one of each other.
/// Sections differ wildly in difficulty, which is why a section
/// bunching into one fold would make fold scores incomparable. Same
/// ids and seed -> same folds, independent of load order -- and shared
/// here so the greedy tuner and the fitter argue over identical folds.
pub fn fold_of(ids: &[String], seed: u64, folds: usize) -> BTreeMap<String, usize> {
    let mut sections: BTreeMap<&str, Vec<&String>> = BTreeMap::new();
    for id in ids {
        let section = id.rsplit_once('-').map_or(id.as_str(), |(head, _)| head);
        sections.entry(section).or_default().push(id);
    }
    let mut state = seed;
    let mut next = 0;
    let mut out = BTreeMap::new();
    for members in sections.values_mut() {
        members.sort();
        for i in (1..members.len()).rev() {
            let j = (splitmix64(&mut state) % (i as u64 + 1)) as usize;
            members.swap(i, j);
        }
        for id in members.iter() {
            out.insert((*id).clone(), next % folds);
            next += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Folds are a deterministic function of ids and seed, every fold is
    /// populated when there are at least as many ids as folds, and the
    /// deal is stratified: each section spreads across folds within one
    /// record of even, whatever the seed shuffles.
    #[test]
    fn folds_are_seeded_and_deterministic() {
        let ids: Vec<String> = (0..9)
            .map(|i| format!("alpha-{i}"))
            .chain((0..4).map(|i| format!("beta-{i}")))
            .chain((0..25).map(|i| format!("gamma-{i}")))
            .collect();
        let once = fold_of(&ids, 42, 5);
        let again = fold_of(&ids, 42, 5);
        assert_eq!(once, again);
        let different = fold_of(&ids, 43, 5);
        assert_ne!(once, different, "a different seed deals differently");
        for fold in 0..5 {
            assert!(once.values().any(|&f| f == fold), "fold {fold} is empty");
        }
        for deal in [&once, &different] {
            for section in ["alpha", "beta", "gamma"] {
                let mut counts = [0usize; 5];
                for (id, fold) in deal {
                    if id.starts_with(section) {
                        counts[*fold] += 1;
                    }
                }
                let most = counts.iter().max().copied().unwrap_or(0);
                let least = counts.iter().min().copied().unwrap_or(0);
                assert!(
                    most - least <= 1,
                    "section {section} bunches: fold counts {counts:?}"
                );
            }
        }
    }

    /// The label rule: identity decides when given, tokens disambiguate,
    /// and a label naming neither matches nothing.
    #[test]
    fn the_label_matches_by_identity_then_tokens() {
        let tokens = vec!["e5".to_string()];
        assert!(hits_label(
            Some("silman.blockade"),
            &tokens,
            Some("silman.blockade"),
            "The knight blockades on e5."
        ));
        assert!(!hits_label(
            Some("silman.blockade"),
            &tokens,
            Some("silman.blockade"),
            "The knight blockades on d6."
        ));
        assert!(!hits_label(
            Some("silman.blockade"),
            &tokens,
            Some("silman.passed-pawn"),
            "The pawn on e5 is passed."
        ));
        assert!(hits_label(
            None,
            &tokens,
            None,
            "A census sentence naming e5."
        ));
        assert!(!hits_label(None, &[], Some("silman.blockade"), "anything"));
    }
}
