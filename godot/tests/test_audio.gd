extends GutTest
## Tests for the Phase 3.5 audio system.
## Verifies AudioManager class, bus layout, and resource existence.

# --- AudioManager class ---

func test_audio_manager_class_exists():
	var mgr = AudioManager.new()
	assert_not_null(mgr, "AudioManager class should be instantiatable")
	mgr.free()

func test_audio_manager_has_phase_changed_method():
	var mgr = AudioManager.new()
	assert_has_method(mgr, "on_phase_changed_audio",
		"AudioManager must handle phase_changed signal")
	mgr.free()

func test_audio_manager_has_sfx_methods():
	var mgr = AudioManager.new()
	assert_has_method(mgr, "play_sfx_event",
		"AudioManager must expose play_sfx_event")
	assert_has_method(mgr, "play_sfx_event_at",
		"AudioManager must expose play_sfx_event_at")
	mgr.free()

func test_audio_manager_has_music_finished_method():
	var mgr = AudioManager.new()
	assert_has_method(mgr, "on_music_finished",
		"AudioManager must handle music track completion")
	mgr.free()

# --- Audio bus layout ---

func test_music_bus_exists():
	var idx = AudioServer.get_bus_index("Music")
	assert_ne(idx, -1, "Music audio bus should exist")

func test_sfx_bus_exists():
	var idx = AudioServer.get_bus_index("SFX")
	assert_ne(idx, -1, "SFX audio bus should exist")

func test_music_bus_routes_to_master():
	var idx = AudioServer.get_bus_index("Music")
	if idx != -1:
		var send = AudioServer.get_bus_send(idx)
		assert_eq(send, &"Master", "Music bus should route to Master")

func test_sfx_bus_routes_to_master():
	var idx = AudioServer.get_bus_index("SFX")
	if idx != -1:
		var send = AudioServer.get_bus_send(idx)
		assert_eq(send, &"Master", "SFX bus should route to Master")

# --- Audio manager in main scene ---

func test_audio_manager_in_main_scene():
	var scene = load("res://scenes/main.tscn")
	assert_not_null(scene, "main.tscn should load")
	var instance = scene.instantiate()
	var audio_mgr = instance.get_node_or_null("AudioManager")
	assert_not_null(audio_mgr, "Main scene should have AudioManager node")
	instance.free()

# --- Audio resource existence (skip if addons not installed) ---

func test_menu_music_file_exists():
	if not DirAccess.dir_exists_absolute("res://addons/audio/music"):
		pass_test("Audio addons not installed, skipping")
		return
	assert_true(
		ResourceLoader.exists("res://addons/audio/music/frozen_whispers.wav"),
		"Menu music file should exist")

func test_laser_sfx_files_exist():
	if not DirAccess.dir_exists_absolute("res://addons/audio/sfx"):
		pass_test("Audio addons not installed, skipping")
		return
	assert_true(
		ResourceLoader.exists("res://addons/audio/sfx/Gunshots/Laser/laser_shoot_01.wav"),
		"Laser SFX file should exist")

func test_impact_sfx_files_exist():
	if not DirAccess.dir_exists_absolute("res://addons/audio/sfx"):
		pass_test("Audio addons not installed, skipping")
		return
	assert_true(
		ResourceLoader.exists("res://addons/audio/sfx/Impacts/impact_kinetic_light_metal_01.wav"),
		"Impact metal SFX file should exist")

# --- Collision SFX integration ---

func _spawn_main_scene():
	var scene = load("res://scenes/main.tscn")
	if scene == null:
		return null
	return scene.instantiate()

func test_audio_manager_has_on_sfx_finished():
	var mgr = AudioManager.new()
	assert_has_method(mgr, "on_sfx_finished",
		"AudioManager must handle SFX completion for polyphony tracking")
	mgr.free()

func test_play_sfx_event_spawns_audio_child():
	# Verify the non-positional SFX pipeline: play_sfx_event adds an
	# AudioStreamPlayer child to AudioManager (standalone, no main scene).
	if not DirAccess.dir_exists_absolute("res://addons/audio/sfx"):
		pass_test("Audio addons not installed, skipping")
		return

	var mgr = AudioManager.new()
	add_child_autofree(mgr)
	await wait_physics_frames(2, "Waiting for AudioManager ready")

	# Baseline includes 2 music crossfade players created in ready()
	var sfx_before = _count_sfx_children(mgr)
	mgr.play_sfx_event(0)  # 0 = LaserFire (non-positional)
	await wait_physics_frames(2, "Waiting for SFX node to spawn")

	var sfx_after = _count_sfx_children(mgr)
	assert_gt(sfx_after, sfx_before,
		"play_sfx_event should spawn an AudioStreamPlayer child (before=%d, after=%d)" % [sfx_before, sfx_after])

func test_rapid_impact_events_are_throttled():
	# Collision cooldown should prevent SFX spam (standalone AudioManager).
	if not DirAccess.dir_exists_absolute("res://addons/audio/sfx"):
		pass_test("Audio addons not installed, skipping")
		return

	var mgr = AudioManager.new()
	add_child_autofree(mgr)
	await wait_physics_frames(2, "Waiting for AudioManager ready")

	# Baseline includes 2 music crossfade players
	var baseline = _count_sfx_children(mgr)

	# Fire 5 impact events in rapid succession (non-positional to count on mgr)
	for i in range(5):
		mgr.play_sfx_event(2)  # 2 = ImpactMetal

	await wait_physics_frames(2, "Waiting for SFX nodes")

	# With 0.3s cooldown, only the first should have spawned (1 new child)
	var sfx_spawned = _count_sfx_children(mgr) - baseline
	assert_true(sfx_spawned <= 2,
		"Collision cooldown should throttle rapid impacts, got %d new SFX" % sfx_spawned)

func _count_sfx_children(node: Node) -> int:
	var count = 0
	for child in node.get_children():
		if child is AudioStreamPlayer or child is AudioStreamPlayer3D:
			count += 1
	return count
