# Principle: Asset pipeline

The asset pipeline is where the most time has been spent and the most
lessons learned. This brief captures those lessons as checks so a change to
the pipeline is held to them, and a new pack gets every trick learned on
the old ones. Reproducibility of the pipeline is covered by
`reproducibility.md`; this brief is about what the pipeline does to assets
and how it judges them.

## Vocabulary

- **Oracle**: the vendor's file as shipped. The truth about what the
  artist intended.
- **Reading**: what a tool extracts from the oracle at ingestion, written
  to a generated file (`catalog/*.generated.toml`, `out/metrics/*.toml`).
- **Policy**: the hand-authored catalog (`catalog/kits.toml`,
  `catalog/attributions.toml`, `rosters/`), the smallest set of knobs a
  human declares.
- **Plan**: pure-Python policy applied to a reading, no Blender, no Godot.
- **Apply**: the mechanism that executes a plan with full authority over
  the product.
- **Gate**: a test the audit fails on. **Yardstick**: a metric the audit
  reports and never fails on.

## Checks

1. **Vendor intent is the spec.** The gates are fidelity: every declared
   material materializes, every texture reaches a material or has a named
   fate (used, waived, unreferenced), every face arrives, the importer
   reports zero errors and zero warnings, nothing escapes containment,
   credits are complete. A change that adds a gate must be one of these.
   A threshold invented by the author (alpha cutoff, playable share,
   coverage fraction, texel density, repeat count) is a yardstick: reported,
   never gated. Cite any threshold that gates and ask whose number it is.
   The owner's words: arbitrary rules "are patching over problems upstream
   in the pipeline."
2. **Upstream before downstream.** A fix that makes a wrong-arrived asset
   look acceptable (a normalizer in the consumer, a waiver in the audit, an
   alpha floor) is layered over a fidelity failure whose mechanical cause
   (a surface cap, a stale sidecar, an unmatched texture name,
   nondeterministic matching, a flattened container material) is fixable
   in the converter. Name the upstream cause the fix is covering.
3. **Oracle, plan, apply.** Extraction is one parse door per format
   (`extract-fbx-materials.py`, `extract-max-materials.py`,
   `mtl_materials.py`), type-agnostic through wrapper chains. Policy is
   pure Python (`material_plan.py`, `preview_plan.py`, `plan_apply.py`'s
   plan side) and testable without Blender. Apply has full authority over
   the sockets it owns; it overrides importer junk, never supplements it.
   A change that parses in the apply step, decides policy inside Blender,
   or lets the importer's defaults survive under the plan's values, is a
   finding.
4. **Genericize; no per-level waivers.** An exemption is derived from data
   (a census field such as in-zone share) with one threshold for every
   scene, never keyed by a level or pack name. A fix found on one pack
   lands in the shared converter path so every pack gets it. A scene that
   is legitimately different declares the difference as a catalog or
   roster fact the rule reads. Cite any name-keyed dict, skip list, or
   `if scene == ...`.
5. **Readings at ingestion, never hand transcription.** Provider metadata
   (audio tags, material tables, piece dimensions) is read by a tool at the
   start of the stage into a generated file. The hand-authored catalog
   holds only policy. The audit joins reading to policy so a new file or
   artist fails loudly with the stanza to add. A fact copied by hand from a
   file into the catalog is a second truth and a BLOCKER.
6. **Derive, don't author.** Pools, memberships, and expectations derive
   from readings times one policy knob: wall pools from the probe's
   measurements and `wall_coverage`; test expectations from the oracle, so
   a new pack gets the audit free with no pinned counts. A hand-listed
   pool, a pinned count, or an expectation that names a specific piece is
   a finding.
7. **Normalize in conversion, not in the consumer.** Provider pieces
   arrive in arbitrary poses and axes; the conversion (`split-panels.py`,
   `convert-hull.py`, `convert-environment.py`) normalizes to the native
   pose the assembler assumes. A rotation, flip, scale, or axis fix in the
   assembler, the roster, or the shell is in the wrong place. The probe
   records the thin axis; the contract that pieces arrive in native pose is
   the gate.
8. **Conservation.** Every input has a counted fate. Materials declared
   versus materialized, textures shipped versus reached, faces in versus
   out, tracks tagged versus credited. A conversion step that can drop
   something without the audit counting it is a finding.
9. **Container materials.** Layered and switch materials (RaySwitch,
   Layered, multi-sub) must be resolved to their intended leaf by the plan;
   the importer flattens them to blank grey and instantiates nothing. This
   was the white-floor bug. Any new format's containers get the same
   treatment before the first pack ships.
10. **Physics budget.** Enemy and hull meshes are decimated to a budget and
    given simplified convex hulls; render meshes never double as collision.
    A change that lets a full-resolution or concave mesh reach a body is a
    finding.
11. **Visual capture.** The capture rig disables fog, renders uniform
    instrument lighting that is never judged for look, verifies the
    viewport's active camera before saving, and every frame is looked at.
    Void is magenta or a black expanse; menus are mostly black by design
    and never gated on whole-frame black. A capture gate that reasons from
    lighting, or that samples one frame, is a finding.
12. **Credits are one truth with two renders.** `catalog/attributions.toml`
    holds team, sources with pack prefixes, downloads with pins, `ships`
    and `verified` flags. `docs/CREDITS.md` and the in-menu roll render
    from it and the audit holds the page to the render. Every git-tracked
    file under `assets/` is claimed by a prefix; every tagged artist is
    named by the claiming source. A credit line, a license, or an
    attribution written anywhere else is a second truth.
13. **Name the reading correctly.** The panel and kit reading is the
    "census"; the audio reading is "credits"; the hull and scene readings
    are "metrics". New readings get a name from the owner, not a coinage,
    and the owner's vocabulary is used in code, targets, and docs.
14. **No hand installs.** The installed state under `godot/addons/` comes
    only from `make assets` via the installer. Cite any product present
    without a producer, and cross-report to `reproducibility.md`.

## Procedure

Start from the `scripts/` and `Makefile` diff. For each changed script,
classify it: extract, plan, apply, audit, or metrics. Confirm it stays in
its class. Then read `scripts/tests/` changes: for each test, decide gate or
yardstick and whose number the threshold is. Then read `catalog/` and
`rosters/` changes for hand-transcribed readings and name-keyed exceptions.
Text search is correct for Makefiles, shell, and TOML; use Serena for the
Python and Rust.

## Repo history to hold against

- Container flattening (RaySwitch/Layered) was the white-floor bug; the fix
  was resolution in the plan, not a material patch in the apply.
- Z-thin plates placed 90 degrees off were the "venetian blind" enclosure
  bug; the fix belongs in split-panels normalization, not the assembler.
- Blanket texel-density waivers for two named scenes were replaced by a
  census-derived in-zone share with one threshold.
- Artists hand-copied from audio tags into the catalog were replaced by
  the credits reading at ingestion.
- Alpha 0.05, playable share, coverage 0.85, and repeat caps were all
  author-invented gates demoted to yardsticks (2026-09-09).

## Severity guidance

An author-invented threshold that gates: BLOCKER. A hand-transcribed
reading, a hand-listed pool, or a pinned count: BLOCKER. A name-keyed
exception: BLOCKER. Normalization in the consumer: BLOCKER. A downstream
patch over an upstream fidelity failure: CONCERN, with the upstream cause
named. A conversion step without conservation accounting: CONCERN. A
capture gate reasoning from lighting: CONCERN. Vocabulary drift: NIT.
