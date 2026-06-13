use godot::prelude::*;
use godot::classes::{BoxShape3D, CharacterBody3D, CollisionShape3D, Node3D, INode3D, OmniLight3D, PackedScene, Performance, RenderingServer, ResourceLoader, SphereMesh, SphereShape3D, StaticBody3D};

use super::constants::{nodes, scenes};
use super::enemy_drone::EnemyDrone;
use super::godot_util;
use super::ship_controller::ShipController;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::seq::IndexedRandom;

use void_logic::generator::{generate, GeneratorConfig};
use void_logic::audio_catalog::{SfxEvent, COLLISION_SFX_MIN_SPEED};
use void_logic::kinetic_world::{
    BallisticBody, BodyId, ContactEvent, ContactWith, KineticWorld, WorldSnapshot,
};
use void_logic::kinetics::{ControlInput, Mass, Restitution, Retention, SpeedLimits};
use void_logic::ram_damage;
use void_logic::level_assembly;
use void_logic::portal as portal_sys;
use void_logic::enemy_type;
use void_logic::seed::Seed;
use void_logic::timing::TimingWindow;

/// Custom monitor ids (graphed in the editor debugger's Monitors tab).
const MONITOR_P50: &str = "kinetics/step_ms_p50";
const MONITOR_P99: &str = "kinetics/step_ms_p99";
const MONITOR_JITTER: &str = "kinetics/step_ms_jitter";
/// Two seconds of samples at the 120 Hz tick.
const TIMING_WINDOW: usize = 240;
/// Print kinetics percentiles to the terminal every ~5 s of play.
const STATS_EVERY_TICKS: u64 = 600;

/// Collision footprint for loose props (sphere, world-simulated).
const PROP_RADIUS: f32 = 0.5;
/// How lively loose props bounce; lossy, so disturbed rooms settle.
const PROP_RESTITUTION: f32 = 0.6;
/// Enemy motion envelope: snappy stops (2% velocity retained per
/// second) so course changes feel deliberate, dull hull bounces.
const ENEMY_RETENTION_PER_SECOND: f32 = 0.02;
const ENEMY_RESTITUTION: f32 = 0.2;
/// Rough heft from collider size (kg per m³ of bounding sphere).
const ENEMY_MASS_PER_M3: f32 = 40.0;
/// The player's drone: heavy enough to shoulder props aside.
const SHIP_MASS_KG: f32 = 60.0;
/// How lively the hull bounces off level geometry.
const SHIP_HULL_RESTITUTION: f32 = 0.35;
/// Enemy bolts: small, fast ballistic bodies with a damage payload.
const BOLT_RADIUS: f32 = 0.15;
const BOLT_MASS_KG: f32 = 2.0;
const BOLT_SPEED: f32 = 15.0;
const BOLT_LIFETIME_S: f32 = 3.0;

fn vec3(a: [f32; 3]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}

fn arr(v: Vector3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

/// Fallback mover footprint when a body has no sphere collider.
const DEFAULT_MOVER_RADIUS: f32 = 0.7;

/// A live enemy bolt: world body, visual node, payload, remaining life.
struct BoltSlot {
    id: BodyId,
    node: Gd<Node3D>,
    damage: f32,
    age: f32,
}

/// Assembles a level on demand: generates the graph, assembles room
/// geometry from modular pieces, and spawns lights. Generation is
/// driven solely by GameManager (one pathway); LevelManager never
/// self-generates.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct LevelManager {
    base: Base<Node3D>,

    #[export]
    grid_cell_size: f32,

    #[export]
    current_level: i32,

    /// Representation of every ballistic mover (loose props): the
    /// world simulates trajectories; the nodes below only render them.
    world: KineticWorld,
    /// View nodes for the world's bodies, indexed by BodyId (`None`
    /// for tombstoned slots, e.g. dead enemies).
    body_nodes: Vec<Option<Gd<Node3D>>>,
    /// Live enemies: world body + the node whose AI is pulled each
    /// tick. Slots tombstone on death.
    enemies: Vec<Option<(BodyId, Gd<EnemyDrone>)>>,
    /// The player's world body + controller (registered per level).
    player_body: Option<(BodyId, Gd<ShipController>)>,
    /// Live enemy bolts: ballistic bodies with a damage payload and a
    /// lifetime. Slots tombstone on impact or expiry.
    bolts: Vec<Option<BoltSlot>>,
    /// Landing zone: the last two published snapshots. The view
    /// interpolates between them at frame rate and never reads the
    /// world directly.
    previous: WorldSnapshot,
    current: WorldSnapshot,
    /// Per-tick physics-stage duration window; jitter (not the mean)
    /// is the SLO metric.
    step_timing: TimingWindow,
    /// Per-rendered-frame delta window: the render-side stutter
    /// detector, measured identically to the physics stage.
    frame_timing: TimingWindow,
    /// Measured viewport render times: CPU submission and GPU
    /// completion ("when did drawing actually finish").
    render_cpu: TimingWindow,
    render_gpu: TimingWindow,
    viewport_rid: Rid,
    monitors_registered: bool,
}

