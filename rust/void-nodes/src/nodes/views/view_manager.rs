use godot::prelude::*;
use godot::builtin::Signal;
use godot::classes::{
    CanvasLayer, DisplayServer, INode3D, Image, MeshInstance3D, Node, Node3D, QuadMesh,
    RenderingServer, StandardMaterial3D, SubViewport, TextureRect, XrCamera3D, XrServer,
    base_material_3d::{ShadingMode, Transparency, CullMode, Flags},
    display_server::WindowMode,
    texture_rect::StretchMode,
    sub_viewport::UpdateMode,
    viewport::Msaa,
};

use crate::nodes::constants::{methods, nodes, signals};
use crate::nodes::live_handle::{LiveOpt, LiveRef};
use crate::nodes::ui::options_wire;
use crate::nodes::views::sbs_interface::{window_pixels, SbsInterface};
use void_logic::game_options::{RenderScale, WindowMode as WindowPreference};
use void_logic::stereo::{msaa_allowed, per_eye_size, ui_plane_size, DisplayMode, StereoConfig};

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
/// by the `SbsInterface` this node registers as the XRServer's primary
/// interface: one view in mono, two side by side, the eye geometry and
/// the window blits inside the interface. The player's `XRCamera3D`
/// (under `Player/XROrigin3D`, the eyepoint) is the one camera.
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

    current_mode: DisplayMode,
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
            current_mode: DisplayMode::Mono,
            pre_sbs_window_size: Vector2i::new(0, 0),
            dynamic_stereo: false,
            window_preference: WindowPreference::Windowed,
            director_focus: None,
            capture_interaxial: None,
            capture_convergence: None,
            ui_plane: None,
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
        godot_print!("ViewManager ready — {}", self.current_mode.label());
        self.log_display_geometry("startup");
    }

    fn exit_tree(&mut self) {
        self.teardown_interface();
    }

    fn process(&mut self, _delta: f64) {
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
        let [eye_w, eye_h] = per_eye_size(self.current_mode, window_pixels());
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
            self.current_mode.label(),
            eye_w,
            eye_h,
            texture_side,
        );
    }

    /// Called when GameManager emits options_changed.
    #[func]
    pub fn on_options_changed(&mut self, options: options_wire::OptionsDictionary) {
        let options = options_wire::from_dictionary(&options);
        let (sbs_enabled, msaa_enabled) = (options.sbs_enabled, options.msaa_enabled);
        self.dynamic_stereo = options.dynamic_stereo;
        self.window_preference = options.window_mode;
        let target = if sbs_enabled {
            DisplayMode::SideBySide
        } else {
            DisplayMode::Mono
        };

        if target != self.current_mode {
            let sbs = target == DisplayMode::SideBySide;
            // Play-path SBS goes fullscreen (the glasses want the whole
            // panel); a CAPTURE run with commanded stereo geometry keeps
            // the commanded --resolution instead — disparity contracts
            // need pixel-deterministic frames, not the host's display
            // size (a fullscreen capture ballooned to the physical
            // screen, 2026-08-20).
            if self.capture_interaxial.is_none() {
                self.resize_window(sbs);
            }
            self.current_mode = target;
            // The interface flips its view count on the same frame as the
            // toggle; the engine re-sizes the render target from it.
            self.push_stereo();
            // Rebuild the UI texture and plane to the new geometry on the
            // same frame, rather than waiting on the OS resize event.
            self.resize_ui();
            self.apply_visibility(sbs);
            self.log_display_geometry("display mode change");
            godot_print!("SBS stereo {}", if sbs { "enabled" } else { "disabled" });
        }

        // ViewManager owns all viewport anti-aliasing: apply MSAA to the
        // viewport that renders the world. Runs on every options change —
        // a pure MSAA toggle and a post-mode-switch re-apply both end up
        // correct.
        self.apply_msaa(msaa_enabled);
        // The 3D render scale rides every broadcast too; the window
        // preference applies in mono (SBS owns the window there), and
        // never on a capture run with a commanded resolution.
        self.apply_render_scale(options.render_scale);
        if self.current_mode == DisplayMode::Mono && self.capture_interaxial.is_none() {
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

    /// The frame the display shows, as an image — the capture rig's one
    /// door. Mono is the root render target; side by side it is the two
    /// multiview layers stitched left|right, exactly what the window
    /// shows (the harness's SBS frame spans the full window, one eye per
    /// half). `None` when nothing has been drawn (headless).
    pub(crate) fn capture_frame(&self) -> Option<Gd<Image>> {
        let viewport = self.base().get_viewport()?;
        let texture = viewport.get_texture()?;
        match self.current_mode {
            DisplayMode::Mono => texture.get_image(),
            DisplayMode::SideBySide => {
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

    /// Register the SBS display interface as the XRServer's primary
    /// interface and hand the root viewport to it: from here on the 3D
    /// world renders through `use_xr`, one pass, one or two views.
    fn setup_interface(&mut self) {
        let mut interface = SbsInterface::new_gd();
        let mut xr = XrServer::singleton();
        xr.add_interface(&interface);
        if !interface.initialize() {
            godot_error!("ViewManager: the SBS display interface failed to initialize");
            return;
        }
        xr.set_primary_interface(&interface);
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
    /// the interface — the one place the render learns them.
    fn push_stereo(&self) {
        let dials = self.stereo_dials();
        let fov = self.camera_fov();
        if let Some(mut iface) = self.sbs_interface() {
            iface.bind_mut().configure(self.current_mode, dials, fov);
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
    fn stereo_dials(&self) -> StereoConfig {
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
        let [viewport_width, viewport_height] = per_eye_size(self.current_mode, window_pixels());
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

    /// Create the UIPlane: a QuadMesh textured with the UIViewport, a child
    /// of the player's XR origin at `ui_plane_distance` straight ahead. In
    /// SBS both eyes render it with natural parallax, giving the UI real
    /// depth; being a child of the origin it rides the ship's rendered
    /// (interpolated) pose exactly like the hull — no per-frame sync.
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
        ui_plane.set_transform(Transform3D::new(
            Basis::IDENTITY,
            Vector3::new(0.0, 0.0, -self.ui_plane_distance),
        ));

        let Some(mut origin) = self
            .base()
            .get_parent()
            .and_then(|main| main.try_get_node_as::<Node3D>(nodes::PLAYER_ORIGIN))
        else {
            godot_error!("ViewManager: no player XR origin to carry the UI plane");
            return;
        };
        origin.add_child(&ui_plane);
        self.ui_plane = Some(LiveRef::new(&ui_plane));
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
    /// CanvasLayer at 1:1, so tiny center chrome survives); in SBS the 3D
    /// UIPlane takes over, so the flat layer hides.
    fn apply_visibility(&mut self, sbs: bool) {
        if let Some(mut mono) = self.base().try_get_node_as::<CanvasLayer>(nodes::MONO_UI_LAYER) {
            mono.set_visible(!sbs);
        }
        self.ui_plane.with(|plane| plane.set_visible(sbs));
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
        let allowed = msaa_allowed(self.current_mode, &driver);
        if enabled && !allowed {
            godot_print!(
                "MSAA requested but the {driver} driver cannot multisample a {} render; rendering without it (TAA stays on)",
                self.current_mode.label()
            );
        }
        let msaa = if enabled && allowed { Msaa::MSAA_4X } else { Msaa::DISABLED };
        if let Some(mut vp) = self.base().get_viewport() {
            vp.set_msaa_3d(msaa);
        }
    }

    fn resize_window(&mut self, sbs: bool) {
        let mut ds = DisplayServer::singleton();
        if sbs {
            // Remember current window size before going fullscreen
            self.pre_sbs_window_size = ds.window_get_size();
            ds.window_set_mode(WindowMode::FULLSCREEN);
        } else {
            ds.window_set_mode(Self::engine_window_mode(self.window_preference));
            // Restore the window size from before SBS was enabled
            if self.pre_sbs_window_size.x > 0 && self.pre_sbs_window_size.y > 0 {
                ds.window_set_size(self.pre_sbs_window_size);
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
            ds.window_set_mode(mode);
        }
    }

    /// Apply the 3D render scale to the rendering viewport — the world is
    /// drawn at that fraction of the per-eye target and upscaled.
    fn apply_render_scale(&mut self, scale: RenderScale) {
        if let Some(mut vp) = self.base().get_viewport() {
            vp.set_scaling_3d_scale(scale.factor());
        }
    }
}
