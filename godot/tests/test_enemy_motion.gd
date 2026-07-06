extends GutTest
## Playtest repro (owner, 2026-07-05, planet 2): an enemy sat completely
## static while the player hung in front of it. The FSM contract says any
## enemy within detection_range (25 m) leaves Idle and CHASES on distance
## alone — line of sight gates only firing. These tests pin that a live
## enemy visibly closes on the player, on both paradigms: planet 1
## (layered megakit) as the control, planet 2 (cubic panel cells) as the
## repro.

const UiStub := preload("res://tests/helpers/ui_stub.gd")

var _gm: GameManager
var _lm: LevelManager
var _player: ShipController

func _build_stack(level: int) -> void:
	var root := Node3D.new()
	add_child_autofree(root)
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	_lm = LevelManager.new()
	_lm.name = "LevelManager"
	_player = ShipController.new()
	_player.name = "Player"
	_player.add_to_group("player")
	var shape := CollisionShape3D.new()
	shape.name = "CollisionShape3D"
	shape.shape = SphereShape3D.new()
	_player.add_child(shape)
	_gm = GameManager.new()
	_gm.fixed_seed = 1
	_gm.start_level = level  # the LEVEL= dev door — one build, no walking
	root.add_child(_lm)
	root.add_child(_player)
	root.add_child(_gm)
	_gm.clear_save_for_tests()

func _walk_to_playing() -> void:
	_gm.advance_from_ship_select()
	for _i in range(12):
		if _gm.get_phase_name() == "Playing":
			break
		_gm.advance_from_bestiary()
	assert_eq(_gm.get_phase_name(), "Playing", "the stack must reach Playing")

func _teleport(pos: Vector3) -> void:
	_player.global_position = pos
	_player.linear_velocity = Vector3.ZERO
	_player.reset_physics_interpolation()

func _one_live_enemy_per_type() -> Array:
	var by_type := {}
	for e in _lm.find_children("*", "EnemyDrone", true, false):
		if e.visible and e.spawns_directly() and not by_type.has(e.enemy_type_id):
			by_type[e.enemy_type_id] = e  # the line roster, by the def's own fact
	return by_type.values()

func _assert_enemies_close_on_player(level: int) -> void:
	_build_stack(level)
	_gm.start_new_game()
	_walk_to_playing()
	await wait_process_frames(2)

	var probes := _one_live_enemy_per_type()
	assert_true(probes.size() > 0, "level %d spawns live enemies" % level)
	for enemy in probes:
		if not is_instance_valid(enemy):
			continue
		var start: Vector3 = enemy.global_position
		# Hang 15 m off the enemy — inside detection (25 m), outside every
		# attack range (a Shooter inside its 10 m range HOLDS and fires by
		# design) — and stay still. The contract: it comes to us. Peak
		# displacement over a window long enough for the full blocked-chase
		# escape ladder (sidestep → flip → retreat, ~2 s) — a drone wedged
		# into panel relief must fight its way out, and an escaping drone
		# may loop back through its start point.
		_teleport(start + Vector3(15.0, 0.0, 0.0))
		var moved := 0.0
		for _chunk in range(8):
			await wait_physics_frames(30, "the enemy must get time to move")
			if not is_instance_valid(enemy):
				break
			moved = maxf(moved, (enemy.global_position - start).length())
			if moved > 1.0:
				break
		assert_gt(moved, 1.0,
			"an enemy with the player 15 m away must chase, peak move %0.2f m (type %d, level %d)"
				% [moved, enemy.enemy_type_id, level])

func test_planet_1_enemies_close_on_the_player():
	await _assert_enemies_close_on_player(1)

func test_planet_2_enemies_close_on_the_player():
	await _assert_enemies_close_on_player(10)
