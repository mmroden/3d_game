//! The controls screen's content: the game's actions (the one list the
//! shell's InputMap action names come from) and the two device pages —
//! a gamepad silhouette with callouts, and a keyboard with the bound
//! keys lit — composed from the bindings the shell reads out of the
//! InputMap at runtime. Nothing here is transcribed from project.godot:
//! a rebinding changes the screen on the next boot.
//!
//! Pure data and layout arithmetic, no Godot dependency. The shell
//! (void-nodes `ControlsPanel`) turns callouts into lines and labels and
//! key caps into panels.

/// Every InputMap action the game declares. `name()` is the action's
/// InputMap name; the shell's action constants re-export it, so the
/// screen and the code that reads the stick can never disagree on a
/// name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameAction {
    MoveForward,
    MoveBack,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    LookUp,
    LookDown,
    LookLeft,
    LookRight,
    RollLeft,
    RollRight,
    Fire,
    FireSecondary,
    Stabilize,
    RouteShields,
    RouteWeapons,
    UseItem,
    ToggleView,
    OpenMenu,
    MenuUp,
    MenuDown,
    MenuLeft,
    MenuRight,
    MenuSelect,
    MenuBack,
}

impl GameAction {
    /// Every action, in the order the keyboard legend lists them.
    pub const ALL: [GameAction; 26] = [
        Self::MoveForward,
        Self::MoveBack,
        Self::MoveLeft,
        Self::MoveRight,
        Self::MoveUp,
        Self::MoveDown,
        Self::LookUp,
        Self::LookDown,
        Self::LookLeft,
        Self::LookRight,
        Self::RollLeft,
        Self::RollRight,
        Self::Fire,
        Self::FireSecondary,
        Self::Stabilize,
        Self::RouteShields,
        Self::RouteWeapons,
        Self::UseItem,
        Self::ToggleView,
        Self::OpenMenu,
        Self::MenuUp,
        Self::MenuDown,
        Self::MenuLeft,
        Self::MenuRight,
        Self::MenuSelect,
        Self::MenuBack,
    ];

    /// The InputMap action name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::MoveForward => "move_forward",
            Self::MoveBack => "move_back",
            Self::MoveLeft => "move_left",
            Self::MoveRight => "move_right",
            Self::MoveUp => "move_up",
            Self::MoveDown => "move_down",
            Self::LookUp => "look_up",
            Self::LookDown => "look_down",
            Self::LookLeft => "look_left",
            Self::LookRight => "look_right",
            Self::RollLeft => "roll_left",
            Self::RollRight => "roll_right",
            Self::Fire => "fire",
            Self::FireSecondary => "fire_secondary",
            Self::Stabilize => "stabilize",
            Self::RouteShields => "route_shields",
            Self::RouteWeapons => "route_weapons",
            Self::UseItem => "use_item",
            Self::ToggleView => "toggle_view",
            Self::OpenMenu => "open_menu",
            Self::MenuUp => "menu_up",
            Self::MenuDown => "menu_down",
            Self::MenuLeft => "menu_left",
            Self::MenuRight => "menu_right",
            Self::MenuSelect => "menu_select",
            Self::MenuBack => "menu_back",
        }
    }

    /// The player-facing name, as the keyboard legend prints it.
    pub fn label(self) -> &'static str {
        match self {
            Self::MoveForward => "Thrust forward",
            Self::MoveBack => "Thrust back",
            Self::MoveLeft => "Strafe left",
            Self::MoveRight => "Strafe right",
            Self::MoveUp => "Thrust up",
            Self::MoveDown => "Thrust down",
            Self::LookUp => "Pitch up",
            Self::LookDown => "Pitch down",
            Self::LookLeft => "Yaw left",
            Self::LookRight => "Yaw right",
            Self::RollLeft => "Roll left",
            Self::RollRight => "Roll right",
            Self::Fire => "Fire",
            Self::FireSecondary => "Secondary fire",
            Self::Stabilize => "Stabilize",
            Self::RouteShields => "Power to shields",
            Self::RouteWeapons => "Power to weapons",
            Self::UseItem => "Use item",
            Self::ToggleView => "Toggle view",
            Self::OpenMenu => "Pause",
            Self::MenuUp => "Menu up",
            Self::MenuDown => "Menu down",
            Self::MenuLeft => "Menu left",
            Self::MenuRight => "Menu right",
            Self::MenuSelect => "Menu select",
            Self::MenuBack => "Menu back",
        }
    }

    /// The name a gamepad callout prints: the four thrust directions
    /// share one stick, so they read as one word there; likewise look.
    pub fn callout_label(self) -> &'static str {
        match self {
            Self::MoveForward | Self::MoveBack | Self::MoveLeft | Self::MoveRight => "Thrust",
            Self::LookUp | Self::LookDown | Self::LookLeft | Self::LookRight => "Look",
            other => other.label(),
        }
    }

    /// Whether the controls screen shows this action at all. Menu
    /// navigation is left off the diagram — the menus explain themselves.
    pub fn on_diagram(self) -> bool {
        !matches!(
            self,
            Self::MenuUp | Self::MenuDown | Self::MenuLeft | Self::MenuRight | Self::MenuSelect | Self::MenuBack
        )
    }
}

/// One physical input the InputMap binds to an action, as the shell
/// reads it: engine-neutral, so this crate never sees an InputEvent.
#[derive(Debug, Clone, PartialEq)]
pub enum Binding {
    /// A keyboard key by its physical (US QWERTY) name, with the label
    /// printed on that key in the player's own layout.
    Key { physical: String, label: String },
    /// A mouse button, 1 = left, 2 = right, 3 = middle.
    MouseButton(u8),
    /// A gamepad button by its SDL index (Godot's `JoyButton`).
    PadButton(u8),
    /// A gamepad axis by its SDL index (Godot's `JoyAxis`) and the
    /// direction the action listens to.
    PadAxis { axis: u8, positive: bool },
}

