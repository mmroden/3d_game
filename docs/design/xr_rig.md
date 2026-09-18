# XR rig — one XR-shaped scene, two interfaces

Status: **SHAPE APPROVED (owner 2026-09-17); phase 1 (the SBS interface, the XR scene, the measurement) LANDED — §7, §8; phase 3 (the OpenXR display on the Mac) LANDED — §6.** Phases 2 and 4 are not built. Replaces
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
  Godot's `OpenXRInterface` (Frame and the Meta XR Simulator, §6).
- One multiview render: one cull, one shadow pass, one submission, TAA per
  view. MSAA, TAA and the 3D render scale apply to the root viewport.
- UI is unchanged in principle: every `CanvasLayer` under Main renders into
  the one `UIViewport`. In every two-view display the `UIPlane` shows that
  texture — ViewManager's child (its visibility is the display's, never
  the ship's: GameManager hides the Player outside the flying phases, and
  a plane parented under the origin took every menu with it in SBS,
  glasses session 2026-09-17), cockpit-locked through a `RemoteTransform3D`
  anchor under the origin that pushes its pose to the plane whenever the
  ship moves. In mono the `MonoUILayer` draws it: Godot renders a
  viewport's 2D canvas at one view and skips it at two, so the split falls
  out of the engine.
- Window-mode changes (fullscreen for SBS, the mono preference) re-enter
  ViewManager on macOS before they return — the OS resize delivers the
  queued `on_window_size_changed` inside the call — so ViewManager makes
  them under gdext's `base_mut()` guard, which releases its own borrow for
  the call (the F3 "bind_mut() failed, already bound" panic, 2026-09-17).
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

- `ViewManager::capture_frame()` is the single entry point for "what the display
  shows": mono = the root render target; SBS = layers 0 and 1 stitched
  left|right. The visual harness's frame contract (an SBS frame spans the
  full window, one eye per half) is unchanged.
- Telemetry prints a final line at exit, so every stage artifact
  (`engine.log`) carries draw CPU/GPU p99 and draw calls. Before/after is
  the same `make check-visual` invocation on the commit before the rig
  change and the commit after — compared across commits, never across two
  code paths kept alive side by side.

Measurements: §7.

## 5. Phase 2 — convergence at the gaze (experiment; not built)

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

## 6. Phase 3 — the OpenXR display (LANDED 2026-09-17, on the Mac)

