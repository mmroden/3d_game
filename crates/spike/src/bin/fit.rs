//! The fitter: the salience regression. Conditional logistic
//! regression -- a softmax over each record's findings -- with the six
//! chain dials as shared features: the exact generalization of the
//! greedy tuner's `base x relevance`, fitted jointly instead of one
//! knob at a time (docs/plan-salience-regression.md).
//!
//! `make spike B=fit` -- defaults are the reported protocol.
//! `ARGS="--folds 5 --seed 20260809 --iters 3000 --step 0.5 --out tmp/fit
//!        --records <path> (repeatable) --ranks"`
//!
//! # The model
//!
//! `weight(f) = relevance(f) x exp(theta_lesson(f) + sum_k theta_k x_k(f))`
//!
//! One bias per lesson slug owning a finding anywhere (`exp(bias)` IS
//! the lesson's base), six shared chain coefficients by rung kind and
//! direction (`exp` of each is the dial's lift). All-zero theta
//! reproduces the flat model bit for bit, which is the journal's
//! opening guard.
//!
//! # The fit
//!
//! Loss: per labeled record, minus the log of the probability mass on
//! the findings hitting the headline -- any hitting instance counts,
//! exactly like the exam's first-hit rank. L1 on the biases (a lesson
//! leaves the flat prior only if the data insists) and light L2 on the
//! dials. Full-batch proximal gradient descent, zero init, fixed
//! budget, theta boxed to +-10 so a separable slug saturates instead
//! of running the exponent out of range. Convex: one optimum.
//!
//! Lambda is chosen by k-fold validation MRR on the corpus's shared
//! seeded folds; exact ties prefer the larger penalty (nearest the
//! flat prior). The honest generalization number is NESTED: per outer
//! fold the grid is chosen on the inner folds alone and the held fold
//! is scored once. Deployment refits on everything at the full-table
//! choice, per standard refit-after-selection.
//!
//! # Reproducibility
//!
//! Same records, same seed, same grid, same code -> byte-identical
//! journal. The corpus fold deal is the only randomness; the optimizer
//! has no RNG and no clock.

use spike::corpus;
use std::collections::BTreeMap;

/// The dials' knob names, in feature-column order -- the order
/// [`bits6`] flattens [`ChainBits`](curriculum::silman::chains::ChainBits)
/// into.
const CHAIN_KNOBS: [&str; 6] = [
    "chains.cascade-up.base",
    "chains.cascade-down.base",
    "chains.gate-up.base",
    "chains.gate-down.base",
    "chains.vocabulary-up.base",
    "chains.vocabulary-down.base",
];

/// The six bits as feature columns. Exhaustive destructure, no `..`:
/// a seventh rung feature fails the build here until the fitter can
/// reach it.
fn bits6(bits: curriculum::silman::chains::ChainBits) -> [bool; 6] {
    let curriculum::silman::chains::ChainBits {
        cascade_up,
        cascade_down,
        gate_up,
        gate_down,
        vocabulary_up,
        vocabulary_down,
    } = bits;
    [
        cascade_up,
        cascade_down,
        gate_up,
        gate_down,
        vocabulary_up,
        vocabulary_down,
    ]
}

/// One finding, encoded for the optimizer: lesson index into the slug
/// table (`None` untunable), relevance, feature bits, label hit.
struct Finding {
    lesson: Option<usize>,
    relevance: f64,
    bits: [bool; 6],
    hits: bool,
}

/// One record: id, encoded findings in emission order, and whether the
/// loss can learn from it (a hitting finding with weight to move).
struct Record {
    id: String,
    findings: Vec<Finding>,
    learnable: bool,
}

/// The encoded corpus: records plus the slug table theta indexes into.
struct Encoded {
    records: Vec<Record>,
    slugs: Vec<&'static str>,
}

