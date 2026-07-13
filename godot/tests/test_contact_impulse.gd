extends GutTest
## Physics Gate 1 (docs/design/physics.md, § Build order): measured proof that
## the godot-jolt backend faithfully fills PhysicsDirectBodyState3D.get_contact_impulse.
## The kinetic damage law ("Δv = |contact impulse| / own_mass", § The one law)
## is priced ENTIRELY off this integral — the solver's own ∫F·dt — so before a
## line of that law is written we demand runtime evidence, not documentation faith,
## that Jolt actually reports a per-contact impulse and that it is physically sane.
##
## The rig is deliberately minimal and analytic: a StaticBody3D wall (a wall is
## effectively infinite mass — it converts the body's WHOLE momentum) and a 1 kg
## RigidBody3D sphere flung at it with gravity switched off, so the trajectory is
## pure and the momentum change is exactly the launch speed's worth. For a 1 kg
## body meeting an immovable wall at v m/s, the impulse J = m·Δv lands in
##   [ m·v , 2·m·v ]  =  [v, 2v]   (restitution 0 → dead stop; 1 → perfect bounce),
## Jolt's default restitution being ~0, so we expect ~v with a generous band.
##
## A probe RigidBody reads state.get_contact_impulse(0) inside _integrate_forces
## (the ship's own contact-reporting hook — physics.md gate 1 note) and records the
## magnitude plus the pre/post-contact velocity for the test to read back.

const WAIT_HZ := 60.0            # Godot's default physics tick
const MAX_WAIT_FRAMES := 120     # ~2 s hard ceiling — fail loudly, never hang


## A 1 kg sphere that records the first contact impulse Jolt hands it.
## _integrate_forces is where PhysicsDirectBodyState3D is live and where the
## solver has already integrated the collision for this substep, so
## get_contact_impulse(0) is the ∫F·dt of the stop the law will price.
class ImpulseProbe:
	extends RigidBody3D

	var recorded := false          # latched on the first contact event
	var impulse_mag := -1.0        # |get_contact_impulse(0)| at first contact
	var impulse_is_vector3 := false  # did the API return a Vector3 without erroring?
	var contact_count := 0
	var vel_before := Vector3.ZERO  # speed carried in the frame BEFORE the stop
	var vel_after := Vector3.ZERO   # speed left AFTER the solver resolved it
	var _prev_velocity := Vector3.ZERO

	func _integrate_forces(state: PhysicsDirectBodyState3D) -> void:
		if recorded:
			return
		if state.get_contact_count() > 0:
			contact_count = state.get_contact_count()
			var imp := state.get_contact_impulse(0)
			impulse_is_vector3 = typeof(imp) == TYPE_VECTOR3
			impulse_mag = imp.length()
			vel_before = _prev_velocity           # last pre-contact frame's velocity
			vel_after = state.get_linear_velocity()  # post-solve velocity this frame
			recorded = true
		else:
			# No contact yet: remember this frame's velocity as the incoming speed.
			_prev_velocity = state.get_linear_velocity()


## Build a fresh wall + probe, fling the probe at the wall at `speed` m/s, and
## return the recorded contact data (or recorded == false on timeout).
func _launch_and_measure(speed: float) -> Dictionary:
	var wall := StaticBody3D.new()
	var wall_shape := CollisionShape3D.new()
	var box := BoxShape3D.new()
	box.size = Vector3(2.0, 10.0, 10.0)  # thick, tall, wide — an unmissable wall
	wall_shape.shape = box
	wall.add_child(wall_shape)
	add_child_autofree(wall)
	wall.global_position = Vector3.ZERO   # near face at x = -1

	var probe := ImpulseProbe.new()
	var probe_shape := CollisionShape3D.new()
	var sphere := SphereShape3D.new()
	sphere.radius = 0.5
	probe_shape.shape = sphere
	probe.add_child(probe_shape)
	probe.mass = 1.0
	probe.gravity_scale = 0.0        # pure horizontal trajectory — momentum is all launch
	probe.contact_monitor = true     # required for get_contact_* to populate
	probe.max_contacts_reported = 4
	probe.can_sleep = false
	add_child_autofree(probe)
	probe.global_position = Vector3(-3.0, 0.0, 0.0)  # 1.5 m of travel to the wall face

	# Let the bodies enter the physics space before we set the launch velocity,
	# then fling straight at the wall.
	await wait_physics_frames(1, "let the wall and probe register in the space")
	probe.linear_velocity = Vector3(speed, 0.0, 0.0)

	# Bounded wait: spin physics frames until the probe latches a contact, then
	# stop. If it never contacts within the ceiling, return recorded == false and
	# let the caller fail loudly — the test must not hang.
	var frames := 0
	while not probe.recorded and frames < MAX_WAIT_FRAMES:
		await wait_physics_frames(1)
		frames += 1

	return {
		"recorded": probe.recorded,
		"impulse_mag": probe.impulse_mag,
		"impulse_is_vector3": probe.impulse_is_vector3,
		"contact_count": probe.contact_count,
		"vel_before": probe.vel_before,
		"vel_after": probe.vel_after,
		"frames": frames,
	}


