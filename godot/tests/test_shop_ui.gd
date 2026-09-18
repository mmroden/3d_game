extends GutTest
## The shop screen must SAY what each row does (playtest 2026-07-03: "no
## sense of what buying an upgrade gets you"). void-logic derives a detail
## line per offer; the UI's only job is to render it with the row.


func _labels(ui: CanvasLayer) -> String:
	var out := ""
	for l in ui.find_children("*", "Label", true, false):
		out += l.text + "\n"
	return out


func test_rows_render_their_detail_lines():
	var ui := ShopUI.new()
	add_child_autofree(ui)
	ui.show_shop(500, 100,
		PackedInt32Array([0, 7]),
		PackedStringArray(["Damage +10%", "Enemy Radar"]),
		PackedStringArray(["now ×1.00 → ×1.10", "HUD arrows point at off-screen enemies"]),
		PackedInt64Array([5000, 300]),
		PackedByteArray([2, 6]))  # PURCHASABLE | (GREEN for the radar)
	await wait_process_frames(1)

	var text := _labels(ui)
	assert_string_contains(text, "now ×1.00 → ×1.10")
	assert_string_contains(text, "HUD arrows point at off-screen enemies")


func _press(action: String) -> void:
	Input.action_press(action)
	await wait_process_frames(2)
	Input.action_release(action)
	await wait_process_frames(1)


func _catalog_args() -> Array:
	return [50_000, 1_000,
		PackedInt32Array([0, 1, 7]),
		PackedStringArray(["Thrust +10%", "Rotation +10%", "Enemy Radar"]),
		PackedStringArray(["now ×1.00 → ×1.10", "now ×1.00 → ×1.10", "HUD arrows"]),
		PackedInt64Array([2000, 1500, 300]),
		PackedByteArray([3, 3, 7])]


func _show(ui: ShopUI) -> void:
	ui.callv("show_shop", _catalog_args())


func test_a_fresh_visit_starts_at_the_top_a_refresh_keeps_the_row():
	# Playtest regression (2026-07-04): the cursor was kept across shop
	# VISITS, so the level-2 shop opened with the cursor parked on Continue
	# — and the press that closed the kill summary fired it instantly.
	# Enter-vs-refresh is EXPLICIT in the API (show_shop vs refresh_shop):
	# ShopUI cannot infer it from visibility, because the phase machine
	# shows the layer before the mediator populates it.
	var ui := ShopUI.new()
	add_child_autofree(ui)
	_show(ui)
	watch_signals(ui)
	await wait_process_frames(1)

	await _press("menu_down")                  # row 0 → row 1
	ui.callv("refresh_shop", _catalog_args())  # a buy re-prices the catalog
	await _press("menu_select")
	assert_signal_emitted_with_parameters(ui, "buy_pressed", [1])

	_show(ui)                                  # a NEW visit — even while visible
	await wait_process_frames(1)
	await _press("menu_select")
	assert_signal_emitted_with_parameters(ui, "buy_pressed", [0])


func test_the_press_that_opened_the_shop_presses_nothing_inside_it():
	# The select that dismissed the kill summary is still just_pressed when
	# the shop becomes visible in the same frame; the shop must swallow it.
	var ui := ShopUI.new()
	add_child_autofree(ui)
	watch_signals(ui)
	Input.action_press("menu_select")
	_show(ui)                   # shown mid-press, same frame
	await wait_process_frames(2)
	Input.action_release("menu_select")
	await wait_process_frames(1)
	assert_signal_not_emitted(ui, "buy_pressed",
		"the opening press must not buy anything")
	assert_signal_not_emitted(ui, "continue_pressed",
		"the opening press must not close the shop it opened")


func test_menu_type_is_readable_from_the_couch():
	# Playtest (2026-07-04): "the menu text is _tiny_". Menu typography comes
	# from the ui_style scale — rows and titles must clear readable floors.
	var ui := ShopUI.new()
	add_child_autofree(ui)
	_show(ui)
	await wait_process_frames(1)
	var row_size := 0
	var title_size := 0
	for l in ui.find_children("*", "Label", true, false):
		var size: int = l.get_theme_font_size("font_size")
		if l.text.contains("Thrust"):
			row_size = size
		if l.text.contains("UPGRADE STATION"):
			title_size = size
	assert_gte(row_size, 36, "shop rows must be readable (>= 36px), got %d" % row_size)
	assert_gte(title_size, 64, "the title must anchor the screen (>= 64px), got %d" % title_size)


