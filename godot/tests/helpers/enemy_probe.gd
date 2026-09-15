## Measurements of a spawned enemy for the shell contracts (preload it:
## the test project registers no global class names).


## Union of the MODEL's MeshInstance3D AABBs (global space). The visual
## body is the model subtree under ModelPivot — the floating health bar
## is UI, not something a size or a collider contract owes anything to.
static func visual_aabb(enemy: Node) -> AABB:
	var model = enemy.get_node_or_null("ModelPivot")
	if model == null:
		return AABB()
	var combined := AABB()
	var first := true
	var stack: Array = [model]
	while not stack.is_empty():
		var node = stack.pop_back()
		for child in node.get_children():
			stack.push_back(child)
		if node is MeshInstance3D:
			var global: AABB = node.global_transform * node.get_aabb()
			combined = global if first else combined.merge(global)
			first = false
	return combined
