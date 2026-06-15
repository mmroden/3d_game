use godot::prelude::*;
use godot::prelude::EulerOrder;
use godot::classes::{
    RigidBody3D, IRigidBody3D, PackedScene, ResourceLoader, PhysicsRayQueryParameters3D,
    GpuParticles3D, ParticleProcessMaterial, SphereMesh, StandardMaterial3D,
    MeshInstance3D, BoxMesh, Node3D,
};

use super::constants::{groups, scenes, signals};
use super::enemy_bolt::EnemyBolt;
use super::godot_util;
use void_logic::enemy_ai::{DroneAi, DroneConfig, DroneState};
use void_logic::audio_catalog::SfxEvent;
use void_logic::enemy_type::EnemyType;
use void_logic::newtypes::{Health, Damage};

const BOLT_SPEED: f32 = 15.0;
/// Clearance so a newborn bolt spawns clear of the firer's own hull.
const MUZZLE_CLEARANCE: f32 = 0.6;
/// Engine damping (≈ -ln(0.05/s) retention). The chase force is scaled by
/// it so terminal cruise speed equals the AI's desired speed
/// (`v* = force / (mass · damp)`).
const ENEMY_LINEAR_DAMP: f32 = 3.0;

/// A hostile drone that chases and attacks the player. Motion and
/// collision are Godot/Jolt's (docs/architecture/physics_ownership.md):
/// the drone chases with force; the engine integrates, damps, and
/// resolves walls and impacts. We never set its velocity.
#[derive(GodotClass)]
#[class(base=RigidBody3D)]
pub struct EnemyDrone {
    base: Base<RigidBody3D>,

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
    player: Option<Gd<Node3D>>,
    health_bar_bg: Option<Gd<MeshInstance3D>>,
    health_bar_fill: Option<Gd<MeshInstance3D>>,
}

#[godot_api]
impl IRigidBody3D for EnemyDrone {
    fn init(base: Base<RigidBody3D>) -> Self {
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

        // Engine owns motion: zero-g; damping is the decay; rotation is
        // locked so impacts don't tumble the drone. We chase with force,
        // never by setting velocity.
        let mut base = self.base_mut();
        base.set_gravity_scale(0.0);
        base.set_linear_damp(ENEMY_LINEAR_DAMP);
        base.set_can_sleep(false);
        base.set_lock_rotation_enabled(true);
        base.add_to_group(groups::ENEMIES);
        drop(base);

        // Find player via group (the ship is a RigidBody3D; Node3D is
        // enough for position and group queries).
        let tree = self.base().get_tree();
        let players = tree.get_nodes_in_group(groups::PLAYER);
        if let Some(player_node) = players.get(0) {
            self.player = Some(player_node.cast::<Node3D>());
        }

        self.create_health_bar();
    }

    fn physics_process(&mut self, delta: f64) {
        if self.ai.is_dead() {
            return;
        }
        let Some(player) = self.player.clone() else { return };
        if !player.is_instance_valid() {
            return;
        }
        let my_pos = self.base().get_global_position();
        let player_pos = player.get_global_position();
        let has_sight = self.has_line_of_sight(my_pos, player_pos);
        let (desired, should_fire) = self.decide(has_sight, my_pos, player_pos, delta as f32);
        // Chase with force, not by setting velocity: scaled by damping so
        // terminal speed equals the desired cruise (v* = force / (mass·damp)).
        self.base_mut().apply_central_force(desired * ENEMY_LINEAR_DAMP);
        if should_fire {
            self.fire_bolt(my_pos, player_pos);
        }
        // Health-bar billboard is a transform write, so it belongs in the
        // physics tick (engine interpolation smooths it between frames).
        self.update_health_bar(player_pos);
    }
}

#[godot_api]
impl EnemyDrone {
    #[signal]
    fn enemy_killed(type_id: i32);

    /// Variant-boundary wrapper: the one f32→Damage conversion for
    /// GDScript and `Object::call` dispatch. Rust callers use
    /// `apply_damage`.
    #[func]
    pub fn take_damage(&mut self, amount: f32) {
        self.apply_damage(Damage::new(amount));
    }

    pub fn apply_damage(&mut self, damage: Damage) {
        let died = self.ai.take_damage(damage);
        self.update_health_bar_fill();

        if died {
            self.on_death();
        } else {
            self.spawn_hit_flash();
        }
    }

