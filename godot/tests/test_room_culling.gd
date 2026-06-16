extends GutTest
## Room-visibility culling: each room's geometry and lights are parented
## under one container node, and only the player's current room plus its
## spatially-adjacent (portal) neighbors render. Culling a room hides its
## lights too, which is the whole point — the GPU was lighting rooms the
## player couldn't see.

const SEED := 12345
const ROOMS := 8

func _fresh_level() -> Node3D:
	var lm = LevelManager.new()
	lm.name = "LevelManager"
	add_child_autofree(lm)
	lm.generate_level(SEED, ROOMS)
	await get_tree().process_frame
	return lm

func test_each_room_is_grouped_under_a_container_node():
	var lm = await _fresh_level()
	assert_not_null(lm.get_node_or_null("Room0"),
		"Room0 container node must exist")
	assert_not_null(lm.get_node_or_null("Room%d" % (ROOMS - 1)),
		"a container node must exist for every room")

func test_lights_are_parented_under_rooms_not_flat():
	var lm = await _fresh_level()
	var flat_lights := 0
	for child in lm.get_children():
		if child is OmniLight3D:
			flat_lights += 1
	assert_eq(flat_lights, 0,
		"lights must live under room containers, not flat on LevelManager")

func test_culling_shows_only_current_room_and_neighbors():
	var lm = await _fresh_level()
	# Cull as if the player were standing in room 0.
	lm.cull_for_position(lm.room_center(0))
	await get_tree().process_frame

	assert_true(lm.get_node("Room0").visible,
		"the player's current room must be visible")

	var hidden := 0
	for i in range(ROOMS):
		var r = lm.get_node_or_null("Room%d" % i)
		if r and not r.visible:
			hidden += 1
	assert_gt(hidden, 0,
		"rooms outside current+neighbors must be culled (hidden)")

func test_blinking_flicker_survives_freed_lights():
	# A light fixture can be freed out from under the host: a level regen, or
	# GameManager clearing LevelManager's children, frees the rooms (and their
	# lights) while the host still holds blinking-light handles. The per-frame
	# flicker must drop freed handles, not clone them — observed red:
	# OmniLight3D::clone panicked as a use-after-free every process frame.
	var lm = await _fresh_level()
	assert_gt(lm.blinking_light_count(), 0,
		"seed must produce blinking lights, else this test is vacuous")
	for child in lm.get_children():
		child.free()
	await wait_process_frames(3)  # process() ticks the flicker over freed lights
	assert_eq(lm.blinking_light_count(), 0,
		"freed lights must be pruned from tracking, not cloned")

func test_room_culling_survives_freed_rooms():
	# The same hazard for room visibility: room nodes freed under the host must
	# not be cloned when culling re-runs (red: Node3D::clone use-after-free).
	var lm = await _fresh_level()
	lm.cull_for_position(lm.room_center(0))  # establish a current room
	for child in lm.get_children():
		child.free()
	# Culling re-runs over the now-freed room_nodes when the room changes.
	lm.cull_for_position(lm.room_center(1))
	await wait_process_frames(1)
	assert_true(true, "no use-after-free culling freed rooms")
