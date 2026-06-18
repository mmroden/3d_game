use super::*;
use crate::room_template::*;

// --- Test fixtures ---

fn room_1x1_east_west() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 1, 1],
    }
}

fn room_2x1_east_west() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [1, 0, 0], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [2, 1, 1],
    }
}

fn room_1x1_north_south() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ, frame: FrameStyle::Door },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 1, 1],
    }
}

fn corridor_1x1_east_west() -> RoomTemplate {
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

// --- Placement tests ---

#[test]
fn place_single_room() {
    let mut graph = LevelGraph::new();
    let idx = graph.place_room(room_1x1_east_west(), [0, 0, 0]);
    assert!(idx.is_ok());
    assert_eq!(graph.room_count(), 1);
}

#[test]
fn overlap_is_rejected() {
    let mut graph = LevelGraph::new();
    graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let result = graph.place_room(room_1x1_east_west(), [0, 0, 0]);
    assert!(result.is_err());
}

#[test]
fn multi_cell_room_occupies_all_cells() {
    let mut graph = LevelGraph::new();
    graph.place_room(room_2x1_east_west(), [0, 0, 0]).unwrap();
    assert!(!graph.is_free([0, 0, 0]));
    assert!(!graph.is_free([1, 0, 0]));
    assert!(graph.is_free([2, 0, 0]));
}

#[test]
fn partial_overlap_with_multi_cell_is_rejected() {
    let mut graph = LevelGraph::new();
    graph.place_room(room_2x1_east_west(), [0, 0, 0]).unwrap();
    let result = graph.place_room(room_1x1_east_west(), [1, 0, 0]);
    assert!(result.is_err());
}

#[test]
fn negative_coordinates_work() {
    let mut graph = LevelGraph::new();
    let idx = graph.place_room(room_1x1_east_west(), [-5, -3, -1]);
    assert!(idx.is_ok());
    assert!(!graph.is_free([-5, -3, -1]));
}

// --- KeyId tests ---

#[test]
fn key_id_display_matches_legacy_string() {
    assert_eq!(format!("{}", KeyId::Red), "red_key");
    assert_eq!(format!("{}", KeyId::Blue), "blue_key");
    assert_eq!(format!("{}", KeyId::Gold), "gold_key");
}

#[test]
fn key_id_roundtrips_through_display() {
    for key in [KeyId::Red, KeyId::Blue, KeyId::Gold] {
        let s = format!("{}", key);
        assert!(!s.is_empty());
    }
}

// --- Adjacent connection tests ---

#[test]
fn connect_adjacent_rooms_with_matching_connectors() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_1x1_east_west(), [1, 0, 0]).unwrap();
    let result = graph.connect_adjacent(a, b);
    assert!(result.is_ok());
    assert_eq!(graph.edge_count(), 1);
}

#[test]
fn connect_adjacent_fails_with_invalid_index() {
    let mut graph = LevelGraph::new();
    graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let bogus = NodeIndex::new(99);
    let result = graph.connect_adjacent(NodeIndex::new(0), bogus);
    assert!(matches!(result, Err(ConnectError::InvalidRoomIndex)));
}

#[test]
fn connect_adjacent_fails_when_no_matching_connectors() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_1x1_east_west(), [0, 0, 1]).unwrap();
    let result = graph.connect_adjacent(a, b);
    assert!(matches!(result, Err(ConnectError::NoMatchingConnectors)));
}

#[test]
fn connect_adjacent_non_adjacent_rooms_fails() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_1x1_east_west(), [5, 0, 0]).unwrap();
    let result = graph.connect_adjacent(a, b);
    assert!(matches!(result, Err(ConnectError::NoMatchingConnectors)));
}

#[test]
fn connect_adjacent_multi_cell_rooms_through_far_connector() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_2x1_east_west(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_1x1_east_west(), [2, 0, 0]).unwrap();
    let result = graph.connect_adjacent(a, b);
    assert!(result.is_ok());
}

