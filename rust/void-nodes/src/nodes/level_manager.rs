use godot::prelude::*;
use godot::classes::{BoxShape3D, CharacterBody3D, CollisionShape3D, Node3D, INode3D, OmniLight3D, PackedScene, ResourceLoader, SphereShape3D, StaticBody3D};

use super::constants::{groups, nodes, scenes};
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::seq::IndexedRandom;

use void_logic::generator::{generate, GeneratorConfig};
use void_logic::kinetic_world::{BallisticBody, KineticWorld, Mover};
use void_logic::kinetics::Restitution;
use void_logic::level_assembly;
use void_logic::portal as portal_sys;
use void_logic::enemy_type;
use void_logic::seed::Seed;

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
        }
    }

    fn physics_process(&mut self, delta: f64) {
        let movers = self.gather_movers();
        self.world.collide_movers(&movers);
        self.world.step(delta as f32);
        for (id, body) in self.world.bodies() {
            if body.is_at_rest() {
                continue;
            }
            let Some(node) = self.prop_nodes.get(id.index()) else {
                continue;
            };
            if !node.is_instance_valid() {
                continue;
            }
            let mut node = node.clone();
            node.set_position(vec3(body.position()));
            // Cosmetic tumble: colliders are spheres, so orientation
            // is a rendering concern integrated here in the view.
            let spin = vec3(body.angular_velocity()) * delta as f32;
            let rotation = node.get_rotation() + spin;
            node.set_rotation(rotation);
        }
    }
}

#[godot_api]
impl LevelManager {
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
