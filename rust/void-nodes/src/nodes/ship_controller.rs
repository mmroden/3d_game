use godot::prelude::*;
use godot::classes::{
    RigidBody3D, IRigidBody3D, PhysicsDirectBodyState3D, PhysicsRayQueryParameters3D,
    MeshInstance3D, Camera3D, Node3D, CollisionShape3D, CapsuleShape3D, OmniLight3D,
    GpuParticles3D, SphereMesh, StandardMaterial3D,
    Input,
};

use super::constants::{actions, groups, methods, scenes, signals};
use super::godot_util;
use super::live_handle::{LiveOpt, LiveRef, LiveVec};

use void_logic::debuff::SlowDebuff;
use void_logic::laser::LaserLevel;
use void_logic::ship::ShipColor;
use void_logic::loadout::Loadout;
use void_logic::power_routing::PowerMode;
use void_logic::upgrade::{Upgrade, UpgradeKind};
use void_logic::audio_catalog::SfxEvent;
use void_logic::weapon::{WeaponState, FireResult};

/// Commanded angular speed (rad/s) at full stick (before rotation
/// upgrades). A feel value to tune.
const TURN_RATE: f32 = 2.5;
/// Stiffness of the steering controller: how hard torque drives spin
/// toward the commanded rate. Higher = snappier. A feel value to tune.
const TURN_GAIN: f32 = 12.0;

/// Wing offset from ship center (local X axis), in meters.
const WING_OFFSET: f32 = 0.3;

/// Aim forgiveness: lasers also test a ring of this radius around the centre
/// line, so a near-miss still connects instead of needing pixel-perfect aim.
const LASER_AIM_RADIUS: f32 = 0.7;
/// Minimum gap between collision sounds (so a wall scrape doesn't machine-gun).
const IMPACT_SOUND_COOLDOWN: f32 = 0.4;

/// Camera-shake burst when a grabber latches on: how long and how hard
/// (Camera3D frustum-offset units).
const SHAKE_DURATION: f32 = 0.5;
const SHAKE_AMP: f32 = 0.25;

/// Target length (longest dimension) of the player ship, in world units.
/// The model is auto-scaled to this regardless of its native size.
const TARGET_SHIP_LENGTH: f32 = 2.0;
/// Flight collider capsule, in world units (independent of the model's scale).
/// Tighter than the visual hull so the ship slides through doorways.
const SHIP_COLLIDER_RADIUS: f32 = 0.45;
const SHIP_COLLIDER_HEIGHT: f32 = 1.1;
/// Cockpit camera offset (local), looking forward.
const COCKPIT_OFFSET: Vector3 = Vector3::new(0.0, 0.5, 0.0);
/// Chase camera offset (local): behind (+Z) and above the ship — high enough
/// to see where the nose points.
const CHASE_OFFSET: Vector3 = Vector3::new(0.0, 2.4, 4.5);

/// Which way the player views the ship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CameraMode {
    Cockpit,
    Chase,
}

/// 6DOF flight controller for the player ship. Per the simulation idiom
/// (docs/architecture/physics_ownership.md): the engine owns motion,
/// collision, decay and rest; we only supply intent inside
/// `integrate_forces` — thrust as force, steering as torque toward a
/// commanded angular velocity — and let the engine integrate and damp.
/// `angular_damp > 0` is the no-infinite-spin guarantee. Lasers stay
/// hitscan (in `physics_process`, where space queries are legal).
#[derive(GodotClass)]
#[class(base=RigidBody3D)]
pub struct ShipController {
    base: Base<RigidBody3D>,

    weapon: WeaponState,
    beam_nodes: LiveVec<MeshInstance3D>,
    loadout: Loadout,
    laser_level: LaserLevel,
    power_mode: PowerMode,
    /// Movement slow applied by swarmer contact.
    slow: SlowDebuff,
    /// Cached player camera, jittered for the grab shake.
    camera: Option<LiveRef<Camera3D>>,
    shake_timer: f32,
    shake_phase: f32,
    camera_mode: CameraMode,
    /// Throttle so a wall scrape doesn't machine-gun the impact clang.
    impact_cooldown: f32,
    /// Edge-detect collision onset so resting against a wall stays silent.
    was_in_contact: bool,
    /// The ship model node — hidden in cockpit view (you look out of it),
    /// shown in chase view.
    ship_model: Option<LiveRef<Node3D>>,
    /// Color accent light on the ship model (the player's chosen color).
    color_glow: Option<LiveRef<OmniLight3D>>,
    /// Flight-speed multiplier from the chosen ship color.
    ship_thrust_mul: f32,
}