#[test]
fn adjacent_edge_records_facings() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_1x1_east_west(), [1, 0, 0]).unwrap();
    graph.connect_adjacent(a, b).unwrap();

    let (_, _, edge) = graph.edges().next().unwrap();
    match edge {
        EdgeKind::Adjacent { from_connector, to_connector } => {
            assert_eq!(from_connector.facing, ConnectorFacing::PosX);
            assert_eq!(to_connector.facing, ConnectorFacing::NegX);
        }
        _ => panic!("Expected Adjacent edge"),
    }
}

// --- Corridor-as-node tests ---

#[test]
fn room_corridor_room_chain() {
    let mut graph = LevelGraph::new();
    let room_a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let corridor = graph.place_room(corridor_1x1_east_west(), [1, 0, 0]).unwrap();
    let room_b = graph.place_room(room_1x1_east_west(), [2, 0, 0]).unwrap();

    graph.connect_adjacent(room_a, corridor).unwrap();
    graph.connect_adjacent(corridor, room_b).unwrap();

    assert_eq!(graph.room_count(), 3);
    assert_eq!(graph.edge_count(), 2);
    assert!(graph.is_fully_connected());

    let corridor_neighbors: Vec<_> = graph.neighbors(corridor).collect();
    assert_eq!(corridor_neighbors.len(), 2);
    assert!(corridor_neighbors.contains(&room_a));
    assert!(corridor_neighbors.contains(&room_b));
}

#[test]
fn corridor_active_facings_has_both_directions() {
    let mut graph = LevelGraph::new();
    let room_a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let corridor = graph.place_room(corridor_1x1_east_west(), [1, 0, 0]).unwrap();
    let room_b = graph.place_room(room_1x1_east_west(), [2, 0, 0]).unwrap();

    graph.connect_adjacent(room_a, corridor).unwrap();
    graph.connect_adjacent(corridor, room_b).unwrap();

    let mut facings = graph.active_facings(corridor);
    facings.sort_by_key(|f| format!("{f:?}"));
    let mut expected = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];
    expected.sort_by_key(|f| format!("{f:?}"));
    assert_eq!(
        facings, expected,
        "corridor active_facings should be [NegX, PosX], got {facings:?}"
    );
}

#[test]
fn corridor_is_not_directly_connecting_distant_rooms() {
    let mut graph = LevelGraph::new();
    let room_a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let _corridor = graph.place_room(corridor_1x1_east_west(), [1, 0, 0]).unwrap();
    let room_b = graph.place_room(room_1x1_east_west(), [3, 0, 0]).unwrap();

    let result = graph.connect_adjacent(room_a, room_b);
    assert!(matches!(result, Err(ConnectError::NoMatchingConnectors)));
}

// --- Teleporter tests ---

#[test]
fn teleporter_connects_non_adjacent_rooms() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_1x1_east_west(), [100, 0, 0]).unwrap();
    assert!(graph.connect_adjacent(a, b).is_err());
    let result = graph.connect_teleporter(a, b);
    assert!(result.is_ok());
    assert_eq!(graph.edge_count(), 1);
}

#[test]
fn teleporter_connects_rooms_with_incompatible_connectors() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_1x1_north_south(), [0, 0, 1]).unwrap();
    assert!(graph.connect_adjacent(a, b).is_err());
    assert!(graph.connect_teleporter(a, b).is_ok());
}

#[test]
fn teleporter_fails_with_invalid_index() {
    let mut graph = LevelGraph::new();
    graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let bogus = NodeIndex::new(99);
    let result = graph.connect_teleporter(NodeIndex::new(0), bogus);
    assert!(matches!(result, Err(ConnectError::InvalidRoomIndex)));
}

#[test]
fn teleporter_edge_is_typed_correctly() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_1x1_east_west(), [50, 0, 0]).unwrap();
    graph.connect_teleporter(a, b).unwrap();

    let (_, _, edge) = graph.edges().next().unwrap();
    assert!(matches!(edge, EdgeKind::Teleporter));
}

#[test]
fn teleporter_counts_for_connectivity() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_1x1_east_west(), [50, 0, 0]).unwrap();
    assert!(!graph.is_fully_connected());
    graph.connect_teleporter(a, b).unwrap();
    assert!(graph.is_fully_connected());
}

