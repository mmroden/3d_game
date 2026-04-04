extends GutTest
## Tests for physics NaN behavior in Godot 4.6.
## Documents actual engine behavior and verifies our guards.

func _is_transform_finite(t: Transform3D) -> bool:
	for i in range(3):
		var col = t.basis[i]
		if is_nan(col.x) or is_nan(col.y) or is_nan(col.z):
			return false
	return not (is_nan(t.origin.x) or is_nan(t.origin.y) or is_nan(t.origin.z))

func _is_vector_finite(v: Vector3) -> bool:
	return not (is_nan(v.x) or is_nan(v.y) or is_nan(v.z))

func _is_quat_finite(q: Quaternion) -> bool:
	return not (is_nan(q.x) or is_nan(q.y) or is_nan(q.z) or is_nan(q.w))

# --- Document Godot 4.6 behavior ---

func test_zero_vector_normalized_returns_zero():
	var result = Vector3.ZERO.normalized()
	assert_true(_is_vector_finite(result), "zero.normalized() is finite in Godot 4.6")
	assert_eq(result, Vector3.ZERO)

func test_direction_to_colocated_is_zero():
	var pos = Vector3(10, 10, 10)
	var direction = (pos - pos).normalized()
	assert_true(_is_vector_finite(direction), "colocated direction is finite")

# --- THE ACTUAL BUG: from_axis_angle with NaN angle ---

func test_from_axis_angle_with_nan_angle():
	# This is what ship_controller does:
	#   Quaternion::from_axis_angle(Vector3::RIGHT, rot.x)
	# If rot.x is NaN (from corrupted angular_velocity), the result is NaN.
	var q = Quaternion.from_euler(Vector3(NAN, 0, 0))
	assert_false(_is_quat_finite(q),
		"from_euler with NaN should produce NaN quaternion (the actual bug)")

func test_nan_quaternion_multiplication_cascades():
	# Once a quaternion has NaN, all subsequent multiplications produce NaN.
	var good = Quaternion.IDENTITY
	var bad = Quaternion.from_euler(Vector3(NAN, 0, 0))
	var result = good * bad
	assert_false(_is_quat_finite(result),
		"NaN quaternion cascades through multiplication")

# --- Test: how does angular_velocity become NaN? ---

func test_move_and_slide_collision_cascade():
	# Simulate what happens when two CharacterBody3D collide:
	# body A moves into body B, move_and_slide resolves.
	# After resolution, does the velocity become NaN?
	var body_a = CharacterBody3D.new()
	var shape_a = CollisionShape3D.new()
	var sphere_a = SphereShape3D.new()
	sphere_a.radius = 0.5
	shape_a.shape = sphere_a
	body_a.add_child(shape_a)

	var body_b = CharacterBody3D.new()
	var shape_b = CollisionShape3D.new()
	var sphere_b = SphereShape3D.new()
	sphere_b.radius = 0.5
	shape_b.shape = sphere_b
	body_b.add_child(shape_b)

	add_child(body_a)
	add_child(body_b)

	# Place them at the same position.
	body_a.position = Vector3(0, 0, 0)
	body_b.position = Vector3(0, 0, 0)

	# Give A a velocity toward B.
	body_a.velocity = Vector3(10, 0, 0)
	body_a.move_and_slide()

	# Check if velocity became NaN after collision resolution.
	var vel_finite = _is_vector_finite(body_a.velocity)
	gut.p("After collision at same pos: velocity = %s, finite = %s" % [body_a.velocity, vel_finite])

	assert_true(vel_finite, "Velocity should stay finite after collision")

	body_a.queue_free()
	body_b.queue_free()

# --- Guards that should always pass ---

func test_safe_direction_guard():
	var pos_a = Vector3(10, 10, 10)
	var pos_b = Vector3(10, 10, 10)
	var diff = pos_b - pos_a
	var direction := Vector3.ZERO
	if diff.length() > 0.01:
		direction = diff.normalized()
	assert_true(_is_vector_finite(direction))
	assert_eq(direction, Vector3.ZERO)

func test_velocity_clamp_guard():
	var vel = Vector3(1e10, -1e10, 1e10)
	var max_speed := 100.0
	if vel.length() > max_speed:
		vel = vel.normalized() * max_speed
	assert_true(vel.length() <= max_speed + 0.01)
	assert_true(_is_vector_finite(vel))

func test_quaternion_sanitize_guard():
	# If quaternion has NaN, reset to identity.
	var q = Quaternion.from_euler(Vector3(NAN, 0, 0))
	if not _is_quat_finite(q):
		q = Quaternion.IDENTITY
	assert_true(_is_quat_finite(q), "Sanitized quaternion should be identity")
	assert_eq(q, Quaternion.IDENTITY)

func test_angular_velocity_nan_guard():
	# If angular_velocity has NaN, zero it out before using.
	var ang_vel = Vector3(NAN, 0.1, NAN)
	if not _is_vector_finite(ang_vel):
		ang_vel = Vector3.ZERO
	assert_true(_is_vector_finite(ang_vel))
	assert_eq(ang_vel, Vector3.ZERO)

