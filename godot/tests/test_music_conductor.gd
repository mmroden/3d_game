extends GutTest
## The music conductor, full stack: GameManager derives ONE bed from
## (phase, boss fight beat, live enemies in the player's room) and pushes
## it to the AudioManager at every input change. Pins the owner's design
## (2026-07-05): level backgrounds per level, a random combat stinger when
## the player shares a room with a live enemy, fade back to the level bed
## once they're dead, the boss track from arena entry, and NEVER relooping
## the boss track — a fight outlasting it continues on combat stingers.
##
## Bed ids mirror void_logic::audio_catalog::MusicBed::id():
## 0 Menu, 1 Level, 2 Combat, 3 Boss.

const UiStub := preload("res://tests/helpers/ui_stub.gd")

var _gm: GameManager
var _lm: LevelManager
var _player: ShipController
var _audio: AudioManager

func _build_stack() -> void:
	# find_audio_manager resolves /root/Main/AudioManager — mirror the real
	# scene shape for the conductor's push target.
	var main := Node.new()
	main.name = "Main"
	get_tree().root.add_child(main)
	autofree(main)
	_audio = AudioManager.new()
	_audio.name = "AudioManager"
	main.add_child(_audio)

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
	_gm.fixed_seed = 1  # the pinned seed — level 3 stages the Brute
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

func _teleport(pos: Vector3) -> void:
	_player.global_position = pos
	_player.linear_velocity = Vector3.ZERO
	_player.reset_physics_interpolation()

func _visible_enemies() -> Array:
	var out := []
	for e in _lm.find_children("*", "EnemyDrone", true, false):
		if e.visible:
			out.append(e)
	return out

func test_the_conductor_walks_level_combat_and_boss_beds():
	_build_stack()

	# --- Boot: the menu bed plays before any run exists ---
	await wait_process_frames(2)
	assert_eq(_audio.music_bed_id(), 0, "the main menu opens on the menu bed")

	_gm.start_new_game()
	_walk_to_playing()
	await wait_process_frames(3)

	# --- Level 1: the level's own background (spec-carried) ---
	assert_eq(_audio.music_bed_id(), 1, "arrival lands on the level bed")
	assert_true(String(_audio.music_track()).ends_with("level_01.mp3"),
		"level 1 plays its own background track")

	# --- Sharing a room with a live enemy starts a combat stinger ---
	var enemies := _visible_enemies()
	assert_true(enemies.size() > 0, "level 1 spawns live enemies")
	if enemies.is_empty():
		return
	_teleport(enemies[0].global_position + Vector3(2.0, 0.0, 0.0))
	await wait_physics_frames(5, "room culling must see the player move")
	await wait_process_frames(2)
	assert_eq(_audio.music_bed_id(), 2, "an occupied room raises the combat bed")
	assert_true(String(_audio.music_track()).contains("combat_"),
		"the stinger comes from the combat pool")

	# --- All enemies dead: fade back to the level background ---
	for e in _visible_enemies():
		e.take_damage(100000.0)
	await wait_physics_frames(5, "the death reports must land")
	await wait_process_frames(2)
	assert_eq(_audio.music_bed_id(), 1, "a cleared room falls back to the level bed")
	assert_true(String(_audio.music_track()).ends_with("level_01.mp3"),
		"the same level background resumes")

	# --- Level 3, the mid-boss: entering the ARENA raises the boss bed ---
	_advance_one_level()
	_advance_one_level()
	assert_eq(_gm.get_current_level(), 3, "two advances reach the boss level")
	await wait_process_frames(3)
	assert_eq(_audio.music_bed_id(), 1,
		"the boss level opens on its level bed — the corridor is not the trigger")

	_teleport(_lm.boss_arena_center())
	await wait_physics_frames(5, "the arena trigger must see the player")
	await wait_process_frames(2)
	assert_eq(_audio.music_bed_id(), 3, "crossing into the arena raises the boss bed")
	assert_true(String(_audio.music_track()).ends_with("boss_1.mp3"),
		"the mid-boss fight plays the mid-boss track")

	# --- The boss track never reloops: a finished track continues on a
	# combat stinger while the bed stays Boss (so re-pushes stay no-ops) ---
	_audio.on_music_finished()
	assert_eq(_audio.music_bed_id(), 3, "the bed holds Boss through the continuation")
	assert_true(String(_audio.music_track()).contains("combat_"),
		"the continuation is a combat stinger, not the boss track again")
