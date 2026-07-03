extends GutTest
## The visible enemy model yaws to face the player every physics frame, even
## though the RigidBody itself is rotation-locked (so impacts can't tumble it).
## The mech models import facing sideways, so a static model points its flank at
## the player; this pins that the model re-aims.
##
## We assert the *unambiguous* contract: the model keeps the player at a constant
## bearing in its own local frame as the player moves around it. That holds for
## any per-type forward offset, so the offset stays a free visual-tune knob and
## this test can't bake in a guessed nose axis.

const ENEMY_SCENE = "res://scenes/enemies/enemy.tscn"


func _make_player() -> Node3D:
	var player := Node3D.new()
	player.add_to_group("player")
	add_child_autofree(player)
	return player


func test_model_faces_player_and_tracks_as_it_moves():
	var player := _make_player()
	var enemy = load(ENEMY_SCENE).instantiate()
	enemy.enemy_type_id = 0 # GunDrone — the mech that was facing sideways
	add_child_autofree(enemy)
	enemy.global_position = Vector3.ZERO

	var pivot = enemy.get_node_or_null("ModelPivot")
	assert_not_null(pivot,
		"enemy model must hang off a ModelPivot that yaws independently of the locked body")
	if pivot == null:
		return

	var bearings: Array = []
	for player_pos in [Vector3(10, 0, 3), Vector3(-6, 0, 8), Vector3(4, 0, -9)]:
		player.global_position = player_pos
		await wait_physics_frames(3, "let the drone re-aim at the player")
		var dir: Vector3 = (player_pos - enemy.global_position).normalized()
		# Direction to the player expressed in the model's own frame. If the
		# model truly faces the player, this local bearing is identical no
		# matter where the player has moved to.
		var local: Vector3 = pivot.global_transform.basis.inverse() * dir
		bearings.append(local)

	for i in range(1, bearings.size()):
		assert_almost_eq(bearings[i].x, bearings[0].x, 0.1,
			"model is not holding a constant bearing on the player (x) — it isn't tracking")
		assert_almost_eq(bearings[i].z, bearings[0].z, 0.1,
			"model is not holding a constant bearing on the player (z) — it isn't tracking")
