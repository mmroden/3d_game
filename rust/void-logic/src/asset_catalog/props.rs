// ── Props ───────────────────────────────────────────────────────────────

/// Where a prop should be placed relative to the room geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropPlacement {
    /// Against a wall face, rotated to match wall orientation.
    WallAdjacent,
    /// In the room interior, away from walls.
    Center,
    /// At a corner where two walls meet.
    Corner,
    /// Mounted on the ceiling.
    Ceiling,
}

/// A prop that can be placed inside rooms.
#[derive(Debug, Clone, Copy)]
pub struct PropEntry {
    pub scene: &'static str,
    pub placement: PropPlacement,
    /// Whether this prop blocks flight paths through the room.
    pub blocks_flight: bool,
}

/// Whether a prop is anchored to a surface and therefore stays fixed.
///
/// In a derelict zero-g base everything floats by default, so a prop is
/// `Dynamic` unless it is mounted to a surface: wall/ceiling equipment
/// (computers, screens, vents, fans, access points), structural columns,
/// the floor teleporter pad, hanging cables, and hologram projectors. Loose
/// furniture and debris (crates, barrels, chests, desks, lockers, shelves,
/// pods, …) are not surface-mounted and tumble freely.
pub fn is_surface_mounted(scene: &str) -> bool {
    scene.contains("Computer")     // wall + ceiling computers
        || scene.contains("Screen")
        || scene.contains("Vent")
        || scene.contains("Fan")
        || scene.contains("AccessPoint")
        || scene.contains("Column") // floor-to-ceiling structure
        || scene.contains("Teleporter")
        || scene.contains("Cable")
        || scene.contains("Hologram")
}

