# Review doctrine

This directory defines the pre-merge review for Void Scavenger. It is not a
generic code review. A generic review asks "does it work, is it readable." This
review asks whether the change was made the way an experienced engineer would
make it, given the codebase as it already exists. Working code that disrespects
the existing design is a defect.

## Map

| File | Role |
|---|---|
| `doctrine.md` | This file. The five doctrines that give the review its focus. |
| `evidence.md` | The contract every reviewer's output obeys: line-linked evidence, severity, confidence, collation. |
| `ground_truth.md` | Repo-specific architecture every reviewer verifies against before judging the diff. |
| `reviewer.md` | The method shared by every reviewer: standing rules, reading order, navigation, output. Stated once. |
| `principles/*.md` | One drill-down per principle. Each is the brief for one reviewer. |
| `../reviews/` | The review log: every collated report, with the owner's outcomes. Read by the next review. |

Agents, in `.claude/agents/`: one orchestrator and six review focuses, run
in two waves. The five design lenses run first, concurrently. The
adversarial focus runs second with their reports in hand, so it hunts what
they missed before working its catalog. Then the orchestrator consolidates
per `evidence.md`: verifies each finding at its site, merges, sets aside
what the owner already decided, ranks. This table is the one declaration of
which focus reads which briefs; the agent files point at `reviewer.md` for
the method.

| Focus | Criteria (briefs under `principles/`) | Extra output |
|---|---|---|
| `mark-review` | Orchestrator. Runs the two waves, consolidates per `evidence.md`, renders the verdict. | |
| `review-strategic` | `strategic_design`, `pattern_literacy`, `encapsulation` | pattern table |
| `review-dry` | `dry`, `identity`, `atherosclerosis` | `Entanglement drag` |
| `review-monkeys-paw` | `monkeys_paw`, `rust_idioms`, `godot_idioms` | |
| `review-done` | `silent_deferrals`, `test_coverage` | coverage table |
| `review-pipeline` | `reproducibility`, `asset_pipeline` | |
| `review-adversarial` | `adversarial` (wave two, with the five reports in hand) | coverage and refuted lists |

The grouping follows the doctrines: strategic design and pattern literacy
are both about the structure the next change will need; DRY, identity, and
atherosclerosis are the same failure at three scales; the Monkey's Paw, Rust
idioms, and Godot idioms are the Zen of the Tool at three scales, from the
language through the binding to the engine; silent deferrals and
test coverage are both "asked versus delivered"; reproducibility and the
asset pipeline are the same stage graph seen from two sides.

To run: invoke the `mark-review` agent with the scope; `reviewer.md` states
the default and what the scope carries. The gates (`make check`,
`make test-assets`, `make check-visual`) are run by
the invoker before the review, one at a time, never by the reviewers, and
their results ride in the scope. The invoker files the returned report
under `docs/reviews/` and records the owner's triage there; the next review
reads it.

## Why a doctrine and not a checklist

An agent generates more code per hour than a human can review. Left to its
defaults it pays for every problem with more code: another helper, another
special case, another layer over the thing that was awkward. Each hunk is
locally fine. A hunk-by-hunk checklist therefore catches nothing. The defects
live in the aggregate, and in the relationship between the change and the
design it landed on. So this review is organized around the failure modes of
agentic coding specifically, and each reviewer reads the whole diff through one
of those lenses.

## Doctrine 1: The Monkey's Paw

A wish granted literally and wrongly. The request was satisfied; the thing that
was wanted was not delivered. Four failure modes, each with its own signature:

- **Wiring.** Well-tested components that nothing real calls. The feature
  exists in isolation; the production path never reaches it. The grab-shake
  removed in 2026-08 had never reached the eye cameras.
- **Algorithm selection.** Something hand-rolled where a battle-tested facility
  exists: inline xor-shift arithmetic beside `SmallRng`, a bespoke graph walk
  beside petgraph, manual hashing beside `Seed`.
- **The Zen of the Tool.** Every tool has design assumptions and tradeoffs.
  Code can use a tool syntactically correctly while violating the way it was
  meant to be used, and the problem surfaces later: under load, at scale,
  when the bill arrives. Here the tools are Rust's compiler and clippy,
  godot-rust, Godot's deferred boundary, rand's seeding machinery, petgraph,
  serde, make. Collision toggles from a physics callback, `Gd<T>` handles
  cached and cloned, `mut` and `unwrap` to silence the checker, build flavor
  selecting behavior: each is the tool fought instead of leaned into.
- **Premature commitment.** Code written before the problem was understood.
  The signature is workarounds layered on workarounds, and build-system or
  configuration changes compensating for a code-design gap.

Corollary: tests must exercise production code paths. A green suite over a
synthetic stand-in is the paw's signature, not evidence.

The four modes share one shape. Think of the work as frequency bands, the way
a Fourier transform decomposes an image: low frequencies are architecture,
data flow, and overall coherence; middle frequencies are integration and
component interfaces; high frequencies are implementation details and tests.
Every failure mode is high-frequency work proceeding without the low and
middle frequencies that would support it. Plan at low frequency before coding
at high frequency. The constraint was never typing speed. It was clarity of
thought.

