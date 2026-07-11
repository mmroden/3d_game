extends GutTest
## The damage tint is the HULL-SEVERITY alarm (owner 2026-07-09): silent
## while the shield absorbs, and on a breach it tints at the health bar's
## own POST-hit zone — nothing while the bar is green, yellow when the bar
## is-or-goes yellow, red when it is-or-goes red. ONE health crossing: the
## tint reads the SAME update_health push the bar renders (no second
## fraction crossing), so GameManager pushes before it flashes. The pure
## decay + zone policy is unit-tested in void-logic (damage_flash.rs);
## this pins the shell contract.

const UiStub := preload("res://tests/helpers/ui_stub.gd")


func _hud() -> HUD:
	var root := Node3D.new()
	add_child_autofree(root)
	var hud := HUD.new()
	hud.name = "HUD"
	root.add_child(hud)
	# ready() hides the HUD; the tint overlay's own visibility is driven by the
	# flash state regardless, so present it as Playing would.
	hud.visible = true
	return hud


func _tint(hud: HUD) -> ColorRect:
	return hud.find_child("DamageTint", true, false)


func test_the_tint_mirrors_the_health_bars_zone():
	var hud := _hud()
	await wait_process_frames(2, "let the HUD build its overlays")
	var tint := _tint(hud)
	assert_not_null(tint, "the HUD builds a DamageTint overlay")
	if tint == null:
		return
	assert_false(tint.visible, "no hit, no tint")

	# A breach with the bar still GREEN raises no alarm. The zone comes from
	# the bar's own push — the one health crossing.
	hud.update_health(80.0, 100.0)
	hud.flash_damage(true)
	await wait_process_frames(2, "render")
	assert_false(tint.visible, "a green-zone breach raises no alarm")

	# The bar goes yellow → yellow tint.
	hud.update_health(40.0, 100.0)
	hud.flash_damage(true)
	await wait_process_frames(2, "render the fresh flash")
	assert_true(tint.visible, "a yellow-zone breach lights the tint")
	assert_gt(tint.color.a, 0.25, "a fresh hit is a deep tinge")
	assert_lt(tint.color.a, 0.4, "but never a solid wash")
	assert_gt(tint.color.g, 0.5, "the bar is yellow, so the tint is yellow")
	assert_lt(tint.color.b, tint.color.g, "yellow, not white")

	# It fades over time.
	var peak: float = tint.color.a
	await wait_process_frames(40, "watch it decay")
	assert_lt(tint.color.a, peak, "the tint fades frame over frame")
	assert_gt(tint.color.a, 0.0, "but the 5s decay is far from done")

	# The bar goes red → red tint.
	hud.update_health(20.0, 100.0)
	hud.flash_damage(true)
	await wait_process_frames(2, "render the red flash")
	assert_gt(tint.color.r, tint.color.g, "the bar is red, so the tint is red")
	assert_gt(tint.color.r, tint.color.b, "red, not white")


func test_shield_hits_never_tint():
	# The shield bar tells the shield's story — the tint only speaks for
	# the hull (owner 2026-07-09: no diminish, no alarm).
	var hud := _hud()
	await wait_process_frames(2, "let the HUD build its overlays")
	var tint := _tint(hud)
	if tint == null:
		return
	hud.update_health(20.0, 100.0)  # even with the bar deep in the red
	hud.flash_damage(false)
	await wait_process_frames(2, "render")
	assert_false(tint.visible, "a held shield raises no hull alarm")


func _playing_stack() -> Dictionary:
	var root := Node3D.new()
	add_child_autofree(root)
	for ui_name in ["MainMenuUI", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	var hud := HUD.new()
	hud.name = "HUD"
	root.add_child(hud)
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	var player := ShipController.new()
	player.name = "Player"
	player.add_to_group("player")
	var shape := CollisionShape3D.new()
	shape.name = "CollisionShape3D"
	shape.shape = SphereShape3D.new()
	player.add_child(shape)
	var gm := GameManager.new()
	root.add_child(lm)
	root.add_child(player)
	root.add_child(gm)
	gm.clear_save_for_tests()
	gm.start_new_game()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	assert_eq(gm.get_phase_name(), "Playing", "the stack must reach Playing")
	return {"gm": gm, "hud": hud}


func test_taking_damage_tints_by_the_bars_post_hit_zone():
	# The real path — and the ORDERING pin: on_player_damaged must push the
	# bar BEFORE it flashes, or the tint reads last frame's (pre-hit) zone
	# and a threshold-crossing hit raises the wrong alarm. This single hit
	# crosses from full straight below green: the tint must see the POST-hit
	# bar within the same callback, before any process frame runs. Expected
	# zone is DERIVED from the same fraction the bar shows — never pinned to
	# tuning (a loadout change must not break this).
	var stack := _playing_stack()
	var gm: GameManager = stack["gm"]
	var hud: HUD = stack["hud"]
	await wait_process_frames(2)
	var tint := _tint(hud)
	assert_not_null(tint, "the HUD has a tint overlay")
	if tint == null:
		return
	assert_false(tint.visible, "no hit yet")

	# A hit big enough to breach the shield and bite deep into the hull.
	gm.on_player_damaged(120.0, Vector3.ZERO)
	var fraction: float = gm.get_health() / gm.get_max_health()
	assert_lt(fraction, 0.5,
		"fixture sanity: the big hit lands the bar below green")
	await wait_process_frames(2, "render the flash")
	assert_true(tint.visible, "a below-green breach flashes the screen")
	if fraction > 0.25:
		assert_gt(tint.color.g, 0.5, "yellow bar, yellow tint")
	else:
		assert_gt(tint.color.r, tint.color.g, "red bar, red tint")
