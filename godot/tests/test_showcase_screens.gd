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

func test_player_camera_is_current_at_boot():
	var cam = main.get_node_or_null("Player/Camera3D")
	assert_not_null(cam, "player camera must exist")
	assert_true(cam.current,
		"the player camera must be current at boot, or every screen is black")

func test_main_menu_shows_the_showcase_ship():
	var showcase = main.get_node_or_null("ShipShowcase")
	assert_not_null(showcase, "showcase node must exist")
	assert_true(showcase.visible,
		"the showcase must be visible on the main menu (not black)")
	assert_not_null(showcase.get_node_or_null("Model"),
		"the showcase ship model must be built")

func test_showcase_sits_in_front_of_the_camera():
	var showcase = main.get_node_or_null("ShipShowcase")
	var cam = main.get_node_or_null("Player/Camera3D")
	var d = showcase.global_position.distance_to(cam.global_position)
	assert_lt(d, 12.0,
		"the showcase must be parked in front of the camera, not off in the void")

func test_ship_select_shows_the_showcase_in_a_backdrop_room():
	var gm = main.get_node("GameManager")
	gm.start_new_game()  # MainMenu -> ShipSelect, builds the backdrop room
	await wait_process_frames(3)
	var showcase = main.get_node("ShipShowcase")
	assert_true(showcase.visible, "showcase must be visible on the ship-select screen")
	var lm = main.get_node("LevelManager")
	assert_not_null(lm.get_node_or_null("Room0"),
		"ship-select must build a backdrop room behind the ship")

func test_bestiary_shows_the_turntable_in_a_backdrop_room():
	var gm = main.get_node("GameManager")
	gm.start_new_game()           # -> ShipSelect
	await wait_process_frames(2)
	gm.advance_from_ship_select()  # -> Bestiary briefing
	await wait_process_frames(3)
	var display = main.get_node("BestiaryDisplay")
	assert_true(display.visible, "the bestiary turntable must be visible")
	assert_not_null(display.get_node_or_null("Model"),
		"the turntable must be spinning a model")
	var cam = main.get_node("Player/Camera3D")
	var d = display.global_position.distance_to(cam.global_position)
	assert_lt(d, 12.0, "the turntable must be in front of the camera")
