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

## Push a contact set covering these enemies — in play, LevelManager scopes
## the set to the lit neighborhood (RADAR_ROOM_DEPTH) and GameManager pushes
## it on every room change; the HUD draws ONLY what was pushed.
func _push_contacts(hud: HUD, enemies: Array) -> void:
	var ids := PackedInt64Array()
	for e in enemies:
		ids.append(e.get_instance_id())
	hud.set_radar_contacts(ids)

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
	var a := _contact(rig["root"], Vector3(5, 0, 50))
	var b := _contact(rig["root"], Vector3(200, 0, -5))  # far right, in front
	_push_contacts(hud, [a, b])
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
	var onscreen := _contact(rig["root"], Vector3(0, 0, -20))  # dead ahead, on screen
	_push_contacts(hud, [onscreen])
	await wait_process_frames(2)
	assert_eq(_visible_arrows(hud).size(), 0,
		"a visibly on-screen enemy needs no arrow")


func test_hidden_enemies_are_off_the_radar():
	var rig := _rig()
	var hud: HUD = rig["hud"]
	hud.set_unlock_flags(true, false)
	var enemy := _contact(rig["root"], Vector3(0, 0, 50))
	_push_contacts(hud, [enemy])
	enemy.visible = false  # dormant minion / culled room
	await wait_process_frames(2)
	assert_eq(_visible_arrows(hud).size(), 0,
		"dormant or culled enemies must not appear on the radar")


func test_revoking_visibility_clears_the_arrows():
	var rig := _rig()
	var hud: HUD = rig["hud"]
	hud.set_unlock_flags(true, false)
	var behind := _contact(rig["root"], Vector3(5, 0, 50))  # off-axis behind
	_push_contacts(hud, [behind])
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


func test_hud_text_is_readable_and_outlined():
	# Playtest (2026-07-04): in-game text is too small and drowns in the
	# scene. Every HUD label draws from the ui_style HUD ladder (>= 26px)
	# with a dark outline so it reads against any backdrop.
	var hud := HUD.new()
	add_child_autofree(hud)
	await wait_process_frames(1)
	var labels := hud.find_children("*", "Label", true, false)
	assert_gt(labels.size(), 3, "the HUD builds its labels in ready")
	for l in labels:
		var size: int = l.get_theme_font_size("font_size")
		var outline: int = l.get_theme_constant("outline_size")
		assert_gte(size, 26, "%s must be couch-readable, got %dpx" % [l.text, size])
		assert_gte(outline, 4, "%s needs an outline to read against the scene" % l.text)


func test_only_pushed_contacts_reach_the_radar():
	# Playtest (2026-07-04): an arrow for every enemy on the level is noise.
	# The radar's scope is the lit neighborhood — LevelManager computes it
	# at RADAR_ROOM_DEPTH, GameManager pushes it on room changes, and the
	# HUD draws ONLY what was pushed. Nothing pushed, nothing drawn.
	var rig := _rig()
	var hud: HUD = rig["hud"]
	hud.set_unlock_flags(true, false)
	var near := _contact(rig["root"], Vector3(5, 0, 50))
	var _far := _contact(rig["root"], Vector3(-5, 0, 50))
	await wait_process_frames(2)
	assert_eq(_visible_arrows(hud).size(), 0,
		"before any contact push the radar is silent")
	_push_contacts(hud, [near])
	await wait_process_frames(2)
	assert_eq(_visible_arrows(hud).size(), 1,
		"only the pushed contact gets an arrow — the far one is out of scope")


func test_radar_contacts_cover_the_neighborhood_not_the_level():
	# Full stack: the wire LevelManager → GameManager → HUD scopes contacts
	# to the lit neighborhood; the level-wide enemy roster is bigger.
	const UiStub := preload("res://tests/helpers/ui_stub.gd")
	var root := Node3D.new()
	add_child_autofree(root)
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	var gm := GameManager.new()
	gm.fixed_seed = 1
	root.add_child(lm)
	root.add_child(gm)
	gm.clear_save_for_tests()
	gm.start_new_game()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	await wait_process_frames(4)  # deferred build + first culling pass

	var contacts: PackedInt64Array = lm.radar_contacts()
	var roster := 0
	for e in lm.find_children("*", "EnemyDrone", true, false):
		roster += 1
	assert_gt(roster, 0, "the pinned level fields enemies")
	assert_lt(contacts.size(), roster,
		"the radar scope (depth 1) must exclude the level's far rooms")
