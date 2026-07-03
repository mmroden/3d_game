use godot::prelude::*;
use godot::classes::{
    Node3D, INode3D, MeshInstance3D, SphereMesh, StandardMaterial3D, node,
};

use super::bolt_pool::BoltPool;
use super::constants::{groups, methods};
use super::godot_util;
use super::live_handle::{LiveOpt, LiveRef};

/// How long a launched drone stays deployed before folding back into the bay.
const DEPLOY_SECONDS: f64 = 30.0;
/// Engagement range for target acquisition.
const ENGAGE_RANGE: f32 = 40.0;
/// Seconds between the drone's shots.
const FIRE_COOLDOWN: f64 = 1.0;
/// Bolt speed and damage for the drone's gun.
const DRONE_BOLT_SPEED: f32 = 30.0;
const DRONE_BOLT_DAMAGE: f32 = 1.0;
/// Where the drone hovers relative to the player (local-ish escort offset).
const ESCORT_OFFSET: Vector3 = Vector3::new(1.8, 1.2, 0.0);
/// How quickly it glides to its escort station.
const FOLLOW_RATE: f32 = 3.0;

/// A player subdrone (the Hive's weapon): pre-built dormant by the
/// LevelManager (Faucet Principle, tier 1 — the squad is fixed per level,
/// launches only flip dormancy), it escorts the player, hunts the nearest
/// live enemy in range, and fires player-faction bolts through the shared
/// pool. A deployment expires back to dormant; the bay's regen timer (in
/// ShipController) gates the next launch.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct PlayerDrone {
    base: Base<Node3D>,
    /// Logical liveness; engine dormancy flags follow on the deferred boundary.
    live: bool,
    deployed_for: f64,
    fire_cooldown: f64,
    /// Which side of the player this drone stations on (set per squad slot).
    escort_side: f32,
    player: Option<LiveRef<Node3D>>,
}

#[godot_api]
impl INode3D for PlayerDrone {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            live: false,
            deployed_for: 0.0,
            fire_cooldown: 0.0,
            escort_side: 1.0,
            player: None,
        }
    }

    fn ready(&mut self) {
        self.base_mut().add_to_group(groups::PLAYER_DRONES);

        // A small glowing pod — reads as friendly (cyan) against the enemy fleet.
        let mut sphere = SphereMesh::new_gd();
        sphere.set_radius(0.25);
        sphere.set_height(0.5);
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_albedo(Color::from_rgb(0.2, 0.9, 0.9));
        mat.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
        mat.set_emission(Color::from_rgb(0.1, 0.8, 0.9));
        let mut mesh = MeshInstance3D::new_alloc();
        mesh.set_mesh(&sphere);
        mesh.set_surface_override_material(0, &mat);
        self.base_mut().add_child(&mesh);
        let mut node: Gd<Node3D> = self.base().clone().upcast();
        godot_util::attach_glow_light(&mut node, &[0.2, 0.9, 0.9], 2.5, 5.0);

        // Pre-built dormant (Faucet tier 1): the bay launches by flipping this.
        self.deactivate();
    }

    fn physics_process(&mut self, delta: f64) {
        if !self.live {
            return;
        }
        self.deployed_for += delta;
        if self.deployed_for >= DEPLOY_SECONDS {
            self.deactivate(); // deployment over — fold back into the bay
            return;
        }

        // Escort: glide toward the station point beside the player.
        if self.player.with(|p| p.is_inside_tree()) != Some(true) {
            self.acquire_player();
        }
        if let Some(station) = self.player.with(|p| {
            let t = p.get_global_transform();
            t.origin
                + t.basis.col_a() * (ESCORT_OFFSET.x * self.escort_side)
                + t.basis.col_b() * ESCORT_OFFSET.y
        }) {
            let pos = self.base().get_global_position();
            let next = pos + (station - pos) * (FOLLOW_RATE * delta as f32).min(1.0);
            self.base_mut().set_global_position(next);
        }

        // Gunnery: nearest live enemy in range, one shot per cooldown.
        self.fire_cooldown -= delta;
        if self.fire_cooldown <= 0.0 {
            if let Some(target_pos) = self.nearest_enemy_position() {
                self.fire_at(target_pos);
                self.fire_cooldown = FIRE_COOLDOWN;
            }
        }
    }
}

#[godot_api]
impl PlayerDrone {
    /// Launch this dormant drone at the player's side. Reset-in-place; the
    /// engine flags flip on the deferred boundary, like every pooled entity.
    #[func]
    pub fn activate_at(&mut self, position: Vector3, escort_side: f32) {
        self.deployed_for = 0.0;
        self.fire_cooldown = 0.0;
        self.escort_side = escort_side;
        self.live = true;
        self.base_mut().set_global_position(position);
        self.base_mut().call_deferred(methods::APPLY_DORMANCY, &[true.to_variant()]);
        self.base_mut().reset_physics_interpolation();
    }

    /// Whether this drone is currently deployed (the bay launches into the
    /// first dormant slot).
    #[func]
    pub fn is_live(&self) -> bool {
        self.live
    }

    fn deactivate(&mut self) {
        self.live = false;
        self.base_mut().call_deferred(methods::APPLY_DORMANCY, &[false.to_variant()]);
    }

    #[func]
    fn apply_dormancy(&mut self, live: bool) {
        let mode = if live { node::ProcessMode::INHERIT } else { node::ProcessMode::DISABLED };
        let mut base = self.base_mut();
        base.set_visible(live);
        base.set_process_mode(mode);
    }

    fn acquire_player(&mut self) {
        let tree = self.base().get_tree();
        let players = tree.get_nodes_in_group(groups::PLAYER);
        if let Some(player) = players.get(0) {
            self.player = Some(LiveRef::new(&player.cast::<Node3D>()));
        }
    }

    /// The nearest live (visible) enemy within engagement range.
    fn nearest_enemy_position(&self) -> Option<Vector3> {
        let pos = self.base().get_global_position();
        let tree = self.base().get_tree();
        let mut best: Option<(f32, Vector3)> = None;
        for node in tree.get_nodes_in_group(groups::ENEMIES).iter_shared() {
            let Ok(enemy) = node.try_cast::<Node3D>() else { continue };
            if !enemy.is_visible_in_tree() {
                continue; // dormant minion or culled room
            }
            let enemy_pos = enemy.get_global_position();
            let distance = pos.distance_to(enemy_pos);
            if distance > ENGAGE_RANGE {
                continue;
            }
            if best.is_none_or(|(d, _)| distance < d) {
                best = Some((distance, enemy_pos));
            }
        }
        best.map(|(_, p)| p)
    }

    /// One player-faction bolt through the shared pool.
    fn fire_at(&self, target: Vector3) {
        let pos = self.base().get_global_position();
        let dir = (target - pos).normalized();
        if !dir.is_finite() {
            return;
        }
        let pool = self
            .base()
            .get_tree()
            .get_first_node_in_group(groups::BOLT_POOL)
            .and_then(|n| n.try_cast::<BoltPool>().ok());
        if let Some(mut pool) = pool {
            pool.bind_mut()
                .fire_player(pos + dir * 0.6, dir * DRONE_BOLT_SPEED, DRONE_BOLT_DAMAGE);
        }
    }
}