#[godot_api]
impl INode3D for LevelManager {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            grid_cell_size: 4.0,
            current_level: 1_i32,
            world: KineticWorld::new(),
            body_nodes: Vec::new(),
            enemies: Vec::new(),
            player_body: None,
            bolts: Vec::new(),
            previous: WorldSnapshot::default(),
            current: WorldSnapshot::default(),
            step_timing: TimingWindow::new(TIMING_WINDOW),
            frame_timing: TimingWindow::new(TIMING_WINDOW),
            render_cpu: TimingWindow::new(TIMING_WINDOW),
            render_gpu: TimingWindow::new(TIMING_WINDOW),
            viewport_rid: Rid::Invalid,
            monitors_registered: false,
        }
    }

    fn ready(&mut self) {
        // Ask the renderer to measure this viewport's CPU and GPU
        // render times (mono path; SBS sub-viewports get their own
        // measurement when stereo profiling is needed).
        if let Some(viewport) = self.base().get_viewport() {
            self.viewport_rid = viewport.get_viewport_rid();
            RenderingServer::singleton().viewport_set_measure_render_time(self.viewport_rid, true);
        }
        let mut perf = Performance::singleton();
        if !perf.has_custom_monitor(MONITOR_P50) {
            let target = self.to_gd();
            perf.add_custom_monitor(
                MONITOR_P50,
                &Callable::from_object_method(&target, "step_ms_p50"),
            );
            perf.add_custom_monitor(
                MONITOR_P99,
                &Callable::from_object_method(&target, "step_ms_p99"),
            );
            perf.add_custom_monitor(
                MONITOR_JITTER,
                &Callable::from_object_method(&target, "step_ms_jitter"),
            );
            self.monitors_registered = true;
        }
    }

    fn exit_tree(&mut self) {
        if self.monitors_registered {
            let mut perf = Performance::singleton();
            perf.remove_custom_monitor(MONITOR_P50);
            perf.remove_custom_monitor(MONITOR_P99);
            perf.remove_custom_monitor(MONITOR_JITTER);
            self.monitors_registered = false;
        }
    }

    fn physics_process(&mut self, delta: f64) {
        let started = std::time::Instant::now();
        self.drive_player(delta as f32);
        self.drive_enemies(delta as f32);
        self.age_bolts(delta as f32);
        let contacts = self.world.step(delta as f32);
        for contact in &contacts {
            self.resolve_contact(contact);
        }
        self.previous = std::mem::replace(&mut self.current, self.world.snapshot());
        self.step_timing
            .record(started.elapsed().as_secs_f32() * 1000.0);

        // Terminal-visible instrumentation for the `make run` workflow
        // (the editor Monitors panel graphs the same counters).
        if self.current.tick > 0 && self.current.tick % STATS_EVERY_TICKS == 0 {
            let draw_calls = Performance::singleton()
                .get_monitor(godot::classes::performance::Monitor::RENDER_TOTAL_DRAW_CALLS_IN_FRAME);
            godot_print!(
                "kinetics: p50 {:.3} | p99 {:.3} | jit {:.3} || frame: p50 {:.2} | p99 {:.2} | jit {:.2} || draw cpu p99 {:.2} | gpu p99 {:.2} | calls {}",
                self.step_timing.percentile(50.0),
                self.step_timing.percentile(99.0),
                self.step_timing.jitter(),
                self.frame_timing.percentile(50.0),
                self.frame_timing.percentile(99.0),
                self.frame_timing.jitter(),
                self.render_cpu.percentile(99.0),
                self.render_gpu.percentile(99.0),
                draw_calls as i64,
            );
        }
    }

    fn process(&mut self, delta: f64) {
        self.frame_timing.record(delta as f32 * 1000.0);
        if self.viewport_rid != Rid::Invalid {
            let rs = RenderingServer::singleton();
            self.render_cpu
                .record(rs.viewport_get_measured_render_time_cpu(self.viewport_rid) as f32);
            self.render_gpu
                .record(rs.viewport_get_measured_render_time_gpu(self.viewport_rid) as f32);
        }
        let fraction =
            godot::classes::Engine::singleton().get_physics_interpolation_fraction() as f32;
        Self::sync_view(
            &self.previous,
            &self.current,
            fraction,
            delta as f32,
            &mut self.body_nodes,
        );
    }
}