#[godot_api]
impl IRigidBody3D for ShipController {
    fn init(base: Base<RigidBody3D>) -> Self {
        Self {
            base,
            weapon: WeaponState::default(),
            beam_nodes: LiveVec::new(),
            loadout: Loadout::new(),
            laser_level: LaserLevel::Red,
            power_mode: PowerMode::default(),
            slow: SlowDebuff::new(),
            camera: None,
            shake_timer: 0.0,
            shake_phase: 0.0,
            camera_mode: CameraMode::Cockpit,
            impact_cooldown: 0.0,
            was_in_contact: false,
            ship_model: None,
            color_glow: None,
            ship_thrust_mul: 1.0,
        }
    }

    fn ready(&mut self) {
        self.base_mut().add_to_group(groups::PLAYER);
        {
            let mut base = self.base_mut();
            base.set_gravity_scale(0.0);
            base.set_can_sleep(false);
            // Continuous collision so fast flight never tunnels through walls.
            base.set_use_continuous_collision_detection(true);
            // Report contacts so we can play an impact clang on collision.
            base.set_contact_monitor(true);
            base.set_max_contacts_reported(4);
        }
        // Decay is the engine's: damping = -ln(retention). Sets the
        // no-infinite-spin invariant (angular_damp > 0).
        self.apply_envelope();

        // Cache the camera for the grab shake (None-safe if absent).
        self.camera = self.base()
            .try_get_node_as::<Camera3D>("Camera3D")
            .map(|c| LiveRef::new(&c));
        self.spawn_ship_model();
        self.apply_camera_mode();
    }

    /// Flight runs in `integrate_forces`, the engine's hook for safely
    /// touching physics state. Forces/torque only — never velocity writes
    /// (the one exception, stabilize, is a deliberate discrete reset).
    fn integrate_forces(&mut self, state: Option<Gd<PhysicsDirectBodyState3D>>) {
        let Some(mut state) = state else {
            return;
        };
        self.fly(&mut state);

        // Heavy collision thud, on impact *onset* (resting against a wall is
        // silent) and throttled by the cooldown. Non-positional — positional
        // audio is currently inaudible in this project.
        let in_contact = state.get_contact_count() > 0;
        if in_contact && !self.was_in_contact && self.impact_cooldown <= 0.0 {
            self.impact_cooldown = IMPACT_SOUND_COOLDOWN;
            if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
                audio.bind_mut().play_event(SfxEvent::ImpactHeavy);
            }
        }
        self.was_in_contact = in_contact;
    }

    /// Weapons + power routing. Hitscan needs the space unlocked, which is
    /// only true in `physics_process` — so it lives here, not in
    /// `integrate_forces`.
    fn physics_process(&mut self, delta: f64) {
        if Input::singleton().is_action_just_pressed(actions::TOGGLE_VIEW) {
            self.camera_mode = match self.camera_mode {
                CameraMode::Cockpit => CameraMode::Chase,
                CameraMode::Chase => CameraMode::Cockpit,
            };
            self.apply_camera_mode();
        }
        self.handle_weapons_and_power(delta as f32);
        // Tick the movement slow; tell the HUD only when it switches on/off.
        if self.slow.tick(delta as f32) {
            let active = self.slow.is_active();
            self.base_mut()
                .emit_signal(signals::PLAYER_SLOWED, &[Variant::from(active)]);
        }
        self.tick_shake(delta as f32);
        self.impact_cooldown = (self.impact_cooldown - delta as f32).max(0.0);
    }
}

#[godot_api]
impl ShipController {
    #[signal]
    fn player_damaged(amount: f32);

    #[signal]
    fn power_mode_changed(mode: i32);

    /// Player started/stopped being slowed (drives the HUD indicator).
    #[signal]
    fn player_slowed(active: bool);

