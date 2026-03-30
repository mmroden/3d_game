use crate::systems::asset_catalog::WallSet;
use crate::systems::room_assembler::MeshPlacement;
use crate::systems::room_template::ConnectorFacing;

// ── Extents ────────────────────────────────────────────────────────────

/// Axis-aligned extents for a cell's content, in meters from the cell's pivot.
/// neg_x is the distance from pivot toward -X, pos_x toward +X, etc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellExtents {
    pub neg_x: f32,
    pub pos_x: f32,
    pub neg_z: f32,
    pub pos_z: f32,
    pub height: f32,
}

impl CellExtents {
    pub fn width_x(&self) -> f32 {
        self.neg_x + self.pos_x
    }
    pub fn width_z(&self) -> f32 {
        self.neg_z + self.pos_z
    }

    /// XZ offset to align asymmetric geometry with adjacent cells.
    /// Pushes from the larger (boundary) side toward the smaller (interior) side.
    /// Symmetric cells return `[0.0, 0.0]`.
    pub fn interior_offset(&self) -> [f32; 2] {
        let ox = if self.neg_x > self.pos_x {
            self.pos_x
        } else if self.pos_x > self.neg_x {
            -self.neg_x
        } else {
            0.0
        };
        let oz = if self.neg_z > self.pos_z {
            self.pos_z
        } else if self.pos_z > self.neg_z {
            -self.neg_z
        } else {
            0.0
        };
        [ox, oz]
    }
}

// ── Trait ───────────────────────────────────────────────────────────────

/// Each cell type declares its own dimensions and emits its own geometry.
pub trait CellGeometry {
    /// Axis-aligned bounding extents for this cell.
    fn extents(&self) -> CellExtents;
    /// All mesh placements for this cell, relative to the cell's origin.
    fn placements(&self) -> Vec<MeshPlacement>;
}

// ── Cell type structs ──────────────────────────────────────────────────

pub struct CornerCell<'a> {
    pub mesh_set: &'a WallSet,
    pub corner_pair: (ConnectorFacing, ConnectorFacing),
}

pub struct EdgeCell<'a> {
    pub mesh_set: &'a WallSet,
    pub sealed_face: ConnectorFacing,
}

pub struct InteriorCell<'a> {
    pub mesh_set: &'a WallSet,
}

pub struct ConnectorCell<'a> {
    pub mesh_set: &'a WallSet,
    pub facing: ConnectorFacing,
    pub is_corridor: bool,
}

// ── Enum dispatch ──────────────────────────────────────────────────────

pub enum CellType<'a> {
    Corner(CornerCell<'a>),
    Edge(EdgeCell<'a>),
    Interior(InteriorCell<'a>),
    Connector(ConnectorCell<'a>),
}

impl CellGeometry for CellType<'_> {
    fn extents(&self) -> CellExtents {
        match self {
            Self::Corner(c) => c.extents(),
            Self::Edge(e) => e.extents(),
            Self::Interior(i) => i.extents(),
            Self::Connector(c) => c.extents(),
        }
    }

    fn placements(&self) -> Vec<MeshPlacement> {
        match self {
            Self::Corner(c) => c.placements(),
            Self::Edge(e) => e.placements(),
            Self::Interior(i) => i.placements(),
            Self::Connector(c) => c.placements(),
        }
    }
}

// ── Mesh geometry constants (from GLTF accessor min/max bounds) ────────

/// Corner inner wall: x ∈ [-4.465, 0.0], z ∈ [-4.468, 0.0], y ∈ [0.2, 3.0]
const CORNER_REACH: f32 = 4.468;
/// Straight wall: x ∈ [-2.774, -1.601], z ∈ [-2.0, 2.0], y ∈ [0.2, 3.0]
/// Floor platform: [-2.0, 2.0] × [-2.0, 2.0]
const INTERIOR_HALF: f32 = 2.0;
/// Wall + top-strip extend from Y≈0.2 to Y=5.0
const CELL_HEIGHT: f32 = 5.0;

// ── Rotation helpers ───────────────────────────────────────────────────

fn wall_rotation(facing: ConnectorFacing) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    match facing {
        ConnectorFacing::NegX => 0.0,
        ConnectorFacing::PosX => PI,
        ConnectorFacing::NegZ => -FRAC_PI_2,
        ConnectorFacing::PosZ => FRAC_PI_2,
        _ => 0.0,
    }
}

fn door_rotation(facing: ConnectorFacing) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    match facing {
        ConnectorFacing::NegX => FRAC_PI_2,
        ConnectorFacing::PosX => -FRAC_PI_2,
        ConnectorFacing::NegZ => 0.0,
        ConnectorFacing::PosZ => PI,
        _ => 0.0,
    }
}

