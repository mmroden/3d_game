//! The player's preferences — display, mouse, audio, and the one-shot
//! briefing flag — as ONE struct that names its own entries. That list
//! is the single wire: the shell saves it, loads it, and broadcasts it
//! (as a dictionary) to every consumer; the options menu walks it as
//! typed rows. No consumer holds a default of its own.

/// A preference's stored value: every entry is a bool or a small int
/// (a choice's index, a slider's notch), so one wire carries all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionValue {
    Bool(bool),
    Int(i64),
}

/// Fullscreen or a window. SBS forces fullscreen (the glasses want the
/// whole panel); this is the mono preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowMode {
    #[default]
    Windowed,
    Fullscreen,
}

impl WindowMode {
    pub const ALL: [WindowMode; 2] = [Self::Windowed, Self::Fullscreen];

    pub fn label(self) -> &'static str {
        match self {
            Self::Windowed => "Windowed",
            Self::Fullscreen => "Fullscreen",
        }
    }
}

/// The 3D render scale: the frame is drawn at this fraction of the
/// window and upscaled — the one honest performance knob for a game
/// whose window is the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RenderScale {
    Half,
    ThreeQuarter,
    #[default]
    Full,
}

impl RenderScale {
    pub const ALL: [RenderScale; 3] = [Self::Half, Self::ThreeQuarter, Self::Full];

    pub fn factor(self) -> f32 {
        match self {
            Self::Half => 0.5,
            Self::ThreeQuarter => 0.75,
            Self::Full => 1.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Half => "50%",
            Self::ThreeQuarter => "75%",
            Self::Full => "100%",
        }
    }
}

/// System-wide preferences that persist across game sessions.
///
/// Display defaults off: SBS is opt-in, 4× MSAA costs ~5–10ms/frame in
/// SBS so players turn it on only on beefier machines, and dynamic
/// stereo (the stereo director's convergence + interaxial tracking,
/// experiment v2) is judged against the static baseline in glasses
/// before it earns a default. Volumes default full; the mouse to the
/// middle of its range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameOptions {
    pub sbs_enabled: bool,
    pub msaa_enabled: bool,
    pub dynamic_stereo: bool,
    /// The controls briefing has been shown once (the first new game).
    /// A preference, not run state: it survives the save wipe a new
    /// game performs, so the card never returns uninvited.
    pub controls_seen: bool,
    pub window_mode: WindowMode,
    pub render_scale: RenderScale,
    /// Mouse look sensitivity, `1..=MOUSE_SENSITIVITY_MAX`.
    pub mouse_sensitivity: u8,
    pub invert_mouse_y: bool,
    /// Bus volumes in tenths, `0..=VOLUME_NOTCHES` (10 = 100%).
    pub master_volume: u8,
    pub music_volume: u8,
    pub sfx_volume: u8,
}

impl Default for GameOptions {
    fn default() -> Self {
        Self {
            sbs_enabled: false,
            msaa_enabled: false,
            dynamic_stereo: false,
            controls_seen: false,
            window_mode: WindowMode::default(),
            render_scale: RenderScale::default(),
            mouse_sensitivity: Self::MOUSE_SENSITIVITY_DEFAULT,
            invert_mouse_y: false,
            master_volume: Self::VOLUME_NOTCHES,
            music_volume: Self::VOLUME_NOTCHES,
            sfx_volume: Self::VOLUME_NOTCHES,
        }
    }
}

impl GameOptions {
    pub const MOUSE_SENSITIVITY_MAX: u8 = 10;
    pub const MOUSE_SENSITIVITY_DEFAULT: u8 = 5;
    /// A volume slider's notches: 0 = silent, 10 = full.
    pub const VOLUME_NOTCHES: u8 = 10;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle_sbs(&mut self) -> bool {
        self.sbs_enabled = !self.sbs_enabled;
        self.sbs_enabled
    }

    pub fn toggle_msaa(&mut self) -> bool {
        self.msaa_enabled = !self.msaa_enabled;
        self.msaa_enabled
    }

    pub fn toggle_dynamic_stereo(&mut self) -> bool {
        self.dynamic_stereo = !self.dynamic_stereo;
        self.dynamic_stereo
    }

