extends GutTest
## Every enemy renders at the size its roster def declares. The model is
## fit-scaled in ready() so its longest edge spans `size` world meters —
## the roster's ONE size knob IS the enemy's size, and nothing the
## provider shipped (native units, an animation pose, a node scale) may
## survive on top of it. Owner 2026-09-06: the planet-3 boss stayed
## gigantic through enemies.toml edits of 5.0, 2.5 and 1.5; this pins the
## knob to what the player sees, for every declared def.

const ENEMY_SCENE = "res://scenes/enemies/enemy.tscn"
const EnemyProbe = preload("res://tests/helpers/enemy_probe.gd")
# Fit tolerance: the AABB of a rotated or re-centered model can run a
# few percent past the fit target; a wrong unit or a surviving pose is
# a factor, not a percent.
const TOLERANCE = 0.05


func test_every_enemy_renders_at_its_declared_size():
	var checked := 0
	for key in EnemyDrone.enemy_keys():
		var enemy = load(ENEMY_SCENE).instantiate()
		enemy.enemy_key = key
		add_child_autofree(enemy)
		await wait_physics_frames(2)
		var aabb: AABB = EnemyProbe.visual_aabb(enemy)
		var longest: float = max(aabb.size.x, max(aabb.size.y, aabb.size.z))
		var declared: float = enemy.def_size()
		gut.p("fit %s: longest edge %.2f m, declared %.2f m" % [key, longest, declared])
		assert_gt(declared, 0.0, "%s: the def declares a size" % key)
		assert_almost_eq(longest, declared, declared * TOLERANCE,
			"%s: the visual longest edge must be the roster's size (%.2f m), got %.2f m"
				% [key, declared, longest])
		checked += 1
		enemy.queue_free()
		await wait_physics_frames(1)
	assert_gt(checked, 0, "audit must check enemy types")