/// An action and everything the InputMap binds to it.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionBindings {
    pub action: GameAction,
    pub bindings: Vec<Binding>,
}

/// The two pages of the screen. Order is the paging order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Gamepad,
    Keyboard,
}

impl Page {
    pub const ALL: [Page; 2] = [Self::Gamepad, Self::Keyboard];

    /// The page's heading.
    pub fn title(self) -> &'static str {
        match self {
            Self::Gamepad => "Controller",
            Self::Keyboard => "Keyboard & Mouse",
        }
    }

    /// The page after this one, wrapping.
    pub fn next(self) -> Page {
        Self::from_index((self.index() + 1) % Self::ALL.len())
    }

    /// The page before this one, wrapping.
    pub fn prev(self) -> Page {
        Self::from_index((self.index() + Self::ALL.len() - 1) % Self::ALL.len())
    }

    /// The page's position in the paging order (the capture door's pose).
    pub fn index(self) -> usize {
        match self {
            Self::Gamepad => 0,
            Self::Keyboard => 1,
        }
    }

    /// The page at `index`; out of range lands on the first page.
    pub fn from_index(index: usize) -> Page {
        Self::ALL.get(index).copied().unwrap_or(Self::Gamepad)
    }
}

/// The two label vocabularies the silhouette speaks. Godot reports every
/// pad on one SDL layout; only the printed names differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadFamily {
    Xbox,
    PlayStation,
}

impl PadFamily {
    /// Read the family off the pad's reported name; anything that is not
    /// recognisably Sony is labelled the SDL (Xbox) way.
    pub fn from_joy_name(name: &str) -> PadFamily {
        let lower = name.to_ascii_lowercase();
        let sony = ["ps5", "ps4", "ps3", "dualsense", "dualshock", "playstation", "sony"];
        if sony.iter().any(|tag| lower.contains(tag)) {
            Self::PlayStation
        } else {
            Self::Xbox
        }
    }
}

/// One physical control on the silhouette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PadControl {
    LeftStick,
    RightStick,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    South,
    East,
    West,
    North,
    Back,
    Guide,
    Start,
    LeftStickClick,
    RightStickClick,
    LeftShoulder,
    RightShoulder,
    LeftTrigger,
    RightTrigger,
}

/// Which side of the silhouette a control's callout label sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl PadControl {
    /// The control a binding lands on, or `None` for an index the
    /// silhouette does not draw (paddles, touchpad) and for keys.
    pub fn for_binding(binding: &Binding) -> Option<PadControl> {
        // Godot's JoyButton / JoyAxis numbering (core/input/input_enums.h).
        match binding {
            Binding::PadButton(index) => match index {
                0 => Some(Self::South),
                1 => Some(Self::East),
                2 => Some(Self::West),
                3 => Some(Self::North),
                4 => Some(Self::Back),
                5 => Some(Self::Guide),
                6 => Some(Self::Start),
                7 => Some(Self::LeftStickClick),
                8 => Some(Self::RightStickClick),
                9 => Some(Self::LeftShoulder),
                10 => Some(Self::RightShoulder),
                11 => Some(Self::DpadUp),
                12 => Some(Self::DpadDown),
                13 => Some(Self::DpadLeft),
                14 => Some(Self::DpadRight),
                _ => None,
            },
            Binding::PadAxis { axis, .. } => match axis {
                0 | 1 => Some(Self::LeftStick),
                2 | 3 => Some(Self::RightStick),
                4 => Some(Self::LeftTrigger),
                5 => Some(Self::RightTrigger),
                _ => None,
            },
            Binding::Key { .. } | Binding::MouseButton(_) => None,
        }
    }

    /// Where the control sits on the silhouette, as fractions of the
    /// drawn rect's width and height.
    pub fn anchor(self) -> [f32; 2] {
        // Read off the silhouette as Godot draws it (Wikimedia "Xbox
        // Controller.svg", KEEP_ASPECT_CENTERED in its rect, measured on
        // a KEEP_FRAMES capture 2026-09-16): each control's centre as a
        // fraction of the rect. A stick's click shares the stick's point.
        match self {
            Self::LeftStick | Self::LeftStickClick => [0.216, 0.494],
            Self::RightStick | Self::RightStickClick => [0.622, 0.68],
            Self::DpadUp => [0.362, 0.61],
            Self::DpadDown => [0.362, 0.75],
            Self::DpadLeft => [0.316, 0.68],
            Self::DpadRight => [0.408, 0.68],
            Self::South => [0.772, 0.574],
            Self::East => [0.848, 0.485],
            Self::West => [0.698, 0.485],
            Self::North => [0.772, 0.396],
            Self::Back => [0.392, 0.485],
            Self::Guide => [0.5, 0.47],
            Self::Start => [0.6, 0.485],
            Self::LeftShoulder => [0.266, 0.232],
            Self::LeftTrigger => [0.25, 0.122],
            Self::RightShoulder => [0.752, 0.232],
            Self::RightTrigger => [0.752, 0.122],
        }
    }

    /// The side its callout label reads from.
    pub fn side(self) -> Side {
        match self {
            Self::LeftStick
            | Self::LeftStickClick
            | Self::DpadUp
            | Self::DpadDown
            | Self::DpadLeft
            | Self::DpadRight
            | Self::Back
            | Self::Guide
            | Self::LeftShoulder
            | Self::LeftTrigger => Side::Left,
            Self::RightStick
            | Self::RightStickClick
            | Self::South
            | Self::East
            | Self::West
            | Self::North
            | Self::Start
            | Self::RightShoulder
            | Self::RightTrigger => Side::Right,
        }
    }

    /// The name printed for this control in a family's vocabulary.
    pub fn name(self, family: PadFamily) -> &'static str {
        let xbox = family == PadFamily::Xbox;
        match self {
            Self::LeftStick => "Left stick",
            Self::RightStick => "Right stick",
            Self::DpadUp => "D-pad up",
            Self::DpadDown => "D-pad down",
            Self::DpadLeft => "D-pad left",
            Self::DpadRight => "D-pad right",
            Self::South => if xbox { "A" } else { "Cross" },
            Self::East => if xbox { "B" } else { "Circle" },
            Self::West => if xbox { "X" } else { "Square" },
            Self::North => if xbox { "Y" } else { "Triangle" },
            Self::Back => if xbox { "Back" } else { "Create" },
            Self::Guide => if xbox { "Xbox" } else { "PS" },
            Self::Start => if xbox { "Start" } else { "Options" },
            Self::LeftStickClick => if xbox { "LS click" } else { "L3" },
            Self::RightStickClick => if xbox { "RS click" } else { "R3" },
            Self::LeftShoulder => if xbox { "LB" } else { "L1" },
            Self::RightShoulder => if xbox { "RB" } else { "R1" },
            Self::LeftTrigger => if xbox { "LT" } else { "L2" },
            Self::RightTrigger => if xbox { "RT" } else { "R2" },
        }
    }

    /// What a hint prints for this control: the face buttons' glyphs in
    /// the family's vocabulary (circled letters for Xbox; the circled X
    /// the house already uses for Cross, then Circle, Square, Triangle
    /// for PlayStation), the name for everything else.
    pub fn glyph(self, family: PadFamily) -> &'static str {
        let xbox = family == PadFamily::Xbox;
        match self {
            Self::South => if xbox { "\u{24B6}" } else { "\u{24CD}" },
            Self::East => if xbox { "\u{24B7}" } else { "\u{24C4}" },
            Self::West => if xbox { "\u{24CD}" } else { "\u{25A1}" },
            Self::North => if xbox { "\u{24CE}" } else { "\u{25B3}" },
            other => other.name(family),
        }
    }
}

