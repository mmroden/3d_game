extends GutTest
## "Continue" is gated on a continuable run — one that finished at least one
## level. The snapshot is written at the start of level 2+, cleared on
## run-over, and the main menu shows Continue only while GameManager says one
## exists (the menu never reads disk).

const UiStub := preload("res://tests/helpers/ui_stub.gd")

## Records GameManager's Continue-availability pushes; everything else is
## the shared UiStub contract (tests/helpers/ui_stub.gd).
class MenuRecordingStub:
	extends UiStub
	var continue_available_pushes: Array = []
	func set_continue_available(a: bool, restarts: bool) -> void:
		continue_available_pushes.append([a, restarts])


func _stack() -> Dictionary:
	var root := Node3D.new()
	add_child_autofree(root)
	var menu_stub: MenuRecordingStub = null
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := MenuRecordingStub.new()
		stub.name = ui_name
		root.add_child(stub)
		if ui_name == "MainMenuUI":
			menu_stub = stub
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	var gm := GameManager.new()
	root.add_child(lm)
	root.add_child(gm)
	# Real persistence leaks profiles across test runs; start fresh.
	gm.clear_save_for_tests()
	return {"gm": gm, "menu": menu_stub}

func _into_playing(gm: GameManager) -> void:
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	assert_eq(gm.get_phase_name(), "Playing", "must reach Playing")


func test_continue_resumes_only_after_finishing_a_level():
	# B8: any persisted profile offers a Continue (restart mode) — but the
	# RESUME mode arrives only with the snapshot, at the start of level 2+.
	var s := _stack()
	var gm: GameManager = s["gm"]

	gm.start_new_game()
	assert_false(gm.has_continuable_run(),
		"a truly clean slate has nothing to continue")
	_into_playing(gm)
	if gm.has_continuable_run():
		assert_true(gm.continue_restarts_run(),
			"mid-level-1 any Continue is a restart — no snapshot exists yet")

	# Finish level 1: portal -> summary -> shop -> ship select -> bestiary -> level 2.
	gm.on_portal_entered()
	gm.advance_to_shop()
	gm.advance_to_next_level()
	_into_playing(gm)
	assert_eq(gm.get_current_level(), 2, "the run reached level 2")
	assert_true(gm.has_continuable_run(),
		"finishing a level makes the run continuable (snapshot at level start)")
	assert_false(gm.continue_restarts_run(),
		"…and now Continue RESUMES the run instead of restarting")


func test_run_over_switches_continue_to_restart_mode():
	# B8 (owner's call): run-over kills the SNAPSHOT but not the Continue —
	# the roguelite loop restarts sector 1 with the profile applied. The
	# menu row flips to restart mode ("Continue — Restart Sector 1").
	var s := _stack()
	var gm: GameManager = s["gm"]
	gm.start_new_game()
	_into_playing(gm)
	gm.on_portal_entered()
	gm.advance_to_shop()
	gm.advance_to_next_level()
	_into_playing(gm)
	assert_true(gm.has_continuable_run(), "sanity: continuable after level 1")
	assert_false(gm.continue_restarts_run(),
		"a live snapshot resumes — no restart mode")

	gm.on_player_damaged(1000000.0, Vector3.ZERO)  # last life: run over
	assert_eq(gm.get_phase_name(), "Death", "the run ended")
	assert_true(gm.has_continuable_run(),
		"run-over keeps a Continue: the profile carries the roguelite loop")
	assert_true(gm.continue_restarts_run(),
		"…but it restarts sector 1, not the dead run")


func test_profile_continue_restarts_sector_1_with_greens_kept():
	var s := _stack()
	var gm: GameManager = s["gm"]
	gm.start_new_game()
	_into_playing(gm)
	gm.on_cache_collected(1, 700)  # green — enough for the 300 radar
	gm.on_portal_entered()
	gm.advance_to_shop()
	assert_true(gm.buy_shop_item(8), "own the radar so the profile has teeth")
	var organics_after_buy := gm.get_organics()
	gm.advance_to_next_level()
	_into_playing(gm)
	assert_eq(gm.get_current_level(), 2, "the dead run had reached level 2")

	gm.on_player_damaged(1000000.0, Vector3.ZERO)  # run over
	assert_eq(gm.get_phase_name(), "Death")
	gm.return_to_menu()

	gm.continue_game()
	assert_eq(gm.get_phase_name(), "Playing",
		"the profile-continue starts a fresh run directly")
	assert_eq(gm.get_current_level(), 1, "…at sector 1, not the dead run's level")
	assert_eq(gm.get_organics(), organics_after_buy, "greens survive the run-over")
	assert_true(gm.has_unlock(0), "the radar survives the run-over")


func test_menu_receives_the_availability_push_on_show():
	var s := _stack()
	var gm: GameManager = s["gm"]
	var menu: MenuRecordingStub = s["menu"]

	# GameManager pushes availability whenever the menu is shown (including
	# the deferred initial phase after ready).
	await wait_process_frames(2)
	assert_gt(menu.continue_available_pushes.size(), 0,
		"showing the menu pushes Continue availability — the menu never reads disk")


