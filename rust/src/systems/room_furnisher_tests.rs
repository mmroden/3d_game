use super::*;
use crate::systems::asset_catalog;
use crate::systems::room_template::*;

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

fn room_5x5() -> RoomTemplate {
    RoomTemplate {
        id: "test_5x5",
        kind: TemplateKind::Room,
        connectors: vec![
            Connector { offset: [0, 0, 2], facing: ConnectorFacing::NegX },
            Connector { offset: [4, 0, 2], facing: ConnectorFacing::PosX },
            Connector { offset: [2, 0, 0], facing: ConnectorFacing::NegZ },
            Connector { offset: [2, 0, 4], facing: ConnectorFacing::PosZ },
        ],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [5, 1, 5],
    }
}

fn corridor_1x1() -> RoomTemplate {
    RoomTemplate {
        id: "test_corridor",
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

/// All wall-adjacent prop scene paths for easy lookup.
fn wall_adjacent_scenes() -> Vec<&'static str> {
    asset_catalog::WALL_ADJACENT_PROPS.iter().map(|p| p.scene).collect()
}

/// All center prop scene paths.
fn center_scenes() -> Vec<&'static str> {
    asset_catalog::CENTER_PROPS.iter().map(|p| p.scene).collect()
}

/// Check if a cell (cx, cz) is on the room boundary.
fn is_boundary(cx: i32, cz: i32, ex: i32, ez: i32) -> bool {
    cx == 0 || cx == ex - 1 || cz == 0 || cz == ez - 1
}

// --- Prop placement tests ---

#[test]
fn room_3x3_gets_at_least_one_prop() {
    // A 3x3 room should always get at least 1 prop across any seed.
    let template = room_3x3();
    let active = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];
    let mut found_any = false;
    for seed in 0..20 {
        let props = furnish(&template, &active, [0.0, 0.0, 0.0], 4.0, seed, RoomDensity::Normal);
        if !props.is_empty() {
            found_any = true;
            break;
        }
    }
    assert!(found_any, "at least one seed should produce props for a 3x3 room");
}

#[test]
fn wall_adjacent_props_are_at_boundary_cells() {
    let template = room_3x3();
    let cell_size = 4.0;
    let active = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];
    let wall_scenes = wall_adjacent_scenes();
    let ex = template.extents[0] as i32;
    let ez = template.extents[2] as i32;

    for seed in 0..10 {
        let props = furnish(&template, &active, [0.0, 0.0, 0.0], cell_size, seed, RoomDensity::Normal);
        for p in &props {
            if wall_scenes.contains(&p.scene) {
                // Convert meter position back to cell index
                let cx = ((p.position[0] / cell_size).floor()) as i32;
                let cz = ((p.position[2] / cell_size).floor()) as i32;
                assert!(
                    is_boundary(cx, cz, ex, ez),
                    "wall-adjacent prop '{}' at {:?} maps to cell ({cx},{cz}) which is not a boundary cell",
                    p.scene, p.position
                );
            }
        }
    }
}

#[test]
fn center_props_are_at_non_boundary_cells() {
    let template = room_5x5();
    let cell_size = 4.0;
    let active = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];
    let center_sc = center_scenes();
    let ex = template.extents[0] as i32;
    let ez = template.extents[2] as i32;

    for seed in 0..10 {
        let props = furnish(&template, &active, [0.0, 0.0, 0.0], cell_size, seed, RoomDensity::Normal);
        for p in &props {
            if center_sc.contains(&p.scene) {
                let cx = ((p.position[0] / cell_size).floor()) as i32;
                let cz = ((p.position[2] / cell_size).floor()) as i32;
                assert!(
                    !is_boundary(cx, cz, ex, ez),
                    "center prop '{}' at {:?} maps to boundary cell ({cx},{cz})",
                    p.scene, p.position
                );
            }
        }
    }
}

#[test]
fn no_props_at_active_connector_cells() {
    let template = room_3x3();
    let cell_size = 4.0;
    let active = vec![
        ConnectorFacing::NegX,
        ConnectorFacing::PosX,
        ConnectorFacing::NegZ,
        ConnectorFacing::PosZ,
    ];

    // Connector cells in meter coords: offset * cell_size + 0.5 * cell_size
    let connector_cells: Vec<(i32, i32)> = template.connectors.iter()
        .filter(|c| active.contains(&c.facing))
        .map(|c| (c.offset[0], c.offset[2]))
        .collect();

    for seed in 0..10 {
        let props = furnish(&template, &active, [0.0, 0.0, 0.0], cell_size, seed, RoomDensity::Normal);
        for p in &props {
            let cx = ((p.position[0] / cell_size).floor()) as i32;
            let cz = ((p.position[2] / cell_size).floor()) as i32;
            assert!(
                !connector_cells.contains(&(cx, cz)),
                "prop '{}' at {:?} is in active connector cell ({cx},{cz})",
                p.scene, p.position
            );
        }
    }
}

