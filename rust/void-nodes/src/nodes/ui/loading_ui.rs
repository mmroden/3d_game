use godot::prelude::*;
use godot::classes::{CanvasLayer, ICanvasLayer, ColorRect, Control, Engine, Label};
use godot::classes::control::{LayoutPreset, SizeFlags};

use crate::nodes::constants::theme;
use crate::nodes::live_handle::LiveRef;
use void_logic::ui_style;

/// Full-screen "building the sector" veil, shown for the frame(s) a level
/// build occupies. Every entry into Playing routes through one deferred
/// build in GameManager (new game, Continue, next level, respawn); this
/// overlay is the player-visible half of that deferral — without it the
/// build frame reads as a freeze (playtest 2026-07-03).
#[derive(GodotClass)]
#[class(base=CanvasLayer)]
pub struct LoadingUI {
    base: Base<CanvasLayer>,
    label: Option<LiveRef<Label>>,
}

#[godot_api]
impl ICanvasLayer for LoadingUI {
    fn init(base: Base<CanvasLayer>) -> Self {
        Self { base, label: None }
    }

    fn ready(&mut self) {
        if Engine::singleton().is_editor_hint() {
            return;
        }
        // Above every other UI layer: the veil covers whatever screen the
        // build was entered from.
        self.base_mut().set_layer(10);

        let mut veil = ColorRect::new_alloc();
        veil.set_color(Color::from_rgba(0.02, 0.03, 0.05, 0.92));
        veil.set_anchors_preset(LayoutPreset::FULL_RECT);
        self.base_mut().add_child(&veil);

        // Centered text sits inside the SBS-safe central band by construction.
        let mut center = Control::new_alloc();
        center.set_anchors_preset(LayoutPreset::FULL_RECT);
        self.base_mut().add_child(&center);

        let mut label = Label::new_alloc();
        label.set_text("ENTERING SECTOR…");
        label.add_theme_font_size_override(theme::FONT_SIZE, ui_style::FONT_TITLE);
        label.add_theme_color_override(theme::FONT_COLOR, Color::from_rgb(0.6, 0.9, 1.0));
        label.set_anchors_preset(LayoutPreset::CENTER);
        label.set_h_size_flags(SizeFlags::SHRINK_CENTER);
        center.add_child(&label);
        self.label = Some(LiveRef::new(&label));

        self.base_mut().set_visible(false);
    }
}

#[godot_api]
impl LoadingUI {
    /// Raise the veil for a build of `level`. Crossing onto a new planet
    /// turns the veil into the arrival interstitial — title plus the flavor
    /// line that carries the story beats (B10; text swaps in when the
    /// narrative lands in `planet::ARRIVAL_FLAVOR`).
    #[func]
    pub fn show_loading(&mut self, level: i32) {
        if let Some(label) = &self.label {
            let text = match void_logic::planet::arrival_banner(level.max(1) as u32) {
                Some((title, flavor)) => {
                    format!("{title}\n\n{flavor}\n\nENTERING SECTOR {level}…")
                }
                None => format!("ENTERING SECTOR {level}…"),
            };
            label.with(|l| l.set_text(&text));
        }
        self.base_mut().set_visible(true);
    }

    /// Drop the veil — the sector is built.
    #[func]
    pub fn hide_loading(&mut self) {
        self.base_mut().set_visible(false);
    }
}