    /// Whether the pre-level briefing should open on the controls card:
    /// only until it has been shown once. Claiming it marks it seen.
    pub fn claim_controls_briefing(&mut self) -> bool {
        let owed = !self.controls_seen;
        self.controls_seen = true;
        owed
    }

    /// A volume in tenths as the linear gain `0.0..=1.0`.
    pub fn volume_fraction(notches: u8) -> f32 {
        notches.min(Self::VOLUME_NOTCHES) as f32 / Self::VOLUME_NOTCHES as f32
    }

    /// Every preference as (key, value) — the one wire for saving,
    /// loading and broadcasting. Order is stable.
    pub fn entries(&self) -> Vec<(OptionKey, OptionValue)> {
        use OptionValue::{Bool, Int};
        let choice = |index: usize| Int(index as i64);
        vec![
            (OptionKey::Sbs, Bool(self.sbs_enabled)),
            (OptionKey::Msaa, Bool(self.msaa_enabled)),
            (OptionKey::DynamicStereo, Bool(self.dynamic_stereo)),
            (OptionKey::ControlsSeen, Bool(self.controls_seen)),
            (OptionKey::WindowMode, choice(WindowMode::ALL.iter().position(|m| *m == self.window_mode).unwrap_or(0))),
            (OptionKey::RenderScale, choice(RenderScale::ALL.iter().position(|s| *s == self.render_scale).unwrap_or(0))),
            (OptionKey::MouseSensitivity, Int(self.mouse_sensitivity as i64)),
            (OptionKey::InvertMouseY, Bool(self.invert_mouse_y)),
            (OptionKey::MasterVolume, Int(self.master_volume as i64)),
            (OptionKey::MusicVolume, Int(self.music_volume as i64)),
            (OptionKey::SfxVolume, Int(self.sfx_volume as i64)),
        ]
    }

    /// Set one preference from the wire; an out-of-range value clamps,
    /// a wrong-typed one is ignored (the default stands).
    pub fn set_entry(&mut self, key: OptionKey, value: OptionValue) {
        use OptionValue::{Bool, Int};
        let notch = |v: i64, max: u8| v.clamp(0, max as i64) as u8;
        match (key, value) {
            (OptionKey::Sbs, Bool(v)) => self.sbs_enabled = v,
            (OptionKey::Msaa, Bool(v)) => self.msaa_enabled = v,
            (OptionKey::DynamicStereo, Bool(v)) => self.dynamic_stereo = v,
            (OptionKey::ControlsSeen, Bool(v)) => self.controls_seen = v,
            (OptionKey::InvertMouseY, Bool(v)) => self.invert_mouse_y = v,
            (OptionKey::WindowMode, Int(v)) => {
                if let Some(mode) = usize::try_from(v).ok().and_then(|i| WindowMode::ALL.get(i)) {
                    self.window_mode = *mode;
                }
            }
            (OptionKey::RenderScale, Int(v)) => {
                if let Some(scale) = usize::try_from(v).ok().and_then(|i| RenderScale::ALL.get(i)) {
                    self.render_scale = *scale;
                }
            }
            (OptionKey::MouseSensitivity, Int(v)) => {
                self.mouse_sensitivity = notch(v, Self::MOUSE_SENSITIVITY_MAX).max(1)
            }
            (OptionKey::MasterVolume, Int(v)) => self.master_volume = notch(v, Self::VOLUME_NOTCHES),
            (OptionKey::MusicVolume, Int(v)) => self.music_volume = notch(v, Self::VOLUME_NOTCHES),
            (OptionKey::SfxVolume, Int(v)) => self.sfx_volume = notch(v, Self::VOLUME_NOTCHES),
            // A wrong-typed value: the default stands.
            (_, Bool(_) | Int(_)) => {}
        }
    }

    /// Rebuild from a wire (missing keys keep their defaults).
    pub fn from_entries<'a>(entries: impl IntoIterator<Item = (&'a str, OptionValue)>) -> Self {
        let mut options = Self::default();
        for (name, value) in entries {
            if let Some(key) = OptionKey::from_name(name) {
                options.set_entry(key, value);
            }
        }
        options
    }
}

/// The stored preferences by name — the keys in options.cfg and in the
/// broadcast dictionary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionKey {
    Sbs,
    Msaa,
    DynamicStereo,
    ControlsSeen,
    WindowMode,
    RenderScale,
    MouseSensitivity,
    InvertMouseY,
    MasterVolume,
    MusicVolume,
    SfxVolume,
}