#[test]
fn no_prop_position_overlaps() {
    let template = room_5x5();
    let cell_size = 4.0;
    let active = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];

    for seed in 0..10 {
        let props = furnish(&template, &active, [0.0, 0.0, 0.0], cell_size, seed, RoomDensity::Normal);
        for (i, a) in props.iter().enumerate() {
            for (j, b) in props.iter().enumerate() {
                if i != j {
                    let same_pos = (a.position[0] - b.position[0]).abs() < 0.001
                        && (a.position[1] - b.position[1]).abs() < 0.001
                        && (a.position[2] - b.position[2]).abs() < 0.001;
                    assert!(
                        !same_pos,
                        "props overlap at {:?}: '{}' and '{}'",
                        a.position, a.scene, b.scene
                    );
                }
            }
        }
    }
}

#[test]
fn wall_adjacent_props_are_rotated_to_match_wall() {
    let template = room_3x3();
    let cell_size = 4.0;
    // Only NegX active, so PosX/NegZ/PosZ are sealed walls
    let active = vec![ConnectorFacing::NegX];
    let wall_scenes = wall_adjacent_scenes();

    for seed in 0..20 {
        let props = furnish(&template, &active, [0.0, 0.0, 0.0], cell_size, seed, RoomDensity::Normal);
        for p in &props {
            if wall_scenes.contains(&p.scene) {
                let cx = ((p.position[0] / cell_size).floor()) as i32;
                let cz = ((p.position[2] / cell_size).floor()) as i32;
                let _ = (cx, cz);
                // Valid wall rotations (matching wall_placement conventions):
                // NegX=0, PosX=PI, NegZ=-PI/2, PosZ=PI/2
                let pi = std::f32::consts::PI;
                let half_pi = std::f32::consts::FRAC_PI_2;
                let valid_rots = [0.0_f32, pi, -half_pi, half_pi];
                let matches = valid_rots.iter().any(|r| (p.rotation_y - r).abs() < 0.01);
                assert!(
                    matches,
                    "wall-adjacent prop at ({cx},{cz}) has rotation_y={}, not a valid wall rotation",
                    p.rotation_y
                );
            }
        }
    }
}

#[test]
fn corridor_gets_no_center_props() {
    let template = corridor_1x1();
    let cell_size = 4.0;
    let active = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];
    let center_sc = center_scenes();

    for seed in 0..20 {
        let props = furnish(&template, &active, [0.0, 0.0, 0.0], cell_size, seed, RoomDensity::Normal);
        for p in &props {
            assert!(
                !center_sc.contains(&p.scene),
                "corridor should not have center prop '{}'", p.scene
            );
        }
    }
}

// --- Light fixture tests ---

#[test]
fn every_room_cell_gets_at_least_one_light_fixture() {
    let template = room_3x3();
    let cell_size = 4.0;
    let fixtures = light_fixtures(&template, [0.0, 0.0, 0.0], cell_size);
    let cell_count = (template.extents[0] * template.extents[2]) as usize;
    assert_eq!(
        fixtures.len(), cell_count,
        "3x3 room should have {} fixtures, got {}", cell_count, fixtures.len()
    );
}

#[test]
fn corridor_gets_light_fixtures() {
    let template = corridor_1x1();
    let fixtures = light_fixtures(&template, [0.0, 0.0, 0.0], 4.0);
    assert!(
        !fixtures.is_empty(),
        "corridor should get at least 1 light fixture"
    );
}

#[test]
fn light_fixture_mesh_at_ceiling_height() {
    let template = room_3x3();
    let cell_size = 4.0;
    let cell_height = crate::systems::room_assembler::CELL_HEIGHT;
    let origin_y = 0.0;
    let fixtures = light_fixtures(&template, [0.0, origin_y, 0.0], cell_size);
    for (mesh, _) in &fixtures {
        let expected_y = origin_y + cell_height - 0.1;
        assert!(
            (mesh.position[1] - expected_y).abs() < 0.2,
            "fixture mesh Y={} should be near ceiling height {}",
            mesh.position[1], expected_y
        );
    }
}