// --- Locked door tests ---

#[test]
fn locked_door_requires_matching_connectors() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_1x1_east_west(), [5, 0, 0]).unwrap();
    let result = graph.connect_locked(a, b, KeyId::Red);
    assert!(matches!(result, Err(ConnectError::NoMatchingConnectors)));
}

#[test]
fn locked_door_works_with_adjacent_connectors() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_1x1_east_west(), [1, 0, 0]).unwrap();
    let result = graph.connect_locked(a, b, KeyId::Red);
    assert!(result.is_ok());

    let (_, _, edge) = graph.edges().next().unwrap();
    match edge {
        EdgeKind::Locked { key_id, .. } => assert_eq!(*key_id, KeyId::Red),
        _ => panic!("Expected Locked edge"),
    }
}

// --- Mixed edge connectivity tests ---

#[test]
fn mixed_adjacent_and_teleporter_connectivity() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_1x1_east_west(), [1, 0, 0]).unwrap();
    let c = graph.place_room(room_1x1_east_west(), [50, 0, 0]).unwrap();
    let d = graph.place_room(room_1x1_east_west(), [51, 0, 0]).unwrap();

    graph.connect_adjacent(a, b).unwrap();
    graph.connect_teleporter(b, c).unwrap();
    graph.connect_adjacent(c, d).unwrap();

    assert!(graph.is_fully_connected());
    assert_eq!(graph.edge_count(), 3);
}

// --- General connectivity tests ---

#[test]
fn empty_graph_is_connected() {
    let graph = LevelGraph::new();
    assert!(graph.is_fully_connected());
}

#[test]
fn single_room_is_connected() {
    let mut graph = LevelGraph::new();
    graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    assert!(graph.is_fully_connected());
}

#[test]
fn disconnected_rooms_are_not_fully_connected() {
    let mut graph = LevelGraph::new();
    graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    graph.place_room(room_1x1_east_west(), [5, 0, 0]).unwrap();
    assert!(!graph.is_fully_connected());
}

#[test]
fn mixed_axes_connection() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_1x1_north_south(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_1x1_north_south(), [0, 0, 1]).unwrap();
    let result = graph.connect_adjacent(a, b);
    assert!(result.is_ok());
}

#[test]
fn neighbors_returns_connected_rooms() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_1x1_east_west(), [1, 0, 0]).unwrap();
    let c = graph.place_room(room_1x1_east_west(), [2, 0, 0]).unwrap();
    graph.connect_adjacent(a, b).unwrap();
    graph.connect_adjacent(b, c).unwrap();

    let b_neighbors: Vec<_> = graph.neighbors(b).collect();
    assert_eq!(b_neighbors.len(), 2);
    assert!(b_neighbors.contains(&a));
    assert!(b_neighbors.contains(&c));

    let a_neighbors: Vec<_> = graph.neighbors(a).collect();
    assert_eq!(a_neighbors.len(), 1);
    assert!(a_neighbors.contains(&b));
}

// --- World position tests ---

#[test]
fn world_position_scales_grid_by_cell_size_and_cell_height() {
    let mut graph = LevelGraph::new();
    let idx = graph.place_room(room_1x1_east_west(), [3, -1, 2]).unwrap();
    let room = graph.room(idx).unwrap();
    let pos = room.world_position(10.0, 5.0);
    assert_eq!(pos, [30.0, -5.0, 20.0]);
}

// --- R2: Hub room with 6 cardinal connections ---

fn room_3x1x3_hub_6way() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [2, 0, 1], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
            Connector { offset: [1, 0, 0], facing: ConnectorFacing::NegZ, frame: FrameStyle::Door },
            Connector { offset: [1, 0, 2], facing: ConnectorFacing::PosZ, frame: FrameStyle::Door },
            Connector { offset: [1, 0, 1], facing: ConnectorFacing::PosY, frame: FrameStyle::Door },
            Connector { offset: [1, 0, 1], facing: ConnectorFacing::NegY, frame: FrameStyle::Door },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [3, 1, 3],
    }
}