pub const WALL_ADJACENT_PROPS: &[PropEntry] = &[
    // Megakit
    PropEntry { scene: megakit_prop!("Prop_Computer.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Vent_Big.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Vent_Small.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Vent_Wide.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Fan_Big.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Fan_Small.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_AccessPoint.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    // Essentials
    PropEntry { scene: essentials_prop!("Prop_Desk_Large.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Desk_Medium.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Desk_Small.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Desk_L.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Shelves_WideTall.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: true },
    PropEntry { scene: essentials_prop!("Prop_Shelves_WideShort.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Shelves_ThinTall.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: true },
    PropEntry { scene: essentials_prop!("Prop_Shelves_ThinShort.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Locker.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: true },
    PropEntry { scene: essentials_prop!("Prop_Screen.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_GunRack.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Computer.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
];

pub const CENTER_PROPS: &[PropEntry] = &[
    // Megakit
    PropEntry { scene: megakit_prop!("Prop_Crate1.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Crate2.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Crate3.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Crate4.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Barrel_Large.gltf"), placement: PropPlacement::Center, blocks_flight: true },
    PropEntry { scene: megakit_prop!("Prop_Barrel_Small.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Teleporter.gltf"), placement: PropPlacement::Center, blocks_flight: true },
    PropEntry { scene: megakit_prop!("Prop_Pod.gltf"), placement: PropPlacement::Center, blocks_flight: true },
    PropEntry { scene: megakit_prop!("Prop_Chest.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    // Essentials
    PropEntry { scene: essentials_prop!("Prop_Crate.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Crate_Large.gltf"), placement: PropPlacement::Center, blocks_flight: true },
    PropEntry { scene: essentials_prop!("Prop_Barrel1.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Barrel2_Closed.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_HologramMap1.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_HologramMap2.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Chest.gltf"), placement: PropPlacement::Center, blocks_flight: false },
];

pub const CORNER_PROPS: &[PropEntry] = &[
    PropEntry { scene: megakit_column!("Column_Astra.gltf"), placement: PropPlacement::Corner, blocks_flight: true },
    PropEntry { scene: megakit_column!("Column_Dark.gltf"), placement: PropPlacement::Corner, blocks_flight: true },
    PropEntry { scene: megakit_column!("Column_Simple.gltf"), placement: PropPlacement::Corner, blocks_flight: true },
    PropEntry { scene: megakit_column!("Column_Hollow.gltf"), placement: PropPlacement::Corner, blocks_flight: true },
    PropEntry { scene: megakit_column!("Column_Pipes.gltf"), placement: PropPlacement::Corner, blocks_flight: true },
    PropEntry { scene: megakit_prop!("Prop_Cable1.gltf"), placement: PropPlacement::Corner, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Cable2.gltf"), placement: PropPlacement::Corner, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Cable3.gltf"), placement: PropPlacement::Corner, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Cable4.gltf"), placement: PropPlacement::Corner, blocks_flight: false },
];

pub const CEILING_PROPS: &[PropEntry] = &[
    PropEntry { scene: essentials_prop!("Prop_CeilingComputer.gltf"), placement: PropPlacement::Ceiling, blocks_flight: false },
];

// ── Themed palette subsets ─────────────────────────────────────────────

/// Warehouse: shelves, lockers, gun racks — storage-oriented wall props.
pub const WAREHOUSE_WALL_PROPS: &[PropEntry] = &[
    PropEntry { scene: essentials_prop!("Prop_Shelves_WideTall.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: true },
    PropEntry { scene: essentials_prop!("Prop_Shelves_WideShort.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Shelves_ThinTall.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: true },
    PropEntry { scene: essentials_prop!("Prop_Shelves_ThinShort.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Locker.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: true },
    PropEntry { scene: essentials_prop!("Prop_GunRack.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
];

/// Warehouse: crates and barrels — bulk storage center props.
pub const WAREHOUSE_CENTER_PROPS: &[PropEntry] = &[
    PropEntry { scene: megakit_prop!("Prop_Crate1.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Crate2.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Crate3.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Crate4.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Barrel_Large.gltf"), placement: PropPlacement::Center, blocks_flight: true },
    PropEntry { scene: megakit_prop!("Prop_Barrel_Small.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Crate.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Crate_Large.gltf"), placement: PropPlacement::Center, blocks_flight: true },
    PropEntry { scene: essentials_prop!("Prop_Barrel1.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Barrel2_Closed.gltf"), placement: PropPlacement::Center, blocks_flight: false },
];

/// Command: computers, screens, desks — control room wall props.
pub const COMMAND_WALL_PROPS: &[PropEntry] = &[
    PropEntry { scene: megakit_prop!("Prop_Computer.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_AccessPoint.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Desk_Large.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Desk_Medium.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Desk_Small.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Desk_L.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Screen.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Computer.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
];

/// Command: hologram displays, chests — tactical center props.
pub const COMMAND_CENTER_PROPS: &[PropEntry] = &[
    PropEntry { scene: essentials_prop!("Prop_HologramMap1.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_HologramMap2.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Chest.gltf"), placement: PropPlacement::Center, blocks_flight: false },
    PropEntry { scene: essentials_prop!("Prop_Chest.gltf"), placement: PropPlacement::Center, blocks_flight: false },
];

/// Laboratory: pods, teleporters, access points — science wall props.
pub const LABORATORY_WALL_PROPS: &[PropEntry] = &[
    PropEntry { scene: megakit_prop!("Prop_AccessPoint.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Computer.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Vent_Big.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
    PropEntry { scene: megakit_prop!("Prop_Vent_Small.gltf"), placement: PropPlacement::WallAdjacent, blocks_flight: false },
];

/// Laboratory: pods, teleporters — experimental center props.
pub const LABORATORY_CENTER_PROPS: &[PropEntry] = &[
    PropEntry { scene: megakit_prop!("Prop_Teleporter.gltf"), placement: PropPlacement::Center, blocks_flight: true },
    PropEntry { scene: megakit_prop!("Prop_Pod.gltf"), placement: PropPlacement::Center, blocks_flight: true },
    PropEntry { scene: megakit_prop!("Prop_Barrel_Large.gltf"), placement: PropPlacement::Center, blocks_flight: true },
    PropEntry { scene: megakit_prop!("Prop_Barrel_Small.gltf"), placement: PropPlacement::Center, blocks_flight: false },
];
