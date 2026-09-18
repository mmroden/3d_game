//! The OpenXR runtime as a fact of the run. Godot enables OpenXR at
//! process start — `--xr-mode on` on the command line, or the project
//! setting — and its interface initializes then, never from the menu
//! (docs/design/xr_rig.md §6). The XRServer is the one truth for whether
//! that happened; every consumer that must know (the view pipeline, the
//! HUD's band, the ship's chase gate) reads it here.

use godot::classes::{XrInterface, XrServer};
use godot::prelude::*;

use void_logic::stereo::Display;

/// The name Godot's OpenXR module registers its interface under.
const OPENXR_INTERFACE: &str = "OpenXR";

/// The OpenXR interface, when the runtime came up this run.
pub(crate) fn openxr_interface() -> Option<Gd<XrInterface>> {
    XrServer::singleton()
        .find_interface(OPENXR_INTERFACE)
        .filter(|iface| iface.is_initialized())
}

/// Whether an OpenXR runtime renders this run.
pub(crate) fn openxr_active() -> bool {
    openxr_interface().is_some()
}

/// The display in force for the saved SBS preference: the runtime when
/// it runs, else the SBS interface in the preferred mode.
pub(crate) fn active_display(sbs_enabled: bool) -> Display {
    Display::effective(sbs_enabled, openxr_active())
}
