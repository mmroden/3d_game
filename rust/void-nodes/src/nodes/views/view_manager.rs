use godot::prelude::*;
use godot::builtin::Signal;
use godot::classes::{
    CanvasLayer, DisplayServer, INode3D, Image, MeshInstance3D, Node, Node3D, QuadMesh,
    RemoteTransform3D, RenderingServer, StandardMaterial3D, SubViewport, TextureRect,
    XrCamera3D, XrServer,
    base_material_3d::{ShadingMode, Transparency, CullMode, Flags},
    display_server::WindowMode,
    texture_rect::StretchMode,
    sub_viewport::UpdateMode,
    viewport::Msaa,
};

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use crate::nodes::constants::{methods, nodes, signals};
use crate::nodes::live_handle::{LiveOpt, LiveRef};
use crate::nodes::ui::options_wire;
use crate::nodes::views::openxr::{active_display, openxr_interface};
use crate::nodes::views::sbs_interface::{window_pixels, SbsInterface};
use void_devices::xreal::control::Setting;
use void_devices::xreal::glasses::{Entered, Glasses, Restore};
use void_devices::xreal::session::Failure;
use void_logic::game_options::{RenderScale, WindowMode as WindowPreference};
use void_logic::handoff::{wide_screen, Handoff, Report, Step};
use void_logic::stereo::{msaa_allowed, per_eye_size, ui_plane_size, Display, DisplayMode, StereoConfig};

/// How long the exit path waits on glasses work already in flight before
/// leaving them as they are.
const EXIT_SETTLE: Duration = Duration::from_secs(5);

/// Default distance (meters) from the eyepoint to the floating UI plane in
/// SBS mode. A moderate in-scene depth: close enough (2 m) forces hard
/// convergence and reads as nausea; this sits the HUD comfortably out in
/// the scene. Comfort/readability is governed more by keeping the HUD
/// *inboard* (see the HUD's safe-area band) than by this distance —
/// disparity on a flat plane is uniform, so distance doesn't fix the
/// "off-center element, one eye reaching" blur. The quad scales with
/// distance, so on-screen size is unchanged. The material disables depth
/// test (`setup_ui_plane`) so this depth isn't occluded by walls.
const DEFAULT_UI_PLANE_DISTANCE: f32 = 4.0;

/// First-class view manager: owns the display pipeline (mono or SBS
/// stereo) — docs/design/xr_rig.md.
///
/// The 3D world renders ONCE, through the root viewport under `use_xr`,
/// by the XRServer's primary interface: the OpenXR runtime when one came
/// up at process start (a headset — `--xr-mode on`), else the
/// `SbsInterface` this node registers — one view in mono, two side by
/// side, the eye geometry and the window blits inside the interface. The
/// player's `XRCamera3D` (under `Player/XROrigin3D`, the eyepoint) is the
/// one camera: the head under a runtime, eyes front otherwise.
///
/// Lives as a direct child of Main. Listens to GameManager's
/// `options_changed` signal and reconfigures accordingly. Never duplicates
/// state that GameManager owns — receives the authoritative `sbs_enabled`
/// via signal.
///
/// UI CanvasLayers always render into a fixed-size UIViewport (set once at
/// startup, never changed). In mono a MonoUILayer composites that texture
/// fullscreen (the engine draws a viewport's 2D canvas at one view); in SBS
/// (two views, where the engine skips 2D) a quad under the XR origin — the
/// cockpit-locked UIPlane — displays the UIViewport texture, giving the UI
/// real stereo depth.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct ViewManager {
    base: Base<Node3D>,

    #[export]
    eye_separation: f32,
    #[export]
    depth_strength: f32,
    #[export]
    convergence_distance: f32,
    #[export]
    ui_plane_distance: f32,

    /// The display in force: the runtime, or the SBS interface in a mode.
    current: Display,
    /// Window size before entering SBS/fullscreen, so we can restore it.
    pre_sbs_window_size: Vector2i,
    /// Dynamic stereo (the ship's stereo director drives convergence +
    /// interaxial) — GameManager's option, received via the broadcast.
    dynamic_stereo: bool,
    /// The mono window preference (SBS always goes fullscreen) —
    /// GameManager's option, received via the broadcast.
    window_preference: WindowPreference,
    /// The director's dials as polled this frame: (convergence distance,
    /// eye separation). None in static mode — the exports above rule.
    director_focus: Option<(f32, f32)>,
    /// Capture diagnostics (`--interaxial=` / `--convergence=`): the rig
    /// commands exact stereo geometry for the disparity contracts — the
    /// `--ambient` family. Both set => they beat every runtime source.
    capture_interaxial: Option<f32>,
    capture_convergence: Option<f32>,
    /// The cockpit-locked UI quad, a child of the player's XR origin.
    ui_plane: Option<LiveRef<MeshInstance3D>>,

    // ── The side-by-side handoff to the glasses (void_logic::handoff) ──
    /// Where the window stands relative to the glasses' display re-plug.
    handoff: Handoff,
    /// The glasses found when side-by-side turned on, if any.
    glasses: Option<Glasses>,
    /// The device work in flight: the glasses entering side-by-side.
    entering: Option<Receiver<Result<Entered, Failure>>>,
    /// Side-by-side turned off while the glasses were still being driven:
    /// undo the moment they report.
    release_when_entered: bool,
    /// What to undo on the glasses when side-by-side ends.
    restore: Option<Restore>,
    /// The device work in flight: the glasses being put back.
    restoring: Option<Receiver<Result<Vec<Setting>, Failure>>>,
    /// The latest word on the glasses — the menus' footer line.
    display_status: GString,
}

