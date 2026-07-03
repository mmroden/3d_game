extends GutTest
## The bolt ring buffer (Faucet Principle, tier 2). Every bolt a level can fire
## is preallocated once as a dormant slot; firing is reset-in-place, expiry/hit
## returns the slot to dormant. Nothing is instantiated or freed once the ring
## is built — so these tests pin: behavior preserved (damage + expiry), the
## dormancy contract (a dormant slot neither renders nor collides), ring wrap
## (overwrite the oldest live bolt, no error), the stale-hit generation guard,
## and that firing performs no structural allocation (child count is constant).

# --- Helpers ---

func _make_pool(capacity: int) -> BoltPool:
	var pool := BoltPool.new()
	pool.capacity = capacity # sized before ready() builds the ring
	add_child_autofree(pool)
	return pool

func _make_player(pos: Vector3) -> ShipController:
	var player := ShipController.new()
	player.add_to_group("player")
	var shape := CollisionShape3D.new()
	var sphere := SphereShape3D.new()
	sphere.radius = 0.5
	shape.shape = sphere
	player.add_child(shape)
	add_child_autofree(player)
	player.global_position = pos
	return player

# --- Ring is built during the load, not per shot ---

func test_pool_preallocates_its_full_ring_on_ready():
	var pool := _make_pool(8)
	assert_eq(pool.capacity, 8, "pool exposes its ring capacity")
	assert_eq(pool.get_child_count(), 8,
		"the whole ring is preallocated as child slots when the pool enters the tree")
	assert_eq(pool.live_count(), 0,
		"every slot starts dormant — nothing live before the first shot")

func test_firing_allocates_no_new_slots():
	# The point of the pool: firing is activation, never instantiation. The
	# child count must not budge across many shots.
	var pool := _make_pool(8)
	var before := pool.get_child_count()
	for i in range(5):
		pool.fire(Vector3(i, 0, 0), Vector3(0, 0, -1), 1.0)
	assert_eq(pool.get_child_count(), before,
		"firing must reuse slots, never add children (no instantiate)")
	assert_eq(pool.live_count(), 5, "five fires leave five live bolts")

# --- Dormancy contract: invisible, non-processing, non-colliding, together ---

func test_dormant_slot_neither_renders_nor_collides():
	var pool := _make_pool(4)
	# Dormancy flags land on the deferred boundary (they may be flipped from
	# physics callbacks, where direct toggles are blocked) — allow one flush.
	await wait_process_frames(1)
	var slot: Area3D = pool.get_child(0)
	assert_false(slot.visible, "a dormant slot is invisible")
	assert_false(slot.monitoring, "a dormant slot does not monitor for bodies")
	assert_false(slot.monitorable, "a dormant slot is not monitorable")
	assert_eq(slot.process_mode, Node.PROCESS_MODE_DISABLED,
		"a dormant slot does not process (it holds position)")

func test_fire_flips_all_three_dormancy_flags_on():
	var pool := _make_pool(4)
	pool.fire(Vector3.ZERO, Vector3(0, 0, -1), 1.0)
	# Engine flags follow the logical liveness within one deferred flush.
	await wait_process_frames(1)
	var slot: Area3D = pool.get_child(0)
	assert_true(slot.visible, "a fired bolt is visible")
	assert_true(slot.monitoring, "a fired bolt monitors for bodies")
	assert_true(slot.monitorable, "a fired bolt is monitorable")
	assert_eq(slot.process_mode, Node.PROCESS_MODE_INHERIT,
		"a fired bolt processes (it flies)")

# --- Behavior preserved: a pooled bolt still damages and still expires ---

func test_pooled_bolt_damages_the_player():
	var player := _make_player(Vector3.ZERO)
	watch_signals(player)
	var pool := _make_pool(4)
	# Fire from just short of the player, straight at it.
	pool.fire(Vector3(0, 0, 2), Vector3(0, 0, -5), 25.0)
	await wait_physics_frames(30, "let the pooled bolt reach and hit the player")
	assert_signal_emitted(player, "player_damaged",
		"a pooled bolt must damage the player it strikes, just like the old per-shot bolt")

func test_pooled_bolt_returns_to_dormant_when_it_expires():
	var pool := _make_pool(4)
	# Fire into empty space so it never hits anything and lives out its lifetime.
	pool.fire(Vector3.ZERO, Vector3(0, 0, -0.001), 1.0)
	assert_eq(pool.live_count(), 1, "sanity: bolt is live right after firing")
	# BOLT_LIFETIME_S is 3.0; at 120 Hz physics that's 360 frames, so 420
	# (3.5s) comfortably outlives it.
	await wait_physics_frames(420, "let the bolt live out its full lifetime")
	assert_eq(pool.live_count(), 0,
		"an expired bolt returns to dormant (not queue_free'd) — the slot is reusable")

# --- Ring wrap: firing past capacity overwrites the oldest live bolt ---

func test_firing_past_capacity_wraps_without_error():
	var pool := _make_pool(3)
	# Fire more than capacity; all in empty space so none expire mid-test.
	for i in range(3 + 2):
		pool.fire(Vector3(i, 0, 0), Vector3(0, 0, -0.001), 1.0)
	assert_eq(pool.get_child_count(), 3,
		"wrapping must not allocate — the ring stays exactly capacity slots")
	assert_eq(pool.live_count(), 3,
		"a full ring stays at capacity live: the oldest is overwritten, not appended")

# --- Stale-hit generation guard ---

func test_reused_slot_does_not_deliver_its_previous_lifes_hit():
	# The sharp edge (faucet doc): a slot re-fired in the same frame its previous
	# occupant's body_entered is still queued must not register the old hit. The
	# per-slot generation counter, checked in the deferred callback, is what
	# no-ops the stale signal. We simulate the race directly: arm a hit against a
	# stale generation and assert it is rejected.
	var player := _make_player(Vector3.ZERO)
	watch_signals(player)
	var pool := _make_pool(2)
	pool.fire(Vector3.ZERO, Vector3(0, 0, -1), 25.0)
	var slot: Area3D = pool.get_child(0)
	var stale_gen: int = slot.generation()
	# The slot re-fires (new life) before the queued hit from the old life runs.
	slot.fire(Vector3(50, 0, 0), Vector3(0, 0, -1), 25.0)
	# Now the old life's deferred hit finally executes, tagged with its (stale)
	# generation. It must be dropped.
	slot.resolve_hit(player, stale_gen)
	await wait_physics_frames(1, "let any deferred damage flush")
	assert_signal_not_emitted(player, "player_damaged",
		"a hit armed for a previous life must be dropped once the slot has re-fired")