#[godot_api]
impl LevelManager {
    #[func]
    pub fn step_ms_p50(&self) -> f64 {
        self.step_timing.percentile(50.0) as f64
    }

    #[func]
    pub fn step_ms_p99(&self) -> f64 {
        self.step_timing.percentile(99.0) as f64
    }

    #[func]
    pub fn step_ms_jitter(&self) -> f64 {
        self.step_timing.jitter() as f64
    }

    #[func]
    pub fn generate_level(&mut self, seed: i64, target_rooms: u32) {
        let seed = Seed::from_i64(seed);
        let config = GeneratorConfig {
            seed,
            max_rooms: if target_rooms == 0 { 0 } else { target_rooms as usize },
            min_room_xz: 3,
            max_room_xz: 6,
            min_room_y: 1,
            max_room_y: 6,
        };

        let graph = match generate(&config) {
            Ok(g) => g,
            Err(e) => {
                godot_error!("Level generation failed: {e:?}");
                return;
            }
        };

        // Assemble all room geometry, furniture, light fixtures, enemy positions,
        // and collision boxes for physics.
        let (placements, light_sources, enemy_positions, collision_boxes) =
            level_assembly::spawn_list_full(&graph, self.grid_cell_size, seed);

        // Fresh ballistic world for the new level, fed the same
        // collision boxes that become the Godot StaticBody3D colliders
        // below — one source of truth for where the walls are.
        self.world = KineticWorld::new();
        self.body_nodes.clear();
        self.enemies.clear();
        self.player_body = None;
        self.bolts.clear();
        self.previous = WorldSnapshot::default();
        self.current = WorldSnapshot::default();
        self.world.add_statics(collision_boxes.iter().copied());
        let mut loader = ResourceLoader::singleton();
        let mut mesh_count = 0;

        let mut loose_rng = SmallRng::seed_from_u64(seed.value());

        for entry in &placements {
            let scene_res = loader.load(entry.scene);
            let Some(resource) = scene_res else {
                godot_warn!("Could not load: {}", entry.scene);
                continue;
            };

            let packed: Gd<PackedScene> = resource.cast();
            let Some(instance) = packed.instantiate() else {
                godot_warn!("Could not instantiate: {}", entry.scene);
                continue;
            };

            if entry.loose {
                // Loose prop: a ballistic body in the kinetic world.
                // It spawns at rest (stillness is the abandoned-base
                // equilibrium); enemies and the player set it moving
                // by collision. The node is pure rendering.
                let mut node: Gd<Node3D> = instance.cast();
                node.set_position(vec3(entry.position));

                // Apply a random initial rotation for visual variety.
                use rand::RngExt;
                let rx: f32 = loose_rng.random_range(0.0..std::f32::consts::TAU);
                let ry: f32 = loose_rng.random_range(0.0..std::f32::consts::TAU);
                let rz: f32 = loose_rng.random_range(0.0..std::f32::consts::TAU);
                node.set_rotation(Vector3::new(rx, ry, rz));

                self.base_mut().add_child(&node);
                // One interpolation authority per node: the landing
                // zone lerps these at frame rate, so the engine's
                // physics interpolation must not smooth them again.
                node.set_physics_interpolation_mode(
                    godot::classes::node::PhysicsInterpolationMode::OFF,
                );
                self.world.add_body(BallisticBody::at_rest(
                    entry.position,
                    PROP_RADIUS,
                    Restitution::clamped(PROP_RESTITUTION),
                ));
                self.body_nodes.push(Some(node));
            } else {
                let mut node: Gd<Node3D> = instance.cast();
                let pos = vec3(entry.position);
                node.set_position(pos);

                if entry.rotation_x.abs() > 0.001 || entry.rotation_y.abs() > 0.001 {
                    node.set_rotation(Vector3::new(entry.rotation_x, entry.rotation_y, 0.0));
                }

                self.base_mut().add_child(&node);
            }
            mesh_count += 1;
        }

        // Spawn collision boxes (StaticBody3D + BoxShape3D) for walls/floors/ceilings.
        for cb in &collision_boxes {
            let mut body = StaticBody3D::new_alloc();
            body.set_position(vec3(cb.position));
            if cb.rotation_y.abs() > 0.001 {
                body.set_rotation(Vector3::new(0.0, cb.rotation_y, 0.0));
            }

            let mut box_shape = BoxShape3D::new_gd();
            box_shape.set_size(Vector3::new(
                cb.half_extents[0] * 2.0,
                cb.half_extents[1] * 2.0,
                cb.half_extents[2] * 2.0,
            ));

            let mut col_shape = CollisionShape3D::new_alloc();
            col_shape.set_shape(&box_shape);
            body.add_child(&col_shape);
            self.base_mut().add_child(&body);
        }

        // Spawn OmniLight3D for each light fixture
        for ls in &light_sources {
            let mut light = OmniLight3D::new_alloc();
            light.set_position(vec3(ls.position));
            light.set_param(godot::classes::light_3d::Param::RANGE, ls.range);
            light.set_param(godot::classes::light_3d::Param::ENERGY, ls.energy);
            light.set_color(Color::from_rgb(0.9, 0.95, 1.0));
            self.base_mut().add_child(&light);
        }

        // Spawn varied enemies based on current level; each becomes a
        // powered body in the kinetic world.
        let mut enemy_count = 0;
        let available_enemies = enemy_type::enemies_for_level(self.current_level as u32);
        let mut enemy_rng = SmallRng::seed_from_u64(seed.value());
        for pos in &enemy_positions {
            let etype = *available_enemies.choose(&mut enemy_rng)
                .expect("available_enemies is non-empty for any valid level");
            let speed = etype.stats().speed;
            let spawned = self.spawn_enemy(&mut loader, etype.scene_path(), speed, *pos)
                || self.spawn_enemy(&mut loader, scenes::ENEMY_DRONE_FALLBACK, speed, *pos);
            if spawned {
                enemy_count += 1;
            }
        }

        // Spawn end-of-level portal in the last room
        if let Some(portal_pos) = portal_sys::portal_position(&graph, self.grid_cell_size) {
            if let Some(portal_scene) = loader.load(scenes::PORTAL) {
                let packed: Gd<PackedScene> = portal_scene.cast();
                if let Some(instance) = packed.instantiate() {
                    let mut node: Gd<Node3D> = instance.cast();
                    node.set_position(vec3(portal_pos));
                    self.base_mut().add_child(&node);
                    godot_print!("Portal spawned at ({}, {}, {})", portal_pos[0], portal_pos[1], portal_pos[2]);
                }
            } else {
                godot_warn!("Could not load portal.tscn");
            }
        }

        // Place the player in the first room's center and register
        // them as a powered body — the whole player-physics pathway.
        let centers = level_assembly::cell_centers(&graph, self.grid_cell_size);
        if let Some(first_center) = centers.first() {
            let spawn = [first_center[0], first_center[1] + 1.5, first_center[2]];
            if let Some(parent) = self.base().get_parent() {
                if let Some(player) = parent.try_get_node_as::<ShipController>(nodes::PLAYER) {
                    let mut player_node = player.clone();
                    player_node.set_position(vec3(spawn));
                    // The landing zone owns this node's motion smoothing.
                    player_node.set_physics_interpolation_mode(
                        godot::classes::node::PhysicsInterpolationMode::OFF,
                    );
                    let radius =
                        Self::collider_radius(&player.clone().upcast::<CharacterBody3D>());
                    let (retention, limits) = player.bind().envelope();
                    let id = self.world.add_powered(
                        spawn,
                        radius,
                        Mass::kilograms(SHIP_MASS_KG),
                        Restitution::clamped(SHIP_HULL_RESTITUTION),
                        retention,
                        limits,
                    );
                    self.body_nodes.push(Some(player.clone().upcast()));
                    self.player_body = Some((id, player));
                    godot_print!(
                        "Player spawned at ({}, {}, {})",
                        spawn[0],
                        spawn[1],
                        spawn[2]
                    );
                }
            }
        }

        godot_print!(
            "Level generated: {} rooms, {} meshes, {} colliders, {} lights, {} enemies",
            target_rooms,
            mesh_count,
            collision_boxes.len(),
            light_sources.len(),
            enemy_count,
        );
    }
}



