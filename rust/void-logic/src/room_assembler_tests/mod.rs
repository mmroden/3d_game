use super::*;
use crate::asset_catalog;
use crate::room_template::*;

mod placement;
mod assembly;
mod corners;
mod theming;

/// Convenience wrapper: assemble with default Astra wall set.
fn assemble_default(
    template: &RoomTemplate,
    active_connectors: &[Connector],
    world_origin: [f32; 3],
) -> Vec<MeshPlacement> {
    assemble(template, active_connectors, world_origin, &asset_catalog::WALL_SET_ASTRA)
}

fn small_room() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 1, 1],
    }
}

fn corridor_ew() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Corridor,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 1, 1],
    }
}

fn large_room() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            Connector { offset: [1, 0, 0], facing: ConnectorFacing::PosX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
            Connector { offset: [0, 0, 1], facing: ConnectorFacing::PosZ },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [2, 1, 2],
    }
}

fn count(placements: &[MeshPlacement], scene: &str) -> usize {
    placements.iter().filter(|p| p.scene == scene).count()
}

fn hub_6way() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosY },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegY },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 1, 1],
    }
}

fn room_3x3() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX },
            Connector { offset: [2, 0, 1], facing: ConnectorFacing::PosX },
            Connector { offset: [1, 0, 0], facing: ConnectorFacing::NegZ },
            Connector { offset: [1, 0, 2], facing: ConnectorFacing::PosZ },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [3, 1, 3],
    }
}

// Floor tiles may be either FLOOR (square) or FLOOR_CURVE (rounded corner).
fn is_floor_scene(scene: &str) -> bool {
    scene == FLOOR || scene == FLOOR_CURVE
}

/// Story height from the default (Astra) wall set.
const STORY_HEIGHT: f32 = 5.0;

fn count_floors(placements: &[MeshPlacement], origin_y: f32) -> usize {
    placements.iter().filter(|p| {
        is_floor_scene(p.scene) && (p.position[1] - origin_y).abs() < 0.001
    }).count()
}

fn count_ceiling_tiles(placements: &[MeshPlacement], origin_y: f32, story_height: f32) -> usize {
    placements.iter().filter(|p| {
        is_floor_scene(p.scene) && (p.position[1] - (origin_y + story_height)).abs() < 0.001
    }).count()
}

/// Apply Godot Y-rotation to a point and return (x', z').
fn rotate_y(x: f32, z: f32, theta: f32) -> (f32, f32) {
    let (s, c) = theta.sin_cos();
    (x * c + z * s, -x * s + z * c)
}

