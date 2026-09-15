# Principle: Reproducibility

A fresh clone plus `make deps` plus the stages must produce every artifact
the repo depends on, byte-for-byte where the tools allow, with nothing done
by hand and nothing done outside a stage. The owner's standing complaint,
verbatim on two occasions: "why do you hate reproducibility" (2026-07-13)
and, on an inline heredoc parsing a built GLB, "how on EARTH is this
reproducible?" (2026-09-06). This reviewer checks that every step in the
diff lives inside the pipeline, that every product is re-derivable, and that
nothing on disk exists that the pipeline could not have made.

## Checks

1. **Every step is inside a stage.** Every command the change relies on (a
   Blender split, a Godot import, a probe, a render, a metrics extraction, a
   fetch) is reachable from a make target, and the diff invokes it only
   through that target. A script under `scripts/` that no target calls is a
   step outside the pipeline. A doc, commit message, or comment that tells
   someone to run `python3`, `blender`, or `godot` directly is a finding.
   The Bash hook in `.claude/settings.json` denies direct python and
   Blender-python runs; treat any workaround of it as a BLOCKER.
2. **Inspections are tools.** A one-off measurement (a heredoc, a scratch
   script, a throwaway Blender probe) leaves nothing behind for the next
   person or the next run. Every fact learned about an asset or a build must
   come from a tool with stable output: `scripts/hull-metrics.py` and
   `scripts/scene-metrics.py` write TOML under `out/metrics/`; log
   histograms land in `out/metrics/log_<step>.toml`. A finding here is a
   fact in the diff (a number, a count, a shape) with no tool that produced
   it.
3. **The stage is the experiment.** A new tool or step goes INTO the make
   target first, its gate goes red first, then the stage runs and the gates
   judge the product. A "trial" on a copy under `out/` that is later
   "integrated" is where steps get dropped. Cite any trial artifact,
   parallel path, or "then integrate" language.
4. **The audit runs inside the stage.** "Then run the audit" is a smell.
   `make assets` is install, import, probe, credits, then `test-assets`;
   every stage's log is tee'd to `out/` and condensed to a histogram the
   audit holds to zero errors and warnings. A stage added without its log
   capture and histogram, or a gate that runs only when someone remembers,
   is a finding.
5. **Dependencies are local and pinned.** Tools live in `tools/`, assets in
   `assets/`, both fetched by `make deps` and `make assets-fetch`. No system
   installs, no `brew`, no global `pip`. Versions are pinned where the tool
   allows (`GLTF_TRANSFORM_VERSION` is the pattern). Third-party downloads
   are declared in `catalog/attributions.toml` with a sha256 and fetched by
   `scripts/fetch_attributed.py`. An undeclared download or an unpinned
   version is a finding.
6. **Products are re-derivable.** Any file on disk that the pipeline cannot
   reproduce from a fresh clone is a defect, even if it is correct. Generated
   catalogs (`catalog/*.generated.toml`) are written by their stage and the
   audit holds them current; a hand edit to a generated file is a BLOCKER.
   A committed artifact whose producer is not in the Makefile is a BLOCKER.
7. **Idempotence.** Running a stage twice from the same inputs yields the
   same state: no accumulation, no dependence on leftovers from a prior run,
   no `-name` where the data needs `-iname`, no timestamps or absolute paths
   in products, sorted iteration wherever order reaches output. A stage that
   only works after another stage's leftovers, or that leaves a partial run
   in a state the next run misreads, is a finding.
8. **One Godot at a time.** Headless Godot runs share `user://` and the
   `.godot` cache. Any target, script, or instruction that can start a
   second Godot (or an import) while one is running is a finding.
9. **Never install by hand.** Even a copy that byte-for-byte mirrors the
   installer is wrong: installed state is reproducible only when it came
   from `scripts/install-addons.sh` via `make assets`. Cite any manual
   placement, and any product that exists because of one.
10. **Make is the single entry point, extended not sprawled.** New targets
    extend the stage graph with declared prerequisites (`deps-*`,
    `build`); they do not duplicate a stage under a new name. Count
    targets before and after. Build flavor never selects behavior.
11. **Serena edits code; the pipeline edits products.** Code files are
    edited with symbol tools: the Edit/Write hook in `.claude/settings.json`
    refuses text edits on `.rs`, `.py`, and `.gd`, but a `sed -i` through
    Bash is not caught, so a whole-file text rewrite in the diff's shape is
    the reviewer's to notice. Products were made by stages. A product edited
    by hand to look right is the same defect as a hand install.

## Procedure

Start from the Makefile diff and the `scripts/` diff. Draw the stage graph:
which target calls which script, with which prerequisites. Then, for every
file the diff adds or changes under `out/`, `catalog/*.generated.*`,
`godot/addons/`, `docs/CREDITS.md`, find its producer in the graph. Then
read the commit messages and any docs in the diff for instructions that
bypass the graph. Text search is correct here: Makefiles, shell, and TOML
are text.

## Severity guidance

A product with no producer in the pipeline, or a hand edit to a generated
file: BLOCKER. A step invoked outside its stage, or a workaround of the
hook: BLOCKER. A trial path beside the stage: BLOCKER (it is also a parallel
pathway). Unpinned dependency, undeclared download: CONCERN. Missing log
capture or histogram for a new stage: CONCERN. Non-idempotent stage:
CONCERN, BLOCKER if it can corrupt the next run. Target sprawl: CONCERN.