func _long_catalog_args() -> Array:
	# Fourteen offers, blue and green interleaved by id — more than any panel
	# shows at once, and in an order the sections must not depend on.
	var ids := PackedInt32Array()
	var labels := PackedStringArray()
	var details := PackedStringArray()
	var costs := PackedInt64Array()
	var flags := PackedByteArray()
	for i in range(14):
		ids.append(i)
		labels.append("Item %d" % i)
		details.append("does thing %d" % i)
		costs.append(100 * (i + 1))
		flags.append(7 if i % 2 == 1 else 3)  # odd ids are organics (GREEN)
	return [50_000, 1_000, ids, labels, details, costs, flags]


func test_a_long_catalog_scrolls_by_section_and_keeps_the_exits_on_screen():
	# SBS playtest (2026-09-18): ten offers pushed Continue and Save & Exit
	# below the panel's bottom edge. The screen is two sections — components
	# stock, then organics — each a window that slides with the cursor, so
	# the cursor row, both exits, and the section markers are always shown.
	var ui := ShopUI.new()
	add_child_autofree(ui)
	ui.callv("show_shop", _long_catalog_args())
	watch_signals(ui)
	await wait_process_frames(1)
	var windows: PackedInt32Array = ui.window_rows()
	assert_eq(windows.size(), 2, "one window per section: %s" % [windows])
	assert_gte(windows[0], 1, "a section always shows at least its focused row")
	assert_gte(windows[1], 1, "a section always shows at least its focused row")

	# Display order is blue ids (0,2,..,12) then green (1,3,..,13): nine
	# presses land on display row 9, the third organics row — id 5.
	for _i in range(9):
		await _press("menu_down")
	var shown: PackedInt32Array = ui.visible_offer_rows()
	assert_true(shown.has(9), "the cursor row stays on screen: %s" % [shown])
	assert_lte(shown.size(), windows[0] + windows[1], "each section shows at most its window")
	var headings := _labels(ui)
	assert_string_contains(headings, "COMPONENTS", "the components stock is headed")
	assert_string_contains(headings, "ORGANICS", "the organics stock is headed")
	for l in ui.find_children("*", "Label", true, false):
		if l.text.contains("Continue") or l.text.contains("Save & Exit"):
			assert_true(l.is_visible_in_tree(), "%s stays on screen below the sections" % l.text.strip_edges())
	await _press("menu_select")  # green: the info screen first
	await _press("menu_select")  # then the buy
	assert_signal_emitted_with_parameters(ui, "buy_pressed", [5])


func test_the_shop_offers_save_and_exit_after_continue():
	# The catalog ends Continue, then Save & Exit (owner's ask 2026-07-04).
	var ui := ShopUI.new()
	add_child_autofree(ui)
	_show(ui)
	watch_signals(ui)
	await wait_process_frames(1)
	for _i in range(4):  # 3 offers → Continue → Save & Exit
		await _press("menu_down")
	await _press("menu_select")
	assert_signal_emitted(ui, "save_exit_pressed", "the last row banks the run")
	assert_signal_not_emitted(ui, "continue_pressed")
	assert_signal_not_emitted(ui, "buy_pressed")


func test_green_rows_confirm_before_buying():
	# Permanent purchases raise an info screen first — what it does and how
	# to trigger it (owner's ask 2026-07-04: nobody buys a mystery).
	var ui := ShopUI.new()
	add_child_autofree(ui)
	_show(ui)
	watch_signals(ui)
	await wait_process_frames(1)
	await _press("menu_down")
	await _press("menu_down")    # row 2 — Enemy Radar, GREEN
	await _press("menu_select")
	assert_signal_not_emitted(ui, "buy_pressed",
		"the first select raises the info screen, it must not buy")
	assert_string_contains(_labels(ui), "HUD arrows")
	await _press("menu_select")  # the confirming select buys
	assert_signal_emitted_with_parameters(ui, "buy_pressed", [7])


func test_circle_cancels_a_green_confirm():
	var ui := ShopUI.new()
	add_child_autofree(ui)
	_show(ui)
	watch_signals(ui)
	await wait_process_frames(1)
	await _press("menu_down")
	await _press("menu_down")
	await _press("menu_select")  # info screen up
	await _press("menu_back")    # circle backs out
	await _press("menu_select")  # select again: the info screen again, NOT a buy
	assert_signal_not_emitted(ui, "buy_pressed",
		"cancelling resets the confirm — no accidental buy")
