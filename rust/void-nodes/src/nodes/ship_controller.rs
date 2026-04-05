use godot::prelude::*;
use godot::classes::{
    CharacterBody3D, ICharacterBody3D, PhysicsRayQueryParameters3D,
    MeshInstance3D,
    GpuParticles3D, SphereMesh, StandardMaterial3D,
    Input,
};

use super::constants::{actions, groups, methods, signals};
use super::godot_util;

use void_logic::laser::LaserLevel;
use void_logic::loadout::Loadout;
use void_logic::power_routing::PowerMode;
use void_logic::ram_damage;
use void_logic::upgrade::{Upgrade, UpgradeKind};
use void_logic::audio_catalog::{SfxEvent, COLLISION_SFX_MIN_SPEED};
use void_logic::weapon::{WeaponState, FireResult};

/// Clamp velocity to max magnitude, reset to zero if NaN.
fn sanitize_velocity(v: Vector3, max_speed: f32) -> Vector3 {
    if v.x.is_nan() || v.y.is_nan() || v.z.is_nan() {
        return Vector3::ZERO;
    }
    let len = v.length();
    if len > max_speed {
        v * (max_speed / len)
    } else {
        v
    }
}

/// Check if all quaternion components are finite.
fn is_quat_finite(q: Quaternion) -> bool {
    q.x.is_finite() && q.y.is_finite() && q.z.is_finite() && q.w.is_finite()
}


/// Wing offset from ship center (local X axis), in meters.
const WING_OFFSET: f32 = 0.3;

/// 6DOF flight controller for the player ship.
/// Dual wing-mounted hitscan lasers, upgrade loadout.
#[derive(GodotClass)]
#[class(base=CharacterBody3D)]
pub struct ShipController {
    base: Base<CharacterBody3D>,

    #[export]
    thrust_power: f32,
    #[export]
    rotation_speed: f32,
    #[export]
    damping: f32,
    linear_velocity: Vector3,
    angular_velocity: Vector3,
    weapon: WeaponState,
    beam_nodes: Vec<Gd<MeshInstance3D>>,
    loadout: Loadout,
    laser_level: LaserLevel,
    power_mode: PowerMode,
}

#[godot_api]
impl ICharacterBody3D for ShipController {
    fn init(base: Base<CharacterBody3D>) -> Self {
        Self {
            base,
            thrust_power: 40.0,
            rotation_speed: 6.0,
            damping: 0.95,
            linear_velocity: Vector3::ZERO,
            angular_velocity: Vector3::ZERO,
            weapon: WeaponState::default(),
            beam_nodes: Vec::new(),
            loadout: Loadout::new(),
            laser_level: LaserLevel::Red,
            power_mode: PowerMode::default(),
        }
    }

    fn ready(&mut self) {
        self.base_mut().add_to_group(groups::PLAYER);
    }

    fn physics_process(&mut self, delta: f64) {
        let delta = delta as f32;
        let input = Input::singleton();

        // --- Movement ---
        let forward = input.get_action_strength(actions::MOVE_FORWARD) - input.get_action_strength(actions::MOVE_BACK);
        let strafe = input.get_action_strength(actions::MOVE_RIGHT) - input.get_action_strength(actions::MOVE_LEFT);
        let vertical = input.get_action_strength(actions::MOVE_UP) - input.get_action_strength(actions::MOVE_DOWN);

        let pitch = input.get_action_strength(actions::LOOK_UP) - input.get_action_strength(actions::LOOK_DOWN);
        let yaw = input.get_action_strength(actions::LOOK_LEFT) - input.get_action_strength(actions::LOOK_RIGHT);
        let roll = input.get_action_strength(actions::ROLL_RIGHT) - input.get_action_strength(actions::ROLL_LEFT);

        // Stabilizer: kill angular velocity on L1/Tab
        if input.is_action_pressed(actions::STABILIZE) {
            self.angular_velocity = Vector3::ZERO;
        }

        // Power routing: toggle on press
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
            self.base_mut().emit_signal(
                "power_mode_changed",
                &[Variant::from(mode_val)],
            );
        }

        let basis = self.base().get_transform().basis;
        let thrust = basis.col_c() * (-forward)
            + basis.col_a() * strafe
            + basis.col_b() * vertical;

