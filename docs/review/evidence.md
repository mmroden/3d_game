# Evidence contract

Every assertion by every reviewer is backed by evidence linked to particular
lines of code. An unsupported impression is not a finding. This file is the
output contract for all review agents and the consolidation contract for the
orchestrator. The method (review only, code is the truth, navigation, scope)
is in `reviewer.md` and not repeated here.

## Rules for a finding

1. **Site.** Repo-relative path and line number or range, at the post-change
   state (the scoped ref, or the working tree when the working tree is in
   scope). For removed code, cite the main-side line and mark it `(removed)`.
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
7. **Confidence.** `CONFIRMED` means confirmed by reading: every reference
   enumerated with Serena, every input traced to the line that produces the
   outcome, and the claim held. `PLAUSIBLE` means reasoned but not traced to
   the end, or settleable only by running something; it is never stated as
   fact, and the finding says what would settle it. Nothing is ever run.
8. **No finding without a site.** General impressions belong in the reviewer's
   closing summary, not in the findings list.
9. **Silence over praise.** Absence of findings in a category is silence, not
   a compliment.

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
can consolidate them mechanically. The site and quote below are illustrative;
the registry the alternative names is real.

```
### F1 [BLOCKER] [CONFIRMED] Second RNG seeded from the raw level seed
- principle: principles/dry.md §1
- site: rust/void-logic/src/director.rs:41-44
- quote:
    let mut rng = SmallRng::seed_from_u64(seed.value());
- claim: The director draws from the same bit-stream as level generation.
- consequence: Any director tuning change shifts every level layout for the same seed; the two domains are correlated, not independent.
- alternative: Mix a named salt from the `seed::salt` registry (rust/void-logic/src/seed.rs), the `salt::HULL` pattern at rust/void-logic/src/boss.rs:30.
```

After the findings, one section titled `Reviewer's summary`: at most one
paragraph, in prose, of what this focus saw across the whole diff. A focus
whose criteria require an extra section (the DRY focus's `Entanglement
drag`, the Definition of Done focus's coverage table, the strategic focus's
pattern table, the adversarial focus's coverage and refuted lists) produces
it after the summary.

If a lens finds nothing, the findings list is empty and the summary says so in
one sentence. Do not manufacture findings to fill the list.

## Consolidation rules for the orchestrator

1. **Same site.** Two findings are the same site when their line ranges
   overlap in the same file, or they name the same symbol. Merge them into one
   finding that lists every principle that tagged it. Severity is the maximum
   across the group. Record the convergence count.
2. **Verify.** For every merged BLOCKER and CONCERN, open the file at the
   cited lines at the scoped ref. The quote must match the file. Trace the
   claim as the reviewer should have, enumerating what it depends on. Stamp
   the finding `VERIFIED` (you reproduced the reading), `REFUTED` (cite the
   line that disproves it; the finding moves under `Refuted` with that
   evidence), or `UNRESOLVED` (state what reading or run would settle it). A
   finding is never promoted above what you verified: a reviewer's
   `CONFIRMED` you could not reproduce is `UNRESOLVED`. NITs keep their
   reviewer's confidence and are not verified unless the check is trivial.
3. **Previously decided.** Read `docs/architecture/prior_decisions.md` and
   every report under `docs/reviews/` whose findings or `Outcomes` touch a
   file, symbol, or concept in the merged findings. A finding that restates a
   decision the owner already made (won't fix with a reason, deferred to a
   tracked home, an accepted exception) goes under `Previously decided` with
   the decision cited, and does not count toward the verdict unless the diff
   reopens the question: the tracked home is gone, or the accepted exception
   grew.
4. **Ranking.** Severity, then convergence count, then verification stamp.
5. **Contradiction.** When reviewers disagree about a site, present both
   positions. The orchestrator adjudicates only with evidence of its own,
   cited to the same standard; otherwise the finding is `UNRESOLVED` with
   what would settle it.
6. **Never silently drop.** A finding leaves the ranked list only by moving
   under `Refuted` (with evidence), `Previously decided` (with the decision),
   or `Gaps` (unsupported by evidence: no site or no quote).
7. **Verdict.** Any BLOCKER: `REQUEST CHANGES`. Only CONCERN and NIT: `APPROVE
   WITH NITS`, naming every CONCERN that needs an owner decision. Nothing:
   `APPROVE`.
8. **Triage is the owner's.** The collated report presents cost, value, and a
   recommendation per finding. Which findings get fixed before merge and which
   become follow-ups is not the orchestrator's decision.
9. **Merging is never the goal.** The owner's standing decision is that
   architectural and structural issues are fixed before merge, however long
   that takes, to pay down technical debt as we go. Severity is never lowered,
   and a BLOCKER is never re-labelled a follow-up, to make a merge convenient.
   A reviewer or orchestrator tempted to write "acceptable for this PR" about
   a structural finding is describing the failure mode this review exists to
   catch.

## Collated report shape

1. The scope line: commit range, working tree in or out, PR body read or
   none, each gate's result or "not run".
2. One-paragraph verdict with the single most important reason.
3. Merged findings, ranked, each showing its convergence (which lenses tagged
   it), its verification stamp, and the evidence block.
4. `Design intent`: in prose, what the original design wanted for the areas
   touched, and whether the change leaves the codebase more or less coherent.
5. `Entanglement drag`: the atherosclerosis verdict, always present, even when
   the answer is "no prior plaque was touched", which is itself a data point.
6. `Previously decided`, `Refuted`, and `Unresolved`, when non-empty.
7. `Gaps`: anything a reviewer could not examine (files too large, tooling
   unavailable, a Bash refusal) so the owner knows what was not looked at.

## The review log

The invoker files each collated report, verbatim, as
`docs/reviews/<YYYY-MM-DD>-<short sha>.md`, and after the owner's triage
appends an `## Outcomes` section: one line per finding id, `fixed <commit>`,
`won't fix: <reason>`, or `deferred: <where tracked>`. The next review reads
the log (rule 3), so a decision is made once. The log is also the record of
what a real finding in this repo looks like; an entry worth teaching from is
promoted by the owner into a brief's "Repo history to hold against", never
automatically.