    /// Apply a movement slow (e.g. from swarmer contact). `factor` is the
    /// thrust multiplier (0..1); `duration` is in seconds.
    #[func]
    pub fn apply_slow(&mut self, factor: f32, duration: f32) {
        let was_active = self.slow.is_active();
        self.slow.apply(factor, duration);
        // On a fresh grab: tell the HUD, kick the camera shake, and play the
        // latch sound so the player feels the attachment land.
        if !was_active && self.slow.is_active() {
            self.shake_timer = SHAKE_DURATION;
            let pos = self.base().get_global_position();
            if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
                audio.bind_mut().play_event_at(SfxEvent::ImpactMetal, pos);
            }
            self.base_mut()
                .emit_signal(signals::PLAYER_SLOWED, &[Variant::from(true)]);
        }
    }

    /// Apply the player's chosen ship color (accent glow) and its flight-speed
    /// multiplier. Called by GameManager when the ship color is chosen/synced.
    #[func]
    pub fn configure_ship(&mut self, color: Color, thrust_mul: f32) {
        self.ship_thrust_mul = thrust_mul;
        godot_util::recolor_glow(&self.color_glow, color);
    }

    /// Build the player ship: the shared model helper, a capsule collider, and
    /// the color accent light.
    fn spawn_ship_model(&mut self) {
        let mut parent: Gd<Node3D> = self.base().clone().upcast();
        let Some(mut model) =
            godot_util::spawn_fitted_model(&mut parent, scenes::SHIP_MODEL, TARGET_SHIP_LENGTH)
        else {
            return;
        };
        self.ship_model = Some(LiveRef::new(&model));

        // Flight collider: a capsule laid along the hull. It's rounded, so it
        // slides off doorframe edges instead of snagging a wingtip, and sized in
        // world units so it never inherits the imported model's extreme scale —
        // a scaled, mesh-derived hull sized wrong in Jolt and let the ship clip
        // through walls. The visual model may overhang it slightly.
        if let Some(mut shape_node) =
            self.base().try_get_node_as::<CollisionShape3D>("CollisionShape3D")
        {
            let mut capsule = CapsuleShape3D::new_gd();
            capsule.set_radius(SHIP_COLLIDER_RADIUS);
            capsule.set_height(SHIP_COLLIDER_HEIGHT);
            shape_node.set_shape(&capsule);
            // Lay the capsule along the ship's length rather than its default Y.
            shape_node.set_rotation(Vector3::new(std::f32::consts::FRAC_PI_2, 0.0, 0.0));
            shape_node.set_position(Vector3::ZERO);
        }

        // Color accent light (Standard until GameManager pushes the choice).
        let c = ShipColor::default().color();
        self.color_glow = Some(godot_util::attach_glow_light(&mut model, &c, 2.0, 6.0));
    }

    /// Place the camera for the current view mode, and show the ship model only
    /// in chase view (in cockpit you're inside the hull, so it would block the view).
    fn apply_camera_mode(&mut self) {
        let chase = self.camera_mode == CameraMode::Chase;
        self.ship_model.with(|model| model.set_visible(chase));
        let transform = match self.camera_mode {
            CameraMode::Cockpit => Transform3D::new(Basis::IDENTITY, COCKPIT_OFFSET),
            CameraMode::Chase => {
                // Look from behind/above toward a point just ahead of the ship.
                let look_dir = (Vector3::new(0.0, 0.0, -2.0) - CHASE_OFFSET).normalized();
                Transform3D::new(godot_util::basis_from_direction(look_dir), CHASE_OFFSET)
            }
        };
        self.camera.with(|camera| camera.set_transform(transform));
    }

    /// Jitter the camera with a decaying offset while the grab shake is active.
    fn tick_shake(&mut self, delta: f32) {
        if self.shake_timer <= 0.0 {
            return;
        }
        // Advance the shake state first, then push the offsets to the camera —
        // self can't be mutated while `camera.with` borrows it.
        self.shake_timer = (self.shake_timer - delta).max(0.0);
        self.shake_phase += delta;
        let (h, v) = if self.shake_timer <= 0.0 {
            (0.0, 0.0)
        } else {
            let amp = SHAKE_AMP * (self.shake_timer / SHAKE_DURATION);
            (
                (self.shake_phase * 91.0).sin() * amp,
                (self.shake_phase * 123.0).cos() * amp,
            )
        };
        self.camera.with(|camera| {
            camera.set_h_offset(h);
            camera.set_v_offset(v);
        });
    }

    /// Set linear/angular damping from the loadout's per-second
    /// `Retention` (`damp = -ln(retention)`). The engine's damping *is*
    /// the project's retention decay; `angular_damp > 0` is the
    /// no-infinite-spin invariant. Re-applied whenever the loadout changes.
    fn apply_envelope(&mut self) {
        let damp = (-self.loadout.damping().factor().ln()).max(0.0);
        // The `.max(0.0)` is belt-and-suspenders: `Retention::decaying`
        // clamps the factor below 1.0, so `-ln(factor)` is already strictly
        // positive. Zero would break the no-infinite-spin invariant, so pin
        // that it's unreachable — and catch any future loosening of the clamp.
        debug_assert!(damp > 0.0, "angular/linear damp must be > 0 (no-infinite-spin invariant)");
        let mut base = self.base_mut();
        base.set_linear_damp(damp);
        base.set_angular_damp(damp);
    }

    /// Flight intent, applied to the physics state inside
    /// `integrate_forces`. Thrust is a force; steering is torque toward a
    /// commanded angular velocity (zero when the stick is centered, so the
    /// engine drives spin back to rest — no uncommanded tumble). We never
    /// assign velocity to steer; the lone exception is stabilize.
    fn fly(&mut self, state: &mut Gd<PhysicsDirectBodyState3D>) {
        let input = Input::singleton();

        let forward = input.get_action_strength(actions::MOVE_FORWARD)
            - input.get_action_strength(actions::MOVE_BACK);
        let strafe = input.get_action_strength(actions::MOVE_RIGHT)
            - input.get_action_strength(actions::MOVE_LEFT);
        let vertical = input.get_action_strength(actions::MOVE_UP)
            - input.get_action_strength(actions::MOVE_DOWN);
        let pitch = input.get_action_strength(actions::LOOK_UP)
            - input.get_action_strength(actions::LOOK_DOWN);
        let yaw = input.get_action_strength(actions::LOOK_LEFT)
            - input.get_action_strength(actions::LOOK_RIGHT);
        let roll = input.get_action_strength(actions::ROLL_RIGHT)
            - input.get_action_strength(actions::ROLL_LEFT);

        let basis = state.get_transform().basis;

        // Thrust: a force along the hull axes. The engine integrates it
        // and (via linear_damp) bleeds it back toward rest — terminal
        // cruise speed ≈ thrust / (mass · linear_damp).
        let thrust_dir =
            basis.col_c() * (-forward) + basis.col_a() * strafe + basis.col_b() * vertical;
        if thrust_dir.length() > 0.01 {
            let force = thrust_dir.normalized()
                * (self.loadout.thrust_power()
                    * self.power_mode.thrust_multiplier()
                    * self.slow.multiplier()
                    * self.ship_thrust_mul);
            state.apply_central_force_ex().force(force).done();
        }

        // Steering: torque toward a commanded angular velocity. Rotation
        // upgrades scale the rate; the controller drives spin toward the
        // command (and to zero on release).
        let turn_mult = self.loadout.rotation_speed() / self.loadout.base.rotation_speed;
        let command = basis * Vector3::new(pitch, yaw, roll) * (TURN_RATE * turn_mult);
        let torque = (command - state.get_angular_velocity()) * TURN_GAIN;
        state.apply_torque(torque);

        // Stabilize: the one allowed direct velocity write — a deliberate,
        // discrete hard stop on the spin.
        if input.is_action_pressed(actions::STABILIZE) {
            state.set_angular_velocity(Vector3::ZERO);
        }
    }

    /// Weapons and power routing — runs in `physics_process` because
    /// hitscan needs the physics space unlocked.
    fn handle_weapons_and_power(&mut self, delta: f32) {
        let input = Input::singleton();

        // Power routing: toggle on press.
        let old_mode = self.power_mode;
        if input.is_action_just_pressed(actions::ROUTE_SHIELDS) {
            self.power_mode = if self.power_mode == PowerMode::ShieldBoost {
                PowerMode::Balanced
            } else {
                PowerMode::ShieldBoost
            };
        }
        if input.is_action_just_pressed(actions::ROUTE_WEAPONS) {
            self.power_mode = if self.power_mode == PowerMode::WeaponBoost {
                PowerMode::Balanced
            } else {
                PowerMode::WeaponBoost
            };
        }
        if self.power_mode != old_mode {
            let mode_val = self.power_mode as i32;
            self.base_mut()
                .emit_signal(signals::POWER_MODE_CHANGED, &[Variant::from(mode_val)]);
        }

        // Weapon.
        self.weapon.fire_rate = self.loadout.fire_rate() * self.power_mode.fire_rate_multiplier();
        self.weapon.damage = void_logic::newtypes::Damage::new(self.laser_level.damage());
        self.weapon.tick(delta);
        self.age_beams(delta);

        if input.is_action_pressed(actions::FIRE) {
            if let FireResult::Fired { damage } = self.weapon.try_fire() {
                self.fire_dual_lasers(damage.as_f32());
            }
        }
    }

    /// Variant-boundary wrapper: the one f32→Damage conversion for
    /// GDScript and `Object::call` dispatch. Rust callers use
    /// `apply_damage`.
    #[func]
    pub fn take_damage(&mut self, amount: f32) {
        self.apply_damage(void_logic::newtypes::Damage::new(amount));
    }

    /// Called when a projectile or enemy hits this ship.
    pub fn apply_damage(&mut self, damage: void_logic::newtypes::Damage) {
        // Energy-hit sound — distinct from the heavy collision thud.
        if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
            audio.bind_mut().play_event(SfxEvent::ImpactShield);
        }
        // Signals are Variant territory: convert at the boundary.
        self.base_mut().emit_signal(
            signals::PLAYER_DAMAGED,
            &[Variant::from(damage.as_f32())],
        );
    }

    /// Reset local state to match a fresh RunState (called by GameManager on new/continue).
    #[func]
    pub fn reset_loadout(&mut self) {
        self.loadout = Loadout::new();
        self.laser_level = LaserLevel::Red;
        self.power_mode = PowerMode::default();
        self.apply_envelope();
    }

    #[func]
    pub fn set_laser_level(&mut self, level: i32) {
        if let Some(laser) = LaserLevel::from_level(level as u32) {
            self.laser_level = laser;
            godot_print!("Laser set to {} (damage: {})", laser.display_name(), laser.damage());
        }
    }

    #[func]
    pub fn apply_upgrade(&mut self, name: GString, kind_id: i32, multiplier: f32) {
        let kind = match kind_id {
            0 => UpgradeKind::Thrust,
            1 => UpgradeKind::RotationSpeed,
            2 => UpgradeKind::Damping,
            3 => UpgradeKind::MaxHealth,
            4 => UpgradeKind::FireRate,
            5 => UpgradeKind::ProjectileSpeed,
            6 => UpgradeKind::ProjectileDamage,
            _ => {
                godot_warn!("Unknown upgrade kind: {kind_id}");
                return;
            }
        };
        let upgrade = Upgrade {
            name: name.to_string(),
            kind,
            multiplier,
        };
        godot_print!("Applied upgrade: {} (x{:.2})", upgrade.name, upgrade.multiplier);
        self.loadout.add_upgrade(upgrade);
        // Stability upgrades change retention → re-derive engine damping.
        self.apply_envelope();
    }

    fn fire_dual_lasers(&mut self, damage: f32) {
        let global_basis = self.base().get_global_transform().basis;
        let center = self.base().get_global_position() + global_basis.col_b() * 0.5;
        let forward = -global_basis.col_c();
        let right = global_basis.col_a();
        let up = global_basis.col_b();

        let left_origin = center - right * WING_OFFSET;
        let right_origin = center + right * WING_OFFSET;

        // Hit-test down the reticle (centre line) with an aim-assist spread, then
        // converge both visible beams on whatever it found — so the lasers hit
        // where the crosshair points, not parallel-offset from the wings.
        let hit_point = self.cast_forgiving(center, forward, right, up, damage * 2.0);
        self.spawn_beam(left_origin, hit_point);
        self.spawn_beam(right_origin, hit_point);

        // Laser fire SFX (non-positional — it's the player's own gun)
        if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
            audio.bind_mut().play_event(SfxEvent::LaserFire);
        }
    }

    /// Hit-test from the centre line plus a ring of offset rays (aim assist).
    /// The first ray to strike a damageable target wins (full dual damage);
    /// otherwise the centre ray's wall/end point is returned so the beams land.
    fn cast_forgiving(
        &mut self,
        center: Vector3,
        forward: Vector3,
        right: Vector3,
        up: Vector3,
        damage: f32,
    ) -> Vector3 {
        let max_range = self.weapon.max_range;
        let fallback = center + forward * max_range;
        let Some(world) = self.base().get_world_3d() else { return fallback };
        let Some(mut space) = world.get_direct_space_state() else { return fallback };
        let self_rid = self.base().get_rid();

        let r = LASER_AIM_RADIUS;
        let offsets = [Vector3::ZERO, right * r, -right * r, up * r, -up * r];
        let mut beam_end = fallback;
        for (i, off) in offsets.iter().enumerate() {
            let origin = center + *off;
            let Some(mut query) =
                PhysicsRayQueryParameters3D::create(origin, origin + forward * max_range)
            else {
                continue;
            };
            query.set_exclude(&array![self_rid]);
            let result = space.intersect_ray(&query);
            if result.is_empty() {
                continue;
            }
            let Some(hit_pos_var) = result.get("position") else { continue };
            let hit_pos = hit_pos_var.to::<Vector3>();
            if let Some(collider) = result.get("collider") {
                let mut obj = collider.to::<Gd<Node3D>>();
                if obj.has_method(methods::TAKE_DAMAGE) {
                    obj.call(methods::TAKE_DAMAGE, &[Variant::from(damage)]);
                    let normal = result
                        .get("normal")
                        .unwrap_or(Variant::from(Vector3::UP))
                        .to::<Vector3>();
                    self.spawn_hit_sparks(hit_pos, normal);
                    return hit_pos;
                }
            }
            // No target on this ray; the centre ray defines where the beam lands.
            if i == 0 {
                beam_end = hit_pos;
            }
        }
        beam_end
    }

    fn spawn_hit_sparks(&mut self, position: Vector3, normal: Vector3) {
        let mut particles = GpuParticles3D::new_alloc();
        particles.set_amount(12);
        particles.set_lifetime(0.3);
        particles.set_one_shot(true);
        particles.set_explosiveness_ratio(1.0);

        let mut mat = godot_util::particle_burst_material(
            45.0,
            Color::from_rgba(1.0, 0.6, 0.2, 1.0),
            (3.0, 6.0),
            Some((0.5, 1.0)),
        );
        mat.set_direction(normal);
        particles.set_process_material(&mat);

        let mut sphere = SphereMesh::new_gd();
        sphere.set_radius(0.015);
        sphere.set_height(0.03);
        let mut spark_mat = StandardMaterial3D::new_gd();
        spark_mat.set_albedo(Color::from_rgba(1.0, 0.5, 0.1, 1.0));
        spark_mat.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
        spark_mat.set_emission(Color::from_rgba(1.0, 0.4, 0.1, 1.0));
        spark_mat.set_emission_energy_multiplier(8.0);
        sphere.set_material(&spark_mat);
        particles.set_draw_pass_mesh(0, &sphere);

        particles.set_transform(Transform3D::new(Basis::IDENTITY, position));
        if let Some(root) = godot_util::scene_root(self.base().get_tree()) {
            root.clone().add_child(&particles);
            particles.set_emitting(true);
            // Self-destruct after sparks finish
            let mut timer = particles.get_tree().create_timer(0.5);
            let callable = particles.callable("queue_free");
            timer.connect("timeout", &callable);
        }
    }

    fn spawn_beam(&mut self, from: Vector3, to: Vector3) {
        if let Some(mesh_instance) = godot_util::create_beam_mesh(from, to, &self.laser_level.color()) {
            if let Some(root) = godot_util::scene_root(self.base().get_tree()) {
                root.clone().add_child(&mesh_instance);
                self.beam_nodes.push(&mesh_instance, ());
            }
        }
    }

    fn age_beams(&mut self, delta: f32) {
        const BEAM_LIFETIME: f32 = 0.08;
        godot_util::age_beams(&mut self.beam_nodes, delta, BEAM_LIFETIME, &self.laser_level.color());
    }
}