#[godot_api]
impl INode3D for ViewManager {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            eye_separation: 0.065,
            depth_strength: 1.0,
            convergence_distance: 0.0,
            ui_plane_distance: DEFAULT_UI_PLANE_DISTANCE,
            current: Display::Sbs(DisplayMode::Mono),
            pre_sbs_window_size: Vector2i::new(0, 0),
            dynamic_stereo: false,
            window_preference: WindowPreference::Windowed,
            director_focus: None,
            capture_interaxial: None,
            capture_convergence: None,
            ui_plane: None,
            handoff: Handoff::Settled,
            glasses: None,
            entering: None,
            release_when_entered: false,
            restore: None,
            restoring: None,
            display_status: GString::new(),
        }
    }

    fn ready(&mut self) {
        // Capture knobs from the command line (the LevelManager pattern):
        // the rig commands exact stereo geometry for disparity contracts.
        for arg in godot::classes::Os::singleton().get_cmdline_user_args().to_vec() {
            let arg = arg.to_string();
            if let Some(v) = arg.strip_prefix("--interaxial=") {
                if let Ok(s) = v.parse::<f32>() {
                    self.capture_interaxial = Some(s);
                }
            } else if let Some(v) = arg.strip_prefix("--convergence=") {
                if let Ok(d) = v.parse::<f32>() {
                    self.capture_convergence = Some(d);
                }
            }
        }
        self.setup_interface();
        self.setup_ui();
        self.set_ui_viewport_once();
        self.connect_to_game_manager();
        self.connect_to_window_resize();
        self.push_stereo();
        godot_print!("ViewManager ready — {}", self.current.label());
        self.log_display_geometry("startup");
    }

    fn exit_tree(&mut self) {
        self.settle_glasses_at_exit();
        self.teardown_interface();
    }

    fn process(&mut self, delta: f64) {
        self.poll_glasses(delta);
        // Dynamic stereo: poll the ship's stereo director for this frame's
        // dials before the interface renders. Static mode leaves None, so
        // the exported tuning values rule (the A/B baseline).
        self.director_focus = if self.dynamic_stereo {
            self.base()
                .get_parent()
                .and_then(|p| {
                    p.try_get_node_as::<crate::nodes::ship_controller::ShipController>(
                        nodes::PLAYER,
                    )
                })
                .map(|player| {
                    let focus = player.bind().stereo_focus();
                    (focus.x, focus.y)
                })
        } else {
            None
        };
        self.push_stereo();
    }
}

#[godot_api]
impl ViewManager {
    /// Called when the window resizes (fullscreen transition, manual resize,
    /// etc.). The 3D render target follows the window by construction (the
    /// engine polls the interface's size every frame); only the UI texture
    /// and its plane are sized here, in BOTH modes.
    #[func]
    pub fn on_window_size_changed(&mut self) {
        self.resize_ui();
        self.log_display_geometry("window resized");
    }

    /// Log the physical display geometry (playtest 2026-07-06: mirrored
    /// vs extended xReal monitors run very different resolutions and the
    /// UI misfits silently — the log names the setup a report came from),
    /// and the GPU's texture-side limit (owner 2026-09-06: the scene
    /// texture cap claims to sit under it — the number that proves it is
    /// the device's own, printed where every run can read it).
    fn log_display_geometry(&self, context: &str) {
        let ds = DisplayServer::singleton();
        let screen = ds.screen_get_size();
        let window = ds.window_get_size();
        // macOS reports PIXELS while the OS Displays widget reports POINTS
        // (playtest 2026-07-09: "4112x2658" read as impossible — it is the
        // 2x Retina backing store of 2056x1329 pt, downsampled to the
        // panel). Print the scale so the log decodes itself.
        let scale = ds.screen_get_scale();
        // Per-eye target: ours from the window in SBS/mono; the runtime's
        // own under OpenXR.
        let [eye_w, eye_h] = match self.current {
            Display::Sbs(mode) => per_eye_size(mode, window_pixels()),
            Display::OpenXr => openxr_interface()
                .map(|iface| {
                    let size = iface.get_render_target_size();
                    [size.x as u32, size.y as u32]
                })
                .unwrap_or([0, 0]),
        };
        let texture_side = RenderingServer::singleton()
            .get_rendering_device()
            .map(|device| {
                device
                    .limit_get(godot::classes::rendering_device::Limit::MAX_TEXTURE_SIZE_2D)
                    .to_string()
            })
            .unwrap_or_else(|| "n/a (no rendering device)".to_string());
        godot_print!(
            "Display [{context}]: screen {}x{} px (scale {:.1} = {:.0}x{:.0} pt), \
             window {}x{} ({:?}), mode {}, per-eye {}x{}, GPU max texture side {}",
            screen.x,
            screen.y,
            scale,
            screen.x as f32 / scale,
            screen.y as f32 / scale,
            window.x,
            window.y,
            ds.window_get_mode(),
            self.current.label(),
            eye_w,
            eye_h,
            texture_side,
        );
    }

