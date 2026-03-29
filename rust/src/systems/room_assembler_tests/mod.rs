use super::*;
use crate::systems::asset_catalog;
use crate::systems::room_template::*;

mod placement;
mod assembly;
mod corners;
mod theming;

/// Convenience wrapper: assemble with default Astra style.
fn assemble_default(
    template: &RoomTemplate,
    active_facings: &[ConnectorFacing],
    world_origin: [f32; 3],
    cell_size: f32,
) -> Vec<MeshPlacement> {
    assemble(template, active_facings, world_origin, cell_size, &RoomStyle::default())
}

fn small_room() -> RoomTemplate {
    RoomTemplate {
        id: "test_small",
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
        id: "test_corridor_ew",
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
        id: "test_large",
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
        id: "test_hub_6way",
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
        id: "test_3x3",
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

fn count_floors(placements: &[MeshPlacement], origin_y: f32) -> usize {
    placements.iter().filter(|p| {
        is_floor_scene(p.scene) && (p.position[1] - origin_y).abs() < 0.001
    }).count()
}

fn count_ceiling_tiles(placements: &[MeshPlacement], origin_y: f32, cell_height: f32) -> usize {
    placements.iter().filter(|p| {
        is_floor_scene(p.scene) && (p.position[1] - (origin_y + cell_height)).abs() < 0.001
    }).count()
}

/// Apply Godot Y-rotation to a point and return (x', z').
fn rotate_y(x: f32, z: f32, theta: f32) -> (f32, f32) {
    let (s, c) = theta.sin_cos();
    (x * c + z * s, -x * s + z * c)
}
