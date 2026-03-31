use godot::prelude::*;
use godot::classes::{
    CharacterBody3D, ICharacterBody3D, PhysicsRayQueryParameters3D,
    MeshInstance3D, BoxMesh, StandardMaterial3D,
    GpuParticles3D, ParticleProcessMaterial, SphereMesh,
};

use crate::systems::laser::LaserLevel;
use crate::systems::loadout::Loadout;
use crate::systems::upgrade::{Upgrade, UpgradeKind};
use crate::systems::weapon::{WeaponState, FireResult};

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
}

#[godot_api]
impl ICharacterBody3D for ShipController {
    fn init(base: Base<CharacterBody3D>) -> Self {
        Self {
            base,
            thrust_power: 20.0,
            rotation_speed: 2.5,
            damping: 0.95,
            linear_velocity: Vector3::ZERO,
            angular_velocity: Vector3::ZERO,
            weapon: WeaponState::default(),
            beam_nodes: Vec::new(),
            loadout: Loadout::new(),
            laser_level: LaserLevel::Red,
        }
    }

    fn ready(&mut self) {
        self.base_mut().add_to_group("player");
    }

    fn physics_process(&mut self, delta: f64) {
        let delta = delta as f32;
        let input = Input::singleton();

        // --- Movement ---
        let forward = input.get_action_strength("move_forward") - input.get_action_strength("move_back");
        let strafe = input.get_action_strength("move_right") - input.get_action_strength("move_left");
        let vertical = input.get_action_strength("move_up") - input.get_action_strength("move_down");

        let pitch = input.get_action_strength("look_down") - input.get_action_strength("look_up");
        let yaw = input.get_action_strength("look_left") - input.get_action_strength("look_right");
        let roll = input.get_action_strength("roll_right") - input.get_action_strength("roll_left");

        let basis = self.base().get_transform().basis;
        let thrust = basis.col_c() * (-forward)
            + basis.col_a() * strafe
            + basis.col_b() * vertical;

        let effective_thrust = self.loadout.thrust_power();
        let effective_rotation = self.loadout.rotation_speed();
        let effective_damping = self.loadout.damping();

        self.linear_velocity += thrust * effective_thrust * delta;
        self.linear_velocity *= effective_damping;

        self.angular_velocity += Vector3::new(pitch, yaw, roll) * effective_rotation * delta;
        self.angular_velocity *= effective_damping;

        let vel = self.linear_velocity;
        let rot = self.angular_velocity * delta;

        self.base_mut().set_velocity(vel);
        self.base_mut().move_and_slide();
        self.base_mut().rotate_x(rot.x);
        self.base_mut().rotate_y(rot.y);
        self.base_mut().rotate_z(rot.z);

        // --- Weapon ---
        self.weapon.fire_rate = self.loadout.fire_rate();
        self.weapon.damage = self.laser_level.damage();
        self.weapon.tick(delta);
        self.age_beams(delta);

        if input.is_action_pressed("fire") {
            if let FireResult::Fired { damage } = self.weapon.try_fire() {
                self.fire_dual_lasers(damage);
            }
        }
    }
}