        let effective_thrust = self.loadout.thrust_power() * self.power_mode.thrust_multiplier();
        let effective_rotation = self.loadout.rotation_speed();
        let effective_damping = self.loadout.damping();

        self.linear_velocity += thrust * effective_thrust * delta;
        self.linear_velocity *= effective_damping;

        self.angular_velocity += Vector3::new(pitch, yaw, roll) * effective_rotation * delta;
        self.angular_velocity *= effective_damping;

        // Sanitize: clamp velocities to sane maximums and reset NaN.
        const MAX_LINEAR_SPEED: f32 = 50.0;
        const MAX_ANGULAR_SPEED: f32 = 5.0;
        self.linear_velocity = sanitize_velocity(self.linear_velocity, MAX_LINEAR_SPEED);
        self.angular_velocity = sanitize_velocity(self.angular_velocity, MAX_ANGULAR_SPEED);

        let vel = self.linear_velocity;
        let rot = self.angular_velocity * delta;

        self.base_mut().set_velocity(vel);
        self.base_mut().move_and_slide();
        // Accept the collision-resolved velocity as truth.
        // Without this, thrust accumulates into walls until the player clips through.
        self.linear_velocity = self.base().get_velocity();

        // Check for physical collisions (walls, objects, enemies)
        let pre_collision_speed = vel.length();
        let collision_count = self.base().get_slide_collision_count();
        let mut played_collision_sfx = false;

        for i in 0..collision_count {
            let Some(collision) = self.base().get_slide_collision(i) else { continue };

            // Play collision SFX once per frame if speed is high enough
            if !played_collision_sfx && pre_collision_speed > COLLISION_SFX_MIN_SPEED {
                let contact = collision.get_position();
                if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
                    audio.bind_mut().play_event_at(SfxEvent::ImpactMetal, contact);
                }
                played_collision_sfx = true;
            }

            // Ram damage to enemies
            if let Some(collider) = collision.get_collider() {
                let obj: Gd<Node3D> = collider.cast();
                if obj.is_in_group(groups::ENEMIES) && obj.has_method(methods::TAKE_DAMAGE) {
                    let dmg = ram_damage::ram_damage(pre_collision_speed);
                    if dmg.as_f32() > 0.0 {
                        let mut enemy = obj;
                        enemy.call(methods::TAKE_DAMAGE, &[Variant::from(dmg.as_f32())]);
                        let player_dmg = dmg.as_f32() * ram_damage::PLAYER_RAM_FRACTION;
                        self.base_mut().emit_signal(
                            signals::PLAYER_DAMAGED,
                            &[Variant::from(player_dmg)],
                        );
                    }
                }
            }
        }

        // Quaternion-based rotation: compose pitch/yaw/roll independently
        // then apply to current orientation. Avoids gimbal lock and
        // frame-of-reference contamination from sequential rotate_x/y/z.
        let local_rotation = Quaternion::from_axis_angle(Vector3::RIGHT, rot.x)
            * Quaternion::from_axis_angle(Vector3::UP, rot.y)
            * Quaternion::from_axis_angle(Vector3::BACK, rot.z);
        let current_quat = self.base().get_quaternion();
        let new_quat = (current_quat * local_rotation).normalized();

        // Guard: if the quaternion became NaN, keep the previous one.
        if is_quat_finite(new_quat) {
            self.base_mut().set_quaternion(new_quat);
        }

        // --- Weapon ---
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
}

#[godot_api]
impl ShipController {
    #[signal]
    fn player_damaged(amount: f32);

    #[signal]
    fn power_mode_changed(mode: i32);

    /// Called when a projectile or enemy hits this ship.
    #[func]
    pub fn take_damage(&mut self, amount: f32) {
        // Player damage SFX (non-positional since it's the player)
        if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
            audio.bind_mut().play_event(SfxEvent::ImpactHeavy);
        }
        self.base_mut().emit_signal(
            signals::PLAYER_DAMAGED,
            &[Variant::from(amount)],
        );
    }

    /// Reset local state to match a fresh RunState (called by GameManager on new/continue).
    #[func]
    pub fn reset_loadout(&mut self) {
        self.loadout = Loadout::new();
        self.laser_level = LaserLevel::Red;
        self.power_mode = PowerMode::default();
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
