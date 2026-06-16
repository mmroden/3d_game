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
	var ui_vp = _main.get_node("ViewManager/UIViewport")
	assert_not_null(ui_vp, "UIViewport must exist under ViewManager")

	var ui_names = ["MainMenuUI", "HUD", "PauseMenuUI",
		"KillSummaryUI", "ShopUI", "DeathScreenUI"]
	for ui_name in ui_names:
		var layer = _main.get_node(ui_name) as CanvasLayer
		assert_not_null(layer, "%s must exist" % ui_name)
		assert_eq(layer.get_custom_viewport(), ui_vp,
			"%s must render into UIViewport for SBS compositing" % ui_name)

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

	var ui_plane = _main.get_node("ViewManager/UIPlane")
	assert_not_null(ui_plane, "UIPlane must exist under ViewManager")
	assert_true(ui_plane.visible, "UIPlane must be visible in SBS mode")

func test_ui_plane_hidden_in_mono_mode():
	var ui_plane = _main.get_node("ViewManager/UIPlane")
	assert_not_null(ui_plane, "UIPlane must exist even in mono mode")
	assert_false(ui_plane.visible, "UIPlane must be hidden in mono mode")

func test_ui_plane_has_viewport_texture():
	var ui_plane = _main.get_node("ViewManager/UIPlane") as MeshInstance3D
	assert_not_null(ui_plane, "UIPlane must be a MeshInstance3D")
	var material = ui_plane.get_surface_override_material(0)
	if material == null:
		material = ui_plane.mesh.surface_get_material(0)
	assert_not_null(material, "UIPlane must have a material")

# --- helpers ---

func _find_panel_container(node: Node) -> PanelContainer:
	for child in node.get_children():
		if child is PanelContainer:
			return child as PanelContainer
		var found = _find_panel_container(child)
		if found:
			return found
	return null
