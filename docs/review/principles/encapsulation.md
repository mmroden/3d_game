# Principle: Encapsulation and hierarchy

Behavior belongs on the type that owns the concept, at the level of the
hierarchy where the concept lives, behind the narrowest surface that serves
real callers. Policy in the shell, mechanism in the model. This reviewer
checks where things were put and what they expose.

## Checks

1. **Placement.** New behavior on an existing concept is on that type or
   trait, not in a caller, not in a new free function, not distributed across
   layers. For each new function, name the type it should have been a method
   on, or confirm it is genuinely free.
2. **Level.** Intrinsic facts on the type; scheduling facts on the level;
   policy in `void-nodes`; mechanism in `void-logic`. A policy decision inside
   the model crate (a tuning constant, a "what should happen" branch) is a
   leak; a mechanism duplicated in the shell is a split.
3. **Surface.** New `pub` items: enumerate callers with
   `find_referencing_symbols`. `pub` with no external caller is surface
   added for nothing, or for a test. A `pub` field where a method-only API
   exists (`LevelGraph` is deliberately opaque) is a violation.
4. **Test-only API.** Surface added solely so a test can poke at internals
   (`#[cfg(test)] pub`, getters used only from tests, `pub(crate)` widened for
   a test module). The design-respecting alternative is a test through the
   real door, or a behavior-level assertion.
5. **Module walls.** `nodes/views/` and `nodes/ui/` do not import each other's
   types; the lint tests enforce it. New modules must be covered by the lint,
   and the reviewer confirms they are.
6. **Ownership.** Who owns the data? A struct that holds a `Gd<T>` it did not
   create, a level structure retained outside LevelManager, a copy of
   `GameOptions` that is not seeded from the broadcast. Ownership placed away
   from the lifetime that governs it is a finding.
7. **Boundary discipline.** Conversions at the Godot and save boundaries live
   in one place on the domain type. An `as` cast, a `GString` build, or a
   Variant unpack scattered at call sites is mechanism smeared across layers.

## Procedure

For each touched module, `get_symbols_overview` of it and of its parent, then
read the whole file. For each new item, decide the owning concept first, then
check the item's actual home against it. For every `pub`, enumerate callers.

## Severity guidance

Policy in the model crate or module-wall breach: BLOCKER. `pub` field on an
opaque type: BLOCKER. Test-only API: CONCERN. Misplaced free function: CONCERN,
NIT if private and local.
