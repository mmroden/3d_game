## The one full-stack builder for the shell tests — consumed via preload:
## `const FullStack := preload("res://tests/helpers/full_stack.gd")` (the
## test project registers no global class names).
##
## A stack is the stub UI layers, a LevelManager, a player and a
## GameManager with its dev knobs set, the save wiped. Nothing here builds
## a level: `walk_to_playing` does, through the real phase machine, and
## GameManager's start_level knob does on a boot. Eleven files carried a
## copy of this by hand (2026-09-16); each copy also built the loadout
## backdrop room a stack with no turntable never looks at — half of a
## GUT run's level builds — so the backdrop is skipped here by default.


const UiStub := preload("res://tests/helpers/ui_stub.gd")

const UI_NAMES := ["MainMenuUI", "HUD", "PauseMenuUI", "KillSummaryUI", "ShopUI",
	"ShipSelectUI", "BestiaryUI", "DeathScreenUI", "LoadingUI"]


## Build the stack under `host` (a GutTest — the root is autofreed after
## the test). Options, all optional:
##   seed: int        GameManager.fixed_seed (omit for a random run seed)
##   level: int       GameManager.start_level (a LEVEL=N boot)
##   player: bool     add a ShipController (default true)
##   shot: String     GameManager.shot_pose
##   sbs: int         GameManager.sbs_override (default -1)
##   populace: int    GameManager.populace_override (default -1)
##   backdrop: int    GameManager.backdrop_override (default 0: skipped)
##   stub: Script     the stub class for every layer (default UiStub)
##   stubs: Dictionary  UI name -> stub script, for one recording stub
##   real: Dictionary   UI name -> a REAL UI node to seat in that layer's
##                    place — before GameManager, whose ready() wires the
##                    UI signals it can find (a real screen added after it
##                    is never connected)
## Returns {root, gm, lm, player, stubs} — `stubs` is UI name -> node,
## the real ones included.
static func build(host: GutTest, opts: Dictionary = {}) -> Dictionary:
	var root := Node3D.new()
	host.add_child_autofree(root)
	var stubs := {}
	var default_stub = opts.get("stub", UiStub)
	var custom: Dictionary = opts.get("stubs", {})
	var real: Dictionary = opts.get("real", {})
	for ui_name in UI_NAMES:
		var node: Node
		if real.has(ui_name):
			node = real[ui_name]
		else:
			var script = custom.get(ui_name, default_stub)
			node = script.new()
		node.name = ui_name
		root.add_child(node)
		stubs[ui_name] = node
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	var player: ShipController = null
	if opts.get("player", true):
		player = ShipController.new()
		player.name = "Player"
		player.add_to_group("player")
		var shape := CollisionShape3D.new()
		shape.name = "CollisionShape3D"
		shape.shape = SphereShape3D.new()
		player.add_child(shape)
	var gm := GameManager.new()
	if opts.has("seed"):
		gm.fixed_seed = opts["seed"]
	gm.start_level = opts.get("level", 0)
	gm.shot_pose = opts.get("shot", "")
	gm.sbs_override = opts.get("sbs", -1)
	gm.populace_override = opts.get("populace", -1)
	gm.backdrop_override = opts.get("backdrop", 0)
	# GameManager last: its ready() finds its siblings.
	root.add_child(lm)
	if player != null:
		root.add_child(player)
	root.add_child(gm)
	# Real persistence leaks profiles across test runs; start fresh.
	gm.clear_save_for_tests()
	return {"root": root, "gm": gm, "lm": lm, "player": player, "stubs": stubs}


## From ShipSelect (after start_new_game), tap through the briefing into
## Playing — the real phase machine builds the level on entry.
static func walk_to_playing(host: GutTest, gm: GameManager) -> void:
	gm.advance_from_ship_select()
	for _i in range(12):
		if gm.get_phase_name() == "Playing":
			break
		gm.advance_from_bestiary()
	host.assert_eq(gm.get_phase_name(), "Playing", "the stack must reach Playing")
