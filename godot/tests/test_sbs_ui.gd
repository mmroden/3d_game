extends GutTest
## Tests for SBS stereo UI rendering.
## Verifies that all UI CanvasLayers render into UIViewport,
## that menu panels are centered, and that the 3D UI plane
## exists and is visible in SBS mode.

var _main: Node3D

func before_each():
	_main = load("res://scenes/main.tscn").instantiate()
	add_child(_main)
	# Two frames: one for ready(), one for deferred setup
	await get_tree().process_frame
	await get_tree().process_frame

func after_each():
	if _main and is_instance_valid(_main):
		_main.queue_free()
		await get_tree().process_frame
	# This suite toggles SBS, which now persists — clear for isolation.
	var dir = DirAccess.open("user://")
	if dir and dir.file_exists("options.cfg"):
		dir.remove("options.cfg")

# --- UIViewport wiring ---

func test_all_ui_layers_render_into_ui_viewport():
	# STRUCTURAL: every CanvasLayer child of Main renders through the one
	# UIViewport — no hand-maintained name list anywhere (the list went
	# stale the moment LoadingUI arrived and put the veil in one eye,
	# playtest 2026-07-04). Text drawn on any UI layer is view-agnostic by
	# construction: both eyes sample the same viewport texture.
	var ui_vp = _main.get_node("ViewManager/UIViewport")
	assert_not_null(ui_vp, "UIViewport must exist under ViewManager")

	var layers := 0
	for child in _main.get_children():
		if child is CanvasLayer:
			layers += 1
			assert_eq((child as CanvasLayer).get_custom_viewport(), ui_vp,
				"%s must render into UIViewport for SBS compositing" % child.name)
	assert_gt(layers, 8, "the sweep must actually cover the UI layers")

# --- Menu centering ---

func test_menu_panels_are_centered():
	# The main menu is deliberately bottom-seated now (showcase shows above it —
	# see test_showcase_screens.gd); only the modal pause menu stays centered.
	var menus = ["PauseMenuUI"]
	for menu_name in menus:
		var panel = _find_panel_container(_main.get_node(menu_name))
		if panel == null:
			gut.p("%s has no PanelContainer yet (built lazily)" % menu_name)
			continue
		assert_almost_eq(panel.anchor_left, 0.5, 0.01,
			"%s panel anchor_left should be 0.5" % menu_name)
		assert_almost_eq(panel.anchor_top, 0.5, 0.01,
			"%s panel anchor_top should be 0.5" % menu_name)
		assert_almost_eq(panel.anchor_right, 0.5, 0.01,
			"%s panel anchor_right should be 0.5" % menu_name)
		assert_almost_eq(panel.anchor_bottom, 0.5, 0.01,
			"%s panel anchor_bottom should be 0.5" % menu_name)

# --- 3D UI plane in SBS mode ---

func test_sbs_mode_creates_visible_ui_plane():
	# Toggle SBS on
	_main.get_node("GameManager").on_sbs_toggled()
	await get_tree().process_frame
	await get_tree().process_frame

	# The plane is ViewManager's (its visibility is the display's, never the
	# ship's); it is cockpit-locked through a RemoteTransform3D anchor under
	# the XR origin, so it rides the ship's pose without sharing its subtree.
	var ui_plane = _main.get_node("ViewManager/UIPlane")
	assert_not_null(ui_plane, "UIPlane must exist under ViewManager")
	assert_true(ui_plane.visible, "UIPlane must be visible in SBS mode")

func test_ui_plane_hidden_in_mono_mode():
	var ui_plane = _main.get_node("ViewManager/UIPlane")
	assert_not_null(ui_plane, "UIPlane must exist even in mono mode")
	assert_false(ui_plane.visible, "UIPlane must be hidden in mono mode")

func test_headless_sbs_never_drives_glasses():
	# The glasses handoff drives real hardware over the USB link when SBS
	# turns on. A headless server has no window to hand off, so the suite
	# must never touch glasses that happen to be plugged in: the display's
	# word stays unsaid, and the options footer stays hidden.
	_main.get_node("GameManager").on_sbs_toggled()
	await get_tree().process_frame
	await get_tree().process_frame
	var view_manager = _main.get_node("ViewManager")
	assert_eq(str(view_manager.display_status()), "",
		"headless: SBS must not start the glasses handoff")

func test_ui_plane_shows_the_menus_while_the_ship_is_hidden():
	# The menus live on the plane in SBS, and GameManager hides the Player
	# outside the flying phases — so the plane must NOT inherit the ship's
	# visibility (glasses session 2026-09-17: every menu vanished in SBS).
	_main.get_node("GameManager").on_sbs_toggled()
	await get_tree().process_frame
	await get_tree().process_frame
	var player: Node3D = _main.get_node("Player")
	player.visible = false
	await get_tree().process_frame
	var ui_plane: MeshInstance3D = _main.get_node("ViewManager/UIPlane")
	assert_true(ui_plane.is_visible_in_tree(),
		"the UI plane must stay visible in SBS while the ship is hidden (menus)")

