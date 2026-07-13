use godot::prelude::*;
use godot::classes::{Node3D, INode3D};

use super::constants::groups;
use super::enemy_bolt::{BoltPayload, BoltProfile, EnemyBolt};
use super::live_handle::LiveVec;
use void_logic::armament::{homing, Faction};

/// Ring capacity: the number of bolts that can be *concurrently* in flight, not
/// the total a level fires. Dormant slots cost only memory (a hidden,
/// non-processing, non-colliding Area3D); live cost scales with active bolts
/// alone. Sized generously so the ring effectively never wraps in play — and if
/// it does, overwriting the oldest live bolt (which vanishes in its final tenths
/// of a second) is imperceptible and keeps the buffer total: no allocation
/// fallback, no error path.
const BOLT_CAPACITY: i64 = 2048;

/// Bolt ring buffer (Faucet Principle, tier 2). Preallocates its whole ring of
/// [`EnemyBolt`] slots once, during the loading phase, and thereafter only flips
/// slots between dormant and active — nothing is instantiated or freed while
/// play runs. Every enemy that fires routes its shot through the single pool via
/// [`fire`](BoltPool::fire); firing past capacity overwrites the oldest live
/// bolt by advancing a cursor around the ring.
///
/// Parented under `LevelManager` as a sibling of the room containers (not inside
/// one), so bolts are not toggled off by per-room visibility culling, and the
/// pool survives level regeneration — `LevelManager::build_level` frees only the
/// room nodes. On rebuild the pool re-dormants its whole ring (in-flight bolts
/// from the previous level vanish cleanly) rather than reallocating.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct BoltPool {
    base: Base<Node3D>,
    /// Concurrent-bolt capacity. Overridable per scene / per test before the
    /// pool enters the tree; defaults to [`BOLT_CAPACITY`].
    #[export]
    capacity: i64,
    /// Weak handles to the ring slots. `LiveVec` stores instance ids, never a
    /// raw `Gd` (the crate lint forbids cached node handles that can dangle).
    slots: LiveVec<EnemyBolt>,
    /// Next slot to fire into. Advances one step per shot and wraps, so a full
    /// ring's next fire lands on the oldest live bolt and overwrites it.
    cursor: usize,
}

#[godot_api]
impl INode3D for BoltPool {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            capacity: BOLT_CAPACITY,
            slots: LiveVec::new(),
            cursor: 0,
        }
    }

    fn ready(&mut self) {
        // Discoverable by every fire site (enemies deep under room containers)
        // without a fragile node path — mirrors how the AudioManager is found.
        self.base_mut().add_to_group(groups::BOLT_POOL);
        self.build_ring();
    }
}

#[godot_api]
impl BoltPool {
    /// Fire an enemy ballistic bolt at the ring cursor — the classic path
    /// every enemy fire site calls.
    #[func]
    pub fn fire(&mut self, position: Vector3, velocity: Vector3, damage: f32) {
        self.arm_next(position, velocity, damage, BoltProfile::ballistic(Faction::Enemy));
    }

    /// Fire a player ballistic bolt (subdrones, cluster fragments).
    #[func]
    pub fn fire_player(&mut self, position: Vector3, velocity: Vector3, damage: f32) {
        self.arm_next(position, velocity, damage, BoltProfile::ballistic(Faction::Player));
    }

    /// Fire a player homing bolt locked onto the node with this instance id
    /// (the tracking laser). An invalid or dead id flies ballistic.
    #[func]
    pub fn fire_homing(&mut self, position: Vector3, velocity: Vector3, damage: f32, target_instance_id: i64) {
        let target = InstanceId::try_from_i64(target_instance_id);
        self.arm_next(position, velocity, damage, BoltProfile {
            homing_target: target,
            turn_rad: homing::TURN_RATE,
            ..BoltProfile::ballistic(Faction::Player)
        });
    }

    /// Fire an enemy homing bolt at the def's declared steer rate (rad/s) —
    /// the `bolt_turn_deg` switch's fire path. An invalid id flies ballistic.
    #[func]
    pub fn fire_enemy_homing(&mut self, position: Vector3, velocity: Vector3, damage: f32, target_instance_id: i64, turn_rad: f32) {
        let target = InstanceId::try_from_i64(target_instance_id);
        self.arm_next(position, velocity, damage, BoltProfile {
            homing_target: target,
            turn_rad,
            ..BoltProfile::ballistic(Faction::Enemy)
        });
    }

    /// Fire a player cluster shell: it bursts into fragments (through this
    /// same pool) when it spends itself.
    #[func]
    pub fn fire_cluster(&mut self, position: Vector3, velocity: Vector3, damage: f32) {
        self.arm_next(position, velocity, damage, BoltProfile {
            payload: BoltPayload::ClusterBurst,
            ..BoltProfile::ballistic(Faction::Player)
        });
    }

    /// Arm the slot at the ring cursor, then advance the cursor.
    /// Reset-in-place: the slot's own `arm` sets its transform, velocity,
    /// damage, age, faction, lock, payload, and generation and flips it live.
    /// No allocation, ever — a full ring reuses its oldest slot.
    fn arm_next(
        &mut self,
        position: Vector3,
        velocity: Vector3,
        damage: f32,
        profile: BoltProfile,
    ) {
        if self.slots.is_empty() {
            return; // ring not built (no capacity) — nothing to fire
        }
        let slot_index = self.cursor % self.slots.len();
        self.cursor = self.cursor.wrapping_add(1);
        if let Some(mut slot) = self.slots.get_live(slot_index) {
            slot.bind_mut().arm(position, velocity, damage, profile);
        }
    }

    /// How many slots are currently in flight. Sums the ring rather than
    /// tracking a counter, so it can't drift from the slots' own state.
    #[func]
    fn live_count(&self) -> i64 {
        let mut n = 0;
        self.slots.for_each_live(|_, slot, _| {
            if slot.bind().is_live() {
                n += 1;
            }
        });
        n
    }

    /// Return every slot to dormant. Called when a level rebuilds so bolts still
    /// in flight from the previous level don't linger — the ring is reused, not
    /// reallocated.
    pub fn reset(&mut self) {
        self.cursor = 0;
        self.slots.for_each_live(|_, slot, _| {
            let mut slot = slot.clone();
            slot.bind_mut().deactivate_for_pool();
        });
    }

    /// Preallocate the whole ring as dormant slots. Loading-phase work: this is
    /// the one structural allocation, done before the spigots open.
    fn build_ring(&mut self) {
        for _ in 0..self.capacity {
            let bolt = EnemyBolt::new_alloc();
            self.base_mut().add_child(&bolt);
            self.slots.push(&bolt, ());
        }
    }
}
