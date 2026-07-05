extends GutTest
## The shop economy, end to end through the real GameManager + void-logic
## purchase authority: typed item ids, deduction, refusal on insufficient
## funds, the laser table, extra lives, and the two death flows (a spare life
## routes Death -> Shop -> same level; the last life ends the run).
##
## Item ids mirror void_logic::shop::ShopItemId: stat kinds 0-5 in
## UpgradeKind::ALL order (Thrust, Rotation, Armor, Fire Rate, Damage,
## Shields), then Laser = 6, ExtraLife = 7.

const THRUST_ID := 0      # costs 2_000 at base
const LASER_ID := 6       # first laser (Orange) costs 10_000
const EXTRA_LIFE_ID := 7  # first life costs 10_000
const RADAR_ID := 8       # Unlock::Radar, 300 organics
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
	gm.on_cache_collected(0, funds, false)  # kind 0 = components
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
	gm.on_cache_collected(0, 5_000, false)
	assert_eq(gm.get_lives(), 1, "a fresh run has the one life")

	gm.on_player_damaged(1000000.0, Vector3.ZERO)
	assert_eq(gm.get_phase_name(), "Death", "the run is over")
	assert_eq(gm.get_components(), 0, "salvage burns up with the run")
	assert_eq(gm.get_lives(), 1, "the next run starts with the one life")


func test_unlock_purchase_spends_organics_and_survives_run_over():
	var gm := _playing_game()
	gm.on_cache_collected(KIND_ORGANICS, 400, false)
	gm.on_cache_collected(KIND_COMPONENTS, 5_000, false)
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
	assert_eq(labels.size(), 3 + 3 + 5,
		"3 offer rows + 3 detail hints + title + balances ×2 + Continue + Save & Exit")
	assert_true(shop.visible, "show_shop presents the screen")


class ShopRecordingStub:
	extends UiStub
	var shop_pushes := 0
	func show_shop(_c: int, _o: int, _ids: PackedInt32Array, _labels: PackedStringArray, _details: PackedStringArray, _costs: PackedInt64Array, _flags: PackedByteArray) -> void:
		shop_pushes += 1


func test_the_shop_returns_after_every_level():
	# Playtest (2026-07-04): the shop appeared after level 1 but not after
	# level 2. Walk two full levels through the real phase machine; the shop
	# must be pushed to the UI both times.
	var root := Node3D.new()
	add_child_autofree(root)
	var shop_stub: ShopRecordingStub = null
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub: UiStub
		if ui_name == "ShopUI":
			shop_stub = ShopRecordingStub.new()
			stub = shop_stub
		else:
			stub = UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	var gm := GameManager.new()
	root.add_child(lm)
	root.add_child(gm)
	gm.clear_save_for_tests()

	gm.start_new_game()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	await wait_process_frames(3)  # let the deferred sector build land

	gm.on_portal_entered()
	gm.advance_to_shop()
	assert_eq(gm.get_phase_name(), "Shop", "level 1 ends at the shop")
	assert_eq(shop_stub.shop_pushes, 1, "the level-1 shop is pushed")

	gm.advance_to_next_level()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	assert_eq(gm.get_phase_name(), "Playing", "level 2 must start")
	await wait_process_frames(3)

	gm.on_portal_entered()
	gm.advance_to_shop()
	assert_eq(gm.get_phase_name(), "Shop", "level 2 must end at the shop too")
	assert_eq(shop_stub.shop_pushes, 2, "the level-2 shop must be pushed")


