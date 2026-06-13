//! BodyRegistry: the kinetic world together with every body↔view
//! binding. The ONLY way to create a body is a `register_*` call that
//! also supplies its binding and kind, so "world body without a view
//! slot" is unrepresentable rather than a convention spread across
//! call sites. All `Gd` liveness checks happen here, by reference,
//! before any clone (cloning a freed `Gd` panics).

use godot::classes::{Node3D, SphereMesh, StandardMaterial3D};
use godot::prelude::*;

use super::enemy_drone::EnemyDrone;
use super::ship_controller::ShipController;
use void_logic::consequence::BodyKind;
use void_logic::kinetic_world::{
    BallisticBody, BodyId, ContactEvent, KineticWorld, PoweredBodySpec, WorldSnapshot,
};
use void_logic::kinetics::{ControlInput, Impulse, Mass, Restitution, Retention, SpeedLimits};
use void_logic::newtypes::Damage;
use void_logic::room_assembler::CollisionBox;

/// Enemy bolts: small, fast ballistic bodies with a damage payload.
const BOLT_RADIUS: f32 = 0.15;
const BOLT_MASS_KG: f32 = 2.0;
const BOLT_SPEED: f32 = 15.0;
const BOLT_LIFETIME_S: f32 = 3.0;
/// Clearance between a shooter's hull and a newborn bolt, beyond both
/// radii — keeps the bolt from detonating on its own firer.
const MUZZLE_CLEARANCE: f32 = 0.2;

/// A live enemy bolt: payload and remaining life.
struct BoltSlot {
    id: BodyId,
    damage: Damage,
    age: f32,
}

pub struct BodyRegistry {
    world: KineticWorld,
    /// View node per BodyId; `None` once tombstoned.
    nodes: Vec<Option<Gd<Node3D>>>,
    /// Gameplay classification per BodyId (stable for the body's life).
    kinds: Vec<BodyKind>,
    enemies: Vec<Option<(BodyId, Gd<EnemyDrone>)>>,
    player: Option<(BodyId, Gd<ShipController>)>,
    bolts: Vec<Option<BoltSlot>>,
    /// Shared bolt visuals (one mesh + material for every bolt).
    bolt_mesh: Option<Gd<SphereMesh>>,
    bolt_material: Option<Gd<StandardMaterial3D>>,
}

impl BodyRegistry {
    pub fn new() -> Self {
        Self {
            world: KineticWorld::new(),
            nodes: Vec::new(),
            kinds: Vec::new(),
            enemies: Vec::new(),
            player: None,
            bolts: Vec::new(),
            bolt_mesh: None,
            bolt_material: None,
        }
    }

    /// The single point where a body id meets its view binding — every
    /// register_* pathway funnels through here, in creation order.
    fn bind(&mut self, id: BodyId, node: Gd<Node3D>, kind: BodyKind) -> BodyId {
        debug_assert_eq!(
            id.index(),
            self.nodes.len(),
            "registration must immediately follow body creation"
        );
        self.nodes.push(Some(node));
        self.kinds.push(kind);
        id
    }

    pub fn add_statics(&mut self, boxes: impl IntoIterator<Item = CollisionBox>) {
        self.world.add_statics(boxes);
    }

    pub fn register_prop(&mut self, body: BallisticBody, node: Gd<Node3D>) -> BodyId {
        let id = self.world.add_body(body);
        self.bind(id, node, BodyKind::Prop)
    }

    pub fn register_enemy(&mut self, enemy: Gd<EnemyDrone>, spec: PoweredBodySpec) -> BodyId {
        let id = self.world.add_powered(spec);
        self.enemies.push(Some((id, enemy.clone())));
        self.bind(id, enemy.upcast(), BodyKind::Enemy)
    }

    pub fn register_player(&mut self, ship: Gd<ShipController>, spec: PoweredBodySpec) -> BodyId {
        let id = self.world.add_powered(spec);
        self.player = Some((id, ship.clone()));
        self.bind(id, ship.upcast(), BodyKind::Player)
    }

