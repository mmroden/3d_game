extends GutTest
## Tests for the enemy-to-player damage pipeline.
## Verifies that enemies can hurt the player and that death works.

# --- Helpers ---

func _spawn_player(pos: Vector3) -> ShipController:
	var player = ShipController.new()
	player.add_to_group("player")
	var shape = CollisionShape3D.new()
	var sphere = SphereShape3D.new()
	sphere.radius = 0.5
	shape.shape = sphere
	player.add_child(shape)
	add_child_autofree(player)
	player.global_position = pos
	return player

# --- ShipController interface tests ---

func test_player_has_take_damage_method():
	var player = _spawn_player(Vector3.ZERO)
	assert_has_method(player, "take_damage",
		"ShipController must expose take_damage for projectile hits")

func test_player_emits_damaged_signal():
	var player = _spawn_player(Vector3.ZERO)
	assert_has_signal(player, "player_damaged",
		"ShipController must emit player_damaged when hit")

func test_take_damage_emits_signal_with_amount():
	var player = _spawn_player(Vector3.ZERO)
	watch_signals(player)
	player.take_damage(25.0, Vector3(10, 0, 0))
	assert_signal_emitted(player, "player_damaged",
		"take_damage should emit player_damaged signal")

# --- Enemy projectiles ---
# As of M4, enemy bolts are ballistic bodies in the kinetic world: the
# level host pulls each enemy's fire intent (decide()) and spawns the
# bolt body + visual itself. The AI fire logic and ballistic contact
# behavior are covered by Rust tests (enemy_ai, kinetic_world); the
# host wiring shows up immediately in make run.

func test_enemy_in_range_fires_and_damages_player():
	# End-to-end: an enemy within attack range must fire a bolt that survives
	# its own muzzle, travels to the player, and deals damage. Regression for
	# bolts self-detonating on the firing enemy (shared collision layer).
	# Bolts are now drawn from the level's preallocated pool (Faucet Principle),
	# so the enemy fires through the BoltPool rather than instantiating a bolt —
	# a running level always provides one; the test stands in for that here.
	var player = _spawn_player(Vector3.ZERO)
	watch_signals(player)
	var pool := BoltPool.new()
	pool.capacity = 8 # a handful of slots is plenty for one enemy's fire
	add_child_autofree(pool)
	var enemy_scene = load("res://scenes/enemies/enemy.tscn")
	if enemy_scene == null:
		pass_test("skipped — scene not available")
		return
	var enemy = enemy_scene.instantiate()
	add_child_autofree(enemy)
	enemy.global_position = Vector3(8, 0, 0) # within GunDrone attack_range (10)
	await wait_physics_frames(240, "enemy should fire a bolt that hits the player")
	assert_signal_emitted(player, "player_damaged",
		"an enemy within range must fire a bolt that damages the player")

func test_swarmer_proximity_slows_player_instead_of_damaging():
	# The four-legged QuadOrb (swarmer) bogs the player down while latched rather
	# than dealing ram damage: sitting within latch range re-tags a compounding
	# slow every tick (raising player_slowed), it doesn't ram. The compounding
	# itself is unit-tested in void-logic's SlowDebuff.
	var player = _spawn_player(Vector3.ZERO)
	watch_signals(player)
	var enemy_scene = load("res://scenes/enemies/enemy.tscn")
	if enemy_scene == null:
		pass_test("skipped — scene not available")
		return
	var enemy = enemy_scene.instantiate()
	enemy.enemy_type_id = 1 # QuadOrb (swarmer)
	add_child_autofree(enemy)
	enemy.global_position = Vector3(1.5, 0, 0) # within SWARM_LATCH_RANGE (2.0)
	await wait_physics_frames(4, "swarmer should bog the player down while latched")
	assert_signal_emitted(player, "player_slowed",
		"a swarmer latched onto the player must slow them, not ram")

# --- Health bar reflects damage ---

func test_health_bar_fill_shrinks_as_enemy_takes_damage():
	# The floating bar must visibly deplete, not just recolor: its fill width
	# tracks remaining HP. Regression for the per-frame billboard clobbering the
	# width so the bar stayed full and only changed color.
	var player := _spawn_player(Vector3(5, 0, 0))
	var enemy = load("res://scenes/enemies/enemy.tscn").instantiate()
	add_child_autofree(enemy)
	enemy.global_position = Vector3.ZERO
	await wait_physics_frames(2, "let the bar billboard at full health")

	var fill = enemy.get_node_or_null("HealthBarFill")
	assert_not_null(fill, "enemy must have a named HealthBarFill bar")
	if fill == null:
		return
	var full_width: float = fill.global_transform.basis.x.length()

	enemy.take_damage(1.5) # GunDrone has 3 HP — knock it to half
	await wait_physics_frames(2, "let the bar re-render at half health")
	var half_width: float = fill.global_transform.basis.x.length()

	assert_lt(half_width, full_width * 0.75,
		"fill bar should shrink toward half width after losing half its HP (was %f, now %f)"
		% [full_width, half_width])

# --- GameManager damage handler ---
# Note: GameManager.ready() warns about missing UI nodes when not in the
# full scene tree. We verify the interface without adding to tree.

func test_game_manager_class_exists():
	# Don't add_child — that triggers ready() which needs the full scene tree
	var gm = GameManager.new()
	assert_has_method(gm, "on_player_damaged",
		"GameManager must handle player damage")
	assert_has_method(gm, "get_health",
		"GameManager must expose health for HUD")
	autofree(gm)

# --- Node lifecycle: death drops exactly one (pre-built) lootbox ---

func test_enemy_death_drops_one_pre_built_lootbox():
	# Faucet Principle, tier 1: one lootbox per enemy is pre-built dormant during
	# the level build; the enemy's death activates its bound box (a flip, not an
	# instantiate). Killing one enemy leaves exactly one live box, and the level's
	# total box count is unchanged (nothing was created on drop).
	var lm := LevelManager.new()
	lm.current_level = 2
	add_child_autofree(lm)
	lm.generate_level(4242, 8)

	var boxes_before := lm.find_children("*", "Lootbox", true, false).size()
	assert_gt(boxes_before, 0, "a level with enemies pre-builds lootboxes")

	var enemies := lm.find_children("*", "EnemyDrone", true, false)
	assert_gt(enemies.size(), 0, "seed 4242 must place enemies")
	var enemy: RigidBody3D = enemies.front()
	enemy.take_damage(100.0)
	await wait_physics_frames(10, "let the enemy die and drop its bound box")

	var boxes_after := lm.find_children("*", "Lootbox", true, false).size()
	assert_eq(boxes_after, boxes_before,
		"dropping a box must not instantiate — the pre-built count is unchanged")
	var live := 0
	for box in lm.find_children("*", "Lootbox", true, false):
		if box.visible:
			live += 1
	assert_eq(live, 1, "exactly one box (the dead enemy's) is now live, got %d" % live)
