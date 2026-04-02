use godot::prelude::*;
use godot::prelude::EulerOrder;
use godot::classes::{
    CharacterBody3D, ICharacterBody3D, PackedScene, ResourceLoader,
    GpuParticles3D, ParticleProcessMaterial, SphereMesh, StandardMaterial3D,
    MeshInstance3D, BoxMesh,
};

use super::constants::{groups, meta_keys, scenes, signals};
use super::godot_util;
use void_logic::enemy_ai::{DroneAi, DroneConfig};
use void_logic::enemy_type::EnemyType;
use void_logic::newtypes::{Health, Damage};

/// A hostile drone that chases and attacks the player.
#[derive(GodotClass)]
#[class(base=CharacterBody3D)]
pub struct EnemyDrone {
    base: Base<CharacterBody3D>,

    #[export]
    enemy_type_id: i32,
    #[export]
    speed: f32,
    #[export]
    health: f32,
    #[export]
    detection_range: f32,
    #[export]
    attack_range: f32,
    #[export]
    damage: f32,

    ai: DroneAi,
    player: Option<Gd<CharacterBody3D>>,
    health_bar_bg: Option<Gd<MeshInstance3D>>,
    health_bar_fill: Option<Gd<MeshInstance3D>>,
}

#[godot_api]
impl ICharacterBody3D for EnemyDrone {
    fn init(base: Base<CharacterBody3D>) -> Self {
        let config = DroneConfig::default();
        let ai = DroneAi::new(config);
        Self {
            base,
            enemy_type_id: 1, // GunDrone by default
            speed: 8.0,
            health: 3.0,
            detection_range: 25.0,
            attack_range: 5.0,
            damage: 3.0,
            ai,
            player: None,
            health_bar_bg: None,
            health_bar_fill: None,
        }
    }

    fn ready(&mut self) {
        // Configure from EnemyType if valid, otherwise use exported values
        if let Some(enemy_type) = EnemyType::from_id(self.enemy_type_id) {
            let stats = enemy_type.stats();
            self.health = stats.hp.as_f32();
            self.speed = stats.speed;
            self.damage = stats.damage.as_f32();
            self.detection_range = stats.detection_range;
            self.attack_range = stats.attack_range;
            self.ai = DroneAi::new(DroneConfig {
                detection_range: stats.detection_range,
                attack_range: stats.attack_range,
                disengage_range: stats.detection_range * 1.2,
                health: stats.hp,
                attack_cooldown: stats.attack_cooldown,
            });
        } else {
            self.ai = DroneAi::new(DroneConfig {
                detection_range: self.detection_range,
                attack_range: self.attack_range,
                disengage_range: self.detection_range * 1.2,
                health: Health::new(self.health),
                attack_cooldown: 1.0,
            });
        }

        // Find player via group
        let tree = self.base().get_tree();
        let players = tree.get_nodes_in_group(groups::PLAYER);
        if let Some(player_node) = players.get(0) {
            let typed: Gd<CharacterBody3D> = player_node.cast();
            self.player = Some(typed);
        }

        self.create_health_bar();
    }

    fn physics_process(&mut self, delta: f64) {
        let delta = delta as f32;

        let Some(player) = &self.player else { return };
        if !player.is_instance_valid() {
            return;
        }

        let my_pos = self.base().get_global_position();
        let player_pos = player.get_global_position();
        let distance = my_pos.distance_to(player_pos);

        let should_fire = self.ai.update(distance, delta);

        match self.ai.state {
            void_logic::enemy_ai::DroneState::Chasing => {
                let direction = (player_pos - my_pos).normalized();
                let velocity = direction * self.speed;
                self.base_mut().set_velocity(velocity);
                self.base_mut().move_and_slide();
                self.base_mut().look_at(player_pos);
            }
            void_logic::enemy_ai::DroneState::Attacking => {
                self.base_mut().look_at(player_pos);
                let direction = (player_pos - my_pos).normalized();
                let velocity = direction * self.speed * 0.3;
                self.base_mut().set_velocity(velocity);
                self.base_mut().move_and_slide();
            }
            _ => {
                self.base_mut().set_velocity(Vector3::ZERO);
            }
        }

        if should_fire {
            self.fire_at_player(player_pos);
        }

        if self.ai.is_dead() {
            self.on_death();
        }

        // Billboard the health bar toward the player
        self.update_health_bar(player_pos);
    }
}

#[godot_api]
impl EnemyDrone {
    #[signal]
    fn enemy_killed(type_id: i32);

    #[func]
    pub fn take_damage(&mut self, amount: f32) {
        let died = self.ai.take_damage(Damage::new(amount));
        self.update_health_bar_fill();

        if !died {
            // Flash the model briefly to indicate hit
            self.spawn_hit_flash();
        }
    }