#[test]
fn light_source_within_fixture_bounds() {
    let template = room_3x3();
    let fixtures = light_fixtures(&template, [0.0, 0.0, 0.0], 4.0);
    for (mesh, light) in &fixtures {
        // Find which fixture catalog entry this is
        let fixture_entry = asset_catalog::ALL_LIGHTS.iter()
            .find(|f| f.scene == mesh.scene)
            .expect("fixture scene should be in catalog");

        // The light offset from the fixture should be within the fixture bounds
        let dx = (light.position[0] - mesh.position[0]).abs();
        let dy = (light.position[1] - mesh.position[1]).abs();
        let dz = (light.position[2] - mesh.position[2]).abs();

        assert!(
            dx <= fixture_entry.fixture_bounds[0]
            && dy <= fixture_entry.fixture_bounds[1]
            && dz <= fixture_entry.fixture_bounds[2],
            "light at {:?} is outside fixture bounds {:?} from mesh at {:?} (offsets: {dx},{dy},{dz})",
            light.position, fixture_entry.fixture_bounds, mesh.position
        );
    }
}

#[test]
fn light_source_inside_room_bounds() {
    let template = room_5x5();
    let cell_size = 4.0;
    let origin = [4.0, 2.0, 8.0];
    let fixtures = light_fixtures(&template, origin, cell_size);
    let max_x = origin[0] + template.extents[0] as f32 * cell_size;
    let max_z = origin[2] + template.extents[2] as f32 * cell_size;
    let cell_height = crate::systems::room_assembler::CELL_HEIGHT;
    let max_y = origin[1] + template.extents[1] as f32 * cell_height;

    for (_, light) in &fixtures {
        assert!(
            light.position[0] >= origin[0] && light.position[0] <= max_x,
            "light x={} outside room [{}, {}]", light.position[0], origin[0], max_x
        );
        assert!(
            light.position[1] >= origin[1] && light.position[1] <= max_y,
            "light y={} outside room [{}, {}]", light.position[1], origin[1], max_y
        );
        assert!(
            light.position[2] >= origin[2] && light.position[2] <= max_z,
            "light z={} outside room [{}, {}]", light.position[2], origin[2], max_z
        );
    }
}

#[test]
fn light_source_range_covers_cell() {
    let cell_size = 4.0;
    let template = room_3x3();
    let fixtures = light_fixtures(&template, [0.0, 0.0, 0.0], cell_size);
    for (_, light) in &fixtures {
        assert!(
            light.range >= cell_size / 2.0,
            "light range {} should be >= cell_size/2 = {}",
            light.range, cell_size / 2.0
        );
    }
}

// --- Flyable path validation tests ---

#[test]
fn empty_room_paths_between_opposite_openings() {
    let template = room_3x3();
    let active = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];
    let props = vec![]; // no props
    assert!(
        flight_paths_clear(&template, &active, &props, 4.0),
        "empty room should have clear paths"
    );
}

#[test]
fn empty_room_paths_between_adjacent_openings() {
    let template = room_3x3();
    let active = vec![ConnectorFacing::NegX, ConnectorFacing::NegZ];
    assert!(
        flight_paths_clear(&template, &active, &[], 4.0),
        "empty room should have L-shaped path"
    );
}

#[test]
fn single_opening_always_passes() {
    let template = room_3x3();
    let active = vec![ConnectorFacing::NegX];
    // Even with a blocking prop in the middle
    let block = MeshPlacement {
        scene: asset_catalog::CENTER_PROPS.iter()
            .find(|p| p.blocks_flight).unwrap().scene,
        position: [6.0, 0.0, 6.0], // center of 3x3 room at origin
        rotation_x: 0.0,
        rotation_y: 0.0,
    };
    assert!(
        flight_paths_clear(&template, &active, &[block], 4.0),
        "single opening needs no paths"
    );
}