impl LevelManager {
    /// View sync: consumes snapshots only — rendering never reaches
    /// the world itself. Positions interpolate between the last two
    /// ticks (smooth at any display rate, ~one tick behind the sim);
    /// tumble integrates at frame rate, purely cosmetic.
    fn sync_view(
        previous: &WorldSnapshot,
        current: &WorldSnapshot,
        fraction: f32,
        frame_delta: f32,
        nodes: &mut [Option<Gd<Node3D>>],
    ) {
        for body in &current.bodies {
            let index = body.id.index();
            let Some(Some(node)) = nodes.get_mut(index) else {
                continue;
            };
            if !node.is_instance_valid() {
                continue;
            }
            // Snapshots compact on removal: correlate by id, never by
            // index (both vecs are id-sorted, so a binary search is
            // exact and cheap).
            let prev = previous
                .bodies
                .binary_search_by_key(&body.id.index(), |b| b.id.index())
                .ok()
                .map(|i| previous.bodies[i])
                .unwrap_or(*body);
            if body.at_rest && prev.position == body.position {
                continue;
            }
            let from = vec3(prev.position);
            let to = vec3(body.position);
            node.set_position(from.lerp(to, fraction));
            let spin = vec3(body.angular_velocity) * frame_delta;
            if spin != Vector3::ZERO {
                let rotation = node.get_rotation() + spin;
                node.set_rotation(rotation);
            }
        }
    }

