use godot::prelude::*;
use godot::classes::{
    CharacterBody3D, ICharacterBody3D, PhysicsRayQueryParameters3D,
    MeshInstance3D,
    GpuParticles3D, SphereMesh, StandardMaterial3D,
    Input,
};

use super::constants::{actions, groups, methods, signals};
use super::godot_util;

use void_logic::kinetics::{AngularState, ControlInput, Retention, SpeedLimits};
use void_logic::laser::LaserLevel;
use void_logic::loadout::Loadout;
use void_logic::power_routing::PowerMode;
use void_logic::upgrade::{Upgrade, UpgradeKind};
use void_logic::audio_catalog::SfxEvent;
use void_logic::weapon::{WeaponState, FireResult};

/// Hard caps on ship velocity, enforced inside the kinetic core.
const SHIP_SPEED_LIMITS: SpeedLimits = SpeedLimits {
    linear: 50.0,
    angular: 5.0,
};

fn to_array(v: Vector3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

fn to_vector(a: [f32; 3]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
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

    /// Angular motion only: orientation is view/control-side (the
    /// camera is the ship). Linear motion lives in the kinetic world.
    angular: AngularState,
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
            angular: AngularState::new(),
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

}

#[godot_api]
impl ShipController {
    #[signal]
    fn player_damaged(amount: f32);

    #[signal]
    fn power_mode_changed(mode: i32);

    /// Per-tick control, pulled by the level host: reads input,
    /// integrates orientation locally (the camera is the ship — its
    /// rotation feel keeps the exact per-second math), fires weapons,
    /// and returns world-space thrust for the kinetic world, which
    /// owns all linear motion, wall response, and ram contacts.
    pub fn control_input(&mut self, delta: f32) -> ControlInput {
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

        // Stabilizer: kill angular velocity on L1/Tab
        if input.is_action_pressed(actions::STABILIZE) {
            self.angular.halt();
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
            self.base_mut()
                .emit_signal(signals::POWER_MODE_CHANGED, &[Variant::from(mode_val)]);
        }

        let basis = self.base().get_transform().basis;
        let thrust = basis.col_c() * (-forward) + basis.col_a() * strafe + basis.col_b() * vertical;

        let effective_thrust = self.loadout.thrust_power() * self.power_mode.thrust_multiplier();
        let effective_rotation = self.loadout.rotation_speed();

        // Angular integration stays local and exact.
        self.angular.step(
            to_array(Vector3::new(pitch, yaw, roll) * effective_rotation),
            self.loadout.damping(),
            SHIP_SPEED_LIMITS.angular,
            delta,
        );

        let rot = to_vector(self.angular.velocity()) * delta;

        // Quaternion-based rotation: compose pitch/yaw/roll independently
        // then apply to current orientation. Avoids gimbal lock and
        // frame-of-reference contamination from sequential rotate_x/y/z.
        let local_rotation = Quaternion::from_axis_angle(Vector3::RIGHT, rot.x)
            * Quaternion::from_axis_angle(Vector3::UP, rot.y)
            * Quaternion::from_axis_angle(Vector3::BACK, rot.z);
        let current_quat = self.base().get_quaternion();
        let new_quat = (current_quat * local_rotation).normalized();
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

        ControlInput {
            thrust: to_array(thrust * effective_thrust),
            torque: [0.0; 3],
        }
    }

    /// The ship's motion envelope for the kinetic world (retention
    /// changes with Stability upgrades mid-run).
    pub fn envelope(&self) -> (Retention, SpeedLimits) {
        (self.loadout.damping(), SHIP_SPEED_LIMITS)
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
