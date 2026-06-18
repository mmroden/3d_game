extends GutTest
## Telemetry must measure the viewports that actually render the 3D
## scene. There is ONE render pathway — the eye sub-viewports: mono draws
## through the left eye (shown fullscreen), SBS through both. The root
## viewport never renders the world (it only hosts the eye canvas), so
## measuring it would report ~0 and hide the real per-eye render cost.

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

func test_mono_measures_the_left_eye():
	var lm = _main.get_node("LevelManager")
	var measured = lm.measured_viewport_rids()
	var left = _main.get_node(
		"ViewManager/StereoCanvas/LeftContainer/LeftViewport")
	assert_eq(measured.size(), 1,
		"mono renders through the left eye, so it measures exactly that one viewport")
	assert_true(measured.has(left.get_viewport_rid()),
		"mono measures the left eye sub-viewport (the one that draws)")
	assert_false(measured.has(_main.get_viewport().get_viewport_rid()),
		"mono must NOT measure the root viewport — the left eye draws, not the root")

func test_sbs_measures_both_eyes_not_root():
	_main.get_node("GameManager").on_sbs_toggled()
	await get_tree().process_frame
	await get_tree().process_frame

	var lm = _main.get_node("LevelManager")
	var measured = lm.measured_viewport_rids()

	var left = _main.get_node(
		"ViewManager/StereoCanvas/LeftContainer/LeftViewport")
	var right = _main.get_node(
		"ViewManager/StereoCanvas/RightContainer/RightViewport")
	var root_rid = _main.get_viewport().get_viewport_rid()

	assert_eq(measured.size(), 2,
		"SBS measures both eye sub-viewports")
	assert_false(measured.has(root_rid),
		"SBS must NOT measure the root compositor viewport")
	assert_true(measured.has(left.get_viewport_rid()),
		"SBS measures the left eye sub-viewport")
	assert_true(measured.has(right.get_viewport_rid()),
		"SBS measures the right eye sub-viewport")

func test_toggling_back_to_mono_keeps_left_eye_only():
	# SBS on, then off again — the measured set must follow the state machine
	# back to mono, which keeps the LEFT eye (it still draws) and drops only the
	# right eye. The root is never in the set.
	var gm = _main.get_node("GameManager")
	gm.on_sbs_toggled()
	await get_tree().process_frame
	await get_tree().process_frame
	gm.on_sbs_toggled()
	await get_tree().process_frame
	await get_tree().process_frame

	var lm = _main.get_node("LevelManager")
	var measured = lm.measured_viewport_rids()
	var left = _main.get_node(
		"ViewManager/StereoCanvas/LeftContainer/LeftViewport")
	var right = _main.get_node(
		"ViewManager/StereoCanvas/RightContainer/RightViewport")

	assert_eq(measured.size(), 1,
		"back in mono, measures exactly the left eye")
	assert_true(measured.has(left.get_viewport_rid()),
		"the left eye keeps drawing in mono, so it stays measured")
	assert_false(measured.has(right.get_viewport_rid()),
		"the right eye is dropped from the set in mono")
	assert_false(measured.has(_main.get_viewport().get_viewport_rid()),
		"the root viewport is never measured")
