use godot::prelude::*;
use godot::classes::{Area3D, IArea3D, OmniLight3D};

use super::constants::{groups, methods, signals};
use super::godot_util;
use super::live_handle::{LiveOpt, LiveRef};
use void_logic::audio_catalog::SfxEvent;
use void_logic::currency::CurrencyKind;

/// A currency cache pickup — the only in-level reward. A blue cache carries
/// components (in-run), a green one organics (permanent); the kind tints the
/// glow. Enemies drop a blue cache on death, a level's loot spawns place green
/// ones, and flying into either credits its amount through GameManager.
/// Nothing is ever auto-awarded: an uncollected cache is money left floating.
#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct CurrencyCache {
    base: Base<Area3D>,

    #[export]
    bob_speed: f32,
    #[export]
    bob_amplitude: f32,

    /// Which account this cache credits. Stamped by `drop_at`.
    kind: CurrencyKind,
    /// How much it credits. Stamped by `drop_at`.
    amount: u32,
    /// The glow light, kept so `drop_at` can retint it to the kind's color.
    glow: Option<LiveRef<OmniLight3D>>,

    time: f32,
    origin_y: f32,
    origin_captured: bool,
    collected: bool,
}

#[godot_api]
impl IArea3D for CurrencyCache {
    fn init(base: Base<Area3D>) -> Self {
        Self {
            base,
            bob_speed: 2.0,
            bob_amplitude: 0.3,
            kind: CurrencyKind::Components,
            amount: 0,
            glow: None,
            time: 0.0,
            origin_y: 0.0,
            origin_captured: false,
            collected: false,
        }
    }

    fn ready(&mut self) {
        // Monitoring is owned by apply_dormancy (deferred): caches are built mid
        // portal-signal flush, where a direct set_monitoring is blocked.
        self.base_mut().set_collision_mask(1); // Detect layer 1 (player)
        self.base_mut().set_collision_layer(0); // Don't block anything

        // Connect body_entered signal
        let callable = self.base().callable(methods::ON_BODY_ENTERED);
        self.base_mut().connect(signals::BODY_ENTERED, &callable);

        // The glow marks the currency: blue components, green organics. Kept as
        // a handle so drop_at retints it when the kind is stamped.
        let color = self.kind.glow_color();
        let mut node: Gd<Node3D> = self.base().clone().upcast();
        self.glow = Some(godot_util::attach_glow_light(&mut node, &color, 4.0, 5.0));

        // Pre-built caches enter the tree dormant (Faucet Principle, tier 1):
        // one blue cache is reserved per enemy during the load, and the green
        // loot-spawn caches are placed by the same `drop_at` activation path.
        self.deactivate_dormant();
    }

    fn process(&mut self, delta: f64) {
        if self.collected {
            return;
        }

        // Capture the bob origin lazily on the first frame so it is correct
        // regardless of whether position was set before or after entering the
        // tree (a spawner that adds-then-positions would otherwise bob at ~0).
        if !self.origin_captured {
            self.origin_y = self.base().get_global_position().y;
            self.origin_captured = true;
        }

        // Gentle bobbing animation
        self.time += delta as f32;
        let mut pos = self.base().get_global_position();
        pos.y = self.origin_y + (self.time * self.bob_speed).sin() * self.bob_amplitude;
        self.base_mut().set_global_position(pos);

        // Slow rotation
        self.base_mut().rotate_y((delta * 1.5) as f32);
    }
}

#[godot_api]
impl CurrencyCache {
    #[signal]
    fn cache_collected(kind_id: i32, amount: i64);

    #[func]
    fn on_body_entered(&mut self, body: Gd<Node3D>) {
        if self.collected {
            return;
        }
        if body.is_in_group(groups::PLAYER) {
            self.collect();
        }
    }

    #[func]
    pub fn collect(&mut self) {
        if self.collected {
            return;
        }
        self.collected = true;

        let pos = self.base().get_global_position();

        // Loot pickup SFX
        if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
            audio.bind_mut().play_event_at(SfxEvent::LootPickup, pos);
        }

        godot_print!("Collected cache: {} ({:?})", self.amount, self.kind);

        // Emit signal — GameManager credits the matching account.
        let (kind_id, amount) = (self.kind.id(), self.amount as i64);
        self.base_mut().emit_signal(
            signals::CACHE_COLLECTED,
            &[Variant::from(kind_id), Variant::from(amount)],
        );

        // Tier-1 pool contract (Faucet Principle): a collected cache goes
        // dormant, never freed. It lived its one life; the pool holds it for the
        // level's lifetime and never reuses it.
        self.deactivate_dormant();
    }

    /// Activate this dormant cache at `pos` carrying `amount` of `kind` — the
    /// drop, on its bound enemy's death (blue) or at a loot spawn during the
    /// build (green). Reset-in-place: stamp the payload, retint the glow, re-arm
    /// the collect/bob state, flip all three dormancy flags on, and place it
    /// (resetting interpolation so it doesn't streak across the map). This is
    /// the one structural drop path; nothing is instantiated here — the cache
    /// was pre-built during the load.
    pub fn drop_at(&mut self, pos: Vector3, kind: CurrencyKind, amount: u32) {
        self.kind = kind;
        self.amount = amount;
        let c = kind.glow_color();
        self.glow.with(|light| light.set_color(Color::from_rgb(c[0], c[1], c[2])));
        self.collected = false;
        self.origin_captured = false;
        self.time = 0.0;
        self.base_mut().set_global_position(pos);
        self.activate();
        self.base_mut().reset_physics_interpolation();
    }

    /// Go live: engine flags applied on the deferred boundary — the drop happens
    /// inside the dying enemy's physics-context death chain, where Godot blocks
    /// monitoring toggles.
    fn activate(&mut self) {
        self.base_mut().call_deferred(methods::APPLY_DORMANCY, &[true.to_variant()]);
    }

    /// Return to dormant, deferred for the same reason: `collect()` runs inside
    /// this Area3D's own `body_entered` flush, where a direct `set_monitoring`
    /// is blocked ("Function blocked during in/out signal") and would strand the
    /// cache half-dormant. The `collected` flag is the immediate logical truth;
    /// the engine flags follow at end of frame.
    pub fn deactivate_dormant(&mut self) {
        self.base_mut().call_deferred(methods::APPLY_DORMANCY, &[false.to_variant()]);
    }

    /// Flip the engine-side dormancy flags, together: visibility, processing,
    /// and monitoring. Always invoked via `call_deferred` — see
    /// [`deactivate_dormant`](Self::deactivate_dormant).
    #[func]
    fn apply_dormancy(&mut self, live: bool) {
        let mode = if live {
            godot::classes::node::ProcessMode::INHERIT
        } else {
            godot::classes::node::ProcessMode::DISABLED
        };
        let mut base = self.base_mut();
        base.set_visible(live);
        base.set_process_mode(mode);
        base.set_monitoring(live);
    }
}
