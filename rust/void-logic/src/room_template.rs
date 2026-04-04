/// Cardinal directions a room connector can face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectorFacing {
    PosX,
    NegX,
    PosZ,
    NegZ,
    PosY,
    NegY,
}

impl ConnectorFacing {
    /// Returns the opposite facing, i.e. the facing a connector must have
    /// to mate with this one.
    pub fn opposite(self) -> Self {
        match self {
            Self::PosX => Self::NegX,
            Self::NegX => Self::PosX,
            Self::PosZ => Self::NegZ,
            Self::NegZ => Self::PosZ,
            Self::PosY => Self::NegY,
            Self::NegY => Self::PosY,
        }
    }

    /// Unit offset in grid coordinates for this facing direction.
    pub fn grid_offset(self) -> [i32; 3] {
        match self {
            Self::PosX => [1, 0, 0],
            Self::NegX => [-1, 0, 0],
            Self::PosZ => [0, 0, 1],
            Self::NegZ => [0, 0, -1],
            Self::PosY => [0, 1, 0],
            Self::NegY => [0, -1, 0],
        }
    }
}

/// A connection point on a room where corridors attach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Connector {
    /// Position relative to room origin, in grid units.
    pub offset: [i32; 3],
    pub facing: ConnectorFacing,
}

impl Connector {
    /// Compute the world grid position this connector points *to*
    /// (i.e. where an adjacent room's origin would need to be
    /// for its opposite connector to mate).
    pub fn target_cell(&self, room_origin: [i32; 3]) -> [i32; 3] {
        let dir = self.facing.grid_offset();
        [
            room_origin[0] + self.offset[0] + dir[0],
            room_origin[1] + self.offset[1] + dir[1],
            room_origin[2] + self.offset[2] + dir[2],
        ]
    }

    /// Whether this connector can mate with another.
    pub fn mates_with(&self, other: &Connector) -> bool {
        self.facing.opposite() == other.facing
    }
}

/// A point where enemies or loot can spawn within a room.
#[derive(Debug, Clone, Copy)]
pub struct SpawnPoint {
    /// Position relative to room origin, in world units.
    pub position: [f32; 3],
}

/// Whether a template represents a room (gameplay space) or a
/// corridor (connective tissue between rooms).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateKind {
    Room,
    Corridor,
}

/// Defines the shape and connection points for a room type.
/// Does not hold any Godot resources — the node layer maps
/// template IDs to actual scene files.
#[derive(Debug, Clone)]
pub struct RoomTemplate {
    pub kind: TemplateKind,
    pub connectors: Vec<Connector>,
    pub enemy_spawns: Vec<SpawnPoint>,
    pub loot_spawns: Vec<SpawnPoint>,
    /// Size in grid units (x, y, z).
    pub extents: [u32; 3],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposite_facings_are_symmetric() {
        assert_eq!(ConnectorFacing::PosX.opposite(), ConnectorFacing::NegX);
        assert_eq!(ConnectorFacing::NegX.opposite(), ConnectorFacing::PosX);
        assert_eq!(ConnectorFacing::PosZ.opposite(), ConnectorFacing::NegZ);
        assert_eq!(ConnectorFacing::NegZ.opposite(), ConnectorFacing::PosZ);
        assert_eq!(ConnectorFacing::PosY.opposite(), ConnectorFacing::NegY);
        assert_eq!(ConnectorFacing::NegY.opposite(), ConnectorFacing::PosY);
    }

    #[test]
    fn double_opposite_is_identity() {
        let all = [
            ConnectorFacing::PosX, ConnectorFacing::NegX,
            ConnectorFacing::PosZ, ConnectorFacing::NegZ,
            ConnectorFacing::PosY, ConnectorFacing::NegY,
        ];
        for f in all {
            assert_eq!(f.opposite().opposite(), f);
        }
    }

    #[test]
    fn matching_connectors_mate() {
        let a = Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX };
        let b = Connector { offset: [0, 0, 0], facing: ConnectorFacing::NegX };
        assert!(a.mates_with(&b));
        assert!(b.mates_with(&a));
    }

    #[test]
    fn same_facing_connectors_do_not_mate() {
        let a = Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX };
        let b = Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX };
        assert!(!a.mates_with(&b));
    }

    #[test]
    fn perpendicular_connectors_do_not_mate() {
        let a = Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX };
        let c = Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosZ };
        assert!(!a.mates_with(&c));
    }

    #[test]
    fn target_cell_at_origin() {
        let c = Connector { offset: [0, 0, 0], facing: ConnectorFacing::PosX };
        assert_eq!(c.target_cell([0, 0, 0]), [1, 0, 0]);
    }

    #[test]
    fn target_cell_with_offset_and_room_origin() {
        // Connector on the far side of a 2x1x1 room
        let c = Connector { offset: [1, 0, 0], facing: ConnectorFacing::PosX };
        // Room placed at grid [3, 0, 0]
        assert_eq!(c.target_cell([3, 0, 0]), [5, 0, 0]);
    }
}
