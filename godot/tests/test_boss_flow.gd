extends GutTest
## The staged boss fight, full stack (B6): level 3 (seed 1) is the first
## mid-boss level. The arena arrives sealed-off-able and dormant; crossing
## into it engages the fight (gate slams, escorts rise); killing the boss
## alone opens nothing; collecting the reward opens the gate and lights the
## portal. Level 1 pins the regression: no gate, portal live as always.
##
## State ids mirror void_logic::boss_fight::BossFightState::id():
## -1 = no boss level, 0 Dormant, 1 Engaged, 2 Defeated, 3 RewardCollected.

const UiStub := preload("res://tests/helpers/ui_stub.gd")

var _gm: GameManager
var _lm: LevelManager
var _player: ShipController

func _build_stack() -> void:
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
	_gm.fixed_seed = 1  # the pinned seed — B4's Rust anchors pin this level
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

func _advance_one_level() -> void:
	_gm.on_portal_entered()
	_gm.advance_to_shop()
	_gm.advance_to_next_level()
	_walk_to_playing()

func _find_enemy_of_type(type_id: int) -> RigidBody3D:
	for e in _lm.find_children("*", "EnemyDrone", true, false):
		if e.enemy_type_id == type_id:
			return e
	return null

func _boss_gate() -> Node:
	var gates := _lm.find_children("*", "BossGate", true, false)
	return gates.front() if not gates.is_empty() else null

func _portal() -> Node:
	var portals := _lm.find_children("*", "Portal", true, false)
	return portals.front() if not portals.is_empty() else null

func _live_caches() -> Array:
	var out := []
	for c in _lm.find_children("*", "CurrencyCache", false, false):
		if c.visible:
			out.append(c)
	return out

func _teleport(pos: Vector3) -> void:
	_player.global_position = pos
	_player.linear_velocity = Vector3.ZERO
	_player.reset_physics_interpolation()

func test_the_boss_fight_walks_seal_kill_collect_open():
	_build_stack()
	_gm.start_new_game()
	_walk_to_playing()
	await wait_process_frames(2)

	# --- Level 1: no boss anywhere (regression pin) ---
	assert_eq(_gm.boss_fight_state(), -1, "level 1 stages no boss fight")
	assert_null(_boss_gate(), "no gate on a regular level")
	var portal := _portal()
	assert_not_null(portal, "regular levels keep their portal")
	if portal != null:
		assert_true(portal.monitoring, "the regular portal is live immediately")

	# --- Walk to level 3, the first mid-boss level ---
	_advance_one_level()
	_advance_one_level()
	assert_eq(_gm.get_current_level(), 3, "two advances reach level 3")
	await wait_process_frames(2)

	assert_eq(_gm.boss_fight_state(), 0, "the fight waits dormant")
	var gate := _boss_gate()
	assert_not_null(gate, "the arena gate is pre-built on a boss level")
	if gate == null:
		return
	assert_false(gate.is_sealed(), "the approach is open inbound")
	portal = _portal()
	assert_not_null(portal, "the portal is pre-built (Faucet) …")
	if portal != null:
		assert_false(portal.monitoring, "… but dark until the fight resolves")
	var boss := _find_enemy_of_type(11)  # BossBrute — rel-3 stages the Brute
	assert_not_null(boss, "the Siege Mech waits in the arena")
	if boss == null:
		return

	# --- Entry engages: gate slams, fight is on ---
	_teleport(_lm.boss_arena_center())
	await wait_physics_frames(5, "the arena trigger must see the player")
	assert_eq(_gm.boss_fight_state(), 1, "crossing in engages the fight")
	assert_true(gate.is_sealed(), "the gate slams behind the player")

	# --- The kill alone opens nothing ---
	boss.take_damage(100000.0)
	await wait_physics_frames(5, "the death report must reach the manager")
	assert_eq(_gm.boss_fight_state(), 2, "the boss is down")
	assert_true(gate.is_sealed(), "the kill is not the end — the pickup is")
	if portal != null:
		assert_false(portal.monitoring, "no exit before the reward")

	# --- ALL boss loot gathered closes the fight — not any one pickup ---
	# The mid-boss sheds a pile: its bound blue cache plus the consolation
	# caches. Grabbing SOME of it must leave the arena sealed (owner's call
	# 2026-07-05); only the complete set opens the way.
	var drops := _live_caches()
	assert_eq(drops.size(), 4, "the mid-boss pile is four caches")
	if drops.is_empty():
		return
	_teleport(drops[0].global_position)
	await wait_physics_frames(10, "the first pickup must register")
	await wait_process_frames(2)
	assert_eq(_gm.boss_fight_state(), 2,
		"one drop of the pile is not the reward beat — still Defeated")
	assert_true(gate.is_sealed(), "the arena stays sealed over the rest")

	for c in _live_caches():
		_teleport(c.global_position)
		await wait_physics_frames(10, "each pickup must register")
	await wait_process_frames(2)  # the arena-open flips ride call_deferred
	assert_eq(_gm.boss_fight_state(), 3, "the WHOLE pile closes the fight")
	assert_false(gate.is_sealed(), "the arena opens")
	if portal != null:
		assert_true(portal.monitoring, "the way onward appears")

func test_the_planet_final_boss_drops_the_hull_container():
	# Level 6 (seed 1) is planet 1's final boss: its cache is the RED hull
	# container. Collecting it grants a random unowned hull (profile-
	# persisted) — the only way hulls enter the fleet now.
	_build_stack()
	_gm.start_new_game()
	_walk_to_playing()
	await wait_process_frames(2)
	assert_eq(_gm.owned_hull_count(), 0, "a fresh profile owns no extra hulls")

	for _lvl in range(5):
		_advance_one_level()
	assert_eq(_gm.get_current_level(), 6, "five advances reach the planet final")
	await wait_process_frames(2)

	assert_eq(_gm.boss_fight_state(), 0, "the final fight waits dormant")
	var boss := _find_enemy_of_type(12)  # BossLatcher — rel-6 stages the Latcher
	assert_not_null(boss, "the Lamprey Mech guards the planet exit")
	if boss == null:
		return

	_teleport(_lm.boss_arena_center())
	await wait_physics_frames(5, "engage")
	assert_eq(_gm.boss_fight_state(), 1)

	boss.take_damage(100000.0)
	await wait_physics_frames(5, "the death report must land")
	assert_eq(_gm.boss_fight_state(), 2)

	var red: Node3D = null
	for c in _lm.find_children("*", "CurrencyCache", false, false):
		if c.visible:
			red = c
			break
	assert_not_null(red, "the container dropped")
	if red == null:
		return
	_teleport(red.global_position)
	await wait_physics_frames(10, "the pickup must register")
	await wait_process_frames(2)
	assert_eq(_gm.boss_fight_state(), 3, "the reward closes the fight")
	assert_eq(_gm.owned_hull_count(), 1,
		"the red container granted a hull, persisted to the profile")
