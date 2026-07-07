# Stereo Convergence & Resolution Scaling (design capture — playtest 2026-07-06)

Status: **captured, not scheduled** — this is the concrete spec for the "SBS
comfort pass" chunk of the 2026-07 playtest backlog. Read
docs/architecture (SBS view geometry) before touching any of it.

## 1. Dynamic convergence on the reticle target (experiment)

Today `convergence_distance = 0.0` is a static export on ViewManager
(main.tscn). Mark wants the eyes to converge on **whatever is behind the
reticle** each frame:

- Raycast down the camera center line; converge at the hit distance.
- In a giant empty room the convergence lands on the distant wall; with an
  object mid-view it lands on that object.
- Effect: the UI plane (text, HUD) then reads as *in front of* the
  convergence surface — text "pops out of the screen" — and world objects
  nearer than convergence pop out too.
- Needs smoothing/hysteresis so convergence doesn't snap frame-to-frame
  (nausea risk — the exact failure this chunk exists to fix).
- Framed explicitly as an **experiment**: build it toggleable so mono and
  fixed-convergence remain comparable.

## 2. Resolution-aware UI scaling + logging

Symptom: mirroring to the main monitor (xReal Pros not extended) gives a
smaller resolution than the extended-monitor path, and the UI does not
scale — the inter-level (shop) menu is TALLER than the screen; text and
menus generally don't fit.

- **Log the detected monitor/window resolution** — DONE (2026-07-06):
  ViewManager logs screen/window/mode/per-eye sizes at startup, on every
  window resize, and on display-mode changes.
- UI must scale to the actual viewport. Findings from the 2026-07-06
  investigation:
  - The project configures NO stretch mode — every Control lays out in
    raw window pixels (720px blurbs, 500px bars, fixed font sizes), which
    is exactly why a smaller mirrored monitor overflows.
  - Hard constraint (house-verified, see the SBS geometry doc): Controls
    anchor their layout to the ROOT WINDOW while rendering into the
    full-window UIViewport — so the UI texture must track the window
    size; pinning the UIViewport at a design resolution breaks layout.
  - **Option A (recommended): root-window content scale** —
    `content_scale_size = 1920×1080, mode = canvas_items`. All Controls
    lay out in constant design space and the UIViewport becomes a
    constant 1920×1080. BUT the eye SubViewportContainers are canvas
    items too: ViewManager's pixel-based sizing must convert to design
    units, and per-eye render sharpness (container scale vs physical
    pixels) must be verified ON THE GLASSES — not headless-provable.
  - Option B: a resolution-derived scale factor through the one ui_style
    door (fonts/panels × window_h/1080, rebuild on resize). No engine
    interplay, but hand-rolls what the engine does natively and touches
    every UI builder — [[feedback-use-existing-structures]] says A.
- Acceptance: the shop menu fits vertically at the mirrored resolution and
  at the extended resolution; HUD corner elements stay inside the SBS
  safe-area band at both.

## Related (already in the backlog)

- "Reticle too high" — unresolved which display mode; mono geometry proven
  center-aligned by construction (camera and fire line both +0.5 in ship
  space, HUD CENTER preset, UI plane camera-axis-pinned).
- Laser beam visuals — fixed separately (owner's call: beams NEVER
  converge; each wing beam runs straight down its own line and clips on
  what it hits, else full range; damage still rides the reticle line).
