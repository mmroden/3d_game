extends GutTest
## Level generation is driven solely by GameManager calling
## generate_level(); LevelManager must not self-generate, and a layout
## must be a pure function of its seed.


## Stub UI layer declaring the union of signals GameManager wires up,
## so a minimal scene can exercise the real phase machinery.
class UIStub:
	extends CanvasLayer
	@warning_ignore("unused_signal")
	signal new_game_selected
	@warning_ignore("unused_signal")
	signal continue_selected
	@warning_ignore("unused_signal")
	signal sbs_toggled(enabled: bool)
	@warning_ignore("unused_signal")
	signal msaa_toggled(enabled: bool)
	@warning_ignore("unused_signal")
	signal resume_selected
	@warning_ignore("unused_signal")
	signal quit_selected
	@warning_ignore("unused_signal")
	signal continue_pressed
	@warning_ignore("unused_signal")
	signal buy_pressed
	@warning_ignore("unused_signal")
	signal return_pressed

	## Death screen contract: GameManager calls this on player death.
	func show_death(_from_laser: String, _to_laser: String, _level: int) -> void:
		pass

	@warning_ignore("unused_signal")
	signal ship_color_selected(id: int)

	## Ship-select contract: GameManager calls this when entering ShipSelect.
	func show_ship_select(_current_id: int) -> void:
		pass


func test_does_not_generate_on_ready():
	var lm = LevelManager.new()
	add_child_autofree(lm)
	await wait_process_frames(2)
	assert_eq(lm.get_child_count(), 0,
		"LevelManager must not generate on ready; GameManager drives generation")


func test_same_seed_produces_same_layout():
	var a = LevelManager.new()
	var b = LevelManager.new()
	add_child_autofree(a)
	add_child_autofree(b)
	a.generate_level(4242, 8)
	b.generate_level(4242, 8)
	assert_gt(_layout_fingerprint(a).size(), 0,
		"generation must actually place nodes (guard against vacuous equality)")
	assert_eq(_layout_fingerprint(a), _layout_fingerprint(b),
		"identical seeds must produce identical layouts")


func test_different_seeds_produce_different_layouts():
	var a = LevelManager.new()
	var b = LevelManager.new()
	add_child_autofree(a)
	add_child_autofree(b)
	a.generate_level(4242, 8)
	b.generate_level(9999, 8)
	assert_ne(_layout_fingerprint(a), _layout_fingerprint(b),
		"different seeds must produce different layouts")


func test_level_generation_is_owned_by_the_phase_machine():
	# The FSM is the sole trigger for generation: entering Playing
	# generates; a rejected transition (Death -> Playing is invalid)
	# must generate nothing.
	var root = Node3D.new()
	add_child_autofree(root)
	# Stub the UI layers show_phase toggles, so the minimal scene
	# exercises the real phase machinery without UI warnings.
	for ui_name in ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI", "ShipSelectUI", "DeathScreenUI"]:
		var stub = UIStub.new()
		stub.name = ui_name
		root.add_child(stub)
	var lm = LevelManager.new()
	lm.name = "LevelManager"
	var gm = GameManager.new()
	root.add_child(lm)
	root.add_child(gm)

	# New game now opens the loadout screen first; entering Playing from it
	# is what generates the level.
	gm.start_new_game()           # MainMenu -> ShipSelect (no generation yet)
	gm.advance_from_ship_select()  # ShipSelect -> Playing (generates)
	assert_gt(lm.get_child_count(), 0,
		"entering Playing through the FSM must generate a level")

	gm.on_player_damaged(1000000.0)  # lethal: Playing -> Death
	for child in lm.get_children():
		child.free()
	gm.continue_game()
	assert_eq(lm.get_child_count(), 0,
		"continue_game from Death must not regenerate: Death -> Playing is invalid")


func test_dead_enemies_are_cleaned_up_without_errors():
	# A freed enemy node (death) must be tombstoned by the host on the
	# next tick — never cloned first (observed red: the pre-fix host
	# panicked with a use-after-free here, which GUT surfaces as
	# engine errors failing the test).
	var lm = LevelManager.new()
	add_child_autofree(lm)
	lm.generate_level(4242, 8)
	var survivors := []
	for child in lm.get_children():
		if child is EnemyDrone:
			survivors.append(child)
	assert_gt(survivors.size(), 0,
		"seed 4242 must spawn enemies for this test to mean anything")
	var victim: Node = survivors.pop_front()
	victim.free()
	await wait_physics_frames(5)
	var remaining := 0
	for child in lm.get_children():
		if child is EnemyDrone:
			remaining += 1
	assert_eq(remaining, survivors.size(),
		"the freed enemy must be gone and every survivor must still be hosted")


func _layout_fingerprint(lm: Node3D) -> Array:
	var entries := []
	for child in lm.get_children():
		if child is Node3D:
			entries.append("%s@%s" % [child.get_class(), child.position])
	return entries
