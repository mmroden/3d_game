# Principle: Atherosclerosis

Doctrine 3 in `../doctrine.md`. The owner's verbatim framing there is the
ground truth for this check. This reviewer runs over the WHOLE diff, in
aggregate, never per hunk. Plaque is only visible in aggregate. It also
produces the `Entanglement drag` paragraph that every collated review
carries.

## Checks

1. **Redundancy wave.** N near-identical hunks: the same three-line pattern at
   many sites, the same match arm added to every consumer, the same guard
   pasted around every call. Ask what single structural change (a helper, a
   type, a trait, a moved boundary) would have made the wave unnecessary. If
   it exists and was not taken, the wave is the finding, however green the
   tests. List every site.
2. **Helper duplication.** The same private function re-declared across
   modules instead of living once at the shared home. Test support included.
3. **Layering instead of moving.** A fix applied at every call site, or a
   wrapper around a broken thing, where moving the fix into the source would
   have fixed every site at once. Name the source.
4. **Plaque left in place.** The diff steps around a previously expedient
   structure instead of excavating it: a shim above it, a special case beside
   it, a comment apologizing for it. Name the plaque, cite its origin (git
   log or blame), and state what excavation would have looked like. Every
   such site compounds the unwrap cost of the next change.
5. **Volume disproportionate to concept.** State the concepts in the diff and
   the lines each cost. The clearer the concept, the smaller the diff should
   be. A one-concept change arriving as a thousand lines is a finding; say
   which lines are the concept and which are accretion.
6. **Sprawl in configuration space.** Make targets, agents, docs, constants,
   TOML tables, environment variables multiplying where one entry could have
   been extended. Count them before and after. (19 to 25 make targets on
   2026-07-18 was this.)
7. **Cover-up code.** Code whose purpose is to make a prior awkward decision
   tolerable rather than to remove it: adapters between two representations
   of the same thing, "compat" shims, normalizers that exist because two
   producers disagree.

## Procedure

Begin with `git diff main...HEAD --stat` and the commit list. Group files by
concept. For each concept, count added and removed lines. Then read the diff
end to end in one pass looking only for repetition and layering; do not stop
to judge individual hunks. Then, for each candidate plaque, `git log -S` or
`git blame` the surrounding structure to find when the expedient decision was
made, so the finding can say whether this diff created it, layered on it, or
excavated it.

## Required output: Entanglement drag

After the findings, a paragraph titled `Entanglement drag`: roughly how much
of this diff was unwrapping prior expedient decisions versus making forward
progress; which plaques it excavated and which it layered over; and whether
the change leaves the patient on the weight-loss program or still accreting.
If the diff touched no prior plaque, say so. That is a data point about the
codebase's health, not filler.

## Severity guidance

Redundancy wave with an available structural fix: CONCERN minimum, BLOCKER if
it entrenches a boundary later work must excavate. Plaque layered over:
CONCERN, with the excavation described so the owner can price it. Sprawl in
configuration space: CONCERN. Volume disproportionate: CONCERN, BLOCKER when
the accretion is itself a parallel pathway.
