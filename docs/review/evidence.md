# Evidence contract

Every assertion by every reviewer is backed by evidence linked to particular
lines of code. An unsupported impression is not a finding. This file is the
output contract for all review agents and the collation rules for the
orchestrator.

## Rules for a finding

1. **Site.** Repo-relative path and line number or range, at the post-change
   state (HEAD, or the working tree when the working tree is in scope). For
   removed code, cite the main-side line and mark it `(removed)`.
2. **Quote.** The exact line or lines, at most about six. The reader must be
   able to see the evidence without opening the file.
3. **Principle.** Which document under `docs/review/` and which numbered check
   within it. A finding that fits no check is either a new check to propose or
   not a finding.
4. **Claim.** One sentence stating the defect.
5. **Consequence.** Concrete. Either a failure scenario (these inputs or this
   state, this wrong outcome) or the specific design cost (what the next change
   in this area will be forced to do because of this).
6. **Alternative.** What the design-respecting version looks like, concretely:
   name the type, trait, function, or module it would live on. "Refactor this"
   is not an alternative.
7. **Confidence.** `CONFIRMED` means the reviewer reproduced it, ran it, or
   enumerated every reference with Serena and the claim held. `PLAUSIBLE`
   means reasoned but not verified. A `PLAUSIBLE` finding is never stated as
   fact, and the reviewer says what verification would settle it.
8. **No finding without a site.** General impressions belong in the reviewer's
   closing summary, not in the findings list.
9. **Silence over praise.** Absence of findings in a category is silence, not
   a compliment.
10. **Navigate semantically.** Code is structured, not text. Use Serena
    (`get_symbols_overview`, `find_symbol`, `find_referencing_symbols`,
    `find_implementations`). Before claiming "nothing calls this" or "used in
    N places", every reference must have been enumerated. Text search is for
    genuinely textual content: comments, docs, Makefiles, TOML.
11. **Read whole files.** Violations live in how a change relates to its
    surroundings, not in the hunk. Read every touched file in full.
12. **Review only.** No reviewer makes any call that can change anything.
    No edits, no file writes, no commits, no builds, no test runs, no probe
    scripts, no `make`, no `cargo`, no `python`. Bash is for read-only git
    commands only (`git diff`, `git show`, `git log`, `git status`,
    `git blame`). `CONFIRMED` therefore means confirmed by reading: every
    reference enumerated, every input traced to the line that produces the
    wrong outcome. Anything that would need to be run to confirm is
    `PLAUSIBLE`, with the exact command or test the owner could run to settle
    it.

## Severity

- **BLOCKER.** Violates a principle; must be fixed before merge. State the
  principle, the evidence, and the alternative.
- **CONCERN.** Likely debt or design drift. Needs an owner decision or a
  tracked follow-up before merge, and the review says which.
- **NIT.** Minor. Fix if touching the file anyway.

Severity is about the design cost, not about how hard the fix is. A one-line
change that entrenches a second identity space is a BLOCKER. A three-hundred
line rename is a NIT.

## Output format

Findings first, ordered by severity, in this exact shape so the orchestrator
can collate them mechanically:

```
### F1 [BLOCKER] [CONFIRMED] Second RNG seeded from the raw run seed
- principle: principles/pattern_literacy.md §3
- site: rust/void-logic/src/director.rs:41-44
- quote:
    let mut rng = SmallRng::seed_from_u64(seed.0);
- claim: The director draws from the same bit-stream as level generation.
- consequence: Any director tuning change shifts every level layout for the same seed; the two domains are correlated, not independent.
- alternative: Derive through `Seed::for_level` with a `DIRECTOR_SALT`, following the `HULL_SALT` pattern at rust/void-logic/src/level/hull.rs:17.
```

After the findings, one section titled `Reviewer's summary`: at most one
paragraph, in prose, of what this focus saw across the whole diff. A focus
whose criteria require an extra section (the DRY focus's `Entanglement
drag`, the Definition of Done focus's coverage table, the strategic focus's
pattern table, the adversarial focus's coverage and refuted lists) produces
it after the summary.

If a lens finds nothing, the findings list is empty and the summary says so in
one sentence. Do not manufacture findings to fill the list.

## Collation rules for the orchestrator

1. **Same site.** Two findings are the same site when their line ranges
   overlap in the same file, or they name the same symbol. Merge them into one
   finding that lists every principle that tagged it. Severity is the maximum
   across the group. Confidence is the maximum. Record the convergence count.
2. **Ranking.** Severity, then convergence count, then confidence.
3. **Contradiction.** When reviewers disagree about a site, present both
   positions. The orchestrator adjudicates only with evidence of its own,
   cited to the same standard; otherwise it marks the finding `UNRESOLVED` and
   states what would settle it.
4. **Refutation.** The orchestrator never silently drops a finding. A finding
   is discarded only when the orchestrator can cite evidence refuting it, and
   it then appears under a `Refuted` heading with that evidence.
5. **Verdict.** Any BLOCKER: `REQUEST CHANGES`. Only CONCERN and NIT: `APPROVE
   WITH NITS`, naming every CONCERN that needs an owner decision. Nothing:
   `APPROVE`.
6. **Triage is the owner's.** The collated report presents cost, value, and a
   recommendation per finding. Which findings get fixed before merge and which
   become follow-ups is not the orchestrator's decision.
7. **Merging is never the goal.** The owner's standing decision is that
   architectural and structural issues are fixed before merge, however long
   that takes, to pay down technical debt as we go. Severity is never lowered,
   and a BLOCKER is never re-labelled a follow-up, to make a merge convenient.
   A reviewer or orchestrator tempted to write "acceptable for this PR" about
   a structural finding is describing the failure mode this review exists to
   catch.

## Collated report shape

1. One-paragraph verdict with the single most important reason.
2. Merged findings, ranked, each showing its convergence (which lenses tagged
   it) and the evidence block.
3. `Design intent`: in prose, what the original design wanted for the areas
   touched, and whether the change leaves the codebase more or less coherent.
4. `Entanglement drag`: the atherosclerosis verdict, always present, even when
   the answer is "no prior plaque was touched", which is itself a data point.
5. `Refuted` and `Unresolved`, when non-empty.
6. `Gaps`: anything a reviewer could not examine (files too large, tooling
   unavailable) so the owner knows what was not looked at.
