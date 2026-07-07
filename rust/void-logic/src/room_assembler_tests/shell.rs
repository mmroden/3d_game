use super::*;
use crate::room_assembler::{shell_slabs, ShellSlab, SHELL_THICKNESS};

/// Grid at the pinned test pitch, origin at the world origin — cell 0
/// spans x,z ∈ [0, TILE_WIDTH], y ∈ [0, STORY_HEIGHT].
fn grid_for(template: &RoomTemplate, active: &[Connector]) -> crate::cell::CellGrid {
    crate::cell::CellGrid::new(template, active, [0.0, 0.0, 0.0], TILE_WIDTH, STORY_HEIGHT)
}

fn slab_min(s: &ShellSlab, axis: usize) -> f32 {
    s.center[axis] - s.size[axis] * 0.5
}

fn slab_max(s: &ShellSlab, axis: usize) -> f32 {
    s.center[axis] + s.size[axis] * 0.5
}

/// The slab guarding a face: thickness axis = SHELL_THICKNESS, sitting
/// flush outside `plane` (on `side`: -1 grows downward/negward, +1 up).
fn face_slab<'a>(slabs: &'a [ShellSlab], axis: usize, plane: f32, side: f32) -> Option<&'a ShellSlab> {
    slabs.iter().find(|s| {
        (s.size[axis] - SHELL_THICKNESS).abs() < 1e-4
            && if side < 0.0 {
                (slab_max(s, axis) - plane).abs() < 1e-4
            } else {
                (slab_min(s, axis) - plane).abs() < 1e-4
            }
    })
}

#[test]
fn a_sealed_cell_gets_all_six_faces_slabbed() {
    let slabs = shell_slabs(&grid_for(&small_room(), &[]));
    assert_eq!(slabs.len(), 6, "one sealed cell: one slab per face, got {slabs:?}");
    assert!(face_slab(&slabs, 0, 0.0, -1.0).is_some(), "NegX slab flush outside x=0");
    assert!(face_slab(&slabs, 0, TILE_WIDTH, 1.0).is_some(), "PosX slab flush outside x=tile");
    assert!(face_slab(&slabs, 2, 0.0, -1.0).is_some(), "NegZ slab flush outside z=0");
    assert!(face_slab(&slabs, 2, TILE_WIDTH, 1.0).is_some(), "PosZ slab flush outside z=tile");
    assert!(face_slab(&slabs, 1, 0.0, -1.0).is_some(), "floor slab flush below y=0");
    assert!(face_slab(&slabs, 1, STORY_HEIGHT, 1.0).is_some(), "ceiling slab flush above y=story");
}

#[test]
fn a_doorway_face_stays_open() {
    let active = [Connector {
        offset: [0, 0, 0],
        facing: ConnectorFacing::PosX,
        frame: FrameStyle::Door,
    }];
    let slabs = shell_slabs(&grid_for(&small_room(), &active));
    assert_eq!(slabs.len(), 5, "the doorway face emits no slab, got {slabs:?}");
    assert!(
        face_slab(&slabs, 0, TILE_WIDTH, 1.0).is_none(),
        "no slab may cover the PosX doorway"
    );
    assert!(face_slab(&slabs, 0, 0.0, -1.0).is_some(), "the sealed NegX face keeps its slab");
}

#[test]
fn slabs_bleed_past_their_cell_so_edges_have_no_pinholes() {
    // A diagonal path through the exact edge line between two orthogonal
    // slabs must still hit solid: every slab overhangs its cell span by
    // the shell thickness on each tangential side, so neighbours overlap
    // along edges and corners.
    let slabs = shell_slabs(&grid_for(&small_room(), &[]));
    let neg_x = face_slab(&slabs, 0, 0.0, -1.0).expect("NegX slab");
    assert!(slab_min(neg_x, 2) <= -SHELL_THICKNESS + 1e-4, "NegX slab bleeds past z=0");
    assert!(slab_max(neg_x, 2) >= TILE_WIDTH + SHELL_THICKNESS - 1e-4, "NegX slab bleeds past z=tile");
    assert!(slab_min(neg_x, 1) <= -SHELL_THICKNESS + 1e-4, "NegX slab bleeds below the floor");
    assert!(slab_max(neg_x, 1) >= STORY_HEIGHT + SHELL_THICKNESS - 1e-4, "NegX slab bleeds above the ceiling");
}

#[test]
fn a_multicell_room_slabs_every_boundary_cell_face() {
    // 2×1×2 sealed room: 4 cells × (2 sealed XZ faces + floor + ceiling).
    let template = RoomTemplate {
        kind: TemplateKind::Room,
        connectors: vec![],
        enemy_spawns: vec![],
        loot_spawns: vec![],
        extents: [2, 1, 2],
    };
    let slabs = shell_slabs(&grid_for(&template, &[]));
    assert_eq!(slabs.len(), 16, "4 cells × 4 sealed faces each, got {}", slabs.len());
    // Interior planes (x = tile between the two columns) carry no slab.
    for s in &slabs {
        let flush_inside = (s.size[0] - SHELL_THICKNESS).abs() < 1e-4
            && ((slab_max(s, 0) - TILE_WIDTH).abs() < 1e-4
                || (slab_min(s, 0) - TILE_WIDTH).abs() < 1e-4);
        assert!(!flush_inside, "no slab may stand inside the room at x=tile: {s:?}");
    }
}