Drill-down: `principles/monkeys_paw.md`.

## Doctrine 2: DRY means concentrated confidence

DRY does not mean "never type the same thing twice", and it is not the rule of
three. The rule of three (tolerate two copies, abstract at the third) is a
heuristic from a Java era when abstraction was expensive for a human to type
and cheap to get wrong. It is not the principle; it is a coping strategy for
the principle's cost, and that cost is gone.

The principle is this: a behavior lives in exactly one place, and that place is
tested, so every caller inherits the confidence of those tests. A second copy
is a second, untested path. The problem does not begin at the third copy. It
begins at the second, because that is where confidence splits.

The same principle cuts the other way. Two textually similar hunks that are
different concepts must not be merged; the yardstick is concept identity, not
textual similarity. A reviewer who flags every repeated three-line pattern is
as illiterate as one who tolerates a second level generator.

The repo's names for this failure are "parallel pathway" and "second truth". A
derived view standing beside a public source is a second truth; the fix
privatizes the source in the same change.

Drill-down: `principles/dry.md`. Identity spaces, the most common site of this
failure in this codebase, have their own drill-down: `principles/identity.md`.

## Doctrine 3: Atherosclerosis

The owner's framing, verbatim, is the ground truth for this check:

> I'm wondering just how the blob entanglement is affecting our current plan,
> how much we're having to do to unwrap the decisions made by expediency
> instead of by good practice (meaning maintainable, extensible code). I get
> the impression that because you can churn out a metric shitton of code
> faster than any human that maintainability isn't in your primary directives,
> because you can just keep coding to cover up the pain from previous clunky
> architectural decisions. The result is that you keep building layers upon
> layers such that the original concepts are buried under atherosclerotic
> lesions to the point that any forward progress is blocked by either going on
> a weight-loss program to dig out from under those plaques or declaring the
> patient dead and moving on to a new system.

That is the failure mode of agentic coding going off the rails: more code
instead of efficient code. Mechanical waves of near-identical hunks, layered
workarounds, per-module copies, configuration sprawl, until the original
concepts are buried and the next change requires excavation. "Just get it done,
I can see everything" is not a defense. The codebase must stay navigable by
structure, not by total recall.

The reviewer's stance: volume disproportionate to concept is itself a finding,
however green the tests. Plaque is only visible in aggregate, so this check
runs over the whole diff, never per hunk. Every review closes with an
entanglement-drag verdict: how much of the diff was unwrapping prior expedience
versus forward progress, which plaques it excavated, which it layered over.

Drill-down: `principles/atherosclerosis.md`.

## Doctrine 4: Strategic design over YAGNI

YAGNI is right about features and wrong when used as a license not to think
about structure. The tactical programmer gets the feature working. The
strategic programmer invests in the design that makes the next change cheap.
Agentic coding is tactical by default, because every task starts fresh and
"we'll clean it up later" has no later.

The line to draw: do not build features nobody asked for. Speculative
generality (a trait with one impl and no planned second, a parameter with one
value, a config knob nobody turns) is a real smell and this review flags it.
But do build the boundary, the type, the abstraction that the current change
already needs in order to be right. Expedient code makes structural decisions
by accident, implicitly, in the shape of a workaround. Strategic code makes
them on purpose, visibly, in the shape of a type.

The test for any deferred thing: is it a feature, or is it a structural
decision the current change is already making implicitly? Defer the feature.
Make the structural decision now, explicitly, and document it.

Where a future design has already been called out, build toward it as the
code develops. Every change in that area is a step along the declared path;
a step the path will later have to undo is a defect today, however well it
fits the present code. Transitional structures ("for now", "interim", a shim
until the real thing) are a code smell without exception. We do not follow
that pattern; the final shape gets built now, or designed before the
dependent code is written.

The owner's stance, which sets the bar for the whole review: merging is never
the goal. Architectural and structural issues are fixed before merge, however
long that takes, because the point is to pay down technical debt as we go
rather than to get things done as quickly as possible. No finding is
softened to make a merge convenient.

Drill-down: `principles/strategic_design.md`. Deliberate deferrals versus
silent ones: `principles/silent_deferrals.md`.

## Doctrine 5: Pattern literacy is part of correctness

The patterns are the Gang of Four's: Strategy, State, Mediator, Observer,
Command, Builder, Abstract Factory, Flyweight, and the rest of the 1994
catalog, together with the architectural patterns that grew beside it, MVC
and its descendants above all. Pattern literacy is the ability to look at a
chunk of code and see that it is an unnamed, partial, or ad-hoc rendering of
a pattern the book already names, and to say what the code would look like if
it were that pattern on purpose. When a big chunk of code can be replaced with
a pattern that makes it more extensible, the code heads toward that pattern.

