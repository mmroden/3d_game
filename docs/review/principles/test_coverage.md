# Principle: Test coverage, completeness and correctness

The doctrine says done is when the behavioral tests pass. At review time
the reviewer cannot tell whether TDD was followed; git history is at best a
weak hint about ordering and proves nothing about a red state ever having
been observed. So for review the doctrine is inverted into two questions
that CAN be answered by reading:

- **Completeness.** Does a test cover each behavior the issue, plan, or
  commit message says this change delivers?
- **Correctness.** Is each test falsifiable on a production pathway? If the
  assertion were inverted, or the production hunk reverted, would the test
  go red? A test for which the answer is no is test theater.

Both are answered without running anything. Falsifiability is established
by tracing: from the assertion back to the production symbol it constrains,
and from the production hunk forward to the assertion that would catch its
reversion.

## Completeness checks

1. **Behavior inventory.** From the plan file, the issue, the design doc, and
   every commit message in scope, list each behavior the change claims to
   deliver. One line each, in the owner's words where possible. This list is
   the coverage target; produce it before reading any test.
2. **Behavior to test.** For each inventoried behavior, name the test that
   specifies it, and the assertion in that test that would fail if the
   behavior were absent. A behavior with no such test is not done;
   cross-report to `silent_deferrals.md`.
3. **Hunk to test.** Invert the direction: for each production hunk, name the
   test that goes red if the hunk is reverted. A hunk with no such test is
   uncovered; say what the test would assert. Gameplay, determinism, save,
   and pipeline-gate hunks without one are BLOCKERs.
4. **The hard case.** For each behavior, is the test on the easy instance
   only (the demo level, the first ship, one seed, the non-empty case)?
   Name the hard case the plan implies and whether any assertion reaches it.

## Correctness checks

5. **Invert the assertion.** For each test, flip the assertion in your
   head (`assert_eq` to `assert_ne`, `true` to `false`, the bound reversed).
   Trace whether the flipped test goes red against the production code as
   written. If it stays green, the test constrains nothing; cite it.
6. **Revert the hunk.** For each test that claims to cover a production
   hunk, trace whether the test goes red with the hunk removed. A test that
   stays green under reversion is theater regardless of its name.
7. **Production pathway.** For each test, name the production symbol it
   calls and confirm with `find_symbol` that the shipped path uses that
   symbol. A test that reconstructs the behavior (its own level builder, its
   own formula, its own fake mediator) certifies a stand-in, and inverting
   its assertion proves nothing about the product.
8. **Tautology.** `assert_has_method`, `assert_not_null` alone, asserting a
   `typeof`, asserting a count without content, asserting that a function
   returns what the test passed in, snapshots with no stated invariant.
   These fail check 5 by construction.
9. **Placeholders.** `pending()`, `skip`, `xfail`, "verify manually", a test
   body that is a comment. A silent deferral wearing a test's name;
   cross-report to `silent_deferrals.md`.
10. **Data pins.** Tests that name specific roster defs, pin shipped TOML
    numbers, or assert a tuned value. Expectations derive from the declared
    grammar so tuning never breaks a test. A data pin passes check 5 for
    the wrong reason: it goes red on tuning, not on a defect. Cite the
    pinned value and the grammar rule it should derive from.
11. **Implementation pins.** Tests asserting internal sequence (call order,
    private state, a log line) rather than an observable outcome. They go
    red on refactors that preserve behavior and stay green on refactors
    that break it: the inverse of check 6.
12. **Test theater by flakiness.** A test the team has learned to ignore is
    not there. Sources: `sleep`-based waits, dependence on leftover state,
    shared `user://` between Godot runs, real-clock timing, unsorted
    iteration reaching an assertion. A retry loop or a widened tolerance
    added to make one pass is the finding, not the fix. Cite the source of
    nondeterminism and the isolation that removes it.
13. **Nondeterminism by design.** Procedural generation and seeded streams
    are tested by asserting properties over the seed space (every room
    watertight, every pool non-empty, every level links), with the bounds a
    decision on record, never by pinning one seed's output. Note a property
    with no recorded bound, and a property test nobody has revisited.
14. **Right home.** Property tests in Rust. GUT tests are shell contracts
    only: one pinned level per scenario, its seed pinned by a Rust anchor
    test, never a seed scan in-engine. A property test in GDScript or a
    shell contract in Rust is in the wrong home and usually fails check 12.
15. **Test code is code.** Duplicated fixtures, leaked `'static` data,
    hand-rolled helpers beside existing test support, stringly-typed node
    paths in tests: findings under the same rules as production.

## Procedure

Build the behavior inventory first (check 1), from the plan and commit
messages, before opening a test file. Then list the tests in scope. For
each, read it in full, map it to its production symbol, and run checks 5
through 7 by tracing. Then walk the inventory (check 2) and the production
hunks (check 3) and record the gaps. Finish with a coverage table:
behavior, test, assertion, falsifiable yes or no.

## Severity guidance

Inventoried behavior with no falsifiable test: BLOCKER. Production hunk on a
gameplay, determinism, save, or pipeline-gate path with no test that goes
red on reversion: BLOCKER; CONCERN elsewhere. A test that stays green under
assertion inversion or hunk reversion: BLOCKER if it is the only test for
that behavior, CONCERN otherwise. Stand-in certification: BLOCKER. Data pin:
CONCERN (it will break on tuning). Tautology, placeholder, flaky source:
CONCERN. Wrong home: CONCERN. Implementation pin: NIT unless it is the only
test, then CONCERN.