    /// Spawn a bolt fired by `shooter`: ballistic body + shared
    /// emissive visual, parented under `host`. The muzzle position is
    /// computed here, from geometry this registry owns — the bolt
    /// spawns clear of its firer's hull.
    pub fn register_bolt(
        &mut self,
        host: &mut Gd<Node3D>,
        shooter: BodyId,
        direction: Vector3,
        damage: Damage,
    ) {
        let Some(body) = self.world.body(shooter) else {
            return;
        };
        let origin = Vector3::from_array(body.position());
        let muzzle = origin + direction * (body.radius() + BOLT_RADIUS + MUZZLE_CLEARANCE);
        let from = [muzzle.x, muzzle.y, muzzle.z];
        let id = self.world.add_body(
            BallisticBody::at_rest(from, BOLT_RADIUS, Restitution::clamped(0.0))
                .with_mass(Mass::kilograms(BOLT_MASS_KG)),
        );
        self.world.disturb(
            id,
            Impulse {
                linear: [
                    direction.x * BOLT_SPEED,
                    direction.y * BOLT_SPEED,
                    direction.z * BOLT_SPEED,
                ],
                angular: [0.0; 3],
            },
        );

        let mut visual = Node3D::new_alloc();
        let mut mesh = godot::classes::MeshInstance3D::new_alloc();
        mesh.set_mesh(&self.bolt_mesh());
        mesh.set_surface_override_material(0, &self.bolt_material());
        visual.add_child(&mesh);
        visual.set_position(Vector3::new(from[0], from[1], from[2]));
        visual
            .set_physics_interpolation_mode(godot::classes::node::PhysicsInterpolationMode::OFF);
        host.add_child(&visual);

        self.bolts.push(Some(BoltSlot {
            id,
            damage,
            age: 0.0,
        }));
        self.bind(id, visual, BodyKind::Bolt);
    }

    fn bolt_mesh(&mut self) -> Gd<SphereMesh> {
        self.bolt_mesh
            .get_or_insert_with(|| {
                let mut sphere = SphereMesh::new_gd();
                sphere.set_radius(BOLT_RADIUS);
                sphere.set_height(BOLT_RADIUS * 2.0);
                sphere
            })
            .clone()
    }

    fn bolt_material(&mut self) -> Gd<StandardMaterial3D> {
        self.bolt_material
            .get_or_insert_with(|| {
                let mut material = StandardMaterial3D::new_gd();
                material.set_albedo(Color::from_rgb(1.0, 0.3, 0.1));
                material.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
                material.set_emission(Color::from_rgb(1.0, 0.4, 0.05));
                material
            })
            .clone()
    }

    // ── World forwarding ────────────────────────────────────────────

    pub fn step(&mut self, delta: f32) -> Vec<ContactEvent> {
        self.world.step(delta)
    }

    pub fn snapshot(&self) -> WorldSnapshot {
        self.world.snapshot()
    }

    pub fn set_control(&mut self, id: BodyId, control: ControlInput) {
        self.world.set_control(id, control);
    }

    pub fn set_envelope(&mut self, id: BodyId, retention: Retention, limits: SpeedLimits) {
        self.world.set_envelope(id, retention, limits);
    }

    pub fn ray_blocked(&self, from: [f32; 3], to: [f32; 3], exclude: &[BodyId]) -> bool {
        self.world.ray_blocked(from, to, exclude)
    }

    pub fn body_position(&self, id: BodyId) -> Option<[f32; 3]> {
        self.world.body(id).map(|b| b.position())
    }

    // ── Queries (liveness-checked) ──────────────────────────────────

    /// Classify a body for the consequence rules. Unbound ids are
    /// unrepresentable by construction (`bind` pairs every body), so
    /// reaching the fallback is a registry bug, not a game state. A
    /// kind is stable for the body's lifetime: a freed-but-uncalled
    /// node still classifies, and the executor's liveness checks gate
    /// any effect on it.
    pub fn kind_of(&self, id: BodyId) -> BodyKind {
        debug_assert!(
            id.index() < self.kinds.len(),
            "every body is bound at creation"
        );
        self.kinds.get(id.index()).copied().unwrap_or(BodyKind::Prop)
    }

    /// The player, if their node is still alive.
    pub fn player(&self) -> Option<(BodyId, Gd<ShipController>)> {
        match &self.player {
            Some((id, ship)) if ship.is_instance_valid() => Some((*id, ship.clone())),
            _ => None,
        }
    }