This is part of correctness, not taste. Code can pass its tests and still be
wrong, because "correct" includes the cost of the next change. A hand-rolled
bag of callbacks and flags that works today and is Observer done badly costs
every future subscriber a patch to the publisher. A `match` over kinds at
twelve sites that is Strategy done badly costs the next ship twelve edits. A
state machine spread across `bool`s that is State done badly has no place to
put the transition rule. When weighing expediency against correctness, the
reviewer must be able to name the pattern the expedient code is failing to
be. "It works" is not a rebuttal to "this is Mediator with the peers talking
behind its back."

Two guards. Rust realizes the patterns differently from the book's C++ and
Smalltalk: an enum with a `match` is a legitimate closed-set Strategy, a
closure is a Command, ownership at the root replaces Singleton. Demanding
Java ceremony where a language feature already is the pattern is a Zen of
the Tool failure, and the brief carries the Rust shape of each pattern for
that reason. And a pattern needs a variation point to justify its
indirection: one use and no declared second is speculative generality, not
literacy.

This muscle has not been flexed much in this repo. A repo-wide pass on this
lens is a separate piece of work the owner has called for; the pre-merge
review applies it to the diff and, where a hunk extends an existing ad-hoc
structure, names the pattern the whole structure wants to be.

Drill-down: `principles/pattern_literacy.md`, which carries the catalog with
each pattern's Rust shape and the place it lives or wants to live in this
codebase. The repo's own established conventions (identity, entropy, Faucet,
the mediator) are a different thing: they are ground truth, catalogued with
canonical sites in `ground_truth.md`, and conformance to them is checked by
the strategic design brief.

## What done means

Done is when the behavioral tests pass. A test wired in properly that fails
means some specified aspect is not behaving as expected; a suite that passes
is the specification, met. The tests capture decisions as executable
assertions, and they surface the questions early, when the answers are cheap
to encode. Two consequences for this review:

- A claimed completion without a behavioral test that would fail on its
  reversion is not done. It is a prototype that worked, which is the most
  expensive thing a team can produce.
- A test the team has learned to ignore is test theater. A flaky test is not
  a tolerable nuisance; it is a test that is not there. Nondeterminism by
  design (procedural generation, seeded streams) is tested by asserting
  properties over the seed space, with the bounds a decision on record, and
  those property tests are reviewed periodically because a statistical test
  that always passes untouched is almost as untrustworthy as a flaky one.

The project's TDD rules follow from this: red first, on an assertion, against
production code; property tests in Rust, shell contracts in GUT; expectations
derived from the grammar, never pinned to shipped data.

For the purposes of review, this doctrine is inverted. At the point of
review nobody can tell whether TDD was followed; the red state, if it ever
existed, is gone. What can be told, by reading, is whether the tests are
complete and whether they are correct. Complete: every behavior the issue
said this change would deliver has a test. Correct: every test is
falsifiable on a production pathway, meaning that inverting its assertion
or reverting the production hunk it covers would turn it red. A test for
which that is not true is test theater whatever its name says, and a
behavior with no such test is not done. The test coverage brief is built on
those two questions.

## Conceptual integrity

One vision, held by one person, is what keeps a system from becoming a
Frankenstein. Agents arrive with no context, choose the path of least
resistance, and need constant onboarding and an explicit definition of
quality. This directory is that onboarding and that definition. The reviewer
is not a second opinion on taste; it is the check that the agent's output
respects the one vision the owner holds, and every finding is phrased so the
owner can make the call.

## How the doctrines interact

They overlap on purpose. A hand-rolled helper beside an existing abstraction is
a Monkey's Paw algorithm-selection failure, a DRY split, an atherosclerotic
layer, a tactical choice, and a pattern-literacy miss all at once. Convergence
of several reviewers on one site is the strongest signal this review produces,
and `evidence.md` says how the orchestrator merges and ranks it.

## Sources

The doctrines are the owner's, written up on his blog. Reviewers do not need
to fetch these, but a human refreshing this directory should.

- The Monkey's Paw: When Velocity Becomes the Problem.
  https://markroden.substack.com/p/the-monkeys-paw
  The four failure modes, the frequency framework, "the constraint was never
  typing speed."
- Managing Agentic Teams. https://markroden.substack.com/p/managing-agentic-teams
  Agents as opportunistic coders on the path of least resistance; constant
  onboarding; Brooks's surgical team and conceptual integrity; management as
  specification of what excellent looks like.
- The Definition of Done, parts 1 to 3.
  https://markroden.substack.com/p/the-definition-of-done-part-1
  https://markroden.substack.com/p/the-definition-of-done-part-2
  https://markroden.substack.com/p/the-definition-of-done-part-3
  Done is when the behavioral tests pass; the four testing classes and test
  theater; the specification surface and deliberate decisions about what is
  not tested.
- Labeling Debt. https://markroden.substack.com/p/labeling-debt
  Debt as a strategic choice with deferred consequences, made knowingly or
  not; the review's job is to make it knowing.