    /// Drive every live enemy: cull dead ones from the world, ask the
    /// world for line of sight, pull the AI's desired velocity, and
    /// write its control. The world owns all enemy motion.
    fn drive_enemies(&mut self, delta: f32) {
        // The player's WORLD body position: physics decisions never
        // read render-interpolated node transforms.
        let player = self
            .player_body
            .clone()
            .and_then(|(pid, _)| self.world.body(pid).map(|b| (pid, vec3(b.position()))));

        let rate = Retention::decaying(ENEMY_RETENTION_PER_SECOND).cruise_thrust(1.0);
        for slot in 0..self.enemies.len() {
            let Some((id, enemy)) = self.enemies[slot].clone() else {
                continue;
            };
            if !enemy.is_instance_valid() {
                self.world.remove_body(id);
                self.enemies[slot] = None;
                if let Some(node_slot) = self.body_nodes.get_mut(id.index()) {
                    *node_slot = None;
                }
                continue;
            }
            let Some((player_id, player_pos)) = player else { continue };
            let Some(body) = self.world.body(id) else { continue };
            let body_pos = body.position();
            let body_radius = body.radius();
            // Exclude BOTH endpoints: the ray targets the player's
            // center, inside their own collider.
            let has_sight = !self
                .world
                .ray_blocked(body_pos, arr(player_pos), &[id, player_id]);
            let (desired, fire) =
                enemy
                    .clone()
                    .bind_mut()
                    .decide(has_sight, vec3(body_pos), player_pos, delta);
            self.world.set_control(
                id,
                ControlInput {
                    thrust: arr(desired * rate),
                    torque: [0.0; 3],
                },
            );
            if fire {
                let from = vec3(body_pos);
                let dir = (player_pos - from).normalized();
                let muzzle = from + dir * (body_radius + BOLT_RADIUS + 0.2);
                let damage = enemy.bind().bolt_damage();
                self.fire_bolt(arr(muzzle), dir, damage);
            }
        }
    }