/// One callout on the gamepad page: a control, its printed name, and the
/// actions bound to it. `slot` is the label's row on its side, counted
/// from the top, so labels stack without overlapping.
#[derive(Debug, Clone, PartialEq)]
pub struct Callout {
    pub control: PadControl,
    pub anchor: [f32; 2],
    pub side: Side,
    pub slot: usize,
    pub name: String,
    pub text: String,
}

/// The gamepad page: one callout per drawn control that has at least one
/// shown action bound, labels stacked top-to-bottom on each side.
pub fn gamepad_callouts(bindings: &[ActionBindings], family: PadFamily) -> Vec<Callout> {
    // Group the shown actions by the control they land on, in binding
    // order, so a button carrying two actions reads them in the order
    // the InputMap declares them.
    let mut grouped: Vec<(PadControl, Vec<GameAction>)> = Vec::new();
    for entry in bindings.iter().filter(|b| b.action.on_diagram()) {
        for control in entry.bindings.iter().filter_map(PadControl::for_binding) {
            match grouped.iter_mut().find(|(c, _)| *c == control) {
                Some((_, actions)) => {
                    if !actions.contains(&entry.action) {
                        actions.push(entry.action);
                    }
                }
                None => grouped.push((control, vec![entry.action])),
            }
        }
    }
    let mut callouts: Vec<Callout> = grouped
        .into_iter()
        .map(|(control, actions)| {
            let mut words: Vec<&str> = Vec::new();
            for word in actions.iter().map(|a| a.callout_label()) {
                if !words.contains(&word) {
                    words.push(word);
                }
            }
            Callout {
                control,
                anchor: control.anchor(),
                side: control.side(),
                slot: 0,
                name: control.name(family).to_string(),
                text: words.join(" / "),
            }
        })
        .collect();
    // Labels stack down each side in the order their controls sit on the
    // silhouette, so a line never has to cross another.
    for side in [Side::Left, Side::Right] {
        let mut on_side: Vec<usize> = (0..callouts.len()).filter(|&i| callouts[i].side == side).collect();
        on_side.sort_by(|&a, &b| {
            let (pa, pb) = (callouts[a].anchor, callouts[b].anchor);
            pa[1].total_cmp(&pb[1]).then(pa[0].total_cmp(&pb[0]))
        });
        for (slot, i) in on_side.into_iter().enumerate() {
            callouts[i].slot = slot;
        }
    }
    callouts
}

/// One key cap on the keyboard page, in key units (1 = one letter key).
#[derive(Debug, Clone, PartialEq)]
pub struct KeyCap {
    /// The physical key's US name — the InputMap's vocabulary.
    pub physical: String,
    /// What the cap prints: the player's layout label for a bound key,
    /// the US name otherwise.
    pub label: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub actions: Vec<GameAction>,
}

/// The drawn keyboard: a US ANSI board from Esc through the arrow
/// cluster, no function row, no numpad, plus the mouse buttons as two
/// caps beside it. Positions and widths in key units. Physical names are
/// Godot's keycode strings for the US layout — the InputMap's vocabulary.
pub struct KeyboardLayout;

