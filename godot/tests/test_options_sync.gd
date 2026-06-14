extends GutTest
## The menus must reflect the authoritative GameOptions — one source of
## truth. The bug this guards: a UI holding its own default (e.g. MSAA
## "ON") that disagrees with GameOptions (default OFF) because nothing
## seeds it at startup, so the menu shows ON while the config is OFF.

var _main: Node3D

func before_each():
	_main = load("res://scenes/main.tscn").instantiate()
	add_child(_main)
	# ready() + the deferred options broadcast.
	await get_tree().process_frame
	await get_tree().process_frame

func after_each():
	if _main and is_instance_valid(_main):
		_main.queue_free()
		await get_tree().process_frame
	# Persisted prefs are global state — clear so tests stay isolated.
	var dir = DirAccess.open("user://")
	if dir and dir.file_exists("options.cfg"):
		dir.remove("options.cfg")

func test_menu_msaa_matches_authoritative_option_at_startup():
	var gm = _main.get_node("GameManager")
	var menu = _main.get_node("MainMenuUI")
	assert_eq(menu.displayed_msaa(), gm.msaa_enabled(),
		"the menu's MSAA must equal GameManager's authoritative option")
	assert_false(gm.msaa_enabled(),
		"MSAA defaults off")
	assert_false(menu.displayed_msaa(),
		"the menu must show MSAA off at startup, matching the default")

func test_toggling_msaa_applies_to_the_active_viewport():
	# Mono by default: the root viewport renders the 3D world. The MSAA
	# option must drive the actual viewport AA, applied by ViewManager
	# (the view) — the controller never pokes the viewport.
	var root := _main.get_viewport()
	assert_eq(root.msaa_3d, Viewport.MSAA_DISABLED,
		"MSAA off by default → active viewport not anti-aliased")
	_main.get_node("GameManager").on_msaa_toggled()
	await get_tree().process_frame
	await get_tree().process_frame
	assert_eq(root.msaa_3d, Viewport.MSAA_4X,
		"toggling MSAA on must anti-alias the active viewport")

func test_options_persist_across_a_reload():
	# Preferences remember themselves: a toggle is written to disk, and a
	# fresh launch loads it back into the one GameOptions.
	var gm = _main.get_node("GameManager")
	assert_false(gm.msaa_enabled(), "starts at the default (off)")
	gm.on_msaa_toggled() # → on, persisted
	assert_true(gm.msaa_enabled())
	gm.reload_options_from_disk() # as a fresh launch would
	assert_true(gm.msaa_enabled(),
		"the MSAA preference must persist across launches")
	# after_each clears the persisted file for isolation.
