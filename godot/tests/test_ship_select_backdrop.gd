extends GutTest
## Ship-select / loadout backdrop: the loadout screen reuses the real level
## generator for a single room, then prunes everything that isn't room geometry
## (enemies, loot, portal) so the player parks in a quiet room with the rotating
## showcase ship in front. These tests pin two things: the prune actually strips
## populace (and isn't vacuously pruning nothing), and room_center — where the
## player is parked — lands inside that room.

const SEED := 12345

func _fresh_level() -> Node3D:
	var lm = LevelManager.new()
	lm.name = "LevelManager"
	add_child_autofree(lm)
	return lm

## World-space union of every MeshInstance3D AABB under `root`.
func _mesh_aabb(root: Node3D) -> AABB:
	var acc := AABB()
	var found := false
	var stack: Array = [root]
	while not stack.is_empty():
		var n = stack.pop_back()
		for c in n.get_children():
			stack.push_back(c)
		if n is MeshInstance3D and n.mesh != null:
			var world: AABB = n.global_transform * n.get_aabb()
			if found:
				acc = acc.merge(world)
			else:
				acc = world
				found = true
	return acc

func test_full_level_has_populace_that_backdrop_must_remove():
	# Guards the prune test below from being vacuous: a populated level really
	# does contain non-room nodes (portal/enemies/loot) for the backdrop to strip.
	var lm = _fresh_level()
	lm.generate_level(SEED, 1)
	await get_tree().process_frame
	var non_room := 0
	for child in lm.get_children():
		if not child.name.begins_with("Room"):
			non_room += 1
	assert_gt(non_room, 0,
		"a populated level must contain non-room nodes for the backdrop to prune")

func test_backdrop_keeps_only_room_geometry():
	var lm = _fresh_level()
	lm.generate_backdrop(SEED)
	await get_tree().process_frame
	assert_not_null(lm.get_node_or_null("Room0"),
		"the backdrop room itself must remain")
	var non_room := 0
	for child in lm.get_children():
		if not child.name.begins_with("Room"):
			non_room += 1
	assert_eq(non_room, 0,
		"backdrop must prune enemies/loot/portal, leaving only Room* containers")

func test_room_center_lands_inside_the_backdrop_room():
	# The player is parked at room_center(0) + a small y offset; that point must
	# sit inside the room or the loadout screen renders the player in a wall/void.
	var lm = _fresh_level()
	lm.generate_backdrop(SEED)
	await get_tree().process_frame
	var center: Vector3 = lm.room_center(0)
	var aabb := _mesh_aabb(lm.get_node("Room0"))
	assert_true(aabb.has_volume(), "the backdrop room must have geometry")
	# Grow slightly to absorb float error at the walls.
	var grown := aabb.grow(0.5)
	assert_true(grown.has_point(center),
		"room_center(0) must lie inside the room the player parks in")