#[godot_api]
impl ShipController {
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
    }

    fn fire_single_ray(&mut self, origin: Vector3, forward: Vector3, damage: f32) {
        let end = origin + forward * self.weapon.max_range;

        let mut space = self.base().get_world_3d().unwrap().get_direct_space_state().unwrap();
        let mut query = PhysicsRayQueryParameters3D::create(origin, end).unwrap();
        let self_rid = self.base().get_rid();
        query.set_exclude(&array![self_rid]);

        let result = space.intersect_ray(&query);
        let hit_point = if result.is_empty() {
            end
        } else {
            let hit_pos = result.get("position").unwrap().to::<Vector3>();

            if let Some(collider) = result.get("collider") {
                let mut obj = collider.to::<Gd<Node3D>>();
                if obj.has_method("take_damage") {
                    obj.call("take_damage", &[Variant::from(damage)]);
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

        let mut mat = ParticleProcessMaterial::new_gd();
        mat.set_direction(normal);
        mat.set_spread(45.0);
        mat.set_color(Color::from_rgba(1.0, 0.6, 0.2, 1.0));
        mat.set_gravity(Vector3::ZERO);
        mat.set_param_min(
            godot::classes::particle_process_material::Parameter::INITIAL_LINEAR_VELOCITY,
            3.0,
        );
        mat.set_param_max(
            godot::classes::particle_process_material::Parameter::INITIAL_LINEAR_VELOCITY,
            6.0,
        );
        mat.set_param_min(
            godot::classes::particle_process_material::Parameter::SCALE,
            0.5,
        );
        mat.set_param_max(
            godot::classes::particle_process_material::Parameter::SCALE,
            1.0,
        );
        particles.set_process_material(&mat);

        // Small sphere as particle mesh
        let mut sphere = SphereMesh::new_gd();
        sphere.set_radius(0.015);
        sphere.set_height(0.03);
        let mut spark_mat = StandardMaterial3D::new_gd();
        spark_mat.set_albedo(Color::from_rgba(1.0, 0.5, 0.1, 1.0));
        spark_mat.set_emission(Color::from_rgba(1.0, 0.4, 0.1, 1.0));
        spark_mat.set_emission_energy_multiplier(8.0);
        sphere.set_material(&spark_mat);
        particles.set_draw_pass_mesh(0, &sphere);

        particles.set_transform(Transform3D::new(Basis::IDENTITY, position));
        self.base_mut().get_tree().unwrap().get_root().unwrap().add_child(&particles);
        particles.set_emitting(true);

        // Auto-free after lifetime
        particles.set_meta("spark_timer", &Variant::from(0.5_f32));
    }

    fn spawn_beam(&mut self, from: Vector3, to: Vector3) {
        let midpoint = (from + to) * 0.5;
        let length = from.distance_to(to);
        if length < 0.01 {
            return;
        }

        let mut mesh_instance = MeshInstance3D::new_alloc();

        let mut box_mesh = BoxMesh::new_gd();
        box_mesh.set_size(Vector3::new(0.02, 0.02, length));
        mesh_instance.set_mesh(&box_mesh);

        let mut material = StandardMaterial3D::new_gd();
        let c = self.laser_level.color();
        material.set_albedo(Color::from_rgba(c[0], c[1], c[2], 1.0));
        material.set_emission(Color::from_rgba(c[0], c[1], c[2], 1.0));
        material.set_emission_energy_multiplier(5.0);
        mesh_instance.set_surface_override_material(0, &material);

        mesh_instance.set_meta("beam_age", &Variant::from(0.0_f32));

        let dir = (to - from).normalized();
        let up = if dir.cross(Vector3::UP).length() > 0.001 {
            Vector3::UP
        } else {
            Vector3::RIGHT
        };
        let z_axis = -dir;
        let x_axis = up.cross(z_axis).normalized();
        let y_axis = z_axis.cross(x_axis);
        let beam_basis = Basis::from_cols(x_axis, y_axis, z_axis);
        let transform = Transform3D { basis: beam_basis, origin: midpoint };
        mesh_instance.set_transform(transform);

        let node = mesh_instance.clone();
        self.base_mut().get_tree().unwrap().get_root().unwrap().add_child(&mesh_instance);
        self.beam_nodes.push(node);
    }

    fn age_beams(&mut self, delta: f32) {
        const BEAM_LIFETIME: f32 = 0.08;

        self.beam_nodes.retain_mut(|beam| {
            if !beam.is_instance_valid() {
                return false;
            }
            let age = beam.get_meta("beam_age").to::<f32>() + delta;
            if age >= BEAM_LIFETIME {
                beam.queue_free();
                false
            } else {
                beam.set_meta("beam_age", &Variant::from(age));
                let alpha = 1.0 - (age / BEAM_LIFETIME);
                if let Some(mat) = beam.get_surface_override_material(0) {
                    let mut std_mat = mat.cast::<StandardMaterial3D>();
                    let bc = self.laser_level.color();
                    std_mat.set_albedo(Color::from_rgba(bc[0], bc[1], bc[2], alpha));
                }
                true
            }
        });
    }
}
