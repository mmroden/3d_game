extends GutTest
## Every enemy's collision shape must cover its visible mesh. A
## collider much smaller than the model lets the mesh poke through
## walls while physics stays legal — the "blob half inside the wall"
## bug. This audit pins the whole class for every current and future
## enemy scene.

const ENEMY_SCENES_DIR = "res://scenes/enemies"
## Collider must reach at least this fraction of the mesh's farthest
## extent from the body origin.
const MIN_COVERAGE = 0.5


func test_every_enemy_collider_covers_its_mesh():
	var dir = DirAccess.open(ENEMY_SCENES_DIR)
	assert_not_null(dir, "enemy scenes directory must exist")
	var checked := 0
	for file in dir.get_files():
		if not file.ends_with(".tscn"):
			continue
		var enemy = load(ENEMY_SCENES_DIR + "/" + file).instantiate()
		add_child_autofree(enemy)
		var collider := _collider_extent(enemy)
		var mesh := _mesh_extent(enemy)
		checked += 1
		if mesh <= 0.0:
			fail_test("%s: no mesh found to audit" % file)
			continue
		var coverage := collider / mesh
		gut.p("%s: collider %.2f / mesh %.2f = coverage %.2f" % [file, collider, mesh, coverage])
		assert_gt(coverage, MIN_COVERAGE,
			"%s: collision shape (extent %.2f) covers too little of the mesh (extent %.2f); the model will visibly clip through walls" % [file, collider, mesh])
	assert_gt(checked, 0, "audit must find enemy scenes")


## Farthest reach of the collision shape from the body origin.
func _collider_extent(enemy: Node) -> float:
	for child in enemy.get_children():
		if child is CollisionShape3D and child.shape != null:
			var s = child.shape
			var offset: float = child.position.length()
			if s is SphereShape3D:
				return offset + s.radius
			if s is BoxShape3D:
				return offset + s.size.length() / 2.0
			if s is CapsuleShape3D:
				return offset + maxf(s.radius, s.height / 2.0)
	return 0.0


## Farthest reach of any mesh vertex AABB from the body origin,
## in the enemy's local space.
func _mesh_extent(enemy: Node3D) -> float:
	var extent := 0.0
	var stack: Array = [enemy]
	while not stack.is_empty():
		var node = stack.pop_back()
		for child in node.get_children():
			stack.push_back(child)
		if node is MeshInstance3D and node.mesh != null:
			var to_local: Transform3D = enemy.global_transform.affine_inverse() * node.global_transform
			var aabb: AABB = to_local * node.get_aabb()
			for i in 8:
				extent = maxf(extent, aabb.get_endpoint(i).length())
	return extent
