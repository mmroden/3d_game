extends GutTest
## Playtest repro (owner, 2026-07-05, planet 2): an enemy sat completely
## static while the player hung in front of it. The FSM contract says any
## enemy within detection_range (25 m) leaves Idle and CHASES on distance
## alone — line of sight gates only firing. These tests pin that a live
## enemy visibly closes on the player, on both paradigms: planet 1
## (layered megakit) as the control, planet 2 (cubic panel cells) as the
## repro.

const FullStack := preload("res://tests/helpers/full_stack.gd")

var _gm: GameManager
var _lm: LevelManager
var _player: ShipController

func _build_stack(level: int) -> void:
	# The LEVEL= dev door — one build, no walking.
	var stack := FullStack.build(self, {"seed": 1, "level": level})
	_gm = stack.gm
	_lm = stack.lm
	_player = stack.player

func _walk_to_playing() -> void:
	FullStack.walk_to_playing(self, _gm)

func _teleport(pos: Vector3) -> void:
	_player.global_position = pos
	_player.linear_velocity = Vector3.ZERO
	_player.reset_physics_interpolation()

func _one_live_enemy_per_type() -> Array:
	var by_key := {}
	for e in _lm.find_children("*", "EnemyDrone", true, false):
		if e.visible and e.spawns_directly() and not by_key.has(e.enemy_key):
			by_key[e.enemy_key] = e  # the line roster, by the def's own fact
	return by_key.values()

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
			"an enemy with the player 15 m away must chase, peak move %0.2f m (%s, level %d)"
				% [moved, enemy.enemy_key, level])

func test_planet_1_enemies_close_on_the_player():
	await _assert_enemies_close_on_player(1)

func test_planet_2_enemies_close_on_the_player():
	await _assert_enemies_close_on_player(10)
