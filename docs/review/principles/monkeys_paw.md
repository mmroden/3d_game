# Principle: The Monkey's Paw

Doctrine 1 in `../doctrine.md`. The wish was granted; the thing wanted was not
delivered. This reviewer reads the whole diff for the four failure modes and
for tests that certify a stand-in instead of the product.

## Checks

1. **Wiring.** For every new or changed public symbol (type, function, signal,
   make target, TOML field), enumerate its references with
   `find_referencing_symbols`. A symbol referenced only from tests, only from
   itself, or from nothing is unwired. A feature whose production entry point
   is not reachable from a `GamePhase` transition, a GameManager signal route,
   or a Makefile stage is unwired. State the path you expected to find and
   where it stops.
2. **Algorithm selection.** For every hand-written algorithm or data
   structure, name the std, petgraph, rand, serde, or godot-rust facility that
   already does it. Inline arithmetic that mimics a PRNG, manual hashing,
   bespoke graph walks, hand-parsed TOML, a re-implemented interner: each is a
   finding with the facility named.
3. **The Zen of the Tool.** A tool used syntactically correctly while its
   design assumptions are violated; the problem surfaces later, under load,
   at scale, when the bill arrives. Watch for:
   `Gd<T>` handles cached and cloned rather than checked with
   `is_instance_valid`; collision or monitoring toggles inside physics
   callbacks or signal flushes instead of via `call_deferred`; `ready()`
   ordering assumptions between siblings; `set_deferred` where Jolt requires
   otherwise; `instantiate()` on a play path. Rand used without the `Seed`
   machinery. Serde fought with manual parsing. Build flavor selecting
   behavior. Python invoked outside its make stage.
4. **Premature commitment.** Evidence that the problem was not understood
   before coding: a workaround wrapped around a workaround; a build-system or
   configuration change compensating for a code-design gap; a fallback arm
   that exists because the main arm was written before its inputs were known;
   a comment explaining why the obvious approach was abandoned without saying
   what replaced the understanding.
5. **Stand-in certification.** For every test in the diff, identify the
   production symbol it exercises. A test that builds its own version of the
   thing (a synthetic level, a fake manager, a re-derived formula) and asserts
   on that is certifying a stand-in. A green suite over stand-ins is the
   paw's signature and is a BLOCKER when the real path has no test at all.
6. **Plan before code.** Where the diff shows the same site rewritten across
   several commits in the branch, or a structure introduced then partially
   abandoned, cite the sequence. That is high-frequency coding without
   low-frequency planning.

## Procedure

Start from `git diff main...HEAD --stat`. For each touched module, take
`get_symbols_overview`, then for each new symbol run
`find_referencing_symbols`. Build the wiring map before reading any hunk.
Then read each touched file in full. Then read the tests and map each to its
production symbol.

## Repo history to hold against

- Grab-shake existed, was wired to nothing that reached the eye cameras, and
  was deleted rather than rewired (2026-08). Wiring.
- Hand-rolled xor-shift seed derivation existed beside `Seed` and `SmallRng`.
  Algorithm selection.
- Dormancy toggles from physics callbacks stranded entities half-dormant; the
  deferred boundary is the fix. Zen of the Tool.
- Box::leak to obtain `'static` was a shortcut that made tests re-link
  unboundedly. Premature commitment.

## Severity guidance

Unwired feature claimed complete in a commit message: BLOCKER. Hand-rolled
algorithm with a named facility available: CONCERN, BLOCKER if it affects
determinism or the entropy faucet. Stand-in certification with no real-path
test: BLOCKER. Premature commitment with a workaround stack: CONCERN with the
excavation described.