    /// The latest word on the glasses (the menus' footer line).
    #[signal]
    fn display_status_changed(status: GString);

    /// The latest word on the glasses, for a menu that opens after it
    /// was said.
    #[func]
    pub fn display_status(&self) -> GString {
        self.display_status.clone()
    }

    /// Called when GameManager emits options_changed.
    #[func]
    pub fn on_options_changed(&mut self, options: options_wire::OptionsDictionary) {
        let options = options_wire::from_dictionary(&options);
        self.dynamic_stereo = options.dynamic_stereo;
        self.window_preference = options.window_mode;
        // The SBS preference chooses between the SBS interface's modes; a
        // running OpenXR runtime is the display regardless.
        let target = active_display(options.sbs_enabled);

        if target != self.current {
            // The display changes first: the window-mode change below
            // re-enters this node (the OS resize reaches
            // on_window_size_changed before it returns), and that reading
            // must see the new display.
            self.current = target;
            // Play-path SBS goes fullscreen (the glasses want the whole
            // panel) — by way of the handoff when glasses are there to
            // drive, at once when not; a CAPTURE run with commanded stereo
            // geometry keeps the commanded --resolution instead —
            // disparity contracts need pixel-deterministic frames, not the
            // host's display size (a fullscreen capture ballooned to the
            // physical screen, 2026-08-20).
            if self.capture_interaxial.is_none() {
                if target.wants_fullscreen() {
                    self.begin_handoff();
                } else {
                    self.handoff = Handoff::Settled;
                    self.resize_window(false);
                    self.release_glasses();
                }
            }
            // The interface flips its view count on the same frame as the
            // toggle; the engine re-sizes the render target from it.
            self.push_stereo();
            // Rebuild the UI texture and plane to the new geometry on the
            // same frame, rather than waiting on the OS resize event.
            self.resize_ui();
            self.apply_visibility(target.ui_on_plane());
            self.log_display_geometry("display mode change");
            godot_print!("Display: {}", target.label());
        }

        // ViewManager owns all viewport anti-aliasing: apply MSAA to the
        // viewport that renders the world. Runs on every options change —
        // a pure MSAA toggle and a post-mode-switch re-apply both end up
        // correct.
        self.apply_msaa(options.msaa_enabled);
        // The 3D render scale rides every broadcast too; the window
        // preference applies where the display doesn't own the window
        // (mono, and the OpenXR mirror), and never on a capture run with
        // a commanded resolution.
        self.apply_render_scale(options.render_scale);
        if !self.current.wants_fullscreen() && self.capture_interaxial.is_none() {
            self.apply_window_preference();
        }
    }
}

impl ViewManager {
    /// The viewport RIDs rendering the 3D world: the root viewport, in
    /// both modes — one multiview pass through the display interface. The
    /// single source of truth for "what is being drawn", since ViewManager
    /// owns the display pipeline. Called by typed Rust collaborators
    /// (LevelManager) — never over a Godot string boundary.
    pub(crate) fn active_viewport_rids(&self) -> Array<Rid> {
        let mut rids = Array::new();
        match self.base().get_viewport() {
            Some(vp) => rids.push(vp.get_viewport_rid()),
            // The root always exists once we are in the tree; a miss is a
            // structural invariant break — make it loud so telemetry
            // can't silently under-measure.
            None => godot_warn!("ViewManager: no root viewport; render telemetry under-measures"),
        }
        rids
    }

    /// The frame the display shows, as an image — the capture rig's single
    /// entry point. Mono is the root render target; side by side it is the two
    /// multiview layers stitched left|right, exactly what the window
    /// shows (the harness's SBS frame spans the full window, one eye per
    /// half). `None` when nothing has been drawn (headless), and under
    /// OpenXR, where the runtime owns the frame (its swapchain overrides
    /// the render target; the window is only a mirror).
    pub(crate) fn capture_frame(&self) -> Option<Gd<Image>> {
        let viewport = self.base().get_viewport()?;
        let texture = viewport.get_texture()?;
        match self.current {
            Display::OpenXr => {
                godot_warn!("capture under OpenXR: the runtime owns the frame; nothing saved");
                None
            }
            Display::Sbs(DisplayMode::Mono) => texture.get_image(),
            Display::Sbs(DisplayMode::SideBySide) => {
                let rs = RenderingServer::singleton();
                let rid = texture.get_rid();
                let left = rs.texture_2d_layer_get(rid, 0)?;
                let right = rs.texture_2d_layer_get(rid, 1)?;
                let (w, h) = (left.get_width(), left.get_height());
                let mut pair = Image::create_empty(w * 2, h, false, left.get_format())?;
                let eye = Rect2i::new(Vector2i::ZERO, Vector2i::new(w, h));
                pair.blit_rect(&left, eye, Vector2i::ZERO);
                pair.blit_rect(&right, eye, Vector2i::new(w, 0));
                Some(pair)
            }
        }
    }

