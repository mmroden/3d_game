use godot::prelude::*;
use godot::classes::{
    RigidBody3D, IRigidBody3D, PhysicsDirectBodyState3D, PhysicsRayQueryParameters3D,
    MeshInstance3D, Camera3D, Node3D, CollisionShape3D, CapsuleShape3D, OmniLight3D,
    GpuParticles3D, SphereMesh, StandardMaterial3D,
    Input,
};

use super::constants::{actions, groups, methods, signals};
use super::godot_util;
use super::live_handle::{LiveOpt, LiveRef, LiveVec};

use void_logic::debuff::SlowDebuff;
use void_logic::laser::LaserLevel;
use void_logic::ship::{self, ShipColor};
use void_logic::loadout::Loadout;
use void_logic::power_routing::PowerMode;
use void_logic::armament::{cluster, subdrone, valkyrie, WeaponKind};
use void_logic::ship_type::ShipType;
use void_logic::upgrade::{Upgrade, UpgradeKind};
use void_logic::audio_catalog::SfxEvent;
use void_logic::weapon::{WeaponState, FireResult};

/// Commanded angular speed (rad/s) at full stick (before rotation
/// upgrades). A feel value to tune.
const TURN_RATE: f32 = 2.5;
/// Stiffness of the steering controller: how hard torque drives spin
/// toward the commanded rate. Higher = snappier. A feel value to tune.
const TURN_GAIN: f32 = 12.0;

/// Wing offset from ship center (local X axis), in meters.
const WING_OFFSET: f32 = 0.3;

/// Aim forgiveness: lasers also test a ring of this radius around the centre
/// line, so a near-miss still connects instead of needing pixel-perfect aim.
/// Sized against the SMALLEST enemies (0.5 m models, ~0.25 m hulls —
/// playtest 2026-07-04: they read as unhittable at 0.7).
const LASER_AIM_RADIUS: f32 = 1.3;
/// Minimum gap between collision sounds (so a wall scrape doesn't machine-gun).
const IMPACT_SOUND_COOLDOWN: f32 = 0.4;
/// How far off the hull (m) the hit sound is placed, toward whatever struck us.
/// Small on purpose: the cue sits right on the ship but still points at the
/// threat, so you can turn toward the shooter.
const HIT_SFX_OFFSET: f32 = 2.0;

/// Camera-shake burst when a grabber latches on: how long and how hard
/// (Camera3D frustum-offset units).
const SHAKE_DURATION: f32 = 0.5;
const SHAKE_AMP: f32 = 0.25;

/// Flight collider capsule, in world units (independent of the model's scale).
/// Tighter than the visual hull so the ship slides through doorways.
const SHIP_COLLIDER_RADIUS: f32 = 0.45;
const SHIP_COLLIDER_HEIGHT: f32 = 1.1;
/// Cockpit camera offset (local), looking forward.
const COCKPIT_OFFSET: Vector3 = Vector3::new(0.0, 0.5, 0.0);
/// Chase camera offset (local): behind (+Z) and above the ship — high enough
/// to see where the nose points.
const CHASE_OFFSET: Vector3 = Vector3::new(0.0, 2.4, 4.5);

/// Which way the player views the ship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CameraMode {
    Cockpit,
    Chase,
}

/// 6DOF flight controller for the player ship. Per the simulation idiom
/// (docs/architecture/physics_ownership.md): the engine owns motion,
/// collision, decay and rest; we only supply intent inside
/// `integrate_forces` — thrust as force, steering as torque toward a
/// commanded angular velocity — and let the engine integrate and damp.
/// `angular_damp > 0` is the no-infinite-spin guarantee. Lasers stay
/// hitscan (in `physics_process`, where space queries are legal).
#[derive(GodotClass)]
#[class(base=RigidBody3D)]
pub struct ShipController {
    base: Base<RigidBody3D>,