impl OptionKey {
    pub const ALL: [OptionKey; 11] = [
        Self::Sbs,
        Self::Msaa,
        Self::DynamicStereo,
        Self::ControlsSeen,
        Self::WindowMode,
        Self::RenderScale,
        Self::MouseSensitivity,
        Self::InvertMouseY,
        Self::MasterVolume,
        Self::MusicVolume,
        Self::SfxVolume,
    ];

    /// The key's name on the wire. The first three keep the names the
    /// options file has carried since the display toggles, so a saved
    /// preference survives this change.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sbs => "sbs",
            Self::Msaa => "msaa",
            Self::DynamicStereo => "dynamic_stereo",
            Self::ControlsSeen => "controls_seen",
            Self::WindowMode => "window_mode",
            Self::RenderScale => "render_scale",
            Self::MouseSensitivity => "mouse_sensitivity",
            Self::InvertMouseY => "invert_mouse_y",
            Self::MasterVolume => "master_volume",
            Self::MusicVolume => "music_volume",
            Self::SfxVolume => "sfx_volume",
        }
    }

    pub fn from_name(name: &str) -> Option<OptionKey> {
        Self::ALL.iter().copied().find(|k| k.name() == name)
    }
}

/// The options menu's rows, in the order both menus show them. A row is
/// a toggle (select flips it), a choice (left/right cycle it, select
/// advances) or a slider (left/right step it), plus Back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionRow {
    SbsStereo,
    Msaa,
    DynamicStereo,
    WindowMode,
    RenderScale,
    MouseSensitivity,
    InvertMouseY,
    MasterVolume,
    MusicVolume,
    SfxVolume,
    Back,
}

/// How a row responds to the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Toggle,
    Choice,
    Slider,
    Back,
}

impl OptionRow {
    pub const ALL: [OptionRow; 11] = [
        Self::SbsStereo,
        Self::Msaa,
        Self::DynamicStereo,
        Self::WindowMode,
        Self::RenderScale,
        Self::MouseSensitivity,
        Self::InvertMouseY,
        Self::MasterVolume,
        Self::MusicVolume,
        Self::SfxVolume,
        Self::Back,
    ];

