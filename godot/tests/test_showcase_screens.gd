extends GutTest
## The non-gameplay screens (main menu, ship select, bestiary briefing) each put
## a lit model in front of the player camera. When that breaks, they render black
## — a regression that keeps recurring. Pin the observable invariants of a
## non-black screen, driven through the real Main scene: the showcase/turntable
## is built and visible, the camera is current, and the model sits in view.

var main

func before_each():
	main = load("res://scenes/main.tscn").instantiate()
	add_child_autofree(main)
	# Let ready() + the deferred options broadcast (which makes the camera
	# current) settle.
	await wait_process_frames(3)

func test_left_eye_camera_renders_at_boot():
	# Mono draws through the left-eye sub-viewport's camera, NOT the player
	# camera on the root viewport (that was the black-in-mono parallel pathway).
	# The left eye camera must be current or every screen is black.
	var left_cam = main.get_node_or_null(
		"ViewManager/StereoCanvas/LeftContainer/LeftViewport/LeftCamera")
	assert_not_null(left_cam, "the left-eye camera must exist")
	assert_true(left_cam.current,
		"the left-eye camera must render the world (mono draws through it)")
	var player_cam = main.get_node_or_null("Player/Camera3D")
	assert_false(player_cam.current,
		"the player camera must NOT be current — it's the reference, the eye draws")

func test_main_menu_shows_the_showcase_ship():
	var showcase = main.get_node_or_null("Turntable")
	assert_not_null(showcase, "showcase node must exist")
	assert_true(showcase.visible,
		"the showcase must be visible on the main menu (not black)")
	assert_not_null(showcase.get_node_or_null("Model"),
		"the showcase ship model must be built")

func test_showcase_sits_in_front_of_the_camera():
	var showcase = main.get_node_or_null("Turntable")
	var cam = main.get_node_or_null("Player/Camera3D")
	var d = showcase.global_position.distance_to(cam.global_position)
	assert_lt(d, 12.0,
		"the showcase must be parked in front of the camera, not off in the void")

func test_main_menu_panel_sits_low_so_action_shows_above_it():
	# The menu panel must be seated in the lower half (like ship-select and the
	# bestiary), leaving the showcase ship — and future live action — visible in
	# the upper-middle rather than covered by a centered panel.
	var ui = main.get_node("MainMenuUI")
	var panel = null
	for c in ui.get_children():
		if c is PanelContainer:
			panel = c
			break
	assert_not_null(panel, "main menu must have a panel")
	# Bottom-anchored (CENTER_BOTTOM), the same framing as ship-select and the
	# bestiary, so the showcase shows above it. The anchor is the robust signal;
	# pixel geometry is unreliable in the tiny headless viewport.
	assert_almost_eq(panel.anchor_top, 1.0, 0.01,
		"main menu panel must be bottom-anchored so the action shows above it")
	assert_almost_eq(panel.anchor_bottom, 1.0, 0.01,
		"main menu panel must be bottom-anchored")

func test_ship_select_shows_the_showcase_in_a_backdrop_room():
	var gm = main.get_node("GameManager")
	gm.start_new_game()  # MainMenu -> ShipSelect, builds the backdrop room
	await wait_process_frames(3)
	var showcase = main.get_node("Turntable")
	assert_true(showcase.visible, "showcase must be visible on the ship-select screen")
	var lm = main.get_node("LevelManager")
	assert_not_null(lm.get_node_or_null("Room0"),
		"ship-select must build a backdrop room behind the ship")

func test_selecting_each_ship_color_drives_the_showcase_without_error():
	# on_ship_color_selected re-skins the showcase via show_ship(id). This
	# guards the dynamic call's signature (id, not Color) — a runtime-only
	# mismatch the compiler can't catch. GUT fails on the engine error if it drifts.
	var gm = main.get_node("GameManager")
	gm.start_new_game()  # -> ShipSelect, showcase visible
	await wait_process_frames(3)
	var showcase = main.get_node("Turntable")
	for id in [0, 1, 2]:
		gm.on_ship_color_selected(id)
		await wait_process_frames(2)
	assert_true(showcase.visible,
		"showcase stays up through every color pick (no signature-mismatch crash)")

func test_bestiary_shows_the_turntable_in_a_backdrop_room():
	var gm = main.get_node("GameManager")
	gm.start_new_game()           # -> ShipSelect
	await wait_process_frames(2)
	gm.advance_from_ship_select()  # -> Bestiary briefing
	await wait_process_frames(3)
	var display = main.get_node("Turntable")
	assert_true(display.visible, "the bestiary turntable must be visible")
	assert_not_null(display.get_node_or_null("Model"),
		"the turntable must be spinning a model")
	var cam = main.get_node("Player/Camera3D")
	var d = display.global_position.distance_to(cam.global_position)
	assert_lt(d, 12.0, "the turntable must be in front of the camera")
	# The briefing must show the SAME lit backdrop room as ship-select — not a
	# black void. The room is built once entering the loadout flow and reused
	# across ShipSelect -> Bestiary, so its container is present and visible.
	var lm = main.get_node("LevelManager")
	var room = lm.get_node_or_null("Room0")
	assert_not_null(room, "the bestiary briefing must show the shared backdrop room")
	assert_true(room.visible, "the backdrop room must render behind the turntable, not be culled")

func test_bestiary_locks_input_briefly_on_entry():
	# The button that opens the briefing (ship-select's Continue / Fire) must not
	# bleed through and instantly begin the mission. Entering locks input briefly.
	var gm = main.get_node("GameManager")
	gm.start_new_game()           # -> ShipSelect
	await wait_process_frames(2)
	gm.advance_from_ship_select()  # -> Bestiary briefing
	await wait_process_frames(1)
	var ui = main.get_node("BestiaryUI")
	assert_true(ui.input_locked(),
		"entering the bestiary must lock input so the entry press can't begin the mission")
