use crate::room_template::{Connector, RoomTemplate};
use petgraph::graph::NodeIndex;

/// A placed room in the level, with its position on the grid.
/// "Room" here means any flyable space: arena, corridor segment,
/// hub, vertical shaft — they're all nodes.
#[derive(Debug, Clone)]
pub struct PlacedRoom {
    pub template: RoomTemplate,
    /// Position of the room's origin on the grid.
    pub grid_pos: [i32; 3],
}

impl PlacedRoom {
    /// Convert grid position to world-space origin.
    /// XZ uses `tile_width`, Y uses `story_height` — both derived from the wall set.
    pub fn world_position(&self, tile_width: f32, story_height: f32) -> [f32; 3] {
        [
            self.grid_pos[0] as f32 * tile_width,
            self.grid_pos[1] as f32 * story_height,
            self.grid_pos[2] as f32 * tile_width,
        ]
    }
}

/// Identifies which key opens a locked door.
/// Exhaustive — adding a variant forces handling at every match site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyId {
    Red,
    Blue,
    Gold,
}

impl std::fmt::Display for KeyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyId::Red => write!(f, "red_key"),
            KeyId::Blue => write!(f, "blue_key"),
            KeyId::Gold => write!(f, "gold_key"),
        }
    }
}

/// How two nodes are connected.
#[derive(Debug, Clone)]
pub enum EdgeKind {
    /// Physically adjacent — player flies directly between them.
    /// Requires matching connectors on the grid.
    Adjacent {
        from_connector: Connector,
        to_connector: Connector,
    },
    /// Teleporter — no spatial adjacency required.
    /// Player steps on a pad/trigger and is transported.
    Teleporter,
    /// One-way connection (e.g. a chute, vent, or collapse).
    /// Stored in an undirected graph but the direction is semantic:
    /// the player can only traverse from → to.
    OneWay {
        from_connector: Connector,
    },
    /// Locked door — requires a key or trigger to open.
    Locked {
        from_connector: Connector,
        to_connector: Connector,
        key_id: KeyId,
    },
}

#[derive(Debug)]
pub enum PlaceError {
    Overlap { cell: [i32; 3], existing_room: NodeIndex },
}

#[derive(Debug)]
pub enum ConnectError {
    InvalidRoomIndex,
    NoMatchingConnectors,
}
