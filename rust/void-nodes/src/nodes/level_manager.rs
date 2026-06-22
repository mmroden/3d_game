use godot::prelude::*;
use godot::classes::{
    CollisionShape3D, ConcavePolygonShape3D, MeshInstance3D, Node3D, INode3D, OmniLight3D,
    PackedScene, ResourceLoader, RigidBody3D, StaticBody3D,
};

use super::constants::{methods, nodes, scenes, signals};
use super::godot_util;
use super::live_handle::{LiveRef, LiveVec, LiveOpt};
use super::enemy_drone::EnemyDrone;
use super::ship_controller::ShipController;
use super::telemetry::Telemetry;
use super::views::ViewManager;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::seq::IndexedRandom;

use void_logic::generator::{generate, GeneratorConfig};
use void_logic::level_assembly::{self, RoomBounds};
use void_logic::level_graph::{LevelGraph, RENDER_ROOM_DEPTH};
use void_logic::room_furnisher::LightState;
use void_logic::room_assembler::{Collision, MeshPlacement};
use void_logic::portal as portal_sys;
use void_logic::enemy_type;
use void_logic::seed::Seed;

fn vec3(a: [f32; 3]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}

fn arr(v: Vector3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

/// Assembles a level on demand: generates the graph, assembles room
/// geometry from modular pieces, hands each object to Godot/Jolt with a
/// collider, and spawns lights. Generation is driven solely by
/// GameManager (one pathway); LevelManager never self-generates. Physics
/// is the engine's: after build, this node only culls rooms and flickers
/// lights.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct LevelManager {
    base: Base<Node3D>,

    #[export]
    grid_cell_size: f32,

    #[export]
    current_level: i32,

    telemetry: Telemetry,
    /// One container node per room (index = room-list order). Toggling
    /// a container's visibility culls that whole room — geometry and
    /// its lights — in one move.
    room_nodes: LiveVec<Node3D>,
    /// Per-room world bounds, parallel to `room_nodes`, for resolving
    /// which room a world point is in.
    room_bounds: Vec<RoomBounds>,
    /// The level's topology graph, retained for the level's lifetime — the
    /// authority for cull visibility (and, later, mapping and route
    /// queries). Empty until the first `generate_level`.
    level_graph: LevelGraph,
    /// The room the player currently occupies; culling only recomputes
    /// when this changes.
    current_room: Option<usize>,
    /// Cached player node, for reading position each tick.
    player: Option<LiveRef<Node3D>>,
    /// Blinking light fixtures and their full ("on") energy, modulated
    /// each frame so a flickering abandoned base reads as alive.
    blinking_lights: LiveVec<OmniLight3D, f32>,
    /// Accumulated time driving the blink phase.
    blink_time: f32,
}

/// Dim fixtures emit a fraction of their rated energy — a weak glow.
const DIM_ENERGY_FACTOR: f32 = 0.5;

#[godot_api]
impl INode3D for LevelManager {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            grid_cell_size: 4.0,
            current_level: 1_i32,
            telemetry: Telemetry::new(),
            room_nodes: LiveVec::new(),
            room_bounds: Vec::new(),
            level_graph: LevelGraph::default(),
            current_room: None,
            player: None,
            blinking_lights: LiveVec::new(),
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

    fn physics_process(&mut self, _delta: f64) {
        // Physics is the engine's. The host only culls rooms by the
        // player's current location (cheap point-in-AABB; only re-toggles
        // visibility when the player changes rooms) — and times that work.
        let started = std::time::Instant::now();
        let player_pos = self.player.with(|p| arr(p.get_global_position()));
        if let Some(pos) = player_pos {
            self.update_room_culling(pos);
        }
        self.telemetry
            .record_step_ms(started.elapsed().as_secs_f32() * 1000.0);
        let tick = godot::classes::Engine::singleton().get_physics_frames();
        self.telemetry.report(tick);
    }