func test_one_press_on_the_level_2_summary_lands_in_the_shop_not_past_it():
	# Playtest (2026-07-04): after the second level the player went straight
	# into the third and could buy nothing. Mechanism: leaving the level-1
	# shop via Continue parked the cursor on the Continue row, and the ONE
	# press that dismissed the level-2 kill summary was still just_pressed
	# when the shop appeared in the same frame — it chained through the shop
	# (Continue), then ship select, menu after menu. This drives the two
	# button moments with REAL UIs and real presses: the press that closes
	# the summary must land the player IN the shop, cursor at the top.
	var root := Node3D.new()
	add_child_autofree(root)
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	var summary := KillSummaryUI.new()
	summary.name = "KillSummaryUI"
	root.add_child(summary)
	var shop := ShopUI.new()
	shop.name = "ShopUI"
	root.add_child(shop)
	var ship_select := ShipSelectUI.new()
	ship_select.name = "ShipSelectUI"
	root.add_child(ship_select)
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	var gm := GameManager.new()
	root.add_child(lm)
	root.add_child(gm)
	gm.clear_save_for_tests()

	# Level 1, driven directly (the two button moments come later).
	gm.start_new_game()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	await wait_process_frames(3)
	gm.on_portal_entered()
	gm.advance_to_shop()
	await wait_process_frames(2)

	# Leave the level-1 shop the way a player does: walk to Continue (a
	# fresh catalog has 9 rows — 6 stats, laser, life, radar) and press it.
	# This is what used to park the cursor on Continue for the next visit.
	for _i in range(9):
		Input.action_press("menu_down")
		await wait_process_frames(2)
		Input.action_release("menu_down")
		await wait_process_frames(1)
	Input.action_press("menu_select")
	await wait_process_frames(2)
	Input.action_release("menu_select")
	await wait_process_frames(1)
	assert_eq(gm.get_phase_name(), "ShipSelect", "Continue leaves the level-1 shop")

	# Level 2, driven directly again.
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	assert_eq(gm.get_phase_name(), "Playing", "level 2 must start")
	await wait_process_frames(3)
	gm.on_portal_entered()
	assert_eq(gm.get_phase_name(), "KillSummary")
	await wait_process_frames(2)

	# THE moment: one press dismisses the summary. The player must land IN
	# the shop — not chained past it into ship select or level 3.
	Input.action_press("menu_select")
	await wait_process_frames(2)
	Input.action_release("menu_select")
	await wait_process_frames(2)
	assert_eq(gm.get_phase_name(), "Shop",
		"the press that closes the summary must stop at the shop")
	assert_true(shop.visible, "the storefront is on screen")
	assert_false(ship_select.visible, "and it did not chain into ship select")

	# And the shop is USABLE: the next press buys the top row (Thrust,
	# 2000 — affordable with the level's salvage untouched by upgrades).
	gm.on_cache_collected(KIND_COMPONENTS, 2_000, false)
	var components_before: int = gm.get_components()
	Input.action_press("menu_select")
	await wait_process_frames(2)
	Input.action_release("menu_select")
	await wait_process_frames(1)
	assert_lt(gm.get_components(), components_before,
		"the player can actually buy something after level 2")
	assert_eq(gm.get_phase_name(), "Shop", "buying keeps the shop open")


func test_save_and_exit_banks_the_run_at_the_next_level():
	# The shop's Save & Exit row (owner's ask 2026-07-04): everything up to
	# this point — purchases included — banks as the next level's start, and
	# the player lands on the main menu with Continue armed.
	var gm := _playing_game()
	await wait_process_frames(3)
	gm.on_cache_collected(KIND_COMPONENTS, 30_000, false)
	gm.on_portal_entered()
	gm.advance_to_shop()
	assert_true(gm.buy_shop_item(THRUST_ID), "bank a purchase before leaving")
	var components_after_buy: int = gm.get_components()

	gm.save_and_exit()
	assert_eq(gm.get_phase_name(), "MainMenu", "Save & Exit lands on the menu")
	assert_true(gm.has_continuable_run(), "the run is banked")

	gm.continue_game()
	assert_eq(gm.get_phase_name(), "Playing", "Continue resumes the banked run")
	assert_eq(gm.get_current_level(), 2, "the run resumes at the NEXT level")
	assert_eq(gm.get_components(), components_after_buy, "purchases and salvage survived")


func test_the_shield_surge_stocks_charges_and_spends_on_the_trigger():
	# The Shield Surge: green item arrives with three charges, refills are
	# 5k blue, and the item trigger spends one for instant shields.
	var gm := _playing_game()
	await wait_process_frames(3)
	gm.on_cache_collected(KIND_ORGANICS, 2_000, false)
	gm.on_cache_collected(KIND_COMPONENTS, 30_000, false)
	gm.on_portal_entered()
	gm.advance_to_shop()
	assert_true(gm.buy_shop_item(RADAR_ID), "the radar opens the surge branch")
	assert_true(gm.buy_shop_item(16), "the surge item sells green")
	assert_eq(gm.get_shield_charges(), 3, "the item arrives stocked")
	var components_before: int = gm.get_components()
	assert_true(gm.buy_shop_item(17), "a refill sells blue")
	assert_eq(components_before - gm.get_components(), 5_000, "refills are 5k flat")
	assert_eq(gm.get_shield_charges(), 4)

	gm.advance_to_next_level()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	await wait_process_frames(3)
	gm.on_shield_burst_requested()
	assert_eq(gm.get_shield_charges(), 3, "the trigger spends a charge")
