extends GutTest
## Faucet Principle, tier 1 (death-spawn minions + currency caches). Everything a
## level can contain is pre-instantiated during the build: an enemy's death-spawn
## minions sit dormant under the SAME room container as their parent, and one
## blue currency cache per enemy sits dormant under the LevelManager. Play only
## flips dormant <-> active — nothing is instantiated or freed while a level runs.
##
## These tests pin, through the real build pathway (LevelManager.generate_level):
##   - minions pre-exist dormant under their parent's room, not the scene root;
##   - killing the parent activates them (all three dormancy flags flip on);
##   - a kill credits NOTHING directly; collecting the dropped cache credits
##     exactly the type's reward (the pickup-only economy);
##   - a cache drop activates a pre-built cache with no new instantiate (the
##     LevelManager child count is constant across a drop);
##   - build-time wiring works with process running — no per-frame scan rewires.

const EYE_DRONE_ID := 3   # EnemyType::ALL index of the EyeDrone
const SPAWN_DRONE_ID := 5 # ... and of the SpawnDrone it coughs up on death

## Seeds are PINNED, never scanned: whether a seed produces a property is a
## pure model question, verified fast on the Rust side. The shell builds one
## level per scenario. Mirrors (level_assembly::tests):
##   EYE_DRONE_SEED       <-> pinned_gut_seed_places_an_eye_drone
##   GREEN_CACHE_RUN_SEED <-> pinned_gut_run_seed_places_a_loot_container
const EYE_DRONE_SEED := 1
const GREEN_CACHE_RUN_SEED := 1

const UiStub := preload("res://tests/helpers/ui_stub.gd")


# --- Helpers ---

## Build the one pinned level that contains an EyeDrone (see EYE_DRONE_SEED).
func _eye_drone_level() -> LevelManager:
	var lm := LevelManager.new()
	lm.current_level = 3
	add_child_autofree(lm)
	lm.generate_level(EYE_DRONE_SEED, 8)
	return lm

func _find_enemies_of_type(lm: Node, type_id: int) -> Array:
	var out := []
	for e in lm.find_children("*", "EnemyDrone", true, false):
		if e.enemy_type_id == type_id:
			out.append(e)
	return out

## The room container a node lives under: LevelManager's direct child that is an
## ancestor of `node`. Enemies and their minions share one such container.
func _room_container_of(lm: Node, node: Node) -> Node:
	for room in lm.get_children():
		if room.is_ancestor_of(node):
			return room
	return null


# --- Death minions are pre-built dormant under the parent's room ---

func test_death_minions_pre_exist_dormant_under_parent_room():
	var lm := _eye_drone_level()
	var eyes := _find_enemies_of_type(lm, EYE_DRONE_ID)
	assert_gt(eyes.size(), 0,
		"the pinned seed places an EyeDrone (Rust: pinned_gut_seed_places_an_eye_drone)")
	var eye: RigidBody3D = eyes.front()
	var eye_room := _room_container_of(lm, eye)
	assert_not_null(eye_room, "the EyeDrone must live under a room container")
	# Dormancy flags land on the deferred boundary — allow one flush post-build.
	await wait_process_frames(1)

	# Its SpawnDrone minion is pre-built in the SAME room, dormant — it does not
	# escape to the scene root the way the old death-path instantiate did.
	var minions := _find_enemies_of_type(eye_room, SPAWN_DRONE_ID)
	assert_eq(minions.size(), 1,
		"an EyeDrone reserves exactly one dormant SpawnDrone minion under its room")
	var minion: RigidBody3D = minions.front()
	assert_false(minion.visible, "a dormant minion is invisible")
	assert_eq(minion.process_mode, Node.PROCESS_MODE_DISABLED,
		"a dormant minion does not process (its AI is gated)")
	assert_eq(minion.collision_layer, 0, "a dormant minion does not collide (layer zeroed)")
	assert_eq(minion.collision_mask, 0, "a dormant minion does not sense (mask zeroed)")

	# No minion leaked to the scene root.
	var root_minions := 0
	for child in get_tree().root.get_children():
		if child is RigidBody3D and child.get_class() == "EnemyDrone":
			root_minions += 1
	assert_eq(root_minions, 0, "no minion may be parented at the scene root")


func test_killing_parent_activates_its_minions():
	var lm := _eye_drone_level()
	var eyes := _find_enemies_of_type(lm, EYE_DRONE_ID)
	assert_gt(eyes.size(), 0,
		"the pinned seed places an EyeDrone (Rust: pinned_gut_seed_places_an_eye_drone)")
	var eye: RigidBody3D = eyes.front()
	var eye_room := _room_container_of(lm, eye)
	var minion: RigidBody3D = _find_enemies_of_type(eye_room, SPAWN_DRONE_ID).front()
	# Dormancy flags land on the deferred boundary — allow one flush post-build.
	await wait_process_frames(1)
	assert_false(minion.visible, "sanity: minion starts dormant")

	eye.take_damage(1000.0) # EyeDrone has 5 HP — lethal
	await wait_physics_frames(3, "let the parent's death activate its minions")

	assert_true(is_instance_valid(minion), "the minion survives its parent's death")
	assert_true(minion.visible, "the parent's death makes its minion visible")
	assert_eq(minion.process_mode, Node.PROCESS_MODE_INHERIT,
		"an activated minion processes (its AI runs)")
	assert_ne(minion.collision_layer, 0, "an activated minion collides again")
	# It stays under the same room container — activation is a flip, not a reparent.
	assert_eq(_room_container_of(lm, minion), eye_room,
		"an activated minion stays under its parent's room container")


