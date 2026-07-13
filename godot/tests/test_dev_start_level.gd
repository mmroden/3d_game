extends GutTest
## The dev knobs are contracts too (owner 2026-07-12: the untested LEVEL
## flag rotted into a menu-walk surprise). `make run LEVEL=N SEED=S` maps
## to GameManager.start_level / .fixed_seed; F9/F10 ride debug_jump_level.
## Each knob's observable behavior is pinned here.

const UiStub := preload("res://tests/helpers/ui_stub.gd")

var _gm: GameManager
var _lm: LevelManager

func _boot_stack(level: int, seed_value: int) -> void:
	var root := Node3D.new()
	add_child_autofree(root)
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	_lm = LevelManager.new()
	_lm.name = "LevelManager"
	var player := ShipController.new()
	player.name = "Player"
	player.add_to_group("player")
	var shape := CollisionShape3D.new()
	shape.name = "CollisionShape3D"
	shape.shape = SphereShape3D.new()
	player.add_child(shape)
	_gm = GameManager.new()
	_gm.fixed_seed = seed_value
	_gm.start_level = level
	root.add_child(_lm)
	root.add_child(player)
	root.add_child(_gm)
	_gm.clear_save_for_tests()
	await wait_process_frames(6)  # the deferred initial phase + level build

func _portal_position() -> Vector3:
	var portals := _lm.find_children("*", "Portal", true, false)
	assert_false(portals.is_empty(), "the level places a portal")
	return portals.front().global_position if not portals.is_empty() else Vector3.INF

func test_start_level_boots_straight_into_the_mission():
	await _boot_stack(2, 1)  # any level > 0 arms the knob; 2 proves propagation
	assert_eq(_gm.get_phase_name(), "Playing",
		"a LEVEL=N boot lands in the mission, not the menu")
	assert_eq(_lm.current_level, 2, "…at the level under inspection")

func test_debug_jump_hops_levels_and_clamps_at_one():
	await _boot_stack(2, 1)
	_gm.debug_jump_level(1)
	await wait_process_frames(4)  # the deferred rebuild (loading veil frame)
	assert_eq(_lm.current_level, 3, "F10 hops one level up")
	assert_eq(_gm.get_phase_name(), "Playing", "the hop stays in flight")
	_gm.debug_jump_level(-5)
	await wait_process_frames(4)
	assert_eq(_lm.current_level, 1, "a hop below the campaign clamps to level 1")

func test_fixed_seed_pins_the_world():
	await _boot_stack(2, 7)
	var first := _portal_position()
	# Tear the first stack down before booting the second — two live
	# GameManagers would both drive the phase machine.
	get_children().back().queue_free()
	await wait_process_frames(2)
	await _boot_stack(2, 7)
	assert_eq(_portal_position(), first,
		"the same SEED= builds the same world, portal and all")