    weapon: WeaponState,
    beam_nodes: LiveVec<MeshInstance3D>,
    loadout: Loadout,
    laser_level: LaserLevel,
    power_mode: PowerMode,
    /// Movement slow applied by swarmer contact.
    slow: SlowDebuff,
    /// Cached player camera, jittered for the grab shake.
    camera: Option<LiveRef<Camera3D>>,
    shake_timer: f32,
    shake_phase: f32,
    camera_mode: CameraMode,
    /// Throttle so a wall scrape doesn't machine-gun the impact clang.
    impact_cooldown: f32,
    /// Edge-detect collision onset so resting against a wall stays silent.
    was_in_contact: bool,
    /// The ship model node — hidden in cockpit view (you look out of it),
    /// shown in chase view.
    ship_model: Option<LiveRef<Node3D>>,
    /// Color accent light on the ship model (the player's chosen color).
    color_glow: Option<LiveRef<OmniLight3D>>,
    /// The chosen hull — model, weapon, and stat multipliers come from its
    /// spec; GameManager pushes changes via `configure_ship`.
    ship_type: ShipType,
    /// Flight-speed multiplier (hull × color), pushed by GameManager.
    ship_thrust_mul: f32,
    /// Turn-rate multiplier from the hull's spec.
    ship_rotation_mul: f32,
    /// The Hive's launch-charge clock. Reset with the loadout; ticks in the
    /// weapons pass (piloting only).
    subdrone_regen: subdrone::RegenTimer,
    /// The Vanguard's purchasable heavy cannon: `Some` once the green
    /// keystone is owned (pushed by GameManager), firing alongside the
    /// hitscan lasers on the same trigger.
    valkyrie: Option<WeaponState>,
    /// Bursts fired — seeds each burst's deterministic fan.
    valkyrie_bursts: u64,
    /// Pilot input (fly, fire, view toggle) is only live during gameplay. On
    /// menu/showcase/bestiary screens the camera must NOT respond to the stick,
    /// so GameManager flips this off there.
    controls_enabled: bool,
}

#[godot_api]
impl IRigidBody3D for ShipController {
    fn init(base: Base<RigidBody3D>) -> Self {
        Self {
            base,
            weapon: WeaponState::default(),
            beam_nodes: LiveVec::new(),
            loadout: Loadout::new(),
            laser_level: LaserLevel::Red,
            power_mode: PowerMode::default(),
            slow: SlowDebuff::new(),
            camera: None,
            shake_timer: 0.0,
            shake_phase: 0.0,
            camera_mode: CameraMode::Cockpit,
            impact_cooldown: 0.0,
            was_in_contact: false,
            ship_model: None,
            color_glow: None,
            ship_type: ShipType::Vanguard,
            ship_thrust_mul: 1.0,
            ship_rotation_mul: 1.0,
            subdrone_regen: subdrone::RegenTimer::new(subdrone::REGEN_SECONDS),
            valkyrie: None,
            valkyrie_bursts: 0,
            controls_enabled: false,
        }
    }

    fn ready(&mut self) {
        self.base_mut().add_to_group(groups::PLAYER);
        {
            let mut base = self.base_mut();
            base.set_gravity_scale(0.0);
            base.set_can_sleep(false);
            // Continuous collision so fast flight never tunnels through walls.
            base.set_use_continuous_collision_detection(true);
            // Report contacts so we can play an impact clang on collision.
            base.set_contact_monitor(true);
            base.set_max_contacts_reported(4);
        }
        // Decay is the engine's: damping = -ln(retention). Sets the
        // no-infinite-spin invariant (angular_damp > 0).
        self.apply_envelope();

        // Cache the camera for the grab shake (None-safe if absent).
        self.camera = self.base()
            .try_get_node_as::<Camera3D>("Camera3D")
            .map(|c| LiveRef::new(&c));
        self.spawn_ship_model();
        self.apply_camera_mode();
    }

    /// Flight runs in `integrate_forces`, the engine's hook for safely
    /// touching physics state. Forces/torque only — never velocity writes
    /// (the one exception, stabilize, is a deliberate discrete reset).
    fn integrate_forces(&mut self, state: Option<Gd<PhysicsDirectBodyState3D>>) {
        let Some(mut state) = state else {
            return;
        };
        // Only fly when piloting is live — otherwise the stick must not rotate
        // the camera on the menu/showcase/bestiary screens.
        if self.controls_enabled {
            self.fly(&mut state);
        }

        // Collision clang on impact *onset* (resting against a wall is silent),
        // throttled by the cooldown. Whether it sounds shielded or bare-hull
        // depends on the shield, which only GameManager knows — so we just
        // report the collision and let it pick the sound, mirroring the
        // player_damaged path.
        let in_contact = state.get_contact_count() > 0;
        if in_contact && !self.was_in_contact && self.impact_cooldown <= 0.0 {
            self.impact_cooldown = IMPACT_SOUND_COOLDOWN;
            self.base_mut().emit_signal(signals::PLAYER_COLLIDED, &[]);
        }
        self.was_in_contact = in_contact;
    }

    /// Weapons + power routing. Hitscan needs the space unlocked, which is
    /// only true in `physics_process` — so it lives here, not in
    /// `integrate_forces`.
    fn physics_process(&mut self, delta: f64) {
        // No piloting on non-gameplay screens: skip view-toggle, weapons, and
        // power routing so the ship sits inert while menus drive the camera.
        if !self.controls_enabled {
            return;
        }
        if Input::singleton().is_action_just_pressed(actions::TOGGLE_VIEW) {
            self.camera_mode = match self.camera_mode {
                CameraMode::Cockpit => CameraMode::Chase,
                CameraMode::Chase => CameraMode::Cockpit,
            };
            self.apply_camera_mode();
        }
        self.handle_weapons_and_power(delta as f32);
        // Tick the movement slow; tell the HUD only when it switches on/off.
        if self.slow.tick(delta as f32) {
            let active = self.slow.is_active();
            self.base_mut()
                .emit_signal(signals::PLAYER_SLOWED, &[Variant::from(active)]);
        }
        self.tick_shake(delta as f32);
        self.impact_cooldown = (self.impact_cooldown - delta as f32).max(0.0);
    }
}

