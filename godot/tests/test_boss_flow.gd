extends GutTest
## The staged boss fight, full stack (B6). A boss level — FOUND, never pinned;
## which level carries a fight is grammar the owner tunes — arrives sealed-off-
## able and dormant; crossing into it engages the fight (gate slams, escorts
## rise); killing the boss alone opens nothing; collecting the reward opens the
## gate and lights the portal. The opening level pins the regression: no gate,
## portal live as always.
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

## Advance level-by-level until the run reaches a staged fight, so tests never
## pin WHICH level carries a boss (that is grammar the owner tunes freely).
func _advance_to_a_boss_level() -> bool:
	for _i in range(12):
		await wait_process_frames(2)  # the staging settles a couple frames in
		if _gm.boss_fight_state() != -1:
			return true
		_advance_one_level()
	await wait_process_frames(2)
	return _gm.boss_fight_state() != -1

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

	# --- The opening level: no boss anywhere (regression pin) ---
	assert_eq(_gm.boss_fight_state(), -1, "the opening level stages no boss fight")
	assert_null(_boss_gate(), "no gate on a regular level")
	var portal := _portal()
	assert_not_null(portal, "regular levels keep their portal")
	if portal != null:
		assert_true(portal.monitoring, "the regular portal is live immediately")

	# --- Walk to the first staged fight (found, never a pinned level) ---
	assert_true(await _advance_to_a_boss_level(), "a boss level must be reachable")
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
	var boss := _lm.staged_boss_node()
	assert_not_null(boss, "the staged boss waits in the arena")
	if boss == null:
		return
	# Pre-entry the boss is DORMANT (playtest 2026-07-06: a live boss could
	# be sniped from the corridor and die before the fight ever engaged):
	# intangible to the hitscan and invisible until the arena trigger rises it.
	assert_eq(boss.collision_layer, 0, "the staged boss is intangible before entry")
	assert_false(boss.visible, "the staged boss is invisible before entry")

	# Bruise the shield on the approach: the arena seal must clear it (owner's
	# call 2026-07-06 — otherwise waiting out the regen at the door is
	# strictly optimal play).
	_gm.on_player_damaged(20.0, Vector3.ZERO)
	assert_lt(_gm.get_shield(), _gm.get_max_shield(),
		"fixture sanity: the approach hit landed on the shield")

	# --- Entry engages: gate slams, fight is on, the boss rises ---
	_teleport(_lm.boss_arena_center())
	await wait_physics_frames(5, "the arena trigger must see the player")
	assert_eq(_gm.boss_fight_state(), 1, "crossing in engages the fight")
	assert_true(gate.is_sealed(), "the gate slams behind the player")
	assert_almost_eq(_gm.get_shield(), _gm.get_max_shield(), 0.01,
		"the arena seal restores shields — no loitering for regen at the door")
	await wait_process_frames(2)  # the rise flips ride call_deferred
	assert_eq(boss.collision_layer, 1, "engagement rises the boss — tangible again")
	assert_true(boss.visible, "engagement rises the boss — visible again")

	# --- The kill alone opens nothing ---
	boss.take_damage(100000.0)
	await wait_physics_frames(5, "the death report must reach the manager")
	assert_eq(_gm.boss_fight_state(), 2, "the boss is down")
	assert_true(gate.is_sealed(), "the kill is not the end — the pickup is")
	if portal != null:
		assert_false(portal.monitoring, "no exit before the reward")

	# --- ALL boss loot gathered closes the fight — not any one pickup ---
	# The fight sheds its reward (a bound cache plus, for a pile fight, its
	# consolation caches). When there's more than one, grabbing SOME must leave
	# the arena sealed (owner's call 2026-07-05); only the full set opens it.
	var drops := _live_caches()
	assert_gt(drops.size(), 0, "the resolved fight sheds its reward")
	if drops.is_empty():
		return
	if drops.size() > 1:
		_teleport(drops[0].global_position)
		await wait_physics_frames(10, "the first pickup must register")
		await wait_process_frames(2)
		assert_eq(_gm.boss_fight_state(), 2,
			"one drop of a pile is not the reward beat — still Defeated")
		assert_true(gate.is_sealed(), "the arena stays sealed over the rest")

	for c in _live_caches():
		_teleport(c.global_position)
		await wait_physics_frames(10, "each pickup must register")
	await wait_process_frames(2)  # the arena-open flips ride call_deferred
	assert_eq(_gm.boss_fight_state(), 3, "the WHOLE reward closes the fight")
	assert_false(gate.is_sealed(), "the arena opens")
	if portal != null:
		assert_true(portal.monitoring, "the way onward appears")

func test_a_hull_container_fight_grants_a_flyable_hull():
	# Some staged fight sheds the RED hull container: collecting it grants a
	# random unowned hull (profile-persisted) — the only way hulls enter the
	# fleet now. WHICH fight is grammar the owner tunes, so it is FOUND, never
	# pinned to a level or a boss def.
	_build_stack()
	_gm.start_new_game()
	_walk_to_playing()
	await wait_process_frames(2)
	assert_eq(_gm.owned_hull_count(), 0, "a fresh profile owns no extra hulls")

	# Walk the campaign, resolving each staged fight, until one grants a hull.
	var granted := false
	for _i in range(24):
		if not await _advance_to_a_boss_level():
			break
		await wait_process_frames(2)
		_teleport(_lm.boss_arena_center())
		await wait_physics_frames(5, "engage")
		var boss := _lm.staged_boss_node()
		if boss != null:
			boss.take_damage(100000.0)
			await wait_physics_frames(5, "the death report must land")
		for c in _live_caches():
			_teleport(c.global_position)
			await wait_physics_frames(10, "each pickup registers")
		await wait_process_frames(2)
		if _gm.owned_hull_count() > 0:
			granted = true
			break
		_advance_one_level()  # not this fight — walk on to the next
	assert_true(granted, "a staged fight grants a hull via its container reward")
	assert_eq(_gm.owned_hull_count(), 1,
		"the container granted exactly one hull, persisted to the profile")

	# --- The granted hull is flyable at the next loadout screen ---
	_gm.on_portal_entered()
	_gm.advance_to_shop()
	_gm.advance_to_next_level()
	assert_eq(_gm.get_phase_name(), "ShipSelect", "the loadout screen follows")
	var granted_ship := 0
	for unlock_id in range(3, 6):  # Unlock ids: Talon 3, Hive 4, Reaver 5
		if _gm.has_unlock(unlock_id):
			granted_ship = unlock_id - 2  # ShipType ids: Talon 1, Hive 2, Reaver 3
			break
	assert_gt(granted_ship, 0, "one purchasable hull is owned")
	_gm.on_ship_type_selected(granted_ship)
	assert_eq(_gm.get_ship_type_id(), granted_ship,
		"the boss-granted hull is selectable at the loadout")
