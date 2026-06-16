extends GutTest
## LiveRef (nodes::live_handle) stores only a node's instance id and resolves it
## on access, so a cached handle to a freed node is structurally a no-op rather
## than a use-after-free. Every cached node handle in the game routes through it
## (enforced by the no_raw_gd_node_fields lint), so this one test underwrites the
## whole chokepoint. Exercised via the LiveHandleProbe seam, since GUT can't
## construct a Rust LiveRef directly.

func test_liveref_tracks_a_live_node():
	var probe = LiveHandleProbe.new()
	add_child_autofree(probe)
	var n = Node.new()
	add_child_autofree(n)
	probe.track(n)
	assert_true(probe.tracked_alive(), "tracks a live node")
	assert_true(probe.touch(), "touch runs while the node is alive")

func test_liveref_no_ops_after_the_node_is_freed():
	var probe = LiveHandleProbe.new()
	add_child_autofree(probe)
	var n = Node.new()
	add_child(n)
	probe.track(n)
	assert_true(probe.touch(), "sanity: alive before free")
	n.free()
	assert_false(probe.tracked_alive(), "a freed node reports not alive")
	# The whole point: touching a freed handle returns false instead of the
	# OmniLight3D::clone-style use-after-free panic GUT would surface as an error.
	assert_false(probe.touch(), "touch after free no-ops, never panics")