    /// Spawn an enemy bolt: a ballistic world body with a damage
    /// payload, plus a small emissive visual the landing zone moves.
    fn fire_bolt(&mut self, from: [f32; 3], direction: Vector3, damage: f32) {
        let id = self.world.add_body(
            BallisticBody::at_rest(from, BOLT_RADIUS, Restitution::clamped(0.0))
                .with_mass(Mass::kilograms(BOLT_MASS_KG)),
        );
        self.world.disturb(
            id,
            void_logic::kinetics::Impulse {
                linear: arr(direction * BOLT_SPEED),
                angular: [0.0; 3],
            },
        );

        let mut visual = Node3D::new_alloc();
        let mut mesh = godot::classes::MeshInstance3D::new_alloc();
        let mut sphere = SphereMesh::new_gd();
        sphere.set_radius(BOLT_RADIUS);
        sphere.set_height(BOLT_RADIUS * 2.0);
        let mut material = godot::classes::StandardMaterial3D::new_gd();
        material.set_albedo(Color::from_rgb(1.0, 0.3, 0.1));
        material.set_feature(
            godot::classes::base_material_3d::Feature::EMISSION,
            true,
        );
        material.set_emission(Color::from_rgb(1.0, 0.4, 0.05));
        mesh.set_mesh(&sphere);
        mesh.set_surface_override_material(0, &material);
        visual.add_child(&mesh);
        visual.set_position(vec3(from));
        visual.set_physics_interpolation_mode(godot::classes::node::PhysicsInterpolationMode::OFF);
        self.base_mut().add_child(&visual);

        self.body_nodes.push(Some(visual.clone()));
        self.bolts.push(Some(BoltSlot {
            id,
            node: visual,
            damage,
            age: 0.0,
        }));
    }

    /// Expire bolts past their lifetime.
    fn age_bolts(&mut self, delta: f32) {
        for slot in 0..self.bolts.len() {
            let expired = match self.bolts[slot].as_mut() {
                Some(bolt) => {
                    bolt.age += delta;
                    bolt.age >= BOLT_LIFETIME_S
                }
                None => false,
            };
            if expired {
                self.despawn_bolt(slot);
            }
        }
    }

    fn despawn_bolt(&mut self, slot: usize) {
        if let Some(bolt) = self.bolts[slot].take() {
            self.world.remove_body(bolt.id);
            if let Some(node_slot) = self.body_nodes.get_mut(bolt.id.index()) {
                *node_slot = None;
            }
            let mut node = bolt.node;
            if node.is_instance_valid() {
                node.queue_free();
            }
        }
    }

