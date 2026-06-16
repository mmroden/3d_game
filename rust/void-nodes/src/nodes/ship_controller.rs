use godot::prelude::*;
use godot::classes::{
    RigidBody3D, IRigidBody3D, PhysicsDirectBodyState3D, PhysicsRayQueryParameters3D,
    MeshInstance3D, Camera3D,
    GpuParticles3D, SphereMesh, StandardMaterial3D,
    Input,
};

use super::constants::{actions, groups, methods, signals};
use super::godot_util;

use void_logic::debuff::SlowDebuff;
use void_logic::laser::LaserLevel;
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

/// Camera-shake burst when a grabber latches on: how long and how hard
/// (Camera3D frustum-offset units).
const SHAKE_DURATION: f32 = 0.5;
const SHAKE_AMP: f32 = 0.25;

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
    beam_nodes: Vec<Gd<MeshInstance3D>>,
    loadout: Loadout,
    laser_level: LaserLevel,
    power_mode: PowerMode,
    /// Movement slow applied by swarmer contact.
    slow: SlowDebuff,
    /// Cached player camera, jittered for the grab shake.
    camera: Option<Gd<Camera3D>>,
    shake_timer: f32,
    shake_phase: f32,
}

#[godot_api]
impl IRigidBody3D for ShipController {
    fn init(base: Base<RigidBody3D>) -> Self {
        Self {
            base,
            weapon: WeaponState::default(),
            beam_nodes: Vec::new(),
            loadout: Loadout::new(),
            laser_level: LaserLevel::Red,
            power_mode: PowerMode::default(),
            slow: SlowDebuff::new(),
            camera: None,
            shake_timer: 0.0,
            shake_phase: 0.0,
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
        }
        // Decay is the engine's: damping = -ln(retention). Sets the
        // no-infinite-spin invariant (angular_damp > 0).
        self.apply_envelope();

        // Cache the camera for the grab shake (None-safe if absent).
        self.camera = self.base().try_get_node_as::<Camera3D>("Camera3D");
    }

    /// Flight runs in `integrate_forces`, the engine's hook for safely
    /// touching physics state. Forces/torque only — never velocity writes
    /// (the one exception, stabilize, is a deliberate discrete reset).
    fn integrate_forces(&mut self, state: Option<Gd<PhysicsDirectBodyState3D>>) {
        let Some(mut state) = state else {
            return;
        };
        self.fly(&mut state);
    }

    /// Weapons + power routing. Hitscan needs the space unlocked, which is
    /// only true in `physics_process` — so it lives here, not in
    /// `integrate_forces`.
    fn physics_process(&mut self, delta: f64) {
        self.handle_weapons_and_power(delta as f32);
        // Tick the movement slow; tell the HUD only when it switches on/off.
        if self.slow.tick(delta as f32) {
            let active = self.slow.is_active();
            self.base_mut()
                .emit_signal(signals::PLAYER_SLOWED, &[Variant::from(active)]);
        }
        self.tick_shake(delta as f32);
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

    /// Jitter the camera with a decaying offset while the grab shake is active.
    fn tick_shake(&mut self, delta: f32) {
        let Some(mut camera) = self.camera.clone() else { return };
        if !camera.is_instance_valid() {
            return;
        }
        if self.shake_timer > 0.0 {
            self.shake_timer = (self.shake_timer - delta).max(0.0);
            self.shake_phase += delta;
            let amp = SHAKE_AMP * (self.shake_timer / SHAKE_DURATION);
            camera.set_h_offset((self.shake_phase * 91.0).sin() * amp);
            camera.set_v_offset((self.shake_phase * 123.0).cos() * amp);
            if self.shake_timer <= 0.0 {
                camera.set_h_offset(0.0);
                camera.set_v_offset(0.0);
            }
        }
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
                    * self.slow.multiplier());
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
        // Player damage SFX (non-positional since it's the player)
        if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
            audio.bind_mut().play_event(SfxEvent::ImpactHeavy);
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

        // Each laser fires full damage independently
        let left_origin = center - right * WING_OFFSET;
        let right_origin = center + right * WING_OFFSET;

        self.fire_single_ray(left_origin, forward, damage);
        self.fire_single_ray(right_origin, forward, damage);

        // Laser fire SFX (non-positional — it's the player's own gun)
        if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
            audio.bind_mut().play_event(SfxEvent::LaserFire);
        }
    }

    fn fire_single_ray(&mut self, origin: Vector3, forward: Vector3, damage: f32) {
        let end = origin + forward * self.weapon.max_range;

        let Some(world) = self.base().get_world_3d() else { return };
        let Some(mut space) = world.get_direct_space_state() else { return };
        let Some(mut query) = PhysicsRayQueryParameters3D::create(origin, end) else { return };
        let self_rid = self.base().get_rid();
        query.set_exclude(&array![self_rid]);

        let result = space.intersect_ray(&query);
        let hit_point = if result.is_empty() {
            end
        } else {
            let Some(hit_pos_var) = result.get("position") else { return };
            let hit_pos = hit_pos_var.to::<Vector3>();

            if let Some(collider) = result.get("collider") {
                let mut obj = collider.to::<Gd<Node3D>>();
                if obj.has_method(methods::TAKE_DAMAGE) {
                    obj.call(methods::TAKE_DAMAGE, &[Variant::from(damage)]);
                }
            }

            // Spawn hit sparks at impact point
            let hit_normal = result.get("normal").unwrap_or(Variant::from(Vector3::UP)).to::<Vector3>();
            self.spawn_hit_sparks(hit_pos, hit_normal);

            hit_pos
        };

        self.spawn_beam(origin, hit_point);
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
                let node = mesh_instance.clone();
                root.clone().add_child(&mesh_instance);
                self.beam_nodes.push(node);
            }
        }
    }

    fn age_beams(&mut self, delta: f32) {
        const BEAM_LIFETIME: f32 = 0.08;
        godot_util::age_beams(&mut self.beam_nodes, delta, BEAM_LIFETIME, &self.laser_level.color());
    }
}
