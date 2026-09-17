extends GutTest
## The options (alpha tester Burhan, 2026-09-15: audio sliders, video
## resolution, mouse for looking). ONE GameOptions in GameManager names
## its own entries; that list is saved, loaded and broadcast as a
## dictionary, and both menus host one OptionsPanel that walks it as
## typed rows and only ever asks GameManager for a change. The rows'
## semantics (toggle/choice/slider, clamps, texts) are void_logic
## (Rust tests); the shell contracts here: the rows reach GameManager,
## a change reaches the bus / the viewport / the ship, persists, and
## the mouse steers.

var _main: Node3D


func before_each():
	_clear_options()
	_main = load("res://scenes/main.tscn").instantiate()
	add_child(_main)
	# ready() + the deferred options broadcast + the opening phase.
	await wait_process_frames(3)


func after_each():
	if _main and is_instance_valid(_main):
		_main.queue_free()
		await get_tree().process_frame
	_clear_options()


func _clear_options() -> void:
	var dir = DirAccess.open("user://")
	if dir and dir.file_exists("options.cfg"):
		dir.remove("options.cfg")


func _press(action: String) -> void:
	Input.action_press(action)
	await wait_process_frames(2)
	Input.action_release(action)
	await wait_process_frames(1)


func _linear_to_db(linear: float) -> float:
	return 20.0 * log(linear) / log(10.0)


func test_the_options_rows_reach_the_mixer_through_game_manager():
	var gm = _main.get_node("GameManager")
	var menu = _main.get_node("MainMenuUI")
	var panel = menu.find_child("OptionsPanel", true, false)
	assert_not_null(panel, "the menu hosts one OptionsPanel")
	assert_false(panel.visible, "the rows wait for the Options row")
	# Walk the cursor to the Options row by its text (a continuable save
	# from an earlier script puts a Continue row on top, so no fixed count
	# of presses is honest), then select. Every walk here is bounded: an
	# unbounded wait on a row hung a full suite for an hour (2026-09-16).
	assert_true(await _walk_menu_to("Options"), "the menu has an Options row")
	await _press("menu_select")
	assert_true(panel.visible, "the Options row shows the rows")
	if not panel.visible:
		return
	assert_eq(panel.selected_row(), "sbs_stereo", "the cursor opens at the top")
	var texts: PackedStringArray = panel.row_texts()
	assert_true(String(texts[0]).begins_with("> "), "the top row carries the cursor: %s" % [texts])
	assert_true(_any_contains(texts, "Music volume: 100%"), "volumes start full: %s" % [texts])
	# Walk to the music volume row and step it down: the Music bus
	# follows through GameManager (the panel never touches the mixer).
	var music := AudioServer.get_bus_index("Music")
	assert_almost_eq(AudioServer.get_bus_volume_db(music), 0.0, 0.01, "full to begin with")
	var steps := 0
	while panel.selected_row() != "music_volume" and steps < 12:
		await _press("menu_down")
		steps += 1
	assert_eq(panel.selected_row(), "music_volume", "the cursor reaches the music row")
	await _press("menu_left")
	assert_almost_eq(AudioServer.get_bus_volume_db(music), _linear_to_db(0.9), 0.05,
		"one notch down is 90% on the Music bus")
	assert_eq(gm.option_value_text("music_volume"), "90%", "GameManager holds the truth")
	texts = panel.row_texts()
	assert_true(_any_contains(texts, "Music volume: 90%"), "the row shows the broadcast: %s" % [texts])
	await _press("menu_right")
	assert_almost_eq(AudioServer.get_bus_volume_db(music), 0.0, 0.01, "…and back up")
	await _press("menu_back")
	assert_false(panel.visible, "back hides the rows")
	assert_eq(gm.get_phase_name(), "MainMenu", "…on the menu")


func test_a_toggle_and_a_choice_reach_their_consumers():
	var gm = _main.get_node("GameManager")
	var left := _main.get_node("ViewManager/StereoCanvas/LeftContainer/LeftViewport") as SubViewport
	assert_almost_eq(left.scaling_3d_scale, 1.0, 0.001, "full render scale to begin with")
	gm.on_option_adjusted("render_scale", -1)
	await wait_process_frames(1)
	assert_almost_eq(left.scaling_3d_scale, 0.75, 0.001,
		"the render scale reaches the eye viewport through the broadcast")
	assert_eq(gm.option_value_text("render_scale"), "75%")
	gm.on_option_adjusted("render_scale", -1)
	gm.on_option_adjusted("render_scale", -1)
	assert_eq(gm.option_value_text("render_scale"), "50%", "choices clamp at their ends")
	# The old toggle doors still go through the one door.
	assert_false(gm.msaa_enabled())
	gm.on_msaa_toggled()
	assert_true(gm.msaa_enabled())
	assert_eq(gm.option_value_text("msaa"), "ON")
	assert_eq(_main.get_node("MainMenuUI").displayed_msaa(), true,
		"the menu's rows show the authoritative option")


func test_every_preference_persists_across_a_reload():
	var gm = _main.get_node("GameManager")
	gm.on_option_adjusted("mouse_sensitivity", 2)
	gm.on_option_adjusted("invert_mouse_y", 1)
	gm.on_option_adjusted("sfx_volume", -3)
	gm.on_option_adjusted("window_mode", 1)
	gm.reload_options_from_disk()
	assert_eq(gm.option_value_text("mouse_sensitivity"), "7")
	assert_eq(gm.option_value_text("invert_mouse_y"), "ON")
	assert_eq(gm.option_value_text("sfx_volume"), "70%")
	assert_eq(gm.option_value_text("window_mode"), "Fullscreen")
	assert_eq(gm.option_value_text("master_volume"), "100%", "an untouched preference keeps its default")


func test_the_mouse_steers_the_ship_while_flying():
	var gm = _main.get_node("GameManager")
	var player := _main.get_node("Player") as RigidBody3D
	gm.on_option_adjusted("mouse_sensitivity", 1)
	await wait_process_frames(1)
	assert_eq(player.mouse_sensitivity(), 6, "the ship hears the mouse preference")
	gm.start_new_game()
	gm.advance_from_ship_select()
	gm.advance_from_bestiary()
	assert_eq(gm.get_phase_name(), "Playing")
	await wait_process_frames(4)
	await wait_physics_frames(4)
	assert_almost_eq(player.angular_velocity.length(), 0.0, 0.01, "at rest before the mouse moves")
	# Mouse right: yaw right, which is negative yaw in the ship's frame.
	var motion := InputEventMouseMotion.new()
	motion.relative = Vector2(40.0, 0.0)
	Input.parse_input_event(motion)
	await wait_physics_frames(3)
	assert_lt(player.angular_velocity.y, -0.01, "mouse right yaws the ship right")


## Press menu_down until the main menu's selected row reads `row`; false
## if it never does within one wrap of the rows.
func _walk_menu_to(row: String) -> bool:
	var menu = _main.get_node("MainMenuUI")
	for _i in range(8):
		for label in menu.find_children("*", "Label", true, false):
			if String(label.text).begins_with("> " + row):
				return true
		await _press("menu_down")
	return false


func _any_contains(texts: PackedStringArray, needle: String) -> bool:
	for t in texts:
		if String(t).contains(needle):
			return true
	return false
