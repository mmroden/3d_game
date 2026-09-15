# Principle: Adversarial bug hunt

This reviewer is not judging design. It is trying to break the change. It
works from a catalog of particular failures, hypothesizes each against every
hunk, and then traces the code to confirm or refute. It does not run
anything: `CONFIRMED` means the failing path was traced input to output
through the actual code, with every line cited. A hypothesis that needs
execution to settle is `PLAUSIBLE`, with the exact test or command that
would settle it.

## Method

For each hunk: pick every catalog entry that could apply, write the failing
scenario as concrete inputs or state, then trace. Trace means: find the
producer of every input with `find_referencing_symbols`, follow the value to
the line where the wrong outcome occurs, and quote that line. Stop at the
first reason the scenario is impossible and record it; a refuted hypothesis
is not reported.

## Failure catalog

**Values and arithmetic**
1. Boundaries: zero, one, empty collection, single element, maximum,
   off-by-one at each `..` and `..=`, last index, first frame, level zero.
2. Signs and ranges: negative where unsigned is assumed, subtraction that
   underflows a `u32`, angle wrap, position beyond a room bound.
3. Floats: NaN and infinity from division by a length or a delta that can
   be zero; equality compares; accumulation drift across frames; `f32`
   precision at large coordinates.
4. Units: degrees versus radians, meters versus millimeters, seconds versus
   frames versus physics ticks, normalized versus raw, per-eye versus full
   window in SBS geometry.
5. Integer casts: truncation and sign change at every `as`; `usize` to
   `i32` at the Godot boundary; `u64` seeds to `i64` Variants.

**State and lifecycle**
6. Godot ordering: `ready()` before a sibling's `ready()`; a signal fired
   before its listener connected; an initial phase set inline and clobbered.
7. Handle lifetime: a stored `Gd<T>` whose node was freed elsewhere; a
   handle cloned and touched after `queue_free`.
8. Physics callbacks: collision or monitoring toggled inside a callback;
   Jolt body state set where `set_deferred` is required or forbidden;
   contact fired twice for one event.
9. FSM: a transition `can_transition_to` does not allow; a phase entered
   without its exit having run; state left from a previous run on reset.
10. Dormancy: an entity flipped active with stale state from its last
    activation; a pool exhausted (allocation attempted during play); a
    dormant entity still receiving signals.
11. Reentrancy: a signal handler that emits the signal it handles; a
    mutation to a collection while iterating it via a callback.

**Determinism and data**
12. Seed correlation: two domains seeded from the same raw value; an
    unsalted stream; an `rand::rng()` on a gameplay path; a `HashMap`
    iteration order reaching output.
13. Grammar drift: a TOML field added to the loader but not the schema
    check or vice versa; a default silently filling a missing field; a
    roster or catalog key that parses but references nothing.
14. Save compatibility: a serialized shape changed without a version bump or
    a migration; an identity serialized in a representation the loader no
    longer accepts.
15. Options: a consumer reading a default literal instead of the broadcast;
    an option persisted but not applied until restart.

**Boundaries and I/O**
16. FFI: `GString` to `String` with non-UTF-8; a `Variant` of the wrong
    type unwrapped; a nil node path; a signal argument count mismatch.
17. Paths: spaces, `+`, unicode, or `..` in asset file names (the planet-3
    packs have them); relative paths resolved from the wrong cwd; case
    sensitivity across filesystems; symlinks.
18. Pipeline stages: a step that succeeds on an empty input set and reports
    success; a census that misses a new provider format; an audit gate that
    passes because the check it needs was never reached; make target
    ordering when a prerequisite is partially built.
19. Export: a resource referenced by path that the export preset excludes;
    an LFS pointer shipped instead of the file; a debug-only feature
    reachable in release.

**Concurrency and time**
20. Frame dependence: logic that assumes a fixed delta; a timer that
    survives a phase change; a one-shot that fires twice on a double
    transition.
21. Two Godots: any path that could run a second engine or import while one
    is active.

**Tests as attack surface**
22. A test that passes for the wrong reason: an assertion on a value the
    setup fixed, a filter that matches nothing, a pinned seed that happens
    to avoid the case.

## Output

Findings per `evidence.md`, with `consequence` written as the concrete
scenario: the inputs or state, the line where it goes wrong, the observed
wrong outcome. For each `PLAUSIBLE` finding, add a `to settle:` line naming
the test or command. Close with a `Reviewer's summary` listing which catalog
entries were applied to which modules, so coverage is visible, and which
were refuted (one line each) so nobody re-derives them.

## Severity guidance

A `CONFIRMED` wrong outcome on a gameplay, determinism, save, or pipeline
gate path: BLOCKER. `CONFIRMED` on a cosmetic or dev-only path: CONCERN.
`PLAUSIBLE` on a gameplay path: CONCERN with the settling test named.
`PLAUSIBLE` elsewhere: NIT.
