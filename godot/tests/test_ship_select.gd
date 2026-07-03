extends GutTest
## The two-stage loadout screen. Stage one lists ONLY owned hulls — a locked
## hull is invisible here, the shop is where it is discovered (playtest
## 2026-07-03: "ships that can't be bought shouldn't be selectable").
## Choosing a hull opens the trim stage: every hull offers the
## Standard/Armored/Swift tradeoffs (paint stays a styled-hull affair, the
## stats are universal). Roster data is pinned in Rust; this drives the real
## UI node with real menu input.

const VANGUARD_ID := 0
const TALON_ID := 1
const ARMORED_TRIM_ID := 1


func _labels(ui: CanvasLayer) -> String:
	var out := ""
	for l in ui.find_children("*", "Label", true, false):
		out += l.text + "\n"
	return out


func _press(action: String) -> void:
	Input.action_press(action)
	await wait_process_frames(2)
	Input.action_release(action)
	await wait_process_frames(1)


func _fresh(owned: PackedByteArray) -> CanvasLayer:
	var ui := ShipSelectUI.new()
	add_child_autofree(ui)
	ui.show_ship_select(VANGUARD_ID, 0, owned)
	await wait_process_frames(1)
	return ui


func test_locked_hulls_do_not_appear_at_all():
	var ui = await _fresh(PackedByteArray([1, 0, 0, 0]))
	var text := _labels(ui)
	assert_string_contains(text, "Vanguard")
	assert_false(text.contains("Talon"), "an unowned hull is invisible, not a signpost")
	assert_false(text.contains("LOCKED"), "no locked rows on the loadout screen")


func test_choosing_a_hull_opens_the_trim_stage_for_every_hull():
	var ui = await _fresh(PackedByteArray([1, 1, 0, 0]))
	watch_signals(ui)
	var stage_one := _labels(ui)
	assert_string_contains(stage_one, "Talon")
	assert_false(stage_one.contains("Armored"), "trims wait for the hull choice")

	await _press("menu_down")    # Vanguard → Talon
	await _press("menu_select")
	assert_signal_emitted_with_parameters(ui, "ship_type_selected", [TALON_ID])

	# The Talon has no painted styles — the trims must be offered anyway.
	var stage_two := _labels(ui)
	assert_string_contains(stage_two, "Standard")
	assert_string_contains(stage_two, "Armored")
	assert_string_contains(stage_two, "Swift")
	assert_string_contains(stage_two, "Continue")
	assert_false(stage_two.contains("Vanguard"), "the hull list yields to the trim list")


func test_trim_choice_emits_and_back_returns_to_the_hull_stage():
	var ui = await _fresh(PackedByteArray([1, 0, 0, 0]))
	watch_signals(ui)
	await _press("menu_select")  # the only hull → trim stage
	await _press("menu_down")    # Standard → Armored
	await _press("menu_select")
	assert_signal_emitted_with_parameters(ui, "ship_color_selected", [ARMORED_TRIM_ID])

	# Walk to the Back row (trims, then Back) and return to the hull stage.
	await _press("menu_down")    # Armored → Swift
	await _press("menu_down")    # Swift → Back
	await _press("menu_select")
	assert_string_contains(_labels(ui), "Vanguard")  # Back re-opens the hull stage


func test_continue_fires_from_the_trim_stage():
	var ui = await _fresh(PackedByteArray([1, 0, 0, 0]))
	watch_signals(ui)
	await _press("menu_select")  # hull → trims
	for _i in range(4):          # Standard → Armored → Swift → Back → Continue
		await _press("menu_down")
	await _press("menu_select")
	assert_signal_emitted(ui, "continue_pressed", "Continue closes the loadout")


func test_circle_backs_the_trim_stage_out_to_the_hulls():
	# Circle (menu_back) is ALWAYS the back button — in the trim stage it is
	# the Back row without the walk.
	var ui = await _fresh(PackedByteArray([1, 0, 0, 0]))
	await _press("menu_select")  # hull → trims
	assert_string_contains(_labels(ui), "Standard")
	await _press("menu_back")
	assert_string_contains(_labels(ui), "Vanguard")  # back on the hull stage
