extends GutTest
## The fused static shell must CONTAIN a persistently pressing body.
## Playtest 2026-07-06: bombers grinding into the curved corner meshes
## leak through the hollow trimesh collider and end up caged inside the
## wall — stuck, still shootable. Pinned scenario: one structure-only
## room; four bomber-sized bodies pressed force-only (the drones' own
## physics contract — force, never velocity writes) into the four
## vertical corner edges for several seconds. Every body must stay
## reachable from the room center: a caged body has shell between it
## and the room.

const GRIND_FRAMES := 480       # 8 s at 60 Hz of sustained grinding
const ENEMY_LINEAR_DAMP := 3.0  # mirror EnemyDrone's damping/force pairing
const GRIND_SPEED := 9.0        # the bomber's cruise speed


class Grinder:
	extends RigidBody3D
	var push := Vector3.ZERO

	func _integrate_forces(state: PhysicsDirectBodyState3D) -> void:
		state.apply_central_force(push)


func test_grinding_bodies_never_leak_through_the_corner_curves():
	var lm := LevelManager.new()
	add_child_autofree(lm)
	lm.generate_backdrop(4242)
	await wait_physics_frames(2, "let the shell enter the physics space")

	var center: Vector3 = lm.room_center(0)
	assert_ne(center, Vector3.ZERO, "the backdrop must build room 0")

	var diagonals: Array[Vector3] = [
		Vector3(1, 0, 1).normalized(),
		Vector3(1, 0, -1).normalized(),
		Vector3(-1, 0, 1).normalized(),
		Vector3(-1, 0, -1).normalized(),
	]
	var grinders: Array[RigidBody3D] = []
	for d in diagonals:
		var g := Grinder.new()
		var shape := CollisionShape3D.new()
		var sphere := SphereShape3D.new()
		sphere.radius = 0.25  # bomber-scale hull
		shape.shape = sphere
		g.add_child(shape)
		g.gravity_scale = 0.0
		g.linear_damp = ENEMY_LINEAR_DAMP
		g.lock_rotation = true
		g.can_sleep = false
		# Terminal cruise = force / (mass · damp) = GRIND_SPEED, exactly the
		# drones' pairing — this IS the pressure a chasing bomber applies.
		g.push = d * GRIND_SPEED * ENEMY_LINEAR_DAMP
		add_child_autofree(g)
		g.global_position = center + d * 1.0
		grinders.append(g)

	await wait_physics_frames(GRIND_FRAMES, "hold the grind against the corners")

	var excludes: Array[RID] = []
	for g in grinders:
		excludes.append(g.get_rid())
	var space := lm.get_world_3d().direct_space_state
	for i in range(grinders.size()):
		var g: RigidBody3D = grinders[i]
		var q := PhysicsRayQueryParameters3D.create(center, g.global_position)
		q.exclude = excludes
		var hit := space.intersect_ray(q)
		assert_true(hit.is_empty(),
			"grinder %d must stay reachable from the room — shell in the way means it leaked into the wall (body at %s)" % [i, g.global_position])


