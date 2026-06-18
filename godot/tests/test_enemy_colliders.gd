extends GutTest
## Every enemy must get a mesh-hugging convex collider built from its model in
## ready() (one ConvexPolygonShape3D per mesh part). A convex hull is the model
## silhouette, so it can't be the "blob half inside the wall" a too-small
## primitive would be — this audit just pins that the hull is actually built for
## every type, never collider-less.

const ENEMY_SCENE = "res://scenes/enemies/enemy.tscn"
## Number of EnemyType variants — pinned in Rust by
## `roster_is_five_direct_enemies_plus_the_spawn_drone` (ALL.len() == 6).
const ENEMY_TYPE_COUNT = 6


func test_every_enemy_type_gets_convex_colliders():
	var checked := 0
	for type_id in range(ENEMY_TYPE_COUNT):
		var enemy = load(ENEMY_SCENE).instantiate()
		enemy.enemy_type_id = type_id
		add_child_autofree(enemy)
		var hulls := _convex_hull_count(enemy)
		checked += 1
		assert_gt(hulls, 0,
			"enemy type %d: built no convex collision shapes; its model would have no collider" % type_id)
	assert_gt(checked, 0, "audit must check enemy types")


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
