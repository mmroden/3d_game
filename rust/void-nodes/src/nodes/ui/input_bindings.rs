//! The one reading of the InputMap: every action's bindings in the
//! engine-neutral shape `void_logic::controls` composes screens and
//! hints from, and the vocabulary of the pad in hand. The controls
//! screen reads it once at ready; GameManager reads it once for the
//! briefing's prompt. Nothing is transcribed from project.godot.

use godot::prelude::*;
use godot::classes::{
    DisplayServer, Input, InputEvent, InputEventJoypadButton, InputEventJoypadMotion,
    InputEventKey, InputEventMouseButton, InputMap, Os,
};
use godot::global::Key;
use godot::obj::EngineEnum;

use void_logic::controls::{ActionBindings, Binding, GameAction, PadFamily};

/// Every action's bindings as the InputMap holds them. A key is named
/// by its physical (US) position — the diagram's vocabulary — and
/// labelled by what the player's own layout prints there.
pub fn read_bindings() -> Vec<ActionBindings> {
    let mut map = InputMap::singleton();
    let os = Os::singleton();
    let display = DisplayServer::singleton();
    GameAction::ALL
        .iter()
        .map(|&action| {
            let events = map.action_get_events(&StringName::from(action.name()));
            let bindings = events
                .iter_shared()
                .filter_map(|event| binding_of(&event, &os, &display))
                .collect();
            ActionBindings { action, bindings }
        })
        .collect()
}

fn binding_of(event: &Gd<InputEvent>, os: &Gd<Os>, display: &Gd<DisplayServer>) -> Option<Binding> {
    if let Ok(key) = event.clone().try_cast::<InputEventKey>() {
        let mut physical = key.get_physical_keycode();
        if physical == Key::NONE {
            physical = key.get_keycode();
        }
        // The player's layout's label for the physical key — where the
        // display server knows layouts; the headless one (GUT, the
        // import stage) does not, and prints an engine error on the ask,
        // so the US name stands there.
        let mut printed = if display.get_name() == "headless" {
            Key::NONE
        } else {
            display.keyboard_get_label_from_physical(physical)
        };
        if printed == Key::NONE {
            printed = physical;
        }
        return Some(Binding::Key {
            physical: os.get_keycode_string(physical).to_string(),
            label: os.get_keycode_string(printed).to_string(),
        });
    }
    if let Ok(mouse) = event.clone().try_cast::<InputEventMouseButton>() {
        return Some(Binding::MouseButton(mouse.get_button_index().ord() as u8));
    }
    if let Ok(button) = event.clone().try_cast::<InputEventJoypadButton>() {
        return Some(Binding::PadButton(button.get_button_index().ord() as u8));
    }
    if let Ok(motion) = event.clone().try_cast::<InputEventJoypadMotion>() {
        return Some(Binding::PadAxis {
            axis: motion.get_axis().ord() as u8,
            positive: motion.get_axis_value() > 0.0,
        });
    }
    None
}

/// The vocabulary of the pad in hand — the first connected pad's, read
/// each time a screen or a hint needs it (a pad can arrive any time).
pub fn pad_family() -> PadFamily {
    let input = Input::singleton();
    match input.get_connected_joypads().iter_shared().next() {
        Some(device) => PadFamily::from_joy_name(&input.get_joy_name(device as i32).to_string()),
        None => PadFamily::Xbox,
    }
}