    /// Hand the root viewport to the display interface: the OpenXR
    /// runtime when one initialized at process start (the XRServer already
    /// holds it primary), else the SBS display interface this node
    /// registers as primary. From here on the 3D world renders through
    /// `use_xr`, one pass, one or two views.
    fn setup_interface(&mut self) {
        let mut xr = XrServer::singleton();
        if let Some(runtime) = openxr_interface() {
            xr.set_primary_interface(&runtime);
            self.current = Display::OpenXr;
            godot_print!(
                "ViewManager: OpenXR runtime up ({}); the headset is the display",
                runtime.get_system_info()
            );
        } else {
            let mut interface = SbsInterface::new_gd();
            xr.add_interface(&interface);
            if !interface.initialize() {
                godot_error!("ViewManager: the SBS display interface failed to initialize");
                return;
            }
            xr.set_primary_interface(&interface);
        }
        match self.base().get_viewport() {
            Some(mut viewport) => {
                viewport.set_use_xr(true);
                // Mouse picking of physics objects is a one-window-ray
                // notion the engine switches off (with a warning) the
                // moment a viewport renders two views; nothing here picks,
                // so it is off by decision rather than by warning.
                viewport.set_physics_object_picking(false);
            }
            None => godot_error!("ViewManager: no root viewport to render through"),
        }
    }

    /// Hand the viewport back and unregister the interface while this
    /// node leaves the tree — before the extension unloads. The XRServer
    /// outlives scene-level GDExtension classes at shutdown, and an
    /// interface it still held would be uninitialized through a class
    /// that no longer exists; it also keeps a test process that boots
    /// Main many times from accumulating interfaces.
    fn teardown_interface(&mut self) {
        if let Some(mut viewport) = self.base().get_viewport() {
            viewport.set_use_xr(false);
        }
        let Some(mut interface) = self.sbs_interface() else {
            return;
        };
        let mut xr = XrServer::singleton();
        xr.set_primary_interface(Gd::null_arg());
        interface.uninitialize();
        xr.remove_interface(&interface);
    }

    /// The primary interface, when it is ours. The XRServer owns the
    /// instance; this node never caches a handle to it.
    fn sbs_interface(&self) -> Option<Gd<SbsInterface>> {
        XrServer::singleton()
            .get_primary_interface()
            .and_then(|iface| iface.try_cast::<SbsInterface>().ok())
    }

    /// Push this frame's display mode, stereo dials and camera FOV into
    /// the SBS interface — the one place that render learns them. Under a
    /// runtime the dials have no display to act on (the runtime owns eye
    /// geometry; `world_scale` is the one knob there, chunk 2's business).
    fn push_stereo(&self) {
        let Display::Sbs(mode) = self.current else {
            return;
        };
        let dials = self.stereo_dials(mode);
        let fov = self.camera_fov();
        if let Some(mut iface) = self.sbs_interface() {
            iface.bind_mut().configure(mode, dials, fov);
        }
    }

    /// The vertical field of view the player's camera is authored with —
    /// inert to the engine under `use_xr`, consumed by the interface.
    fn camera_fov(&self) -> f32 {
        self.base()
            .get_parent()
            .and_then(|main| main.try_get_node_as::<XrCamera3D>(nodes::PLAYER_CAMERA))
            .map(|camera| camera.get_fov())
            .unwrap_or(75.0)
    }

    /// The stereo dials in force this frame. Dial priority: capture
    /// diagnostics (the rig commanding exact geometry) beat the director,
    /// which beats the static exports. In both override modes the
    /// director/rig owns the dials absolutely — depth_strength is the
    /// static mode's volume knob and must not double-scale a computed
    /// baseline. The per-eye dimensions are the interface's business
    /// (it re-reads the window every frame).
    fn stereo_dials(&self, mode: DisplayMode) -> StereoConfig {
        let (eye_separation, depth_strength, convergence_distance) =
            match (self.capture_interaxial, self.capture_convergence) {
                (Some(interaxial), Some(convergence)) => (interaxial, 1.0, convergence),
                _ => match self.director_focus {
                    Some((convergence, interaxial)) => (interaxial, 1.0, convergence),
                    None => (
                        self.eye_separation,
                        self.depth_strength,
                        self.convergence_distance,
                    ),
                },
            };
        let [viewport_width, viewport_height] = per_eye_size(mode, window_pixels());
        StereoConfig {
            eye_separation,
            depth_strength,
            convergence_distance,
            viewport_width,
            viewport_height,
        }
    }