    pub fn enemy(&self, id: BodyId) -> Option<Gd<EnemyDrone>> {
        self.enemies
            .iter()
            .flatten()
            .find(|(body, enemy)| *body == id && enemy.is_instance_valid())
            .map(|(_, enemy)| enemy.clone())
    }

    /// Tombstone every enemy whose node has been freed (death): the
    /// world body is removed and the binding cleared, so the dead can
    /// never be cloned or stepped again.
    pub fn cull_dead_enemies(&mut self) {
        for slot in 0..self.enemies.len() {
            let dead = match &self.enemies[slot] {
                Some((id, enemy)) => (!enemy.is_instance_valid()).then_some(*id),
                None => None,
            };
            if let Some(id) = dead {
                self.world.remove_body(id);
                self.enemies[slot] = None;
                self.nodes[id.index()] = None;
            }
        }
    }

    /// Live enemies, validity-checked. Call `cull_dead_enemies` first
    /// each tick; this is a pure read.
    pub fn live_enemies(&self) -> Vec<(BodyId, Gd<EnemyDrone>)> {
        self.enemies
            .iter()
            .flatten()
            .filter(|(_, enemy)| enemy.is_instance_valid())
            .map(|(id, enemy)| (*id, enemy.clone()))
            .collect()
    }

    pub fn bolt_payload(&self, id: BodyId) -> Option<Damage> {
        self.bolts
            .iter()
            .flatten()
            .find(|bolt| bolt.id == id)
            .map(|bolt| bolt.damage)
    }

    /// Age bolts; expired ones despawn (body, slot, and visual).
    pub fn age_bolts(&mut self, delta: f32) {
        for slot in 0..self.bolts.len() {
            let expired = match self.bolts[slot].as_mut() {
                Some(bolt) => {
                    bolt.age += delta;
                    bolt.age >= BOLT_LIFETIME_S
                }
                None => false,
            };
            if expired {
                self.despawn_bolt_slot(slot);
            }
        }
    }

    pub fn despawn_bolt(&mut self, id: BodyId) {
        if let Some(slot) = self
            .bolts
            .iter()
            .position(|s| s.as_ref().is_some_and(|b| b.id == id))
        {
            self.despawn_bolt_slot(slot);
        }
    }

    fn despawn_bolt_slot(&mut self, slot: usize) {
        if let Some(bolt) = self.bolts[slot].take() {
            self.world.remove_body(bolt.id);
            if let Some(node) = self.nodes[bolt.id.index()].take() {
                if node.is_instance_valid() {
                    let mut node = node;
                    node.queue_free();
                }
            }
        }
    }

    // ── View sync (the landing zone's consumer) ─────────────────────

    /// Position every bound node from the snapshots: interpolated
    /// between ticks, correlated by id (snapshots compact on removal).
    pub fn sync_view(
        &mut self,
        previous: &WorldSnapshot,
        current: &WorldSnapshot,
        fraction: f32,
        frame_delta: f32,
    ) {
        for body in &current.bodies {
            let index = body.id.index();
            let Some(Some(node)) = self.nodes.get_mut(index) else {
                continue;
            };
            if !node.is_instance_valid() {
                continue;
            }
            let prev = previous
                .bodies
                .binary_search_by_key(&index, |b| b.id.index())
                .ok()
                .map(|i| previous.bodies[i])
                .unwrap_or(*body);
            if body.at_rest && prev.position == body.position {
                continue;
            }
            let from = Vector3::new(prev.position[0], prev.position[1], prev.position[2]);
            let to = Vector3::new(body.position[0], body.position[1], body.position[2]);
            node.set_position(from.lerp(to, fraction));
            let spin = Vector3::new(
                body.angular_velocity[0],
                body.angular_velocity[1],
                body.angular_velocity[2],
            ) * frame_delta;
            if spin != Vector3::ZERO {
                let rotation = node.get_rotation() + spin;
                node.set_rotation(rotation);
            }
        }
    }
}

impl Default for BodyRegistry {
    fn default() -> Self {
        Self::new()
    }
}
