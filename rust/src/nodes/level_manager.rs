use godot::prelude::*;
use godot::classes::{Node3D, INode3D, OmniLight3D, PackedScene, ResourceLoader};

use crate::systems::generator::{generate, GeneratorConfig};
use crate::systems::template_catalog;

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
}

#[godot_api]
impl INode3D for LevelManager {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            grid_cell_size: 4.0,
            seed: 42_i64,
            target_rooms: 5_i32,
        }
    }

    fn ready(&mut self) {
        let seed = self.seed as u64;
        let target = self.target_rooms as u32;
        self.generate_level(seed, target);
    }
}

#[godot_api]
impl LevelManager {
    #[func]
    pub fn generate_level(&mut self, seed: u64, target_rooms: u32) {
        let config = GeneratorConfig {
            seed,
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

        // Assemble all room geometry, furniture, and light fixtures
        let (placements, light_sources) =
            template_catalog::spawn_list(&graph, self.grid_cell_size, seed);
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
            let pos = Vector3::new(entry.position[0], entry.position[1], entry.position[2]);
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
            light.set_position(Vector3::new(ls.position[0], ls.position[1], ls.position[2]));
            light.set_param(godot::classes::light_3d::Param::RANGE, ls.range);
            light.set_param(godot::classes::light_3d::Param::ENERGY, ls.energy);
            light.set_color(Color::from_rgb(0.9, 0.95, 1.0));
            self.base_mut().add_child(&light);
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
            "Level generated: {} rooms, {} meshes, {} lights",
            target_rooms,
            mesh_count,
            light_sources.len()
        );
    }
}