func test_jolt_reports_a_physically_plausible_per_contact_impulse():
	# --- Launch 1: 10 m/s head-on into the wall (the anchor measurement) ---
	var fast := await _launch_and_measure(10.0)
	gut.p("GATE 1 — launch @ 10 m/s: %s" % fast)

	assert_true(fast["recorded"],
		"the 10 m/s probe must register a contact within %d frames (~%.1f s) — no contact means no evidence" % [MAX_WAIT_FRAMES, MAX_WAIT_FRAMES / WAIT_HZ])
	if not fast["recorded"]:
		return  # nothing measured — the assertions below would be meaningless

	# (a) the API exists and returns a Vector3 without erroring under Jolt
	assert_true(fast["impulse_is_vector3"],
		"get_contact_impulse(0) must return a Vector3 under the Jolt backend")
	assert_gt(fast["contact_count"], 0,
		"get_contact_count() must be positive at the recorded contact")

	# (b) the impulse magnitude is strictly positive — a zero vector would mean
	# Jolt does NOT fill the API and the whole kinetic law loses its footing
	assert_gt(fast["impulse_mag"], 0.0,
		"the measured contact impulse magnitude must be > 0 — a zero here fails the gate outright")

	# (c) physically plausible: J = m·Δv for a 1 kg body shedding 10 m/s into a
	# wall is 10 N·s at restitution 0, 20 N·s at restitution 1. The [5, 25] band
	# generously brackets the whole restitution range plus solver slop.
	assert_between(fast["impulse_mag"], 5.0, 25.0,
		"a 1 kg body stopped/bounced from 10 m/s must report an impulse in [5, 25] N·s (got %.3f)" % fast["impulse_mag"])

	# --- Launch 2: 3 m/s — the same collision, softer. Impulse must scale down ---
	var slow := await _launch_and_measure(3.0)
	gut.p("GATE 1 — launch @ 3 m/s: %s" % slow)

	assert_true(slow["recorded"],
		"the 3 m/s probe must also register a contact within the ceiling")
	if not slow["recorded"]:
		return

	assert_gt(slow["impulse_mag"], 0.0,
		"the slower launch must also report a positive impulse")

	# (d) proportionality (loose, not exact): a 3 m/s hit carries ~0.3× the
	# momentum of a 10 m/s hit, so its impulse must be clearly smaller. We only
	# demand strictly-less plus a sane ratio band [0.1, 0.7] — restitution and
	# solver detail move the exact figure, but the ordering is a law.
	assert_lt(slow["impulse_mag"], fast["impulse_mag"],
		"the 3 m/s impulse (%.3f) must be smaller than the 10 m/s impulse (%.3f)" % [slow["impulse_mag"], fast["impulse_mag"]])
	var ratio: float = slow["impulse_mag"] / fast["impulse_mag"]
	assert_between(ratio, 0.1, 0.7,
		"the slow/fast impulse ratio must track the ~0.3 speed ratio, loosely (got %.3f)" % ratio)

	# --- The gate evidence, emitted raw for the log ---
	gut.p("GATE 1 VERDICT — Jolt fills get_contact_impulse: 10 m/s -> %.3f N·s, 3 m/s -> %.3f N·s, ratio %.3f" % [fast["impulse_mag"], slow["impulse_mag"], ratio])
