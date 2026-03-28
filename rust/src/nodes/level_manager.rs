use godot::prelude::*;
use godot::classes::{Node3D, INode3D, PackedScene, ResourceLoader};

use crate::systems::generator::{generate, GeneratorConfig};
use crate::systems::template_catalog;

/// Orchestrates level lifecycle: generates the graph, instantiates
/// room scenes, spawns enemies and loot.
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
            grid_cell_size: 10.0,
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

        let entries = template_catalog::spawn_list(&graph, self.grid_cell_size);
        let mut loader = ResourceLoader::singleton();

        for entry in &entries {
            let scene_res = loader.load(entry.scene);
            let Some(resource) = scene_res else {
                godot_warn!("Could not load scene: {}", entry.scene);
                continue;
            };

            let packed: Gd<PackedScene> = resource.cast();
            let Some(instance) = packed.instantiate() else {
                godot_warn!("Could not instantiate scene: {}", entry.scene);
                continue;
            };

            let mut node3d: Gd<Node3D> = instance.cast();
            let pos = Vector3::new(entry.world_pos[0], entry.world_pos[1], entry.world_pos[2]);
            node3d.set_position(pos);
            self.base_mut().add_child(&node3d);
        }

        // Move player to the first room's position
        if let Some(first_entry) = entries.first() {
            let spawn = Vector3::new(
                first_entry.world_pos[0],
                first_entry.world_pos[1] + self.grid_cell_size * 0.25,
                first_entry.world_pos[2],
            );
            if let Some(parent) = self.base().get_parent() {
                if let Some(mut player) = parent.try_get_node_as::<Node3D>("Player") {
                    player.set_position(spawn);
                    godot_print!("Player spawned at ({}, {}, {})", spawn.x, spawn.y, spawn.z);
                }
            }
        }

        godot_print!(
            "Level generated: {} rooms, {} total nodes",
            target_rooms,
            entries.len()
        );
    }
}