    fn bolt_slot_by_id(&self, id: BodyId) -> Option<usize> {
        self.bolts
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|b| b.id == id))
    }

    /// Instantiate an enemy scene and register it as a powered body in
    /// the kinetic world (the node renders; the world moves).
    fn spawn_enemy(
        &mut self,
        loader: &mut Gd<ResourceLoader>,
        scene_path: &str,
        cruise_speed: f32,
        pos: [f32; 3],
    ) -> bool {
        let Some(scene_res) = loader.load(scene_path) else {
            return false;
        };
        let packed: Gd<PackedScene> = scene_res.cast();
        let Some(instance) = packed.instantiate() else {
            return false;
        };
        let Ok(mut enemy) = instance.try_cast::<EnemyDrone>() else {
            return false;
        };
        enemy.set_position(vec3(pos));
        self.base_mut().add_child(&enemy);
        // The landing zone owns this node's motion smoothing.
        enemy.set_physics_interpolation_mode(godot::classes::node::PhysicsInterpolationMode::OFF);

        let radius = Self::collider_radius(&enemy.clone().upcast::<CharacterBody3D>());
        let id = self.world.add_powered(
            pos,
            radius,
            Mass::kilograms(ENEMY_MASS_PER_M3 * radius.powi(3)),
            Restitution::clamped(ENEMY_RESTITUTION),
            Retention::decaying(ENEMY_RETENTION_PER_SECOND),
            SpeedLimits {
                linear: cruise_speed * 1.5,
                angular: 5.0,
            },
        );
        self.body_nodes.push(Some(enemy.clone().upcast()));
        self.enemies.push(Some((id, enemy)));
        true
    }

    /// Pull the ship's control and keep its envelope current (the
    /// player is a world body; this is the whole player-physics path).
    fn drive_player(&mut self, delta: f32) {
        let Some((id, ship)) = self.player_body.clone() else {
            return;
        };
        if !ship.is_instance_valid() {
            self.player_body = None;
            return;
        }
        let control = ship.clone().bind_mut().control_input(delta);
        let (retention, limits) = ship.bind().envelope();
        self.world.set_envelope(id, retention, limits);
        self.world.set_control(id, control);
    }

    /// Map a world contact to gameplay consequences: hull SFX on hard
    /// wall hits, symmetric ram damage on player-enemy collisions —
    /// one rule for both directions, replacing both old per-node ram
    /// loops.
    fn resolve_contact(&mut self, contact: &ContactEvent) {
        // Bolts detonate on any contact; their payload applies only to
        // the player. The body shove they impart (props!) is already
        // the world's doing.
        let bolt_hit = self
            .bolt_slot_by_id(contact.body)
            .map(|slot| (slot, contact.with))
            .or_else(|| match contact.with {
                ContactWith::Body(other) => self
                    .bolt_slot_by_id(other)
                    .map(|slot| (slot, ContactWith::Body(contact.body))),
                ContactWith::Static => None,
            });
        if let Some((slot, hit)) = bolt_hit {
            if let (Some((player_id, ship)), ContactWith::Body(other)) =
                (self.player_body.clone(), hit)
            {
                if other == player_id && ship.is_instance_valid() {
                    let damage = self.bolts[slot].as_ref().map(|b| b.damage).unwrap_or(0.0);
                    ship.clone().bind_mut().take_damage(damage);
                }
            }
            self.despawn_bolt(slot);
            return;
        }

        let Some((player_id, ship)) = self.player_body.clone() else {
            return;
        };
        if !ship.is_instance_valid() {
            return;
        }
        let involves_player =
            contact.body == player_id || contact.with == ContactWith::Body(player_id);
        if !involves_player {
            return;
        }

        if contact.with == ContactWith::Static {
            if contact.impact_speed > COLLISION_SFX_MIN_SPEED {
                if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
                    audio
                        .bind_mut()
                        .play_event_at(SfxEvent::ImpactMetal, vec3(contact.position));
                }
            }
            return;
        }

        let other = match contact.with {
            ContactWith::Body(b) if contact.body == player_id => b,
            _ => contact.body,
        };
        let Some(enemy) = self.enemy_by_id(other) else {
            // Prop contact: momentum exchange is consequence enough.
            return;
        };
        let dmg = ram_damage::ram_damage(contact.impact_speed);
        if dmg.as_f32() <= 0.0 {
            return;
        }
        enemy.clone().bind_mut().take_damage(dmg.as_f32());
        ship.clone()
            .bind_mut()
            .take_damage(dmg.as_f32() * ram_damage::PLAYER_RAM_FRACTION);
    }

    fn enemy_by_id(&self, id: BodyId) -> Option<Gd<EnemyDrone>> {
        self.enemies
            .iter()
            .flatten()
            .find(|(body, _)| *body == id)
            .map(|(_, enemy)| enemy.clone())
    }

    /// Read the mover's sphere collider radius; fall back to a rough
    /// footprint for non-sphere shapes.
    fn collider_radius(body: &Gd<CharacterBody3D>) -> f32 {
        for child in body.get_children().iter_shared() {
            let Ok(shape_node) = child.try_cast::<CollisionShape3D>() else {
                continue;
            };
            let Some(shape) = shape_node.get_shape() else {
                continue;
            };
            if let Ok(sphere) = shape.try_cast::<SphereShape3D>() {
                return sphere.get_radius();
            }
        }
        DEFAULT_MOVER_RADIUS
    }
}
