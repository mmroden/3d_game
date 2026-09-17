extends GutTest
## The controls screen (alpha tester Burhan, 2026-09-15: "Need to see the
## keymapping, took a lot of guessing"): a Controls row on the main menu and
## the pause menu opens a two-page screen — the gamepad silhouette with
## callouts, and the keyboard with the bound keys lit — read from the
## InputMap at runtime, never transcribed. Left/Right page between the
## devices; Back or Select close it. The first new game's pre-level
## briefing opens on the same card, once (a preference, so it survives the
## save wipe). The pages' content is void_logic::controls (Rust tests);
## the shell contracts here are open / page / close, the hosting, and
## the InputMap's own sanity.

var _main: Node3D


func before_each():
	# A fresh profile: the first briefing is owed.
	_clear_options()
	_main = load("res://scenes/main.tscn").instantiate()
	add_child(_main)
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


func test_the_controls_row_opens_the_screen_and_back_returns_to_the_rows():
	var menu = _main.get_node("MainMenuUI")
	assert_false(menu.controls_visible(), "the menu opens on its rows")
	# Row 0 is New Game (no continuable run); the rows wrap, so three ups
	# land on Controls — the row above Credits, above Exit.
	await _press("menu_up")
	await _press("menu_up")
	await _press("menu_up")
	await _press("menu_select")
	assert_true(menu.controls_visible(), "the Controls row opens the screen")
	assert_eq(_main.get_node("GameManager").get_phase_name(), "MainMenu",
		"the screen never leaves the menu phase")
	await _press("menu_back")
	assert_false(menu.controls_visible(), "back returns to the rows")
	assert_false(menu.credits_visible(), "…the rows, not another screen")


func test_left_and_right_page_between_the_devices():
	var menu = _main.get_node("MainMenuUI")
	menu.open_controls()
	assert_true(menu.controls_visible(), "open_controls is the row's own door")
	# Headless: no pad is connected, so the screen opens on the keyboard.
	var opened: int = menu.controls_page()
	assert_eq(opened, 1, "with no pad connected the keyboard page opens first")
	await _press("menu_right")
	assert_eq(menu.controls_page(), 0, "right pages to the controller")
	await _press("menu_right")
	assert_eq(menu.controls_page(), 1, "…and wraps")
	await _press("menu_left")
	assert_eq(menu.controls_page(), 0, "left pages back")
	menu.set_controls_page(1)
	assert_eq(menu.controls_page(), 1, "the capture door parks a page")
	await _press("menu_select")
	assert_false(menu.controls_visible(), "select closes the screen too")


func test_the_pages_carry_the_silhouette_the_callouts_and_the_lit_keys():
	var menu = _main.get_node("MainMenuUI")
	menu.open_controls()
	var panel = menu.find_child("ControlsPanel", true, false)
	assert_not_null(panel, "the screen is one ControlsPanel node the menu hosts")
	assert_true(panel.silhouette_loaded(), "the gamepad silhouette texture is installed (make assets)")
	# Every drawn gamepad control with a shown action bound gets a
	# callout; the sticks, both triggers, the four face buttons, the two
	# bumpers, the d-pad edges, Back and Start are all bound today.
	assert_gte(panel.callout_count(), 12, "the project's pad bindings all reach the page")
	assert_eq(panel.unplaced_pad_bindings(), 0,
		"every joypad binding in the InputMap lands on a drawn control")
	# WASD, R/F, Q/E, Space, Tab, Z/X, C, V, Esc, the four arrows and the
	# two mouse buttons: the keys the project binds all light.
	assert_gte(panel.lit_key_count(), 18, "the bound keys light up")


func test_the_input_map_is_physical_and_no_two_flight_actions_share_a_pad_button():
	# Keys bind by physical position (the diagram lights the physical
	# key; WASD stays under the fingers on AZERTY); one gamepad button
	# serves one flight action; nothing sits on the guide button the OS
	# owns; the pause is on Start/Options.
	var owners := {}
	for action in InputMap.get_actions():
		var name := String(action)
		if name.begins_with("ui_") or name.begins_with("menu_"):
			continue
		for event in InputMap.action_get_events(action):
			if event is InputEventKey:
				assert_ne(event.physical_keycode, KEY_NONE, "%s: keys bind physically" % name)
				assert_eq(event.keycode, KEY_NONE, "%s: never by label" % name)
			elif event is InputEventJoypadButton:
				assert_ne(event.button_index, JOY_BUTTON_GUIDE, "%s sits on the guide button" % name)
				assert_false(owners.has(event.button_index),
					"%s and %s share pad button %d" % [name, owners.get(event.button_index), event.button_index])
				owners[event.button_index] = name
	assert_eq(owners.get(JOY_BUTTON_START), "open_menu", "pause is on Start/Options")


func test_the_first_briefing_opens_on_the_card_once():
	var gm = _main.get_node("GameManager")
	var bestiary = _main.get_node("BestiaryUI")
	gm.start_new_game()
	gm.advance_from_ship_select()
	assert_eq(gm.get_phase_name(), "Bestiary")
	assert_true(bestiary.controls_visible(), "a fresh profile's briefing opens on the controls card")
	# The briefing's entry debounce (wall-clock, 0.25 s) holds the card too.
	await wait_seconds(0.4)
	assert_false(bestiary.input_locked(), "the entry lockout has expired")
	await _press("menu_select")
	assert_false(bestiary.controls_visible(), "select closes the card onto the bestiary entries")
	assert_eq(gm.get_phase_name(), "Bestiary", "…without starting the mission")
	# A second new game: the card was shown once and stays away.
	gm.back_from_bestiary()
	gm.start_new_game()
	gm.advance_from_ship_select()
	assert_eq(gm.get_phase_name(), "Bestiary")
	assert_false(bestiary.controls_visible(), "the card shows once per profile")
	gm.back_from_bestiary()
	gm.reload_options_from_disk()
	gm.start_new_game()
	gm.advance_from_ship_select()
	assert_false(bestiary.controls_visible(), "…and the once is remembered on disk")


func test_the_pause_menu_hosts_the_screen_too():
	var gm = _main.get_node("GameManager")
	var pause = _main.get_node("PauseMenuUI")
	gm.start_new_game()
	gm.advance_from_ship_select()
	gm.advance_from_bestiary()
	assert_eq(gm.get_phase_name(), "Playing")
	# The deferred sector build lands two ticks in.
	await wait_process_frames(4)
	# The pause press must HOLD: Escape is both open_menu and menu_back,
	# so the pause menu that appears on the press frame must not read the
	# same press as its own "back".
	watch_signals(gm)
	Input.action_press("open_menu")
	await wait_process_frames(1)
	var phases := []
	for i in range(get_signal_emit_count(gm, "phase_changed")):
		phases.append(get_signal_parameters(gm, "phase_changed", i))
	assert_eq(gm.get_phase_name(), "Paused", "the press pauses (phase history: %s)" % [phases])
	await wait_process_frames(2)
	Input.action_release("open_menu")
	await wait_process_frames(1)
	assert_eq(gm.get_phase_name(), "Paused", "…and the pause holds past the press frame")
	# Pause rows: Resume / Options / Controls / New Game / Quit.
	await _press("menu_down")
	await _press("menu_down")
	await _press("menu_select")
	assert_true(pause.controls_visible(), "the pause menu's Controls row opens the screen")
	await _press("menu_back")
	assert_false(pause.controls_visible(), "back returns to the pause rows")
	assert_eq(gm.get_phase_name(), "Paused", "…still paused")
	await _press("menu_back")
	assert_eq(gm.get_phase_name(), "Playing", "back again resumes")
