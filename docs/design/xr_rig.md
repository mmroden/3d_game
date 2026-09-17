# XR rig — one XR-shaped scene, two interfaces

Status: **SHAPE APPROVED (owner 2026-09-17); chunk 1 (the SBS interface, the XR scene, the measurement) LANDED — §7, §8.** Chunks 2–4 are not built. Replaces
the hand-rolled side-by-side rig (two `SubViewport`s, two `Camera3D`s,
per-frame pose copying) recorded as an open decision in
`docs/review/ground_truth.md`. Read `docs/architecture` (SBS view geometry)
and `stereo_convergence_and_scaling.md` for the stereo vocabulary.

## 1. The model

Treat the xReal as a crippled headset: two eye ports, no head tracking, no
eye tracking, a ~51°×30° window. The OSD's "screen distance" (4 m default)
and "screen size" (147 in = 100% FOV) are the glasses' own dials; XREAL's
guide documents neither as depth. Whether "distance" moves the vergence of
identical left/right content or only the fraction of the field the screen
fills (the owner's read) is settled by one glasses session with 2D content
at two settings, not by this doc. Either way it is an input to the depth
math — as the zero-parallax anchor if it is vergence, as magnification if
it is size — and the interface carries the glasses' settings rather than
assuming them. The Valve Steam Frame is an OpenXR headset: 6DoF head, eye
gaze, ~110°. One scene serves both; the display is an `XRInterface`, chosen
by the display option.

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

Measurements: §7.

## 5. Chunk 2 — convergence at the gaze (experiment; not built)

The director splits into three pure stages: a **gaze source** (Frame: the
eye-gaze ray's hit depth; xReal: the subject ladder as proxy), a
**policy**, and the shared **easing + hysteresis**.

- xReal policy: the `(d, s)` dials as shipped. Under the fixed-anchor
  model, roundness-1 at the subject is `s = e · d / D` (viewer IPD `e`,
  the depth `D` at which identical content fuses — the optics' fixed
  anchor if the OSD distance is only size); the shipped `K_SPHERICITY =
  100` is a dial, not that law — owner's call whether the law replaces it
  or joins it, after the glasses session in §1 says what `D` is.
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

## 7. Measurements (chunk 1, 2026-09-17)

Same stage both sides — `RUST_TEST_THREADS=1 KEEP_FRAMES=1 make check-visual`
with `FILTER=stereo_flythrough` (SBS, 5 artery poses, commanded static
geometry) and `FILTER=cockpit_console` (mono, 5 poses) — read from the
closing telemetry line in each run's `engine.log`. Level 7 seed 1,
`--ambient=1 --populace=0 --cull=0`, 1144×828 window (SBS per eye 572×828),
Godot 4.6.1 Metal / Forward+ / M2 Max. "Before" is commit 59e3ff1 (the old
two-sub-viewport rig with the telemetry door), "after" the rig change.

| reading (p50 over the posed frames) | before | after |
|---|---|---|
| SBS draw CPU (ms, the viewports that render) | 0.37 | 0.25 |
| SBS draw calls per frame | 135 | 75 |
| SBS frame time (ms) | 16.6 | 16.0 |
| mono draw CPU (ms) | 0.19 | 0.22 |
| mono draw calls per frame | 135 | 76 |
| mono frame time (ms) | 16.6 | 16.6 |

Reading it: the two-view render now costs what one view cost — draw calls
halve (one multiview submission instead of two eyes), CPU submission drops
by a third and lands within noise of mono (whose measured target now also
carries the 2D canvas). Frame time is vsync-bound at 60 Hz in this stage
either way and cannot show the saving; GPU time reads 0 on this host
(Metal timestamps not reported), so it is unmeasured. The old mono drew
135 calls to the new 76 — consistent with the hidden right eye still
drawing under the old rig, unverified.

Frame parity: the SBS disparity contract (`stereo_pairs_obey_the_off_axis_
geometry`) passes unchanged on the new rig; before/after pairs at the same
pose differ only at antialiased edges (RMSE 1.8%, MSAA off in SBS — §8),
mono frames are identical to 0.05%.

## 8. Found in the spike

- **`xr/shaders/enabled = true`** (project.godot) is required: the engine
  compiles the multiview scene-shader variants only under it; without it a
  two-view render has no vertex shader ("Pre-raster shader is not provided
  for pipeline creation").
- **MSAA under two views on the Metal driver** asserts inside Metal's
  texture-view validation (a `2DMultisampleArray` sliced as `2D`, the
  per-view MSAA resolve), TAA on or off. `stereo::msaa_allowed` keeps MSAA
  single-view on Metal (TAA stays on; the log says when a requested MSAA
  is withheld); Vulkan hosts keep it in both modes. Re-test on an engine
  upgrade before widening.
- **A runtime view-count change works**: the engine re-sizes the XR
  render target from the interface every frame (`render_target_set_size`
  recreates on a view-count change), so one interface serves mono and SBS
  — the fallback (two configurations swapped as primary) was not needed.
- **The capture reads the render target's layers**
  (`RenderingServer.texture_2d_layer_get`) through
  `ViewManager::capture_frame`; the window is never read.
- **The interface is unregistered on `exit_tree`** (`use_xr` off, primary
  cleared, uninitialized, removed). The XRServer outlives scene-level
  GDExtension classes at shutdown, so an interface it still held would be
  torn down through a class that no longer exists; it also keeps a GUT
  process that boots Main dozens of times from accumulating interfaces.
- **Object picking is switched off on the root viewport** when it is
  handed to the interface: the engine disables it with a warning the first
  time a viewport renders two views, and GUT counts engine warnings as
  failures. Nothing here picks.
- **Test rigs mirror the scene's camera shape**: `test_radar_hud.gd`
  builds `Player/XROrigin3D/XRCamera3D` (the HUD projects through that
  path); with no interface registered the XR camera projects as a plain
  camera.
- **Exit-time noise**: nine "Attempt to disconnect a nonexistent
  connection from 'Main' … `Viewport::canvas_parent_mark_dirty`" errors at
  teardown, from the `UIViewport` (under ViewManager, an earlier sibling)
  being freed before the UI layers redirected into it — the same order as
  before this change, believed pre-existing (the harness nulls stderr, so
  it was never read), not verified against main. The instances leaked at
  exit are the music streams still playing at quit (AudioStreamWAV/MP3 and
  their playbacks), unrelated to the rig; the owner has flagged both as
  debt to pay (stop the music players and release the UI layers'
  `custom_viewport` on teardown).
