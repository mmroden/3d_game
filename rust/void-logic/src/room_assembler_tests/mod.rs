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
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ, frame: FrameStyle::Door },
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
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
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
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [1, 0, 0], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 1], facing: ConnectorFacing::PosZ, frame: FrameStyle::Door },
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
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosY, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegY, frame: FrameStyle::Door },
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
            Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [2, 0, 1], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
            Connector { offset: [1, 0, 0], facing: ConnectorFacing::NegZ, frame: FrameStyle::Door },
            Connector { offset: [1, 0, 2], facing: ConnectorFacing::PosZ, frame: FrameStyle::Door },
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

/// Completeness/assignment invariant: structural geometry is fixed, so
/// every placement `assemble` emits is `Collision::Static`. The Godot
/// shell turns each into a `StaticBody3D` with a mesh-derived collider —
/// no structural mesh can be emitted without a collider intent.
#[test]
fn structural_assembly_is_all_static() {
    let placements = assemble_default(&small_room(), &[], [0.0, 0.0, 0.0]);
    assert!(!placements.is_empty(), "a sealed room should emit geometry");
    for p in &placements {
        assert_eq!(
            p.collision,
            Collision::Static,
            "structural mesh {} must be Static, got {:?}",
            p.scene,
            p.collision,
        );
    }
}

/// The single shared prop classifier: loose debris tumbles (`Dynamic`),
/// anchored equipment stays fixed (`Static`).
#[test]
fn for_prop_classifies_loose_as_dynamic_and_anchored_as_static() {
    assert_eq!(
        Collision::for_prop("res://props/Prop_Crate1.gltf"),
        Collision::Dynamic,
    );
    assert_eq!(
        Collision::for_prop("res://props/Prop_Barrel_Large.gltf"),
        Collision::Dynamic,
    );
    // Free-standing furniture and installations float too, not just debris.
    assert_eq!(
        Collision::for_prop("res://props/Prop_Desk_Large.gltf"),
        Collision::Dynamic,
    );
    assert_eq!(
        Collision::for_prop("res://props/Prop_Pod.gltf"),
        Collision::Dynamic,
    );
    assert_eq!(
        Collision::for_prop("res://columns/Column_Astra.gltf"),
        Collision::Static,
    );
    assert_eq!(
        Collision::for_prop("res://props/Prop_Computer.gltf"),
        Collision::Static,
    );
}

/// The up/down corridor is a hollow shaft: it emits walls at every level but
/// NO floor or ceiling tiles. Its bottom (active NegY) and top (active PosY)
/// are open, and intermediate levels never get a slab — so nothing caps or
/// covers the vertical passage. Proven on a 3-tall shaft.
#[test]
fn vertical_corridor_is_a_hollow_walled_shaft() {
    let corridor = crate::spatial_layout::make_corridor(ConnectorFacing::PosY, 3);
    let active = corridor.connectors.clone();
    let placements = assemble_default(&corridor, &active, [0.0, 0.0, 0.0]);

    let floor_or_ceiling = placements.iter().filter(|p| is_floor_scene(p.scene)).count();
    assert_eq!(
        floor_or_ceiling, 0,
        "a vertical shaft must emit no floor/ceiling tiles, got {floor_or_ceiling}"
    );
    // Everything a frameless corridor emits besides floor/ceiling is wall
    // structure (a 2×2 shaft is all corner pieces), so a non-empty placement
    // list with zero slabs proves the shaft is walled but open top-and-bottom.
    assert!(
        !placements.is_empty(),
        "the shaft must still be walled at every level"
    );
}

/// A vertical shaft reads as a square right-angle tube: straight wall pieces
/// on every face, never the rounded corner pieces. (Rooms keep their curves —
/// see `rooms_keep_rounded_corners`.)
#[test]
fn vertical_shaft_uses_square_walls_not_round_corners() {
    let corridor = crate::spatial_layout::make_corridor(ConnectorFacing::PosY, 3);
    let active = corridor.connectors.clone();
    let placements = assemble_default(&corridor, &active, [0.0, 0.0, 0.0]);
    assert!(count(&placements, WALL) > 0, "shaft should use straight walls");
    assert_eq!(
        count(&placements, CORNER), 0,
        "shaft must not use rounded inner-corner pieces"
    );
    assert_eq!(
        count(&placements, CORNER_OUTER), 0,
        "shaft must not use rounded outer-corner pieces"
    );
}

/// Regression guard: ordinary rooms still use rounded corner pieces, so
/// squaring the shaft never flattened the rooms.
#[test]
fn rooms_keep_rounded_corners() {
    let placements = assemble_default(&small_room(), &[], [0.0, 0.0, 0.0]);
    assert!(
        count(&placements, CORNER) > 0,
        "a sealed room should keep its rounded corner pieces"
    );
}

/// A vertical connector opens a 2×2 cell (8×8 m) hole, not a single cell,
/// so the surrounding floor frames have room. A 4×4 room with one active
/// NegY connector centered at [1,0,1] omits the 2×2 block (cells 1..2 in
/// x and z) — 4 of 16 floor tiles — leaving a bordered hole.
#[test]
fn vertical_opening_removes_a_2x2_floor_block() {
    let template = RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![Connector {
            offset: [1, 0, 1],
            facing: ConnectorFacing::NegY,
            frame: FrameStyle::None,
        }],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [4, 1, 4],
    };
    let active = vec![template.connectors[0]];
    let placements = assemble_default(&template, &active, [0.0, 0.0, 0.0]);
    let floors = count_floors(&placements, 0.0);
    assert_eq!(
        floors, 12,
        "a 2×2 vertical opening should omit 4 of the 16 floor tiles, got {floors}"
    );
}

