extends GutTest
## Ship-select / loadout backdrop: the loadout screen shows a quiet room behind
## the rotating showcase ship. It is built structure-only — the same generator,
## with populace (props, loot, enemies, the exit portal) skipped — so no live
## enemy fires at the parked player and no exit gate floats in the loadout room.
## These tests pin: a full level really does carry that populace (so the
## structure-only build is meaningfully different), the backdrop carries none of
## it anywhere in the room subtree, and room_center lands inside the room.

const SEED := 12345


func _fresh_level() -> Node3D:
	var lm = LevelManager.new()
	lm.name = "LevelManager"
	add_child_autofree(lm)
	return lm


## Every descendant of `root` (depth-first), excluding root itself.
func _descendants(root: Node) -> Array:
	var out: Array = []
	var stack: Array = [root]
	while not stack.is_empty():
		var n = stack.pop_back()
		for c in n.get_children():
			out.append(c)
			stack.push_back(c)
	return out


func _portal_count(root: Node) -> int:
	var count := 0
	for n in _descendants(root):
		if n.get_class() == "Portal":
			count += 1
	return count


func _enemy_count(root: Node) -> int:
	var count := 0
	for n in _descendants(root):
		if n.is_in_group("enemies"):
			count += 1
	return count


func test_full_level_carries_the_populace_the_backdrop_drops():
	# Guards the structure-only test below from being vacuous: a real level does
	# build an exit portal (and, seed permitting, enemies) somewhere in its rooms.
	var lm = _fresh_level()
	lm.generate_level(SEED, 1)
	await get_tree().process_frame
	assert_eq(_portal_count(lm), 1,
		"a full level must spawn exactly one exit portal for the backdrop to drop")


func test_backdrop_is_structure_only():
	var lm = _fresh_level()
	lm.generate_backdrop(SEED)
	await get_tree().process_frame
	assert_not_null(lm.get_node_or_null("Room0"),
		"the backdrop room itself must remain")
	assert_eq(_portal_count(lm), 0,
		"the structure-only backdrop must carry no exit portal")
	assert_eq(_enemy_count(lm), 0,
		"the structure-only backdrop must carry no live enemies")


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


## World-space union of every MeshInstance3D AABB under `root`.
func _mesh_aabb(root: Node3D) -> AABB:
	var acc := AABB()
	var found := false
	for n in _descendants(root):
		if n is MeshInstance3D and n.mesh != null:
			var world: AABB = n.global_transform * n.get_aabb()
			if found:
				acc = acc.merge(world)
			else:
				acc = world
				found = true
	return acc
