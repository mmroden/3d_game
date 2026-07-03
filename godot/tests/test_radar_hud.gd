extends GutTest
## The Enemy Radar unlock: the HUD pins edge arrows for enemies that are not
## visibly on screen, inside the safe band, only while GameManager has pushed
## the unlock flag. Arrow placement math lives in void-logic (radar.rs); this
## suite covers the shell contract: gating, group/visibility filtering, and
## band confinement. (EnemyDrone joins the "enemies" group in its ready() —
## enemy_drone.rs — which the combat suite exercises; the rig uses light
## group members so no level build is needed.)

func _rig() -> Dictionary:
	var root := Node3D.new()
	add_child_autofree(root)
	var player := Node3D.new()
	player.name = "Player"
	root.add_child(player)
	var camera := Camera3D.new()
	camera.name = "Camera3D"
	player.add_child(camera)
	var hud := HUD.new()
	hud.name = "HUD"
	root.add_child(hud)
	# ready() hides the HUD; GameManager shows it in Playing. The radar only
	# draws while visible, so present it as Playing would.
	hud.visible = true
	return {"root": root, "hud": hud, "camera": camera}

## A minimal radar contact: what the radar consumes is the "enemies" group +
## Node3D visibility/position (the contract EnemyDrone fulfils in ready()).
func _contact(root: Node3D, pos: Vector3) -> Node3D:
	var enemy := Node3D.new()
	enemy.add_to_group("enemies")
	root.add_child(enemy)
	enemy.global_position = pos
	return enemy

func _visible_arrows(hud: Node) -> Array:
	var out := []
	for poly in hud.find_children("*", "Polygon2D", true, false):
		if poly.visible:
			out.append(poly)
	return out


func test_locked_radar_draws_nothing():
	var rig := _rig()
	_contact(rig["root"], Vector3(0, 0, 50))  # behind the camera (-Z forward)
	await wait_process_frames(2)
	assert_eq(_visible_arrows(rig["hud"]).size(), 0,
		"without the unlock, the radar draws nothing")


func test_unlocked_radar_marks_offscreen_enemies_inside_the_band():
	var rig := _rig()
	var hud: HUD = rig["hud"]
	hud.set_unlock_flags(true, false)
	# Off-axis behind the camera — dead-center-behind is the (tested) no-
	# direction degenerate that correctly draws nothing.
	_contact(rig["root"], Vector3(5, 0, 50))
	_contact(rig["root"], Vector3(200, 0, -5))  # far right, in front
	await wait_process_frames(2)

	var arrows := _visible_arrows(hud)
	assert_eq(arrows.size(), 2, "both off-screen enemies get an arrow")

	# Every arrow stays inside the safe band (in mono: the whole window).
	var band: Rect2 = hud.find_children("*", "Control", true, false)[0].get_global_rect()
	for arrow in arrows:
		assert_true(band.grow(1.0).has_point(arrow.global_position),
			"arrow at %s must sit inside the band %s" % [arrow.global_position, band])


func test_onscreen_enemy_gets_no_arrow():
	var rig := _rig()
	var hud: HUD = rig["hud"]
	hud.set_unlock_flags(true, false)
	_contact(rig["root"], Vector3(0, 0, -20))  # dead ahead, on screen
	await wait_process_frames(2)
	assert_eq(_visible_arrows(hud).size(), 0,
		"a visibly on-screen enemy needs no arrow")


func test_hidden_enemies_are_off_the_radar():
	var rig := _rig()
	var hud: HUD = rig["hud"]
	hud.set_unlock_flags(true, false)
	var enemy := _contact(rig["root"], Vector3(0, 0, 50))
	enemy.visible = false  # dormant minion / culled room
	await wait_process_frames(2)
	assert_eq(_visible_arrows(hud).size(), 0,
		"dormant or culled enemies must not appear on the radar")


func test_revoking_visibility_clears_the_arrows():
	var rig := _rig()
	var hud: HUD = rig["hud"]
	hud.set_unlock_flags(true, false)
	_contact(rig["root"], Vector3(5, 0, 50))  # off-axis behind
	await wait_process_frames(2)
	assert_gt(_visible_arrows(hud).size(), 0, "sanity: an arrow is up")

	hud.visible = false  # leaving Playing hides the HUD
	# Arrows are children of the HUD; with the layer hidden nothing renders,
	# and the next pass parks the pool.
	await wait_process_frames(2)
	hud.visible = true
	hud.set_unlock_flags(false, false)
	await wait_process_frames(2)
	assert_eq(_visible_arrows(hud).size(), 0,
		"revoking the flag parks every arrow")
