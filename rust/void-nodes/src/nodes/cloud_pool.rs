use godot::prelude::*;
use godot::classes::{Node3D, INode3D};

use super::constants::groups;
use super::dust_cloud::DustCloud;
use super::live_handle::LiveVec;

/// Ring capacity: the number of dust clouds that can *linger concurrently*, not
/// the total a level detonates. Dormant slots cost only memory (a hidden,
/// non-processing Node3D with a mesh); live cost scales with drifting clouds
/// alone. Sized to comfortably cover a room's worth of overlapping bomber
/// blasts — and if it wraps, overwriting the oldest cloud (which is already
/// fading out) is imperceptible and keeps the buffer total: no allocation
/// fallback, no error path.
const CLOUD_CAPACITY: i64 = 16;

/// Dust-cloud ring buffer (Faucet Principle, tier 2). Preallocates its whole
/// ring of [`DustCloud`] slots once, during the loading phase, and thereafter
/// only flips slots between dormant and live — nothing is instantiated or freed
/// while play runs. Every bomber that detonates with a `cloud_seconds` linger
/// routes its cloud through the single pool via [`spawn`](CloudPool::spawn);
/// spawning past capacity overwrites the oldest live cloud by advancing a
/// cursor around the ring.
///
/// Parented under `LevelManager` as a sibling of the room containers (not inside
/// one), so clouds are not toggled off by per-room visibility culling, and the
/// pool survives level regeneration. On rebuild the pool re-dormants its whole
/// ring (clouds lingering from the previous level vanish cleanly) rather than
/// reallocating.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct CloudPool {
    base: Base<Node3D>,
    /// Concurrent-cloud capacity. Overridable per scene / per test before the
    /// pool enters the tree; defaults to [`CLOUD_CAPACITY`].
    #[export]
    capacity: i64,
    /// Weak handles to the ring slots. `LiveVec` stores instance ids, never a
    /// raw `Gd` (the crate lint forbids cached node handles that can dangle).
    slots: LiveVec<DustCloud>,
    /// Next slot to spawn into. Advances one step per cloud and wraps, so a full
    /// ring's next spawn lands on the oldest live cloud and overwrites it.
    cursor: usize,
}

#[godot_api]
impl INode3D for CloudPool {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            capacity: CLOUD_CAPACITY,
            slots: LiveVec::new(),
            cursor: 0,
        }
    }

    fn ready(&mut self) {
        // Discoverable by every detonation site (bombers deep under room
        // containers) without a fragile node path — mirrors the bolt pool.
        self.base_mut().add_to_group(groups::CLOUD_POOL);
        self.build_ring();
    }
}

#[godot_api]
impl CloudPool {
    /// Spawn a dust cloud at the ring cursor: bloom to `radius`, linger
    /// `seconds`. Reset-in-place — the slot's own `spawn` sets its transform,
    /// radius, lifetime, and age and flips it live. No allocation, ever — a
    /// full ring reuses its oldest slot.
    #[func]
    pub fn spawn(&mut self, position: Vector3, radius: f32, seconds: f32) {
        if self.slots.is_empty() {
            return; // ring not built (no capacity) — nothing to spawn
        }
        let slot_index = self.cursor % self.slots.len();
        self.cursor = self.cursor.wrapping_add(1);
        if let Some(mut slot) = self.slots.get_live(slot_index) {
            slot.bind_mut().spawn(position, radius, seconds);
        }
    }

    /// How many clouds are currently drifting. Sums the ring rather than
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

    /// Return every slot to dormant. Called when a level rebuilds so clouds
    /// still lingering from the previous level don't persist — the ring is
    /// reused, not reallocated.
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
            let cloud = DustCloud::new_alloc();
            self.base_mut().add_child(&cloud);
            self.slots.push(&cloud, ());
        }
    }
}
