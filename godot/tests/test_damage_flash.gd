extends GutTest
## The damage tint (playtest 2026-07-06): taking a hit briefly flashes a deep
## full-screen tinge that fades over ~5s — amber when the shield absorbs it,
## red on a hull breach. The pure decay + color policy is unit-tested in
## void-logic (damage_flash.rs); this pins the shell contract — the HUD renders
## the overlay, and GameManager reports the hit's layer.

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


func test_the_tint_flashes_by_layer_and_fades():
	var hud := _hud()
	await wait_process_frames(2, "let the HUD build its overlays")
	var tint := _tint(hud)
	assert_not_null(tint, "the HUD builds a DamageTint overlay")
	if tint == null:
		return
	assert_false(tint.visible, "no hit, no tint")

	# Hull breach → red.
	hud.flash_damage(true)
	await wait_process_frames(2, "render the fresh flash")
	assert_true(tint.visible, "a hit lights the tint")
	assert_gt(tint.color.a, 0.25, "a fresh hit is a deep tinge")
	assert_lt(tint.color.a, 0.4, "but never a solid wash")
	assert_gt(tint.color.r, tint.color.g, "a hull breach reads red")
	assert_gt(tint.color.r, tint.color.b, "a hull breach reads red")

	# It fades over time.
	var peak: float = tint.color.a
	await wait_process_frames(40, "watch it decay")
	assert_lt(tint.color.a, peak, "the tint fades frame over frame")
	assert_gt(tint.color.a, 0.0, "but the 5s decay is far from done")

	# A shield-held hit relights it amber.
	hud.flash_damage(false)
	await wait_process_frames(2, "render the shield flash")
	assert_gt(tint.color.g, 0.5, "a held shield reads amber (strong green)")
	assert_lt(tint.color.b, tint.color.g, "amber, not white")


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


func test_taking_damage_flashes_the_screen_through_the_manager():
	# The real path: a projectile hit reaches GameManager.on_player_damaged,
	# which reports the struck layer to the HUD tint.
	var stack := _playing_stack()
	var gm: GameManager = stack["gm"]
	var hud: HUD = stack["hud"]
	await wait_process_frames(2)
	var tint := _tint(hud)
	assert_not_null(tint, "the HUD has a tint overlay")
	if tint == null:
		return
	assert_false(tint.visible, "no hit yet")

	# A hit big enough to breach the shield and reach the hull → red flash.
	gm.on_player_damaged(120.0, Vector3.ZERO)
	await wait_process_frames(2, "render the flash")
	assert_true(tint.visible, "taking damage flashes the screen")
	assert_gt(tint.color.r, tint.color.g, "a hull breach reads red")
