extends GutTest
## Faucet Principle, tier 1 (death-spawn minions + lootboxes). Everything a level
## can contain is pre-instantiated during the build: an enemy's death-spawn
## minions sit dormant under the SAME room container as their parent, and one
## lootbox per enemy sits dormant under the LevelManager. Play only flips dormant
## <-> active — nothing is instantiated or freed while a level runs.
##
## These tests pin, through the real build pathway (LevelManager.generate_level):
##   - minions pre-exist dormant under their parent's room, not the scene root;
##   - killing the parent activates them (all three dormancy flags flip on);
##   - a killed minion's enemy_killed reaches GameManager (the reward path);
##   - a lootbox drop activates a pre-built box with no new instantiate (the
##     LevelManager child count is constant across a drop);
##   - build-time wiring works with process running — no per-frame scan rewires.

const EYE_DRONE_ID := 3   # EnemyType::ALL index of the EyeDrone
const SPAWN_DRONE_ID := 5 # ... and of the SpawnDrone it coughs up on death

## Stub UI layer for a minimal GameManager scene (mirrors test_level_generation).
class UIStub:
	extends CanvasLayer
	@warning_ignore("unused_signal")
	signal new_game_selected
	@warning_ignore("unused_signal")
	signal continue_selected
	@warning_ignore("unused_signal")
	signal sbs_toggled(enabled: bool)
	@warning_ignore("unused_signal")
	signal msaa_toggled(enabled: bool)
	@warning_ignore("unused_signal")
	signal resume_selected
	@warning_ignore("unused_signal")
	signal quit_selected
	@warning_ignore("unused_signal")
	signal continue_pressed
	@warning_ignore("unused_signal")
	signal buy_pressed
	@warning_ignore("unused_signal")
	signal return_pressed
	@warning_ignore("unused_signal")
	signal ship_color_selected(id: int)
	@warning_ignore("unused_signal")
	signal bestiary_paged(delta: int)
	func show_death(_a: String, _b: String, _c: int) -> void: pass
	func show_ship_select(_id: int) -> void: pass
	func show_bestiary(_t: String, _b: String, _p: String, _h: String) -> void: pass
	func begin_briefing() -> void: pass
	func hide_bestiary() -> void: pass
	# HUD contract: GameManager.update_hud() calls these every frame in Playing.
	func update_health(_h: float, _m: float) -> void: pass
	func update_shield(_c: float, _m: float) -> void: pass
	func update_power_mode(_m: int) -> void: pass
	func update_components(_c: int) -> void: pass
	func update_organics(_o: int) -> void: pass
	func update_laser(_n: String, _c: Color) -> void: pass
	func update_level(_l: int) -> void: pass


# --- Helpers ---

## Build a real level at `level` and return the LevelManager, choosing the first
## seed (from a small span) whose build actually places an EyeDrone. Returns null
## if no seed did — the caller pass/skips so the suite never flakes on RNG.
func _level_with_eye_drone(level: int) -> LevelManager:
	for seed in range(1, 60):
		var lm := LevelManager.new()
		lm.current_level = level
		add_child_autofree(lm)
		lm.generate_level(seed, 8)
		if not _find_enemies_of_type(lm, EYE_DRONE_ID).is_empty():
			return lm
		lm.free()
	return null

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
	var lm := _level_with_eye_drone(3)
	if lm == null:
		pass_test("no EyeDrone placed across the seed span — skipped")
		return
	var eye: RigidBody3D = _find_enemies_of_type(lm, EYE_DRONE_ID).front()
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
	var lm := _level_with_eye_drone(3)
	if lm == null:
		pass_test("no EyeDrone placed across the seed span — skipped")
		return
	var eye: RigidBody3D = _find_enemies_of_type(lm, EYE_DRONE_ID).front()
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
	# enemy_killed to the mediator and pays the 1000-component reward — with
	# process running the whole time (nothing re-scans to make it work).
	var root := Node3D.new()
	add_child_autofree(root)
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI"]:
		var stub := UIStub.new()
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

	var enemies := lm.find_children("*", "EnemyDrone", true, false)
	assert_gt(enemies.size(), 0, "the build must place enemies")
	# Confirm the enemy is genuinely nested under a room container (the depth the
	# old two-level scan reached but this test also exercises via recursion).
	var enemy: RigidBody3D = enemies.front()
	assert_not_null(_room_container_of(lm, enemy),
		"the enemy must live under a room container, not directly under LevelManager")

	var components_before: int = gm.get_components()
	enemy.take_damage(1000.0)
	await wait_physics_frames(3, "let the kill route to the mediator")
	assert_eq(gm.get_components(), components_before + 1000,
		"a container-nested enemy's kill must reach the mediator via the build-time wire")


# --- Lootboxes: pre-built dormant under the level, drop is a flip not a spawn ---

func test_lootboxes_pre_exist_dormant_and_drop_without_instantiate():
	# One lootbox per enemy, pre-built dormant under the LevelManager. Killing an
	# enemy drops (activates) its box; the LevelManager's child count must not
	# change across the drop — no per-drop instantiate remains.
	var lm := LevelManager.new()
	lm.current_level = 2
	add_child_autofree(lm)
	lm.generate_level(4242, 8)
	# Dormancy flags land on the deferred boundary — allow one flush post-build.
	await wait_process_frames(1)

	var boxes := lm.find_children("*", "Lootbox", true, false)
	assert_gt(boxes.size(), 0, "a level with enemies pre-builds lootboxes")
	for box in boxes:
		assert_false(box.visible, "a pre-built lootbox is dormant (invisible)")
		assert_false(box.monitoring, "a pre-built lootbox does not monitor for the player")
		assert_eq(box.get_parent(), lm,
			"lootboxes parent under the level (persist across rooms), not a room container")

	var enemies := lm.find_children("*", "EnemyDrone", true, false)
	assert_gt(enemies.size(), 0, "seed must place enemies")
	var child_count_before := lm.get_child_count()

	var enemy: RigidBody3D = enemies.front()
	enemy.take_damage(1000.0)
	await wait_physics_frames(3, "let the enemy die and drop its box")

	assert_eq(lm.get_child_count(), child_count_before,
		"dropping a box must reuse a pre-built one — the level's child count is constant")
	var live_boxes := 0
	for box in lm.find_children("*", "Lootbox", true, false):
		if box.visible:
			live_boxes += 1
	assert_eq(live_boxes, 1, "exactly one box (the dead enemy's) is now live")
