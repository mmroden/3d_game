extends GutTest
## Telemetry must measure the viewport that actually renders the 3D
## scene. There is ONE render pathway — the root viewport under `use_xr`,
## drawn in a single multiview pass by the display interface: one view in
## mono, two side by side. Nothing else renders the world, so the measured
## set is the root in both modes and a mode toggle never moves it.

var _main: Node3D

func before_each():
	_main = load("res://scenes/main.tscn").instantiate()
	add_child(_main)
	# Two frames: one for ready(), one for deferred view setup.
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

func test_mono_measures_the_root_xr_viewport():
	var lm = _main.get_node("LevelManager")
	var measured = lm.measured_viewport_rids()
	var root := _main.get_viewport()
	assert_true(root.use_xr,
		"the root viewport renders through the display interface (use_xr)")
	assert_eq(measured.size(), 1,
		"one render pathway → exactly one measured viewport")
	assert_true(measured.has(root.get_viewport_rid()),
		"mono measures the root XR viewport — the one that draws")

func test_sbs_measures_the_same_root_viewport():
	_main.get_node("GameManager").on_sbs_toggled()
	await get_tree().process_frame
	await get_tree().process_frame

	var lm = _main.get_node("LevelManager")
	var measured = lm.measured_viewport_rids()
	var root := _main.get_viewport()
	assert_eq(measured.size(), 1,
		"SBS is the same pass with two views — still one measured viewport")
	assert_true(measured.has(root.get_viewport_rid()),
		"SBS measures the root XR viewport")
	assert_eq(XRServer.primary_interface.get_view_count(), 2,
		"…and that pass now renders two views")
