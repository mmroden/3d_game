extends GutTest
## Dev cheats for level inspection (owner's ask 2026-07-04): `start_level`
## jumps a new game straight to a level (same knob family as `fixed_seed`),
## and debug level-hopping rebuilds through the ONE build pathway — so a
## jumped-to boss level arrives fully staged (arena, gate, schedule).

const UiStub := preload("res://tests/helpers/ui_stub.gd")

var _gm: GameManager
var _lm: LevelManager

func _build_stack() -> void:
	var root := Node3D.new()
	add_child_autofree(root)
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	_lm = LevelManager.new()
	_lm.name = "LevelManager"
	_gm = GameManager.new()
	_gm.fixed_seed = 1
	root.add_child(_lm)
	root.add_child(_gm)
	_gm.clear_save_for_tests()

func _walk_to_playing() -> void:
	_gm.advance_from_ship_select()
	for _i in range(12):
		if _gm.get_phase_name() == "Playing":
			break
		_gm.advance_from_bestiary()
	assert_eq(_gm.get_phase_name(), "Playing", "the stack must reach Playing")

func test_start_level_jumps_a_new_game_straight_to_the_level():
	_build_stack()
	_gm.start_level = 5
	_gm.start_new_game()
	_walk_to_playing()
	assert_eq(_gm.get_current_level(), 5,
		"start_level drops the run at the level under inspection")
	assert_eq(_gm.boss_fight_state(), -1, "level 5 stages no boss")

func test_start_level_lands_boss_levels_fully_staged():
	_build_stack()
	_gm.start_level = 3
	_gm.start_new_game()
	_walk_to_playing()
	await wait_process_frames(2)
	assert_eq(_gm.get_current_level(), 3)
	assert_eq(_gm.boss_fight_state(), 0,
		"a jumped-to boss level arrives with the fight staged")
	var gates := _lm.find_children("*", "BossGate", true, false)
	assert_false(gates.is_empty(), "…and the arena gate pre-built")

func test_debug_jump_rebuilds_the_next_level_in_place():
	_build_stack()
	_gm.start_new_game()
	_walk_to_playing()
	assert_eq(_gm.get_current_level(), 1)

	_gm.debug_jump_level(1)
	await wait_process_frames(4)  # the rebuild rides the deferred build tick
	assert_eq(_gm.get_current_level(), 2, "F10 hops one level forward")
	assert_eq(_gm.get_phase_name(), "Playing", "no shop, no menus — inspection stays in flight")

	_gm.debug_jump_level(1)
	await wait_process_frames(4)
	assert_eq(_gm.get_current_level(), 3, "hops chain")
	assert_eq(_gm.boss_fight_state(), 0, "a hopped-to boss level stages its fight")

	_gm.debug_jump_level(-1)
	_gm.debug_jump_level(-1)
	await wait_process_frames(4)
	assert_eq(_gm.get_current_level(), 1, "F9 hops back; the floor is level 1")
	_gm.debug_jump_level(-1)
	await wait_process_frames(4)
	assert_eq(_gm.get_current_level(), 1, "hopping below level 1 clamps")