## Minimal GameManager stand-in so a standalone MainMenuUI can wire its
## options listener without engine errors.
class GameManagerStub:
	extends Node
	@warning_ignore("unused_signal")
	signal options_changed(sbs_enabled: bool, msaa_enabled: bool)


func test_main_menu_hides_and_shows_the_continue_row():
	var root := Node.new()
	add_child_autofree(root)
	var gm_stub := GameManagerStub.new()
	gm_stub.name = "GameManager"
	root.add_child(gm_stub)
	var menu := MainMenuUI.new()
	root.add_child(menu)

	menu.set_continue_available(false, false)
	var texts := _label_texts(menu)
	assert_false(_any_contains(texts, "Continue"),
		"no continuable run, no Continue row: %s" % [texts])
	assert_true(_any_contains(texts, "> New Game"),
		"New Game is the default selection without a run: %s" % [texts])

	menu.set_continue_available(true, false)
	texts = _label_texts(menu)
	assert_true(_any_contains(texts, "Continue"),
		"a continuable run shows the Continue row: %s" % [texts])
	assert_false(_any_contains(texts, "Restart Sector 1"),
		"a live snapshot resumes — the row must not threaten a restart: %s" % [texts])
	assert_true(_any_contains(texts, "> New Game"),
		"the cursor still defaults to New Game: %s" % [texts])

	menu.set_continue_available(true, true)
	texts = _label_texts(menu)
	assert_true(_any_contains(texts, "Continue — Restart Sector 1"),
		"post-run-over the row says what it will do: %s" % [texts])

func _label_texts(node: Node) -> Array:
	var out := []
	for label in node.find_children("*", "Label", true, false):
		out.append(label.text)
	return out

func _any_contains(texts: Array, needle: String) -> bool:
	for t in texts:
		if t.contains(needle):
			return true
	return false

func test_new_game_wipes_the_save_completely():
	# Owner's call (2026-07-03): New Game is the explicit clean slate — the
	# whole save goes, profile included. (Run-over is different: the roguelite
	# loop keeps organics/unlocks there.) Nothing may carry into the new game,
	# and nothing may survive on disk for the next boot.
	var s := _stack()
	var gm: GameManager = s["gm"]
	gm.start_new_game()
	_into_playing(gm)
	gm.on_cache_collected(1, 700)  # green — enough for the 300 radar
	gm.on_portal_entered()
	gm.advance_to_shop()
	assert_true(gm.buy_shop_item(8), "own the radar so there is a profile to lose")
	gm.advance_to_next_level()
	_into_playing(gm)

	# The real mid-run route to a new game: pause, then New Game (the pause
	# menu wires straight to start_new_game, which unpauses first).
	Input.action_press("open_menu")
	await wait_process_frames(2)
	Input.action_release("open_menu")
	assert_eq(gm.get_phase_name(), "Paused", "the pause route must engage")

	gm.start_new_game()
	assert_eq(gm.get_organics(), 0, "a new game starts with no organics")
	assert_false(gm.has_unlock(0), "a new game owns no unlocks")
	assert_false(gm.has_continuable_run(), "a new game has nothing to continue")

	# The wipe reaches the disk: a fresh GameManager booting on the same
	# save file must find nothing.
	var reboot := GameManager.new()
	gm.get_parent().add_child(reboot)
	assert_false(reboot.has_continuable_run(), "the save file itself is wiped")
	assert_eq(reboot.get_organics(), 0, "no profile survives on disk")
	reboot.queue_free()

func test_continue_is_the_default_row_when_available():
	# Playtest (2026-07-04): with a run to continue, Continue is what the
	# player almost always wants — the cursor starts there, not on New Game.
	# Build the whole stack DETACHED, then attach once — like scene
	# instancing, every sibling exists before any ready() runs (the real
	# MainMenuUI and GameManager wire to each other by name at ready).
	var root := Node3D.new()
	for ui_name in ["HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]:
		var stub := MenuRecordingStub.new()
		stub.name = ui_name
		root.add_child(stub)
	var ui := MainMenuUI.new()
	ui.name = "MainMenuUI"
	root.add_child(ui)
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	var gm := GameManager.new()
	gm.name = "GameManager"
	root.add_child(lm)
	root.add_child(gm)
	add_child_autofree(root)
	gm.clear_save_for_tests()
	await wait_process_frames(2)  # let GM's initial-phase push land

	ui.visible = true
	ui.set_continue_available(true, false)
	watch_signals(ui)
	await wait_process_frames(1)
	Input.action_press("menu_select")
	await wait_process_frames(2)
	Input.action_release("menu_select")
	await wait_process_frames(1)
	assert_signal_emitted(ui, "continue_selected",
		"select on the default row must continue the run")
	assert_signal_not_emitted(ui, "new_game_selected",
		"and must not start a new game")

	# Without a run, New Game leads and is the default.
	ui.set_continue_available(false, false)
	await wait_process_frames(1)
	Input.action_press("menu_select")
	await wait_process_frames(2)
	Input.action_release("menu_select")
	await wait_process_frames(1)
	assert_signal_emitted(ui, "new_game_selected",
		"a fresh install defaults to New Game")