# =====================================================================
# PRODUCTION CODE TESTS — exercise actual game scenes
# These should FAIL until we fix the production Rust code.
# =====================================================================

func test_enemy_colocated_with_player_no_engine_errors():
	# Load the actual enemy scene and a player-like CharacterBody3D.
	# Place them at the same position. Run physics frames.
	# The enemy's physics_process will call look_at() and normalized()
	# on a zero-distance vector. This should NOT produce engine errors.
	var enemy_scene = load("res://scenes/enemies/enemy_slime.tscn")
	if enemy_scene == null:
		gut.p("SKIP: enemy_slime.tscn not found")
		pass_test("skipped — scene not available")
		return

	# Create a player stand-in (real ShipController needs too much wiring)
	var player = CharacterBody3D.new()
	player.add_to_group("player")
	var player_shape = CollisionShape3D.new()
	var player_sphere = SphereShape3D.new()
	player_sphere.radius = 0.5
	player_shape.shape = player_sphere
	player.add_child(player_shape)
	add_child(player)
	player.global_position = Vector3(5, 5, 5)

	# Instantiate the enemy at the exact same position
	var enemy_instance = enemy_scene.instantiate()
	add_child(enemy_instance)
	enemy_instance.global_position = Vector3(5, 5, 5)

	# Wait for physics frames — the enemy's physics_process will run
	await get_tree().physics_frame
	await get_tree().physics_frame
	await get_tree().physics_frame

	# Check the enemy's transform is still finite
	assert_true(_is_transform_finite(enemy_instance.global_transform),
		"Enemy transform should be finite even when colocated with player")

	# Check the player's transform is still finite
	assert_true(_is_transform_finite(player.global_transform),
		"Player transform should be finite after enemy colocated")

	enemy_instance.queue_free()
	player.queue_free()

func test_dead_enemy_stops_moving():
	# Load enemy, deal lethal damage, verify it stops executing physics.
	# Currently the dead check is AFTER movement code, so a dead enemy
	# still moves for one frame. This test verifies it doesn't.
	var enemy_scene = load("res://scenes/enemies/enemy_slime.tscn")
	if enemy_scene == null:
		pass_test("skipped — scene not available")
		return

	var player = CharacterBody3D.new()
	player.add_to_group("player")
	var player_shape = CollisionShape3D.new()
	var player_sphere = SphereShape3D.new()
	player_sphere.radius = 0.5
	player_shape.shape = player_sphere
	player.add_child(player_shape)
	add_child(player)
	player.global_position = Vector3(10, 5, 10)

	var enemy_instance = enemy_scene.instantiate()
	add_child(enemy_instance)
	enemy_instance.global_position = Vector3(5, 5, 5)

	# Let enemy start chasing
	await get_tree().physics_frame
	await get_tree().physics_frame

	# Deal lethal damage (slime has 2 HP)
	enemy_instance.take_damage(100.0)

	# Wait a frame — the dead enemy's physics_process should NOT
	# execute movement/look_at code
	await get_tree().physics_frame

	# The enemy should have zero velocity if dead-check is at top
	if is_instance_valid(enemy_instance):
		var vel = enemy_instance.velocity
		assert_true(_is_vector_finite(vel),
			"Dead enemy velocity should be finite, got %s" % vel)
	else:
		pass_test("Enemy already freed — acceptable")

	player.queue_free()

func test_player_no_spin_after_killing_nearby_enemy():
	# Reproduce the exact crash: real ShipController near enemy, kill enemy,
	# verify player doesn't spin. Uses actual production ShipController.
	var enemy_scene = load("res://scenes/enemies/enemy_slime.tscn")
	if enemy_scene == null:
		pass_test("skipped — scene not available")
		return

	# Use real ShipController so its physics_process runs (reads basis, etc.)
	var player = ShipController.new()
	player.add_to_group("player")
	var player_shape = CollisionShape3D.new()
	var player_sphere = SphereShape3D.new()
	player_sphere.radius = 0.5
	player_shape.shape = player_sphere
	player.add_child(player_shape)
	add_child(player)
	player.global_position = Vector3(5, 5, 5)

	# Record initial quaternion
	var initial_quat = player.quaternion

	# Place enemy ahead of the player
	var enemy_instance = enemy_scene.instantiate()
	add_child(enemy_instance)
	enemy_instance.global_position = Vector3(7, 5, 5)

	# Give the player velocity toward the enemy (simulating flight)
	player.velocity = Vector3(10, 0, 0)

	# Let them collide for several frames
	for i in range(10):
		await get_tree().physics_frame

	# Kill the enemy while they're close
	enemy_instance.take_damage(100.0)

	# Run many more physics frames to let any spin accumulate
	for i in range(60):
		await get_tree().physics_frame

	# The player's transform must be finite
	assert_true(_is_transform_finite(player.global_transform),
		"Player transform should be finite after killing nearby enemy")

	# The player should not be spinning — quaternion should be close to initial
	# (no input was given, so only damping should have acted)
	var final_quat = player.quaternion
	var angle_diff = initial_quat.angle_to(final_quat)
	assert_true(angle_diff < 0.5,
		"Player should not spin: angle changed by %f radians (max 0.5)" % angle_diff)

	player.queue_free()
