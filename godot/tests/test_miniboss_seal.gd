extends GutTest
## The miniboss room seal, full stack, on the FIXTURE grammar (owner
## 2026-07-09: mechanism tests never depend on rosters/ tuning). ONE pinned
## run (fixed_seed = 1, start_level = 1 — the Rust anchor
## pinned_gut_run_anchors_a_miniboss_room pins that this build anchors a
## miniboss room): the anchor waits dormant, entry seals EVERY connector of
## its room and rises it WITHOUT the boss arena's shield mercy, and the kill
## re-opens at once — no reward ritual, a wounded player can flee the moment
## it drops (owner 2026-07-06).
##
## State ids mirror void_logic::boss_fight::BossFightState::id():
## -1 = no seal on this level, 0 Dormant, 1 Engaged, 2 Defeated,
## 3 RewardCollected (a miniboss lands on 3 AT the kill — defeat(0)).

const UiStub := preload("res://tests/helpers/ui_stub.gd")

var _gm: GameManager
var _lm: LevelManager
var _player: ShipController

func before_all():
	assert_true(GameManager.install_test_grammar(
		FileAccess.get_file_as_string("res://tests/fixtures/grammar/enemies.toml"),
		FileAccess.get_file_as_string("res://tests/fixtures/grammar/kits.toml"),
		FileAccess.get_file_as_string("res://tests/fixtures/grammar/kits.generated.toml"),
		FileAccess.get_file_as_string("res://tests/fixtures/grammar/planet_1.toml"),
	), "the fixture grammar installs")

func after_all():
	GameManager.clear_test_grammar()

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
	_gm.fixed_seed = 1   # the pinned run — the Rust anchor pins this build
	_gm.start_level = 1  # the fixture's level 1 fields the miniboss
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

func _gates() -> Array:
	return _lm.find_children("*", "BossGate", true, false)

func _sealed_count(gates: Array) -> int:
	var sealed := 0
	for g in gates:
		if g.is_sealed():
			sealed += 1
	return sealed

func _teleport(pos: Vector3) -> void:
	_player.global_position = pos
	_player.linear_velocity = Vector3.ZERO
	_player.reset_physics_interpolation()

func test_the_miniboss_room_seals_on_entry_and_reopens_on_the_kill():
	_build_stack()
	_gm.start_new_game()
	_walk_to_playing()
	await wait_process_frames(2)

	# --- Pre-entry: the seal FSM waits dormant, the anchor with it ---
	assert_eq(_gm.boss_fight_state(), 0,
		"a miniboss level stages a seal FSM, dormant (no boss slot here)")
	var boss := _lm.staged_boss_node()
	assert_not_null(boss, "the miniboss waits as the room's dormant anchor")
	if boss == null:
		return
	assert_eq(boss.collision_layer, 0, "dormant: intangible before entry")
	assert_false(boss.visible, "dormant: invisible before entry")

	var gates := _gates()
	assert_gt(gates.size(), 0, "every active connector of the room is gated")
	assert_eq(_sealed_count(gates), 0, "all gates open before entry")

	var portal: Node = _lm.find_children("*", "Portal", true, false).front()
	assert_not_null(portal, "the level keeps its portal")
	if portal != null:
		assert_true(portal.monitoring,
			"the portal is live from the start — the miniboss gates only its room")

	# Bruise the shield on the approach: the MINIBOSS seal must NOT heal it
	# (owner 2026-07-09: only bosses get the shield-restore mercy, for now).
	_gm.on_player_damaged(20.0, Vector3.ZERO)
	var bruised: float = _gm.get_shield()
	assert_lt(bruised, _gm.get_max_shield(),
		"fixture sanity: the approach hit landed on the shield")

	# --- Entry engages: every gate slams, the anchor rises, no heal ---
	_teleport(boss.global_position)
	await wait_physics_frames(5, "the room sensor must see the player")
	assert_eq(_gm.boss_fight_state(), 1, "crossing in engages the fight")
	assert_eq(_sealed_count(gates), gates.size(),
		"every connector of the room seals at once")
	# Unlike the empty boss arena, this room's populace is LIVE — the player
	# takes real fire across the engage frames, so the no-heal contract is
	# "nowhere near full": the boss arena's restore would pin it at max.
	assert_lt(_gm.get_shield(), _gm.get_max_shield() * 0.8,
		"the miniboss seal restores NOTHING — the bruise (and the brawl) stand")
	await wait_process_frames(2)  # the rise flips ride call_deferred
	assert_eq(boss.collision_layer, 1, "engagement rises the miniboss")
	assert_true(boss.visible, "engagement rises the miniboss — visible")

	# --- The kill IS the end: the seal lifts at once, no reward beat ---
	boss.take_damage(100000.0)
	await wait_physics_frames(5, "the death report must reach the manager")
	await wait_process_frames(2)  # the re-open flips ride call_deferred
	assert_eq(_gm.boss_fight_state(), 3,
		"the kill resolves the fight outright — no loot ritual")
	assert_eq(_sealed_count(gates), 0,
		"death re-opens every exit at once — a wounded player can flee")
	if portal != null:
		assert_true(portal.monitoring, "the portal never depended on the fight")
