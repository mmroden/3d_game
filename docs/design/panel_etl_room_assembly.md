# Panel ETL & Room Assembly — Plan Draft

Status: DRAFT for review, 2026-07-18. Nothing below is implemented beyond
what "Already in the tree" lists. Supersedes the one-pool/six-rotations
panel assembler design.

## The design in one paragraph

Every kit flows through one ETL pipeline: **Extract** (unpack provider
assets) → **Transform** (per-kit policy, runs offline in Blender) →
**Load** (probe census → generated catalogs → linker pools). The
Transform bakes *lego atoms*: per-role variants (floor / ceiling / wall)
of each qualifying piece, plus metric-completion preassemblies that
quantize off-module pieces onto the grid. Room assembly stays fully
programmatic and seeded at build time — per-course covering of wall runs
by a width family (trivial 1D DP, never NP-hard territory), then a
seeded, colliderless, instanced decoration layer. Cells stop being the
masonry unit and are re-derived post-hoc as the sampling grid for
population. Whole-composition prefabs are reserved for boss rooms / set
pieces (out of scope here).

Key decisions already made in discussion (Mark, 2026-07-18):

- No baked "wall bay" libraries for normal rooms — that compromises
  roguelite regeneration. Baking stops at the atom / two-or-three-piece
  preassembly level.
