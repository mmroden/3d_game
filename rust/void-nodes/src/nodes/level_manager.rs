use godot::prelude::*;
use godot::classes::{BoxShape3D, CharacterBody3D, CollisionShape3D, Node3D, INode3D, OmniLight3D, PackedScene, Performance, ResourceLoader, SphereShape3D, StaticBody3D};

use super::constants::{groups, nodes, scenes};
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::seq::IndexedRandom;

use void_logic::generator::{generate, GeneratorConfig};
use void_logic::kinetic_world::{BallisticBody, KineticWorld, Mover, WorldSnapshot};
use void_logic::kinetics::Restitution;
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

/// Collision footprint for loose props (sphere, world-simulated).
const PROP_RADIUS: f32 = 0.5;
/// How lively loose props bounce; lossy, so disturbed rooms settle.
const PROP_RESTITUTION: f32 = 0.6;

fn vec3(a: [f32; 3]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}

fn arr(v: Vector3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

/// Fallback mover footprint when a body has no sphere collider.
const DEFAULT_MOVER_RADIUS: f32 = 0.7;

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
    /// View nodes for the world's bodies, indexed by BodyId.
    prop_nodes: Vec<Gd<Node3D>>,
    /// Landing zone: the last two published snapshots. The view
    /// interpolates between them at frame rate and never reads the
    /// world directly.
    previous: WorldSnapshot,
    current: WorldSnapshot,
    /// Per-tick physics-stage duration window; jitter (not the mean)
    /// is the SLO metric.
    step_timing: TimingWindow,
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
            prop_nodes: Vec::new(),
            previous: WorldSnapshot::default(),
            current: WorldSnapshot::default(),
            step_timing: TimingWindow::new(TIMING_WINDOW),
            monitors_registered: false,
        }
    }

    fn ready(&mut self) {
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
        let movers = self.gather_movers();
        self.world.collide_movers(&movers);
        // Contact events become consequences (ram damage, SFX) in M2;
        // drained every tick so onset semantics hold regardless.
        let _contacts = self.world.step(delta as f32);
        self.previous = std::mem::replace(&mut self.current, self.world.snapshot());
        self.step_timing
            .record(started.elapsed().as_secs_f32() * 1000.0);
    }

    fn process(&mut self, delta: f64) {
        let fraction =
            godot::classes::Engine::singleton().get_physics_interpolation_fraction() as f32;
        Self::sync_view(
            &self.previous,
            &self.current,
            fraction,
            delta as f32,
            &mut self.prop_nodes,
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
        self.prop_nodes.clear();
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
                self.world.add_body(BallisticBody::at_rest(
                    entry.position,
                    PROP_RADIUS,
                    Restitution::clamped(PROP_RESTITUTION),
                ));
                self.prop_nodes.push(node);
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

        // Spawn varied enemies based on current level
        let mut enemy_count = 0;
        let available_enemies = enemy_type::enemies_for_level(self.current_level as u32);
        let mut enemy_rng = SmallRng::seed_from_u64(seed.value());
        for pos in &enemy_positions {
            let etype = *available_enemies.choose(&mut enemy_rng)
                .expect("available_enemies is non-empty for any valid level");

            if let Some(scene_res) = loader.load(etype.scene_path()) {
                let packed: Gd<PackedScene> = scene_res.cast();
                if let Some(instance) = packed.instantiate() {
                    let mut node: Gd<Node3D> = instance.cast();
                    node.set_position(vec3(*pos));
                    self.base_mut().add_child(&node);
                    enemy_count += 1;
                }
            } else {
                // Fallback: try loading as the default drone scene
                if let Some(scene_res) = loader.load(scenes::ENEMY_DRONE_FALLBACK) {
                    let packed: Gd<PackedScene> = scene_res.cast();
                    if let Some(instance) = packed.instantiate() {
                        let mut node: Gd<Node3D> = instance.cast();
                        node.set_position(vec3(*pos));
                        self.base_mut().add_child(&node);
                        enemy_count += 1;
                    }
                }
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

        // Move player to the first room's center
        let centers = level_assembly::cell_centers(&graph, self.grid_cell_size);
        if let Some(first_center) = centers.first() {
            let spawn = Vector3::new(
                first_center[0],
                first_center[1] + 1.5,
                first_center[2],
            );
            if let Some(parent) = self.base().get_parent() {
                if let Some(mut player) = parent.try_get_node_as::<Node3D>(nodes::PLAYER) {
                    player.set_position(spawn);
                    godot_print!("Player spawned at ({}, {}, {})", spawn.x, spawn.y, spawn.z);
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
        nodes: &mut [Gd<Node3D>],
    ) {
        for body in &current.bodies {
            let index = body.id.index();
            let Some(node) = nodes.get_mut(index) else {
                continue;
            };
            if !node.is_instance_valid() {
                continue;
            }
            let prev = previous.bodies.get(index).copied().unwrap_or(*body);
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

    /// Mirror the powered movers (player + enemies) into the kinetic
    /// world for this tick so props can be shoved aside. The engine
    /// stays authoritative for the movers themselves.
    fn gather_movers(&self) -> Vec<Mover> {
        let mut movers = Vec::new();
        if let Some(parent) = self.base().get_parent() {
            if let Some(player) = parent.try_get_node_as::<CharacterBody3D>(nodes::PLAYER) {
                movers.push(Self::mover_from(&player));
            }
        }
        let tree = self.base().get_tree();
        for node in tree.get_nodes_in_group(groups::ENEMIES).iter_shared() {
            if let Ok(body) = node.try_cast::<CharacterBody3D>() {
                movers.push(Self::mover_from(&body));
            }
        }
        movers
    }

    fn mover_from(body: &Gd<CharacterBody3D>) -> Mover {
        Mover {
            position: arr(body.get_global_position()),
            velocity: arr(body.get_velocity()),
            radius: Self::collider_radius(body),
        }
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