    fn fire_at_player(&self, _player_pos: Vector3) {
        godot_print!("Drone fires at player!");
        // TODO: enemy projectile or hitscan toward player
    }

    fn on_death(&mut self) {
        // Emit signal so GameManager can track the kill
        let type_id = self.enemy_type_id;
        self.base_mut().emit_signal(
            signals::ENEMY_KILLED,
            &[Variant::from(type_id)],
        );

        let pos = self.base().get_global_position();
        let Some(root) = godot_util::scene_root(self.base().get_tree()) else {
            self.base_mut().queue_free();
            return;
        };

        // Spawn explosion particles
        Self::spawn_explosion(&root, pos);

        // Spawn wreckage (small debris meshes that fall)
        Self::spawn_wreckage(&root, pos);

        // Spawn lootbox
        if let Some(scene) = ResourceLoader::singleton()
            .load(scenes::LOOTBOX)
        {
            let packed = scene.cast::<PackedScene>();
            if let Some(instance) = packed.instantiate() {
                let mut node: Gd<Node3D> = instance.cast();
                root.clone().add_child(&node);
                node.set_global_position(pos);
            }
        }

        self.base_mut().queue_free();
    }

    fn spawn_explosion(root: &Gd<Node>, pos: Vector3) {
        // Fire/orange burst
        let mut particles = GpuParticles3D::new_alloc();
        particles.set_amount(30);
        particles.set_lifetime(0.6);
        particles.set_one_shot(true);
        particles.set_explosiveness_ratio(0.9);

        let mat = godot_util::particle_burst_material(
            180.0,
            Color::from_rgba(1.0, 0.4, 0.05, 1.0),
            (4.0, 10.0),
            Some((0.5, 2.0)),
        );
        particles.set_process_material(&mat);

        let mut sphere = SphereMesh::new_gd();
        sphere.set_radius(0.04);
        sphere.set_height(0.08);
        let mut fire_mat = StandardMaterial3D::new_gd();
        fire_mat.set_albedo(Color::from_rgba(1.0, 0.3, 0.0, 1.0));
        fire_mat.set_emission(Color::from_rgba(1.0, 0.5, 0.0, 1.0));
        fire_mat.set_emission_energy_multiplier(10.0);
        sphere.set_material(&fire_mat);
        particles.set_draw_pass_mesh(0, &sphere);

        particles.set_transform(Transform3D::new(Basis::IDENTITY, pos));
        root.clone().add_child(&particles);
        particles.set_emitting(true);
    }

    fn spawn_wreckage(root: &Gd<Node>, pos: Vector3) {
        // A few small dark boxes as debris
        for i in 0..5 {
            let mut debris = MeshInstance3D::new_alloc();
            let mut box_mesh = BoxMesh::new_gd();
            let size = 0.05 + (i as f32) * 0.02;
            box_mesh.set_size(Vector3::new(size, size * 0.6, size * 1.2));
            debris.set_mesh(&box_mesh);

            let mut debris_mat = StandardMaterial3D::new_gd();
            debris_mat.set_albedo(Color::from_rgba(0.15, 0.15, 0.15, 1.0));
            debris.set_surface_override_material(0, &debris_mat);

            // Scatter positions slightly from explosion center
            let angle = (i as f32) * 1.2566; // ~72 degrees apart
            let offset = Vector3::new(angle.cos() * 0.3, -0.1, angle.sin() * 0.3);
            debris.set_transform(Transform3D::new(
                Basis::from_euler(EulerOrder::XYZ, Vector3::new(angle, angle * 0.7, 0.0)),
                pos + offset,
            ));

            // Tag for cleanup after a few seconds
            debris.set_meta(meta_keys::DEBRIS_TIMER, &Variant::from(3.0_f32));
            root.clone().add_child(&debris);
        }
    }

