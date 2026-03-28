use godot::prelude::*;
use godot::classes::{Node3D, INode3D};

use crate::systems::level_graph::LevelGraph;

/// Orchestrates level lifecycle: generates the graph, instantiates
/// room scenes, spawns enemies and loot.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct LevelManager {
    base: Base<Node3D>,

    #[export]
    grid_cell_size: f32,

    graph: Option<LevelGraph>,
}

#[godot_api]
impl INode3D for LevelManager {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            grid_cell_size: 10.0,
            graph: None,
        }
    }

    fn ready(&mut self) {
        // TODO: generate level graph, instantiate room scenes
        godot_print!("LevelManager ready — awaiting level generation");
    }
}

#[godot_api]
impl LevelManager {
    #[func]
    pub fn generate_level(&mut self, seed: u64) {
        let graph = LevelGraph::new();
        // TODO: use seed + room templates to populate the graph,
        // then iterate rooms and instantiate Godot scenes at
        // grid_pos * grid_cell_size world coordinates.
        godot_print!("Generating level with seed {seed}");
        self.graph = Some(graph);
    }

    #[func]
    pub fn room_count(&self) -> u32 {
        self.graph.as_ref().map_or(0, |g| g.room_count() as u32)
    }
}
