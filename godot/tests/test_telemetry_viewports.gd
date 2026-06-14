extends GutTest
## Telemetry must measure the viewports that actually render the 3D
## scene. In mono that is the root viewport; in SBS it is the two eye
## sub-viewports, never the root viewport — which only composites the
## two eyes and does almost no 3D work, so measuring it reports ~0 and
## hides the real per-eye render cost.

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

func test_mono_measures_the_root_viewport():
	var lm = _main.get_node("LevelManager")
	var measured = lm.measured_viewport_rids()
	assert_eq(measured.size(), 1,
		"mono mode measures exactly the root viewport")
	assert_true(measured.has(_main.get_viewport().get_viewport_rid()),
		"mono mode measures the root viewport")

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

func test_toggling_back_to_mono_restores_root_only():
	# SBS on, then off again — the measured set must follow the full
	# state machine back to the root, dropping the eyes. (Telemetry also
	# disables the eyes' render-time timers on the way out; that side
	# effect has no GUT-visible query, so it is pinned by code review and
	# the set-membership assertions below.)
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
		"back in mono, measures exactly the root viewport again")
	assert_true(measured.has(_main.get_viewport().get_viewport_rid()),
		"back in mono, measures the root viewport")
	assert_false(measured.has(left.get_viewport_rid()),
		"the left eye is dropped from the set in mono")
	assert_false(measured.has(right.get_viewport_rid()),
		"the right eye is dropped from the set in mono")