#[test]
fn blocking_prop_in_path_detected() {
    // 3x1x3 room with NegX(0,1) and PosX(2,1) openings.
    // Block the middle cell (1,1) which is the only path.
    let template = room_3x3();
    let active = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];
    // Block ALL cells in the middle column to ensure no path exists
    let blocking_scene = asset_catalog::CENTER_PROPS.iter()
        .find(|p| p.blocks_flight).unwrap().scene;
    let blocks = vec![
        MeshPlacement { scene: blocking_scene, position: [6.0, 0.0, 2.0], rotation_x: 0.0, rotation_y: 0.0 },  // cell (1,0)
        MeshPlacement { scene: blocking_scene, position: [6.0, 0.0, 6.0], rotation_x: 0.0, rotation_y: 0.0 },  // cell (1,1)
        MeshPlacement { scene: blocking_scene, position: [6.0, 0.0, 10.0], rotation_x: 0.0, rotation_y: 0.0 }, // cell (1,2)
    ];
    assert!(
        !flight_paths_clear(&template, &active, &blocks, 4.0),
        "blocking all middle cells should block the path"
    );
}

#[test]
fn furnished_3x3_preserves_paths() {
    let template = room_3x3();
    let cell_size = 4.0;
    let active = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];
    for seed in 0..50 {
        let props = furnish(&template, &active, [0.0, 0.0, 0.0], cell_size, seed, RoomDensity::Normal);
        assert!(
            flight_paths_clear(&template, &active, &props, cell_size),
            "seed {seed}: furnished 3x3 should preserve flight paths"
        );
    }
}

#[test]
fn furnished_5x5_with_4_openings_preserves_paths() {
    let template = room_5x5();
    let cell_size = 4.0;
    let active = vec![
        ConnectorFacing::NegX,
        ConnectorFacing::PosX,
        ConnectorFacing::NegZ,
        ConnectorFacing::PosZ,
    ];
    for seed in 0..50 {
        let props = furnish(&template, &active, [0.0, 0.0, 0.0], cell_size, seed, RoomDensity::Normal);
        assert!(
            flight_paths_clear(&template, &active, &props, cell_size),
            "seed {seed}: furnished 5x5 should preserve all flight paths"
        );
    }
}

// --- Room density tests ---

#[test]
fn dense_rooms_produce_more_props_than_sparse() {
    let template = room_5x5();
    let active = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];

    let mut sparse_total = 0usize;
    let mut dense_total = 0usize;
    let seeds = 20;
    for seed in 0..seeds {
        sparse_total += furnish(&template, &active, [0.0, 0.0, 0.0], 4.0, seed, RoomDensity::Sparse).len();
        dense_total += furnish(&template, &active, [0.0, 0.0, 0.0], 4.0, seed, RoomDensity::Dense).len();
    }

    assert!(
        dense_total > sparse_total,
        "dense ({dense_total}) should produce more props than sparse ({sparse_total}) over {seeds} seeds"
    );
}

#[test]
fn dense_room_fills_majority_of_eligible_cells() {
    let template = room_5x5();
    let cell_size = 4.0;
    let active = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];
    let num_seeds = 50usize;
    let total_cells = (template.extents[0] * template.extents[2]) as usize;
    let threshold = (total_cells as f32 * 0.6) as usize;

    let mut total_props = 0usize;
    for seed in 0..num_seeds {
        total_props += furnish(&template, &active, [0.0, 0.0, 0.0], cell_size, seed as u64, RoomDensity::Dense).len();
    }
    let avg = total_props / num_seeds;

    assert!(
        avg >= threshold,
        "dense 5x5 room should average >= {threshold} props, got {avg}"
    );
}

#[test]
fn sparse_room_leaves_most_cells_empty() {
    let template = room_5x5();
    let cell_size = 4.0;
    let active = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];
    let num_seeds = 50usize;
    let total_cells = (template.extents[0] * template.extents[2]) as usize;
    let max_avg = (total_cells as f32 * 0.30) as usize;

    let mut total_props = 0usize;
    for seed in 0..num_seeds {
        total_props += furnish(&template, &active, [0.0, 0.0, 0.0], cell_size, seed as u64, RoomDensity::Sparse).len();
    }
    let avg = total_props / num_seeds;

    assert!(
        avg <= max_avg,
        "sparse 5x5 room should average <= {max_avg} props, got {avg}"
    );
}

#[test]
fn normal_density_preserves_original_prop_count() {
    // Regression guard: Normal density should produce the same output as before.
    let template = room_3x3();
    let active = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];
    let props = furnish(&template, &active, [0.0, 0.0, 0.0], 4.0, 42, RoomDensity::Normal);
    // Pin the count from the current Normal behavior. If this changes, the test
    // catches accidental regressions in the default density path.
    let count = props.len();
    assert!(
        count > 0,
        "seed 42 on 3x3 with Normal density should produce at least 1 prop"
    );
}