    /// ViewManager is a direct child of Main; GameManager is a sibling.
    fn connect_to_game_manager(&mut self) {
        let Some(main_scene) = self.base().get_parent() else {
            godot_warn!("ViewManager: could not find Main scene");
            return;
        };
        if let Some(game_mgr) = main_scene.try_get_node_as::<Node>(nodes::GAME_MANAGER) {
            let callable = self.base().callable(methods::ON_OPTIONS_CHANGED);
            if !game_mgr.is_connected(signals::OPTIONS_CHANGED, &callable) {
                let mut gm = game_mgr;
                gm.connect(signals::OPTIONS_CHANGED, &callable);
            }
        } else {
            godot_warn!("ViewManager: GameManager not found");
        }
    }

    /// Connect to the root viewport's size_changed signal so the UI texture
    /// and plane follow the window (fullscreen, drag, etc.). Uses
    /// CONNECT_DEFERRED to avoid re-entrant borrow panics — the callback
    /// runs next frame, not during the signal emission that triggered the
    /// resize.
    fn connect_to_window_resize(&mut self) {
        let Some(viewport) = self.base().get_viewport() else {
            godot_warn!("ViewManager: no viewport for size_changed signal");
            return;
        };
        let callable = self.base().callable(methods::ON_WINDOW_SIZE_CHANGED);
        let signal = Signal::from_object_signal(&viewport, signals::SIZE_CHANGED);
        if !signal.is_connected(&callable) {
            signal.connect_flags(
                &callable,
                godot::classes::object::ConnectFlags::DEFERRED,
            );
        }
    }

    /// Set custom_viewport on all UI CanvasLayers to point at UIViewport.
    /// Called once at startup — never changed again at runtime.
    /// STRUCTURAL, not a name list: every CanvasLayer child of Main is a UI
    /// layer and renders through the one UIViewport, so any new screen is
    /// SBS-correct by construction (a hand-maintained list went stale the
    /// moment LoadingUI arrived and put the veil in one eye).
    fn set_ui_viewport_once(&self) {
        let Some(main_scene) = self.base().get_parent() else {
            return;
        };
        let Some(ui_vp) = self.base().try_get_node_as::<SubViewport>(nodes::UI_VIEWPORT) else {
            godot_warn!("ViewManager: UIViewport not found");
            return;
        };
        for child in main_scene.get_children().iter_shared() {
            if let Ok(mut canvas) = child.try_cast::<CanvasLayer>() {
                canvas.set_custom_viewport(&ui_vp);
            }
        }
    }

    /// The UI pipeline: the one UIViewport every UI layer renders into,
    /// the fullscreen MonoUILayer that shows it in mono, and the
    /// cockpit-locked UIPlane that shows it in SBS.
    fn setup_ui(&mut self) {
        let [ui_w, ui_h] = self.ui_texture_size();

        let mut ui_viewport = SubViewport::new_alloc();
        ui_viewport.set_name(nodes::UI_VIEWPORT);
        ui_viewport.set_size(Vector2i::new(ui_w as i32, ui_h as i32));
        ui_viewport.set_transparent_background(true);
        ui_viewport.set_update_mode(UpdateMode::ALWAYS);
        self.base_mut().add_child(&ui_viewport);

        // MonoUILayer — shows the UIViewport texture in mono (fullscreen
        // overlay on the root canvas, which the engine draws at one view).
        let mut mono_ui_layer = CanvasLayer::new_alloc();
        mono_ui_layer.set_name(nodes::MONO_UI_LAYER);
        let mut mono_ui_rect = TextureRect::new_alloc();
        mono_ui_rect.set_name("MonoUIRect");
        mono_ui_rect.set_anchors_preset(godot::classes::control::LayoutPreset::FULL_RECT);
        mono_ui_rect.set_stretch_mode(StretchMode::SCALE);
        mono_ui_rect.set_mouse_filter(godot::classes::control::MouseFilter::IGNORE);
        mono_ui_layer.add_child(&mono_ui_rect);
        self.base_mut().add_child(&mono_ui_layer);

        let ui_texture = ui_viewport.get_texture()
            .expect("UIViewport must have a texture after creation");
        mono_ui_rect.set_texture(&ui_texture);

        // --- 3D UI plane: cockpit-locked quad for SBS stereo depth ---
        self.setup_ui_plane(ui_texture);

        // Mono by default: MonoUILayer shows the UI, the plane hides.
        self.apply_visibility(false);
    }

