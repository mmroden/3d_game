# Principle: Silent deferrals

A declared deferral is a decision. An undeclared one is a defect. Scope
decisions (what gets narrowed, stubbed, postponed) belong to the owner, and
they can only be made if the deferral is visible. This reviewer compares what
was ASKED against what was DELIVERED and hunts the scatter that betrays a
quiet narrowing.

## Checks

1. **Asked versus delivered.** Read the plan file or design doc the branch
   claims to implement, the owner decisions recorded there, and every commit
   message. List each claimed completion. For each, find the production code
   and the test that prove it. A claim with no proof is a finding.
2. **The scatter.** Search the diff (this is textual content, grep is
   correct here) for: `for now`, `later`, `TODO`, `FIXME`, `XXX`, `HACK`,
   `will`, `pending`, `placeholder`, `[PLACEHOLDER]`, `stub`, `not yet`,
   `temporary`, `xfail`, `skip`, `pending()`, `verify manually`, `B\d+ will`.
   Each hit is either a declared deferral (cite where it is declared) or a
   finding. List every site.
3. **Fallback arms.** A `_ =>`, an `else`, an `unwrap_or`, a default branch
   that stands in for unbuilt behavior rather than handling a real case. The
   telltale is a fallback that produces a plausible result instead of an
   error.
4. **Weaker contract.** A test that pins less than the plan states: asserts
   the demo case and not the hard case, asserts presence and not content,
   asserts a count and not the elements, `xfail`s the case that matters.
   Cite the plan sentence and the test that falls short of it.
5. **Hard case unimplemented.** Features whose easy path works and whose hard
   path (the second planet, the second ship, the concurrent case, the empty
   case) does not. Name the hard case and where it dead-ends.
6. **Placeholder content.** Values, names, strings, or assets standing in for
   the real thing: `[PLACEHOLDER]` in shipped text, a seller still unnamed in
   attributions, a magic number with a "tune later" comment.
7. **Declared deferrals.** For each declared one, confirm it is tracked
   somewhere an owner will see it: the plan file, the commit message, a
   follow-up entry. A deferral declared only in a code comment is half
   declared; report it as CONCERN so the owner can promote it.

## Procedure

Start from the plan and the commit messages, not from the code. Build the
list of claims first. Then grep the diff for the scatter. Then read each
touched test against its plan sentence.

## Severity guidance

Undeclared narrowing of a claimed completion: BLOCKER. Fallback arm standing
in for unbuilt behavior: BLOCKER when on a gameplay or pipeline-gate path,
CONCERN otherwise. Scatter site without declaration: CONCERN each, and list
them all. Half-declared deferral: CONCERN. Placeholder in shipped content:
CONCERN unless the owner has stamped it (the `[PLACEHOLDER]` convention for
proper nouns is owner-stamped).