    fn create_health_bar(&mut self) {
        // Background (dark bar)
        let mut bg = MeshInstance3D::new_alloc();
        let mut bg_mesh = BoxMesh::new_gd();
        bg_mesh.set_size(Vector3::new(0.8, 0.08, 0.01));
        bg.set_mesh(&bg_mesh);
        let mut bg_mat = StandardMaterial3D::new_gd();
        bg_mat.set_albedo(Color::from_rgba(0.2, 0.2, 0.2, 0.8));
        bg_mat.set_transparency(godot::classes::base_material_3d::Transparency::ALPHA);
        bg.set_surface_override_material(0, &bg_mat);
        bg.set_position(Vector3::new(0.0, 1.0, 0.0));
        self.base_mut().add_child(&bg);
        self.health_bar_bg = Some(bg);

        // Fill (green/red bar)
        let mut fill = MeshInstance3D::new_alloc();
        let mut fill_mesh = BoxMesh::new_gd();
        fill_mesh.set_size(Vector3::new(0.76, 0.06, 0.015));
        fill.set_mesh(&fill_mesh);
        let mut fill_mat = StandardMaterial3D::new_gd();
        fill_mat.set_albedo(Color::from_rgba(0.1, 0.9, 0.1, 0.9));
        fill_mat.set_emission(Color::from_rgba(0.0, 0.5, 0.0, 1.0));
        fill_mat.set_emission_energy_multiplier(2.0);
        fill_mat.set_transparency(godot::classes::base_material_3d::Transparency::ALPHA);
        fill.set_surface_override_material(0, &fill_mat);
        fill.set_position(Vector3::new(0.0, 1.0, 0.005));
        self.base_mut().add_child(&fill);
        self.health_bar_fill = Some(fill);
    }

    fn update_health_bar(&mut self, player_pos: Vector3) {
        let my_pos = self.base().get_global_position();
        let dir = (player_pos - my_pos).normalized();
        let bar_pos = my_pos + Vector3::new(0.0, 1.0, 0.0);

        // Face the player using a manual basis
        let forward = -dir;
        let up = Vector3::UP;
        let right = up.cross(forward).normalized();
        let actual_up = forward.cross(right);
        let billboard_basis = Basis::from_cols(right, actual_up, forward);

        let bg_transform = Transform3D::new(billboard_basis, bar_pos);
        let fill_transform = Transform3D::new(billboard_basis, bar_pos + dir * -0.005);

        if let Some(bg) = &mut self.health_bar_bg {
            if bg.is_instance_valid() {
                bg.set_global_transform(bg_transform);
            }
        }
        if let Some(fill) = &mut self.health_bar_fill {
            if fill.is_instance_valid() {
                fill.set_global_transform(fill_transform);
            }
        }
    }

    fn update_health_bar_fill(&mut self) {
        let fraction = self.ai.health.fraction(self.ai.config.health);

        if let Some(fill) = &mut self.health_bar_fill {
            if !fill.is_instance_valid() {
                return;
            }
            // Scale X to represent remaining health
            let mut transform = fill.get_transform();
            transform.basis = transform.basis.scaled(Vector3::new(fraction.max(0.01), 1.0, 1.0));
            fill.set_transform(transform);

            // Color: green → yellow → red
            let color = if fraction > 0.5 {
                let t = (fraction - 0.5) * 2.0;
                Color::from_rgba(1.0 - t, 0.9, 0.1 * (1.0 - t), 0.9)
            } else {
                let t = fraction * 2.0;
                Color::from_rgba(0.9, t * 0.9, 0.0, 0.9)
            };

            if let Some(mat) = fill.get_surface_override_material(0) {
                let mut std_mat = mat.cast::<StandardMaterial3D>();
                std_mat.set_albedo(color);
            }
        }
    }

    fn spawn_hit_flash(&mut self) {
        // Small spark burst at drone position to indicate damage
        let pos = self.base().get_global_position();
        let mut particles = GpuParticles3D::new_alloc();
        particles.set_amount(6);
        particles.set_lifetime(0.2);
        particles.set_one_shot(true);
        particles.set_explosiveness_ratio(1.0);

        let mut mat = ParticleProcessMaterial::new_gd();
        mat.set_spread(90.0);
        mat.set_color(Color::from_rgba(1.0, 0.8, 0.3, 1.0));
        mat.set_gravity(Vector3::ZERO);
        mat.set_param_min(
            godot::classes::particle_process_material::Parameter::INITIAL_LINEAR_VELOCITY,
            2.0,
        );
        mat.set_param_max(
            godot::classes::particle_process_material::Parameter::INITIAL_LINEAR_VELOCITY,
            5.0,
        );
        particles.set_process_material(&mat);

        let mut sphere = SphereMesh::new_gd();
        sphere.set_radius(0.02);
        sphere.set_height(0.04);
        let mut spark_mat = StandardMaterial3D::new_gd();
        spark_mat.set_albedo(Color::from_rgba(1.0, 0.7, 0.2, 1.0));
        spark_mat.set_emission(Color::from_rgba(1.0, 0.6, 0.1, 1.0));
        spark_mat.set_emission_energy_multiplier(6.0);
        sphere.set_material(&spark_mat);
        particles.set_draw_pass_mesh(0, &sphere);

        particles.set_transform(Transform3D::new(Basis::IDENTITY, pos));
        if let Some(root) = godot_util::scene_root(self.base().get_tree()) {
            root.clone().add_child(&particles);
            particles.set_emitting(true);
        }
    }
}
