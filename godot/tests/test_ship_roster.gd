extends GutTest
## The hull roster: locked hulls cannot be flown until bought (green), the
## selection routes through GameManager's ownership check, and a hull change
## reaches the flying ship as a model swap. Roster data (specs, prices,
## weapons) is pinned in Rust (ship_type.rs); this suite covers the shell.

const TALON_SHIP_ID := 1        # ShipType::id (Talon costs 3000 organics)
# ShopItemId layout: stats 0-5 (Shields joined last), laser 6, life 7, then
# the green ladder — radar 8, map 9, Valkyrie 10, ships from 11.
const RADAR_ITEM := 8
const MAP_ITEM := 9
const VALKYRIE_ITEM := 10
const TALON_UNLOCK_ITEM := 11
const KIND_ORGANICS := 1
const KIND_HULL_REWARD := 2     # CurrencyKind::id of the red container
const VALKYRIE_UNLOCK := 2      # Unlock::id of Valkyrie

const UiStub := preload("res://tests/helpers/ui_stub.gd")


func _stack() -> GameManager:
	var root := Node3D.new()
	add_child_autofree(root)
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	var player := ShipController.new()
	player.name = "Player"
	var shape := CollisionShape3D.new()
	shape.name = "CollisionShape3D"
	shape.shape = SphereShape3D.new()
	player.add_child(shape)
	var gm := GameManager.new()
	root.add_child(lm)
	root.add_child(player)
	root.add_child(gm)
	gm.clear_save_for_tests()
	return gm


func test_locked_hull_cannot_be_selected_until_bought():
	var gm := _stack()
	gm.start_new_game()
	assert_eq(gm.get_ship_type_id(), 0, "a new run flies the starter")

	gm.on_ship_type_selected(TALON_SHIP_ID)
	assert_eq(gm.get_ship_type_id(), 0,
		"an unowned hull is refused by the authority, whatever the UI sends")

	# B7: hulls left the shop — the green ladder never reaches them, however
	# deep the wallet and whatever is owned. Bosses drop hulls now.
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	gm.on_cache_collected(KIND_ORGANICS, 5000, false)
	gm.on_portal_entered()
	gm.advance_to_shop()
	assert_true(gm.buy_shop_item(RADAR_ITEM), "the radar is the first rung")
	assert_true(gm.buy_shop_item(MAP_ITEM), "the radar opens the map")
	assert_true(gm.buy_shop_item(VALKYRIE_ITEM), "the map opens the Valkyrie")
	assert_false(gm.buy_shop_item(TALON_UNLOCK_ITEM),
		"even the full spine never puts a hull on sale — bosses drop them")
	assert_eq(gm.get_organics(), 5000 - 300 - 500 - 800,
		"the refused hull deducted nothing")

	# Since the LevelSpec (B12), a hull container grants only what the
	# level's spec STAGED — a forged red collect on an ordinary level is
	# refused outright, which this pins. The real grant→selectable arc
	# lives in test_boss_flow (level-6 walk → red container → ShipSelect).
	gm.on_cache_collected(KIND_HULL_REWARD, 12000, true)
	assert_eq(gm.owned_hull_count(), 0,
		"an unstaged hull container grants nothing — no forgery door")
	gm.on_ship_type_selected(TALON_SHIP_ID)
	assert_eq(gm.get_ship_type_id(), 0, "the hull stays locked")


func test_buying_the_valkyrie_arms_the_flying_ship():
	# GameManager pushes ownership to the ShipController on the grant receipt
	# and on every sync — the cannon works the moment the purchase lands,
	# and again after any respawn/continue rebuild.
	var gm := _stack()
	var player: ShipController = gm.get_parent().get_node("Player")
	gm.start_new_game()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	assert_false(player.is_valkyrie_owned(), "a fresh profile flies without the cannon")

	gm.on_cache_collected(KIND_ORGANICS, 2000, false)
	gm.on_portal_entered()
	gm.advance_to_shop()
	assert_true(gm.buy_shop_item(RADAR_ITEM), "rung one")
	assert_true(gm.buy_shop_item(MAP_ITEM), "rung two")
	assert_true(gm.buy_shop_item(VALKYRIE_ITEM), "rung three")
	assert_true(gm.has_unlock(VALKYRIE_UNLOCK), "the profile records the keystone")
	assert_true(player.is_valkyrie_owned(), "the receipt arms the cannon immediately")


func test_hull_change_swaps_the_flying_model():
	var player := ShipController.new()
	var shape := CollisionShape3D.new()
	shape.name = "CollisionShape3D"
	shape.shape = SphereShape3D.new()
	player.add_child(shape)
	add_child_autofree(player)
	await wait_process_frames(1)
	assert_not_null(player.get_node_or_null("Model"), "the starter model spawns")

	player.configure_ship(TALON_SHIP_ID, 0, 1.0)
	await wait_process_frames(2)  # the old model frees on the deferred boundary
	var models := player.find_children("Model", "", false, false)
	assert_eq(models.size(), 1, "exactly one hull model after the swap")


func test_cockpit_shell_wraps_the_camera_and_flips_with_the_view():
	var player := ShipController.new()
	var shape := CollisionShape3D.new()
	shape.name = "CollisionShape3D"
	shape.shape = SphereShape3D.new()
	player.add_child(shape)
	add_child_autofree(player)
	await wait_process_frames(1)

	var shell: Node3D = player.get_node_or_null("CockpitShell")
	var hull: Node3D = player.get_node_or_null("Model")
	assert_not_null(shell, "the cockpit shell spawns beside the hull model")
	assert_not_null(hull, "the starter model spawns")
	if shell == null or hull == null:
		return
	assert_true(shell.visible, "cockpit view (the default) shows the shell")
	assert_false(hull.visible, "cockpit view hides the exterior hull")

	player.toggle_view()
	assert_false(shell.visible, "chase view hides the shell")
	assert_true(hull.visible, "chase view shows the exterior hull")

	player.configure_ship(TALON_SHIP_ID, 0, 1.0)
	await wait_process_frames(2)
	assert_eq(player.find_children("CockpitShell", "", false, false).size(), 1,
		"the shared shell survives a hull swap untouched")


func test_reticle_rides_the_camera_axis():
	var player := ShipController.new()
	var cam := Camera3D.new()
	cam.name = "Camera3D"
	player.add_child(cam)
	var shape := CollisionShape3D.new()
	shape.name = "CollisionShape3D"
	shape.shape = SphereShape3D.new()
	player.add_child(shape)
	add_child_autofree(player)
	await wait_process_frames(1)

	var reticle: Node3D = player.get_node_or_null("Camera3D/Reticle")
	assert_not_null(reticle, "the depth-adaptive reticle spawns under the camera")
	if reticle == null:
		return
	assert_lt(reticle.position.z, 0.0, "the reticle sits in front of the camera")
	assert_almost_eq(reticle.position.x, 0.0, 0.001, "dead center on the aim axis")
	assert_almost_eq(reticle.position.y, 0.0, 0.001, "dead center on the aim axis")
