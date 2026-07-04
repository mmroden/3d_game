extends GutTest
## The FogMap unlock: GameManager derives the recon map from the retained
## LevelGraph and pushes it to the HUD when (and only when) a new room is
## visited; the corner widget renders only while the unlock flag is pushed.
## Map geometry itself is pinned in Rust (level_map.rs); this suite covers
## the shell contract: the room_changed wire, per-new-room pushes, and the
## widget's unlock gating.

const UiStub := preload("res://tests/helpers/ui_stub.gd")

## Records GameManager's recon-map pushes; everything else is the shared
## UiStub contract (tests/helpers/ui_stub.gd).
class MapRecordingStub:
	extends UiStub
	# Solid (visited, non-frontier) ROOM rects per push — the fog contract's
	# observable. Corridor rects and frontier hints don't count as rooms.
	# Also records each push's current-flagged rect so tests can watch the
	# player marker FOLLOW the player (playtest 2026-07-04: it lagged).
	var map_room_counts: Array = []
	var current_rects: Array = []
	var projections: Array = []
	func update_map(rects: PackedFloat32Array, flags: PackedByteArray, projection: PackedFloat32Array) -> void:
		projections.append(projection)
		var rooms := 0
		for i in flags.size():
			var f: int = flags[i]
			if f & 2 == 0 and f & 4 == 0:  # not CORRIDOR, not FRONTIER
				rooms += 1
			if f & 1 != 0:  # CURRENT
				current_rects.append(Rect2(rects[4 * i], rects[4 * i + 1], rects[4 * i + 2], rects[4 * i + 3]))
		map_room_counts.append(rooms)


func test_visiting_rooms_grows_the_pushed_map():
	var root := Node3D.new()
	add_child_autofree(root)
	var hud_stub: MapRecordingStub = null
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := MapRecordingStub.new()
		stub.name = ui_name
		root.add_child(stub)
		if ui_name == "HUD":
			hud_stub = stub
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	# A real ShipController: LevelManager binds it (typed) for room
	# resolution, and GameManager pushes controls state into it.
	var player := ShipController.new()
	player.name = "Player"
	var shape := CollisionShape3D.new()
	var sphere := SphereShape3D.new()
	sphere.radius = 0.5
	shape.shape = sphere
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
	await wait_process_frames(3, "the sector build defers past the loading veil's frame")

	# The build parks the player in room 0; culling resolves it and the
	# (deferred) room_changed lands the first map push.
	await wait_physics_frames(3, "let culling resolve the spawn room")
	assert_gt(hud_stub.map_room_counts.size(), 0,
		"entering the level pushes the first map view")
	assert_eq(hud_stub.map_room_counts.back(), 1, "one room visited, one room mapped")

	# Walk node by node until a second ROOM is mapped: the node list mixes
	# rooms and corridors, and only room footprints count as rooms on the
	# map (corridors draw as corridors).
	var pushes_before: int = hud_stub.map_room_counts.size()
	var target := 1
	while hud_stub.map_room_counts.back() < 2 and target < 8:
		player.global_position = lm.room_floor_center(target)
		player.reset_physics_interpolation()
		await wait_physics_frames(3, "let culling resolve the hop")
		target += 1
	assert_gt(hud_stub.map_room_counts.size(), pushes_before,
		"newly visited nodes push fresh views")
	assert_eq(hud_stub.map_room_counts.back(), 2, "two rooms visited, two mapped")

	# Re-entering a known room reveals nothing new — but the current marker
	# must FOLLOW the player (playtest 2026-07-04: it stuck on the last
	# newly-visited room).
	var pushes_after_two: int = hud_stub.map_room_counts.size()
	var marker_before: Rect2 = hud_stub.current_rects.back()
	player.global_position = lm.room_floor_center(0)
	player.reset_physics_interpolation()
	await wait_physics_frames(3, "let culling resolve the return")
	assert_gt(hud_stub.map_room_counts.size(), pushes_after_two,
		"re-entering a known room still pushes — the marker must move")
	assert_eq(hud_stub.map_room_counts.back(), 2,
		"but reveals nothing new — the fog only lifts on first visits")
	assert_ne(hud_stub.current_rects.back(), marker_before,
		"the current marker followed the player back to the start room")
	# The projection rides every push, so the panel can place the LIVE
	# player marker between room changes.
	var proj: PackedFloat32Array = hud_stub.projections.back()
	assert_eq(proj.size(), 3, "projection = [scale, off_x, off_z]")
	assert_gt(proj[0], 0.0, "a real level projects at a positive scale")


func test_map_panel_renders_only_with_the_unlock():
	var hud := HUD.new()
	add_child_autofree(hud)
	hud.visible = true
	hud.update_map(
		PackedFloat32Array([0.4, 0.4, 0.2, 0.2]),
		PackedByteArray([1]),  # the current room
		PackedFloat32Array([0.01, 0.5, 0.5]),
	)

	await wait_process_frames(2)
	var panel: Control = hud.find_children("*", "MapPanel", true, false).front()
	assert_not_null(panel, "the HUD builds its map panel")
	assert_false(panel.visible, "no unlock, no map")

	hud.set_unlock_flags(false, true, false)
	await wait_process_frames(2)
	assert_true(panel.visible, "the FogMap unlock shows the corner map")
