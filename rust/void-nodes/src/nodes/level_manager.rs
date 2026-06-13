use godot::prelude::*;
use godot::classes::{BoxShape3D, CharacterBody3D, CollisionShape3D, Node3D, INode3D, OmniLight3D, PackedScene, ResourceLoader, SphereShape3D, StaticBody3D};

use super::body_registry::BodyRegistry;
use super::constants::{methods, nodes, scenes, signals};
use super::enemy_drone::EnemyDrone;
use super::godot_util;
use super::ship_controller::ShipController;
use super::telemetry::Telemetry;
use super::views::ViewManager;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::seq::IndexedRandom;

use void_logic::generator::{generate, GeneratorConfig};
use void_logic::audio_catalog::SfxEvent;
use void_logic::consequence::{consequence_of, Consequence};
use void_logic::kinetic_world::{BallisticBody, ContactEvent, PoweredBodySpec, WorldSnapshot};
use void_logic::kinetics::{ControlInput, Mass, Restitution, Retention, SpeedLimits};
use void_logic::level_assembly::{self, RoomBounds};
use void_logic::room_furnisher::LightState;
use void_logic::portal as portal_sys;
use void_logic::enemy_type;
use void_logic::seed::Seed;

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

    /// The kinetic world and every body↔view binding, registered
    /// atomically — the world simulates, the bound nodes only render.
    registry: BodyRegistry,
    /// Landing zone: the last two published snapshots. The view
    /// interpolates between them at frame rate and never reads the
    /// world directly.
    previous: WorldSnapshot,
    current: WorldSnapshot,
    telemetry: Telemetry,
    /// One container node per room (index = room-list order). Toggling
    /// a container's visibility culls that whole room — geometry and
    /// its lights — in one move.
    room_nodes: Vec<Gd<Node3D>>,
    /// Per-room world bounds + adjacency, parallel to `room_nodes`,
    /// for deciding which rooms to draw.
    room_bounds: Vec<RoomBounds>,
    /// The room the player currently occupies; culling only recomputes
    /// when this changes.
    current_room: Option<usize>,
    /// Cached player node, for reading position each tick.
    player: Option<Gd<Node3D>>,
    /// Blinking light fixtures and their full ("on") energy, modulated
    /// each frame so a flickering abandoned base reads as alive.
    blinking_lights: Vec<(Gd<OmniLight3D>, f32)>,
    /// Accumulated time driving the blink phase.
    blink_time: f32,
}

/// Dim fixtures emit a fraction of their rated energy — a weak glow.
const DIM_ENERGY_FACTOR: f32 = 0.3;

#[godot_api]
impl INode3D for LevelManager {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            grid_cell_size: 4.0,
            current_level: 1_i32,
            registry: BodyRegistry::new(),
            previous: WorldSnapshot::default(),
            current: WorldSnapshot::default(),
            telemetry: Telemetry::new(),
            room_nodes: Vec::new(),
            room_bounds: Vec::new(),
            current_room: None,
            player: None,
            blinking_lights: Vec::new(),
            blink_time: 0.0,
        }
    }

    fn ready(&mut self) {
        self.connect_render_viewports();
        let target = self.to_gd();
        self.telemetry.register_monitors(
            Callable::from_object_method(&target, "step_ms_p50"),
            Callable::from_object_method(&target, "step_ms_p99"),
            Callable::from_object_method(&target, "step_ms_jitter"),
        );
    }

    fn exit_tree(&mut self) {
        self.telemetry.unregister_monitors();
    }

    fn physics_process(&mut self, delta: f64) {
        let started = std::time::Instant::now();
        self.drive_player(delta as f32);
        self.drive_enemies(delta as f32);
        self.registry.age_bolts(delta as f32);
        let contacts = self.registry.step(delta as f32);
        for contact in &contacts {
            self.resolve_contact(contact);
        }
        self.previous = std::mem::replace(&mut self.current, self.registry.snapshot());
        // Cull rooms by the player's current location (cheap point-in-AABB;
        // only re-toggles visibility when the player changes rooms).
        let player_pos = self.player.as_ref().map(|p| arr(p.get_global_position()));
        if let Some(pos) = player_pos {
            self.update_room_culling(pos);
        }
        self.telemetry
            .record_step_ms(started.elapsed().as_secs_f32() * 1000.0);
        self.telemetry.report(self.current.tick);
    }

    fn process(&mut self, delta: f64) {
        self.telemetry.record_frame(delta as f32 * 1000.0);
        let fraction =
            godot::classes::Engine::singleton().get_physics_interpolation_fraction() as f32;
        self.registry
            .sync_view(&self.previous, &self.current, fraction, delta as f32);
        self.update_blinking_lights(delta as f32);
    }
}