/// Encode the corpus: the tunable slug table is every lesson owning a
/// finding anywhere, sorted -- theta's first `slugs.len()` coordinates
/// are its biases, the last six are the chain dials.
fn encode(loaded: Vec<corpus::Record>) -> Encoded {
    let mut slugs: Vec<&'static str> = loaded
        .iter()
        .flat_map(|record| record.findings.iter().filter_map(|f| f.slug))
        .collect();
    slugs.sort_unstable();
    slugs.dedup();
    let index: BTreeMap<&'static str, usize> = slugs
        .iter()
        .enumerate()
        .map(|(index, slug)| (*slug, index))
        .collect();
    let records = loaded
        .into_iter()
        .map(|record| {
            let findings: Vec<Finding> = record
                .findings
                .iter()
                .map(|f| Finding {
                    lesson: f.slug.map(|slug| index[slug]),
                    relevance: f64::from(f.relevance),
                    bits: bits6(f.bits),
                    hits: f.hits_label,
                })
                .collect();
            // A zero-relevance hit carries no probability mass and
            // cannot be learned from; such a record is a structural
            // zero exactly like a label with no hitting finding.
            let learnable = findings.iter().any(|f| f.hits && f.relevance > 0.0);
            Record {
                id: record.id,
                findings,
                learnable,
            }
        })
        .collect();
    Encoded { records, slugs }
}

/// One finding's exponent under theta.
fn exponent(theta: &[f64], n_slugs: usize, finding: &Finding) -> f64 {
    let mut z = finding.lesson.map_or(0.0, |lesson| theta[lesson]);
    for (k, &bit) in finding.bits.iter().enumerate() {
        if bit {
            z += theta[n_slugs + k];
        }
    }
    z
}

/// A record's candidate weights, emission order.
fn weights(theta: &[f64], n_slugs: usize, record: &Record) -> Vec<f64> {
    record
        .findings
        .iter()
        .map(|finding| finding.relevance * exponent(theta, n_slugs, finding).exp())
        .collect()
}

/// The headline's rank under theta, through the corpus's one
/// comparator.
fn rank_of(theta: &[f64], n_slugs: usize, record: &Record) -> Option<usize> {
    corpus::label_rank(
        weights(theta, n_slugs, record)
            .into_iter()
            .zip(record.findings.iter().map(|finding| finding.hits)),
    )
}

/// Mean reciprocal rank over records -- structural zeros included, the
/// exam's own denominator.
fn mrr(records: &[&Record], theta: &[f64], n_slugs: usize) -> f64 {
    if records.is_empty() {
        return 0.0;
    }
    records
        .iter()
        .map(|record| rank_of(theta, n_slugs, record).map_or(0.0, |rank| 1.0 / rank as f64))
        .sum::<f64>()
        / records.len() as f64
}

/// The averaged negative log-likelihood over learnable records, and
/// its gradient. Pure model, no penalty: the optimizer adds L2 itself
/// and applies L1 as the proximal step.
fn nll_grad(records: &[&Record], theta: &[f64], n_slugs: usize) -> (f64, Vec<f64>) {
    let mut nll = 0.0;
    let mut grad = vec![0.0; theta.len()];
    let mut learnable = 0usize;
    for record in records {
        if !record.learnable {
            continue;
        }
        learnable += 1;
        let w = weights(theta, n_slugs, record);
        let total: f64 = w.iter().sum();
        let hit_mass: f64 = w
            .iter()
            .zip(&record.findings)
            .filter(|(_, finding)| finding.hits)
            .map(|(weight, _)| weight)
            .sum();
        nll -= (hit_mass / total).ln();
        for (weight, finding) in w.iter().zip(&record.findings) {
            let p = weight / total;
            let q = if finding.hits { weight / hit_mass } else { 0.0 };
            let coefficient = p - q;
            if let Some(lesson) = finding.lesson {
                grad[lesson] += coefficient;
            }
            for (k, &bit) in finding.bits.iter().enumerate() {
                if bit {
                    grad[n_slugs + k] += coefficient;
                }
            }
        }
    }
    let n = learnable.max(1) as f64;
    nll /= n;
    for g in &mut grad {
        *g /= n;
    }
    (nll, grad)
}

