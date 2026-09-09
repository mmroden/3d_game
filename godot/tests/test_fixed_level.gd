extends GutTest
## Planet-3 fixed paradigm, shell contracts: ONE pinned fixture level (the
## fx_house environment over the real installed apartment scene). The
## constants below pin the FIXTURE's shape and are anchored by
## test_fixtures.rs::gut_fixed_level_anchors — edit the fixture files and
## both move together. Mechanism only; the shipped planet-3 tuning never
## appears here.

const UiStub := preload("res://tests/helpers/ui_stub.gd")

const SEED := 1
const SCALE := 5.0                       # kits_fixed.toml (Rust-anchored)
const ZONES := 4                         # env_fixed.toml (Rust-anchored)
const START_MIN := Vector3(0.0, 0.0, 0.0)   # porch box × SCALE
const START_MAX := Vector3(10.0, 10.0, 10.0)
const ARENA_CENTER := Vector3(25.0, 5.0, 5.0)  # den box center × SCALE

var _gm: GameManager
var _lm: LevelManager
var _player: ShipController

func before_all():
	assert_true(GameManager.install_test_grammar(
		FileAccess.get_file_as_string("res://tests/fixtures/grammar/enemies.toml"),
		FileAccess.get_file_as_string("res://tests/fixtures/grammar/kits_fixed.toml"),
		"[kits]\n",  # fixed kits derive no grid — nothing probed
		FileAccess.get_file_as_string("res://tests/fixtures/grammar/planet_fixed.toml"),
		FileAccess.get_file_as_string("res://tests/fixtures/grammar/env_fixed.toml"),
	), "the fixed fixture grammar installs")

func after_all():
	GameManager.clear_test_grammar()

# ── Structural contracts (LevelManager alone) ─────────────────────────────

func _fresh_level() -> LevelManager:
	var lm := LevelManager.new()
	lm.current_level = 1
	add_child_autofree(lm)
	lm.generate_level(SEED, 0)  # the room budget is the environment's; 0 is ignored
	await wait_process_frames(2)
	return lm

func test_the_authored_ambient_overrides_the_world_environment():
	# The bounce stand-in: a fixed build sets the world ambient from the
	# environment's [ambient]. (Restoration on non-fixed builds is Rust-side
	# logic; this fixture grammar is all-fixed, so only the override is
	# assertable here.)
	var world_env := WorldEnvironment.new()
	world_env.name = "WorldEnvironment"
	world_env.environment = Environment.new()
	world_env.environment.ambient_light_source = Environment.AMBIENT_SOURCE_DISABLED
	world_env.environment.ambient_light_energy = 0.123
	add_child_autofree(world_env)
	var lm := LevelManager.new()
	lm.current_level = 1
	add_child_autofree(lm)
	lm.generate_level(SEED, 0)
	await wait_process_frames(2)
	assert_eq(world_env.environment.ambient_light_source,
		Environment.AMBIENT_SOURCE_COLOR, "the fixed build overrides ambient")
	assert_almost_eq(world_env.environment.ambient_light_energy, 0.8, 0.001,
		"…with the authored energy (env_fixed.toml, Rust-anchored)")


func test_the_declared_sky_backs_the_world_environment():
	# The exterior is a skyscape (owner 2026-09-08): a fixed build whose
	# kit names a sky (kits_fixed.toml [skies.fx_sky], Rust-anchored) puts
	# that panorama behind every opening — the world environment's
	# background becomes the sky, its material a PanoramaSkyMaterial on
	# the catalog's texture, imported as float (VRAM uncompressed) so the
	# stars keep their range.
	var world_env := WorldEnvironment.new()
	world_env.name = "WorldEnvironment"
	world_env.environment = Environment.new()
	world_env.environment.background_mode = Environment.BG_COLOR
	add_child_autofree(world_env)
	var lm := LevelManager.new()
	lm.current_level = 1
	add_child_autofree(lm)
	lm.generate_level(SEED, 0)
	await wait_process_frames(2)
	assert_eq(world_env.environment.background_mode, Environment.BG_SKY,
		"the fixed build backs the world environment with its sky")
	var sky := world_env.environment.sky
	assert_not_null(sky, "a Sky resource rides the environment")
	if sky == null:
		return
	var material := sky.sky_material
	assert_true(material is PanoramaSkyMaterial, "the sky is a panorama")
	if not (material is PanoramaSkyMaterial):
		return
	var panorama: Texture2D = (material as PanoramaSkyMaterial).panorama
	assert_not_null(panorama, "the panorama texture loaded")
	if panorama == null:
		return
	assert_eq(panorama.resource_path, "res://addons/sky/starmap_2020_8k_gal.exr",
		"…the catalog's texture (kits_fixed.toml, Rust-anchored)")
	var image := panorama.get_image()
	assert_not_null(image, "the imported texture reads back as an image")
	if image == null:
		return
	var format: int = image.get_format()
	assert_true(format in [Image.FORMAT_RGBH, Image.FORMAT_RGBAH, Image.FORMAT_RGBF,
		Image.FORMAT_RGBAF, Image.FORMAT_RGBE9995],
		"the sky imported as float (got Image.Format %d)" % format)
