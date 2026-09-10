extends GutTest
## Credits from the main menu (owner 2026-09-09: "can we have a menu item
## that is 'credits'?" … "I imagine that they need to scroll"). A scrolling
## roll INSIDE the menu panel — the team first, then every shipped
## third-party source — reached from a Credits row. It moves on its own,
## Down hurries it, Up rewinds it, both ends clamp, and Back or Select
## return to the rows. It never leaves the menu phase: no GamePhase, no
## GameManager traffic. The roll's content and motion rule are
## void_logic::credits (their own Rust tests); the shell contract here is
## open / move / park / close. The parked offset is also the capture door
## (`--screen=credits --shot=<offset;…>`, test_credits_capture.gd).

var _main: Node3D


func before_each():
	_main = load("res://scenes/main.tscn").instantiate()
	add_child(_main)
	# ready() + the deferred options broadcast + the opening phase.
	await wait_process_frames(3)


func after_each():
	if _main and is_instance_valid(_main):
		_main.queue_free()
		await get_tree().process_frame


func _press(action: String) -> void:
	Input.action_press(action)
	await wait_process_frames(2)
	Input.action_release(action)
	await wait_process_frames(1)


func test_the_credits_row_opens_the_roll_at_the_top():
	var menu = _main.get_node("MainMenuUI")
	assert_false(menu.credits_visible(), "the menu opens on its rows")
	# The cursor parks on row 0 (Continue or New Game) and the rows wrap, so
	# two ups land on Credits — the row above Exit — whatever the Continue
	# availability.
	await _press("menu_up")
	await _press("menu_up")
	await _press("menu_select")
	assert_true(menu.credits_visible(), "the Credits row opens the roll")
	# The roll is already moving on its own — the press helper waited a
	# few frames — so "the top" is within those frames' travel.
	assert_almost_eq(menu.credits_offset(), 0.0, 8.0, "the roll opens at the top")


func test_the_roll_moves_by_itself_up_rewinds_and_back_returns_to_the_menu():
	var menu = _main.get_node("MainMenuUI")
	menu.open_credits()
	assert_true(menu.credits_visible(), "open_credits is the row's own door")
	# The content lays out a frame after it is built; then the roll has
	# somewhere to go — the credits outrun the panel by construction.
	await wait_process_frames(2)
	assert_gt(menu.credits_extent(), 0.0, "the roll reaches past its window")
	await wait_process_frames(30)
	var rolled: float = menu.credits_offset()
	assert_gt(rolled, 0.0, "the roll moves on its own")
	Input.action_press("menu_up")
	await wait_process_frames(30)
	Input.action_release("menu_up")
	assert_lt(menu.credits_offset(), rolled, "holding Up rewinds")
	await _press("menu_back")
	assert_false(menu.credits_visible(), "back closes the roll")
	assert_eq(_main.get_node("GameManager").get_phase_name(), "MainMenu",
		"the roll never leaves the menu phase")


func test_a_parked_offset_holds_still_for_a_capture():
	var menu = _main.get_node("MainMenuUI")
	menu.open_credits()
	await wait_process_frames(2)
	menu.set_credits_offset(120.0)
	await wait_process_frames(20)
	assert_almost_eq(menu.credits_offset(), 120.0, 1.0,
		"a parked roll stays where the capture put it")


func test_select_also_closes_the_roll():
	var menu = _main.get_node("MainMenuUI")
	menu.open_credits()
	assert_true(menu.credits_visible(), "the roll is open")
	await _press("menu_select")
	assert_false(menu.credits_visible(), "select closes the roll, like the bestiary browse")



func test_the_crawl_ends_by_returning_to_the_menu():
	# A credits screen ends itself: once the last line has cleared the top,
	# the menu rows are back. Park a hair short of the end (the extent is
	# the column's height plus the window's), let it run, and it finishes.
	var menu = _main.get_node("MainMenuUI")
	menu.open_credits()
	await wait_process_frames(2)
	var extent: float = menu.credits_extent()
	assert_gt(extent, 0.0, "the crawl has a travel once the column has laid out")
	menu.set_credits_offset(extent - 2.0)
	menu.release_credits()
	await wait_process_frames(10)
	assert_false(menu.credits_visible(), "the crawl closed itself at its end")
	assert_eq(_main.get_node("GameManager").get_phase_name(), "MainMenu",
		"…back on the menu, never elsewhere")
