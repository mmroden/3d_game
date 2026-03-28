#![deny(warnings)]
#![allow(clippy::result_large_err)] // godot-rust macros generate large Result types

use godot::prelude::*;

mod nodes;
pub mod systems;
pub mod util;

struct VoidScavenger;

#[gdextension]
unsafe impl ExtensionLibrary for VoidScavenger {}
