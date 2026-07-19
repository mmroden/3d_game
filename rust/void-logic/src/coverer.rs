//! The per-course COVERER (phase 3): covers a 1D run — a wall course, a
//! floor strip — with plates from a kit's width family, exactly, seeded.
//! This is coin-change DP, deliberately: the general cover problem is
//! NP-hard, ours never is, and the universal-filler rule (every kit
//! pools a 1-module plate; the linker enforces it) keeps completion
//! guaranteed for every run the cell grid can produce. Widths are meters
//! (f32 at the API), quantized to centimeters internally so the DP is
//! exact — the probe's own snap tolerance.

use rand::rngs::SmallRng;

/// Cover a run of `len` meters with plate widths drawn from `widths`,
/// seeded by `rng`: returns the chosen widths in placement order,
/// summing exactly to `len` (within centimeter quantization). `None`
/// when no partition of `len` by `widths` exists — with the filler rule
/// in force that only happens for runs shorter than one module.
pub fn cover_run(len: f32, widths: &[f32], rng: &mut SmallRng) -> Option<Vec<f32>> {
    use rand::RngExt;

    let target = (len * 100.0).round() as i64;
    let steps: Vec<i64> = widths.iter().map(|w| (w * 100.0).round() as i64).collect();
    if target < 0 || steps.iter().any(|&s| s <= 0) {
        return None;
    }
    if target == 0 {
        return Some(Vec::new());
    }

    // feasible[r]: r centimeters can be covered exactly. O(len × family).
    let n = target as usize;
    let mut feasible = vec![false; n + 1];
    feasible[0] = true;
    for r in 1..=n {
        feasible[r] = steps
            .iter()
            .any(|&s| (s as usize) <= r && feasible[r - s as usize]);
    }
    if !feasible[n] {
        return None;
    }

    // The seeded walk: at every position choose uniformly among widths
    // whose remainder stays feasible — no partition is unreachable, the
    // seed picks among them, and feasibility means the walk cannot
    // dead-end.
    let mut out = Vec::new();
    let mut rem = n;
    while rem > 0 {
        let options: Vec<usize> = steps
            .iter()
            .enumerate()
            .filter(|&(_, &s)| (s as usize) <= rem && feasible[rem - s as usize])
            .map(|(i, _)| i)
            .collect();
        let pick = options[rng.random_range(0..options.len())];
        out.push(widths[pick]);
        rem -= steps[pick] as usize;
    }
    Some(out)
}

/// Split a course of `len` meters into coverable sub-runs around holes
/// (connector openings), each `(offset, len)` from the course origin.
/// Holes are `(offset, len)` too; they may touch but not overlap.
pub fn split_runs(len: f32, holes: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut holes: Vec<(f32, f32)> = holes.to_vec();
    holes.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut runs = Vec::new();
    let mut cursor = 0.0f32;
    for &(off, hole_len) in &holes {
        if off > cursor + 1e-4 {
            runs.push((cursor, off - cursor));
        }
        cursor = cursor.max(off + hole_len);
    }
    if len > cursor + 1e-4 {
        runs.push((cursor, len - cursor));
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    /// vol01's measured family — used as a REPRESENTATIVE shape (integer
    /// meters, filler included), never as a pin on shipped data: the
    /// properties below hold for any family containing the module.
    const FAMILY: [f32; 4] = [3.0, 4.0, 5.0, 6.0];

    fn rng(seed: u64) -> SmallRng {
        SmallRng::seed_from_u64(seed)
    }

    #[test]
    fn every_feasible_run_covers_exactly() {
        // With {3,4,5,6}, every integer length >= 3 has a partition.
        for len in 3..=60 {
            let len = len as f32;
            let cover = cover_run(len, &FAMILY, &mut rng(7))
                .unwrap_or_else(|| panic!("run of {len} m must cover"));
            let sum: f32 = cover.iter().sum();
            assert!(
                (sum - len).abs() < 0.02,
                "run {len}: cover sums to {sum}"
            );
            for w in &cover {
                assert!(
                    FAMILY.iter().any(|f| (f - w).abs() < 0.001),
                    "run {len}: width {w} is not in the family"
                );
            }
        }
    }

    #[test]
    fn infeasible_runs_refuse_and_empty_runs_are_trivial() {
        assert_eq!(cover_run(1.0, &FAMILY, &mut rng(1)), None, "1 m has no partition");
        assert_eq!(cover_run(2.0, &FAMILY, &mut rng(1)), None, "2 m has no partition");
        assert_eq!(
            cover_run(0.0, &FAMILY, &mut rng(1)),
            Some(Vec::new()),
            "an empty run needs nothing"
        );
        assert_eq!(
            cover_run(4.0, &[3.0], &mut rng(1)),
            None,
            "a filler-only family cannot cover an off-module run"
        );
    }

    #[test]
    fn covers_are_seed_deterministic_and_varied() {
        let a = cover_run(30.0, &FAMILY, &mut rng(11)).unwrap();
        let b = cover_run(30.0, &FAMILY, &mut rng(11)).unwrap();
        assert_eq!(a, b, "same seed, same cover");

        use std::collections::HashSet;
        let mut distinct: HashSet<Vec<u32>> = HashSet::new();
        let mut widths_seen: HashSet<u32> = HashSet::new();
        for seed in 0..40 {
            let c = cover_run(30.0, &FAMILY, &mut rng(seed)).unwrap();
            widths_seen.extend(c.iter().map(|w| (w * 100.0).round() as u32));
            distinct.insert(c.iter().map(|w| (w * 100.0).round() as u32).collect());
        }
        assert!(distinct.len() > 1, "seeds explore distinct partitions");
        assert_eq!(
            widths_seen.len(),
            FAMILY.len(),
            "every family width appears across seeds"
        );
    }

    #[test]
    fn holes_split_a_course_into_the_complement() {
        // A 12 m course with a 3 m hole at 3: runs [0,3) and [6,12).
        let runs = split_runs(12.0, &[(3.0, 3.0)]);
        assert_eq!(runs, vec![(0.0, 3.0), (6.0, 6.0)]);

        // Hole at the start, hole at the end: one middle run.
        let runs = split_runs(12.0, &[(0.0, 3.0), (9.0, 3.0)]);
        assert_eq!(runs, vec![(3.0, 6.0)]);

        // No holes: the whole course.
        assert_eq!(split_runs(9.0, &[]), vec![(0.0, 9.0)]);

        // Wall-to-wall hole: nothing to cover.
        assert_eq!(split_runs(3.0, &[(0.0, 3.0)]), Vec::<(f32, f32)>::new());
    }
}