impl KeyboardLayout {
    /// The rows, top to bottom, each starting at x = 0.
    const ROWS: [&'static [(&'static str, f32)]; 6] = [
        &[("Escape", 1.0)],
        &[
            ("QuoteLeft", 1.0), ("1", 1.0), ("2", 1.0), ("3", 1.0), ("4", 1.0), ("5", 1.0),
            ("6", 1.0), ("7", 1.0), ("8", 1.0), ("9", 1.0), ("0", 1.0), ("Minus", 1.0),
            ("Equal", 1.0), ("BackSpace", 2.0),
        ],
        &[
            ("Tab", 1.5), ("Q", 1.0), ("W", 1.0), ("E", 1.0), ("R", 1.0), ("T", 1.0), ("Y", 1.0),
            ("U", 1.0), ("I", 1.0), ("O", 1.0), ("P", 1.0), ("BracketLeft", 1.0),
            ("BracketRight", 1.0), ("BackSlash", 1.5),
        ],
        &[
            ("CapsLock", 1.75), ("A", 1.0), ("S", 1.0), ("D", 1.0), ("F", 1.0), ("G", 1.0),
            ("H", 1.0), ("J", 1.0), ("K", 1.0), ("L", 1.0), ("Semicolon", 1.0),
            ("Apostrophe", 1.0), ("Enter", 2.25),
        ],
        &[
            ("Shift", 2.25), ("Z", 1.0), ("X", 1.0), ("C", 1.0), ("V", 1.0), ("B", 1.0),
            ("N", 1.0), ("M", 1.0), ("Comma", 1.0), ("Period", 1.0), ("Slash", 1.0),
            ("Shift", 2.75),
        ],
        &[
            ("Ctrl", 1.25), ("Meta", 1.25), ("Alt", 1.25), ("Space", 5.25), ("Alt", 1.5),
            ("Meta", 1.5), ("Menu", 1.5), ("Ctrl", 1.5),
        ],
    ];
    /// Keys placed by hand, to the right of the rows: (name, x, y).
    const PLACED: [(&'static str, f32, f32); 6] = [
        ("Up", 16.25, 4.0),
        ("Left", 15.25, 5.0),
        ("Down", 16.25, 5.0),
        ("Right", 17.25, 5.0),
        ("Mouse1", 19.25, 2.0),
        ("Mouse2", 20.5, 2.0),
    ];
    pub const WIDTH: f32 = 21.5;
    pub const HEIGHT: f32 = 6.0;

    /// Every cap as (physical name, x, y, width).
    fn caps() -> impl Iterator<Item = (&'static str, f32, f32, f32)> {
        let rows = Self::ROWS.iter().enumerate().flat_map(|(row, keys)| {
            let mut x = 0.0;
            keys.iter().map(move |&(name, width)| {
                let at = x;
                x += width;
                (name, at, row as f32, width)
            })
        });
        rows.chain(Self::PLACED.iter().map(|&(name, x, y)| (name, x, y, 1.0)))
    }

    /// What a cap prints for a key name: the name itself, shortened
    /// where Godot's is too long for one key.
    pub fn short_label(name: &str) -> String {
        match name {
            "Escape" => "Esc",
            "BackSpace" => "Bksp",
            "CapsLock" => "Caps",
            "QuoteLeft" => "`",
            "Minus" => "-",
            "Equal" => "=",
            "BracketLeft" => "[",
            "BracketRight" => "]",
            "BackSlash" => "\\",
            "Semicolon" => ";",
            "Apostrophe" => "'",
            "Comma" => ",",
            "Period" => ".",
            "Slash" => "/",
            "Up" => "\u{25B2}",
            "Down" => "\u{25BC}",
            "Left" => "\u{25C0}",
            "Right" => "\u{25B6}",
            "Mouse1" => "LMB",
            "Mouse2" => "RMB",
            "Mouse3" => "MMB",
            other => other,
        }
        .to_string()
    }
}

/// The keyboard region the screen draws — Esc through the arrow cluster,
/// no function row, no numpad — every key, the bound ones carrying their
/// actions. Mouse buttons follow as caps of their own to the right.
pub fn keyboard_caps(bindings: &[ActionBindings]) -> Vec<KeyCap> {
    let mut caps: Vec<KeyCap> = KeyboardLayout::caps()
        .map(|(physical, x, y, width)| KeyCap {
            physical: physical.to_string(),
            label: KeyboardLayout::short_label(physical),
            x,
            y,
            width,
            actions: Vec::new(),
        })
        .collect();
    for entry in bindings.iter().filter(|b| b.action.on_diagram()) {
        for binding in &entry.bindings {
            let (physical, label) = match binding {
                Binding::Key { physical, label } => (physical.clone(), Some(label.clone())),
                Binding::MouseButton(button) => (format!("Mouse{button}"), None),
                Binding::PadButton(_) | Binding::PadAxis { .. } => continue,
            };
            for cap in caps.iter_mut().filter(|c| c.physical == physical) {
                if !cap.actions.contains(&entry.action) {
                    cap.actions.push(entry.action);
                }
                if let Some(label) = &label {
                    cap.label = KeyboardLayout::short_label(label);
                }
            }
        }
    }
    caps
}

/// Width of the keyboard region in key units, mouse caps included.
pub fn keyboard_width() -> f32 {
    KeyboardLayout::WIDTH
}

/// Height of the keyboard region in key units.
pub fn keyboard_height() -> f32 {
    KeyboardLayout::HEIGHT
}

/// How a hint names an action: the pad's glyph for its button (in the
/// family's vocabulary) beside the keyboard's key — "Ⓧ / Enter" — or
/// whichever of the two the InputMap binds. Never a hardcoded glyph: a
/// keyboard player is told the key, a pad player the button in hand.
pub fn action_prompt(action: GameAction, bindings: &[ActionBindings], family: PadFamily) -> String {
    let Some(entry) = bindings.iter().find(|b| b.action == action) else {
        return String::new();
    };
    let pad = entry.bindings.iter().find_map(|binding| match binding {
        Binding::PadButton(_) | Binding::PadAxis { .. } => {
            PadControl::for_binding(binding).map(|c| c.glyph(family).to_string())
        }
        Binding::Key { .. } | Binding::MouseButton(_) => None,
    });
    let key = entry.bindings.iter().find_map(|binding| match binding {
        Binding::Key { label, .. } => Some(KeyboardLayout::short_label(label)),
        Binding::MouseButton(button) => Some(KeyboardLayout::short_label(&format!("Mouse{button}"))),
        Binding::PadButton(_) | Binding::PadAxis { .. } => None,
    });
    [pad, key].into_iter().flatten().collect::<Vec<_>>().join(" / ")
}

