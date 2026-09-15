# Principle: Strategic design over YAGNI

Doctrine 4 in `../doctrine.md`. Do not build features nobody asked for. Do
build the structure the current change already needs to be right. And where
a future design has already been called out (in `docs/architecture/`, a plan
file, a design doc, or an owner decision on record), build toward that
future as the code develops: every change in the area is a step along the
called-out path, never a step that the path will have to undo. This reviewer
reconstructs the design intent of each touched area, present and declared
future, and judges whether the change extends that design deliberately or
works around it tactically.

The owner's stance, which sets the severity bar for this brief: merging is
never the goal. Architectural and structural issues get fixed before merge,
however long that takes, because the point is to pay down technical debt as
we go, not to get things done as quickly as possible. A finding here is not
softened to make a merge convenient.

## Checks

1. **Design-intent fidelity.** For each touched area, reconstruct what the
   original design intends: read the surrounding types, the module structure,
   `docs/architecture/game_plan.md`, and `docs/architecture/prior_decisions.md`.
   State the intent in one sentence. Then state whether the change extends it
   or conflicts with it. A conflicting change needs the design changed
   deliberately and documented, not silently worked around.
   Then do the same for the declared future: find any design already called
   out for this area (an architecture doc, a plan file, a `[FUTURE]` or
   "next" section, an owner decision in `prior_decisions.md`) and state
   whether the change moves toward it, is neutral, or moves away. A change
   that the called-out design will have to undo is a finding even when it
   fits the present code perfectly; cite the future-design sentence and the
   line that contradicts it.
2. **Implicit structural decision.** Find every place the change decides
   something structural by accident: a new representation of an existing
   concept, a new boundary crossing, a new place state lives, a new
   allocation point, a new source of a policy number. Each is a decision
   that should be explicit, on a type, and named. Cite the workaround shape
   that hides it.
3. **Tactical "for now".** A structure introduced with the intent to replace
   it: a temporary field, a transitional wrapper, a shim, an adapter between
   the old shape and the new, anything described as "good enough until",
   "for now", "interim", "transitional", or "we'll clean this up". This is a
   code smell, without exception. We do not follow this pattern. Agentic work
   has no later, and a transitional structure is the plaque the
   atherosclerosis brief is hunting, caught at the moment it is laid down.
   It is always a finding, BLOCKER by default. Declaring the deferral does
   not exempt it: `silent_deferrals.md` distinguishes a deferred FEATURE
   (a decision the owner can make) from a transitional STRUCTURE (a debt
   nobody agreed to take on). The alternative is always the same: build the
   final shape now, and if the final shape is not yet known, stop and design
   it before writing the code that depends on it.
4. **Speculative generality.** The other failure: a trait with one impl and no
   planned second, a parameter with one value at every call site, a config
   knob nothing turns, an enum variant nothing constructs, a `pub` wider than
   any caller needs. Flag these too; strategic design is about the structure
   the change needs, not structure for its own sake.
5. **Right level of the hierarchy.** New behavior on an existing concept
   belongs on that type or trait at the level where the concept lives:
   intrinsic facts on the type, scheduling facts on the level, policy in the
   shell, mechanism in the model. A behavior placed in a caller, a free
   function, or smeared across layers is a tactical placement.
6. **Documentation of deliberate change.** When the change does alter a
   design on purpose, the diff must carry the rationale where the next reader
   will find it: the architecture doc, the type's doc comment, or
   `prior_decisions.md`. A deliberate change without recorded rationale reads
   as an accident to the next change.
7. **Experiments.** Code labelled an experiment (the stereo director is one)
   must be isolated so it can be wiped on the owner's word without excavation:
   its own module, its own option, its own tests, no tendrils into
   unrelated types. An experiment that has grown tendrils is a finding.

8. **House pattern conformance.** The repo's established conventions
   (`ground_truth.md`, "Canonical sites for the house patterns": identity
   newtypes, salted entropy, the Faucet Principle, the mediator, the FSM,
   grammar-not-pins, stage-is-the-door, census-at-ingestion) are the
   present design intent made concrete. For each hunk in a governed area,
   name the convention and cite its canonical site. A hunk that departs
   from it is incorrect even when it works: it creates a second way
   (`dry.md`), the next reader follows the convention and misses the
   exception, and it signals the surrounding code was not read. Where the
   departure was taken for expediency, say so; expediency is a legitimate
   owner decision only when declared.
9. **Cargo cult.** The inverse: a house convention applied where its
   concept does not apply (a newtype over a value that never crosses a
   boundary, a salt for a stream nobody consumes, dormancy machinery for a
   transient FX). Knowing when a convention does not apply is part of
   knowing it.
10. **New convention.** When the diff establishes a genuinely new house
    convention, it must be documented where the next author will find it
    (`ground_truth.md` at minimum) and must not overlap an existing one's
    concept.

## Procedure

For each touched module, read the whole file and its sibling modules'
`get_symbols_overview` before reading the diff. Write down the intent. Then
read the hunks and classify each as extend, work-around, or unrelated. For
every new type or field, `find_referencing_symbols` to test for speculative
generality.

## Severity guidance

Intent conflict worked around silently: BLOCKER. A step the called-out future
design will have to undo: BLOCKER. Tactical "for now" structure: BLOCKER.
Implicit structural decision in workaround shape: CONCERN, BLOCKER when it
creates a second representation of an existing concept. Speculative
generality: NIT unless it widens a public surface, then CONCERN. Experiment
with tendrils: CONCERN.

The bar is not lowered to get a merge. Structural findings are fixed before
merge; that is the owner's standing decision, and it is the reason this brief
exists.
