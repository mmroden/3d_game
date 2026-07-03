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
