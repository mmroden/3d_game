extends GutTest
## Loose debris are dynamic RigidBody3D props. Their collider must hug the
## visible mesh, not a fixed sphere: a sphere on a boxy crate gives glancing,
## angle-dependent hits — you fly through one way and bounce another. Audit a
## real generated level so every debris body's collider covers its mesh.

## A convex hull contains every mesh vertex by construction, so its reach
## should match the farthest vertex (~1.0). A fixed 0.5 m sphere reaches only
## a fraction of these metre-plus props, so it falls far below this.
const MIN_COVERAGE = 0.95


func test_debris_colliders_cover_their_mesh():
	var lm = LevelManager.new()
	add_child_autofree(lm)
	lm.generate_level(4242, 12)

	var checked := 0
	var stack: Array = [lm]
	while not stack.is_empty():
		var node = stack.pop_back()
		for child in node.get_children():
			stack.push_back(child)
		# Debris are plain dynamic bodies; enemies/ship drive themselves.
		if node is RigidBody3D and not (node is EnemyDrone) and not (node is ShipController):
			var mesh := _mesh_extent(node)
			if mesh <= 0.0:
				continue
			var collider := _collider_extent(node)
			checked += 1
			var coverage := collider / mesh
			gut.p("debris collider %.2f / mesh %.2f = coverage %.2f" % [collider, mesh, coverage])
			assert_gt(coverage, MIN_COVERAGE,
				"debris collider (extent %.2f) covers too little of its mesh (extent %.2f); a sphere on a box clips at the corners" % [collider, mesh])
	assert_gt(checked, 0, "seed must spawn debris bodies for this audit to mean anything")


## Farthest reach of the body's collision shapes from its origin, applying
## each shape's own transform so the hull points land in body space.
func _collider_extent(body: Node) -> float:
	var best := 0.0
	for child in body.get_children():
		if child is CollisionShape3D and child.shape != null:
			var s = child.shape
			var xf: Transform3D = child.transform
			if s is ConvexPolygonShape3D:
				for p in s.points:
					best = maxf(best, (xf * p).length())
			elif s is SphereShape3D:
				best = maxf(best, child.position.length() + s.radius)
			elif s is BoxShape3D:
				best = maxf(best, child.position.length() + s.size.length() / 2.0)
			elif s is CapsuleShape3D:
				best = maxf(best, child.position.length() + maxf(s.radius, s.height / 2.0))
	return best


## Farthest mesh vertex from the body origin, in body space. Uses real
## vertices (not the AABB, whose corners jut into empty space for irregular
## props) so a true mesh-hugging hull scores ~1.0.
func _mesh_extent(body: Node3D) -> float:
	var extent := 0.0
	var stack: Array = [body]
	while not stack.is_empty():
		var node = stack.pop_back()
		for child in node.get_children():
			stack.push_back(child)
		if node is MeshInstance3D and node.mesh != null:
			var to_local: Transform3D = body.global_transform.affine_inverse() * node.global_transform
			for si in node.mesh.get_surface_count():
				var verts = node.mesh.surface_get_arrays(si)[Mesh.ARRAY_VERTEX]
				for v in verts:
					extent = maxf(extent, (to_local * v).length())
	return extent
