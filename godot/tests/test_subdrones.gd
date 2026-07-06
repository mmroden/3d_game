extends GutTest
## The Hive's subdrones: SQUAD_SIZE pre-built dormant per level (Faucet tier
## 1 — a launch is a dormancy flip, never a spawn), they hunt nearby enemies
## with player-faction bolts through the shared pool, and player-faction fire
## damages enemies while enemy fire never does. Regen/steering math is pinned
## in Rust (armament.rs); this suite covers the shell.

const PINNED_SEED := 1  # mirrors level_assembly's pinned GUT seed
const SQUAD_SIZE := 2   # mirrors armament::subdrone::SQUAD_SIZE


func _level() -> LevelManager:
	var lm := LevelManager.new()
	lm.current_level = 2
	add_child_autofree(lm)
	lm.generate_level(PINNED_SEED, 8)
	return lm


func test_squad_is_prebuilt_dormant_and_launch_is_a_flip():
	var lm := _level()
	await wait_process_frames(1)

	var drones := get_tree().get_nodes_in_group("player_drones")
	assert_eq(drones.size(), SQUAD_SIZE, "the whole squad exists after the build")
	for drone in drones:
		assert_false(drone.is_live(), "a pre-built drone starts folded in the bay")
		assert_false(drone.visible, "a dormant drone is invisible")

	var child_count_before := lm.get_child_count()
	var drone: Node3D = drones.front()
	drone.activate_at(Vector3.ZERO, 1.0)
	await wait_physics_frames(2, "let the dormancy flip land")

	assert_true(drone.is_live(), "the launch deploys the drone")
	assert_true(drone.visible, "a deployed drone is visible")
	assert_eq(lm.get_child_count(), child_count_before,
		"a launch is a flip — nothing was instantiated")


func test_deployed_drone_acquires_and_opens_fire():
	var lm := _level()
	await wait_process_frames(1)
	var pool: Node = lm.find_children("*", "BoltPool", false, false).front()

	var enemies := lm.find_children("*", "EnemyDrone", true, false)
	assert_gt(enemies.size(), 0, "the pinned seed places enemies")
	var enemy: RigidBody3D = enemies.front()

	# Park a drone right next to the enemy and let it hunt. (Damage routing is
	# proven by the direct-fire kill test below; this pins acquisition+firing.)
	var drone: Node3D = get_tree().get_nodes_in_group("player_drones").front()
	drone.activate_at(enemy.global_position + Vector3(3, 0, 0), 1.0)

	# A bolt at 3m lives only a few frames before detonating on its target, so
	# poll rather than sampling one instant between shots.
	var saw_fire := false
	for _i in range(90):
		await get_tree().physics_frame
		if pool.live_count() > 0:
			saw_fire = true
			break
	assert_true(saw_fire,
		"the deployed drone fires through the shared pool at the enemy in range")


func test_player_faction_bolt_kills_enemies_and_enemy_bolts_do_not():
	var lm := _level()
	await wait_process_frames(1)
	var pool: Node = lm.find_children("*", "BoltPool", false, false).front()
	assert_not_null(pool, "the level builds its bolt pool")

	var enemies := lm.find_children("*", "EnemyDrone", true, false)
	var enemy: RigidBody3D = enemies.front()

	# Point-blank at the enemy's REAL physics position (a mid-simulation
	# RigidBody teleport doesn't reliably commit under Jolt, and rooms are
	# full of props that eat longer shots).
	# An enemy-faction bolt passes through enemies harmlessly, even a lethal one.
	pool.fire(enemy.global_position + Vector3(0, 0, 1.5), Vector3(0, 0, -8), 1000.0)
	await wait_physics_frames(30, "let the enemy bolt fly through")
	assert_true(is_instance_valid(enemy), "enemy fire never hurts enemies")

	# A lethal player-faction bolt detonates on them.
	pool.fire_player(enemy.global_position + Vector3(0, 0, 1.5), Vector3(0, 0, -8), 1000.0)
	await wait_physics_frames(30, "let the player bolt land and the death chain run")
	assert_false(is_instance_valid(enemy), "player fire kills enemies")


func test_homing_bolt_curves_into_a_target_it_was_not_aimed_at():
	var lm := _level()
	await wait_process_frames(1)
	var pool: Node = lm.find_children("*", "BoltPool", false, false).front()
	var enemy: RigidBody3D = lm.find_children("*", "EnemyDrone", true, false).front()

	# Fired from beside the enemy, aimed PERPENDICULAR to it: a ballistic
	# bolt would sail past; the homing lock bends it in for the kill. Slow
	# enough that the turn radius (speed / TURN_RATE = 4/2.5 = 1.6m) fits
	# inside the 4m offset, close enough that props stay out of the curve.
	var start: Vector3 = enemy.global_position + Vector3(4, 0, 0)
	pool.fire_homing(start, Vector3(0, 0, 4), 1000.0, enemy.get_instance_id())
	await wait_physics_frames(150, "let the bolt curve home")

	assert_false(is_instance_valid(enemy),
		"the homing bolt found (and killed) a target it was never aimed at")
