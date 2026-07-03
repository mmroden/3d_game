extends GutTest
## Level generation is driven solely by GameManager calling
## generate_level(); LevelManager must not self-generate, and a layout
## must be a pure function of its seed.


const UiStub := preload("res://tests/helpers/ui_stub.gd")


func test_does_not_generate_on_ready():
	var lm = LevelManager.new()
	add_child_autofree(lm)
	await wait_process_frames(2)
	assert_eq(lm.get_child_count(), 0,
		"LevelManager must not generate on ready; GameManager drives generation")


func test_same_seed_produces_same_layout():
	var a = LevelManager.new()
	var b = LevelManager.new()
	add_child_autofree(a)
	add_child_autofree(b)
	a.generate_level(4242, 8)
	b.generate_level(4242, 8)
	assert_gt(_layout_fingerprint(a).size(), 0,
		"generation must actually place nodes (guard against vacuous equality)")
	assert_eq(_layout_fingerprint(a), _layout_fingerprint(b),
		"identical seeds must produce identical layouts")


func test_different_seeds_produce_different_layouts():
	var a = LevelManager.new()
	var b = LevelManager.new()
	add_child_autofree(a)
	add_child_autofree(b)
	a.generate_level(4242, 8)
	b.generate_level(9999, 8)
	assert_ne(_layout_fingerprint(a), _layout_fingerprint(b),
		"different seeds must produce different layouts")


func test_level_generation_is_owned_by_the_phase_machine():
	# The FSM is the sole trigger for generation: entering Playing
	# generates; a rejected transition (Death -> Playing is invalid)
	# must generate nothing.
	var root = Node3D.new()
	add_child_autofree(root)
	# Stub the UI layers show_phase toggles, so the minimal scene
	# exercises the real phase machinery without UI warnings.
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub = UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	var lm = LevelManager.new()
	lm.name = "LevelManager"
	var gm = GameManager.new()
	root.add_child(lm)
	root.add_child(gm)

	# New game now opens the loadout screen, then the bestiary briefing, before
	# the level: MainMenu -> ShipSelect -> Bestiary -> ... -> Playing (generates).
	gm.start_new_game()            # MainMenu -> ShipSelect (no generation yet)
	gm.advance_from_ship_select()  # ShipSelect -> Bestiary briefing
	# Tap through the briefing; the final tap enters Playing and generates.
	for _i in range(10):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	assert_eq(gm.get_phase_name(), "Playing",
		"the briefing must lead into Playing")
	assert_gt(lm.get_child_count(), 0,
		"entering Playing through the FSM must generate a level")

	gm.on_player_damaged(1000000.0, Vector3.ZERO)  # lethal: Playing -> Death
	for child in lm.get_children():
		child.free()
	gm.continue_game()
	assert_eq(lm.get_child_count(), 0,
		"continue_game from Death must not regenerate: Death -> Playing is invalid")


func test_dead_enemies_are_cleaned_up_without_errors():
	# A freed enemy node (death) must be tombstoned by the host on the
	# next tick — never cloned first (observed red: the pre-fix host
	# panicked with a use-after-free here, which GUT surfaces as
	# engine errors failing the test).
	var lm = LevelManager.new()
	add_child_autofree(lm)
	lm.generate_level(4242, 8)
	# Enemies live under per-room containers (cell inhabitants), so scan
	# the subtree rather than direct children.
	var survivors := lm.find_children("*", "EnemyDrone", true, false)
	assert_gt(survivors.size(), 0,
		"seed 4242 must spawn enemies for this test to mean anything")
	var victim: Node = survivors.pop_front()
	victim.free()
	await wait_physics_frames(5)
	var remaining := lm.find_children("*", "EnemyDrone", true, false).size()
	assert_eq(remaining, survivors.size(),
		"the freed enemy must be gone and every survivor must still be hosted")


func test_static_props_are_fused_into_the_room_collider():
	# Differential oracle: generate_backdrop builds the same seed's room as
	# generate_level minus populace, so the two merged colliders differ by
	# exactly the surface-mounted (Collision::Static) props. Passable and
	# Dynamic placements are never fused in either build, so a full-build
	# collider with MORE triangles than the backdrop's proves props were
	# fused; equal counts on every seed means props were dropped (the
	# observed red: props were collected after the merge had already run).
	var grew := 0
	for level_seed in [4242, 7, 1234]:
		var bare = LevelManager.new()
		var full = LevelManager.new()
		add_child_autofree(bare)
		add_child_autofree(full)
		bare.generate_backdrop(level_seed)
		full.generate_level(level_seed, 1)
		var bare_faces := _merged_collider_face_count(bare)
		var full_faces := _merged_collider_face_count(full)
		assert_gt(bare_faces, 0,
			"seed %d: backdrop must build a merged collider" % level_seed)
		assert_true(full_faces >= bare_faces,
			"seed %d: fusing populace statics must never shrink the collider" % level_seed)
		if full_faces > bare_faces:
			grew += 1
	assert_gt(grew, 0,
		"at least one seed must furnish a static prop into the collider, else this test is vacuous")


## Total triangle vertices across every room's merged collider: the
## StaticBody3D placed as a direct child of each room container.
func _merged_collider_face_count(lm: Node3D) -> int:
	var count := 0
	for room in lm.get_children():
		for child in room.get_children():
			if child.get_class() == "StaticBody3D":
				for shape_node in child.find_children("*", "CollisionShape3D", true, false):
					if shape_node.shape is ConcavePolygonShape3D:
						count += shape_node.shape.get_faces().size()
	return count


## Rooms are identity-transform containers, so the layout lives in their
## descendants: fingerprint every Node3D in the subtree, not just direct
## children (which would make all levels look like N rooms at the origin).
func _layout_fingerprint(lm: Node3D) -> Array:
	var entries := []
	for node in lm.find_children("*", "Node3D", true, false):
		entries.append("%s@%s" % [node.get_class(), node.position])
	return entries