#[godot_api]
impl ShipController {
    /// The ship took a hit. `hit_position` is a point just off the hull toward
    /// the source, so GameManager can play the hit sound directionally.
    #[signal]
    fn player_damaged(amount: f32, hit_position: Vector3);

    /// The pilot pressed the item trigger (Shield Surge). GameManager owns
    /// the charges and the shield; this only reports intent.
    #[signal]
    fn shield_burst_requested();

    /// Ship hit static geometry (onset, throttled). Carries no shield state —
    /// GameManager owns that and picks the shielded/bare collision sound.
    #[signal]
    fn player_collided();

    #[signal]
    fn power_mode_changed(mode: i32);

    /// Player started/stopped being slowed (drives the HUD indicator).
    #[signal]
    fn player_slowed(active: bool);

    /// Apply a movement slow (e.g. from swarmer contact). `factor` is the
    /// thrust multiplier (0..1); `duration` is in seconds.
    #[func]
    pub fn apply_slow(&mut self, factor: f32, duration: f32) {
        let was_active = self.slow.is_active();
        self.slow.apply(factor, duration);
        // On a fresh grab: tell the HUD, kick the camera shake, and play the
        // latch sound so the player feels the attachment land.
        if !was_active && self.slow.is_active() {
            self.shake_timer = SHAKE_DURATION;
            let pos = self.base().get_global_position();
            if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
                audio.bind_mut().play_event_at(SfxEvent::SwarmerLatch, pos);
            }
            self.base_mut()
                .emit_signal(signals::PLAYER_SLOWED, &[Variant::from(true)]);
        }
    }

    /// Enable/disable pilot input. GameManager turns it on only for gameplay so
    /// the camera doesn't fly around on the menu/showcase/bestiary screens.
    #[func]
    pub fn set_controls_enabled(&mut self, enabled: bool) {
        self.controls_enabled = enabled;
    }

    /// Apply the player's chosen ship color and its flight-speed multiplier.
    /// Called by GameManager (hull id, ShipColor id, combined thrust
    /// multiplier) when the loadout is chosen/synced. A hull change respawns
    /// the model from its spec; the color re-paints hulls that support the
    /// painted styles, and the accent glow matches it either way.
    #[func]
    pub fn configure_ship(&mut self, ship_type_id: i32, color_id: i32, thrust_mul: f32) {
        self.ship_thrust_mul = thrust_mul;
        let new_type = ShipType::from_id(ship_type_id).unwrap_or_default();
        self.ship_rotation_mul = new_type.spec().rotation_mul;
        if new_type != self.ship_type {
            self.ship_type = new_type;
            // The glow lives on the model; both rebuild from the new spec.
            // The retiring model is renamed first: it lives until the deferred
            // free, and Godot would otherwise rename the incoming "Model".
            self.ship_model.with(|model| {
                model.set_name("RetiredModel");
                model.queue_free();
            });
            self.ship_model = None;
            self.color_glow = None;
            self.spawn_ship_model();
            // The fresh model must honor the current view: in cockpit the
            // hull is hidden (you're inside it) — without this the swapped
            // ship floats visibly in front of the camera.
            self.apply_camera_mode();
        }
        let sc = ShipColor::from_id(color_id).unwrap_or_default();
        godot_util::recolor_glow(&self.color_glow, godot_util::to_color(sc.color()));
        if self.ship_type.spec().supports_styles {
            let style = sc.body_style();
            let idx = ship::style_texture_index(ship::STYLED_BODY_PART, style);
            self.ship_model.with(|model| {
                let m: Gd<Node3D> = model.clone();
                godot_util::apply_body_style(&m, style, idx);
            });
        }
    }

    /// Build the player ship from the hull's spec — model path, fit size,
    /// and imported-front-axis yaw all come from `ShipType` (one source of
    /// truth, like `EnemyType::model_path`) — plus a capsule collider and
    /// the color accent light.
    fn spawn_ship_model(&mut self) {
        let spec = self.ship_type.spec();
        let mut parent: Gd<Node3D> = self.base().clone().upcast();
        let Some(mut model) =
            godot_util::spawn_model_fitted(&mut parent, spec.model_path, spec.model_size)
        else {
            return;
        };
        model.rotate_y(spec.model_yaw_offset);
        self.ship_model = Some(LiveRef::new(&model));

        // Flight collider: a capsule laid along the hull. It's rounded, so it
        // slides off doorframe edges instead of snagging a wingtip, and sized in
        // world units so it never inherits the imported model's extreme scale —
        // a scaled, mesh-derived hull sized wrong in Jolt and let the ship clip
        // through walls. The visual model may overhang it slightly.
        if let Some(mut shape_node) =
            self.base().try_get_node_as::<CollisionShape3D>("CollisionShape3D")
        {
            let mut capsule = CapsuleShape3D::new_gd();
            capsule.set_radius(SHIP_COLLIDER_RADIUS);
            capsule.set_height(SHIP_COLLIDER_HEIGHT);
            shape_node.set_shape(&capsule);
            // Lay the capsule along the ship's length rather than its default Y.
            shape_node.set_rotation(Vector3::new(std::f32::consts::FRAC_PI_2, 0.0, 0.0));
            shape_node.set_position(Vector3::ZERO);
        }

        // Color accent light + painted body style (Standard until GameManager
        // pushes the choice). The body texture carries the color; the glow
        // matches. Styles only exist on hulls that support them.
        let default = ShipColor::default();
        let c = default.color();
        self.color_glow = Some(godot_util::attach_glow_light(&mut model, &c, 2.0, 6.0));
        if spec.supports_styles {
            let style = default.body_style();
            godot_util::apply_body_style(&model, style, ship::style_texture_index(ship::STYLED_BODY_PART, style));
        }
    }

    /// Place the camera for the current view mode, and show the ship model only
    /// in chase view (in cockpit you're inside the hull, so it would block the view).
    fn apply_camera_mode(&mut self) {
        let chase = self.camera_mode == CameraMode::Chase;
        self.ship_model.with(|model| model.set_visible(chase));
        let transform = match self.camera_mode {
            CameraMode::Cockpit => Transform3D::new(Basis::IDENTITY, COCKPIT_OFFSET),
            CameraMode::Chase => {
                // Look from behind/above toward a point just ahead of the ship.
                let look_dir = (Vector3::new(0.0, 0.0, -2.0) - CHASE_OFFSET).normalized();
                Transform3D::new(godot_util::basis_from_direction(look_dir), CHASE_OFFSET)
            }
        };
        self.camera.with(|camera| camera.set_transform(transform));
    }

    /// Jitter the camera with a decaying offset while the grab shake is active.
    fn tick_shake(&mut self, delta: f32) {
        if self.shake_timer <= 0.0 {
            return;
        }
        // Advance the shake state first, then push the offsets to the camera —
        // self can't be mutated while `camera.with` borrows it.
        self.shake_timer = (self.shake_timer - delta).max(0.0);
        self.shake_phase += delta;
        let (h, v) = if self.shake_timer <= 0.0 {
            (0.0, 0.0)
        } else {
            let amp = SHAKE_AMP * (self.shake_timer / SHAKE_DURATION);
            (
                (self.shake_phase * 91.0).sin() * amp,
                (self.shake_phase * 123.0).cos() * amp,
            )
        };
        self.camera.with(|camera| {
            camera.set_h_offset(h);
            camera.set_v_offset(v);
        });
    }

    /// Set linear/angular damping from the loadout's per-second
    /// `Retention` (`damp = -ln(retention)`). The engine's damping *is*
    /// the project's retention decay; `angular_damp > 0` is the
    /// no-infinite-spin invariant. Re-applied whenever the loadout changes.
    fn apply_envelope(&mut self) {
        let damp = (-self.loadout.damping().factor().ln()).max(0.0);
        // The `.max(0.0)` is belt-and-suspenders: `Retention::decaying`
        // clamps the factor below 1.0, so `-ln(factor)` is already strictly
        // positive. Zero would break the no-infinite-spin invariant, so pin
        // that it's unreachable — and catch any future loosening of the clamp.
        debug_assert!(damp > 0.0, "angular/linear damp must be > 0 (no-infinite-spin invariant)");
        let mut base = self.base_mut();
        base.set_linear_damp(damp);
        base.set_angular_damp(damp);
    }

    /// Flight intent, applied to the physics state inside
    /// `integrate_forces`. Thrust is a force; steering is torque toward a
    /// commanded angular velocity (zero when the stick is centered, so the
    /// engine drives spin back to rest — no uncommanded tumble). We never
    /// assign velocity to steer; the lone exception is stabilize.
    fn fly(&mut self, state: &mut Gd<PhysicsDirectBodyState3D>) {
        let input = Input::singleton();

        let forward = input.get_action_strength(actions::MOVE_FORWARD)
            - input.get_action_strength(actions::MOVE_BACK);
        let strafe = input.get_action_strength(actions::MOVE_RIGHT)
            - input.get_action_strength(actions::MOVE_LEFT);
        let vertical = input.get_action_strength(actions::MOVE_UP)
            - input.get_action_strength(actions::MOVE_DOWN);
        let pitch = input.get_action_strength(actions::LOOK_UP)
            - input.get_action_strength(actions::LOOK_DOWN);
        let yaw = input.get_action_strength(actions::LOOK_LEFT)
            - input.get_action_strength(actions::LOOK_RIGHT);
        let roll = input.get_action_strength(actions::ROLL_RIGHT)
            - input.get_action_strength(actions::ROLL_LEFT);

        let basis = state.get_transform().basis;

        // Thrust: a force along the hull axes. The engine integrates it
        // and (via linear_damp) bleeds it back toward rest — terminal
        // cruise speed ≈ thrust / (mass · linear_damp).
        let thrust_dir =
            basis.col_c() * (-forward) + basis.col_a() * strafe + basis.col_b() * vertical;
        if thrust_dir.length() > 0.01 {
            let force = thrust_dir.normalized()
                * (self.loadout.thrust_power()
                    * self.power_mode.thrust_multiplier()
                    * self.slow.multiplier()
                    * self.ship_thrust_mul);
            state.apply_central_force_ex().force(force).done();
        }

        // Steering: torque toward a commanded angular velocity. Rotation
        // upgrades scale the rate; the controller drives spin toward the
        // command (and to zero on release).
        let turn_mult = self.loadout.rotation_speed() / self.loadout.base.rotation_speed
            * self.ship_rotation_mul;
        let command = basis * Vector3::new(pitch, yaw, roll) * (TURN_RATE * turn_mult);
        let torque = (command - state.get_angular_velocity()) * TURN_GAIN;
        state.apply_torque(torque);

        // Stabilize: the one allowed direct velocity write — a deliberate,
        // discrete hard stop on the spin.
        if input.is_action_pressed(actions::STABILIZE) {
            state.set_angular_velocity(Vector3::ZERO);
        }
    }

    /// Weapons and power routing — runs in `physics_process` because
    /// hitscan needs the physics space unlocked.
    fn handle_weapons_and_power(&mut self, delta: f32) {
        let input = Input::singleton();

        // Power routing: toggle on press.
        let old_mode = self.power_mode;
        if input.is_action_just_pressed(actions::ROUTE_SHIELDS) {
            self.power_mode = if self.power_mode == PowerMode::ShieldBoost {
                PowerMode::Balanced
            } else {
                PowerMode::ShieldBoost
            };
        }
        if input.is_action_just_pressed(actions::ROUTE_WEAPONS) {
            self.power_mode = if self.power_mode == PowerMode::WeaponBoost {
                PowerMode::Balanced
            } else {
                PowerMode::WeaponBoost
            };
        }
        if self.power_mode != old_mode {
            let mode_val = self.power_mode as i32;
            self.base_mut()
                .emit_signal(signals::POWER_MODE_CHANGED, &[Variant::from(mode_val)]);
        }

        // Weapon.
        self.weapon.fire_rate = self.loadout.fire_rate() * self.power_mode.fire_rate_multiplier();
        self.weapon.damage = void_logic::newtypes::Damage::new(self.laser_level.damage());
        self.weapon.tick(delta);
        if let Some(cannon) = self.valkyrie.as_mut() {
            // Cadence stays fixed; the punch follows the equipped laser.
            cannon.damage = valkyrie::state(self.weapon.damage).damage;
            cannon.tick(delta);
        }
        self.subdrone_regen.tick(delta);
        self.age_beams(delta);

        if input.is_action_just_pressed(actions::USE_ITEM) {
            // The Shield Surge (and future consumables): the node only
            // reports the press — charges and the shield are RunState's,
            // so GameManager mediates.
            self.base_mut().emit_signal(signals::SHIELD_BURST_REQUESTED, &[]);
        }
        if input.is_action_pressed(actions::FIRE) {
            // The hull's weapon decides what the trigger does. Everything
            // shares the one cooldown state; the subdrone bay layers its
            // regen clock on top.
            match self.ship_type.spec().weapon {
                WeaponKind::HitscanLaser => {
                    if let FireResult::Fired { damage } = self.weapon.try_fire() {
                        self.fire_dual_lasers(damage.as_f32());
                    }
                    // The Valkyrie rides the same trigger with its own,
                    // slower clock — a heavy center bolt over the beams.
                    let cannon_shot = self.valkyrie.as_mut().map(WeaponState::try_fire);
                    if let Some(FireResult::Fired { damage }) = cannon_shot {
                        self.fire_valkyrie(damage.as_f32());
                    }
                }
                WeaponKind::TrackingLaser => {
                    if let FireResult::Fired { damage } = self.weapon.try_fire() {
                        self.fire_tracking(damage.as_f32());
                    }
                }
                WeaponKind::ClusterMunition => {
                    if let FireResult::Fired { damage } = self.weapon.try_fire() {
                        self.fire_cluster(damage.as_f32());
                    }
                }
                WeaponKind::SubdroneLauncher => {
                    if self.subdrone_regen.ready() {
                        self.launch_subdrone();
                    }
                }
            }
        }
    }

    /// Variant-boundary wrapper: the one f32→Damage conversion for
    /// GDScript and `Object::call` dispatch. Rust callers use
    /// `apply_damage`.
    #[func]
    pub fn take_damage(&mut self, amount: f32, from_position: Vector3) {
        self.apply_damage(void_logic::newtypes::Damage::new(amount), from_position);
    }

    /// Called when a projectile or enemy hits this ship. `from_position` points
    /// at the source so the hit sound can be localized directionally.
    pub fn apply_damage(&mut self, damage: void_logic::newtypes::Damage, from_position: Vector3) {
        // Localize the hit just off the hull, toward whatever struck us — close
        // enough to read as "on the ship" but offset so it points at the threat.
        let self_pos = self.base().get_global_position();
        let to_source = from_position - self_pos;
        let hit_position = if to_source.length() > 0.01 {
            self_pos + to_source.normalized() * HIT_SFX_OFFSET
        } else {
            self_pos
        };
        // The hit sound (shielded vs. hull) is chosen by GameManager, the only
        // place that knows whether the shield held — see `on_player_damaged`.
        // The node just reports the hit and where it came from.
        self.base_mut().emit_signal(
            signals::PLAYER_DAMAGED,
            &[Variant::from(damage.as_f32()), Variant::from(hit_position)],
        );
    }

    /// Reset local state to match a fresh RunState (called by GameManager on new/continue).
    #[func]
    pub fn reset_loadout(&mut self) {
        self.loadout = Loadout::new();
        self.laser_level = LaserLevel::Red;
        self.power_mode = PowerMode::default();
        self.subdrone_regen = subdrone::RegenTimer::new(subdrone::REGEN_SECONDS);
        self.apply_envelope();
    }

    /// Ownership of the Valkyrie cannon, pushed by GameManager on the
    /// purchase receipt and on every player-state sync. The node never
    /// reads the profile itself.
    #[func]
    pub fn set_valkyrie_owned(&mut self, owned: bool) {
        self.valkyrie = if owned {
            Some(valkyrie::state(self.weapon.damage))
        } else {
            None
        };
    }

    #[func]
    pub fn is_valkyrie_owned(&self) -> bool {
        self.valkyrie.is_some()
    }

    #[func]
    pub fn set_laser_level(&mut self, level: i32) {
        if let Some(laser) = LaserLevel::from_level(level as u32) {
            self.laser_level = laser;
            godot_print!("Laser set to {} (damage: {})", laser.display_name(), laser.damage());
        }
    }

    #[func]
    pub fn apply_upgrade(&mut self, name: GString, kind_id: i32, multiplier: f32) {
        let Some(kind) = UpgradeKind::from_id(kind_id) else {
            godot_warn!("Unknown upgrade kind: {kind_id}");
            return;
        };
        let upgrade = Upgrade {
            name: name.to_string(),
            kind,
            multiplier,
        };
        godot_print!("Applied upgrade: {} (x{:.2})", upgrade.name, upgrade.multiplier);
        self.loadout.add_upgrade(upgrade);
        // Re-derive the engine envelope from the updated loadout.
        self.apply_envelope();
    }

    /// The shared bolt pool, found by group like the audio manager.
    fn bolt_pool(&self) -> Option<Gd<super::bolt_pool::BoltPool>> {
        self.base()
            .get_tree()
            .get_first_node_in_group(groups::BOLT_POOL)
            .and_then(|n| n.try_cast::<super::bolt_pool::BoltPool>().ok())
    }

    /// The ship's muzzle and forward direction.
    fn muzzle(&self) -> (Vector3, Vector3) {
        let t = self.base().get_global_transform();
        let forward = -t.basis.col_c();
        (t.origin + forward * 1.2, forward)
    }

    /// Lock the nearest live enemy inside the aim cone: the shared acquire
    /// for every seeking weapon (Talon tracking, Valkyrie burst).
    fn acquire_lock(&self, muzzle: Vector3, forward: Vector3) -> Option<i64> {
        const AIM_CONE_COS: f32 = 0.75;
        const LOCK_RANGE: f32 = 80.0;
        let mut best: Option<(f32, i64)> = None;
        let tree = self.base().get_tree();
        for node in tree.get_nodes_in_group(groups::ENEMIES).iter_shared() {
            let Ok(enemy) = node.try_cast::<Node3D>() else { continue };
            if !enemy.is_visible_in_tree() {
                continue;
            }
            let to = enemy.get_global_position() - muzzle;
            let distance = to.length();
            if distance > LOCK_RANGE || distance <= f32::EPSILON {
                continue;
            }
            if forward.dot(to / distance) < AIM_CONE_COS {
                continue; // outside the aim cone — no lock
            }
            if best.is_none_or(|(d, _)| distance < d) {
                best = Some((distance, enemy.instance_id().to_i64()));
            }
        }
        best.map(|(_, id)| id)
    }

    /// Tracking laser: lock the nearest live enemy inside the aim cone and
    /// fire a homing bolt at it; with no lock the bolt flies ballistic.
    fn fire_tracking(&mut self, damage: f32) {
        const TRACKING_BOLT_SPEED: f32 = 45.0;
        let (muzzle, forward) = self.muzzle();
        let lock = self.acquire_lock(muzzle, forward);
        if let Some(mut pool) = self.bolt_pool() {
            let velocity = forward * TRACKING_BOLT_SPEED;
            match lock {
                Some(target_id) => {
                    pool.bind_mut().fire_homing(muzzle, velocity, damage, target_id)
                }
                None => pool.bind_mut().fire_player(muzzle, velocity, damage),
            }
        }
    }

    /// Valkyrie: empty the rack — a fan of heavy bolts that all curve onto
    /// whatever the reticle holds (the cannon's job is CONNECTING when raw
    /// aim can't — playtest 2026-07-04: one straight bolt read as nothing).
    /// With no lock the fan flies ballistic.
    fn fire_valkyrie(&mut self, damage: f32) {
        let (muzzle, forward) = self.muzzle();
        let lock = self.acquire_lock(muzzle, forward);
        self.valkyrie_bursts = self.valkyrie_bursts.wrapping_add(1);
        let directions = cluster::fragment_directions(
            [forward.x, forward.y, forward.z],
            valkyrie::BURST_COUNT,
            valkyrie::BURST_SPREAD,
            self.valkyrie_bursts,
        );
        if let Some(mut pool) = self.bolt_pool() {
            let mut pool = pool.bind_mut();
            for dir in directions {
                let velocity = Vector3::new(dir[0], dir[1], dir[2]) * valkyrie::BOLT_SPEED;
                match lock {
                    Some(target_id) => pool.fire_homing(muzzle, velocity, damage, target_id),
                    None => pool.fire_player(muzzle, velocity, damage),
                }
            }
        }
        if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
            audio.bind_mut().play_event(SfxEvent::LaserFire);
        }
    }

    /// Cluster cannon: lob a shell that bursts into fragments when it spends
    /// itself (the burst lives on the bolt; see `EnemyBolt::burst`).
    fn fire_cluster(&mut self, damage: f32) {
        const SHELL_SPEED: f32 = 25.0;
        let (muzzle, forward) = self.muzzle();
        if let Some(mut pool) = self.bolt_pool() {
            pool.bind_mut().fire_cluster(muzzle, forward * SHELL_SPEED, damage);
        }
    }

    /// Subdrone bay: spend the regen charge to deploy the first drone still
    /// folded in the bay (the squad is pre-built dormant by the LevelManager;
    /// a launch is a dormancy flip, never a spawn).
    fn launch_subdrone(&mut self) {
        let tree = self.base().get_tree();
        let mut side = 1.0;
        for node in tree.get_nodes_in_group(groups::PLAYER_DRONES).iter_shared() {
            let Ok(drone) = node.try_cast::<super::player_drone::PlayerDrone>() else { continue };
            if drone.bind().is_live() {
                side = -side; // station the next launch on the other flank
                continue;
            }
            if !self.subdrone_regen.consume() {
                return;
            }
            let position = self.base().get_global_position();
            let mut drone = drone;
            drone.bind_mut().activate_at(position, side);
            return;
        }
    }

    fn fire_dual_lasers(&mut self, damage: f32) {
        let global_basis = self.base().get_global_transform().basis;
        let center = self.base().get_global_position() + global_basis.col_b() * 0.5;
        let forward = -global_basis.col_c();
        let right = global_basis.col_a();
        let up = global_basis.col_b();

        let left_origin = center - right * WING_OFFSET;
        let right_origin = center + right * WING_OFFSET;

        // Hit-test down the reticle (centre line) with an aim-assist spread, then
        // converge both visible beams on whatever it found — so the lasers hit
        // where the crosshair points, not parallel-offset from the wings.
        let hit_point = self.cast_forgiving(center, forward, right, up, damage * 2.0);
        self.spawn_beam(left_origin, hit_point);
        self.spawn_beam(right_origin, hit_point);

        // Laser fire SFX (non-positional — it's the player's own gun)
        if let Some(mut audio) = godot_util::find_audio_manager(self.base().get_tree()) {
            audio.bind_mut().play_event(SfxEvent::LaserFire);
        }
    }

    /// Hit-test from the centre line plus a ring of offset rays (aim assist).
    /// The first ray to strike a damageable target wins (full dual damage);
    /// otherwise the centre ray's wall/end point is returned so the beams land.
    fn cast_forgiving(
        &mut self,
        center: Vector3,
        forward: Vector3,
        right: Vector3,
        up: Vector3,
        damage: f32,
    ) -> Vector3 {
        let max_range = self.weapon.max_range;
        let fallback = center + forward * max_range;
        let Some(world) = self.base().get_world_3d() else { return fallback };
        let Some(mut space) = world.get_direct_space_state() else { return fallback };
        let self_rid = self.base().get_rid();

        let r = LASER_AIM_RADIUS;
        // Center, the four cardinals, and the four diagonals: nine rays so
        // a small hull can't slip between the spokes.
        let d = r * std::f32::consts::FRAC_1_SQRT_2;
        let offsets = [
            Vector3::ZERO,
            right * r,
            -right * r,
            up * r,
            -up * r,
            (right + up) * d,
            (right - up) * d,
            (-right + up) * d,
            (-right - up) * d,
        ];
        let mut beam_end = fallback;
        for (i, off) in offsets.iter().enumerate() {
            let origin = center + *off;
            let Some(mut query) =
                PhysicsRayQueryParameters3D::create(origin, origin + forward * max_range)
            else {
                continue;
            };
            query.set_exclude(&array![self_rid]);
            // Point-blank forgiveness: a hugging enemy surrounds the ray
            // origin, and rays don't hit shapes they start inside unless
            // told to — without this, an enemy in your face is unkillable.
            query.set_hit_from_inside(true);
            let result = space.intersect_ray(&query);
            if result.is_empty() {
                continue;
            }
            let Some(hit_pos_var) = result.get("position") else { continue };
            let hit_pos = hit_pos_var.to::<Vector3>();
            if let Some(collider) = result.get("collider") {
                let mut obj = collider.to::<Gd<Node3D>>();
                if obj.has_method(methods::TAKE_DAMAGE) {
                    obj.call(methods::TAKE_DAMAGE, &[Variant::from(damage)]);
                    let normal = result
                        .get("normal")
                        .unwrap_or(Variant::from(Vector3::UP))
                        .to::<Vector3>();
                    self.spawn_hit_sparks(hit_pos, normal);
                    return hit_pos;
                }
            }
            // No target on this ray; the centre ray defines where the beam lands.
            if i == 0 {
                beam_end = hit_pos;
            }
        }
        beam_end
    }

    fn spawn_hit_sparks(&mut self, position: Vector3, normal: Vector3) {
        let mut particles = GpuParticles3D::new_alloc();
        particles.set_amount(12);
        particles.set_lifetime(0.3);
        particles.set_one_shot(true);
        particles.set_explosiveness_ratio(1.0);

        let mut mat = godot_util::particle_burst_material(
            45.0,
            Color::from_rgba(1.0, 0.6, 0.2, 1.0),
            (3.0, 6.0),
            Some((0.5, 1.0)),
        );
        mat.set_direction(normal);
        particles.set_process_material(&mat);

        let mut sphere = SphereMesh::new_gd();
        sphere.set_radius(0.015);
        sphere.set_height(0.03);
        let mut spark_mat = StandardMaterial3D::new_gd();
        spark_mat.set_albedo(Color::from_rgba(1.0, 0.5, 0.1, 1.0));
        spark_mat.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
        spark_mat.set_emission(Color::from_rgba(1.0, 0.4, 0.1, 1.0));
        spark_mat.set_emission_energy_multiplier(8.0);
        sphere.set_material(&spark_mat);
        particles.set_draw_pass_mesh(0, &sphere);

        particles.set_transform(Transform3D::new(Basis::IDENTITY, position));
        if let Some(root) = godot_util::scene_root(self.base().get_tree()) {
            root.clone().add_child(&particles);
            particles.set_emitting(true);
            // Self-destruct after sparks finish
            let mut timer = particles.get_tree().create_timer(0.5);
            let callable = particles.callable("queue_free");
            timer.connect("timeout", &callable);
        }
    }

    fn spawn_beam(&mut self, from: Vector3, to: Vector3) {
        if let Some(mesh_instance) = godot_util::create_beam_mesh(from, to, &self.laser_level.color()) {
            if let Some(root) = godot_util::scene_root(self.base().get_tree()) {
                root.clone().add_child(&mesh_instance);
                self.beam_nodes.push(&mesh_instance, ());
            }
        }
    }

    fn age_beams(&mut self, delta: f32) {
        const BEAM_LIFETIME: f32 = 0.08;
        godot_util::age_beams(&mut self.beam_nodes, delta, BEAM_LIFETIME, &self.laser_level.color());
    }
}
