# Principle: Identity coalescence

An instance of DRY as concentrated confidence, with its own drill-down because
this class of wiring rots fastest into remapper sprawl. Lesson on record: the
2026-07 enemy-identity unification deleted `crossing_id`, `EnemyId`, and seven
accessor doors that never needed to exist. The end-state to demand: one type,
one accessor, one boundary parser, zero remappers.

Whenever the diff introduces, touches, or crosses an identifier space (a
numeric id, an array index, a string key, an enum discriminant, a
`GString`/`i32` crossing), interrogate it.

## Checks

1. **Is the id space necessary at all?** Or is it a re-representation of an
   identity the system already has? A new id minted beside an existing name
   is a second truth. Cite the existing identity.
2. **Is each transition necessary?** Every int to string to index hop is a
   translation tax and a sync hazard. The bar is the identity flowing
   unchanged end to end: authored form, typed at the boundary, everywhere
   after. Count the hops and cite each.
3. **Most user-friendly representation.** Usually the one a human authors and
   reads: the TOML key, not a counter. That form, strongly typed (newtype or
   interned handle, never a raw string internally), should BE the identity.
4. **Paired accessors.** `x_by_key` / `x_by_id` / `expect_x_by_*`. Two doors
   to one thing means two representations exist.
5. **Translation tables.** A lookup whose only job is mapping between
   representations of the same identity.
6. **Boundary ids.** An id invented so a value can cross a boundary that the
   authored identity could cross as-is.
7. **Round trips.** Resolve, extract a field, re-resolve: the caller already
   held the identity and threw it away.
8. **Human doing a machine's bookkeeping.** "Append-only, never reuse" on a
   hand-maintained id list; hand-numbered variants; ids that must be kept in
   sync between a TOML file and an enum.
9. **Interning discipline.** Interned handles are owned data with a single
   interner; a `Box::leak` to obtain `'static` is a shortcut that makes tests
   re-link unboundedly and needs the owner's sign-off.
10. **The two string sites.** Raw strings for an identity exist at exactly
    TOML serde and the Godot/save crossing. A raw string anywhere else is a
    departure; cite it.

## Procedure

For each identifier-like type or field in the diff, `find_symbol` and
`find_referencing_symbols`; draw the flow from authoring to every consumer.
Mark each conversion. Then compare against `EnemyKey` and `SceneId` as the
reference flows.

## Severity guidance

New id space beside an existing identity: BLOCKER. Paired accessors or a
translation table: BLOCKER, because they entrench the second representation.
Round trip: CONCERN. Raw string away from the two sites: CONCERN. Leaked
`'static`: CONCERN pending owner sign-off.