fn room_1x1_vertical() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosY, frame: FrameStyle::Door },
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegY, frame: FrameStyle::Door },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [1, 1, 1],
    }
}

#[test]
fn hub_room_connects_six_neighbors() {
    let mut graph = LevelGraph::new();
    let hub = graph.place_room(room_3x1x3_hub_6way(), [0, 0, 0]).unwrap();

    let neg_x = graph.place_room(room_1x1_east_west(), [-1, 0, 1]).unwrap();
    let pos_x = graph.place_room(room_1x1_east_west(), [3, 0, 1]).unwrap();
    let neg_z = graph.place_room(room_1x1_north_south(), [1, 0, -1]).unwrap();
    let pos_z = graph.place_room(room_1x1_north_south(), [1, 0, 3]).unwrap();
    let pos_y = graph.place_room(room_1x1_vertical(), [1, 1, 1]).unwrap();
    let neg_y = graph.place_room(room_1x1_vertical(), [1, -1, 1]).unwrap();

    graph.connect_adjacent(hub, neg_x).unwrap();
    graph.connect_adjacent(hub, pos_x).unwrap();
    graph.connect_adjacent(hub, neg_z).unwrap();
    graph.connect_adjacent(hub, pos_z).unwrap();
    graph.connect_adjacent(hub, pos_y).unwrap();
    graph.connect_adjacent(hub, neg_y).unwrap();

    assert_eq!(graph.edge_count(), 6, "hub should have 6 edges");
    let neighbors: Vec<_> = graph.neighbors(hub).collect();
    assert_eq!(neighbors.len(), 6, "hub should have 6 neighbors");
    assert!(graph.is_fully_connected(), "all 7 rooms should be connected");
}

#[test]
fn hub_active_facings_returns_all_six() {
    let mut graph = LevelGraph::new();
    let hub = graph.place_room(room_3x1x3_hub_6way(), [0, 0, 0]).unwrap();

    let neg_x = graph.place_room(room_1x1_east_west(), [-1, 0, 1]).unwrap();
    let pos_x = graph.place_room(room_1x1_east_west(), [3, 0, 1]).unwrap();
    let neg_z = graph.place_room(room_1x1_north_south(), [1, 0, -1]).unwrap();
    let pos_z = graph.place_room(room_1x1_north_south(), [1, 0, 3]).unwrap();
    let pos_y = graph.place_room(room_1x1_vertical(), [1, 1, 1]).unwrap();
    let neg_y = graph.place_room(room_1x1_vertical(), [1, -1, 1]).unwrap();

    graph.connect_adjacent(hub, neg_x).unwrap();
    graph.connect_adjacent(hub, pos_x).unwrap();
    graph.connect_adjacent(hub, neg_z).unwrap();
    graph.connect_adjacent(hub, pos_z).unwrap();
    graph.connect_adjacent(hub, pos_y).unwrap();
    graph.connect_adjacent(hub, neg_y).unwrap();

    let mut facings = graph.active_facings(hub);
    facings.sort_by_key(|f| format!("{f:?}"));
    let mut expected = vec![
        ConnectorFacing::NegX, ConnectorFacing::PosX,
        ConnectorFacing::NegZ, ConnectorFacing::PosZ,
        ConnectorFacing::PosY, ConnectorFacing::NegY,
    ];
    expected.sort_by_key(|f| format!("{f:?}"));
    assert_eq!(facings, expected, "active_facings should return all 6 directions");
}

#[test]
fn vertical_adjacent_connection() {
    let mut graph = LevelGraph::new();
    let bottom = graph.place_room(room_1x1_vertical(), [0, 0, 0]).unwrap();
    let top = graph.place_room(room_1x1_vertical(), [0, 1, 0]).unwrap();
    graph.connect_adjacent(bottom, top).unwrap();

    let (_, _, edge) = graph.edges().next().unwrap();
    match edge {
        EdgeKind::Adjacent { from_connector, to_connector } => {
            assert_eq!(from_connector.facing, ConnectorFacing::PosY);
            assert_eq!(to_connector.facing, ConnectorFacing::NegY);
        }
        _ => panic!("Expected Adjacent edge"),
    }
}

