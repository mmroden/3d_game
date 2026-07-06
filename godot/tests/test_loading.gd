extends GutTest
## Every level build — new game, Continue, the next level after a shop stop,
## a respawn — funnels through GameManager's one entry into Playing, and a
## synchronous build there reads as a freeze (playtest 2026-07-03). The
## contract: the loading veil goes up the frame the phase flips, the build
## waits for a rendered frame, and the veil lifts once the sector exists.

const UiStub := preload("res://tests/helpers/ui_stub.gd")


func _stack() -> GameManager:
	var root := Node3D.new()
	add_child_autofree(root)
	# No LoadingUI stub here — this suite observes the REAL veil below.
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI"]:
		var stub := UiStub.new()
		stub.name = ui_name
		root.add_child(stub)
	var loading := LoadingUI.new()
	loading.name = "LoadingUI"
	root.add_child(loading)
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	var gm := GameManager.new()
	gm.fixed_seed = 1
	root.add_child(lm)
	root.add_child(gm)
	gm.clear_save_for_tests()
	return gm


func test_entering_playing_veils_the_build_then_lifts():
	var gm := _stack()
	var root := gm.get_parent()
	var loading: LoadingUI = root.get_node("LoadingUI")
	var lm: LevelManager = root.get_node("LevelManager")

	gm.start_new_game()
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	assert_eq(gm.get_phase_name(), "Playing", "must reach Playing")

	# The frame the phase flips: veil up, no enemies yet — the build waits
	# for a rendered frame (the backdrop room may still be dying, so the
	# enemy roster is the build's signature, not Room0).
	assert_true(loading.visible, "the veil must cover the build")
	assert_eq(lm.find_children("*", "EnemyDrone", true, false).size(), 0,
		"the sector build waits for a rendered frame")

	await wait_process_frames(4)
	assert_gt(lm.find_children("*", "EnemyDrone", true, false).size(), 0,
		"the deferred build lands")
	assert_false(loading.visible, "the veil lifts once the sector exists")