    /// The row's name on the wire (the menus' `option_adjusted` signal).
    pub const fn name(self) -> &'static str {
        match self {
            Self::SbsStereo => "sbs_stereo",
            Self::Msaa => "msaa",
            Self::DynamicStereo => "dynamic_stereo",
            Self::WindowMode => "window_mode",
            Self::RenderScale => "render_scale",
            Self::MouseSensitivity => "mouse_sensitivity",
            Self::InvertMouseY => "invert_mouse_y",
            Self::MasterVolume => "master_volume",
            Self::MusicVolume => "music_volume",
            Self::SfxVolume => "sfx_volume",
            Self::Back => "back",
        }
    }

    pub fn from_name(name: &str) -> Option<OptionRow> {
        Self::ALL.iter().copied().find(|r| r.name() == name)
    }

    /// The row's printed name.
    pub fn label(self) -> &'static str {
        match self {
            Self::SbsStereo => "SBS Stereo",
            Self::Msaa => "MSAA",
            Self::DynamicStereo => "Dynamic 3D",
            Self::WindowMode => "Window",
            Self::RenderScale => "Render scale",
            Self::MouseSensitivity => "Mouse sensitivity",
            Self::InvertMouseY => "Invert mouse Y",
            Self::MasterVolume => "Master volume",
            Self::MusicVolume => "Music volume",
            Self::SfxVolume => "Effects volume",
            Self::Back => "Back",
        }
    }

    /// How the row responds to the cursor.
    pub fn kind(self) -> RowKind {
        match self {
            Self::SbsStereo | Self::Msaa | Self::DynamicStereo | Self::InvertMouseY => RowKind::Toggle,
            Self::WindowMode | Self::RenderScale => RowKind::Choice,
            Self::MouseSensitivity | Self::MasterVolume | Self::MusicVolume | Self::SfxVolume => {
                RowKind::Slider
            }
            Self::Back => RowKind::Back,
        }
    }

    /// The value the row prints beside its label ("ON", "75%", "5");
    /// nothing for Back.
    pub fn value_text(self, options: &GameOptions) -> String {
        let on_off = |on: bool| if on { "ON" } else { "OFF" }.to_string();
        let percent = |notches: u8| format!("{}%", notches as u32 * 100 / GameOptions::VOLUME_NOTCHES as u32);
        match self {
            Self::SbsStereo => on_off(options.sbs_enabled),
            Self::Msaa => on_off(options.msaa_enabled),
            Self::DynamicStereo => on_off(options.dynamic_stereo),
            Self::InvertMouseY => on_off(options.invert_mouse_y),
            Self::WindowMode => options.window_mode.label().to_string(),
            Self::RenderScale => options.render_scale.label().to_string(),
            Self::MouseSensitivity => options.mouse_sensitivity.to_string(),
            Self::MasterVolume => percent(options.master_volume),
            Self::MusicVolume => percent(options.music_volume),
            Self::SfxVolume => percent(options.sfx_volume),
            Self::Back => String::new(),
        }
    }

    /// Apply the cursor to the row: `delta` is -1 for left, +1 for right
    /// or select. Toggles flip on any delta; choices and sliders step and
    /// clamp; Back changes nothing. Returns whether anything changed.
    pub fn adjust(self, options: &mut GameOptions, delta: i32) -> bool {
        if delta == 0 {
            return false;
        }
        fn step<T: Copy + PartialEq>(current: T, choices: &[T], delta: i32) -> Option<T> {
            let at = choices.iter().position(|c| *c == current)? as i32;
            let next = (at + delta).clamp(0, choices.len() as i32 - 1);
            (next != at).then(|| choices[next as usize])
        }
        fn notch(current: u8, min: u8, max: u8, delta: i32) -> Option<u8> {
            let next = (current as i32 + delta).clamp(min as i32, max as i32) as u8;
            (next != current).then_some(next)
        }
        let before = options.clone();
        match self {
            Self::SbsStereo => options.sbs_enabled = !options.sbs_enabled,
            Self::Msaa => options.msaa_enabled = !options.msaa_enabled,
            Self::DynamicStereo => options.dynamic_stereo = !options.dynamic_stereo,
            Self::InvertMouseY => options.invert_mouse_y = !options.invert_mouse_y,
            Self::WindowMode => {
                if let Some(mode) = step(options.window_mode, &WindowMode::ALL, delta) {
                    options.window_mode = mode;
                }
            }
            Self::RenderScale => {
                if let Some(scale) = step(options.render_scale, &RenderScale::ALL, delta) {
                    options.render_scale = scale;
                }
            }
            Self::MouseSensitivity => {
                if let Some(v) = notch(options.mouse_sensitivity, 1, GameOptions::MOUSE_SENSITIVITY_MAX, delta) {
                    options.mouse_sensitivity = v;
                }
            }
            Self::MasterVolume => {
                if let Some(v) = notch(options.master_volume, 0, GameOptions::VOLUME_NOTCHES, delta) {
                    options.master_volume = v;
                }
            }
            Self::MusicVolume => {
                if let Some(v) = notch(options.music_volume, 0, GameOptions::VOLUME_NOTCHES, delta) {
                    options.music_volume = v;
                }
            }
            Self::SfxVolume => {
                if let Some(v) = notch(options.sfx_volume, 0, GameOptions::VOLUME_NOTCHES, delta) {
                    options.sfx_volume = v;
                }
            }
            Self::Back => {}
        }
        *options != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_sbs_off() {
        let opts = GameOptions::new();
        assert!(!opts.sbs_enabled);
    }

    #[test]
    fn defaults_msaa_off() {
        // MSAA is opt-in: too costly in SBS to enable by default.
        let opts = GameOptions::new();
        assert!(!opts.msaa_enabled);
    }

    #[test]
    fn defaults_dynamic_stereo_off() {
        // The stereo director is an experiment: the static baseline is the
        // control condition, so the dynamic mode never ships on by default
        // until it wins in glasses.
        let opts = GameOptions::new();
        assert!(!opts.dynamic_stereo);
    }

    #[test]
    fn the_new_preferences_default_sanely() {
        let opts = GameOptions::new();
        assert_eq!(opts.window_mode, WindowMode::Windowed);
        assert_eq!(opts.render_scale, RenderScale::Full);
        assert_eq!(opts.mouse_sensitivity, GameOptions::MOUSE_SENSITIVITY_DEFAULT);
        assert!(!opts.invert_mouse_y);
        for volume in [opts.master_volume, opts.music_volume, opts.sfx_volume] {
            assert_eq!(volume, GameOptions::VOLUME_NOTCHES, "volumes start full");
        }
        assert_eq!(GameOptions::volume_fraction(GameOptions::VOLUME_NOTCHES), 1.0);
        assert_eq!(GameOptions::volume_fraction(0), 0.0);
        assert!((GameOptions::volume_fraction(7) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn the_controls_briefing_is_owed_exactly_once() {
        // Burhan's alpha note (2026-09-15): the first level needs the
        // controls shown before anything shoots. Once is the contract —
        // the card is a briefing, not a nag.
        let mut opts = GameOptions::new();
        assert!(!opts.controls_seen, "a fresh profile has not been briefed");
        assert!(opts.claim_controls_briefing(), "the first briefing shows the card");
        assert!(opts.controls_seen);
        assert!(!opts.claim_controls_briefing(), "the second does not");
        assert!(opts.controls_seen, "claiming never un-sees it");
    }

    #[test]
    fn toggle_sbs_returns_new_state() {
        let mut opts = GameOptions::new();
        let result = opts.toggle_sbs();
        assert!(result);
        assert!(opts.sbs_enabled);
    }

    #[test]
    fn toggle_sbs_twice_roundtrips() {
        let mut opts = GameOptions::new();
        opts.toggle_sbs();
        let result = opts.toggle_sbs();
        assert!(!result);
        assert!(!opts.sbs_enabled);
    }

    #[test]
    fn toggle_msaa_returns_new_state() {
        let mut opts = GameOptions::new();
        let result = opts.toggle_msaa();
        assert!(result);
        assert!(opts.msaa_enabled);
    }

    #[test]
    fn toggle_dynamic_stereo_returns_new_state() {
        let mut opts = GameOptions::new();
        let result = opts.toggle_dynamic_stereo();
        assert!(result);
        assert!(opts.dynamic_stereo);
    }

    // --- The wire ---

    #[test]
    fn every_key_is_on_the_wire_once_and_round_trips() {
        let mut opts = GameOptions::new();
        opts.sbs_enabled = true;
        opts.controls_seen = true;
        opts.window_mode = WindowMode::Fullscreen;
        opts.render_scale = RenderScale::Half;
        opts.mouse_sensitivity = 8;
        opts.invert_mouse_y = true;
        opts.music_volume = 3;
        let entries = opts.entries();
        assert_eq!(entries.len(), OptionKey::ALL.len(), "one entry per key");
        for key in OptionKey::ALL {
            assert_eq!(entries.iter().filter(|(k, _)| *k == key).count(), 1, "{key:?} once");
        }
        let back = GameOptions::from_entries(entries.iter().map(|(k, v)| (k.name(), *v)));
        assert_eq!(back, opts, "the wire loses nothing");
    }

    #[test]
    fn the_wire_keeps_the_saved_names_of_the_old_display_toggles() {
        // A profile saved before the refactor read sbs/msaa/dynamic_stereo
        // from the [display] section; those names must not move.
        assert_eq!(OptionKey::Sbs.name(), "sbs");
        assert_eq!(OptionKey::Msaa.name(), "msaa");
        assert_eq!(OptionKey::DynamicStereo.name(), "dynamic_stereo");
        assert_eq!(OptionKey::ControlsSeen.name(), "controls_seen");
        let mut names: Vec<&str> = OptionKey::ALL.iter().map(|k| k.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), OptionKey::ALL.len(), "names are distinct");
    }

    #[test]
    fn a_partial_or_wrong_typed_wire_keeps_the_defaults_and_clamps() {
        let opts = GameOptions::from_entries([
            ("msaa", OptionValue::Bool(true)),
            ("mouse_sensitivity", OptionValue::Int(99)),
            ("music_volume", OptionValue::Int(-4)),
            ("render_scale", OptionValue::Int(7)),
            ("sbs", OptionValue::Int(1)),
            ("no_such_key", OptionValue::Bool(true)),
        ]);
        assert!(opts.msaa_enabled);
        assert_eq!(opts.mouse_sensitivity, GameOptions::MOUSE_SENSITIVITY_MAX, "clamped high");
        assert_eq!(opts.music_volume, 0, "clamped low");
        assert_eq!(opts.render_scale, RenderScale::Full, "an unknown choice index stays at the default");
        assert!(!opts.sbs_enabled, "a wrong-typed value is ignored");
        assert_eq!(opts.master_volume, GameOptions::VOLUME_NOTCHES, "untouched keys keep defaults");
    }

    // --- The rows ---

    #[test]
    fn every_row_has_a_label_a_kind_and_a_distinct_name() {
        let mut names: Vec<&str> = OptionRow::ALL.iter().map(|r| r.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), OptionRow::ALL.len());
        for row in OptionRow::ALL {
            assert!(!row.label().is_empty(), "{row:?} unlabelled");
            assert_eq!(OptionRow::from_name(row.name()), Some(row));
        }
        assert_eq!(OptionRow::Back.kind(), RowKind::Back);
        assert_eq!(OptionRow::Msaa.kind(), RowKind::Toggle);
        assert_eq!(OptionRow::RenderScale.kind(), RowKind::Choice);
        assert_eq!(OptionRow::MusicVolume.kind(), RowKind::Slider);
        assert_eq!(*OptionRow::ALL.last().unwrap(), OptionRow::Back, "Back is the last row");
    }

    #[test]
    fn toggles_flip_on_any_delta_and_report_their_state() {
        let mut opts = GameOptions::new();
        assert_eq!(OptionRow::Msaa.value_text(&opts), "OFF");
        assert!(OptionRow::Msaa.adjust(&mut opts, 1));
        assert!(opts.msaa_enabled);
        assert_eq!(OptionRow::Msaa.value_text(&opts), "ON");
        assert!(OptionRow::Msaa.adjust(&mut opts, -1), "left flips too");
        assert!(!opts.msaa_enabled);
        assert!(OptionRow::InvertMouseY.adjust(&mut opts, 1));
        assert!(opts.invert_mouse_y);
    }

    #[test]
    fn choices_cycle_and_clamp_at_their_ends() {
        let mut opts = GameOptions::new();
        assert_eq!(OptionRow::RenderScale.value_text(&opts), "100%");
        assert!(!OptionRow::RenderScale.adjust(&mut opts, 1), "already at the last choice");
        assert!(OptionRow::RenderScale.adjust(&mut opts, -1));
        assert_eq!(opts.render_scale, RenderScale::ThreeQuarter);
        assert_eq!(OptionRow::RenderScale.value_text(&opts), "75%");
        OptionRow::RenderScale.adjust(&mut opts, -1);
        assert!(!OptionRow::RenderScale.adjust(&mut opts, -1), "…and at the first");
        assert_eq!(opts.render_scale, RenderScale::Half);
        assert!(OptionRow::WindowMode.adjust(&mut opts, 1));
        assert_eq!(opts.window_mode, WindowMode::Fullscreen);
        assert_eq!(OptionRow::WindowMode.value_text(&opts), "Fullscreen");
    }

    #[test]
    fn sliders_step_a_notch_and_clamp() {
        let mut opts = GameOptions::new();
        assert_eq!(OptionRow::MusicVolume.value_text(&opts), "100%");
        assert!(!OptionRow::MusicVolume.adjust(&mut opts, 1), "full is full");
        assert!(OptionRow::MusicVolume.adjust(&mut opts, -1));
        assert_eq!(opts.music_volume, 9);
        assert_eq!(OptionRow::MusicVolume.value_text(&opts), "90%");
        for _ in 0..20 {
            OptionRow::MusicVolume.adjust(&mut opts, -1);
        }
        assert_eq!(opts.music_volume, 0);
        assert_eq!(OptionRow::MusicVolume.value_text(&opts), "0%");
        assert_eq!(OptionRow::MouseSensitivity.value_text(&opts), "5");
        OptionRow::MouseSensitivity.adjust(&mut opts, 1);
        assert_eq!(opts.mouse_sensitivity, 6);
        for _ in 0..20 {
            OptionRow::MouseSensitivity.adjust(&mut opts, -1);
        }
        assert_eq!(opts.mouse_sensitivity, 1, "sensitivity never reaches zero");
        assert!(!OptionRow::Back.adjust(&mut opts, 1), "Back changes nothing");
        assert_eq!(OptionRow::Back.value_text(&opts), "");
    }
}