    /// Create the UIPlane: a QuadMesh textured with the UIViewport, sitting
    /// `ui_plane_distance` straight ahead of the player's XR origin. In
    /// every two-view display both eyes render it with natural parallax,
    /// giving the UI real depth. The plane is ViewManager's child — its
    /// visibility is the display's — and it is cockpit-locked through the
    /// engine's `RemoteTransform3D`: an anchor under the origin pushes its
    /// global pose to the plane whenever the ship moves, so the plane rides
    /// the eyepoint without living in the ship's subtree (GameManager hides
    /// the Player outside the flying phases; a plane parented there took
    /// every menu with it in SBS, glasses session 2026-09-17).
    fn setup_ui_plane(&mut self, ui_texture: Gd<godot::classes::ViewportTexture>) {
        let (fov, aspect) = self.camera_fov_and_aspect();
        let [quad_w, quad_h] = ui_plane_size(self.ui_plane_distance, fov, aspect);

        let mut quad_mesh = QuadMesh::new_gd();
        quad_mesh.set_size(Vector2::new(quad_w, quad_h));

        let mut material = StandardMaterial3D::new_gd();
        material.set_shading_mode(ShadingMode::UNSHADED);
        material.set_transparency(Transparency::ALPHA);
        material.set_cull_mode(CullMode::DISABLED);
        // The plane floats deep in the scene for stereo comfort; without this it
        // would be occluded by any nearer wall. Draw it on top regardless — its
        // stereo depth still comes from its 3D distance, so the HUD reads deep
        // and stays visible.
        material.set_flag(Flags::DISABLE_DEPTH_TEST, true);
        // Upcast ViewportTexture → Texture2D for set_texture
        let texture_2d: Gd<godot::classes::Texture2D> = ui_texture.upcast();
        material.set_texture(
            godot::classes::base_material_3d::TextureParam::ALBEDO,
            &texture_2d,
        );
        quad_mesh.set_material(&material);

        let mut ui_plane = MeshInstance3D::new_alloc();
        ui_plane.set_name(nodes::UI_PLANE);
        ui_plane.set_mesh(&quad_mesh);
        ui_plane.set_visible(false);
        self.base_mut().add_child(&ui_plane);
        self.ui_plane = Some(LiveRef::new(&ui_plane));

        // The anchor: the eyepoint's child, ui_plane_distance ahead, driving
        // the plane's global pose (position, rotation and scale) every time
        // the ship's transform changes.
        let Some(mut origin) = self
            .base()
            .get_parent()
            .and_then(|main| main.try_get_node_as::<Node3D>(nodes::PLAYER_ORIGIN))
        else {
            godot_error!("ViewManager: no player XR origin to anchor the UI plane");
            return;
        };
        let mut anchor = RemoteTransform3D::new_alloc();
        anchor.set_name(nodes::UI_PLANE_ANCHOR);
        anchor.set_transform(Transform3D::new(
            Basis::IDENTITY,
            Vector3::new(0.0, 0.0, -self.ui_plane_distance),
        ));
        origin.add_child(&anchor);
        anchor.set_remote_node(&ui_plane.get_path());
    }

    /// FOV from the player camera, and the aspect of the UI plane. The plane
    /// shows the UIViewport texture, which spans the FULL window, so its aspect
    /// must be the full-window aspect (not per-eye) — otherwise a wide texture
    /// is crushed onto a narrow quad and the UI reads squished.
    fn camera_fov_and_aspect(&self) -> (f32, f32) {
        let fov = self.camera_fov();
        let [win_w, win_h] = window_pixels();
        let aspect = if win_h > 0 {
            win_w as f32 / win_h as f32
        } else {
            16.0 / 9.0
        };
        (fov, aspect)
    }

    /// The UI texture spans the FULL window in both modes. The UI
    /// CanvasLayers anchor their controls to the root window
    /// (custom_viewport redirects rendering, not layout), so a centered
    /// panel sits at window-center. If the UIViewport were only per-eye
    /// wide, window-center would land at its right edge — which is exactly
    /// the "menus shoved right in SBS" bug.
    fn ui_texture_size(&self) -> [u32; 2] {
        window_pixels()
    }

    /// Size the UI texture and the plane quad to the current window.
    fn resize_ui(&self) {
        let [ui_w, ui_h] = self.ui_texture_size();
        if let Some(mut vp) = self.base().try_get_node_as::<SubViewport>(nodes::UI_VIEWPORT) {
            vp.set_size(Vector2i::new(ui_w as i32, ui_h as i32));
        }
        let (fov, aspect) = self.camera_fov_and_aspect();
        let [quad_w, quad_h] = ui_plane_size(self.ui_plane_distance, fov, aspect);
        self.ui_plane.with(|plane| {
            if let Some(mesh) = plane.get_mesh() {
                if let Ok(mut quad) = mesh.try_cast::<QuadMesh>() {
                    quad.set_size(Vector2::new(quad_w, quad_h));
                }
            }
        });
    }

    /// UI: in mono the HUD is drawn by the fullscreen MonoUILayer (a proper
    /// CanvasLayer at 1:1, so tiny center chrome survives); in every
    /// two-view display the cockpit-locked UIPlane takes over, so the flat
    /// layer hides.
    fn apply_visibility(&mut self, on_plane: bool) {
        if let Some(mut mono) = self.base().try_get_node_as::<CanvasLayer>(nodes::MONO_UI_LAYER) {
            mono.set_visible(!on_plane);
        }
        self.ui_plane.with(|plane| plane.set_visible(on_plane));
    }