#[godot_api]
impl LevelManager {
    #[func]
    pub fn step_ms_p50(&self) -> f64 {
        self.telemetry.step_ms_p50() as f64
    }

    #[func]
    pub fn step_ms_p99(&self) -> f64 {
        self.telemetry.step_ms_p99() as f64
    }

    #[func]
    pub fn step_ms_jitter(&self) -> f64 {
        self.telemetry.step_ms_jitter() as f64
    }

    /// The viewport RIDs whose render time telemetry is measuring.
    /// Exposed for tests: in SBS this must be the two eye sub-viewports,
    /// not the root compositor.
    #[func]
    pub fn measured_viewport_rids(&self) -> Array<Rid> {
        self.telemetry.measured_viewports().into_iter().collect()
    }

    /// ViewManager republishes its active 3D viewports on every mode
    /// change; retarget render measurement onto them.
    #[func]
    fn on_render_viewports_changed(&mut self, viewports: Array<Rid>) {
        self.apply_measured_viewports(viewports);
    }

    /// Test seam: world-space center of room `i` (midpoint of its
    /// bounds), for driving culling from a known interior point.
    #[func]
    pub fn room_center(&self, room: i64) -> Vector3 {
        match self.room_bounds.get(room as usize) {
            Some(b) => Vector3::new(
                (b.min[0] + b.max[0]) * 0.5,
                (b.min[1] + b.max[1]) * 0.5,
                (b.min[2] + b.max[2]) * 0.5,
            ),
            None => Vector3::ZERO,
        }
    }

    /// Test seam: run room culling as if the player were at `point`.
    #[func]
    pub fn cull_for_position(&mut self, point: Vector3) {
        self.update_room_culling(arr(point));
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

        // Assemble all room geometry, furniture, light fixtures, enemy
        // positions, and collision boxes for physics, grouped per room.
        let rooms = level_assembly::spawn_list_full(&graph, self.grid_cell_size, seed);

        // Fresh registry (world + bindings) for the new level. The
        // kinetic world holds every wall, always — physics is never
        // culled, only rendering is. One source of truth for the walls:
        // these same collision boxes become the Godot StaticBody3D query
        // colliders below.
        self.registry = BodyRegistry::new();
        self.previous = WorldSnapshot::default();
        self.current = WorldSnapshot::default();
        self.registry
            .add_statics(rooms.iter().flat_map(|r| r.colliders.iter().copied()));

        // Drop any room nodes from a previous level before rebuilding.
        for mut node in std::mem::take(&mut self.room_nodes) {
            node.queue_free();
        }
        self.room_bounds.clear();
        self.current_room = None;
        // Blinking lights are children of the freed room nodes.
        self.blinking_lights.clear();

        let mut loader = ResourceLoader::singleton();
        let mut loose_rng = SmallRng::seed_from_u64(seed.value());
        let mut mesh_count = 0;
        let mut collider_count = 0;
        let mut light_count = 0;

        // One container Node3D per room (identity transform; children
        // keep world positions). Hiding a container culls that room's
        // geometry AND its lights in a single visibility toggle.
        for room in &rooms {
            let mut room_node = Node3D::new_alloc();
            room_node.set_name(&format!("Room{}", self.room_nodes.len()));
            self.base_mut().add_child(&room_node);

            for entry in &room.meshes {
                let Some(resource) = loader.load(entry.scene) else {
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

                    // Random initial rotation for visual variety.
                    use rand::RngExt;
                    let rx: f32 = loose_rng.random_range(0.0..std::f32::consts::TAU);
                    let ry: f32 = loose_rng.random_range(0.0..std::f32::consts::TAU);
                    let rz: f32 = loose_rng.random_range(0.0..std::f32::consts::TAU);
                    node.set_rotation(Vector3::new(rx, ry, rz));

                    room_node.add_child(&node);
                    // One interpolation authority per node: the landing
                    // zone lerps these at frame rate, so the engine's
                    // physics interpolation must not smooth them again.
                    node.set_physics_interpolation_mode(
                        godot::classes::node::PhysicsInterpolationMode::OFF,
                    );
                    self.registry.register_prop(
                        BallisticBody::at_rest(
                            entry.position,
                            PROP_RADIUS,
                            Restitution::clamped(PROP_RESTITUTION),
                        ),
                        node,
                    );
                } else {
                    let mut node: Gd<Node3D> = instance.cast();
                    node.set_position(vec3(entry.position));
                    if entry.rotation_x.abs() > 0.001 || entry.rotation_y.abs() > 0.001 {
                        node.set_rotation(Vector3::new(entry.rotation_x, entry.rotation_y, 0.0));
                    }
                    room_node.add_child(&node);
                }
                mesh_count += 1;
            }

            // Collision boxes (StaticBody3D + BoxShape3D). Parented under
            // the room node for organisation only — visibility never
            // disables collision, and wall collision is rapier-side.
            for cb in &room.colliders {
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
                room_node.add_child(&body);
                collider_count += 1;
            }

            // Light fixtures. Most are dead in an abandoned base, so an
            // Off fixture gets no light node at all (the mesh stays, dark)
            // — that absence is the real GPU saving. Hidden with their
            // room when culled.
            for ls in &room.lights {
                if ls.state == LightState::Off {
                    continue;
                }
                let energy = match ls.state {
                    LightState::Dim => ls.energy * DIM_ENERGY_FACTOR,
                    // On full; Blinking carries full energy and is
                    // modulated each frame.
                    _ => ls.energy,
                };
                let mut light = OmniLight3D::new_alloc();
                light.set_position(vec3(ls.position));
                light.set_param(godot::classes::light_3d::Param::RANGE, ls.range);
                light.set_param(godot::classes::light_3d::Param::ENERGY, energy);
                light.set_color(Color::from_rgb(ls.color[0], ls.color[1], ls.color[2]));
                room_node.add_child(&light);
                if ls.state == LightState::Blinking {
                    self.blinking_lights.push((light.clone(), energy));
                }
                light_count += 1;
            }

            self.room_bounds.push(room.bounds.clone());
            self.room_nodes.push(room_node);
        }

        let enemy_positions: Vec<[f32; 3]> = rooms
            .iter()
            .flat_map(|r| r.enemy_positions.iter().copied())
            .collect();

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
                    self.registry.register_player(
                        player,
                        PoweredBodySpec {
                            position: spawn,
                            radius,
                            mass: Mass::kilograms(SHIP_MASS_KG),
                            restitution: Restitution::clamped(SHIP_HULL_RESTITUTION),
                            retention,
                            limits,
                        },
                    );
                    // Cache for per-tick room culling (position source).
                    self.player = Some(player_node.upcast::<Node3D>());
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
            collider_count,
            light_count,
            enemy_count,
        );
    }
}