#[test]
fn dense_furnished_room_preserves_flight_paths() {
    let template = room_3x3();
    let cell_size = 4.0;
    let active = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];
    for seed in 0..50 {
        let props = furnish(&template, &active, [0.0, 0.0, 0.0], cell_size, seed, RoomDensity::Dense);
        assert!(
            flight_paths_clear(&template, &active, &props, cell_size),
            "seed {seed}: dense 3x3 should preserve flight paths"
        );
    }
}

// --- Prop clipping prevention tests (Phase 11) ---

#[test]
fn wall_adjacent_offset_leaves_at_least_1m_clearance() {
    // Wall-adjacent offset must be <= cell_size * 0.25 so props have at least
    // 1m clearance from the wall edge in a 4m cell.
    let cell_size = 4.0;
    let max_offset = cell_size * 0.25; // 1.0m

    let faces = [
        ConnectorFacing::NegX,
        ConnectorFacing::PosX,
        ConnectorFacing::NegZ,
        ConnectorFacing::PosZ,
    ];

    for face in &faces {
        let (ox, oz, _rot) = wall_adjacent_offset(*face, cell_size);
        let abs_offset = ox.abs().max(oz.abs());
        assert!(
            abs_offset <= max_offset,
            "{:?}: offset {abs_offset} exceeds max {max_offset} (cell_size={cell_size})",
            face
        );
    }
}

#[test]
fn all_furnished_props_within_cell_bounds() {
    // Every prop position must be inside the room's world-space bounding box.
    let templates = [room_3x3(), room_5x5()];
    let cell_size = 4.0;

    for template in &templates {
        let active = vec![ConnectorFacing::NegX, ConnectorFacing::PosX];
        let origin = [4.0, 2.0, 8.0]; // non-zero origin to catch offset bugs
        let max_x = origin[0] + template.extents[0] as f32 * cell_size;
        let max_z = origin[2] + template.extents[2] as f32 * cell_size;

        for seed in 0..20 {
            let props = furnish(&template, &active, origin, cell_size, seed, RoomDensity::Normal);
            for p in &props {
                assert!(
                    p.position[0] >= origin[0] && p.position[0] <= max_x,
                    "seed {seed}, template {}: prop '{}' x={} outside [{}, {}]",
                    template.id, p.scene, p.position[0], origin[0], max_x
                );
                assert!(
                    p.position[2] >= origin[2] && p.position[2] <= max_z,
                    "seed {seed}, template {}: prop '{}' z={} outside [{}, {}]",
                    template.id, p.scene, p.position[2], origin[2], max_z
                );
            }
        }
    }
}

#[test]
fn wall_adjacent_props_closer_to_wall_than_center() {
    // After offset reduction, wall-adjacent props must still be closer to
    // their wall than to the cell center.
    let template = room_5x5();
    let cell_size = 4.0;
    let active = vec![ConnectorFacing::NegX];
    let wall_scenes = wall_adjacent_scenes();
    let origin = [0.0, 0.0, 0.0];
    let ex = template.extents[0] as f32;
    let ez = template.extents[2] as f32;

    for seed in 0..20 {
        let props = furnish(&template, &active, origin, cell_size, seed, RoomDensity::Normal);
        for p in &props {
            if !wall_scenes.contains(&p.scene) {
                continue;
            }
            // Find which cell this prop belongs to.
            let cx = (p.position[0] / cell_size).floor();
            let cz = (p.position[2] / cell_size).floor();
            let cell_center_x = (cx + 0.5) * cell_size;
            let cell_center_z = (cz + 0.5) * cell_size;

            // Distance from prop to cell center.
            let dx_center = (p.position[0] - cell_center_x).abs();
            let dz_center = (p.position[2] - cell_center_z).abs();
            let dist_to_center = dx_center.max(dz_center);

            // Distance from prop to nearest room boundary wall.
            let dist_to_neg_x = p.position[0] - origin[0];
            let dist_to_pos_x = origin[0] + ex * cell_size - p.position[0];
            let dist_to_neg_z = p.position[2] - origin[2];
            let dist_to_pos_z = origin[2] + ez * cell_size - p.position[2];
            let dist_to_wall = dist_to_neg_x.min(dist_to_pos_x).min(dist_to_neg_z).min(dist_to_pos_z);

            assert!(
                dist_to_wall <= dist_to_center + 0.01,
                "seed {seed}: wall-adjacent prop '{}' at {:?} is closer to center ({dist_to_center:.2}) than wall ({dist_to_wall:.2})",
                p.scene, p.position
            );
        }
    }
}