func test_a_chasing_bomber_never_leaks_through_the_corner_seam():
	# The production repro: a REAL bomber (angular convex hull, real AI —
	# detection is distance-only, so walls are invisible to its intent)
	# lured by a real parked ship placed diagonally beyond the room's
	# upper corner, where three curved pieces seam together. It grinds
	# with the full flip/retreat pounding cycle. However long it works
	# the seam, it must stay reachable from the room.
	var lm := LevelManager.new()
	add_child_autofree(lm)
	lm.generate_backdrop(4242)
	await wait_physics_frames(2, "let the shell enter the physics space")

	var center: Vector3 = lm.room_center(0)
	assert_ne(center, Vector3.ZERO, "the backdrop must build room 0")
	var corner_dir := Vector3(1, 1, 1).normalized()

	# The lure: a real ship, parked and frozen, in the "player" group —
	# 14 m out along the corner diagonal (outside the room, inside the
	# bomber's 25 m through-wall detection, beyond its 5 m fuse range).
	var player := ShipController.new()
	player.name = "Player"
	player.add_to_group("player")
	var pshape := CollisionShape3D.new()
	pshape.shape = SphereShape3D.new()
	player.add_child(pshape)
	add_child_autofree(player)
	player.global_position = center + corner_dir * 14.0
	player.freeze = true

	var bomber_keys := EnemyDrone.enemy_keys_with_ai("bomber")
	assert_gt(bomber_keys.size(), 0, "the grammar must declare a bomber for the wall test")
	if bomber_keys.is_empty():
		return
	var bomber = load("res://scenes/enemies/enemy.tscn").instantiate()
	bomber.enemy_key = bomber_keys[0]  # a bomber, found by capability
	add_child_autofree(bomber)
	bomber.global_position = center
	await wait_physics_frames(2, "let the bomber build its hull and find the lure")

	await wait_physics_frames(720, "12 s of grinding into the corner seam")

	var q := PhysicsRayQueryParameters3D.create(
		center, bomber.global_position)
	var excludes: Array[RID] = [bomber.get_rid(), player.get_rid()]
	q.exclude = excludes
	var hit := lm.get_world_3d().direct_space_state.intersect_ray(q)
	assert_true(hit.is_empty(),
		"the bomber must stay reachable from the room — shell in the way means it leaked into the corner seam (body at %s)" % bomber.global_position)


func test_the_sealed_room_shell_is_watertight_to_rays():
	# The direct hunt for the hole (playtest 2026-07-06: "like there's a
	# hole in there that causes a leak"): a sealed single room must stop
	# EVERY ray cast from its center. A ray that escapes to open space IS
	# a hole in the fused shell — found deterministically, no grinding
	# required. Dense uniform sphere plus extra-dense cones around the
	# eight corner diagonals, where three curved pieces seam together.
	var lm := LevelManager.new()
	add_child_autofree(lm)
	lm.generate_backdrop(4242)
	await wait_physics_frames(2, "let the shell enter the physics space")

	var center: Vector3 = lm.room_center(0)
	assert_ne(center, Vector3.ZERO, "the backdrop must build room 0")
	var space := lm.get_world_3d().direct_space_state

	var dirs: Array[Vector3] = []
	# Fibonacci sphere: ~800 uniform directions.
	var n := 800
	for i in range(n):
		var y := 1.0 - (2.0 * i + 1.0) / n
		var r := sqrt(maxf(0.0, 1.0 - y * y))
		var phi := i * 2.399963229728653  # golden angle
		dirs.append(Vector3(r * cos(phi), y, r * sin(phi)))
	# Corner cones: ~200 jittered rays inside ~30° of each corner diagonal.
	var rng := RandomNumberGenerator.new()
	rng.seed = 4242  # pinned — no fresh entropy in a contract
	for sx in [-1.0, 1.0]:
		for sy in [-1.0, 1.0]:
			for sz in [-1.0, 1.0]:
				var axis := Vector3(sx, sy, sz).normalized()
				for _i in range(200):
					var jitter := Vector3(
						rng.randf_range(-0.5, 0.5),
						rng.randf_range(-0.5, 0.5),
						rng.randf_range(-0.5, 0.5))
					dirs.append((axis + jitter).normalized())

	var escapes := []
	for d in dirs:
		var q := PhysicsRayQueryParameters3D.create(center, center + d * 200.0)
		var hit := space.intersect_ray(q)
		if hit.is_empty():
			escapes.append(d)
	assert_eq(escapes.size(), 0,
		"every ray from the room center must hit the shell — escapes are holes: %s" % [escapes.slice(0, 8)])