# --- Build-time wiring: a container-nested kill reaches the mediator ---

func test_build_time_wire_routes_room_container_kill_to_mediator():
	# Everything a level can emit is wired ONCE at the end of the build, not per
	# frame (the per-frame connect_spawned_entities scan is deleted). Enemies —
	# and the death-spawn minions that share their depth — live several levels
	# down under room containers, so the wire must recurse. This proves it does:
	# a full GameManager-driven build, then a container-nested enemy kill routes
	# enemy_killed to the mediator — crediting NOTHING — and collecting the
	# dropped cache credits exactly the type's reward, with process running the
	# whole time (nothing re-scans to make it work).
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

	# Drive the FSM into Playing — this builds the level AND wires it once.
	gm.start_new_game()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	assert_eq(gm.get_phase_name(), "Playing", "must reach Playing (which builds + wires)")
	await wait_process_frames(3, "the sector build defers past the loading veil's frame")

	var enemies := lm.find_children("*", "EnemyDrone", true, false)
	assert_gt(enemies.size(), 0, "the build must place enemies")
	# Confirm the enemy is genuinely nested under a room container (the depth the
	# old two-level scan reached but this test also exercises via recursion).
	var enemy: RigidBody3D = enemies.front()
	assert_not_null(_room_container_of(lm, enemy),
		"the enemy must live under a room container, not directly under LevelManager")

	var components_before: int = gm.get_components()
	var reward: int = enemy.cache_reward()
	assert_gt(reward, 0, "every enemy type carries a positive cache reward")
	enemy.take_damage(1000.0)
	await wait_physics_frames(3, "let the kill route to the mediator")
	assert_eq(gm.get_components(), components_before,
		"a kill credits nothing directly — the reward rides the dropped cache")

	# The bound blue cache activated under the LevelManager; fly-through = collect().
	var live_cache: Node = null
	for cache in lm.find_children("*", "CurrencyCache", true, false):
		if cache.get_parent() == lm and cache.visible:
			live_cache = cache
			break
	assert_not_null(live_cache, "the kill must activate the enemy's bound cache")
	live_cache.collect()
	await wait_physics_frames(2, "let the pickup route to the mediator")
	assert_eq(gm.get_components(), components_before + reward,
		"collecting the cache credits exactly the type's reward via the build-time wire")


# --- Currency caches: blue pool pre-built dormant under the level, green
# --- loot-spawn caches live under their rooms; a drop is a flip not a spawn ---

## The level's blue-cache pool: caches parented directly under the LevelManager
## (green loot-spawn caches parent under room containers instead).
func _blue_caches(lm: Node) -> Array:
	var out := []
	for cache in lm.find_children("*", "CurrencyCache", true, false):
		if cache.get_parent() == lm:
			out.append(cache)
	return out

func test_caches_pre_exist_dormant_and_drop_without_instantiate():
	# One blue cache per enemy, pre-built dormant under the LevelManager. Killing
	# an enemy drops (activates) its cache; the LevelManager's child count must
	# not change across the drop — no per-drop instantiate remains.
	var lm := LevelManager.new()
	lm.current_level = 2
	add_child_autofree(lm)
	lm.generate_level(4242, 8)
	# Dormancy flags land on the deferred boundary — allow one flush post-build.
	await wait_process_frames(1)

	var blue := _blue_caches(lm)
	assert_gt(blue.size(), 0, "a level with enemies pre-builds blue caches")
	for cache in blue:
		assert_false(cache.visible, "a pre-built blue cache is dormant (invisible)")
		assert_false(cache.monitoring, "a pre-built blue cache does not monitor for the player")

	var enemies := lm.find_children("*", "EnemyDrone", true, false)
	assert_gt(enemies.size(), 0, "seed must place enemies")
	var child_count_before := lm.get_child_count()

	var enemy: RigidBody3D = enemies.front()
	enemy.take_damage(1000.0)
	await wait_physics_frames(3, "let the enemy die and drop its cache")

	assert_eq(lm.get_child_count(), child_count_before,
		"dropping a cache must reuse a pre-built one — the level's child count is constant")
	var live_caches := 0
	for cache in _blue_caches(lm):
		if cache.visible:
			live_caches += 1
	assert_eq(live_caches, 1, "exactly one blue cache (the dead enemy's) is now live")


func test_green_cache_collect_credits_organics_only():
	# Green loot-spawn caches are placed live under their room containers during
	# the build; flying through one credits organics (never components), through
	# the same build-time wire as everything else. One pinned run — the seed's
	# loot-spawn property is verified on the Rust side.
	var root := Node3D.new()
	add_child_autofree(root)
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	var gm := GameManager.new()
	gm.fixed_seed = GREEN_CACHE_RUN_SEED
	root.add_child(lm)
	root.add_child(gm)
	# Real persistence leaks profiles across test runs; start fresh.
	gm.clear_save_for_tests()

	gm.start_new_game()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	await wait_process_frames(3, "the sector build defers past the loading veil's frame")

	var green: Node = null
	for cache in lm.find_children("*", "CurrencyCache", true, false):
		if cache.get_parent() != lm:
			green = cache
			break
	assert_not_null(green,
		"the pinned run places a green cache (Rust: pinned_gut_run_seed_places_a_loot_container)")

	var organics_before: int = gm.get_organics()
	var components_before: int = gm.get_components()
	green.collect()
	await wait_physics_frames(2, "let the pickup route to the mediator")
	assert_gt(gm.get_organics(), organics_before,
		"collecting a green cache credits the organics account")
	assert_eq(gm.get_components(), components_before,
		"a green cache never credits components")