/// The penalties: L1 on lesson biases, L2 on the six dials.
#[derive(Clone, Copy)]
struct Penalty {
    l1: f64,
    l2: f64,
}

/// Proximal gradient descent on the boxed, penalized objective.
/// Deterministic: zero init, fixed step, early stop on a still step.
/// The soft threshold is what makes an unearned bias EXACTLY zero --
/// the flat prior -- rather than merely small.
fn fit(records: &[&Record], n_slugs: usize, penalty: Penalty, iters: usize, step: f64) -> Vec<f64> {
    let mut theta = vec![0.0; n_slugs + 6];
    for _ in 0..iters {
        let (_, mut grad) = nll_grad(records, &theta, n_slugs);
        for k in 0..6 {
            grad[n_slugs + k] += 2.0 * penalty.l2 * theta[n_slugs + k];
        }
        let mut still = true;
        for (j, t) in theta.iter_mut().enumerate() {
            let before = *t;
            let mut next = *t - step * grad[j];
            if j < n_slugs {
                let bar = step * penalty.l1;
                next = if next > bar {
                    next - bar
                } else if next < -bar {
                    next + bar
                } else {
                    0.0
                };
            }
            // The box: a separable slug saturates at exp(+-10) instead
            // of running the exponent out of range.
            next = next.clamp(-10.0, 10.0);
            if (next - before).abs() > 1e-10 {
                still = false;
            }
            *t = next;
        }
        if still {
            break;
        }
    }
    theta
}

/// The lambda grids. Small and stated: six L1 stops from feather to
/// heavy, three L2 stops for the dials.
const L1_GRID: [f64; 6] = [0.0003, 0.001, 0.003, 0.01, 0.03, 0.1];
const L2_GRID: [f64; 3] = [0.001, 0.01, 0.1];