    /// Desired cruise velocity toward the player and whether to fire.
    fn decide(
        &mut self,
        has_sight: bool,
        my_pos: Vector3,
        player_pos: Vector3,
        delta: f32,
    ) -> (Vector3, bool) {
        if self.ai.is_dead() {
            return (Vector3::ZERO, false);
        }
        let distance = my_pos.distance_to(player_pos);
        let should_fire = self.ai.update(distance, has_sight, delta);
        if should_fire {
            if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
                audio.bind_mut().play_event_at(SfxEvent::EnemyFire, my_pos);
            }
        }

        const MIN_DISTANCE: f32 = 0.1;
        if distance <= MIN_DISTANCE {
            return (Vector3::ZERO, should_fire);
        }
        let direction = (player_pos - my_pos).normalized();
        let desired = match self.ai.state {
            DroneState::Chasing => direction * self.speed,
            DroneState::Attacking => direction * self.speed * 0.3,
            _ => Vector3::ZERO,
        };
        (desired, should_fire)
    }

    /// Clear sight to the player: cast a ray and confirm the first thing
    /// it hits is the player, not a wall or prop.
    fn has_line_of_sight(&self, from: Vector3, to: Vector3) -> bool {
        let Some(world) = self.base().get_world_3d() else { return false };
        let Some(mut space) = world.get_direct_space_state() else { return false };
        let Some(mut query) = PhysicsRayQueryParameters3D::create(from, to) else { return false };
        query.set_exclude(&array![self.base().get_rid()]);
        let result = space.intersect_ray(&query);
        if result.is_empty() {
            return true; // nothing between us
        }
        match result.get("collider") {
            Some(collider) => collider
                .to::<Gd<Node3D>>()
                .is_in_group(groups::PLAYER),
            None => true,
        }
    }

    fn fire_bolt(&mut self, my_pos: Vector3, player_pos: Vector3) {
        let Some(mut root) = godot_util::scene_root(self.base().get_tree()) else { return };
        let to_player = player_pos - my_pos;
        if to_player.length() < 0.01 {
            return; // colocated with the target — no firing direction
        }
        let dir = to_player.normalized();
        let muzzle = my_pos + dir * MUZZLE_CLEARANCE;
        let mut bolt = EnemyBolt::new_alloc();
        root.add_child(&bolt);
        bolt.bind_mut().launch(muzzle, dir * BOLT_SPEED, self.damage);
    }

    fn on_death(&mut self) {
        // Emit signal so GameManager can track the kill
        let type_id = self.enemy_type_id;
        self.base_mut().emit_signal(
            signals::ENEMY_KILLED,
            &[Variant::from(type_id)],
        );

        let pos = self.base().get_global_position();

        // Death explosion SFX
        if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
            audio.bind_mut().play_event_at(SfxEvent::ImpactHeavy, pos);
        }

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
        fire_mat.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
        fire_mat.set_emission(Color::from_rgba(1.0, 0.5, 0.0, 1.0));
        fire_mat.set_emission_energy_multiplier(10.0);
        sphere.set_material(&fire_mat);
        particles.set_draw_pass_mesh(0, &sphere);

        particles.set_transform(Transform3D::new(Basis::IDENTITY, pos));
        root.clone().add_child(&particles);
        particles.set_emitting(true);
        // Self-destruct after particles finish
        let mut timer = particles.get_tree().create_timer(1.0);
        let callable = particles.callable("queue_free");
        timer.connect("timeout", &callable);
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

            root.clone().add_child(&debris);
            // Self-destruct after 3 seconds
            let mut timer = debris.get_tree().create_timer(3.0);
            let callable = debris.callable("queue_free");
            timer.connect("timeout", &callable);
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
        fill_mat.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
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
        let diff = player_pos - my_pos;
        if diff.length() < 0.1 {
            return; // Too close — skip billboard update to avoid zero-vector normalization
        }
        let dir = diff.normalized();
        let bar_pos = my_pos + Vector3::new(0.0, 1.0, 0.0);

        // Face the player using a manual basis
        let forward = -dir;
        let up = Vector3::UP;
        let cross = up.cross(forward);
        if cross.length() < 0.001 {
            return; // Forward is parallel to up — can't compute billboard basis
        }
        let right = cross.normalized();
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
        spark_mat.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
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
