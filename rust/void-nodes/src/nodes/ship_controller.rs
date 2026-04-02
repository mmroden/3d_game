use godot::prelude::*;
use godot::classes::{
    CharacterBody3D, ICharacterBody3D, PhysicsRayQueryParameters3D,
    MeshInstance3D,
    GpuParticles3D, SphereMesh, StandardMaterial3D,
    Input,
};

use super::constants::{actions, groups, meta_keys, methods};
use super::godot_util;

use void_logic::laser::LaserLevel;
use void_logic::loadout::Loadout;
use void_logic::upgrade::{Upgrade, UpgradeKind};
use void_logic::weapon::{WeaponState, FireResult};

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
        self.base_mut().add_to_group(groups::PLAYER);
    }

    fn physics_process(&mut self, delta: f64) {
        let delta = delta as f32;
        let input = Input::singleton();

        // --- Movement ---
        let forward = input.get_action_strength(actions::MOVE_FORWARD) - input.get_action_strength(actions::MOVE_BACK);
        let strafe = input.get_action_strength(actions::MOVE_RIGHT) - input.get_action_strength(actions::MOVE_LEFT);
        let vertical = input.get_action_strength(actions::MOVE_UP) - input.get_action_strength(actions::MOVE_DOWN);

        let pitch = input.get_action_strength(actions::LOOK_DOWN) - input.get_action_strength(actions::LOOK_UP);
        let yaw = input.get_action_strength(actions::LOOK_LEFT) - input.get_action_strength(actions::LOOK_RIGHT);
        let roll = input.get_action_strength(actions::ROLL_RIGHT) - input.get_action_strength(actions::ROLL_LEFT);

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
        spark_mat.set_emission(Color::from_rgba(1.0, 0.4, 0.1, 1.0));
        spark_mat.set_emission_energy_multiplier(8.0);
        sphere.set_material(&spark_mat);
        particles.set_draw_pass_mesh(0, &sphere);

        particles.set_transform(Transform3D::new(Basis::IDENTITY, position));
        if let Some(root) = godot_util::scene_root(self.base().get_tree()) {
            root.clone().add_child(&particles);
            particles.set_emitting(true);
            particles.set_meta(meta_keys::SPARK_TIMER, &Variant::from(0.5_f32));
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
