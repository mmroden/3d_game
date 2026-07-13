use godot::prelude::*;
use godot::classes::{
    Node3D, INode3D, MeshInstance3D, SphereMesh, StandardMaterial3D,
    node, base_material_3d,
};

use super::constants::methods;
use super::live_handle::{LiveOpt, LiveRef};

/// Peak opacity of a live cloud — thick enough to murk the view from inside,
/// sheer enough that the room still reads through it. The cloud attacks vision
/// only; it has no collider and deals no damage.
const DUST_ALPHA: f32 = 0.8;
/// Dust colour — a dull grey-tan so the fog reads as debris, not smoke.
const DUST_RGB: (f32, f32, f32) = (0.45, 0.42, 0.36);
/// Fraction of life spent swelling from a point to full blast radius, so the
/// cloud *blooms* out of the detonation instead of snapping into being.
const GROW_FRAC: f64 = 0.1;
/// Fraction of life (at the tail) spent fading to nothing, so the cloud thins
/// out rather than blinking off.
const FADE_FRAC: f64 = 0.25;

/// A single occluding dust cloud: one slot of the [`CloudPool`](super::cloud_pool::CloudPool)
/// ring (Faucet Principle, tier 2). A translucent, unshaded sphere with no
/// collider — it denies vision, never hull. The mesh is a *unit* sphere; a
/// spawn scales the node to the blast radius, so one preallocated slot serves
/// any bomber. Cull is disabled so the interior stays drawn when the player
/// flies inside the fog. Expiry returns the slot to *dormant* rather than
/// freeing it, so the pool reuses it.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct DustCloud {
    base: Base<Node3D>,
    /// The fog sphere, held weakly so its per-slot material can animate alpha
    /// without a cached `Gd` that would dangle on level rebuild.
    mesh: Option<LiveRef<MeshInstance3D>>,
    /// Logical liveness, flipped immediately on spawn/expiry. The engine-side
    /// dormancy flags follow on the deferred boundary — Godot blocks visibility
    /// and processing toggles inside physics callbacks — so this field, not
    /// `is_visible`, is the truth `physics_process` and the pool read.
    live: bool,
    age: f64,
    /// This life's linger, in seconds (the def's `cloud_seconds`).
    lifetime: f64,
    /// This life's full radius (the detonation's blast radius). The unit mesh
    /// is scaled to it once grow-in completes.
    radius: f32,
}

#[godot_api]
impl INode3D for DustCloud {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            mesh: None,
            live: false,
            age: 0.0,
            lifetime: 0.0,
            radius: 0.0,
        }
    }

    fn ready(&mut self) {
        let mut sphere = SphereMesh::new_gd();
        sphere.set_radius(1.0);
        sphere.set_height(2.0);

        // Each slot owns its own material instance so its alpha animates
        // independently of every other cloud in flight.
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_transparency(base_material_3d::Transparency::ALPHA);
        mat.set_shading_mode(base_material_3d::ShadingMode::UNSHADED);
        // Draw both faces: the player flies into the fog and must still see it
        // from the inside.
        mat.set_cull_mode(base_material_3d::CullMode::DISABLED);
        mat.set_albedo(Color::from_rgba(DUST_RGB.0, DUST_RGB.1, DUST_RGB.2, DUST_ALPHA));

        let mut mesh = MeshInstance3D::new_alloc();
        mesh.set_mesh(&sphere);
        mesh.set_surface_override_material(0, &mat);
        self.base_mut().add_child(&mesh);
        self.mesh = Some(LiveRef::new(&mesh));

        // Preallocated slots enter the tree dormant: they cost only memory until
        // spawned. The pool builds the whole ring during the loading phase.
        self.deactivate();
    }

    fn physics_process(&mut self, delta: f64) {
        // The engine flags lag liveness by one deferred flush; don't age or draw
        // during that gap.
        if !self.live {
            return;
        }
        self.age += delta;
        if self.age >= self.lifetime {
            // A spent cloud returns to dormant for reuse — never freed.
            self.deactivate();
            return;
        }
        self.apply_visual();
    }
}

#[godot_api]
impl DustCloud {
    /// Spawn this slot as a cloud at `position`, blooming to `radius` and
    /// lingering `seconds`. Reset-in-place: transform, radius, lifetime, age,
    /// and visual are reset and the slot flips live — no allocation, ever.
    pub fn spawn(&mut self, position: Vector3, radius: f32, seconds: f32) {
        self.radius = radius;
        self.lifetime = seconds as f64;
        self.age = 0.0;
        self.base_mut().set_global_position(position);
        self.apply_visual();
        self.activate();
        // Placed at the detonation point, not moved there — don't interpolate
        // from the slot's previous resting position.
        self.base_mut().reset_physics_interpolation();
    }

    /// Drive scale and opacity from the current age: swell to full radius over
    /// the first [`GROW_FRAC`] of life, hold, then fade alpha to zero over the
    /// last [`FADE_FRAC`].
    fn apply_visual(&mut self) {
        let t = if self.lifetime > 0.0 { self.age / self.lifetime } else { 1.0 };
        let grow = (t / GROW_FRAC).min(1.0) as f32;
        let scale = self.radius * grow;
        self.base_mut().set_scale(Vector3::splat(scale));

        let fade = ((1.0 - t) / FADE_FRAC).clamp(0.0, 1.0) as f32;
        let alpha = DUST_ALPHA * fade;
        self.mesh.with(|m| {
            if let Some(mat) = m
                .get_surface_override_material(0)
                .and_then(|mat| mat.try_cast::<StandardMaterial3D>().ok())
            {
                let mut mat = mat;
                mat.set_albedo(Color::from_rgba(DUST_RGB.0, DUST_RGB.1, DUST_RGB.2, alpha));
            }
        });
    }

    /// Whether this slot is currently drifting (spawned, not yet spent). The
    /// pool sums this across the ring for its live count.
    #[func]
    pub fn is_live(&self) -> bool {
        self.live
    }

    /// Go live: logical flag now, engine flags on the deferred boundary.
    fn activate(&mut self) {
        self.live = true;
        self.base_mut().call_deferred(methods::APPLY_DORMANCY, &[true.to_variant()]);
    }

    /// Return to dormant: logical flag now, engine flags on the deferred
    /// boundary. `physics_process` gates on the logical flag, so nothing
    /// observable happens during the one-flush lag.
    fn deactivate(&mut self) {
        self.live = false;
        self.base_mut().call_deferred(methods::APPLY_DORMANCY, &[false.to_variant()]);
    }

    /// Flip the engine-side dormancy flags together: visibility and processing.
    /// Always invoked via `call_deferred` — expiry fires inside
    /// `physics_process`, where Godot blocks these setters; the deferred
    /// boundary is always legal. (No collision/monitoring to toggle: the cloud
    /// has no collider.)
    #[func]
    fn apply_dormancy(&mut self, live: bool) {
        let mode = if live { node::ProcessMode::INHERIT } else { node::ProcessMode::DISABLED };
        let mut base = self.base_mut();
        base.set_visible(live);
        base.set_process_mode(mode);
    }

    /// Force this slot back to dormant regardless of its drift state — for the
    /// pool to clear the ring when a level rebuilds.
    pub fn deactivate_for_pool(&mut self) {
        self.deactivate();
    }
}