func _sweep_directions(rng_seed: int) -> Array[Vector3]:
	var dirs: Array[Vector3] = []
	var n := 800
	for i in range(n):
		var y := 1.0 - (2.0 * i + 1.0) / n
		var r := sqrt(maxf(0.0, 1.0 - y * y))
		var phi := i * 2.399963229728653
		dirs.append(Vector3(r * cos(phi), y, r * sin(phi)))
	var rng := RandomNumberGenerator.new()
	rng.seed = rng_seed
	for sx in [-1.0, 1.0]:
		for sy in [-1.0, 1.0]:
			for sz in [-1.0, 1.0]:
				var axis := Vector3(sx, sy, sz).normalized()
				for _i in range(200):
					var jitter := Vector3(
						rng.randf_range(-0.5, 0.5),
						rng.randf_range(-0.5, 0.5),
						rng.randf_range(-0.5, 0.5))
					dirs.append((axis + jitter).normalized())
	return dirs


func test_the_full_level_stops_every_ray_from_every_room():
	# The real levels carry the full panel variety, doorway framing, and
	# connector-adjacent corners the plain backdrop room lacks. A ray from
	# any room center may legally pass through doorways into neighboring
	# rooms — but the level is finite and every path ends at a wall. A ray
	# reaching 200 m reached the void: a hole in some room's skin.
	var lm := LevelManager.new()
	add_child_autofree(lm)
	lm.generate_level(4242, 8)
	await wait_physics_frames(2, "let the shells enter the physics space")
	var space := lm.get_world_3d().direct_space_state
	var dirs := _sweep_directions(4242)

	var escapes := []
	for room in range(64):
		var center: Vector3 = lm.room_center(room)
		if center == Vector3.ZERO:
			break
		for d in dirs:
			var q := PhysicsRayQueryParameters3D.create(center, center + d * 200.0)
			var hit := space.intersect_ray(q)
			if hit.is_empty():
				escapes.append("room %d dir %s" % [room, d])
	assert_eq(escapes.size(), 0,
		"every ray from every room must end at a wall — escapes are holes: %s" % [escapes.slice(0, 8)])


func test_a_pile_of_chasing_bombers_never_shoves_one_through_the_corner():
	# The pile-up: real fights press SEVERAL drones onto the same corner,
	# and mutual shoving multiplies the force on the hollow trimesh far
	# beyond one body's cruise — the classic penetration amplifier, and
	# the likeliest true mechanism behind the stuck-in-the-curve bomber.
	var lm := LevelManager.new()
	add_child_autofree(lm)
	lm.generate_backdrop(4242)
	await wait_physics_frames(2, "let the shell enter the physics space")

	var center: Vector3 = lm.room_center(0)
	assert_ne(center, Vector3.ZERO, "the backdrop must build room 0")
	var corner_dir := Vector3(1, 1, 1).normalized()

	var player := ShipController.new()
	player.name = "Player"
	player.add_to_group("player")
	var pshape := CollisionShape3D.new()
	pshape.shape = SphereShape3D.new()
	player.add_child(pshape)
	add_child_autofree(player)
	player.global_position = center + corner_dir * 14.0
	player.freeze = true

	var bomber_keys := EnemyDrone.enemy_keys_with_ai("bomber")
	assert_gt(bomber_keys.size(), 0, "the grammar must declare a bomber for the pile test")
	if bomber_keys.is_empty():
		return
	var bombers := []
	for i in range(4):
		var b = load("res://scenes/enemies/enemy.tscn").instantiate()
		b.enemy_key = bomber_keys[0]  # a bomber, found by capability
		add_child_autofree(b)
		# Staggered along the diagonal so the pile forms a shoving column.
		b.global_position = center - corner_dir * (i * 1.2)
		bombers.append(b)
	await wait_physics_frames(2, "let the pile build hulls and find the lure")

	await wait_physics_frames(720, "12 s of pile-up grinding on the corner seam")

	var excludes: Array[RID] = [player.get_rid()]
	for b in bombers:
		excludes.append(b.get_rid())
	var space := lm.get_world_3d().direct_space_state
	for i in range(bombers.size()):
		var b: RigidBody3D = bombers[i]
		var q := PhysicsRayQueryParameters3D.create(center, b.global_position)
		q.exclude = excludes
		var hit := space.intersect_ray(q)
		assert_true(hit.is_empty(),
			"bomber %d must stay reachable from the room — shell in the way means the pile shoved it into the wall (body at %s)" % [i, b.global_position])