    /// Apply the MSAA option to the viewport that renders the 3D world —
    /// the root, in both modes. ViewManager is the sole owner of viewport
    /// anti-aliasing (TAA rides the project setting, always on). Where the
    /// engine's driver cannot multisample a multiview render (`msaa_allowed`),
    /// the option stays recorded but the render goes without, and the log
    /// says so.
    fn apply_msaa(&mut self, enabled: bool) {
        let driver = RenderingServer::singleton()
            .get_current_rendering_driver_name()
            .to_string();
        let allowed = msaa_allowed(self.current, &driver);
        if enabled && !allowed {
            godot_print!(
                "MSAA requested but the {driver} driver cannot multisample a {} render; rendering without it (TAA stays on)",
                self.current.label()
            );
        }
        let msaa = if enabled && allowed { Msaa::MSAA_4X } else { Msaa::DISABLED };
        if let Some(mut vp) = self.base().get_viewport() {
            vp.set_msaa_3d(msaa);
        }
    }

    /// Take or give back the whole panel. A window-mode change on macOS
    /// re-enters this node before it returns: the OS resize arrives
    /// synchronously and the queued `on_window_size_changed` is delivered
    /// inside the call (F3 in glasses, 2026-09-17: "bind_mut() failed,
    /// already bound"). The `base_mut()` guard is gdext's rule for that —
    /// it makes this borrow inaccessible for the call so the re-entrant
    /// method may bind — hence every DisplayServer call here is made under
    /// it, with the fields it needs read first.
    fn resize_window(&mut self, fullscreen: bool) {
        let mut ds = DisplayServer::singleton();
        if fullscreen {
            // Remember current window size before going fullscreen
            self.pre_sbs_window_size = ds.window_get_size();
            let _reentrant = self.base_mut();
            ds.window_set_mode(WindowMode::FULLSCREEN);
        } else {
            let mode = Self::engine_window_mode(self.window_preference);
            let restore = self.pre_sbs_window_size;
            let _reentrant = self.base_mut();
            ds.window_set_mode(mode);
            // Restore the window size from before SBS was enabled
            if restore.x > 0 && restore.y > 0 {
                ds.window_set_size(restore);
            }
        }
    }

    /// The engine's window mode for the mono preference.
    fn engine_window_mode(preference: WindowPreference) -> WindowMode {
        match preference {
            WindowPreference::Windowed => WindowMode::WINDOWED,
            WindowPreference::Fullscreen => WindowMode::FULLSCREEN,
        }
    }

    /// Apply the mono window preference to the OS window. The headless
    /// server (GUT, the import stage) has no window and errors on the ask.
    fn apply_window_preference(&mut self) {
        let mut ds = DisplayServer::singleton();
        if ds.get_name() == "headless" {
            return;
        }
        let mode = Self::engine_window_mode(self.window_preference);
        if ds.window_get_mode() != mode {
            // Re-entrant like resize_window: release the borrow for the call.
            let _reentrant = self.base_mut();
            ds.window_set_mode(mode);
        }
    }


    // ── The side-by-side handoff to the glasses ──────────────────────

    /// Side-by-side turned on: drive the glasses first when there are
    /// glasses to drive, and take the panel when the handoff says.
    fn begin_handoff(&mut self) {
        let glasses = self.drivable_glasses();
        let (state, step) = Handoff::begin(glasses.is_some());
        self.handoff = state;
        if let Some(glasses) = glasses {
            self.set_display_status(format!(
                "XREAL glasses on {}: entering side-by-side…",
                glasses.link().interface
            ));
            let (tx, rx) = mpsc::channel();
            let worker = glasses.clone();
            std::thread::spawn(move || {
                let _ = tx.send(worker.enter_side_by_side());
            });
            self.glasses = Some(glasses);
            self.entering = Some(rx);
            self.release_when_entered = false;
        }
        self.apply_step(step);
    }

    /// Glasses the game may drive: none on a headless server (GUT, the
    /// import stage), where there is no window to hand off.
    fn drivable_glasses(&self) -> Option<Glasses> {
        if DisplayServer::singleton().get_name() == "headless" {
            return None;
        }
        Glasses::find()
    }

