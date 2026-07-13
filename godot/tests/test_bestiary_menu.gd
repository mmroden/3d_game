extends GutTest
## The root-level bestiary (playtest 2026-07-07): the catalog is browsable from
## the main menu, distinct from the pre-level briefing. Opening it from the menu
## and pressing select or back both return to the menu — never a mission. The
## pre-level briefing (ship-select -> bestiary -> play) is unchanged.

const UiStub := preload("res://tests/helpers/ui_stub.gd")


func _menu_stack() -> GameManager:
	var root := Node3D.new()
	add_child_autofree(root)
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	var gm := GameManager.new()
	root.add_child(lm)
	root.add_child(gm)
	gm.clear_save_for_tests()
	return gm


func test_the_bestiary_opens_from_the_menu_and_returns_to_it():
	var gm := _menu_stack()
	await wait_process_frames(3)
	assert_eq(gm.get_phase_name(), "MainMenu", "the stack opens on the menu")

	gm.show_bestiary_from_menu()
	assert_eq(gm.get_phase_name(), "Bestiary", "the menu opens the catalog")

	# Select from a menu browse returns to the menu — never starts a mission.
	gm.advance_from_bestiary()
	assert_eq(gm.get_phase_name(), "MainMenu", "select returns to the menu, not a mission")

	# Back does the same.
	gm.show_bestiary_from_menu()
	assert_eq(gm.get_phase_name(), "Bestiary")
	gm.back_from_bestiary()
	assert_eq(gm.get_phase_name(), "MainMenu", "back returns to the menu")


func test_the_pre_level_briefing_still_starts_a_mission():
	var gm := _menu_stack()
	await wait_process_frames(3)
	gm.start_new_game()
	gm.advance_from_ship_select()  # ShipSelect -> Bestiary briefing
	assert_eq(gm.get_phase_name(), "Bestiary", "the briefing sits before the level")
	gm.advance_from_bestiary()     # briefing select -> Playing, NOT the menu
	assert_eq(gm.get_phase_name(), "Playing", "the briefing still drops into the level")
