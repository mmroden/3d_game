//! Shared on-disk persistence via Godot's `ConfigFile`. One place for
//! the file I/O so options and save games reuse it rather than each
//! growing their own load/save boilerplate.

use godot::classes::ConfigFile;
use godot::global::Error;
use godot::prelude::*;

/// Load the config at `path`, or `None` if it is missing or unreadable.
/// Callers read individual values off the returned `ConfigFile`.
pub fn load(path: &str) -> Option<Gd<ConfigFile>> {
    let mut cfg = ConfigFile::new_gd();
    if cfg.load(path) == Error::OK {
        Some(cfg)
    } else {
        None
    }
}

/// Write `pairs` (key → value) under `section` and save to `path`.
pub fn save(path: &str, section: &str, pairs: &[(&str, Variant)]) {
    let mut cfg = ConfigFile::new_gd();
    for (key, value) in pairs {
        cfg.set_value(section, *key, value);
    }
    cfg.save(path);
}