    /// Side-by-side turned off: put the glasses back as they were. With
    /// the entering work still in flight, the undo waits for its word.
    fn release_glasses(&mut self) {
        if self.entering.is_some() {
            self.release_when_entered = true;
            return;
        }
        let (Some(glasses), Some(restore)) = (self.glasses.take(), self.restore.take()) else {
            return;
        };
        self.set_display_status("XREAL glasses: restoring…".to_string());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(glasses.restore(restore));
        });
        self.restoring = Some(rx);
    }

    /// Each frame: take the device work's word, run the handoff's clock.
    fn poll_glasses(&mut self, delta: f64) {
        if let Some(rx) = &self.entering {
            let word = match rx.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(Err(Failure::NoReply)),
            };
            if let Some(result) = word {
                self.entering = None;
                let report = match result {
                    Ok(entered) => {
                        godot_print!(
                            "ViewManager: XREAL {} entered side-by-side (was mode {}; {} write(s))",
                            entered.id,
                            entered.before,
                            entered.applied.len()
                        );
                        self.set_display_status(format!(
                            "XREAL {}: side-by-side (was mode {})",
                            entered.id, entered.before
                        ));
                        self.restore = Some(entered.restore);
                        Report::Driven
                    }
                    Err(e) => {
                        godot_warn!("ViewManager: XREAL glasses not driven: {e}");
                        self.set_display_status(format!("XREAL glasses not driven ({e}) — fullscreen here"));
                        Report::NotDriven
                    }
                };
                if self.release_when_entered {
                    // Side-by-side already went away: nothing to hand off.
                    self.release_when_entered = false;
                    self.handoff = Handoff::Settled;
                    self.release_glasses();
                } else {
                    let (state, step) = self.handoff.report(report);
                    self.handoff = state;
                    self.apply_step(step);
                }
            }
        }
        if matches!(self.handoff, Handoff::AwaitingWideScreen { .. }) {
            let (state, step) = self.handoff.tick(delta, self.wide_screen_index());
            self.handoff = state;
            self.apply_step(step);
        }
        if let Some(rx) = &self.restoring {
            match rx.try_recv() {
                Ok(Ok(applied)) => {
                    self.restoring = None;
                    godot_print!("ViewManager: XREAL glasses restored ({} write(s))", applied.len());
                    self.set_display_status("XREAL glasses: restored".to_string());
                }
                Ok(Err(e)) => {
                    self.restoring = None;
                    godot_warn!("ViewManager: XREAL glasses not restored: {e}");
                    self.set_display_status(format!("XREAL glasses not restored ({e})"));
                }
                Err(TryRecvError::Disconnected) => self.restoring = None,
                Err(TryRecvError::Empty) => {}
            }
        }
    }

    /// The process is ending: give the glasses back before the window
    /// goes, waiting a bounded moment on work already in flight.
    fn settle_glasses_at_exit(&mut self) {
        if let Some(rx) = self.entering.take() {
            if let Ok(Ok(entered)) = rx.recv_timeout(EXIT_SETTLE) {
                self.restore = Some(entered.restore);
            }
        }
        if let Some(rx) = self.restoring.take() {
            let _ = rx.recv_timeout(EXIT_SETTLE);
            return;
        }
        if let (Some(glasses), Some(restore)) = (self.glasses.take(), self.restore.take()) {
            match glasses.restore(restore) {
                Ok(applied) => godot_print!("ViewManager: XREAL glasses restored at exit ({} write(s))", applied.len()),
                Err(e) => godot_warn!("ViewManager: XREAL glasses not restored at exit: {e}"),
            }
        }
    }

    fn apply_step(&mut self, step: Option<Step>) {
        match step {
            Some(Step::Fullscreen { screen }) => self.fullscreen_on(screen),
            None => {}
        }
    }

    /// The wide screen among the host's screens, if one exists now.
    fn wide_screen_index(&self) -> Option<usize> {
        let ds = DisplayServer::singleton();
        let sizes: Vec<(i32, i32)> = (0..ds.get_screen_count())
            .map(|i| {
                let size = ds.screen_get_size_ex().screen(i).done();
                (size.x, size.y)
            })
            .collect();
        wide_screen(&sizes)
    }

    /// Take the whole panel, on `screen` when given. A fullscreen window
    /// does not change screens, so it steps down, moves, and steps back
    /// up; every DisplayServer call goes under the re-entrancy guard
    /// (see `resize_window`).
    fn fullscreen_on(&mut self, screen: Option<usize>) {
        let mut ds = DisplayServer::singleton();
        let target = screen
            .and_then(|i| i32::try_from(i).ok())
            .filter(|&i| i != ds.window_get_current_screen());
        if let Some(i) = target {
            let was_fullscreen = ds.window_get_mode() == WindowMode::FULLSCREEN;
            let _reentrant = self.base_mut();
            if was_fullscreen {
                ds.window_set_mode(WindowMode::WINDOWED);
            }
            ds.window_set_current_screen(i);
        }
        self.resize_window(true);
    }

    fn set_display_status(&mut self, status: String) {
        self.display_status = GString::from(&status);
        let status = self.display_status.clone();
        self.base_mut()
            .emit_signal(signals::DISPLAY_STATUS_CHANGED, &[status.to_variant()]);
    }

    /// Apply the 3D render scale to the rendering viewport — the world is
    /// drawn at that fraction of the per-eye target and upscaled.
    fn apply_render_scale(&mut self, scale: RenderScale) {
        if let Some(mut vp) = self.base().get_viewport() {
            vp.set_scaling_3d_scale(scale.factor());
        }
    }
}
