extends GutTest
## Projectiles must respect level geometry: a bolt that reaches a wall
## despawns there instead of flying through and hitting the player in
## another room.

const PROJECTILE_SCENE = preload("res://scenes/items/enemy_projectile.tscn")


func test_enemy_projectile_despawns_on_wall():
	var wall = StaticBody3D.new()
	var shape = CollisionShape3D.new()
	var box = BoxShape3D.new()
	box.size = Vector3(8, 8, 1)
	shape.shape = box
	wall.position = Vector3(0, 0, -5)
	wall.add_child(shape)
	add_child_autofree(wall)

	var bolt = PROJECTILE_SCENE.instantiate()
	add_child(bolt)
	autofree(bolt)
	bolt.global_position = Vector3.ZERO
	bolt.launch(Vector3(0, 0, -1), 20.0, 1.0)

	# 20 m/s toward a wall 5 m away: impact within ~0.3 s, well before
	# the 3 s lifetime — a despawn here is the wall, not old age.
	await wait_physics_frames(45)
	assert_false(is_instance_valid(bolt),
		"projectile must despawn on level geometry, not fly through")