    fn process(&mut self, delta: f64) {
        self.telemetry.record_frame(delta as f32 * 1000.0);
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

    /// World-space center of room `i` (midpoint of its bounds). Drives room
    /// culling from a known interior point. Returns `ZERO` for an out-of-range
    /// index. NOTE: the Y is the vertical *midpoint*, not the floor — use
    /// [`room_floor_center`](Self::room_floor_center) to place a camera.
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

    /// Horizontally-centered point at eye height above room `i`'s floor — where
    /// to park the camera for the loadout/briefing backdrop. Uses the floor
    /// (`min_y`), not the vertical midpoint, so the camera sits inside the room
    /// rather than up at the ceiling. `ZERO` for an out-of-range index.
    #[func]
    pub fn room_floor_center(&self, room: i64) -> Vector3 {
        match self.room_bounds.get(room as usize) {
            Some(b) => Vector3::new(
                (b.min[0] + b.max[0]) * 0.5,
                b.min[1] + 1.5,
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

    /// Test seam: how many blinking light fixtures the host is tracking. Lets a
    /// test confirm there are handles to strand before exercising the per-frame
    /// flicker against freed lights.
    #[func]
    pub fn blinking_light_count(&self) -> i64 {
        self.blinking_lights.live_count() as i64
    }

    /// Build the quiet one-room backdrop shown behind the loadout/briefing
    /// screens. Structure-only: the same generator with populace (props, loot,
    /// enemies, and the exit portal) skipped, so no live enemy fires at the
    /// parked player and no exit gate floats in the loadout room.
    #[func]
    pub fn generate_backdrop(&mut self, seed: i64) {
        self.build_level(seed, 1, true);
    }

    #[func]
    pub fn generate_level(&mut self, seed: i64, target_rooms: u32) {
        self.build_level(seed, target_rooms, false);
    }
}

impl LevelManager {
    /// The single level-build pathway. `structure_only` builds just the room
    /// shells + lights — used for the loadout/briefing backdrop, where populace
    /// would mean a live enemy firing at the parked player and the exit gate
    /// floating in the menu. The full build additionally furnishes props and
    /// containers, spawns enemies, and places the exit portal. Both drive the
    /// same per-room steps, so the backdrop room is identical to the one the
    /// player would fly through.
    fn build_level(&mut self, seed: i64, target_rooms: u32, structure_only: bool) {
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

        // Assemble each room's content, grouped into the three build steps the
        // shell mirrors: structure, then non-enemy inhabitants (props +
        // containers), then enemies.
        let rooms = level_assembly::spawn_list_full(&graph, self.grid_cell_size, seed);

        // Drop any room nodes from a previous level before rebuilding.
        self.room_nodes.for_each_live(|_, node, _| node.queue_free());
        self.room_nodes.clear();
        self.room_bounds.clear();
        self.current_room = None;
        // Blinking lights are children of the freed room nodes.
        self.blinking_lights.clear();

        let mut loader = ResourceLoader::singleton();
        let mut loose_rng = SmallRng::seed_from_u64(seed.value());
        let mut mesh_count = 0;
        let mut light_count = 0;
        let mut enemy_count = 0;

        // Enemy type selection scales with the level; one stream for the whole
        // level so the mix is deterministic per seed.
        let level = self.current_level;
        let available_enemies = enemy_type::enemies_for_level(level as u32);
        let mut enemy_rng = SmallRng::seed_from_u64(seed.value());

        // One container Node3D per room (identity transform; children keep world
        // positions). Hiding a container culls that room's geometry, lights, AND
        // inhabitants in a single visibility toggle.
        for room in &rooms {
            let mut room_node = Node3D::new_alloc();
            room_node.set_name(&format!("Room{}", self.room_nodes.len()));
            self.base_mut().add_child(&room_node);

            // --- Step 1: structure — the room's shell. Static pieces are
            // collected and fused into ONE merged collider per room instead of
            // a body per tile (keeps Jolt's broadphase and gen time sane).
            let mut static_meshes: Vec<Gd<Node3D>> = Vec::new();
            for entry in &room.structure {
                if Self::place_mesh(&mut loader, &mut room_node, entry, &mut static_meshes, &mut loose_rng) {
                    mesh_count += 1;
                }
            }
            Self::build_merged_collision(&mut room_node, &static_meshes);
            // Light fixtures. Most are dead in an abandoned base, so an Off
            // fixture gets no light node at all (the mesh stays, dark) — that
            // absence is the real GPU saving. Hidden with their room when culled.
            for ls in &room.lights {
                if ls.state == LightState::Off {
                    continue;
                }
                let energy = match ls.state {
                    LightState::Dim | LightState::Blinking => ls.energy * DIM_ENERGY_FACTOR,
                    LightState::On => ls.energy,
                    LightState::Off => ls.energy, // unreachable (Off skipped above)
                };
                let mut light = OmniLight3D::new_alloc();
                light.set_position(vec3(ls.position));
                light.set_param(godot::classes::light_3d::Param::RANGE, ls.range);
                light.set_param(godot::classes::light_3d::Param::ENERGY, energy);
                light.set_color(Color::from_rgb(ls.color[0], ls.color[1], ls.color[2]));
                room_node.add_child(&light);
                if ls.state == LightState::Blinking {
                    self.blinking_lights.push(&light, energy);
                }
                light_count += 1;
            }

            // Populace — props, loot containers, and enemies. Skipped entirely
            // for the structure-only backdrop so the loadout room stays quiet.
            if !structure_only {
                // --- Step 2: non-enemy inhabitants — furnished props (cell-rolled)
                // and organics containers (template loot spawns).
                for entry in &room.props {
                    if Self::place_mesh(&mut loader, &mut room_node, entry, &mut static_meshes, &mut loose_rng) {
                        mesh_count += 1;
                    }
                }
                for pos in &room.containers {
                    Self::spawn_organic_barrel(&mut loader, &mut room_node, *pos);
                }

                // --- Step 3: enemies — each a self-driving RigidBody3D parented
                // under the room so it culls/hides with it.
                for pos in &room.enemies {
                    let etype = *available_enemies.choose(&mut enemy_rng)
                        .expect("available_enemies is non-empty for any valid level");
                    if Self::spawn_enemy(&mut loader, &mut room_node, etype, level, *pos) {
                        enemy_count += 1;
                    }
                }
            }

            self.room_bounds.push(room.bounds.clone());
            self.room_nodes.push(&room_node, ());
        }

        // End-of-level portal: parent it under the room that contains it so it
        // culls and hides with that room like every other inhabitant. Skipped
        // for the structure-only backdrop (no exit gate behind the loadout).
        if !structure_only {
            if let Some(portal_pos) = portal_sys::portal_position(&graph, self.grid_cell_size) {
                if let Some(portal_scene) = loader.load(scenes::PORTAL) {
                    let packed: Gd<PackedScene> = portal_scene.cast();
                    if let Some(instance) = packed.instantiate() {
                        let mut node: Gd<Node3D> = instance.cast();
                        node.set_position(vec3(portal_pos));
                        let room_idx = self.room_bounds.iter().position(|b| b.contains(portal_pos));
                        match room_idx.and_then(|i| self.room_nodes.get_live(i)) {
                            Some(mut room_node) => room_node.add_child(&node),
                            None => self.base_mut().add_child(&node),
                        }
                        godot_print!("Portal spawned at ({}, {}, {})", portal_pos[0], portal_pos[1], portal_pos[2]);
                    }
                } else {
                    godot_warn!("Could not load portal.tscn");
                }
            }
        }

        // Place the player in the first room's center. The ship is a
        // RigidBody3D and drives its own motion; we only position it.
        let centers = level_assembly::cell_centers(&graph, self.grid_cell_size);
        if let Some(first_center) = centers.first() {
            let spawn = [first_center[0], first_center[1] + 1.5, first_center[2]];
            if let Some(parent) = self.base().get_parent() {
                if let Some(player) = parent.try_get_node_as::<ShipController>(nodes::PLAYER) {
                    let mut player_node = player.clone();
                    player_node.set_position(vec3(spawn));
                    player_node.reset_physics_interpolation();
                    self.player = Some(LiveRef::new(&player_node.upcast::<Node3D>()));
                    godot_print!("Player spawned at ({}, {}, {})", spawn[0], spawn[1], spawn[2]);
                }
            }
        }

        // Retain the graph for the level's lifetime: it's the authority for
        // cull visibility, and later for mapping and route queries. Moved in
        // last, after every generation step that borrowed it.
        self.level_graph = graph;

        godot_print!(
            "Level generated: {} rooms, {} meshes, {} lights, {} enemies{}",
            target_rooms,
            mesh_count,
            light_count,
            enemy_count,
            if structure_only { " (structure-only)" } else { "" },
        );
    }

    /// Free the collision bodies the glTF importer baked from `_col`/
    /// `_convcolonly` node suffixes, leaving a pure visual subtree. Our build
    /// is the single source of collision; freeing the body also frees its
    /// collision-shape children.
    fn strip_baked_colliders(node: &Gd<Node3D>) {
        let mut bodies: Vec<Gd<Node>> = Vec::new();
        Self::collect_static_bodies(&node.clone().upcast::<Node>(), &mut bodies);
        for body in bodies {
            body.free();
        }
    }

    fn collect_static_bodies(node: &Gd<Node>, out: &mut Vec<Gd<Node>>) {
        for child in node.get_children().iter_shared() {
            if child.clone().try_cast::<StaticBody3D>().is_ok() {
                out.push(child); // freeing it also frees its shape children
            } else {
                Self::collect_static_bodies(&child, out);
            }
        }
    }

    /// Fuse every structural mesh's triangles into one `ConcavePolygonShape3D`
    /// on a single `StaticBody3D` for the whole room. Same triangles as a
    /// per-mesh trimesh — so collision still hugs corners — but Jolt tracks
    /// one body instead of thousands. One source: the meshes.
    fn build_merged_collision(room_node: &mut Gd<Node3D>, static_meshes: &[Gd<Node3D>]) {
        let mut faces = PackedVector3Array::new();
        for node in static_meshes {
            Self::collect_faces(node, node.get_transform(), &mut faces);
        }
        if faces.is_empty() {
            return;
        }
        let mut shape = ConcavePolygonShape3D::new_gd();
        shape.set_faces(&faces);
        let mut col = CollisionShape3D::new_alloc();
        col.set_shape(&shape);
        let mut body = StaticBody3D::new_alloc();
        body.add_child(&col);
        room_node.add_child(&body);
    }

    /// Append every triangle of every `MeshInstance3D` under `node` to `faces`,
    /// transformed into the room's space by the accumulated `xform`.
    fn collect_faces(node: &Gd<Node3D>, xform: Transform3D, faces: &mut PackedVector3Array) {
        if let Ok(mesh_inst) = node.clone().try_cast::<MeshInstance3D>() {
            if let Some(mesh) = mesh_inst.get_mesh() {
                for v in mesh.get_faces().as_slice() {
                    faces.push(xform * *v);
                }
            }
        }
        for child in node.get_children().iter_shared() {
            if let Ok(child3d) = child.try_cast::<Node3D>() {
                let child_xform = xform * child3d.get_transform();
                Self::collect_faces(&child3d, child_xform, faces);
            }
        }
    }

    /// Give a dynamic body a convex collider per `MeshInstance3D` under
    /// `node`, each placed at the mesh's transform relative to the body so the
    /// hull hugs the rendered geometry. Convex (not trimesh) because Jolt
    /// allows concave shapes only on static bodies — one source: the mesh.
    /// Subscribe to ViewManager's active-viewport publication (so
    /// render measurement follows mode changes) and seed the current
    /// set immediately — ViewManager is the sole authority on which
    /// viewports draw the 3D world, so we never recompute that here.
    fn connect_render_viewports(&mut self) {
        let Some(main) = self.base().get_parent() else {
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
            return;
        };
        if self.current_room == Some(current) {
            return;
        }
        self.current_room = Some(current);
        // The current room-list index maps to the graph node at that
        // placement position; visibility is the graph's to decide.
        let Some(current_node) = self.level_graph.room_indices().nth(current) else {
            return;
        };
        let visible: std::collections::HashSet<usize> = self
            .level_graph
            .visible_from(current_node, RENDER_ROOM_DEPTH)
            .into_iter()
            .map(|n| n.index())
            .collect();
        // `for_each_live` skips any room freed out from under us and keeps the
        // index aligned with room_bounds — a freed handle is structurally
        // unreachable, so this can't use-after-free.
        self.room_nodes.for_each_live(|i, node, _| {
            node.set_visible(visible.contains(&i));
        });
    }

    /// Flicker blinking fixtures: each toggles between its full energy
    /// and dark on an out-of-phase cycle, so the base reads as alive
    /// rather than uniformly lit.
    fn update_blinking_lights(&mut self, delta: f32) {
        self.blink_time += delta;
        let t = self.blink_time;
        // `for_each_live` resolves each light by id and skips freed ones, so a
        // fixture freed out from under us (a regen, or GameManager clearing our
        // children) is a no-op rather than the use-after-free that panicked here
        // every frame.
        self.blinking_lights.for_each_live(|i, light, base| {
            let phase = i as f32 * 0.7;
            let lit = (t * 1.7 + phase).sin() > -0.55;
            light.set_param(godot::classes::light_3d::Param::ENERGY, if lit { *base } else { 0.0 });
        });
    }

    /// Instantiate an enemy scene and position it. It self-drives as a
    /// RigidBody3D; the engine owns its motion.
    fn spawn_enemy(
        loader: &mut Gd<ResourceLoader>,
        parent: &mut Gd<Node3D>,
        etype: enemy_type::EnemyType,
        level: i32,
        pos: [f32; 3],
    ) -> bool {
        let Some(scene_res) = loader.load(etype.scene_path()) else {
            return false;
        };
        let packed: Gd<PackedScene> = scene_res.cast();
        let Some(instance) = packed.instantiate() else {
            return false;
        };
        let Ok(mut enemy) = instance.try_cast::<EnemyDrone>() else {
            return false;
        };
        // Stamp type + level before entering the tree so ready() configures it.
        {
            let mut g = enemy.bind_mut();
            g.set_spawn_type(etype.id());
            g.set_spawn_level(level);
        }
        enemy.set_position(vec3(pos));
        // Parented under the room container (a cell inhabitant), so hiding or
        // culling the room takes its enemies with it.
        parent.add_child(&enemy);
        enemy.reset_physics_interpolation();
        true
    }

    /// Instantiate an organics barrel at `pos` under the given room container (a
    /// cell inhabitant). GameManager's spawned-entity scan recurses into room
    /// containers to connect its `organics_collected` signal.
    fn spawn_organic_barrel(loader: &mut Gd<ResourceLoader>, parent: &mut Gd<Node3D>, pos: [f32; 3]) -> bool {
        let Some(scene_res) = loader.load(scenes::ORGANIC_BARREL) else {
            return false;
        };
        let packed: Gd<PackedScene> = scene_res.cast();
        let Some(instance) = packed.instantiate() else {
            return false;
        };
        let mut node: Gd<Node3D> = instance.cast();
        node.set_position(vec3(pos));
        parent.add_child(&node);
        node.reset_physics_interpolation();
        true
    }

    /// Instantiate one placed mesh under `room_node`, applying its collision
    /// intent: Static renders and is collected into `statics` for the room's
    /// merged collider; Dynamic becomes a tumbling RigidBody3D with a convex
    /// hull; Passable just renders. Shared by the structure and inhabitant
    /// (prop) build steps so both honor the same collision contract.
    fn place_mesh(
        loader: &mut Gd<ResourceLoader>,
        room_node: &mut Gd<Node3D>,
        entry: &MeshPlacement,
        statics: &mut Vec<Gd<Node3D>>,
        loose_rng: &mut SmallRng,
    ) -> bool {
        let Some(resource) = loader.load(entry.scene) else {
            godot_warn!("Could not load: {}", entry.scene);
            return false;
        };
        let packed: Gd<PackedScene> = resource.cast();
        let Some(instance) = packed.instantiate() else {
            godot_warn!("Could not instantiate: {}", entry.scene);
            return false;
        };
        let mut node: Gd<Node3D> = instance.cast();
        // The asset pack bakes its own collision via `_convcolonly` node
        // suffixes; strip it so our build owns collision as the single source.
        Self::strip_baked_colliders(&node);

        match entry.collision {
            Collision::Dynamic => {
                node.set_position(Vector3::ZERO);
                use rand::RngExt;
                let rx: f32 = loose_rng.random_range(0.0..std::f32::consts::TAU);
                let ry: f32 = loose_rng.random_range(0.0..std::f32::consts::TAU);
                let rz: f32 = loose_rng.random_range(0.0..std::f32::consts::TAU);

                let mut body = RigidBody3D::new_alloc();
                body.set_position(vec3(entry.position));
                body.set_rotation(Vector3::new(rx, ry, rz));
                body.set_gravity_scale(0.0);

                body.add_child(&node);
                let node_xform = node.get_transform();
                godot_util::add_convex_collision(&mut body, &node, node_xform);
                room_node.add_child(&body);
                body.reset_physics_interpolation();
            }
            Collision::Static => {
                node.set_position(vec3(entry.position));
                if entry.rotation_x.abs() > 0.001 || entry.rotation_y.abs() > 0.001 {
                    node.set_rotation(Vector3::new(entry.rotation_x, entry.rotation_y, 0.0));
                }
                room_node.add_child(&node);
                statics.push(node);
            }
            Collision::Passable => {
                node.set_position(vec3(entry.position));
                if entry.rotation_x.abs() > 0.001 || entry.rotation_y.abs() > 0.001 {
                    node.set_rotation(Vector3::new(entry.rotation_x, entry.rotation_y, 0.0));
                }
                room_node.add_child(&node);
            }
        }
        true
    }
}
