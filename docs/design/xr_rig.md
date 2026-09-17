# XR rig — one XR-shaped scene, two interfaces

Status: **SHAPE APPROVED (owner 2026-09-17), chunk 1 in flight.** Replaces
the hand-rolled side-by-side rig (two `SubViewport`s, two `Camera3D`s,
per-frame pose copying) recorded as an open decision in
`docs/review/ground_truth.md`. Read `docs/architecture` (SBS view geometry)
and `stereo_convergence_and_scaling.md` for the stereo vocabulary.

## 1. The model

Treat the xReal as a crippled headset: two eye ports, no head tracking, no
eye tracking, a ~51°×30° window, identical left/right content fusing at the
distance the glasses are set to (4 m default — an OSD setting, i.e. a shift
applied inside the glasses, not optics). The Valve Steam Frame is an OpenXR
headset: 6DoF head, eye gaze, ~110°. One scene serves both; the display is
an `XRInterface`, chosen by the display option.

Owner's requirements (2026-09-17): convergence on the place the person is
looking; head rotation inside the cockpit (that is why the ships have
cockpits); an efficient render. No chase view in VR — nauseating.

## 2. Scene shape

- `Player/XROrigin3D` is the eyepoint. `ShipController::apply_camera_mode`
  moves the origin (cockpit or chase; under OpenXR cockpit only). Its child
  `XRCamera3D` sits at identity: the head pose when a runtime supplies one,
  eyes-front otherwise. The camera is current by construction — there is no
  other camera.
- The root viewport has `use_xr = true` in both display modes.
  `XRServer.primary_interface` is the `SbsInterface` (xReal and mono) or
  Godot's `OpenXRInterface` (Frame, chunk 3).
- One multiview render: one cull, one shadow pass, one submission, TAA per
  view. MSAA, TAA and the 3D render scale apply to the root viewport.
- UI is unchanged in principle: every `CanvasLayer` under Main renders into
  the one `UIViewport`. In SBS the `UIPlane` shows that texture — now a
  child of the origin (cockpit-locked, no per-frame sync, rides the ship's
  interpolation like the hull). In mono the `MonoUILayer` draws it: Godot
  renders a viewport's 2D canvas at one view and skips it at two, so the
  split falls out of the engine.
- Deleted: `StereoCanvas`, the eye containers/sub-viewports/cameras, the
  parked player camera, per-frame eye-pose copying, the audio-listener
  sub-viewport workaround, per-eye MSAA/scale loops, the
  `render_viewports_changed` signal (the rendering viewport is always the
  root now, so the telemetry target never changes).

## 3. `SbsInterface` contract (`rust/void-nodes/src/nodes/views/sbs_interface.rs`)

An `XRInterfaceExtension` in Rust. Math in `void_logic::stereo`, pure and
tested; the interface only adapts it to the engine's virtuals.

| virtual | mono | SBS |
|---|---|---|
| `get_view_count` | 1 | 2 |
| `get_render_target_size` | window | window / 2 × height (per eye) |
| `get_camera_transform` | identity | identity |
| `get_transform_for_view(v, cam)` | `cam` | `cam · translate(±s/2 along X)` |
| `get_projection_for_view(v, aspect, near, far)` | perspective(fov_v) | perspective(fov_v) + off-axis term `shift / (tan(fov_v/2) · aspect)`, `shift = ±(s/2)/C` (the shipped `frustum_offsets`), `C = 0` → parallel |
| `post_draw_viewport` | layer 0 → window | layer 0 → left half, layer 1 → right half |

`fov_v` is the `XRCamera3D`'s authored `fov` (the engine ignores it under
`use_xr`; the interface is its consumer). Near/far arrive from the camera.
Dial priority is unchanged: capture overrides (`--interaxial=`,
`--convergence=`) beat the director, which beats the static exports.

The projection is one swappable function: toe-in would be a rotation in
`get_transform_for_view` instead of the off-axis term. The owner does not
accept the frustum-vs-toe-in argument; the rig's disparity contract can
measure corner vertical parallax for either, and that is where it is
settled, not here.

## 4. Capture and measurement

- `ViewManager::capture_frame()` is the one door for "what the display
  shows": mono = the root render target; SBS = layers 0 and 1 stitched
  left|right. The visual harness's frame contract (an SBS frame spans the
  full window, one eye per half) is unchanged.
- Telemetry prints a final line at exit, so every stage artifact
  (`engine.log`) carries draw CPU/GPU p99 and draw calls. Before/after is
  the same `make check-visual` invocation on the commit before the rig
  change and the commit after — compared across commits, never across two
  code paths kept alive side by side.

Measurements: see §7 once the spike lands.

## 5. Chunk 2 — convergence at the gaze (experiment; not built)

The director splits into three pure stages: a **gaze source** (Frame: the
eye-gaze ray's hit depth; xReal: the subject ladder as proxy), a
**policy**, and the shared **easing + hysteresis**.

- xReal policy: the `(d, s)` dials as shipped. Roundness-1 at the subject
  is `s = e · d / D` (viewer IPD `e`, fusion distance `D`); the shipped
  `K_SPHERICITY = 100` is a dial, not that law — owner's call whether the
  law replaces it or joins it.
- Frame policy A: `world_scale = (z_gaze / d_focal)^γ`. Dials: `γ` (0 =
  off, 1 = the full law), a cap on `w`, rate, hysteresis. Shell strategy is
  a dial: fixed (cap `w` by fusibility, ~1.5–2) or riding the origin at
  inverse scale (the nested-in-capsule guarantee is spent when `w` is
  large). A fixed `w` is a self-consistent dollhouse; only `dw/dt`
  conflicts.
- Frame policy B: relief-only — a compositor disparity remap around the
  fixation applied beyond the canopy (depth-masked), shell honest. One pass
  per eye; needs gdext's `experimental-godot-api` for `CompositorEffect`.
- Both ship behind options with the dials exposed; the headset session is
  an A/B. The simulator validates plumbing, not perception.

## 6. Chunks 3–4 (pointers)

- Meta XR Simulator (Apple Silicon) is an OpenXR runtime on the Mac:
  head in the cockpit, eye-gaze tracker on `/user/eyes_ext`, `z_gaze` by
  raycast, UI as an `OpenXRCompositionLayerQuad` with the quad as fallback.
- Frame: Linux arm64 or Android export of the gdext crate, the Frame
  controller profile in the action map, renderer choice measured on device
  (Forward+ stays until that measurement says otherwise).

## 7. Measurements

Filled in by the spike.
