extends GutTest
## Cubic-cell panel worlds (B11), in-engine: a planet-2 level builds through
## the ONE pathway with panel structure — cubic pitch, panel skin, fused
## room colliders — while planet 1 keeps the megakit. Same pinned seed as
## every structural suite.

func test_a_planet_two_level_builds_from_the_panel_pool():
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	add_child_autofree(lm)
	lm.current_level = 7  # planet 2's first sector
	lm.generate_level(1, 8)

	var panels := lm.find_children("*sf_pp01*", "", true, false)
	assert_gt(panels.size(), 0, "planet-2 rooms are skinned with kit panels")

	var megakit_walls := 0
	for n in lm.find_children("*WallAstra*", "", true, false):
		megakit_walls += 1
	assert_eq(megakit_walls, 0, "no megakit wall pieces on planet 2")

	var colliders := lm.find_children("*", "StaticBody3D", true, false)
	assert_gt(colliders.size(), 0, "panel rooms still fuse their colliders")

func test_a_planet_one_level_keeps_the_megakit():
	var lm := LevelManager.new()
	lm.name = "LevelManager"
	add_child_autofree(lm)
	lm.current_level = 1
	lm.generate_level(1, 8)

	var panels := lm.find_children("*sf_pp01*", "", true, false)
	assert_eq(panels.size(), 0, "planet 1 stays terrestrial: no panels")
