extends GutTest
## Every enemy must get a mesh-hugging convex collider built from its model in
## ready() (one ConvexPolygonShape3D per mesh part). A convex hull is the model
## silhouette, so it can't be the "blob half inside the wall" a too-small
## primitive would be — this audit just pins that the hull is actually built for
## every type, never collider-less.

const ENEMY_SCENE = "res://scenes/enemies/enemy.tscn"


func test_every_enemy_type_gets_convex_colliders():
	var checked := 0
	# The roster size comes from Rust (EnemyType::ALL.len()), not a restated
	# constant — a seventh enemy type is audited the moment it exists.
	for type_id in range(EnemyDrone.enemy_type_count()):
		var enemy = load(ENEMY_SCENE).instantiate()
		enemy.enemy_type_id = type_id
		add_child_autofree(enemy)
		var hulls := _convex_hull_count(enemy)
		checked += 1
		assert_gt(hulls, 0,
			"enemy type %d: built no convex collision shapes; its model would have no collider" % type_id)
	assert_gt(checked, 0, "audit must check enemy types")


## The collider must cover what players actually SEE — for EVERY enemy: a
## grid of rays through the central 60% of the visual bounding box, front-
## on, must land on the enemy's body. Per-part hulls fail this on the
## multi-part mechs — a quadruped is mostly gaps between leg slivers, so
## shots through the silhouette sail clean through (playtest 2026-07-05:
## "I'll be shocked if I hit anything").
func test_every_colliders_covers_its_visual_silhouette():
	for type_id in range(EnemyDrone.enemy_type_count()):
		var enemy = load(ENEMY_SCENE).instantiate()
		enemy.enemy_type_id = type_id
		add_child_autofree(enemy)
		await wait_physics_frames(2)
		var fraction := _silhouette_hit_fraction(enemy)
		gut.p("silhouette coverage type %d: %.0f%%" % [type_id, fraction * 100.0])
		assert_gt(fraction, 0.9,
			"type %d: the collider must cover the visual silhouette, %.0f%% of center-box rays hit"
				% [type_id, fraction * 100.0])
		enemy.queue_free()
		await wait_physics_frames(1)


## Fraction of a 7x7 ray grid (through the central 60% of the visual AABB,
## front-on) that lands on this enemy's body.
func _silhouette_hit_fraction(enemy: Node) -> float:
	var aabb := _visual_aabb(enemy)
	if aabb.size.length() <= 0.0:
		return 0.0
	var space: PhysicsDirectSpaceState3D = enemy.get_world_3d().direct_space_state
	var hits := 0
	var total := 0
	var steps := 7
	for ix in range(steps):
		for iy in range(steps):
			# Sample the inscribed central disk (radius 30% of the face):
			# a convex silhouette with limbs at the box corners is diamond-
			# shaped, and no convex hull owes coverage to the corners of a
			# centered BOX — the disk is what "aimed at the middle of it"
			# means for every body plan.
			var fx: float = 0.2 + 0.6 * ix / float(steps - 1)
			var fy: float = 0.2 + 0.6 * iy / float(steps - 1)
			if Vector2(fx - 0.5, fy - 0.5).length() > 0.3:
				continue
			var from := Vector3(
				aabb.position.x + fx * aabb.size.x,
				aabb.position.y + fy * aabb.size.y,
				aabb.position.z + aabb.size.z + 1.0)
			var to := from + Vector3(0, 0, -(aabb.size.z + 2.0))
			var query := PhysicsRayQueryParameters3D.create(from, to)
			var result: Dictionary = space.intersect_ray(query)
			total += 1
			if not result.is_empty() and result["collider"] == enemy:
				hits += 1
	return hits / float(total)


## Union of the MODEL's MeshInstance3D AABBs (global space). The silhouette
## is the model subtree under ModelPivot — the floating health bar is UI,
## not something the collider owes coverage to.
func _visual_aabb(enemy: Node) -> AABB:
	var model = enemy.get_node_or_null("ModelPivot")
	if model == null:
		return AABB()
	var combined := AABB()
	var first := true
	var stack: Array = [model]
	while not stack.is_empty():
		var node = stack.pop_back()
		for child in node.get_children():
			stack.push_back(child)
		if node is MeshInstance3D:
			var global: AABB = node.global_transform * node.get_aabb()
			combined = global if first else combined.merge(global)
			first = false
	return combined


## Count ConvexPolygonShape3D collision shapes anywhere under the enemy.
func _convex_hull_count(enemy: Node) -> int:
	var count := 0
	var stack: Array = [enemy]
	while not stack.is_empty():
		var node = stack.pop_back()
		for child in node.get_children():
			stack.push_back(child)
		if node is CollisionShape3D and node.shape is ConvexPolygonShape3D:
			count += 1
	return count