func _room_containers(lm: LevelManager) -> Array:
	var out := []
	for child in lm.get_children():
		if child is Node3D and String(child.name).begins_with("Room"):
			out.append(child)
	return out

func test_every_zone_gets_a_container_and_the_house_rides_room_zero():
	var lm = await _fresh_level()
	var rooms := _room_containers(lm)
	assert_eq(rooms.size(), ZONES, "one container per authored zone")
	# The scene mesh: exactly one scaled child, under Room0.
	var scaled := []
	for room in rooms:
		for child in room.get_children():
			if child is Node3D and absf(child.scale.x - SCALE) < 0.001:
				scaled.append(child)
	assert_eq(scaled.size(), 1, "ONE house scene at the kit's declared scale")
	if scaled.size() == 1:
		assert_eq(scaled[0].get_parent(), rooms[0], "the scene rides room 0")

func test_room_zero_carries_the_containment_shell():
	var lm = await _fresh_level()
	var rooms := _room_containers(lm)
	if rooms.is_empty():
		fail_test("no rooms built")
		return
	var shell: Node = null
	for child in rooms[0].get_children():
		if String(child.name) == "RoomShell":
			shell = child
	assert_not_null(shell, "room 0 owns the containment envelope")
	if shell != null:
		var boxes := 0
		for c in shell.get_children():
			if c is CollisionShape3D and c.shape is BoxShape3D:
				boxes += 1
		assert_eq(boxes, 64, "one slab per unshared zone-cell face (Rust-anchored)")

func test_the_authored_daylight_rig_spawns():
	var lm = await _fresh_level()
	var suns := lm.find_children("*", "DirectionalLight3D", true, false)
	assert_eq(suns.size(), 1, "the environment's [sun] spawns exactly one sun")
	if not suns.is_empty():
		assert_true(suns.front().shadow_enabled,
			"window light needs shadows or the walls don't matter")
	var panels := lm.find_children("*", "SpotLight3D", true, false)
	assert_eq(panels.size(), 1, "one panel spot per authored [[window]]")
	# The window itself GLOWS: an emissive quad at the opening, so the
	# panel reads as the light source (and bakes as one, later).
	var quads := lm.find_children("WindowPanel*", "MeshInstance3D", true, false)
	assert_eq(quads.size(), 1, "one emissive quad per authored [[window]]")

func test_culling_never_hides_the_house():
	var lm = await _fresh_level()
	var rooms := _room_containers(lm)
	# Stand in the arena (far end): on a generated level depth-2 culling
	# would hide the start room — on a fixed level everything stays lit.
	lm.cull_for_position(ARENA_CENTER)
	await wait_process_frames(1)
	for room in rooms:
		assert_true(room.visible, "%s stays visible (frustum culling only)" % room.name)

# ── The staged fight, gateless (full stack) ───────────────────────────────

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
	_gm.fixed_seed = SEED
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

func test_the_fixed_fight_engages_on_entry_without_gates():
	_build_stack()
	_gm.start_new_game()
	_walk_to_playing()
	await wait_process_frames(2)

	# The player spawns inside the start zone (the porch).
	var pos := _player.global_position
	assert_true(
		pos.x >= START_MIN.x and pos.x <= START_MAX.x
		and pos.y >= START_MIN.y and pos.y <= START_MAX.y
		and pos.z >= START_MIN.z and pos.z <= START_MAX.z,
		"the player spawns in the start zone, got %s" % pos)

	# Every fixed level stages its fight; it waits dormant, portal dark.
	assert_eq(_gm.boss_fight_state(), 0, "the fight waits dormant")
	var gates := _lm.find_children("*", "BossGate", true, false)
	assert_eq(gates.size(), 0, "an open house has no gates (v1)")
	var portals := _lm.find_children("*", "Portal", true, false)
	assert_not_null(portals.front(), "the portal is pre-built (Faucet) …")
	if not portals.is_empty():
		assert_false(portals.front().monitoring, "… but dark until the fight resolves")
	var boss = _lm.staged_boss_node()
	assert_not_null(boss, "the staged boss waits in the arena")
	if boss == null:
		return
	assert_false(boss.visible, "the staged boss is invisible before entry")

	# Fly into the den: entering the arena IS the engagement.
	_player.global_position = ARENA_CENTER
	_player.linear_velocity = Vector3.ZERO
	_player.reset_physics_interpolation()
	await wait_physics_frames(4)
	await wait_process_frames(2)
	assert_eq(_gm.boss_fight_state(), 1, "entry engages the fight")
	assert_true(boss.visible, "the boss rises on entry")
