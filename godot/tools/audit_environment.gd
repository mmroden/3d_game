# Stage-4 audit: what did Godot's importer make of the environment glb?
# Loads the IMPORTED scene (the exact resource the game instantiates) and
# reports material states — the in-engine end of the asset-pipeline audit
# (`make test-assets` covers stages 1-3 from the extracts and the glb).
#
#   tools/Godot.app/Contents/MacOS/Godot --headless --path godot \
#       -s res://tools/audit_environment.gd [-- <res-path-to-glb>]
extends SceneTree


func _init() -> void:
	var path := "res://addons/environments/apartment.glb"
	var args := OS.get_cmdline_user_args()
	if args.size() > 0:
		path = args[0]
	var packed: PackedScene = load(path)
	if packed == null:
		push_error("audit: cannot load %s (run make assets)" % path)
		quit(1)
		return
	var scene := packed.instantiate()

	var meshes := 0
	var surfaces := 0
	var mats := {}  # name -> StandardMaterial3D (deduped)
	var stack: Array[Node] = [scene]
	while not stack.is_empty():
		var node: Node = stack.pop_back()
		for child in node.get_children():
			stack.push_back(child)
		var mi := node as MeshInstance3D
		if mi == null or mi.mesh == null:
			continue
		meshes += 1
		for s in range(mi.mesh.get_surface_count()):
			surfaces += 1
			var mat := mi.mesh.surface_get_material(s) as StandardMaterial3D
			if mat != null:
				mats[mat.resource_name] = mat

	var textured := 0
	var flat := 0
	var emissive: Array = []
	var blended := 0
	var default_grey := 0
	for name: String in mats:
		var m: StandardMaterial3D = mats[name]
		if m.albedo_texture != null:
			textured += 1
		else:
			flat += 1
			var c := m.albedo_color
			if absf(c.r - 0.5) < 0.11 and absf(c.g - 0.5) < 0.11 \
					and absf(c.b - 0.5) < 0.11:
				default_grey += 1
		if m.emission_enabled:
			emissive.append([name, m.emission_energy_multiplier])
		if m.transparency != BaseMaterial3D.TRANSPARENCY_DISABLED:
			blended += 1

	emissive.sort_custom(func(a: Array, b: Array) -> bool: return a[1] > b[1])
	print("audit: %s" % path)
	print("audit: %d mesh instances, %d surfaces, %d unique materials"
			% [meshes, surfaces, mats.size()])
	print("audit: %d textured, %d flat (%d suspicious mid-grey), %d transparent"
			% [textured, flat, default_grey, blended])
	print("audit: %d emissive; hottest:" % emissive.size())
	for i in range(mini(8, emissive.size())):
		print("audit:   %s energy=%.2f" % [emissive[i][0], emissive[i][1]])
	scene.free()
	quit(0)
