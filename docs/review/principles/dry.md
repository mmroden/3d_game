# Principle: DRY as concentrated confidence

Doctrine 2 in `../doctrine.md`. A behavior lives in exactly one tested place
so every caller inherits that confidence. The second copy is the defect, not
the third. Concept identity, not textual similarity, decides what is a copy.

## Checks

1. **Parallel pathway.** A second way to do something the codebase already
   does: a second level generator, a second signal route into state, a second
   loader for a catalog, a second seed derivation, a second config default.
   Cite the existing path and the new one side by side. A change that leaves
   both alive is incomplete even when the new one is better; the fix retires
   the old path in the same change.
2. **Lookalike helper.** A private convenience function whose body does what
   an existing abstraction already does, placed beside it instead of on it.
   Test-support code counts: a fixture builder re-declared across test modules
   is the same split.
3. **Second truth.** A derived view standing beside a public source, such as a
   cached count next to the collection, a computed flag next to the state it
   is computed from, a second struct mirroring a TOML record. The fix
   privatizes the source in the same change so the derived view is the only
   door. Intrinsic facts live on the type; scheduling facts live on the level.
4. **Confidence split.** For each copy, ask which one the tests cover. A copy
   that the tests do not reach is an untested production path, and that is
   the consequence to state, not "duplication".
5. **False positive: different concepts.** Two hunks that look alike but
   encode different concepts (a `Health` clamp and a `Shield` clamp with
   different floors, two TOML grammars that happen to share a shape) are not
   copies. Merging them would be a pattern-literacy failure. Do not flag
   these; if tempted, state why the concepts differ and move on.
6. **Rule-of-three residue.** Comments or commit messages saying "will
   abstract when there's a third", "duplicated for now", "copied from X" are
   findings in themselves: they record a known split and defer it without an
   owner decision.

## Procedure

For each new function or type, search for the existing concept before reading
the new body: `find_symbol` on name fragments and on the domain noun,
`get_symbols_overview` of the module it landed in and of the module it should
have landed in. Ask what existing structure should have absorbed this. Then
read the body and confirm whether it is the same concept.

For each removed path, confirm with `find_referencing_symbols` that no caller
survives. For each added path, confirm the old one is gone or cite it.

## Repo history to hold against

- The enemy identity had `EnemyKey`, `EnemyId`, `crossing_id`, and seven
  accessor doors; coalescence deleted all but the key (2026-07).
- Config consumers once held independent default literals beside
  `GameOptions`; the startup broadcast made the options the single source.
- Wall pools were authored beside the probe measurements they duplicated; they
  now derive from the probe and the authored coverage.

## Severity guidance

Parallel pathway left alive: BLOCKER. Second truth beside a public source:
BLOCKER when the source is public, CONCERN when both are private to a module.
Lookalike helper: CONCERN, NIT if it is three lines and test-only. Rule-of-
three residue: CONCERN, because it is a silent deferral.