/// The controls screen's own hint line: the arrows that page between
/// the devices (the arrow keys' caps and the d-pad wear the same
/// glyphs) and the prompt that closes it, on both devices.
pub fn screen_hint(bindings: &[ActionBindings], family: PadFamily) -> String {
    let close = action_prompt(GameAction::MenuSelect, bindings, family);
    format!("\u{25C0} \u{25B6}  switch device     \u{2022}     {close}  Close")
}

/// The legend under the keyboard: each lit cap's label and the actions
/// it carries, in reading order.
pub fn keyboard_legend(caps: &[KeyCap]) -> Vec<(String, String)> {
    let mut lit: Vec<&KeyCap> = caps.iter().filter(|c| !c.actions.is_empty()).collect();
    lit.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    lit.iter()
        .map(|cap| {
            let text = cap.actions.iter().map(|a| a.label()).collect::<Vec<_>>().join(" / ");
            (cap.label.clone(), text)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(physical: &str) -> Binding {
        Binding::Key { physical: physical.to_string(), label: physical.to_string() }
    }

    fn bound(action: GameAction, bindings: Vec<Binding>) -> ActionBindings {
        ActionBindings { action, bindings }
    }

    /// The project's own bindings, in the shape the shell hands over.
    fn project_bindings() -> Vec<ActionBindings> {
        vec![
            bound(GameAction::MoveForward, vec![key("W"), Binding::PadAxis { axis: 1, positive: false }]),
            bound(GameAction::MoveBack, vec![key("S"), Binding::PadAxis { axis: 1, positive: true }]),
            bound(GameAction::MoveLeft, vec![key("A"), Binding::PadAxis { axis: 0, positive: false }]),
            bound(GameAction::MoveRight, vec![key("D"), Binding::PadAxis { axis: 0, positive: true }]),
            bound(GameAction::MoveUp, vec![key("R"), Binding::PadButton(3)]),
            bound(GameAction::MoveDown, vec![key("F"), Binding::PadButton(0)]),
            bound(GameAction::LookUp, vec![key("Up"), Binding::PadAxis { axis: 3, positive: false }]),
            bound(GameAction::LookDown, vec![key("Down"), Binding::PadAxis { axis: 3, positive: true }]),
            bound(GameAction::LookLeft, vec![key("Left"), Binding::PadAxis { axis: 2, positive: false }]),
            bound(GameAction::LookRight, vec![key("Right"), Binding::PadAxis { axis: 2, positive: true }]),
            bound(GameAction::RollLeft, vec![key("Q"), Binding::PadButton(13)]),
            bound(GameAction::RollRight, vec![key("E"), Binding::PadButton(14)]),
            bound(GameAction::Fire, vec![key("Space"), Binding::MouseButton(1), Binding::PadAxis { axis: 5, positive: true }]),
            bound(GameAction::FireSecondary, vec![Binding::MouseButton(2), Binding::PadAxis { axis: 4, positive: true }]),
            bound(GameAction::Stabilize, vec![key("Tab"), Binding::PadButton(4)]),
            bound(GameAction::RouteShields, vec![key("Z"), Binding::PadButton(2)]),
            bound(GameAction::RouteWeapons, vec![key("X"), Binding::PadButton(1)]),
            bound(GameAction::UseItem, vec![key("C"), Binding::PadButton(9)]),
            bound(GameAction::ToggleView, vec![key("V"), Binding::PadButton(10)]),
            bound(GameAction::OpenMenu, vec![key("Escape"), Binding::PadButton(6)]),
            bound(GameAction::MenuUp, vec![key("Up"), key("W"), Binding::PadButton(11)]),
            bound(GameAction::MenuDown, vec![key("Down"), key("S"), Binding::PadButton(12)]),
            bound(GameAction::MenuLeft, vec![key("Left"), key("A"), Binding::PadButton(13)]),
            bound(GameAction::MenuRight, vec![key("Right"), key("D"), Binding::PadButton(14)]),
            bound(GameAction::MenuSelect, vec![key("Enter"), key("Space"), Binding::PadButton(0)]),
            bound(GameAction::MenuBack, vec![key("Escape"), Binding::PadButton(1)]),
        ]
    }

    // --- The action catalog ---

    #[test]
    fn every_action_has_a_distinct_name_and_a_label() {
        let mut names: Vec<&str> = GameAction::ALL.iter().map(|a| a.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), GameAction::ALL.len(), "action names collide");
        for action in GameAction::ALL {
            assert!(!action.label().is_empty(), "{action:?} has no label");
            assert!(!action.callout_label().is_empty(), "{action:?} has no callout label");
            assert!(
                action.name().chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "{action:?}: InputMap names are snake_case"
            );
        }
    }

    #[test]
    fn menu_navigation_stays_off_the_diagram_but_the_pause_button_is_on_it() {
        for action in [
            GameAction::MenuUp, GameAction::MenuDown, GameAction::MenuLeft,
            GameAction::MenuRight, GameAction::MenuSelect, GameAction::MenuBack,
        ] {
            assert!(!action.on_diagram(), "{action:?} is menu navigation");
        }
        assert!(GameAction::OpenMenu.on_diagram(), "the player must learn how to pause");
        assert!(GameAction::Fire.on_diagram());
        assert!(GameAction::Stabilize.on_diagram());
    }

    #[test]
    fn the_thrust_and_look_directions_share_one_callout_word() {
        assert_eq!(GameAction::MoveForward.callout_label(), GameAction::MoveBack.callout_label());
        assert_eq!(GameAction::MoveLeft.callout_label(), GameAction::MoveRight.callout_label());
        assert_eq!(GameAction::LookUp.callout_label(), GameAction::LookRight.callout_label());
        assert_ne!(GameAction::MoveForward.callout_label(), GameAction::LookUp.callout_label());
        // The keyboard legend, key by key, keeps the direction.
        assert_ne!(GameAction::MoveForward.label(), GameAction::MoveBack.label());
    }

    // --- Pages ---

    #[test]
    fn pages_wrap_both_ways_and_round_trip_their_index() {
        assert_eq!(Page::Gamepad.next(), Page::Keyboard);
        assert_eq!(Page::Keyboard.next(), Page::Gamepad);
        assert_eq!(Page::Gamepad.prev(), Page::Keyboard);
        for page in Page::ALL {
            assert_eq!(Page::from_index(page.index()), page);
            assert!(!page.title().is_empty());
        }
        assert_eq!(Page::from_index(99), Page::Gamepad, "an out-of-range index lands on the first page");
    }

    // --- The pad family ---

    #[test]
    fn sony_pads_read_as_playstation_and_everything_else_as_xbox() {
        for name in ["PS5 Controller", "DualSense Wireless Controller", "Sony Interactive Entertainment Wireless Controller", "PS4 Controller", "DualShock 4"] {
            assert_eq!(PadFamily::from_joy_name(name), PadFamily::PlayStation, "{name}");
        }
        for name in ["Xbox Series X Controller", "Xbox 360 Controller", "8BitDo Pro 2", "", "Nintendo Switch Pro Controller"] {
            assert_eq!(PadFamily::from_joy_name(name), PadFamily::Xbox, "{name}");
        }
    }

    #[test]
    fn the_face_buttons_wear_each_familys_names() {
        assert_eq!(PadControl::South.name(PadFamily::Xbox), "A");
        assert_eq!(PadControl::South.name(PadFamily::PlayStation), "Cross");
        assert_eq!(PadControl::North.name(PadFamily::PlayStation), "Triangle");
        assert_eq!(PadControl::LeftShoulder.name(PadFamily::Xbox), "LB");
        assert_eq!(PadControl::LeftShoulder.name(PadFamily::PlayStation), "L1");
        assert_eq!(PadControl::RightTrigger.name(PadFamily::PlayStation), "R2");
        assert_eq!(PadControl::Back.name(PadFamily::PlayStation), "Create");
        assert_eq!(PadControl::Start.name(PadFamily::PlayStation), "Options");
        assert_eq!(PadControl::Guide.name(PadFamily::Xbox), "Xbox");
        assert_eq!(PadControl::Guide.name(PadFamily::PlayStation), "PS");
    }

    // --- The silhouette ---

    #[test]
    fn sdl_indices_land_on_the_drawn_controls() {
        // Godot's JoyButton / JoyAxis numbering (core/input/input_enums.h).
        let cases = [
            (Binding::PadButton(0), Some(PadControl::South)),
            (Binding::PadButton(1), Some(PadControl::East)),
            (Binding::PadButton(2), Some(PadControl::West)),
            (Binding::PadButton(3), Some(PadControl::North)),
            (Binding::PadButton(4), Some(PadControl::Back)),
            (Binding::PadButton(5), Some(PadControl::Guide)),
            (Binding::PadButton(6), Some(PadControl::Start)),
            (Binding::PadButton(7), Some(PadControl::LeftStickClick)),
            (Binding::PadButton(8), Some(PadControl::RightStickClick)),
            (Binding::PadButton(9), Some(PadControl::LeftShoulder)),
            (Binding::PadButton(10), Some(PadControl::RightShoulder)),
            (Binding::PadButton(11), Some(PadControl::DpadUp)),
            (Binding::PadButton(12), Some(PadControl::DpadDown)),
            (Binding::PadButton(13), Some(PadControl::DpadLeft)),
            (Binding::PadButton(14), Some(PadControl::DpadRight)),
            (Binding::PadButton(15), None),
            (Binding::PadButton(20), None),
            (Binding::PadAxis { axis: 0, positive: true }, Some(PadControl::LeftStick)),
            (Binding::PadAxis { axis: 1, positive: false }, Some(PadControl::LeftStick)),
            (Binding::PadAxis { axis: 2, positive: true }, Some(PadControl::RightStick)),
            (Binding::PadAxis { axis: 3, positive: false }, Some(PadControl::RightStick)),
            (Binding::PadAxis { axis: 4, positive: true }, Some(PadControl::LeftTrigger)),
            (Binding::PadAxis { axis: 5, positive: true }, Some(PadControl::RightTrigger)),
            (Binding::PadAxis { axis: 6, positive: true }, None),
            (key("W"), None),
            (Binding::MouseButton(1), None),
        ];
        for (binding, expected) in cases {
            assert_eq!(PadControl::for_binding(&binding), expected, "{binding:?}");
        }
    }

    #[test]
    fn every_drawn_control_sits_inside_the_image_on_its_labelled_side() {
        let all = [
            PadControl::LeftStick, PadControl::RightStick, PadControl::DpadUp, PadControl::DpadDown,
            PadControl::DpadLeft, PadControl::DpadRight, PadControl::South, PadControl::East,
            PadControl::West, PadControl::North, PadControl::Back, PadControl::Guide, PadControl::Start,
            PadControl::LeftStickClick, PadControl::RightStickClick, PadControl::LeftShoulder,
            PadControl::RightShoulder, PadControl::LeftTrigger, PadControl::RightTrigger,
        ];
        for control in all {
            let [x, y] = control.anchor();
            assert!((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y), "{control:?} anchor {x},{y}");
            // A label reads from the side its control is nearer to; the
            // middle band (start/back/guide) may go either way.
            match control.side() {
                Side::Left => assert!(x < 0.6, "{control:?} labelled left but sits at x={x}"),
                Side::Right => assert!(x > 0.4, "{control:?} labelled right but sits at x={x}"),
            }
            for family in [PadFamily::Xbox, PadFamily::PlayStation] {
                assert!(!control.name(family).is_empty(), "{control:?} unnamed for {family:?}");
            }
        }
    }

    // --- The gamepad page ---

    #[test]
    fn the_gamepad_page_has_one_callout_per_bound_control_with_its_actions_joined() {
        let callouts = gamepad_callouts(&project_bindings(), PadFamily::PlayStation);
        let by_control = |c: PadControl| {
            callouts.iter().find(|k| k.control == c).unwrap_or_else(|| panic!("no callout for {c:?}"))
        };
        assert_eq!(by_control(PadControl::LeftStick).text, "Thrust");
        assert_eq!(by_control(PadControl::RightStick).text, "Look");
        assert_eq!(by_control(PadControl::RightTrigger).text, "Fire");
        assert_eq!(by_control(PadControl::RightTrigger).name, "R2");
        assert_eq!(by_control(PadControl::North).text, GameAction::MoveUp.label());
        assert_eq!(by_control(PadControl::North).name, "Triangle");
        assert_eq!(by_control(PadControl::Start).text, GameAction::OpenMenu.label());
        // The d-pad edges carry roll; the menu directions on them stay off.
        assert_eq!(by_control(PadControl::DpadLeft).text, GameAction::RollLeft.label());
        // Controls bound only to menu navigation get no callout at all.
        assert!(callouts.iter().all(|k| k.control != PadControl::DpadUp), "d-pad up is menu-only");
        // South carries thrust down (menu select stays off).
        assert_eq!(by_control(PadControl::South).text, GameAction::MoveDown.label());
        // One callout per control, never two.
        let mut controls: Vec<PadControl> = callouts.iter().map(|k| k.control).collect();
        controls.sort();
        controls.dedup();
        assert_eq!(controls.len(), callouts.len(), "a control appears twice");
    }

    #[test]
    fn two_actions_on_one_button_read_as_one_joined_callout() {
        let bindings = vec![
            bound(GameAction::Stabilize, vec![Binding::PadButton(4)]),
            bound(GameAction::RollLeft, vec![Binding::PadButton(4)]),
        ];
        let callouts = gamepad_callouts(&bindings, PadFamily::Xbox);
        assert_eq!(callouts.len(), 1);
        assert_eq!(callouts[0].name, "Back");
        assert_eq!(
            callouts[0].text,
            format!("{} / {}", GameAction::Stabilize.label(), GameAction::RollLeft.label())
        );
    }

    #[test]
    fn callout_slots_stack_top_to_bottom_without_gaps_on_each_side() {
        let callouts = gamepad_callouts(&project_bindings(), PadFamily::Xbox);
        for side in [Side::Left, Side::Right] {
            let mut on_side: Vec<&Callout> = callouts.iter().filter(|k| k.side == side).collect();
            assert!(!on_side.is_empty(), "{side:?} has callouts");
            on_side.sort_by_key(|k| k.slot);
            for (i, k) in on_side.iter().enumerate() {
                assert_eq!(k.slot, i, "{side:?} slots are 0..n");
            }
            // Slot order follows the anchor's height: the topmost control
            // takes the topmost label.
            for pair in on_side.windows(2) {
                assert!(pair[0].anchor[1] <= pair[1].anchor[1],
                    "{side:?}: {:?} above {:?} but slotted below", pair[1].control, pair[0].control);
            }
        }
    }

    // --- The keyboard page ---

    #[test]
    fn the_keyboard_region_lays_every_key_inside_its_extent_without_overlap() {
        let caps = keyboard_caps(&[]);
        assert!(caps.len() > 60, "a real keyboard region, not a sketch: {}", caps.len());
        let width = keyboard_width();
        let height = keyboard_height();
        for cap in &caps {
            assert!(cap.width >= 1.0, "{}: narrower than a key", cap.physical);
            assert!(cap.x >= 0.0 && cap.x + cap.width <= width + 1e-3, "{} runs off the right", cap.physical);
            assert!(cap.y >= 0.0 && cap.y + 1.0 <= height + 1e-3, "{} runs off the bottom", cap.physical);
            assert!(!cap.label.is_empty(), "{} prints nothing", cap.physical);
            assert!(cap.actions.is_empty(), "nothing bound, nothing lit");
        }
        for (i, a) in caps.iter().enumerate() {
            for b in &caps[i + 1..] {
                let same_row = (a.y - b.y).abs() < 1e-3;
                let overlap = a.x < b.x + b.width - 1e-3 && b.x < a.x + a.width - 1e-3;
                assert!(!(same_row && overlap), "{} and {} overlap", a.physical, b.physical);
            }
        }
    }

    #[test]
    fn bound_keys_light_up_with_their_actions_and_the_players_label() {
        let mut bindings = project_bindings();
        // A French layout prints Z where the physical W sits.
        bindings[0].bindings[0] = Binding::Key { physical: "W".into(), label: "Z".into() };
        let caps = keyboard_caps(&bindings);
        let cap = |name: &str| caps.iter().find(|c| c.physical == name).unwrap_or_else(|| panic!("no {name} cap"));
        assert_eq!(cap("W").actions, vec![GameAction::MoveForward], "menu-up on W stays off");
        assert_eq!(cap("W").label, "Z", "the cap prints the player's own label");
        assert_eq!(cap("Space").actions, vec![GameAction::Fire]);
        assert_eq!(cap("Escape").actions, vec![GameAction::OpenMenu]);
        assert_eq!(cap("Up").actions, vec![GameAction::LookUp]);
        assert!(cap("G").actions.is_empty(), "an unbound key stays dark");
        assert_eq!(cap("Escape").label, "Esc", "long names shorten to fit a cap");
        // Both shift caps exist; neither is bound.
        assert_eq!(caps.iter().filter(|c| c.physical == "Shift").count(), 2);
    }

    #[test]
    fn mouse_buttons_are_caps_of_their_own_beside_the_keyboard() {
        let caps = keyboard_caps(&project_bindings());
        let lmb = caps.iter().find(|c| c.physical == "Mouse1").expect("left mouse cap");
        let rmb = caps.iter().find(|c| c.physical == "Mouse2").expect("right mouse cap");
        assert_eq!(lmb.actions, vec![GameAction::Fire]);
        assert_eq!(rmb.actions, vec![GameAction::FireSecondary]);
        let keys_right_edge = caps.iter().filter(|c| !c.physical.starts_with("Mouse"))
            .map(|c| c.x + c.width).fold(0.0_f32, f32::max);
        assert!(lmb.x >= keys_right_edge, "the mouse sits to the right of the keys");
        assert!(lmb.x + lmb.width <= keyboard_width() + 1e-3);
    }

    #[test]
    fn a_prompt_names_the_action_on_both_devices_with_the_pads_own_glyph() {
        // Owner 2026-09-16: "the glyph needs to match the action … the
        // 'close' button should also include the keyboard (a kind of
        // (X)/Enter to close)". Menu select is Cross on a DualSense (SDL
        // south), A on an Xbox pad, Enter on the keyboard.
        let bindings = project_bindings();
        assert_eq!(action_prompt(GameAction::MenuSelect, &bindings, PadFamily::PlayStation), "\u{24CD} / Enter");
        assert_eq!(action_prompt(GameAction::MenuSelect, &bindings, PadFamily::Xbox), "\u{24B6} / Enter");
        assert_eq!(action_prompt(GameAction::MenuBack, &bindings, PadFamily::PlayStation), "\u{24C4} / Esc");
        // A pad-only or key-only action prompts with what it has.
        assert_eq!(action_prompt(GameAction::FireSecondary, &bindings, PadFamily::Xbox), "LT / RMB");
        let key_only = vec![bound(GameAction::UseItem, vec![key("C")])];
        assert_eq!(action_prompt(GameAction::UseItem, &key_only, PadFamily::Xbox), "C");
        assert_eq!(action_prompt(GameAction::Fire, &key_only, PadFamily::Xbox), "", "unbound: nothing to say");
    }

    #[test]
    fn the_face_buttons_have_glyphs_in_each_familys_vocabulary() {
        assert_eq!(PadControl::South.glyph(PadFamily::PlayStation), "\u{24CD}");
        assert_eq!(PadControl::East.glyph(PadFamily::PlayStation), "\u{24C4}");
        assert_eq!(PadControl::West.glyph(PadFamily::PlayStation), "\u{25A1}");
        assert_eq!(PadControl::North.glyph(PadFamily::PlayStation), "\u{25B3}");
        assert_eq!(PadControl::South.glyph(PadFamily::Xbox), "\u{24B6}");
        assert_eq!(PadControl::North.glyph(PadFamily::Xbox), "\u{24CE}");
        // Everything else prompts by its name.
        assert_eq!(PadControl::LeftShoulder.glyph(PadFamily::PlayStation), "L1");
        assert_eq!(PadControl::Start.glyph(PadFamily::Xbox), "Start");
    }

    #[test]
    fn the_screen_hint_pages_with_arrows_and_closes_on_both_devices() {
        // Owner 2026-09-16: "how are you gonna close the keyboard menu if
        // you don't have a controller?" — the close prompt names the key
        // beside the pad glyph; the paging arrows are the matched
        // triangle pair U+25C0/U+25B6 (the "pointer" pair renders two
        // sizes), the same glyphs the arrow-key caps and the d-pad wear.
        let bindings = project_bindings();
        let hint = screen_hint(&bindings, PadFamily::PlayStation);
        assert!(hint.contains("\u{25C0} \u{25B6}") && hint.contains("switch device"), "{hint}");
        assert!(hint.contains("\u{24CD} / Enter") && hint.ends_with("Close"), "{hint}");
        assert!(!hint.contains("\u{25C4}") && !hint.contains("\u{25BA}"), "no pointer glyphs: {hint}");
        let xbox = screen_hint(&bindings, PadFamily::Xbox);
        assert!(xbox.contains("\u{24B6} / Enter"), "the pad in hand's glyph: {xbox}");
    }

    #[test]
    fn the_legend_lists_each_lit_cap_once_in_reading_order() {
        let caps = keyboard_caps(&project_bindings());
        let legend = keyboard_legend(&caps);
        let keys: Vec<&str> = legend.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"W") && keys.contains(&"Space") && keys.contains(&"LMB"),
            "the legend prints cap labels: {keys:?}");
        assert!(!keys.contains(&"G"), "dark keys are not in the legend");
        let w = legend.iter().position(|(k, _)| k == "W").unwrap();
        let z = legend.iter().position(|(k, _)| k == "Z").unwrap();
        assert!(w < z, "reading order: row by row, left to right");
        let (_, text) = legend.iter().find(|(k, _)| k == "W").unwrap();
        assert_eq!(text, GameAction::MoveForward.label());
        let lit = caps.iter().filter(|c| !c.actions.is_empty()).count();
        assert_eq!(legend.len(), lit, "one legend line per lit cap");
    }
}