- Megakit (planet 1) goes through Transform as **identity** — roles are
  already authored. Untouched otherwise. ("Fully rotated megakit for
  late-level trippiness" is noted as a future option, not planned.)
- vol03 is the *cooler* kit; the goal is to use **all** of it — bases,
  machinery modules, strips, add-on greebles — via composition, layered
  onto any base (including vol01 bases). Stretching vol03 into metric
  space within an authored tolerance is acceptable; so are authored
  preassemblies (e.g. a 0.7 m piece married to a 0.3 m piece = one 1.0 m
  unit).
- The 3 m story module stands (vol01's 12 solids are all 3 m tall);
  wall-run *widths* are partition-covered by the {3,4,5,6} m family — no
  stretching needed for vol01. The cell grid remains 3 m for topology,
  connectors, and population; masonry is decoupled from it.
- Universal-filler rule: every panel kit MUST provide a 1-module base
  plate (both kits' 3×3s). The coverer can therefore always complete;
  everything else is upside. Link error otherwise.

## Census facts this plan is grounded on (probe, 2026-07-18)

- vol01: 31 pieces, ALL authored thin-along-Z (one consistent provider
  convention). 12 solids (coverage 1.0): 3×3 ×2 (d_001, f_001), 4×3 ×2
  (d_002, f_002), 5×3 ×6 (d_003, e_001, e_002, f_003, g_001, g_002),
  6×3 ×2 (e_003, g_003). The a/b/c families (coverage 0.13–0.47) are
  trusses — future interior lattice/prop material, never walls.
- vol03: solid 3×3 bases (m_001, n_001), a 5.6 m banner family
  (p/q/r/s, coverage ~1.0, heights 0.4/1.2/1.8/2.4), machinery
  mid-sizes (~1.5 × 0.6–1.0 and ~0.9–1.2 squares), small add-ons
  (l_*, 0.3–0.5 m). A composition kit, not a tiling kit.
- Solid plates carry directional, single-faced detail: grain and facing
  must be measured and baked, not assumed.

## Stage map: exists / changes / new

### Extract — `scripts/install-addons.sh`
- EXISTS: format-sniffed unpack, LFS guards, `split_panel_kit` per kit.
- CHANGES: none structural. A new kit remains "one more line + kits.toml
  entry".

### Transform — `scripts/split-panels.py` (Blender, offline)
- EXISTS: texture downscale, decimation, double-sided export, origin
  centering, near-pitch square snapping, stable naming. An uncommitted
  `pose_normalize` (single canonical pose) sits in the working tree —
  raw material: canonicalization becomes step 1 of the role bake.
- NEW:
  1. Canonical intermediate pose per piece (thin axis up, detailed face
     up — facing from area-weighted normals; all-Z-thin vol01 makes this
     reliable).
  2. **Role bake**: each qualifying solid emits `<stem>_floor`,
     `<stem>_ceiling`, `<stem>_wall` variants with orientation and
     facing baked. Wall variants leave yaw to placement (yaw about the
     face normal cannot break facing). Optional in-plane roll variants
     behind a per-kit flag (off for vol01's gravity-anchored detail; a
     candidate for vol03's greebles).
  3. **Metric-completion preassemblies**: authored recipes (TOML) that
     group off-module pieces into on-module units; authored stretch
     table for pieces conscripted onto the module (census reports the
     exact stretch % for review before any value is committed).
  4. Universal-filler presence enforced per panel kit.

### Load — probe census, generated catalogs, linker
- EXISTS: per-piece census (face, thick, axis, coverage, tris,
  textures) in `kits.generated.toml`; `wall_coverage` policy knob;
  `GeneratedPieceRaw::qualifies_as_wall` (single shared filter); pool
  derivation with leaked `PanelSet`; empty-pool and one-planet-one-grid
  link errors. Native-pose contract test (currently RED, by design).
- CHANGES / NEW:
  1. Census v2: one record per emitted VARIANT — role, facing-verified,
     grain, module span (a×b in modules), stretch applied, source stem.
  2. Linker derives **role pools** (floor / ceiling / wall / decoration
     / add-on) per kit instead of one PanelSet; universal-filler link
     rule; module-agreement rule restated (shared story module + width
     family, not "same square pitch").
  3. Decoration density/adjacency knobs enter the roster schema as
     authored per-planet policy (sparse early → encrusted late is a
     difficulty-progression tell).

### Assemble — `room_assembler` / `level_assembly` (runtime, seeded)
- EXISTS: `CellGrid` sealed-face derivation (verified 3D-correct);
  per-room kit pick from level pools; plate seating (back on face
  plane); megakit layered path (untouched).
- REPLACED: `assemble_panels_from_grid`'s one-pool-rotated-six-ways
  loop.
- NEW:
  1. **Surface extraction**: each room face → rectilinear polygon with
     connector holes (from the same sealed-face data).
  2. **Coverer**: per-course 1D partition of run lengths by the width
     family — coin-change DP, O(L), always feasible for L ≥ 3 with the
     filler rule; seeded choice among feasible partitions. Floors and
     ceilings covered per-strip the same way.
  3. **Decoration pass**: seeded satisficing scatter of modules /
     strips / add-ons onto covered surfaces; overlap with base allowed,
     colliderless by construction, grouped for MultiMesh instancing.
     Cross-kit layering allowed (vol03 machinery on vol01 masonry).
  4. **Cells post-hoc**: occupancy/population grid re-derived from the
     covered surfaces; population and props keep consuming cells
     exactly as today.

### Shell — `void-nodes`
- EXISTS: room containers, culling, staged build under the loading
  screen, capture rig knobs (`--cull`, `--ambient`, fog-off in capture).
- NEW: a decoration placement kind (no collider) in the placement
  vocabulary; MultiMesh grouping per room per greeble type; build-time
  telemetry split so load-cost claims are measured, not vibed.

### Instruments (already fixed this session, uncommitted)
- Visual containment contract: void = sentinel magenta OR near-black
  expanse ≥ 5% of frame; capture disables fog. RED today (as it should
  be — the levels ARE open).
- Capture-rig integrity: parked-body freeze, re-park after rebuild,
  byte-identical-frame self-check. One OPEN defect: nondeterministic
  stale-viewport save (self-check catches it; fix tracked separately).

## TDD sequence

Strict red-green-refactor; every RED is an assertion failure against
running production code, never a compile error (stubs first). No data
pins: tests derive expectations from the declared grammar and the
census, never from shipped TOML numbers. `make` targets only.

- **Phase 1 — schema & linker (pure Rust, no assets).**
  RED: fixture-TOML tests for census-v2 parsing, role-pool derivation,
  universal-filler link error, module-agreement rule. GREEN: schema +
  linker. The existing native-pose contract is subsumed by per-variant
  pose/facing contracts and retired with them.
- **Phase 2 — Transform v2 (Blender, gated on `make assets`).**
  RED: probe contracts over installed variants — every qualifying solid
  emits exactly the declared roles; facing verified; module spans
  integral; stretch within the authored tolerance; filler present.
  GREEN: split-panels.py v2, `make assets`. The stretch/preassembly
  table goes to Mark for review before values are committed.
- **Phase 3 — coverer (pure Rust property tests).**
  RED: for arbitrary run lengths and hole layouts — cover is exact, no
  overlap, inventory-only widths, deterministic per seed, all widths
  appear across seeds. GREEN: the DP.
- **Phase 4 — assembler v2 (property tests over real builds).**
  RED: watertightness restated as covered area == sealed area minus
  connector openings, over generated planet-2 rooms; role-correctness
  (floor variants on ±Y faces, wall variants on XZ faces); megakit
  regression suite stays green throughout. GREEN: integration in
  `spawn_list_full`.
- **Phase 5 — decoration (Rust + one pinned GUT scenario).**
  RED: decoration placements are colliderless, in-bounds, within the
  authored density knob, seed-deterministic; GUT pins ONE scenario for
  MultiMesh node-count and physics-body-count contracts (Rust anchor
  test pins the seed). GREEN: decoration pass + shell instancing.
- **Phase 6 — the payoff gate.**
  `make check-visual` green across planet-2 extremes on both void
  signatures; full-frame flythrough review (ALL frames, per standing
  practice); build-telemetry report for the load-time story.

Sequencing note: phases 1–3 are independent of each other and of the
Blender work; 4 depends on 1–3; 5 depends on 4; 6 gates the branch.

## Open items (decisions parked, not blockers)

- Stretch tolerance value and the per-piece stretch/preassembly table
  (review after Phase 2 census output; Mark signs the table).
- Decoration knob defaults per planet.
- In-plane roll variants for vol03: on or off at first landing.
- Boss-room set-piece pipeline (pre-composed; separate plan).
- Rig stale-viewport save defect (separate small fix).
- Post-hoc cell derivation beyond population (parked as "idea shelf" —
  topology/connectors stay pre-hoc).

---

# Amendment A: Catalog split & ownership — DRAFT, pending Mark's approval

Status: DRAFT 2026-07-18, nothing implemented. Mandated by review: the
kits-in-roster entanglement and the leak-to-`'static` ownership model
(both accreted by expediency) get unwound in ONE operation, landing
BEFORE Phase 3 — every later phase then deposits into the right module
with the right ownership instead of deepening what must move.

## Module boundary

- `roster/` returns to the BESTIARY: enemies, swarms, curves, model
  references by key. Nothing about kits, plates, scenes, or pitches.
- `asset_catalog/` becomes the real catalog module: the kit grammar
  (authored kit manifest + generated census + probe + link rules), role
  pools, fixtures, environments, and the enemy-MODEL map (models are
  assets; the bestiary references model KEYS — same join pattern as
  planets→kits). Owns every runtime string.
- Planets remain the JOIN POINT: a planet declares per-level enemies
  (bestiary) and kit KEYS (catalog). The linker resolves keys across the
  boundary; module dependency points one way (roster → catalog types at
  the join only).
- Disk layout (OPEN — naming is Mark's call): authored `kits.toml`,
  `kits.generated.toml`, `models.generated.toml`, environments move from
  `rosters/` to a sibling `catalog/` dir; `rosters/` keeps enemies,
  swarms, planets.

## Ownership: SceneId + Fixture (as discussed 2026-07-18)

- `SceneId(u32)`: Copy/Eq/Hash interned handle; ONLY the catalog's
  linker mints them (no public constructor, no `From<&str>`) — an id in
  hand proves link-time resolution against installed assets. The catalog
  owns `Vec<String>` of res:// paths; `catalog.path(id) -> &str` borrows;
  the shell resolves at spawn and the string dies at that call frame.
  `MeshPlacement.scene`, `RolePlate.scene`, and the model map all carry
  `SceneId`. No `Box::leak` anywhere in the linker.
- Scene provenances and their typed doors:
  - OPEN SET (censused variants, enemy models): code asks by role/key,
    never by name. Strings exist only in TOML at rest.
  - CLOSED SET (code-referenced fixtures): `enum Fixture { DoorFrame,
    JumpGate, … }` resolved `Fixture -> SceneId` at link from ONE owned
    table inside the catalog, audited by a probe contract (`make assets`
    fails if any Fixture slot doesn't resolve to an installed file).
    Today's `DOOR: &str` const dies here.
- Why index over `Arc<str>`: Copy + 4 bytes for seeded pool shuffling
  and Phase-5 instancing-group hashing, and — decisive — no
  string-to-id path exists in application code.

## What moves, what changes, what pins it

- MOVE intact: kit schema types (incl. census v2 + Phase-1 rules),
  probe, role pools, `KitDef`. The Phase-1 tests move with them
  unchanged in substance — they are the behavior pin for the move.
- CHANGE: pool/placement types swap `&'static str` → `SceneId`;
  consumers (level_assembly, room_assembler, shell spawn path) resolve
  through the catalog handle they already receive alongside GameOptions.
- NEW tests (RED first): catalog API surface (mint/resolve round-trip,
  foreign-id panic), Fixture audit contract, roster-links-catalog-by-key
  join errors (unknown kit key names planet + key).
- The one-shot migration bridges (legacy `pieces` census, `panel_pool`)
  move as-is and still die in Phases 2/4.

## Effects on Phases 2–6

Content unchanged; landing site changes. Phase 2's Transform manifest
and probe contracts live in the catalog; Phase 3's coverer consumes
catalog pools; Phase 4 threads `SceneId` placements through assembly;
Phase 5's instancing groups key on `SceneId`; Phase 6 unchanged. The
shell work in Phase 5 gets a standing constraint recorded here: it lands
as a focused subsystem, not more methods on GameManager/LevelManager.

## Open for Mark

1. `catalog/` as the disk dir name (vs `assets/`, vs staying under
   `rosters/` with only the module split)?
2. Initial `Fixture` membership (DoorFrame, JumpGate — anything else
   code-known today?).
3. Confirm models.generated.toml belongs to the catalog (recommended)
   rather than the bestiary.