OpenXR is a fact of the run, not a preference. Godot enables it at process
start — `--xr-mode on` on the command line, or `xr/openxr/enabled` — and
its interface initializes then; nothing can switch it on from the menu.
So the saved preference stays the SBS toggle (the xReal's knob), and
`void_logic::stereo::Display` names the display in force:
`Sbs(Mono | SideBySide)` or `OpenXr`, decided by `Display::effective(sbs,
openxr_active)`. The XRServer is the single source of truth for
`openxr_active` (`views/openxr.rs`); the view pipeline, the HUD band and
the ship's chase gate all read it there.

- `make run-xr` launches the game in the **Meta XR Simulator** (Meta's
  Homebrew tap, Apple Silicon; `make deps-xrsim`, part of `make deps`),
  activated per run through `XR_RUNTIME_JSON` — the system-wide runtime
  symlink is never touched. The project setting stays off, so every other
  boot skips the runtime lookup; `xr/openxr/startup_alert` is off, so a
  headset run without a runtime warns and falls back to the SBS interface.
- Under the runtime: the head moves inside the cockpit (the `XRCamera3D`
  tracks it), the ship still aims by its nose, the UI rides the
  cockpit-locked plane, the HUD band is full-bleed (a headset's eye sees
  the whole plane), the chase view is not offered (`Display::chase_allowed`),
  the window is the runtime's mirror and keeps the mono window preference,
  MSAA follows `msaa_allowed` (two views), the stereo director's dials are
  not pushed (no display of ours to act on — `world_scale` is phase 2's
  knob), and `capture_frame` returns nothing (the runtime owns the frame).
- The contract `openxr_runtime_is_the_display_when_it_runs` (visual
  suite) boots a level under the simulator and reads the startup line;
  a missing runtime FAILS it (never a silent skip). First reading on the
  M2 Max: Godot on Metal, Meta XR Simulator 1.71.0, per-eye target
  1680×1760, ~28 fps under the simulator's compositor.
- The simulator publishes `XR_FB_eye_tracking_social`, not
  `XR_EXT_eye_gaze_interaction`; under it phase 2's gaze source runs on
  the ladder proxy.
- Not in this phase: the UI as an `OpenXRCompositionLayerQuad` (the quad
  stands), an OpenXR action map (the simulator's controllers are unused;
  keyboard/pad drive the ship as before), a capture path for headset
  frames.

## 6b. Phase 4 (pointer)

Frame: Linux arm64 or Android export of the gdext crate, the Frame
controller profile in the action map, renderer choice measured on device
(Forward+ stays until that measurement says otherwise).

## 6c. The glasses' own link (LANDED 2026-09-17)

The One Pro is more than a display. Its X1 chip puts two CDC network
functions on the USB-C cable and serves each with DHCP (`169.254.N.1`,
the host `.10`); over that network it speaks two TCP services, both
framed as `magic(2) + length(u32 BE) + body`
(`rust/void-devices/src/xreal/`):

- **reports**, port 52998: IMU at 1000 Hz (gyro rad/s, accel m/s²,
  temperature) and magnetometer at 400 Hz, each kind on its own 24-bit
  counter, pushed from the moment a client connects;
- **control**, port 52999: transactions (identity, firmware, the factory
  calibration JSON, EIS/"stabilizer", proximity, brightness, shade,
  buttons-enabled, and the two display knobs) plus pushed **button
  events** (bottom, top, rocker front/back; down/up).

The two display knobs are independent, and full side-by-side is both:
`display` (the EDID advertised to the host: `9` on the One Pro in 2D;
`5` = 3840×1080@60) and `input-mode` (`regular` shows the whole frame to
both eyes; `sbs` splits it). `sbs` alone on a 1920-wide EDID is half-SBS
(960 px per eye); `wide` alone is a squeezed double-wide desktop. The
native 3D button's EDID code is still unread (button, then
`make probe-xreal`).

Tools, both run from a terminal with macOS Local Network access (the
launching app's permission is what the OS checks — the Claude desktop app
lacks it, a user Terminal has it): `make probe-xreal` observes and writes
`out/metrics/xreal_probe.toml` (+ `xreal_config.json`, the factory
calibration: per-eye intrinsics fx≈2190 fy≈2216 on 1920×1080 ≈ 47°×27°,
each display's pose in the IMU frame — both ≈36° about X, 68.8 mm apart —
IMU biases with a 21-point temperature table, magnetometer alignment,
35×61 pre-distortion grids); `make xreal-ctl ARGS="<setting> <value>"`
writes one setting.

**The handoff** (`void_logic::handoff`, executed by `ViewManager`): the SBS
option stays the one truth. When it turns on and a glasses link is up
(never on a headless server), the glasses are driven on a worker thread —
the wide EDID unless they already advertise one, then `sbs` — with the
mode they showed remembered; the window waits for the re-plugged wide
screen (aspect ≥ 3) for up to 8 s and goes fullscreen on it, else where
it is. Without glasses, or if they don't answer, the panel is taken at
once as before and the options footer says so. SBS off and quit put the
glasses back (`regular`, then the remembered mode); glasses that were
already wide are left as found. The .app carries
`NSLocalNetworkUsageDescription` so macOS asks for the permission.

Not built here: head tracking from the report stream (`SbsInterface`
consuming a pose source; EIS off at that point), button events as
InputMap actions.

## 7. Measurements (phase 1, 2026-09-17)

Same stage both sides — `RUST_TEST_THREADS=1 KEEP_FRAMES=1 make check-visual`
with `FILTER=stereo_flythrough` (SBS, 5 artery poses, commanded static
geometry) and `FILTER=cockpit_console` (mono, 5 poses) — read from the
closing telemetry line in each run's `engine.log`. Level 7 seed 1,
`--ambient=1 --populace=0 --cull=0`, 1144×828 window (SBS per eye 572×828),
Godot 4.6.1 Metal / Forward+ / M2 Max. "Before" is commit 59e3ff1 (the old
two-sub-viewport rig with the telemetry closing line), "after" the rig change.

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
