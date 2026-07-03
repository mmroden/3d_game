extends GutTest
## The shop economy, end to end through the real GameManager + void-logic
## purchase authority: typed item ids, deduction, refusal on insufficient
## funds, the laser table, extra lives, and the two death flows (a spare life
## routes Death -> Shop -> same level; the last life ends the run).
##
## Item ids mirror void_logic::shop::ShopItemId: stat kinds 0-4 in
## UpgradeKind::ALL order (Thrust, Rotation, Armor, Fire Rate, Damage),
## then Laser = 5, ExtraLife = 6.

const THRUST_ID := 0      # costs 2_000 at base
const LASER_ID := 5       # first laser (Orange) costs 10_000
const EXTRA_LIFE_ID := 6  # first life costs 10_000
const RADAR_ID := 7       # Unlock::Radar, 300 organics
const RADAR_UNLOCK := 0   # Unlock::id of Radar
const KIND_COMPONENTS := 0
const KIND_ORGANICS := 1

const UiStub := preload("res://tests/helpers/ui_stub.gd")


## Build the minimal stack and drive the FSM into Playing. Returns the GameManager.
func _playing_game() -> GameManager:
	var root := Node3D.new()
	add_child_autofree(root)
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	var gm := GameManager.new()
	root.add_child(lm)
	root.add_child(gm)
	# Real persistence leaks profiles (organics, unlocks) across test runs;
	# every stack starts as a fresh install.
	gm.clear_save_for_tests()

	gm.start_new_game()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	assert_eq(gm.get_phase_name(), "Playing", "the stack must reach Playing")
	return gm

## Portal out of the level and into the shop with `funds` components banked.
func _shop_with_funds(gm: GameManager, funds: int) -> void:
	gm.on_cache_collected(0, funds)  # kind 0 = components
	gm.on_portal_entered()
	gm.advance_to_shop()
	assert_eq(gm.get_phase_name(), "Shop", "must reach the shop")


func test_stat_purchase_deducts_and_refusal_is_a_no_op():
	var gm := _playing_game()
	_shop_with_funds(gm, 2_500)

	assert_true(gm.buy_shop_item(THRUST_ID), "2.5k affords the 2k thrust upgrade")
	assert_eq(gm.get_components(), 500, "the purchase deducts exactly the price")

	assert_false(gm.buy_shop_item(THRUST_ID), "500 does not afford the next (pricier) thrust upgrade")
	assert_eq(gm.get_components(), 500, "a refused purchase deducts nothing")


func test_laser_purchase_upgrades_through_the_cost_table():
	var gm := _playing_game()
	_shop_with_funds(gm, 10_000)
	var level_before: int = gm.get_laser_level()

	assert_true(gm.buy_shop_item(LASER_ID), "10k affords the first laser upgrade")
	assert_eq(gm.get_laser_level(), level_before + 1, "the laser stepped one color")
	assert_eq(gm.get_components(), 0, "the Orange laser costs exactly 10k")


func test_spare_life_routes_death_through_shop_back_to_the_same_level():
	var gm := _playing_game()
	_shop_with_funds(gm, 15_000)
	assert_true(gm.buy_shop_item(EXTRA_LIFE_ID), "15k affords the first 10k life")
	assert_eq(gm.get_lives(), 2, "one spare life bought")

	# On to the next level with the spare life in the bank.
	gm.advance_to_next_level()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	assert_eq(gm.get_phase_name(), "Playing", "the run continues into level 2")
	var level: int = gm.get_current_level()
	var components_before: int = gm.get_components()

	# Lethal hit: the spare life keeps the run alive.
	gm.on_player_damaged(1000000.0, Vector3.ZERO)
	assert_eq(gm.get_phase_name(), "Death", "death shows the (life-lost) death screen")
	assert_eq(gm.get_lives(), 1, "the spare life was spent")
	assert_eq(gm.get_components(), components_before, "banked components survive a life loss")

	# Re-arm at the shop, then back into the SAME level.
	gm.on_respawn_pressed()
	assert_eq(gm.get_phase_name(), "Shop", "a lost life re-arms at the shop")
	gm.advance_to_next_level()
	assert_eq(gm.get_phase_name(), "Playing", "the shop's continue restarts the level")
	assert_eq(gm.get_current_level(), level, "a life loss never advances the level")


func test_last_life_ends_the_run_and_burns_the_salvage():
	var gm := _playing_game()
	gm.on_cache_collected(0, 5_000)
	assert_eq(gm.get_lives(), 1, "a fresh run has the one life")

	gm.on_player_damaged(1000000.0, Vector3.ZERO)
	assert_eq(gm.get_phase_name(), "Death", "the run is over")
	assert_eq(gm.get_components(), 0, "salvage burns up with the run")
	assert_eq(gm.get_lives(), 1, "the next run starts with the one life")


func test_unlock_purchase_spends_organics_and_survives_run_over():
	var gm := _playing_game()
	gm.on_cache_collected(KIND_ORGANICS, 400)
	gm.on_cache_collected(KIND_COMPONENTS, 5_000)
	gm.on_portal_entered()
	gm.advance_to_shop()

	assert_true(gm.buy_shop_item(RADAR_ID), "400 organics affords the 300 radar")
	assert_eq(gm.get_organics(), 100, "the radar spends the green account")
	assert_eq(gm.get_components(), 5_000, "the blue account is untouched")
	assert_true(gm.has_unlock(RADAR_UNLOCK), "the radar is owned")
	assert_false(gm.buy_shop_item(RADAR_ID), "owned unlocks cannot be bought again")

	# Back into play, then run over: the unlock is permanent.
	gm.advance_to_next_level()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	gm.on_player_damaged(1000000.0, Vector3.ZERO)
	assert_eq(gm.get_phase_name(), "Death", "the run ended")
	assert_true(gm.has_unlock(RADAR_UNLOCK), "permanent unlocks survive run-over")
	assert_eq(gm.get_organics(), 100, "organics survive run-over")


func test_shop_ui_renders_one_row_per_offer_plus_continue():
	# Pure presentation check against the real ShopUI: N offers render N rows
	# and N detail hints, plus title, the two balances, and the Continue row.
	var shop := ShopUI.new()
	add_child_autofree(shop)
	shop.show_shop(
		1_000, 50,
		PackedInt32Array([0, 5, 6]),
		PackedStringArray(["Thrust +10%", "Laser: Orange", "Extra Life"]),
		PackedStringArray(["now ×1.00 → ×1.10", "damage 2 → 3", "lives 1 → 2"]),
		PackedInt64Array([2_000, 10_000, 10_000]),
		PackedByteArray([3, 3, 3]),
	)
	var labels := shop.find_children("*", "Label", true, false)
	assert_eq(labels.size(), 3 + 3 + 4,
		"3 offer rows + 3 detail hints + title + components + organics + continue")
	assert_true(shop.visible, "show_shop presents the screen")
