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

const UiStub := preload("res://tests/helpers/ui_stub.gd")

func test_enemy_fires_inside_the_full_game_stack():
	# Playtest regression report (2026-07-03): enemies stopped firing in real
	# play. The bare-pipeline test above passes, so this reproduces the FULL
	# stack: GameManager-driven build, Playing phase, real ShipController —
	# then parks the player beside a shooter and expects fire.
	var root := Node3D.new()
	add_child_autofree(root)
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	var player := ShipController.new()
	player.name = "Player"
	var shape := CollisionShape3D.new()
	shape.name = "CollisionShape3D"
	shape.shape = SphereShape3D.new()
	player.add_child(shape)
	var gm := GameManager.new()
	gm.fixed_seed = 1  # the pinned seed
	root.add_child(lm)
	root.add_child(player)
	root.add_child(gm)
	gm.clear_save_for_tests()

	gm.start_new_game()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	assert_eq(gm.get_phase_name(), "Playing", "must reach Playing")
	await wait_process_frames(2)

	# Park the player right beside a SHOOTER (GunDrone id 0 — QuadOrbs swarm,
	# they never fire). Node-transform teleport is what the AI reads.
	var shooter: RigidBody3D = null
	for e in lm.find_children("*", "EnemyDrone", true, false):
		if e.enemy_type_id == 0:
			shooter = e
			break
	if shooter == null:
		pass_test("pinned seed placed no GunDrone at level 1 — skipped")
		return
	player.global_position = shooter.global_position + Vector3(0, 0, 5)
	player.reset_physics_interpolation()

	var pool: Node = lm.find_children("*", "BoltPool", false, false).front()
	var fired := false
	for _i in range(300):  # 5 seconds
		await get_tree().physics_frame
		if pool.live_count() > 0:
			fired = true
			break
	assert_true(fired,
		"a shooter with the player parked in range must open fire inside the full stack")


func test_player_trigger_damages_an_enemy_inside_the_full_game_stack():
	# Playtest regression (2026-07-03): a freshly bought hull fired nothing and
	# dealt no damage until a restart. The enemy half of the pipeline is pinned
	# above; this pins the player half the same way — real GameManager build,
	# Playing phase, aim at a live enemy, hold the trigger, expect its health
	# bar to deplete (or the enemy to die outright).
	var root := Node3D.new()
	add_child_autofree(root)
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	var player := ShipController.new()
	player.name = "Player"
	var shape := CollisionShape3D.new()
	shape.name = "CollisionShape3D"
	shape.shape = SphereShape3D.new()
	player.add_child(shape)
	var gm := GameManager.new()
	gm.fixed_seed = 1  # the pinned seed
	root.add_child(lm)
	root.add_child(player)
	root.add_child(gm)
	gm.clear_save_for_tests()

	gm.start_new_game()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	assert_eq(gm.get_phase_name(), "Playing", "must reach Playing")
	await wait_process_frames(2)

	var enemy: RigidBody3D = lm.find_children("*", "EnemyDrone", true, false).front()
	assert_not_null(enemy, "the pinned level must field at least one enemy")
	if enemy == null:
		return
	var fill = enemy.get_node_or_null("HealthBarFill")
	assert_not_null(fill, "the target needs a health bar to observe")
	if fill == null:
		return

	# Park in front of the target and aim down the -Z muzzle line.
	player.global_position = enemy.global_position + Vector3(0, 0, 5)
	player.look_at(enemy.global_position)
	player.reset_physics_interpolation()
	await wait_physics_frames(2, "let the bar render at full health")
	var full_width: float = fill.global_transform.basis.x.length()

	Input.action_press("fire")
	var hurt := false
	for i in range(300):  # 5 seconds
		await get_tree().physics_frame
		if not is_instance_valid(enemy) or not enemy.visible:
			hurt = true  # killed outright — dormancy flip
			break
		if fill.global_transform.basis.x.length() < full_width * 0.9:
			hurt = true
			break
		if i % 30 == 0 and is_instance_valid(enemy):  # the target drifts; re-aim
			player.look_at(enemy.global_position)
			player.reset_physics_interpolation()
	Input.action_release("fire")
	assert_true(hurt,
		"holding the trigger at a live enemy must deal damage inside the full stack")


func test_valkyrie_cannon_fires_only_once_owned():
	# The green keystone: buying the Valkyrie arms a heavy bolt alongside the
	# Vanguard's hitscan lasers. Before ownership the trigger spawns no bolts
	# at all (hitscan is beams, not pool slots) — so the pool is the witness.
	var player = _spawn_player(Vector3.ZERO)
	player.set_controls_enabled(true)
	var pool := BoltPool.new()
	pool.capacity = 8
	add_child_autofree(pool)
	await wait_physics_frames(1, "let the pool join its group")

	Input.action_press("fire")
	await wait_physics_frames(30, "an unowned cannon must stay silent")
	assert_eq(pool.live_count(), 0,
		"without the Valkyrie the trigger is hitscan only — no pooled bolts")

	player.set_valkyrie_owned(true)
	var fired := false
	for _i in range(120):
		await get_tree().physics_frame
		if pool.live_count() > 0:
			fired = true
			break
	Input.action_release("fire")
	assert_true(fired, "owning the Valkyrie arms the heavy bolt on the same trigger")


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

# --- Node lifecycle: death drops exactly one (pre-built) blue cache ---

func test_enemy_death_drops_one_pre_built_cache():
	# Faucet Principle, tier 1: one blue currency cache per enemy is pre-built
	# dormant during the level build; the enemy's death activates its bound cache
	# (a flip, not an instantiate). Killing one enemy leaves exactly one live
	# cache under the level, and the level's total cache count is unchanged
	# (nothing was created on drop). Green loot-spawn caches live under room
	# containers and start live, so the live-count check filters to the blue
	# pool (caches parented directly under the LevelManager).
	var lm := LevelManager.new()
	lm.current_level = 2
	add_child_autofree(lm)
	lm.generate_level(4242, 8)

	var caches_before := lm.find_children("*", "CurrencyCache", true, false).size()
	assert_gt(caches_before, 0, "a level with enemies pre-builds blue caches")

	var enemies := lm.find_children("*", "EnemyDrone", true, false)
	assert_gt(enemies.size(), 0, "seed 4242 must place enemies")
	var enemy: RigidBody3D = enemies.front()
	enemy.take_damage(100.0)
	await wait_physics_frames(10, "let the enemy die and drop its bound cache")

	var caches_after := lm.find_children("*", "CurrencyCache", true, false).size()
	assert_eq(caches_after, caches_before,
		"dropping a cache must not instantiate — the pre-built count is unchanged")
	var live := 0
	for cache in lm.find_children("*", "CurrencyCache", true, false):
		if cache.visible and cache.get_parent() == lm:
			live += 1
	assert_eq(live, 1, "exactly one blue cache (the dead enemy's) is now live, got %d" % live)
