extends GutTest
## Floor/platform tiles are placed on a 4 m grid; a tile whose mesh
## footprint exceeds the grid pitch encroaches into neighboring cells —
## most visibly into vertical-passage holes, shrinking their apparent
## aperture below the (honest, 4 m) collision aperture.

const PLATFORMS_DIR = "res://addons/quaternius/modularscifimegakit/platforms"
const GRID_PITCH = 4.0
## Decorative trim may slightly overhang; beyond this it materially
## misrepresents the aperture.
const TOLERANCE = 0.3

## The floor tiles the wall sets actually place on the grid — keep in
## sync with rust/void-logic/src/asset_catalog/wall_sets.rs.
const GRID_FLOOR_TILES = [
	"Platform_Simple.gltf",
	"Platform_Simple_Curve.gltf",
	"Platform_Metal.gltf",
	"Platform_Metal_Curve.gltf",
	"Platform_DarkPlates.gltf",
	"Platform_DarkPlates_Curves.gltf",
	"Platform_CenterPlate.gltf",
	"Platform_CenterPlate_Curve.gltf",
	"Platform_Squares.gltf",
	"Platform_Squares_Curve.gltf",
	"Platform_Padded.gltf",
]


func test_floor_tiles_fit_the_grid_pitch():
	var checked := 0
	for file in GRID_FLOOR_TILES:
		var scene = load(PLATFORMS_DIR + "/" + file)
		assert_not_null(scene, "%s must exist (catalog references it)" % file)
		if scene == null:
			continue
		var tile: Node3D = scene.instantiate()
		add_child_autofree(tile)
		var aabb := _merged_aabb(tile)
		var x := aabb.size.x
		var z := aabb.size.z
		checked += 1
		# Coverage is what matters, not size: a 4x4 tile pivoted off
		# center covers the wrong 4x4. The assembler places tiles at
		# cell centers, so coverage must be [-2, 2] on both axes.
		gut.p("%s: x [%.2f, %.2f], z [%.2f, %.2f] (size %.2f x %.2f)" % [
			file, aabb.position.x, aabb.position.x + x,
			aabb.position.z, aabb.position.z + z, x, z])
		assert_lt(maxf(x, z), GRID_PITCH + TOLERANCE,
			"%s: footprint %.2f x %.2f overhangs the %.1f m grid" % [file, x, z, GRID_PITCH])
		var half := GRID_PITCH / 2.0
		assert_almost_eq(aabb.position.x + x / 2.0, 0.0, 0.35,
			"%s: pivot is off-center in x — placed at a cell center it covers the wrong cell area" % file)
		assert_almost_eq(aabb.position.z + z / 2.0, 0.0, 0.35,
			"%s: pivot is off-center in z — placed at a cell center it covers the wrong cell area" % file)
		assert_true(half > 0.0)
	assert_gt(checked, 0, "audit must find platform tiles")


const WALLS_DIR = "res://addons/quaternius/modularscifimegakit/walls"
## Ceiling (Top*) and floor-rim (Bottom*) trim pieces ring every room at
## exactly the planes where vertical-passage holes open. Their reach
## inward from the wall plane determines how much of a hole they cover.
const RIM_TRIM_PIECES = [
	"TopAstra_Straight.gltf",
	"TopSimple_Straight.gltf",
	"TopPlates_Straight.gltf",
	"TopWindow_Straight.gltf",
	"TopPadded_Flat_Straight.gltf",
	"TopSimple_Corner_Round_Inner.gltf",
	"TopAstra_Curve_Round_Outer.gltf",
	"TopPlates_Corner_Round_Inner.gltf",
	"BottomAccent_Straight.gltf",
	"BottomSimple_Straight.gltf",
	"BottomMetal_Straight.gltf",
	"BottomAccent_Corner_Round_Inner.gltf",
	"BottomSimple_Corner_Round_Inner.gltf",
]


func test_measure_rim_trim_reach():
	# Diagnostic: prints each trim piece's coverage relative to its
	# pivot. Asserts only existence; the numbers inform the vertical-
	# passage aperture investigation.
	var checked := 0
	for file in RIM_TRIM_PIECES:
		var scene = load(WALLS_DIR + "/" + file)
		assert_not_null(scene, "%s must exist (catalog references it)" % file)
		if scene == null:
			continue
		var piece: Node3D = scene.instantiate()
		add_child_autofree(piece)
		var aabb := _merged_aabb(piece)
		checked += 1
		gut.p("%s: x [%.2f, %.2f], y [%.2f, %.2f], z [%.2f, %.2f]" % [
			file,
			aabb.position.x, aabb.position.x + aabb.size.x,
			aabb.position.y, aabb.position.y + aabb.size.y,
			aabb.position.z, aabb.position.z + aabb.size.z])
	assert_gt(checked, 0)


func _merged_aabb(root: Node3D) -> AABB:
	var merged := AABB()
	var first := true
	var stack: Array = [root]
	while not stack.is_empty():
		var node = stack.pop_back()
		for child in node.get_children():
			stack.push_back(child)
		if node is MeshInstance3D and node.mesh != null:
			var to_root: Transform3D = root.global_transform.affine_inverse() * node.global_transform
			var aabb: AABB = to_root * node.get_aabb()
			merged = aabb if first else merged.merge(aabb)
			first = false
	return merged
