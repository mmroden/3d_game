extends GutTest
## The dev knobs are contracts too (owner 2026-07-12: the untested LEVEL
## flag rotted into a menu-walk surprise). `make run LEVEL=N SEED=S` maps
## to GameManager.start_level / .fixed_seed; F9/F10 ride debug_jump_level.
## Each knob's observable behavior is pinned here.

const UiStub := preload("res://tests/helpers/ui_stub.gd")

var _gm: GameManager
var _lm: LevelManager

func _boot_stack(level: int, seed_value: int, shot := "", sbs := -1) -> void:
	var root := Node3D.new()
	add_child_autofree(root)
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	_lm = LevelManager.new()
	_lm.name = "LevelManager"
	var player := ShipController.new()
	player.name = "Player"
	player.add_to_group("player")
	var shape := CollisionShape3D.new()
	shape.name = "CollisionShape3D"
	shape.shape = SphereShape3D.new()
	player.add_child(shape)
	_gm = GameManager.new()
	_gm.fixed_seed = seed_value
	_gm.start_level = level
	_gm.shot_pose = shot
	_gm.sbs_override = sbs
	root.add_child(_lm)
	root.add_child(player)
	root.add_child(_gm)
	_gm.clear_save_for_tests()
	await wait_process_frames(6)  # the deferred initial phase + level build

func _portal_position() -> Vector3:
	var portals := _lm.find_children("*", "Portal", true, false)
	assert_false(portals.is_empty(), "the level places a portal")
	return portals.front().global_position if not portals.is_empty() else Vector3.INF

func test_start_level_boots_straight_into_the_mission():
	await _boot_stack(2, 1)  # any level > 0 arms the knob; 2 proves propagation
	assert_eq(_gm.get_phase_name(), "Playing",
		"a LEVEL=N boot lands in the mission, not the menu")
	assert_eq(_lm.current_level, 2, "…at the level under inspection")

func test_debug_jump_hops_levels_and_clamps_at_one():
	await _boot_stack(2, 1)
	_gm.debug_jump_level(1)
	await wait_process_frames(4)  # the deferred rebuild (loading veil frame)
	assert_eq(_lm.current_level, 3, "F10 hops one level up")
	assert_eq(_gm.get_phase_name(), "Playing", "the hop stays in flight")
	_gm.debug_jump_level(-5)
	await wait_process_frames(4)
	assert_eq(_lm.current_level, 1, "a hop below the campaign clamps to level 1")

func test_shot_pose_parks_the_ship_exactly_there():
	# The reference-capture knob (`--shot=x,y,z,yaw_deg,pitch_deg`): after
	# the build, the ship sits at the authored pose, motionless — the same
	# vantage every run, so captures compare against vendor stills.
	await _boot_stack(2, 1, "3.5,9.0,12.25,90,-5")
	var player := _gm.get_parent().get_node("Player") as ShipController
	assert_almost_eq(player.global_position, Vector3(3.5, 9.0, 12.25),
		Vector3.ONE * 0.01, "the ship parks at the authored position")
	assert_almost_eq(player.rotation_degrees.y, 90.0, 0.1,
		"…turned to the authored yaw")
	assert_almost_eq(player.rotation_degrees.x, -5.0, 0.1,
		"…pitched to the authored angle")
	assert_eq(player.linear_velocity, Vector3.ZERO, "…and motionless")

func test_shot_list_steps_through_every_pose():
	# One boot, many vantages: a ';'-separated shot list advances to the
	# next pose every SHOT_SETTLE_FRAMES drawn frames — the reference-
	# capture run visits all authored shots in a single build instead of
	# booting per pose. (Pixel saving needs a rendered run; the stepping
	# contract is what headless can pin.)
	await _boot_stack(2, 1, "1,2,3,0,0;4,5,6,90,0")
	# The boot wait spans the first settle window; within two more the
	# sequencer must have traversed to the LAST pose (an empty shot-dir
	# means park there, don't quit)…
	await wait_process_frames(_gm.shot_settle_frames() * 2)
	var player := _gm.get_parent().get_node("Player") as ShipController
	assert_almost_eq(player.global_position, Vector3(4, 5, 6),
		Vector3.ONE * 0.01, "the sequence traverses to the last pose")
	assert_almost_eq(player.rotation_degrees.y, 90.0, 0.1,
		"…with that pose's yaw")
	# …and STAYS parked: the sequence terminates, it never wraps.
	await wait_process_frames(_gm.shot_settle_frames() * 2)
	assert_almost_eq(player.global_position, Vector3(4, 5, 6),
		Vector3.ONE * 0.01, "the sequence ends parked, no wraparound")

func test_sbs_override_beats_the_saved_view_mode():
	# Capture-mode knob (`--sbs=0/1`): forces the view mode for THIS run
	# without touching saved options — reference comparisons want mono
	# regardless of the developer's SBS preference. The test profile
	# defaults SBS off, so an on-reading can only come from the override.
	await _boot_stack(2, 1, "", 1)
	assert_true(_gm.sbs_enabled(),
		"--sbs=1 forces stereo on over the default-off profile")

func test_fixed_seed_pins_the_world():
	await _boot_stack(2, 7)
	var first := _portal_position()
	# Tear the first stack down before booting the second — two live
	# GameManagers would both drive the phase machine.
	get_children().back().queue_free()
	await wait_process_frames(2)
	await _boot_stack(2, 7)
	assert_eq(_portal_position(), first,
		"the same SEED= builds the same world, portal and all")
