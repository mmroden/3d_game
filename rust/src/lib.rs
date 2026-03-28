#![deny(warnings)]

use godot::prelude::*;

mod nodes;
pub mod systems;
pub mod util;

struct VoidScavenger;

#[gdextension]
unsafe impl ExtensionLibrary for VoidScavenger {}
