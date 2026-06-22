extends GutTest
## The end-of-level exit portal must present the imported jump-gate model, not a
## procedural primitive. This pins the asset swap (toroid -> JumpGate.glb): the
## gate model is instanced under the portal (a "Model" node carrying mesh
## geometry) and no TorusMesh primitive survives. The portal's collision trigger
## must still be built so the level can still be completed.

const PORTAL_SCENE = "res://scenes/items/portal.tscn"


func test_portal_presents_jump_gate_model_not_a_torus():
	var portal = load(PORTAL_SCENE).instantiate()
	add_child_autofree(portal)

	var model := _find_named(portal, "Model")
	assert_not_null(model, "portal did not instance the jump-gate Model node")
	if model != null:
		assert_gt(_mesh_instance_count(model), 0,
			"jump-gate Model carries no MeshInstance3D geometry")

	# No procedural torus may remain anywhere under the portal.
	for mesh_inst in _all_mesh_instances(portal):
		assert_false(mesh_inst.mesh is TorusMesh,
			"portal still uses a procedural TorusMesh; expected the jump-gate model")


func test_portal_keeps_its_collision_trigger():
	var portal = load(PORTAL_SCENE).instantiate()
	add_child_autofree(portal)
	var shapes := 0
	for node in _descendants(portal):
		if node is CollisionShape3D and node.shape != null:
			shapes += 1
	assert_gt(shapes, 0, "portal built no CollisionShape3D; it could never be entered")


## First descendant whose name matches, or null.
func _find_named(root: Node, target: String) -> Node:
	for node in _descendants(root):
		if node.name == target:
			return node
	return null


func _mesh_instance_count(root: Node) -> int:
	var count := 0
	for node in _descendants(root):
		if node is MeshInstance3D:
			count += 1
	return count


func _all_mesh_instances(root: Node) -> Array:
	var found: Array = []
	for node in _descendants(root):
		if node is MeshInstance3D:
			found.append(node)
	return found


func _descendants(root: Node) -> Array:
	var out: Array = []
	var stack: Array = [root]
	while not stack.is_empty():
		var node = stack.pop_back()
		for child in node.get_children():
			out.append(child)
			stack.push_back(child)
	return out
