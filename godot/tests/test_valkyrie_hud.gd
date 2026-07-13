extends GutTest
## The Valkyrie charge row (playtest 2026-07-06): the HUD shows one square
## per bar, filling left-to-right as the single charge crosses the row —
## hidden entirely until the cannon is owned. The ship's ChargeState is
## the truth; the row is its per-frame readout.


func test_the_charge_row_fills_square_by_square():
	var root := Node3D.new()
	add_child_autofree(root)
	var player := ShipController.new()
	player.name = "Player"
	var shape := CollisionShape3D.new()
	shape.shape = SphereShape3D.new()
	player.add_child(shape)
	var hud := HUD.new()
	hud.name = "HUD"
	root.add_child(player)
	root.add_child(hud)
	player.set_controls_enabled(true)
	hud.visible = true
	await wait_process_frames(2)

	var row := hud.find_child("ValkyrieChargeRow", true, false)
	assert_not_null(row, "the HUD builds the charge row")
	if row == null:
		return
	assert_false(row.visible, "no cannon, no row")

	player.set_valkyrie_owned(true)
	await wait_process_frames(3)
	assert_true(row.visible, "owning the cannon shows the row")
	var slots := []
	for c in row.get_children():
		if String(c.name).begins_with("ValkyrieSlot") and c.visible:
			slots.append(c)
	assert_eq(slots.size(), 3, "a fresh cannon shows its three base squares")
	if slots.is_empty():
		return

	var fill0 := row.find_child("ValkyrieFill0", false, false)
	var fill2 := row.find_child("ValkyrieFill2", false, false)
	assert_not_null(fill0, "each square carries a fill")
	assert_not_null(fill2, "each square carries a fill")
	if fill0 == null or fill2 == null:
		return
	assert_lt(fill0.size.x, 1.0, "a fresh rack starts empty")

	await wait_physics_frames(200, "1.67s at the 120Hz physics tick — fills the first square")
	await wait_process_frames(2)
	var slot_w: float = slots[0].size.x
	assert_gt(fill0.size.x, slot_w * 0.9, "the first square is full")
	assert_lt(fill2.size.x, slot_w * 0.5, "the third square has barely begun")
