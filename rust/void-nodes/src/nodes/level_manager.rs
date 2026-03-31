use godot::prelude::*;
use godot::classes::{Node3D, INode3D, OmniLight3D, PackedScene, ResourceLoader};
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::seq::IndexedRandom;

use void_logic::generator::{generate, GeneratorConfig};
use void_logic::template_catalog;
use void_logic::portal as portal_sys;
use void_logic::enemy_type;

fn vec3(a: [f32; 3]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}

/// Orchestrates level lifecycle: generates the graph, assembles
/// room geometry from modular pieces, and spawns lights.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct LevelManager {
    base: Base<Node3D>,

    #[export]
    grid_cell_size: f32,

    #[export]
    seed: i64,

    #[export]
    target_rooms: i32,

    #[export]
    current_level: i32,
}

#[godot_api]
impl INode3D for LevelManager {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            grid_cell_size: 4.0,
            seed: 42_i64,
            target_rooms: 5_i32,
            current_level: 1_i32,
        }
    }

    fn ready(&mut self) {
        let seed = self.seed;
        let target = self.target_rooms as u32;
        self.generate_level(seed, target);
    }
}

#[godot_api]
impl LevelManager {
    #[func]
    pub fn generate_level(&mut self, seed: i64, target_rooms: u32) {
        let config = GeneratorConfig {
            seed: seed as u64,
            room_templates: template_catalog::room_templates(),
            corridor_templates: template_catalog::corridor_templates(),
            target_room_count: target_rooms as usize,
        };

        let graph = match generate(&config) {
            Ok(g) => g,
            Err(e) => {
                godot_error!("Level generation failed: {e:?}");
                return;
            }
        };

        // Assemble all room geometry, furniture, light fixtures, and enemy positions
        let (placements, light_sources, enemy_positions) =
            template_catalog::spawn_list_full(&graph, self.grid_cell_size, seed as u64);
        let mut loader = ResourceLoader::singleton();
        let mut mesh_count = 0;

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

            let mut node: Gd<Node3D> = instance.cast();
            let pos = vec3(entry.position);
            node.set_position(pos);

            if entry.rotation_x.abs() > 0.001 || entry.rotation_y.abs() > 0.001 {
                node.set_rotation(Vector3::new(entry.rotation_x, entry.rotation_y, 0.0));
            }

            self.base_mut().add_child(&node);
            mesh_count += 1;
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
        let mut enemy_rng = SmallRng::seed_from_u64(seed as u64);
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
                if let Some(scene_res) = loader.load("res://scenes/enemies/enemy_drone.tscn") {
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
            if let Some(portal_scene) = loader.load("res://scenes/items/portal.tscn") {
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
        let centers = template_catalog::cell_centers(&graph, self.grid_cell_size);
        if let Some(first_center) = centers.first() {
            let spawn = Vector3::new(
                first_center[0],
                first_center[1] + 1.5,
                first_center[2],
            );
            if let Some(parent) = self.base().get_parent() {
                if let Some(mut player) = parent.try_get_node_as::<Node3D>("Player") {
                    player.set_position(spawn);
                    godot_print!("Player spawned at ({}, {}, {})", spawn.x, spawn.y, spawn.z);
                }
            }
        }

        godot_print!(
            "Level generated: {} rooms, {} meshes, {} lights, {} enemies",
            target_rooms,
            mesh_count,
            light_sources.len(),
            enemy_count,
        );
    }
}