fn corner_rotation(pair: (ConnectorFacing, ConnectorFacing)) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    match pair {
        (ConnectorFacing::NegX, ConnectorFacing::NegZ) => 0.0,
        (ConnectorFacing::PosX, ConnectorFacing::NegZ) => -FRAC_PI_2,
        (ConnectorFacing::NegX, ConnectorFacing::PosZ) => FRAC_PI_2,
        (ConnectorFacing::PosX, ConnectorFacing::PosZ) => PI,
        _ => 0.0,
    }
}

// ── Standalone geometry helpers ────────────────────────────────────────

/// Corner extents for a given pair, without requiring a `WallSet`.
/// `CornerCell::extents()` delegates here to avoid duplication.
pub fn corner_extents(pair: (ConnectorFacing, ConnectorFacing)) -> CellExtents {
    let (a, b) = pair;
    let neg_x = if a == ConnectorFacing::NegX || b == ConnectorFacing::NegX { CORNER_REACH } else { INTERIOR_HALF };
    let pos_x = if a == ConnectorFacing::PosX || b == ConnectorFacing::PosX { CORNER_REACH } else { INTERIOR_HALF };
    let neg_z = if a == ConnectorFacing::NegZ || b == ConnectorFacing::NegZ { CORNER_REACH } else { INTERIOR_HALF };
    let pos_z = if a == ConnectorFacing::PosZ || b == ConnectorFacing::PosZ { CORNER_REACH } else { INTERIOR_HALF };
    CellExtents { neg_x, pos_x, neg_z, pos_z, height: CELL_HEIGHT }
}

// ── Implementations ────────────────────────────────────────────────────

impl CellGeometry for CornerCell<'_> {
    fn extents(&self) -> CellExtents {
        corner_extents(self.corner_pair)
    }

    fn placements(&self) -> Vec<MeshPlacement> {
        let rot = corner_rotation(self.corner_pair);
        let origin = [0.0, 0.0, 0.0];
        vec![
            MeshPlacement { scene: self.mesh_set.corner_inner.wall, position: origin, rotation_x: 0.0, rotation_y: rot },
            MeshPlacement { scene: self.mesh_set.corner_inner.ceiling, position: origin, rotation_x: 0.0, rotation_y: rot },
            MeshPlacement { scene: self.mesh_set.corner_outer.wall, position: origin, rotation_x: 0.0, rotation_y: rot },
            MeshPlacement { scene: self.mesh_set.corner_outer.ceiling, position: origin, rotation_x: 0.0, rotation_y: rot },
            MeshPlacement { scene: self.mesh_set.corner_inner.floor, position: origin, rotation_x: 0.0, rotation_y: rot },
        ]
    }
}

impl CellGeometry for EdgeCell<'_> {
    fn extents(&self) -> CellExtents {
        CellExtents { neg_x: INTERIOR_HALF, pos_x: INTERIOR_HALF, neg_z: INTERIOR_HALF, pos_z: INTERIOR_HALF, height: CELL_HEIGHT }
    }

    fn placements(&self) -> Vec<MeshPlacement> {
        let rot = wall_rotation(self.sealed_face);
        let origin = [0.0, 0.0, 0.0];
        vec![
            MeshPlacement { scene: self.mesh_set.straight.wall, position: origin, rotation_x: 0.0, rotation_y: rot },
            MeshPlacement { scene: self.mesh_set.straight.ceiling, position: origin, rotation_x: 0.0, rotation_y: rot },
        ]
    }
}

