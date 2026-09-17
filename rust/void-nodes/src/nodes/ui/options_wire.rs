//! The options broadcast's wire: `GameOptions` ↔ a Godot `Dictionary`
//! keyed by `OptionKey::name()`. GameManager emits the dictionary; every
//! consumer (menus, ViewManager, HUD, AudioManager, the ship) rebuilds
//! the one struct from it. The same entries are what options.cfg holds,
//! so a consumer, the file and the broadcast can never name a
//! preference differently.

use godot::prelude::*;

use void_logic::game_options::{GameOptions, OptionKey, OptionValue};

/// The broadcast's shape: preference name → bool or int.
pub type OptionsDictionary = Dictionary<GString, Variant>;

/// The broadcast payload.
pub fn to_dictionary(options: &GameOptions) -> OptionsDictionary {
    let mut dict = OptionsDictionary::new();
    for (key, value) in options.entries() {
        dict.set(key.name(), &to_variant(value));
    }
    dict
}

/// The options a broadcast carries; missing or mistyped keys keep the
/// defaults, exactly as a partial options file would.
pub fn from_dictionary(dict: &OptionsDictionary) -> GameOptions {
    let entries: Vec<(String, OptionValue)> = dict
        .iter_shared()
        .filter_map(|(key, value)| Some((key.to_string(), from_variant(&value)?)))
        .collect();
    GameOptions::from_entries(entries.iter().map(|(k, v)| (k.as_str(), *v)))
}

/// The pairs the options file stores (`persistence::save`).
pub fn to_pairs(options: &GameOptions) -> Vec<(&'static str, Variant)> {
    options.entries().into_iter().map(|(key, value)| (key.name(), to_variant(value))).collect()
}

/// A stored value back onto the wire (`persistence::load` readers).
pub fn from_variant(value: &Variant) -> Option<OptionValue> {
    match value.get_type() {
        VariantType::BOOL => Some(OptionValue::Bool(value.to::<bool>())),
        VariantType::INT => Some(OptionValue::Int(value.to::<i64>())),
        _ => None,
    }
}

fn to_variant(value: OptionValue) -> Variant {
    match value {
        OptionValue::Bool(b) => b.to_variant(),
        OptionValue::Int(i) => i.to_variant(),
    }
}

/// Every stored key, for a loader that reads them one by one.
pub fn keys() -> impl Iterator<Item = OptionKey> {
    OptionKey::ALL.into_iter()
}