// --- Aperture alignment: multi-Y connector tests ---

fn room_3x2x3_tall() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 1], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [2, 0, 1], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
            Connector { offset: [0, 1, 1], facing: ConnectorFacing::NegX, frame: FrameStyle::Door },
            Connector { offset: [2, 1, 1], facing: ConnectorFacing::PosX, frame: FrameStyle::Door },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [3, 2, 3],
    }
}

#[test]
fn connect_adjacent_wires_all_matching_y_levels() {
    let mut graph = LevelGraph::new();
    let a = graph.place_room(room_3x2x3_tall(), [0, 0, 0]).unwrap();
    let b = graph.place_room(room_3x2x3_tall(), [3, 0, 0]).unwrap();
    graph.connect_adjacent(a, b).unwrap();
    assert_eq!(graph.edge_count(), 2,
        "two matching connector pairs (y=0 and y=1) should produce 2 edges");
}

#[test]
fn tall_room_to_short_corridor_active_facings_precise() {
    let mut graph = LevelGraph::new();
    let tall = graph.place_room(room_3x2x3_tall(), [0, 0, 0]).unwrap();
    let corridor = graph.place_room(corridor_1x1_east_west(), [-1, 0, 1]).unwrap();
    graph.connect_adjacent(tall, corridor).unwrap();

    let active = graph.active_connectors(tall);
    assert!(active.iter().any(|c|
        c.facing == ConnectorFacing::NegX && c.offset == [0, 0, 1]),
        "should have y=0 NegX connector active");
    assert!(!active.iter().any(|c|
        c.facing == ConnectorFacing::NegX && c.offset == [0, 1, 1]),
        "y=1 NegX connector should NOT be active — nothing is wired there");
}

// --- R3: Multiple connections on the same face ---

fn room_3x1x3_multi_negz() -> RoomTemplate {
    RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegZ, frame: FrameStyle::Door },
            Connector { offset: [2, 0, 0], facing: ConnectorFacing::NegZ, frame: FrameStyle::Door },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [3, 1, 3],
    }
}

#[test]
fn large_room_multiple_connections_same_face() {
    let mut graph = LevelGraph::new();
    let room = graph.place_room(room_3x1x3_multi_negz(), [0, 0, 0]).unwrap();

    let n1 = graph.place_room(room_1x1_north_south(), [0, 0, -1]).unwrap();
    let n2 = graph.place_room(room_1x1_north_south(), [2, 0, -1]).unwrap();

    graph.connect_adjacent(room, n1).unwrap();
    graph.connect_adjacent(room, n2).unwrap();

    assert_eq!(graph.edge_count(), 2, "two connections on the same face");
    assert!(graph.is_fully_connected());
}

#[test]
fn farthest_room_from_linear_chain() {
    let mut graph = LevelGraph::new();
    let r0 = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let c0 = graph.place_room(corridor_1x1_east_west(), [1, 0, 0]).unwrap();
    graph.connect_adjacent(r0, c0).unwrap();
    let r1 = graph.place_room(room_1x1_east_west(), [2, 0, 0]).unwrap();
    graph.connect_adjacent(c0, r1).unwrap();
    let c1 = graph.place_room(corridor_1x1_east_west(), [3, 0, 0]).unwrap();
    graph.connect_adjacent(r1, c1).unwrap();
    let r2 = graph.place_room(room_1x1_east_west(), [4, 0, 0]).unwrap();
    graph.connect_adjacent(c1, r2).unwrap();
    let c2 = graph.place_room(corridor_1x1_east_west(), [5, 0, 0]).unwrap();
    graph.connect_adjacent(r2, c2).unwrap();
    let r3 = graph.place_room(room_1x1_east_west(), [6, 0, 0]).unwrap();
    graph.connect_adjacent(c2, r3).unwrap();

    let farthest = graph.farthest_room_from(r0);
    assert_eq!(farthest, Some(r3), "farthest room from r0 should be r3");
}