impl LevelManager {
    /// Subscribe to ViewManager's active-viewport publication (so
    /// render measurement follows mode changes) and seed the current
    /// set immediately — ViewManager is the sole authority on which
    /// viewports draw the 3D world, so we never recompute that here.
    fn connect_render_viewports(&mut self) {
        let Some(main) = self.base().get_parent() else {
            // Isolated instance (e.g. logic unit tests): no view pipeline
            // to measure. Telemetry stays idle; not an error condition.
            godot_print!("LevelManager: no parent; render telemetry idle");
            return;
        };
        let Some(mut view_mgr) = main.try_get_node_as::<ViewManager>(nodes::VIEW_MANAGER) else {
            godot_print!("LevelManager: no ViewManager sibling; render telemetry idle");
            return;
        };
        let callable = self.base().callable(methods::ON_RENDER_VIEWPORTS_CHANGED);
        if !view_mgr.is_connected(signals::RENDER_VIEWPORTS_CHANGED, &callable) {
            view_mgr.connect(signals::RENDER_VIEWPORTS_CHANGED, &callable);
        }
        // Pull the current set (mono → root) so the first frames measure
        // correctly without waiting for a mode toggle. Typed call against
        // ViewManager — no stringly-typed method name or Variant cast.
        let rids = view_mgr.bind().active_viewport_rids();
        self.apply_measured_viewports(rids);
    }

    fn apply_measured_viewports(&mut self, viewports: Array<Rid>) {
        let rids: Vec<Rid> = viewports.iter_shared().collect();
        self.telemetry.measure_viewports(&rids);
    }

    /// Show only the player's current room and its portal-neighbors;
    /// hide the rest. Recomputes only when the player changes rooms.
    fn update_room_culling(&mut self, player_pos: [f32; 3]) {
        let Some(current) = level_assembly::room_at(player_pos, &self.room_bounds) else {
            // Between rooms (a doorway/gap): keep the last visible set
            // rather than flickering the whole level off.
            return;
        };
        if self.current_room == Some(current) {
            return;
        }
        self.current_room = Some(current);
        let visible = level_assembly::visible_rooms(current, &self.room_bounds);
        for (i, node) in self.room_nodes.iter().enumerate() {
            node.clone().set_visible(visible.contains(&i));
        }
    }

