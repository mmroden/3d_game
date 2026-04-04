//! Portal position logic for level exit placement.

use crate::level_graph::LevelGraph;

/// Returns the world-space center of the farthest room from the start as the portal position.
/// The portal is placed at hover height (1.5m above floor).
pub fn portal_position(graph: &LevelGraph, cell_size: f32) -> Option<[f32; 3]> {
    let first_idx = graph.room_indices().next()?;
    let farthest_idx = graph.farthest_room_from(first_idx)?;
    let room = graph.room(farthest_idx)?;
    let story_height = crate::asset_catalog::WALL_SET_ASTRA.story_height;
    let origin = room.world_position(cell_size, story_height);
    let ex = room.template.extents[0] as f32;
    let ez = room.template.extents[2] as f32;
    Some([
        origin[0] + (ex * cell_size) / 2.0,
        origin[1] + 1.5, // hover height
        origin[2] + (ez * cell_size) / 2.0,
    ])
}

#[cfg(test)]
mod tests {
    // Portal tests will be rewritten once procedural generation is implemented.
}