// --- Cull visibility (visible_from) ---

/// r0 —c0— r1 —c1— r2 —c2— r3, alternating room/corridor. Node indices
/// follow placement order: r0=0 c0=1 r1=2 c1=3 r2=4 c2=5 r3=6.
fn linear_chain() -> (LevelGraph, [NodeIndex; 7]) {
    let mut graph = LevelGraph::new();
    let r0 = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let c0 = graph.place_room(corridor_1x1_east_west(), [1, 0, 0]).unwrap();
    graph.connect_adjacent(r0, c0).unwrap();
    let r1 = graph.place_room(room_1x1_east_west(), [2, 0, 0]).unwrap();
    graph.connect_adjacent(c0, r1).unwrap();
    let c1 = graph.place_room(corridor_1x1_east_west(), [3, 0, 0]).unwrap();
    graph.connect_adjacent(r1, c1).unwrap();
    let r2 = graph.place_room(room_1x1_east_west(), [4, 0, 0]).unwrap();
    graph.connect_adjacent(c1, r2).unwrap();
    let c2 = graph.place_room(corridor_1x1_east_west(), [5, 0, 0]).unwrap();
    graph.connect_adjacent(r2, c2).unwrap();
    let r3 = graph.place_room(room_1x1_east_west(), [6, 0, 0]).unwrap();
    graph.connect_adjacent(c2, r3).unwrap();
    (graph, [r0, c0, r1, c1, r2, c2, r3])
}

fn visible_indices(graph: &LevelGraph, start: NodeIndex, budget: usize) -> Vec<usize> {
    let mut v: Vec<usize> = graph
        .visible_from(start, budget)
        .into_iter()
        .map(|n| n.index())
        .collect();
    v.sort_unstable();
    v
}

#[test]
fn visible_from_reaches_two_rooms_deep_through_corridors() {
    let (graph, n) = linear_chain();
    // From r0 at the shipped depth (2), sight passes through r1 and on to
    // r2 but stops before r3: r0, c0, r1, c1, r2 plus the corridor c2
    // leaving r2 (cost 2) are lit; r3 (cost 3) stays dark.
    assert_eq!(
        visible_indices(&graph, n[0], 2),
        vec![0, 1, 2, 3, 4, 5],
        "two rooms deep: current + r1 + r2 and their corridors"
    );
}

#[test]
fn visible_from_lights_one_more_room_per_budget_unit() {
    let (graph, n) = linear_chain();
    assert_eq!(
        visible_indices(&graph, n[0], 1),
        vec![0, 1, 2, 3],
        "budget 1: current room + the room one corridor away"
    );
    assert_eq!(
        visible_indices(&graph, n[0], 2),
        vec![0, 1, 2, 3, 4, 5],
        "budget 2: one room further"
    );
    assert_eq!(
        visible_indices(&graph, n[0], 3),
        vec![0, 1, 2, 3, 4, 5, 6],
        "budget 3: reaches r3 at the end of the chain"
    );
}

#[test]
fn visible_from_never_crosses_a_teleporter() {
    let mut graph = LevelGraph::new();
    let r0 = graph.place_room(room_1x1_east_west(), [0, 0, 0]).unwrap();
    let c0 = graph.place_room(corridor_1x1_east_west(), [1, 0, 0]).unwrap();
    graph.connect_adjacent(r0, c0).unwrap();
    let r1 = graph.place_room(room_1x1_east_west(), [2, 0, 0]).unwrap();
    graph.connect_adjacent(c0, r1).unwrap();
    // A teleporter to a room far across the map (not spatially adjacent).
    let far = graph.place_room(room_1x1_east_west(), [20, 0, 0]).unwrap();
    graph.connect_teleporter(r0, far).unwrap();

    let visible = visible_indices(&graph, r0, 9);
    assert!(
        !visible.contains(&far.index()),
        "the room on the far side of a teleporter is not in view"
    );
    assert!(
        visible.contains(&r1.index()),
        "rooms reachable by flying through corridors stay in view"
    );
}