    /// Flicker blinking fixtures: each toggles between its full energy
    /// and dark on an out-of-phase cycle, so the base reads as alive
    /// rather than uniformly lit. Culled rooms hide their lights anyway,
    /// so this only shows where it is seen.
    fn update_blinking_lights(&mut self, delta: f32) {
        self.blink_time += delta;
        let t = self.blink_time;
        for (i, (light, base)) in self.blinking_lights.iter().enumerate() {
            // Per-light phase offset spreads the flicker out.
            let phase = i as f32 * 0.7;
            let lit = (t * 6.0 + phase).sin() > 0.5;
            light
                .clone()
                .set_param(godot::classes::light_3d::Param::ENERGY, if lit { *base } else { 0.0 });
        }
    }

    /// Drive every live enemy: ask the world for line of sight, pull
    /// the AI's desired velocity, and write its control. The world
    /// owns all enemy motion.
    fn drive_enemies(&mut self, delta: f32) {
        self.registry.cull_dead_enemies();
        // The player's WORLD body position: physics decisions never
        // read render-interpolated node transforms.
        let player = self
            .registry
            .player()
            .and_then(|(pid, _)| self.registry.body_position(pid).map(|p| (pid, vec3(p))));

        let rate = Retention::decaying(ENEMY_RETENTION_PER_SECOND).cruise_thrust(1.0);
        let mut host: Gd<Node3D> = self.to_gd().upcast();
        for (id, enemy) in self.registry.live_enemies() {
            let Some((player_id, player_pos)) = player else {
                continue;
            };
            let Some(body_pos) = self.registry.body_position(id) else {
                continue;
            };
            // Exclude BOTH endpoints: the ray targets the player's
            // center, inside their own collider.
            let has_sight = !self
                .registry
                .ray_blocked(body_pos, arr(player_pos), &[id, player_id]);
            let (desired, fire) =
                enemy
                    .clone()
                    .bind_mut()
                    .decide(has_sight, vec3(body_pos), player_pos, delta);
            self.registry.set_control(
                id,
                ControlInput {
                    thrust: arr(desired * rate),
                    torque: [0.0; 3],
                },
            );
            if fire {
                let dir = (player_pos - vec3(body_pos)).normalized();
                let damage = enemy.bind().bolt_damage();
                self.registry.register_bolt(&mut host, id, dir, damage);
            }
        }
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
        self.registry.register_enemy(
            enemy,
            PoweredBodySpec {
                position: pos,
                radius,
                mass: Mass::kilograms(ENEMY_MASS_PER_M3 * radius.powi(3)),
                restitution: Restitution::clamped(ENEMY_RESTITUTION),
                retention: Retention::decaying(ENEMY_RETENTION_PER_SECOND),
                limits: SpeedLimits {
                    linear: cruise_speed * 1.5,
                    angular: 5.0,
                },
            },
        );
        true
    }

    /// Pull the ship's control and keep its envelope current (the
    /// player is a world body; this is the whole player-physics path).
    fn drive_player(&mut self, delta: f32) {
        let Some((id, ship)) = self.registry.player() else {
            return;
        };
        let control = ship.clone().bind_mut().control_input(delta);
        let (retention, limits) = ship.bind().envelope();
        self.registry.set_envelope(id, retention, limits);
        self.registry.set_control(id, control);
    }

    /// Apply a contact's gameplay effect. WHAT happens is decided by
    /// the pure consequence rules in void-logic; the registry
    /// classifies ids and this method executes the outcome.
    fn resolve_contact(&mut self, contact: &ContactEvent) {
        let consequence = consequence_of(contact, |id| self.registry.kind_of(id));
        match consequence {
            Consequence::None => {}
            Consequence::HullImpact { position } => {
                if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
                    audio
                        .bind_mut()
                        .play_event_at(SfxEvent::ImpactMetal, vec3(position));
                }
            }
            Consequence::BoltImpact {
                bolt,
                struck_player,
            } => {
                if struck_player {
                    if let (Some((_, ship)), Some(payload)) =
                        (self.registry.player(), self.registry.bolt_payload(bolt))
                    {
                        ship.clone().bind_mut().apply_damage(payload);
                    }
                }
                self.registry.despawn_bolt(bolt);
            }
            Consequence::Ram {
                enemy,
                enemy_damage,
                player_damage,
            } => {
                let (Some(enemy_node), Some((_, ship))) =
                    (self.registry.enemy(enemy), self.registry.player())
                else {
                    return;
                };
                enemy_node.clone().bind_mut().apply_damage(enemy_damage);
                ship.clone().bind_mut().apply_damage(player_damage);
            }
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
