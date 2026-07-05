use godot::classes::{
    Area3D, BoxMesh, BoxShape3D, CollisionShape3D, IArea3D, IStaticBody3D, MeshInstance3D,
    Node3D, StandardMaterial3D, StaticBody3D,
};
use godot::prelude::*;

use super::constants::{groups, methods, signals};

/// Barrier thickness (m) along the doorway's through axis.
const GATE_THICKNESS: f32 = 0.4;

/// The arena seal: a pre-built energy barrier across the boss room's one
/// doorway (Faucet Principle — built dormant at level creation, only ever
/// FLIPPED during the run). Unsealed it is invisible and intangible; sealed
/// it is a solid glowing wall behind the player. GameManager drives the
/// flips off the `BossFight` FSM; this node holds no fight logic.
#[derive(GodotClass)]
#[class(base=StaticBody3D)]
pub struct BossGate {
    base: Base<StaticBody3D>,
    /// Doorway span (width, height) the barrier must cover, set by the
    /// LevelManager before the node enters the tree.
    span: Vector2,
    sealed: bool,
}

#[godot_api]
impl IStaticBody3D for BossGate {
    fn init(base: Base<StaticBody3D>) -> Self {
        Self {
            base,
            span: Vector2::new(4.0, 5.0),
            sealed: false,
        }
    }

    fn ready(&mut self) {
        let size = Vector3::new(self.span.x, self.span.y, GATE_THICKNESS);

        let mut mesh = BoxMesh::new_gd();
        mesh.set_size(size);
        let mut mat = StandardMaterial3D::new_gd();
        mat.set_albedo(Color::from_rgba(1.0, 0.25, 0.15, 0.65));
        mat.set_transparency(godot::classes::base_material_3d::Transparency::ALPHA);
        mat.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
        mat.set_emission(Color::from_rgb(1.0, 0.2, 0.1));
        mat.set_emission_energy_multiplier(2.5);
        mesh.set_material(&mat);
        let mut visual = MeshInstance3D::new_alloc();
        visual.set_mesh(&mesh);
        self.base_mut().add_child(&visual);

        let mut shape = CollisionShape3D::new_alloc();
        let mut boxy = BoxShape3D::new_gd();
        boxy.set_size(size);
        shape.set_shape(&boxy);
        shape.set_name("GateShape");
        self.base_mut().add_child(&shape);

        // Born unsealed: invisible, intangible, passable inbound.
        self.apply_sealed_state();
    }
}

#[godot_api]
impl BossGate {
    /// Doorway span the barrier covers; set before adding to the tree.
    pub fn set_span(&mut self, width: f32, height: f32) {
        self.span = Vector2::new(width, height);
    }

    #[func]
    pub fn is_sealed(&self) -> bool {
        self.sealed
    }

    /// Seal or open the barrier. Flips directly — from a physics callback,
    /// invoke it via `call_deferred` (the house dormancy pattern) so space
    /// state never mutates mid-step.
    #[func]
    pub fn set_sealed(&mut self, sealed: bool) {
        if self.sealed != sealed {
            self.sealed = sealed;
            self.apply_sealed_state();
        }
    }

    fn apply_sealed_state(&mut self) {
        let sealed = self.sealed;
        self.base_mut().set_visible(sealed);
        if let Some(shape) = self
            .base()
            .try_get_node_as::<CollisionShape3D>("GateShape")
        {
            let mut shape = shape;
            shape.set_disabled(!sealed);
        }
    }
}

/// One-shot arena entry sensor: a volume spanning the boss room's doorway,
/// just inside the arena. The first player crossing emits
/// `boss_arena_entered` (wired to GameManager by the level signal pass) and
/// the sensor disarms itself — the `BossFight` FSM refuses repeats anyway,
/// but a dead sensor also costs no further overlap tests.
#[derive(GodotClass)]
#[class(base=Area3D)]
pub struct BossTrigger {
    base: Base<Area3D>,
    /// Sensor volume, set by the LevelManager before the node enters the tree.
    extents: Vector3,
}

#[godot_api]
impl IArea3D for BossTrigger {
    fn init(base: Base<Area3D>) -> Self {
        Self {
            base,
            extents: Vector3::new(6.0, 5.0, 6.0),
        }
    }

    fn ready(&mut self) {
        let mut shape = CollisionShape3D::new_alloc();
        let mut boxy = BoxShape3D::new_gd();
        boxy.set_size(self.extents);
        shape.set_shape(&boxy);
        self.base_mut().add_child(&shape);

        self.base_mut().set_monitoring(true);
        self.base_mut().set_collision_mask(1);
        self.base_mut().set_collision_layer(0);

        let callable = self.base().callable(methods::ON_BODY_ENTERED);
        self.base_mut().connect(signals::BODY_ENTERED, &callable);
    }
}

#[godot_api]
impl BossTrigger {
    #[signal]
    fn boss_arena_entered();

    /// Sensor volume; set before adding to the tree.
    pub fn set_extents(&mut self, extents: Vector3) {
        self.extents = extents;
    }

    #[func]
    fn on_body_entered(&mut self, body: Gd<Node3D>) {
        if body.is_in_group(groups::PLAYER) {
            self.base_mut()
                .emit_signal(signals::BOSS_ARENA_ENTERED, &[]);
            self.base_mut()
                .set_deferred("monitoring", &Variant::from(false));
        }
    }
}