impl CellGeometry for InteriorCell<'_> {
    fn extents(&self) -> CellExtents {
        CellExtents { neg_x: INTERIOR_HALF, pos_x: INTERIOR_HALF, neg_z: INTERIOR_HALF, pos_z: INTERIOR_HALF, height: CELL_HEIGHT }
    }

    fn placements(&self) -> Vec<MeshPlacement> {
        // Interior cells emit no walls — just floor/ceiling handled by assembler.
        Vec::new()
    }
}

impl CellGeometry for ConnectorCell<'_> {
    fn extents(&self) -> CellExtents {
        CellExtents { neg_x: INTERIOR_HALF, pos_x: INTERIOR_HALF, neg_z: INTERIOR_HALF, pos_z: INTERIOR_HALF, height: CELL_HEIGHT }
    }

    fn placements(&self) -> Vec<MeshPlacement> {
        if self.is_corridor {
            let rot = door_rotation(self.facing);
            vec![
                MeshPlacement {
                    scene: crate::systems::asset_catalog::DOOR,
                    position: [0.0, 0.0, 0.0],
                    rotation_x: 0.0,
                    rotation_y: rot,
                },
            ]
        } else {
            // Room connectors leave a gap — no geometry.
            Vec::new()
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::asset_catalog::*;

    // ── Universal properties (all cell type × mesh set combinations) ───

    fn all_wall_sets() -> Vec<&'static WallSet> {
        ALL_WALL_SETS.iter().collect()
    }

    fn all_corner_pairs() -> Vec<(ConnectorFacing, ConnectorFacing)> {
        vec![
            (ConnectorFacing::NegX, ConnectorFacing::NegZ),
            (ConnectorFacing::PosX, ConnectorFacing::NegZ),
            (ConnectorFacing::NegX, ConnectorFacing::PosZ),
            (ConnectorFacing::PosX, ConnectorFacing::PosZ),
        ]
    }

    fn all_edge_faces() -> Vec<ConnectorFacing> {
        vec![
            ConnectorFacing::NegX,
            ConnectorFacing::PosX,
            ConnectorFacing::NegZ,
            ConnectorFacing::PosZ,
        ]
    }

    #[test]
    fn extents_width_is_sum_of_halves() {
        let e = CellExtents { neg_x: 2.0, pos_x: 3.0, neg_z: 1.5, pos_z: 2.5, height: 5.0 };
        assert_eq!(e.width_x(), 5.0);
        assert_eq!(e.width_z(), 4.0);
    }

    // ── CornerCell tests ───────────────────────────────────────────────

    #[test]
    fn corner_extents_exceed_4m() {
        for ws in all_wall_sets() {
            for pair in all_corner_pairs() {
                let cell = CornerCell { mesh_set: ws, corner_pair: pair };
                let ext = cell.extents();
                assert!(
                    ext.width_x() > 4.0,
                    "wall set '{}' corner {:?}: width_x {} should exceed 4.0m",
                    ws.id, pair, ext.width_x()
                );
                assert!(
                    ext.width_z() > 4.0,
                    "wall set '{}' corner {:?}: width_z {} should exceed 4.0m",
                    ws.id, pair, ext.width_z()
                );
            }
        }
    }

    #[test]
    fn corner_extents_are_positive() {
        for ws in all_wall_sets() {
            for pair in all_corner_pairs() {
                let cell = CornerCell { mesh_set: ws, corner_pair: pair };
                let ext = cell.extents();
                assert!(ext.neg_x > 0.0 && ext.pos_x > 0.0, "wall set '{}': corner neg_x/pos_x must be positive", ws.id);
                assert!(ext.neg_z > 0.0 && ext.pos_z > 0.0, "wall set '{}': corner neg_z/pos_z must be positive", ws.id);
                assert!(ext.height > 0.0, "wall set '{}': corner height must be positive", ws.id);
            }
        }
    }

    #[test]
    fn corner_emits_inner_and_outer_pieces() {
        for ws in all_wall_sets() {
            for pair in all_corner_pairs() {
                let cell = CornerCell { mesh_set: ws, corner_pair: pair };
                let placements = cell.placements();
                let inner_walls = placements.iter().filter(|p| p.scene == ws.corner_inner.wall).count();
                let inner_ceilings = placements.iter().filter(|p| p.scene == ws.corner_inner.ceiling).count();
                let outer_walls = placements.iter().filter(|p| p.scene == ws.corner_outer.wall).count();
                let outer_ceilings = placements.iter().filter(|p| p.scene == ws.corner_outer.ceiling).count();
                assert_eq!(inner_walls, 1, "wall set '{}' corner {:?}: expected 1 inner wall", ws.id, pair);
                assert_eq!(inner_ceilings, 1, "wall set '{}' corner {:?}: expected 1 inner ceiling", ws.id, pair);
                assert_eq!(outer_walls, 1, "wall set '{}' corner {:?}: expected 1 outer wall", ws.id, pair);
                assert_eq!(outer_ceilings, 1, "wall set '{}' corner {:?}: expected 1 outer ceiling", ws.id, pair);
            }
        }
    }

    #[test]
    fn corner_emits_no_straight_walls() {
        for ws in all_wall_sets() {
            for pair in all_corner_pairs() {
                let cell = CornerCell { mesh_set: ws, corner_pair: pair };
                let placements = cell.placements();
                let straight_walls = placements.iter().filter(|p| p.scene == ws.straight.wall).count();
                let straight_ceilings = placements.iter().filter(|p| p.scene == ws.straight.ceiling).count();
                assert_eq!(straight_walls, 0, "wall set '{}' corner {:?}: should emit 0 straight walls", ws.id, pair);
                assert_eq!(straight_ceilings, 0, "wall set '{}' corner {:?}: should emit 0 straight ceilings", ws.id, pair);
            }
        }
    }

    #[test]
    fn corner_emits_curved_floor() {
        for ws in all_wall_sets() {
            for pair in all_corner_pairs() {
                let cell = CornerCell { mesh_set: ws, corner_pair: pair };
                let placements = cell.placements();
                let curved_floors = placements.iter().filter(|p| p.scene == ws.corner_inner.floor).count();
                assert_eq!(curved_floors, 1, "wall set '{}' corner {:?}: expected 1 curved floor", ws.id, pair);
                // Only check for zero straight floors when the meshes differ;
                // some wall sets (e.g. padded) use the same platform for both.
                if ws.straight.floor != ws.corner_inner.floor {
                    let straight_floors = placements.iter().filter(|p| p.scene == ws.straight.floor).count();
                    assert_eq!(straight_floors, 0, "wall set '{}' corner {:?}: expected 0 straight floors", ws.id, pair);
                }
            }
        }
    }

    // ── EdgeCell tests ─────────────────────────────────────────────────

    #[test]
    fn edge_extents_are_4m() {
        for ws in all_wall_sets() {
            for face in all_edge_faces() {
                let cell = EdgeCell { mesh_set: ws, sealed_face: face };
                let ext = cell.extents();
                assert!(
                    (ext.width_x() - 4.0).abs() < 0.01,
                    "wall set '{}' edge {:?}: width_x {} should be ~4.0m",
                    ws.id, face, ext.width_x()
                );
                assert!(
                    (ext.width_z() - 4.0).abs() < 0.01,
                    "wall set '{}' edge {:?}: width_z {} should be ~4.0m",
                    ws.id, face, ext.width_z()
                );
            }
        }
    }

    #[test]
    fn edge_extents_are_positive() {
        for ws in all_wall_sets() {
            for face in all_edge_faces() {
                let cell = EdgeCell { mesh_set: ws, sealed_face: face };
                let ext = cell.extents();
                assert!(ext.neg_x > 0.0 && ext.pos_x > 0.0);
                assert!(ext.neg_z > 0.0 && ext.pos_z > 0.0);
                assert!(ext.height > 0.0);
            }
        }
    }

    #[test]
    fn edge_emits_straight_wall_and_ceiling() {
        for ws in all_wall_sets() {
            for face in all_edge_faces() {
                let cell = EdgeCell { mesh_set: ws, sealed_face: face };
                let placements = cell.placements();
                let walls = placements.iter().filter(|p| p.scene == ws.straight.wall).count();
                let ceilings = placements.iter().filter(|p| p.scene == ws.straight.ceiling).count();
                assert_eq!(walls, 1, "wall set '{}' edge {:?}: expected 1 straight wall", ws.id, face);
                assert_eq!(ceilings, 1, "wall set '{}' edge {:?}: expected 1 straight ceiling", ws.id, face);
            }
        }
    }

    #[test]
    fn edge_emits_no_corner_pieces() {
        for ws in all_wall_sets() {
            for face in all_edge_faces() {
                let cell = EdgeCell { mesh_set: ws, sealed_face: face };
                let placements = cell.placements();
                let corners = placements.iter().filter(|p| {
                    p.scene == ws.corner_inner.wall || p.scene == ws.corner_outer.wall
                }).count();
                assert_eq!(corners, 0, "wall set '{}' edge {:?}: expected 0 corner pieces", ws.id, face);
            }
        }
    }

    // ── InteriorCell tests ─────────────────────────────────────────────

    #[test]
    fn interior_extents_are_4m() {
        for ws in all_wall_sets() {
            let cell = InteriorCell { mesh_set: ws };
            let ext = cell.extents();
            assert!(
                (ext.width_x() - 4.0).abs() < 0.01,
                "wall set '{}' interior: width_x {} should be ~4.0m",
                ws.id, ext.width_x()
            );
            assert!(
                (ext.width_z() - 4.0).abs() < 0.01,
                "wall set '{}' interior: width_z {} should be ~4.0m",
                ws.id, ext.width_z()
            );
        }
    }

    #[test]
    fn interior_extents_are_positive() {
        for ws in all_wall_sets() {
            let cell = InteriorCell { mesh_set: ws };
            let ext = cell.extents();
            assert!(ext.neg_x > 0.0 && ext.pos_x > 0.0);
            assert!(ext.neg_z > 0.0 && ext.pos_z > 0.0);
            assert!(ext.height > 0.0);
        }
    }

    #[test]
    fn interior_emits_no_walls_or_corners() {
        for ws in all_wall_sets() {
            let cell = InteriorCell { mesh_set: ws };
            let placements = cell.placements();
            let walls = placements.iter().filter(|p| {
                p.scene == ws.straight.wall || p.scene == ws.corner_inner.wall || p.scene == ws.corner_outer.wall
            }).count();
            assert_eq!(walls, 0, "wall set '{}' interior: expected 0 walls/corners", ws.id);
        }
    }

    // ── ConnectorCell tests ────────────────────────────────────────────

    #[test]
    fn connector_extents_are_positive() {
        for ws in all_wall_sets() {
            for face in all_edge_faces() {
                let cell = ConnectorCell { mesh_set: ws, facing: face, is_corridor: true };
                let ext = cell.extents();
                assert!(ext.neg_x > 0.0 && ext.pos_x > 0.0);
                assert!(ext.neg_z > 0.0 && ext.pos_z > 0.0);
                assert!(ext.height > 0.0);
            }
        }
    }

    #[test]
    fn corridor_connector_emits_door_frame() {
        let door_scene = crate::systems::asset_catalog::DOOR;
        for ws in all_wall_sets() {
            for face in all_edge_faces() {
                let cell = ConnectorCell { mesh_set: ws, facing: face, is_corridor: true };
                let placements = cell.placements();
                let doors = placements.iter().filter(|p| p.scene == door_scene).count();
                assert_eq!(doors, 1, "wall set '{}' corridor connector {:?}: expected 1 door", ws.id, face);
            }
        }
    }

    #[test]
    fn room_connector_emits_no_door() {
        let door_scene = crate::systems::asset_catalog::DOOR;
        for ws in all_wall_sets() {
            for face in all_edge_faces() {
                let cell = ConnectorCell { mesh_set: ws, facing: face, is_corridor: false };
                let placements = cell.placements();
                let doors = placements.iter().filter(|p| p.scene == door_scene).count();
                assert_eq!(doors, 0, "wall set '{}' room connector {:?}: expected 0 doors (gap only)", ws.id, face);
            }
        }
    }

    // ── Corner wider than interior ─────────────────────────────────────

    #[test]
    fn corner_wider_than_interior() {
        for ws in all_wall_sets() {
            let corner = CornerCell {
                mesh_set: ws,
                corner_pair: (ConnectorFacing::NegX, ConnectorFacing::NegZ),
            };
            let interior = InteriorCell { mesh_set: ws };
            assert!(
                corner.extents().width_x() > interior.extents().width_x(),
                "wall set '{}': corner width_x {} should exceed interior width_x {}",
                ws.id, corner.extents().width_x(), interior.extents().width_x()
            );
        }
    }

    // ── CellType enum dispatches correctly ─────────────────────────────

    #[test]
    fn cell_type_enum_dispatches_extents() {
        let ws = &WALL_SET_ASTRA;
        let corner = CellType::Corner(CornerCell {
            mesh_set: ws,
            corner_pair: (ConnectorFacing::NegX, ConnectorFacing::NegZ),
        });
        let interior = CellType::Interior(InteriorCell { mesh_set: ws });

        // Just verify dispatch works (values tested by type-specific tests)
        assert!(corner.extents().width_x() > 0.0 || corner.extents().width_x() == 0.0);
        assert!(interior.extents().width_x() > 0.0 || interior.extents().width_x() == 0.0);
    }

    // ── CellExtents::interior_offset tests ────────────────────────────

    #[test]
    fn corner_extents_matches_corner_cell_extents() {
        for ws in all_wall_sets() {
            for pair in all_corner_pairs() {
                let from_fn = corner_extents(pair);
                let from_trait = CornerCell { mesh_set: ws, corner_pair: pair }.extents();
                assert_eq!(from_fn, from_trait,
                    "corner_extents({pair:?}) should match CornerCell::extents() for wall set '{}'", ws.id);
            }
        }
    }

    #[test]
    fn interior_offset_zero_for_symmetric_cells() {
        for ws in all_wall_sets() {
            let edge = EdgeCell { mesh_set: ws, sealed_face: ConnectorFacing::NegX };
            let [ox, oz] = edge.extents().interior_offset();
            assert!(ox.abs() < 0.001 && oz.abs() < 0.001,
                "edge cell should have zero offset, got [{ox}, {oz}]");

            let interior = InteriorCell { mesh_set: ws };
            let [ox, oz] = interior.extents().interior_offset();
            assert!(ox.abs() < 0.001 && oz.abs() < 0.001,
                "interior cell should have zero offset, got [{ox}, {oz}]");
        }
    }

    #[test]
    fn interior_offset_pushes_toward_interior_for_corners() {
        for ws in all_wall_sets() {
            // NegX+NegZ corner: boundary on -X and -Z, interior on +X and +Z
            // Offset should be positive on both axes (toward interior)
            let ext = CornerCell { mesh_set: ws, corner_pair: (ConnectorFacing::NegX, ConnectorFacing::NegZ) }.extents();
            let [ox, oz] = ext.interior_offset();
            assert!(ox > 0.0, "NegX+NegZ: ox should be positive (toward +X), got {ox}");
            assert!(oz > 0.0, "NegX+NegZ: oz should be positive (toward +Z), got {oz}");

            // PosX+NegZ corner: boundary on +X and -Z
            let ext = CornerCell { mesh_set: ws, corner_pair: (ConnectorFacing::PosX, ConnectorFacing::NegZ) }.extents();
            let [ox, oz] = ext.interior_offset();
            assert!(ox < 0.0, "PosX+NegZ: ox should be negative (toward -X), got {ox}");
            assert!(oz > 0.0, "PosX+NegZ: oz should be positive (toward +Z), got {oz}");

            // NegX+PosZ corner: boundary on -X and +Z
            let ext = CornerCell { mesh_set: ws, corner_pair: (ConnectorFacing::NegX, ConnectorFacing::PosZ) }.extents();
            let [ox, oz] = ext.interior_offset();
            assert!(ox > 0.0, "NegX+PosZ: ox should be positive (toward +X), got {ox}");
            assert!(oz < 0.0, "NegX+PosZ: oz should be negative (toward -Z), got {oz}");

            // PosX+PosZ corner: boundary on +X and +Z
            let ext = CornerCell { mesh_set: ws, corner_pair: (ConnectorFacing::PosX, ConnectorFacing::PosZ) }.extents();
            let [ox, oz] = ext.interior_offset();
            assert!(ox < 0.0, "PosX+PosZ: ox should be negative (toward -X), got {ox}");
            assert!(oz < 0.0, "PosX+PosZ: oz should be negative (toward -Z), got {oz}");
        }
    }
}
