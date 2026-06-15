extends GutTest
## The no-infinite-spin invariant moved from the deleted owned-sim into the
## shell: ShipController.apply_envelope sets damping = -ln(retention) (always
## > 0), and fly() steers via torque toward a commanded angular velocity that
## is zero when the stick is centered — so spin decays to rest. Its old
## tests went with the old sim; this re-pins both halves on the real node.

func test_damping_is_positive():
	var ship = ShipController.new()
	add_child_autofree(ship)
	await wait_physics_frames(2)
	assert_gt(ship.angular_damp, 0.0,
		"angular_damp must be > 0 — the no-infinite-spin invariant")
	assert_gt(ship.linear_damp, 0.0,
		"linear_damp shares the same retention decay")


func test_commanded_spin_decays_to_rest_without_input():
	var ship = ShipController.new()
	add_child_autofree(ship)
	await wait_physics_frames(2)
	# Spin it, then leave the stick centered: torque toward a zero command
	# plus angular damping must drive it back toward rest.
	ship.angular_velocity = Vector3(0.0, 3.0, 0.0)
	var initial: float = ship.angular_velocity.length()
	await wait_physics_frames(120)
	var final: float = ship.angular_velocity.length()
	assert_lt(final, initial * 0.5,
		"centered stick must decay spin toward rest (got %.3f from %.3f)" % [final, initial])
