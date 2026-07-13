# Stereo Rig — convergence, interaxial, focus & scaling (spec — 2026-07-09)

Status: **SPECCED, not scheduled** — the design below was agreed with the
owner 2026-07-09 and is a plan for a FUTURE chunk, not work in flight. Only
§1 (resolution scaling) is a standing prerequisite. Read
docs/architecture (SBS view geometry) before touching any of it.
Supersedes the 2026-07-06 capture (whose "dynamic convergence = nausea
risk" framing the owner corrected: continuous convergence is what good 3D
does — the failure mode is *unmotivated or discontinuous* change).

## 1. Resolution-aware UI scaling (prerequisite, unchanged)

Symptom: mirroring to the main monitor gives a smaller resolution than the
extended path and the UI does not scale — the shop menu is TALLER than the
screen. Findings (2026-07-06): the project configures NO stretch mode;
Controls lay out in raw window pixels. Hard constraint (house-verified):
Controls anchor to the ROOT WINDOW while rendering into the full-window
UIViewport, so the UI texture must track the window size.

- **Option A (recommended): root-window content scale** —
  `content_scale_size = 1920×1080, mode = canvas_items`. Constant design
  space; ViewManager's pixel math converts to design units; per-eye
  sharpness verified ON THE GLASSES.
- Resolution logging is DONE (startup/resize/mode-change).
- Acceptance: the shop fits vertically at mirrored AND extended
  resolutions; HUD corners stay inside the SBS safe-area band at both.

## 2. The operator model

Good live-action 3D runs a crew: the focus puller and the
convergence/interaxial operator adjust CONTINUOUSLY through the shot,
tracking the subject (two people carrying a bucket toward camera: focus,
convergence, and i-o all ride the walk). The game equivalent is a rig that
tracks the aim subject the same way. We hold one advantage over the film
crew: they pad the frame edges because captured footage must be reframed
to reconverge in post — we re-render every frame, so reconvergence costs
nothing and clips nothing.

Movement idiom (owner): already comfort-shaped — forward/back dominant,
no barrel rolls, rise/lower secondary, no drift. Locomotion mitigation
(vignette, roll damping, cockpit frame) is NOT load-bearing; revisit only
if on-glass sessions say so.

## 3. Encapsulation — the StereoRig

Rendering experiments live behind ONE bounded system, distinct from
gameplay (owner 2026-07-09):

- **`void-logic` (stereo.rs): the rig math, pure and unit-tested.**
  Input: a subject sample `{distance, extent, elapsed}` (extent from the
  def's declared `size` — we know subject geometry exactly). State: the
  damped follow (the operator's hand: critically-damped ease, no snap).
  Output: one solution `{convergence, interaxial, focus_hint}`.
- **ViewManager is the SOLE consumer** — it applies the solution to the
  eye cameras. Nothing else touches eye geometry.
- **Subject selection is a separate provider** (§4) feeding the rig.
- **GameOptions carries every tunable and toggle**: rig policy
  (fixed-convergence vs subject-following), damping constants, disparity
  budget, i-o range, mono fallback — all live-tunable for on-glass
  sessions, no rebuilds. Mono and fixed-convergence remain as A/B
  comparison instruments.

## 4. Subject selection (v1: deliberately boring)

The rig's quality is mostly this system's quality.

- Candidate = aim-ray hit (reuse the aim-assist candidate machinery the
  ship controller already runs for hitscan forgiveness).
- A candidate must HOLD the ray for an acquire dwell before it captures
  the rig; a captured subject persists through brief occlusion/sweep-off
  before the rig eases to the next. A fast transiter never captures — the
  rapid-traversal case dies here, not in the damping.
- Dwell units OPEN (owner: maybe physics frames rather than ms) — the rig
  takes `elapsed` as an input, so the unit is the provider's choice and a
  tunable either way.

## 5. Interaxial for isotropy (roundness)

Owner's requirement: the subject renders ISOTROPIC — depth magnification
equals lateral magnification (no cardboard, no stretch). With parallel
cameras + image-shift convergence, screen disparity is
`d(z) = k·b·(1/C − 1/z)` (b interaxial, C convergence distance, k the
projection/display scale). The roundness condition at subject distance
`z_s` closes to **b linear in z_s**, with coefficients entirely from known
quantities: render FOV, the xReal virtual screen's size and distance, and
viewer IPD. Since the subject's geometry is grammar data (def `size`),
isotropy can be verified across the object's NEAR and FAR faces, not just
its center. Exact derivation happens at build time as unit-tested rig
math; the plan bet is the formula's existence, which is standard
stereography.

## 6. The corollary — the background self-budgets (for disparity)

With the rig holding roundness (`b ∝ z_s`) and convergence at the subject
(`C = z_s`), the far-field disparity limit is
`d(∞) = k·b/C = CONSTANT` — chosen once, ≤ the fusion budget. The 40 m
wall never diverges no matter how close the bomber gets: narrowing i-o
for the subject IS the background disparity fix. One law serves subject
roundness and background budget. No separate background paradigm is
required for *disparity*.

## 7. Occlusion rivalry — MEASURE FIRST (owner 2026-07-09)

What the corollary does NOT fix: occlusion rivalry — one eye's ray clears
a near occluder's edge and sees a far emissive, the other's doesn't (the
red-light case). This is an edge/visibility difference, not a disparity
magnitude, and it is known-bad in cinema but of UNKNOWN severity here.
Do not solve it pre-emptively.

- **The halo is computable, not just observable**: the rivalry region is
  a LEFT-RIGHT-ONLY band around near-object silhouettes (never up-down),
  and with full scene geometry + the same math that yields b(z) we can
  identify exactly which pixels each eye sees exclusively.
- **Build the diagnostic first**: a pixel-isolation mode that marks the
  halo per eye and reports metrics — halo area, luminance contrast inside
  it, count of sharp emissives falling in it. Decide from data on-glass.
- Candidate mitigations ONLY if measurement warrants:
  - depth-graded emissive attenuation / distance fog — cheap, thematic;
    NOTE: verify what exists today first (the current far-field dimming
    impression may be per-room culling, not true depth fog);
  - screen-space disparity soft-clamp warp (tier 2);
  - bokeh DoF — REJECTED as the default tool (full-res post × 2 eyes,
    owner 2026-07-09: too expensive).

## 8. Order of work (future chunk — none of it this session)

1. §1 Option A content scale (prerequisite; everything after is judged
   on-glass and needs UI that fits).
2. StereoRig + subject selection + the b(z) derivation, behind GameOptions.
3. The §7 halo diagnostic.
4. On-glass tuning sessions (owner is the instrument): rig policies A/B,
   damping/budget/i-o tuning, halo severity read.
5. Mitigation only as measured.

## Related

- "Reticle too high" — unresolved which display mode; mono geometry proven
  center-aligned by construction. A reticle rendered at the rig's
  convergence depth (it IS the subject) may resolve this as a side effect
  — check during on-glass sessions.
- Laser beam visuals — fixed separately (owner's call: beams NEVER
  converge; each wing beam clips on its own line; damage rides the
  reticle line).