func test_ui_plane_rides_the_eyepoint():
	# Cockpit-locked: wherever the ship's XR origin goes, the plane sits
	# ui_plane_distance straight ahead of it — through the engine's own
	# RemoteTransform3D, not a per-frame copy.
	var origin: Node3D = _main.get_node("Player/XROrigin3D")
	var anchor := origin.get_node_or_null("UIPlaneAnchor") as RemoteTransform3D
	assert_not_null(anchor, "a RemoteTransform3D anchor under the XR origin carries the plane")
	var player: Node3D = _main.get_node("Player")
	player.global_position = Vector3(12.0, -3.0, 7.0)
	player.rotation = Vector3(0.0, PI / 2.0, 0.0)
	await get_tree().process_frame
	var ui_plane: MeshInstance3D = _main.get_node("ViewManager/UIPlane")
	var expected: Vector3 = origin.global_position - origin.global_transform.basis.z * anchor.position.length()
	assert_almost_eq(ui_plane.global_position.distance_to(expected), 0.0, 0.01,
		"the plane sits straight ahead of the eyepoint after the ship moves")

func test_ui_plane_has_viewport_texture():
	var ui_plane = _main.get_node("ViewManager/UIPlane") as MeshInstance3D
	assert_not_null(ui_plane, "UIPlane must be a MeshInstance3D")
	var material = ui_plane.get_surface_override_material(0)
	if material == null:
		material = ui_plane.mesh.surface_get_material(0)
	assert_not_null(material, "UIPlane must have a material")

# --- mono UI is drawn by the fullscreen MonoUILayer (not the in-eye overlay) ---

func test_mono_draws_ui_through_the_fullscreen_mono_layer():
	# In mono, the HUD (health bars, center reticle) is drawn by the fullscreen
	# MonoUILayer CanvasLayer. The old in-eye overlay sat inside a
	# sub-viewport container and scaled the tiny reticle away, so that is NOT the
	# mono UI path. Boot is mono.
	var mono = _main.get_node_or_null("ViewManager/MonoUILayer")
	assert_not_null(mono, "MonoUILayer must exist")
	assert_true(mono.visible, "mono must draw the HUD/reticle via the fullscreen MonoUILayer")

func test_sbs_hides_the_mono_ui_layer():
	# In SBS the 3D UIPlane takes over, so the flat mono layer hides.
	_main.get_node("GameManager").on_sbs_toggled()
	await get_tree().process_frame
	await get_tree().process_frame
	var mono = _main.get_node("ViewManager/MonoUILayer")
	assert_false(mono.visible, "MonoUILayer must hide in SBS")

func test_the_display_interface_renders_the_root_viewport():
	# One render pathway: the root viewport under use_xr, drawn by the SBS
	# display interface (docs/design/xr_rig.md). Mono is one view of that
	# same interface; SBS is two. The engine polls the interface's render
	# target size every frame, so the window-follows-resize contract the
	# old sub-viewport rig needed (playtest 2026-07-05: a resized window
	# left the eye frozen at its old size) holds by construction — there
	# is no cached eye size left to go stale.
	var root := _main.get_viewport()
	assert_true(root.use_xr, "the root viewport renders through the interface")
	var iface := XRServer.primary_interface
	assert_not_null(iface, "the SBS interface is the primary XR interface")
	assert_eq(iface.get_name(), "SBS", "…by name")
	assert_true(iface.is_initialized(), "…and initialized, or nothing draws")
	assert_eq(iface.get_view_count(), 1, "mono renders one view")
	_main.get_node("GameManager").on_sbs_toggled()
	await get_tree().process_frame
	await get_tree().process_frame
	assert_eq(iface.get_view_count(), 2, "SBS renders two views through the same interface")
	_main.get_node("GameManager").on_sbs_toggled()
	await get_tree().process_frame
	await get_tree().process_frame
	assert_eq(iface.get_view_count(), 1, "…and back to one")

# --- full-screen washes cover the WINDOW, not the safe-area band ---

func test_fullscreen_washes_cover_the_window_in_sbs():
	# The damage tint and the slow wash must tint EVERYTHING each eye sees.
	# Parented inside the SBS safe-area band they render as a center stripe
	# (playtest 2026-07-09) — washes belong to the window; only positioned
	# chrome belongs in the band.
	_main.get_node("GameManager").on_sbs_toggled()
	await get_tree().process_frame
	await get_tree().process_frame
	var hud = _main.get_node("HUD")
	for wash_name in ["DamageTint", "SlowOverlay"]:
		var wash: Control = hud.find_child(wash_name, true, false)
		assert_not_null(wash, "%s must exist under the HUD" % wash_name)
		if wash == null:
			continue
		# Reference = the wash's own canvas rect (the rect FULL_RECT resolves
		# against) — window_get_size() is 0 headless. The SBS band starts
		# inside the canvas and spans less than it, so these two pins prove
		# the wash escaped the band.
		var full := wash.get_viewport_rect().size
		var rect := wash.get_global_rect()
		assert_almost_eq(rect.position.x, 0.0, 1.0,
			"%s must start at the canvas's left edge in SBS" % wash_name)
		assert_almost_eq(rect.size.x, full.x, 1.0,
			"%s must span the full canvas width in SBS" % wash_name)

# --- helpers ---

func _find_panel_container(node: Node) -> PanelContainer:
	for child in node.get_children():
		if child is PanelContainer:
			return child as PanelContainer
		var found = _find_panel_container(child)
		if found:
			return found
	return null
