extends GutTest
## The credits capture door (owner 2026-09-09: "can we do the same thing
## with the menu as a way to validate that credits are displayed?"):
## `--screen=credits --shot=0;150` boots straight into the roll, and the
## shot sequencer visits each OFFSET — the roll's own pose — the way it
## visits camera poses in a level: settle, verify, draw, save; ending
## parked at the last one, never wrapping. The exports are the same knobs
## as the command line, set before the scene enters the tree. The frames
## themselves are the visual suite's contract (tests/visual.rs); this
## pins the sequence.

var _main: Node3D


func before_each():
	_main = load("res://scenes/main.tscn").instantiate()
	var gm = _main.get_node("GameManager")
	gm.screen = "credits"
	gm.shot_pose = "0;150"
	add_child(_main)
	# ready() + the deferred options broadcast + the opening phase.
	await wait_process_frames(3)


func after_each():
	if _main and is_instance_valid(_main):
		_main.queue_free()
		await get_tree().process_frame


func test_a_credits_screen_boot_opens_the_roll_and_parks_it_at_each_shot():
	var gm = _main.get_node("GameManager")
	var menu = _main.get_node("MainMenuUI")
	assert_true(menu.credits_visible(), "the boot opens the roll")
	assert_eq(gm.get_phase_name(), "MainMenu", "on the menu — no mission")
	await wait_process_frames(gm.shot_settle_frames() * 3)
	assert_almost_eq(menu.credits_offset(), 150.0, 1.0,
		"the sequence traverses to the last offset")
	await wait_process_frames(gm.shot_settle_frames() * 2)
	assert_almost_eq(menu.credits_offset(), 150.0, 1.0, "…and stays parked there")