/// Choose the penalty on validation MRR over the given fold ids: per
/// pair, fit on each fold's complement and score the fold, take the
/// mean. Exact ties prefer the larger penalty (iteration is ascending
/// and `>=` keeps the later pair), the flat prior's side of the
/// argument. Returns the pair, its mean, and the table for the
/// journal.
fn choose(
    records: &[&Record],
    fold_ids: &[usize],
    of: &BTreeMap<String, usize>,
    n_slugs: usize,
    iters: usize,
    step: f64,
) -> (Penalty, f64, Vec<String>) {
    let mut best = (
        Penalty {
            l1: L1_GRID[0],
            l2: L2_GRID[0],
        },
        f64::MIN,
    );
    let mut table = Vec::new();
    for l1 in L1_GRID {
        let mut row = Vec::new();
        for l2 in L2_GRID {
            let penalty = Penalty { l1, l2 };
            let mut mean = 0.0;
            for &fold in fold_ids {
                let train: Vec<&Record> = records
                    .iter()
                    .filter(|record| of[&record.id] != fold)
                    .copied()
                    .collect();
                let held: Vec<&Record> = records
                    .iter()
                    .filter(|record| of[&record.id] == fold)
                    .copied()
                    .collect();
                let theta = fit(&train, n_slugs, penalty, iters, step);
                mean += mrr(&held, &theta, n_slugs);
            }
            mean /= fold_ids.len() as f64;
            row.push(format!("{mean:.4}"));
            if mean >= best.1 {
                best = (penalty, mean);
            }
        }
        table.push(format!("l1 {l1:<6}  validation MRR: {}", row.join("  ")));
    }
    table.push(format!(
        "(columns: l2 = {})",
        L2_GRID.map(|l2| l2.to_string()).join("  ")
    ));
    (best.0, best.1, table)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let value_of = |flag: &str, default: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1).cloned())
            .unwrap_or_else(|| default.to_string())
    };
    let folds: usize = value_of("--folds", "5")
        .parse()
        .expect("--folds is a count");
    let seed: u64 = value_of("--seed", "20260809")
        .parse()
        .expect("--seed is a number");
    let iters: usize = value_of("--iters", "3000")
        .parse()
        .expect("--iters is a count");
    let step: f64 = value_of("--step", "0.5")
        .parse()
        .expect("--step is a number");
    let out = value_of("--out", "tmp/fit");
    let ranks = args.iter().any(|a| a == "--ranks");
    // --records is repeatable: the merged corpus is however many halves
    // exist, and the fitter does not care which.
    let mut record_paths: Vec<std::path::PathBuf> = args
        .iter()
        .enumerate()
        .filter(|(_, a)| a.as_str() == "--records")
        .filter_map(|(i, _)| args.get(i + 1).map(std::path::PathBuf::from))
        .collect();
    if record_paths.is_empty() {
        record_paths.push(
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/exam"))
                .join("test-half/records.json"),
        );
    }

    let mut loaded = Vec::new();
    for path in &record_paths {
        loaded.extend(corpus::load(path));
    }
    let Encoded { records, slugs } = encode(loaded);
    let n_slugs = slugs.len();
    let all: Vec<&Record> = records.iter().collect();
    let learnable = all.iter().filter(|record| record.learnable).count();

    let mut journal = String::new();
    let mut log = |line: String| {
        println!("{line}");
        journal.push_str(&line);
        journal.push('\n');
    };

    log(format!(
        "fit: {} records ({learnable} learnable, {} structural zeros), \
         {n_slugs} tunable slugs + 6 chain dials, {folds} folds, seed {seed}, \
         l1 grid {}, l2 grid {}, iters {iters}, step {step}",
        all.len(),
        all.len() - learnable,
        L1_GRID.len(),
        L2_GRID.len()
    ));
    let flat = vec![0.0; n_slugs + 6];
    log(format!(
        "flat MRR {:.4}  (must reproduce the frozen flat baseline)",
        mrr(&all, &flat, n_slugs)
    ));

    let ids: Vec<String> = records.iter().map(|record| record.id.clone()).collect();
    let of = corpus::fold_of(&ids, seed, folds);

    log(String::new());
    log(
        "== nested cross-validation: the honest number -- per outer fold, the \
         grid is chosen on the inner folds alone and the held fold scored once =="
            .to_string(),
    );
    let mut held_scores = Vec::new();
    for outer in 0..folds {
        let train: Vec<&Record> = all
            .iter()
            .filter(|record| of[&record.id] != outer)
            .copied()
            .collect();
        let held: Vec<&Record> = all
            .iter()
            .filter(|record| of[&record.id] == outer)
            .copied()
            .collect();
        let inner: Vec<usize> = (0..folds).filter(|fold| *fold != outer).collect();
        let (penalty, _, _) = choose(&train, &inner, &of, n_slugs, iters, step);
        let theta = fit(&train, n_slugs, penalty, iters, step);
        let score = mrr(&held, &theta, n_slugs);
        held_scores.push(score);
        log(format!(
            "outer fold {outer}: chose l1 {} l2 {}  held MRR {score:.4}",
            penalty.l1, penalty.l2
        ));
    }
    let nested = held_scores.iter().sum::<f64>() / held_scores.len().max(1) as f64;
    let spread: Vec<String> = held_scores.iter().map(|s| format!("{s:.2}")).collect();
    log(format!(
        "nested validation MRR {nested:.4}  (folds: {})",
        spread.join(" ")
    ));

    log(String::new());
    log(
        "== deployment: the grid on the full fold table, then a refit on \
         everything at the chosen penalty =="
            .to_string(),
    );
    let every_fold: Vec<usize> = (0..folds).collect();
    let (penalty, mean, table) = choose(&all, &every_fold, &of, n_slugs, iters, step);
    for line in table {
        log(line);
    }
    log(format!(
        "chosen l1 {} l2 {}  (mean validation MRR {mean:.4})",
        penalty.l1, penalty.l2
    ));
    let theta = fit(&all, n_slugs, penalty, iters, step);
    let full = mrr(&all, &theta, n_slugs);
    let learnable_records: Vec<&Record> = all
        .iter()
        .filter(|record| record.learnable)
        .copied()
        .collect();
    let labeled_only = mrr(&learnable_records, &theta, n_slugs);
    let moved = theta[..n_slugs].iter().filter(|t| t.abs() > 0.0).count();
    log(format!(
        "final fit on all records: MRR {full:.4}  labeled-only {labeled_only:.4}  \
         ({moved} biases off the flat prior, of {n_slugs})"
    ));
    let dials: Vec<String> = CHAIN_KNOBS
        .iter()
        .zip(&theta[n_slugs..])
        .map(|(name, t)| format!("{name} {:.4}", t.exp()))
        .collect();
    log(format!("dials: {}", dials.join("  ")));

    if ranks {
        for record in &all {
            let rank = rank_of(&theta, n_slugs, record)
                .map_or_else(|| "absent".to_string(), |rank| format!("#{rank}"));
            log(format!("rank {:24} {rank}", record.id));
        }
    }

    let journal_path = format!("{out}-journal.txt");
    let state_path = format!("{out}-state.txt");
    std::fs::write(&journal_path, &journal).expect("the journal writes");
    let mut state =
        String::from("# fitted bases (exp of the bias); unlisted slugs are the flat prior 1\n");
    for (slug, t) in slugs.iter().zip(&theta[..n_slugs]) {
        if t.abs() > 0.0 {
            state.push_str(&format!("{slug} {:.6}\n", t.exp()));
        }
    }
    for (name, t) in CHAIN_KNOBS.iter().zip(&theta[n_slugs..]) {
        state.push_str(&format!("{name} {:.6}\n", t.exp()));
    }
    std::fs::write(&state_path, state).expect("the state writes");
    println!("journal: {journal_path}\nstate: {state_path}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(lesson: Option<usize>, relevance: f64, bits: [bool; 6], hits: bool) -> Finding {
        Finding {
            lesson,
            relevance,
            bits,
            hits,
        }
    }

    fn record(id: &str, findings: Vec<Finding>) -> Record {
        let learnable = findings.iter().any(|f| f.hits && f.relevance > 0.0);
        Record {
            id: id.into(),
            findings,
            learnable,
        }
    }

    const QUIET: [bool; 6] = [false; 6];

    /// GUARD: the analytic gradient is the numeric one -- central
    /// finite differences over every coordinate, on records mixing
    /// owned, unowned, chained and multi-hit findings.
    #[test]
    fn the_gradient_matches_finite_differences() {
        let records = [
            record(
                "a",
                vec![
                    finding(Some(0), 1.0, [true, false, false, false, false, true], true),
                    finding(Some(1), 2.0, QUIET, false),
                    finding(None, 0.5, QUIET, false),
                ],
            ),
            record(
                "b",
                vec![
                    finding(
                        Some(1),
                        1.5,
                        [false, true, false, false, false, false],
                        true,
                    ),
                    finding(Some(0), 1.0, QUIET, true),
                    finding(Some(1), 0.25, QUIET, false),
                ],
            ),
        ];
        let refs: Vec<&Record> = records.iter().collect();
        let n_slugs = 2;
        let theta: Vec<f64> = (0..n_slugs + 6).map(|i| 0.1 * i as f64 - 0.3).collect();
        let (_, grad) = nll_grad(&refs, &theta, n_slugs);
        let epsilon = 1e-6;
        for j in 0..theta.len() {
            let mut up = theta.clone();
            up[j] += epsilon;
            let mut down = theta.clone();
            down[j] -= epsilon;
            let numeric = (nll_grad(&refs, &up, n_slugs).0 - nll_grad(&refs, &down, n_slugs).0)
                / (2.0 * epsilon);
            assert!(
                (grad[j] - numeric).abs() < 1e-5,
                "coordinate {j}: analytic {} vs numeric {numeric}",
                grad[j]
            );
        }
    }

    /// GUARD: the proximal step leaves an unearned bias EXACTLY zero
    /// -- the flat prior, not merely a small number.
    #[test]
    fn the_l1_prox_reaches_exact_zero() {
        let records = [record(
            "a",
            vec![
                finding(Some(0), 1.0, QUIET, true),
                finding(Some(1), 1.0, QUIET, false),
            ],
        )];
        let refs: Vec<&Record> = records.iter().collect();
        let theta = fit(&refs, 2, Penalty { l1: 10.0, l2: 0.01 }, 500, 0.5);
        for (j, t) in theta.iter().take(2).enumerate() {
            assert_eq!(
                t.to_bits(),
                0.0f64.to_bits(),
                "bias {j} must sit exactly on the flat prior, got {t}"
            );
        }
    }

    /// GUARD: with two findings hitting the label, raising EITHER
    /// one's lesson lowers the loss -- any hitting instance counts,
    /// exactly like the exam's first-hit rank.
    #[test]
    fn any_hitting_instance_earns_credit() {
        let records = [record(
            "a",
            vec![
                finding(Some(0), 1.0, QUIET, true),
                finding(Some(1), 1.0, QUIET, true),
                finding(Some(2), 3.0, QUIET, false),
            ],
        )];
        let refs: Vec<&Record> = records.iter().collect();
        let flat = vec![0.0; 9];
        let (at_flat, _) = nll_grad(&refs, &flat, 3);
        for lesson in 0..2 {
            let mut theta = flat.clone();
            theta[lesson] = 1.0;
            let (raised, _) = nll_grad(&refs, &theta, 3);
            assert!(
                raised < at_flat,
                "raising lesson {lesson} must lower the loss: {raised} vs {at_flat}"
            );
        }
    }

    /// GUARD: all-zero theta prices every finding at its relevance --
    /// the flat model -- so the fitter's opening journal line can
    /// reproduce the frozen baseline.
    #[test]
    fn flat_theta_is_the_flat_model() {
        let records = [record(
            "a",
            vec![
                finding(Some(0), 2.0, QUIET, false),
                finding(Some(1), 1.0, [true; 6], true),
            ],
        )];
        let refs: Vec<&Record> = records.iter().collect();
        let flat = vec![0.0; 8];
        assert!(
            (mrr(&refs, &flat, 2) - 0.5).abs() < 1e-12,
            "the hit ranks second under relevance alone"
        );
    }

    /// GUARD: two fits of the same data are bit-identical -- the
    /// optimizer carries no clock and no RNG.
    #[test]
    fn the_fit_is_deterministic() {
        let records = [
            record(
                "a",
                vec![
                    finding(
                        Some(0),
                        1.0,
                        [true, false, false, false, false, false],
                        true,
                    ),
                    finding(Some(1), 2.0, QUIET, false),
                ],
            ),
            record(
                "b",
                vec![
                    finding(Some(1), 1.0, QUIET, true),
                    finding(Some(0), 1.0, QUIET, false),
                ],
            ),
        ];
        let refs: Vec<&Record> = records.iter().collect();
        let penalty = Penalty { l1: 0.01, l2: 0.01 };
        let once = fit(&refs, 2, penalty, 1000, 0.5);
        let again = fit(&refs, 2, penalty, 1000, 0.5);
        let bits = |theta: &[f64]| theta.iter().map(|t| t.to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&once), bits(&again));
    }
}
