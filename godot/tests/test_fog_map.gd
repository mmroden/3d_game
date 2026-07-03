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
	var map_room_counts: Array = []
	func update_map(rooms: PackedVector2Array, _rf: PackedByteArray, _e: PackedVector2Array, _ef: PackedByteArray) -> void:
		map_room_counts.append(rooms.size())


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

	# Teleport to another room's floor center: a new visit, a bigger map.
	var pushes_before: int = hud_stub.map_room_counts.size()
	player.global_position = lm.room_floor_center(1)
	player.reset_physics_interpolation()
	await wait_physics_frames(3, "let culling resolve the new room")
	assert_gt(hud_stub.map_room_counts.size(), pushes_before,
		"a newly visited room pushes a fresh view")
	assert_eq(hud_stub.map_room_counts.back(), 2, "two rooms visited, two mapped")

	# Re-entering a known room must NOT push again.
	var pushes_after_two: int = hud_stub.map_room_counts.size()
	player.global_position = lm.room_floor_center(0)
	player.reset_physics_interpolation()
	await wait_physics_frames(3, "let culling resolve the return")
	assert_eq(hud_stub.map_room_counts.size(), pushes_after_two,
		"revisiting a known room pushes nothing — redraws are per NEW room")


func test_map_panel_renders_only_with_the_unlock():
	var hud := HUD.new()
	add_child_autofree(hud)
	hud.visible = true
	hud.update_map(
		PackedVector2Array([Vector2(0.5, 0.5)]),
		PackedByteArray([1]),
		PackedVector2Array([Vector2(0.5, 0.5), Vector2(0.56, 0.5)]),
		PackedByteArray([1]),
	)

	await wait_process_frames(2)
	var panel: Control = hud.find_children("*", "MapPanel", true, false).front()
	assert_not_null(panel, "the HUD builds its map panel")
	assert_false(panel.visible, "no unlock, no map")

	hud.set_unlock_flags(false, true)
	await wait_process_frames(2)
	assert_true(panel.visible, "the FogMap unlock shows the corner map")
