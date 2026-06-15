extends GutTest
## Structural collision is merged to one StaticBody3D per room/corridor, not
## one per tile mesh. A collider-per-mesh explosion (thousands of static
## bodies) bloats Jolt's broadphase and the level-gen time — the in-play
## physics spikes and the multi-second generation freeze. This pins the
## static-body count to the room scale, far below the mesh count.

func test_static_collision_is_merged_per_room():
	var lm = LevelManager.new()
	add_child_autofree(lm)
	lm.generate_level(4242, 12)

	var bodies := 0
	var meshes := 0
	var stack: Array = [lm]
	while not stack.is_empty():
		var node = stack.pop_back()
		for child in node.get_children():
			stack.push_back(child)
		if node is StaticBody3D:
			bodies += 1
		if node is MeshInstance3D:
			meshes += 1

	gut.p("static bodies: %d, mesh instances: %d" % [bodies, meshes])
	assert_gt(bodies, 0, "rooms must still have structural collision")
	assert_gt(meshes, 200, "level must actually have tile meshes for this to mean anything")
	# One merged body per structural room/corridor — dozens, not thousands.
	assert_lt(bodies, 60,
		"structural collision must be merged per room (%d bodies for %d meshes = per-mesh explosion)" % [bodies, meshes])
