use godot::prelude::*;
use godot::classes::{
    BoxShape3D, CollisionShape3D, ConcavePolygonShape3D, MeshInstance3D, Node3D, INode3D,
    OmniLight3D, PackedScene, ResourceLoader, RigidBody3D, StaticBody3D,
};

use super::constants::{groups, methods, nodes, scenes, signals};
use super::godot_util;
use super::live_handle::{LiveRef, LiveVec, LiveOpt};
use super::bolt_pool::BoltPool;
use super::cloud_pool::CloudPool;
use super::boss_gate::{BossGate, BossTrigger};
use super::enemy_drone::EnemyDrone;
use super::currency_cache::CurrencyCache;
use super::player_drone::PlayerDrone;
use super::portal::Portal;
use void_logic::armament::subdrone;
use void_logic::currency::{CurrencyKind, ORGANIC_CACHE_AMOUNT};
use super::ship_controller::ShipController;
use super::telemetry::Telemetry;
use super::views::ViewManager;
use rand::SeedableRng;
use rand::rngs::SmallRng;

use void_logic::generator;
use void_logic::level_spec::LevelSpec;
use void_logic::level_assembly::{self, RoomBounds};
use void_logic::level_graph::{LevelGraph, RENDER_ROOM_DEPTH};
use void_logic::room_furnisher::LightState;
use void_logic::room_assembler::{Collision, MeshPlacement};
use void_logic::portal as portal_sys;
use void_logic::seed::Seed;

fn vec3(a: [f32; 3]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}

fn arr(v: Vector3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

/// Assembles a level on demand: generates the graph, assembles room
/// geometry from modular pieces, hands each object to Godot/Jolt with a
/// collider, and spawns lights. Generation is driven solely by
/// GameManager (one pathway); LevelManager never self-generates. Physics
/// is the engine's: after build, this node only culls rooms and flickers
/// lights.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct LevelManager {
    base: Base<Node3D>,

    #[export]
    current_level: i32,

    /// Capture-mode culling override (`--cull=0`): keep every room
    /// visible regardless of the player's room — exterior establishing
    /// shots park the camera OUTSIDE all rooms, where depth-culling
    /// would otherwise leave the level mostly hidden. Same knob family
    /// as `--populace`/`--sbs` on GameManager. -1 = normal culling.
    #[export]
    cull_override: i32,

    /// Capture diagnostic (`--ambient=1`): flat white ambient on every
    /// build, so a capture's remaining black is a real opening, never an
    /// unlit wall. -1 = the authored look.
    #[export]
    ambient_override: i32,

    telemetry: Telemetry,
    /// One container node per room (index = room-list order). Toggling
    /// a container's visibility culls that whole room — geometry and
    /// its lights — in one move.
    room_nodes: LiveVec<Node3D>,
    /// Per-room world bounds, parallel to `room_nodes`, for resolving
    /// which room a world point is in.
    room_bounds: Vec<RoomBounds>,
    /// The level's topology graph, retained for the level's lifetime — the
    /// authority for cull visibility (and, later, mapping and route
    /// queries). Empty until the first `generate_level`.
    level_graph: LevelGraph,
    /// The room the player currently occupies; culling only recomputes
    /// when this changes.
    current_room: Option<usize>,
    /// Cached player node, for reading position each tick.
    player: Option<LiveRef<Node3D>>,
    /// The level's bolt ring buffer (Faucet Principle tier 2). Built once, the
    /// first time a level is generated, and reused across regenerations — it is
    /// a sibling of the room containers so bolts escape per-room culling, and it
    /// outlives the room nodes `build_level` frees. Re-dormanted on each rebuild.
    bolt_pool: Option<LiveRef<BoltPool>>,
    /// The level's dust-cloud ring buffer (Faucet Principle tier 2). Built once
    /// and reused across regenerations, exactly like `bolt_pool` — a sibling of
    /// the room containers so clouds escape per-room culling. Re-dormanted on
    /// each rebuild.
    cloud_pool: Option<LiveRef<CloudPool>>,
    /// The level's blue-cache pool (Faucet Principle tier 1): one dormant
    /// currency cache per enemy, pre-built during the load and parented here
    /// under the LevelManager (not under a room container) so a dropped cache
    /// persists and stays visible when the player leaves the room it dropped
    /// in — drops are collected across rooms. Unlike the bolt ring these are
    /// one-life-per-level, so they are freed and rebuilt on each regeneration
    /// rather than reused. (Green loot-spawn caches parent under their rooms
    /// and are freed with them.)
    caches: LiveVec<CurrencyCache>,
    /// The player's subdrone squad (Faucet tier 1): SQUAD_SIZE drones
    /// pre-built dormant per level, launched by dormancy flips from the
    /// Hive's bay. One-life-per-level like the caches: freed and rebuilt on
    /// regeneration.
    player_drones: LiveVec<PlayerDrone>,
    /// The room seal (empty off sealed levels) — every active connector of a
    /// boss arena OR a miniboss room gets a gate, pre-built unsealed with the
    /// level, flipped as ONE set by GameManager off the fight FSM. A boss
    /// arena has one connector; a miniboss room has however many.
    boss_gates: LiveVec<BossGate>,
    /// The exit portal on a boss level (`None` elsewhere) — pre-built
    /// dormant; activates when the fight's reward is collected.
    boss_portal: Option<LiveRef<Portal>>,
    /// The staged boss on a boss level (`None` elsewhere) — pre-built
    /// DORMANT like any reserved minion; the arena entry rises it
    /// (playtest 2026-07-06: a live boss died to corridor sniping).
    staged_boss: Option<LiveRef<EnemyDrone>>,
    /// The typed description of the level under (or after) construction —
    /// THE door for every level attribute (pitch, paradigm, roster, boss
    /// staging). Constructed by GameManager (production) or the test door
    /// (`generate_level`); retained beside the graph for the level's life.
    spec: Option<LevelSpec>,
    /// Blinking light fixtures and their full ("on") energy, modulated
    /// each frame so a flickering abandoned base reads as alive.
    blinking_lights: LiveVec<OmniLight3D, f32>,
    /// Accumulated time driving the blink phase.
    blink_time: f32,
    /// The world environment's ambient settings as they were BEFORE a fixed
    /// level overrode them (source, color, energy) — captured on the first
    /// override, restored on the next non-overriding build. The space
    /// levels' dark ambient is authored in main.tscn; the house's daylight
    /// ambient is authored on its environment file. One truth each.
    saved_ambient: Option<(i32, Color, f32)>,
}

/// Dim fixtures emit a fraction of their rated energy — a weak glow.
const DIM_ENERGY_FACTOR: f32 = 0.5;

#[godot_api]
impl INode3D for LevelManager {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            current_level: 1_i32,
            cull_override: -1,
            ambient_override: -1,
            telemetry: Telemetry::new(),
            room_nodes: LiveVec::new(),
            room_bounds: Vec::new(),
            level_graph: LevelGraph::default(),
            current_room: None,
            player: None,
            bolt_pool: None,
            cloud_pool: None,
            caches: LiveVec::new(),
            player_drones: LiveVec::new(),
            boss_gates: LiveVec::new(),
            boss_portal: None,
            staged_boss: None,
            spec: None,
            blinking_lights: LiveVec::new(),
            blink_time: 0.0,
            saved_ambient: None,
        }
    }

    fn ready(&mut self) {
        // Capture knob from the command line (the GameManager pattern):
        // explicit args beat exported defaults.
        for arg in godot::classes::Os::singleton().get_cmdline_user_args().to_vec() {
            let arg = arg.to_string();
            if let Some(v) = arg.strip_prefix("--cull=") {
                if let Ok(c) = v.parse::<i32>() {
                    self.cull_override = c.clamp(0, 1);
                }
            } else if let Some(v) = arg.strip_prefix("--ambient=") {
                if let Ok(a) = v.parse::<i32>() {
                    self.ambient_override = a.clamp(0, 1);
                }
            }
        }
        self.connect_render_viewports();
        let target = self.to_gd();
        self.telemetry.register_monitors(
            Callable::from_object_method(&target, "step_ms_p50"),
            Callable::from_object_method(&target, "step_ms_p99"),
            Callable::from_object_method(&target, "step_ms_jitter"),
        );
    }

    fn exit_tree(&mut self) {
        self.telemetry.unregister_monitors();
    }

    fn physics_process(&mut self, _delta: f64) {
        // Physics is the engine's. The host only culls rooms by the
        // player's current location (cheap point-in-AABB; only re-toggles
        // visibility when the player changes rooms) — and times that work.
        let started = std::time::Instant::now();
        let player_pos = self.player.with(|p| arr(p.get_global_position()));
        if let Some(pos) = player_pos {
            self.update_room_culling(pos);
        }
        self.telemetry
            .record_step_ms(started.elapsed().as_secs_f32() * 1000.0);
        let tick = godot::classes::Engine::singleton().get_physics_frames();
        self.telemetry.report(tick);
    }

    fn process(&mut self, delta: f64) {
        self.telemetry.record_frame(delta as f32 * 1000.0);
        self.update_blinking_lights(delta as f32);
    }
}

#[godot_api]
impl LevelManager {
    /// Fired when the player's room resolves to a new room-list index —
    /// the same detection that drives culling. GameManager visits the room
    /// (RunState) and refreshes the recon map from it.
    #[signal]
    fn room_changed(room: i64);

    #[func]
    pub fn step_ms_p50(&self) -> f64 {
        self.telemetry.step_ms_p50() as f64
    }

    #[func]
    pub fn step_ms_p99(&self) -> f64 {
        self.telemetry.step_ms_p99() as f64
    }

    #[func]
    pub fn step_ms_jitter(&self) -> f64 {
        self.telemetry.step_ms_jitter() as f64
    }

    /// The viewport RIDs whose render time telemetry is measuring.
    /// Exposed for tests: in SBS this must be the two eye sub-viewports,
    /// not the root compositor.
    #[func]
    pub fn measured_viewport_rids(&self) -> Array<Rid> {
        self.telemetry.measured_viewports().into_iter().collect()
    }

    /// ViewManager republishes its active 3D viewports on every mode
    /// change; retarget render measurement onto them.
    #[func]
    fn on_render_viewports_changed(&mut self, viewports: Array<Rid>) {
        self.apply_measured_viewports(viewports);
    }

    /// The world grid's cell size — GameManager derives the map projection
    /// from it (one source; the exported field stays private).
    /// World-space center of the boss arena at flight height —
    /// `Vector3::ZERO` when this level has none. Teleport/escort anchor.
    #[func]
    pub fn boss_arena_center(&self) -> Vector3 {
        let Some(boss_idx) = self.level_graph.boss_room() else {
            return Vector3::ZERO;
        };
        let Some(room) = self.level_graph.room(boss_idx) else {
            return Vector3::ZERO;
        };
        let pitch = self.pitch();
        let origin = room.world_position(pitch.tile, pitch.story);
        let [ex, _ey, ez] = room.template.extents;
        Vector3::new(
            origin[0] + ex as f32 * pitch.tile * 0.5,
            origin[1] + 1.5,
            origin[2] + ez as f32 * pitch.tile * 0.5,
        )
    }

    /// Seal or open the room's gates as ONE set (no-op off sealed levels).
    /// GameManager drives this from physics callbacks, so each flip rides
    /// call_deferred (the house dormancy pattern — space state never mutates
    /// mid-step). A boss arena flips its one gate; a miniboss room flips all
    /// of its connectors together.
    #[func]
    pub fn seal_boss_gate(&mut self, sealed: bool) {
        for i in 0..self.boss_gates.len() {
            if let Some(mut gate) = self.boss_gates.get_live(i) {
                gate.call_deferred(methods::SET_SEALED, &[Variant::from(sealed)]);
            }
        }
    }

    /// Rise the staged boss (no-op off boss levels): dormant since the
    /// build, it becomes visible and tangible exactly when the fight
    /// engages — `activate_at` rides the same deferred dormancy flip as
    /// every reserved minion.
    #[func]
    pub fn rise_boss(&mut self) {
        if let Some(boss) = &self.staged_boss {
            boss.with(|b| {
                let pos = b.get_global_position();
                b.bind_mut().activate_at(pos);
            });
        }
    }

    /// Apply (or restore) the world-environment ambient for this build's
    /// paradigm. A fixed environment authors daylight ambient — the
    /// stand-in for the bounce its window light would produce under GI;
    /// every other build restores what main.tscn authored (the space
    /// levels' darkness is a look, not a default). No-op without a
    /// WorldEnvironment sibling (bare test stacks).
    fn apply_ambient_override(&mut self, spec: &LevelSpec) {
        use godot::classes::environment::AmbientSource;
        use godot::classes::WorldEnvironment;
        let Some(parent) = self.base().get_parent() else { return };
        let Some(we) = parent.try_get_node_as::<WorldEnvironment>("WorldEnvironment") else {
            return;
        };
        let Some(mut env) = we.get_environment() else { return };
        let ambient = if self.ambient_override == 1 {
            // Capture diagnostic (`--ambient=1`): flat white ambient makes
            // matte surfaces visible, and the background becomes SENTINEL
            // MAGENTA for two measured reasons (2026-07-14): an escaping
            // sightline renders the background color EXACTLY, and metallic
            // plates (vol03 ships metallic=1) mirror the environment — a
            // dark sky renders solid metal walls as false void, a bright
            // sentinel keeps them bright. Same knob family as `--cull`;
            // never a play-path look.
            env.set_background(godot::classes::environment::BgMode::COLOR);
            env.set_bg_color(Color::from_rgb(1.0, 0.0, 1.0));
            // Fog extinction ate the sentinel: at the authored density the
            // background reads near-BLACK past ~12 m, so every distant
            // escape wore the "unlit wall" color and the magenta detector
            // was blind (all 32 vantages, 2026-07-18). No fog in a
            // diagnostic that exists to make the background unmistakable.
            env.set_fog_enabled(false);
            Some(void_logic::roster::AmbientDef { color: [1.0, 1.0, 1.0], energy: 1.0 })
        } else if let void_logic::level_spec::Paradigm::Fixed(e) = &spec.paradigm {
            e.ambient
        } else {
            None
        };
        match ambient {
            Some(a) => {
                if self.saved_ambient.is_none() {
                    self.saved_ambient = Some((
                        env.get_ambient_source().ord(),
                        env.get_ambient_light_color(),
                        env.get_ambient_light_energy(),
                    ));
                }
                env.set_ambient_source(AmbientSource::COLOR);
                env.set_ambient_light_color(Color::from_rgb(a.color[0], a.color[1], a.color[2]));
                env.set_ambient_light_energy(a.energy);
            }
            None => {
                if let Some((source, color, energy)) = self.saved_ambient.take() {
                    env.set_ambient_source(AmbientSource::from_ord(source));
                    env.set_ambient_light_color(color);
                    env.set_ambient_light_energy(energy);
                }
            }
        }
    }

    /// Build the seal for one anchored room: a `BossGate` across EVERY active
    /// connector (a boss arena has one; a miniboss room has however many) plus
    /// the interior entry sensor — "entering the room" IS the engagement,
    /// however the player comes in. Both parent under the room container
    /// (freed and culled with it); the gates push into `boss_gates`, flipped
    /// as one set. One build path for the boss arena and the miniboss alike.
    fn build_room_seal(&mut self, graph: &LevelGraph, room_pos: usize) {
        let story = self.pitch().story;
        let cell = self.pitch().tile;
        let Some(mut room_node) = self.room_nodes.get_live(room_pos) else { return };
        let Some(room_idx) = graph.room_indices().nth(room_pos) else { return };
        let Some(room) = graph.room(room_idx) else { return };
        let origin = room.world_position(cell, story);
        // Fixed levels build the entry sensor but no gates (v1): zone
        // connectors are coarse graph doorways, not the house's actual
        // openings — a barrier there would float in a wall and seal
        // nothing real. The open house's staged fight engages on entry
        // and holds the portal dark until resolved; that is the seal.
        let fixed = matches!(
            self.spec.as_ref().map(|s| &s.paradigm),
            Some(void_logic::level_spec::Paradigm::Fixed(_))
        );
        let connectors =
            if fixed { Vec::new() } else { graph.active_connectors(room_idx) };
        for conn in connectors {
            let center = Vector3::new(
                origin[0] + (conn.offset[0] as f32 + 0.5) * cell,
                origin[1] + conn.offset[1] as f32 * story + story * 0.5,
                origin[2] + (conn.offset[2] as f32 + 0.5) * cell,
            );
            let dir = conn.facing.grid_offset();
            let outward = Vector3::new(dir[0] as f32, dir[1] as f32, dir[2] as f32);
            // The barrier sits in the doorway plane (half a cell out, where
            // room wall meets corridor), thin axis along the way through.
            let mut gate = BossGate::new_alloc();
            gate.bind_mut().set_span(cell, story);
            room_node.add_child(&gate);
            gate.set_global_position(center + outward * (0.5 * cell));
            if dir[0] != 0 {
                gate.set_rotation(Vector3::new(0.0, std::f32::consts::FRAC_PI_2, 0.0));
            }
            self.boss_gates.push(&gate, ());
        }
        // The sensor spans the whole room interior (inset ~1m from the walls).
        let [ex, ey, ez] = room.template.extents;
        let interior = Vector3::new(
            ex as f32 * cell - 2.0,
            ey as f32 * story - 1.0,
            ez as f32 * cell - 2.0,
        );
        let center = Vector3::new(
            origin[0] + ex as f32 * cell * 0.5,
            origin[1] + ey as f32 * story * 0.5,
            origin[2] + ez as f32 * cell * 0.5,
        );
        let mut trigger = BossTrigger::new_alloc();
        trigger.bind_mut().set_extents(interior);
        room_node.add_child(&trigger);
        trigger.set_global_position(center);
    }

    /// The staged boss node by ROLE, if this level staged a fight — so tests
    /// and UI can reference "the boss" without knowing which def it fights as.
    #[func]
    pub fn staged_boss_node(&self) -> Option<Gd<EnemyDrone>> {
        self.staged_boss.as_ref().and_then(|r| r.with(|b| b.clone()))
    }

    /// Light or darken the boss level's portal (no-op off boss levels).
    #[func]
    pub fn set_boss_portal_active(&mut self, active: bool) {
        if let Some(portal) = &self.boss_portal {
            portal.with(|p| {
                p.call_deferred(methods::SET_DORMANT, &[Variant::from(!active)]);
            });
        }
    }

    /// Enemy instance ids inside the radar's scope — deliberately local
    /// (`LevelGraph::radar_scope`: the room you stand in, or from a
    /// corridor the rooms it joins). GameManager pulls this on every room
    /// change and pushes it to the HUD; enemies activated mid-fight join
    /// on the next room change (they spawn in the player's own room, on
    /// screen anyway).
    #[func]
    pub fn radar_contacts(&self) -> PackedInt64Array {
        let Some(current) = self.current_room else { return PackedInt64Array::new() };
        let Some(current_node) = self.level_graph.room_indices().nth(current) else {
            return PackedInt64Array::new();
        };
        let in_scope: std::collections::HashSet<usize> = self
            .level_graph
            .radar_scope(current_node)
            .into_iter()
            .map(|n| n.index())
            .collect();
        let tree = self.base().get_tree();
        let mut ids = PackedInt64Array::new();
        for node in tree.get_nodes_in_group(groups::ENEMIES).iter_shared() {
            let Ok(enemy) = node.try_cast::<Node3D>() else { continue };
            let p = enemy.get_global_position();
            let Some(room) = level_assembly::room_at([p.x, p.y, p.z], &self.room_bounds) else {
                continue;
            };
            // Room-list index → graph node index, the same alignment
            // update_room_culling uses.
            let Some(room_node) = self.level_graph.room_indices().nth(room) else { continue };
            if in_scope.contains(&room_node.index()) {
                ids.push(enemy.instance_id().to_i64());
            }
        }
        ids
    }

    /// Does a live enemy share the player's room? Drives the Combat music
    /// bed (GameManager's conductor re-derives on every input change).
    /// "Live" = visible (dormant staging is invisible, `apply_dormancy`)
    /// and not already queued for freeing (a corpse mid-death-report must
    /// not hold the combat bed). Same position→room resolution as
    /// `radar_contacts`.
    pub fn live_enemies_in_current_room(&self) -> bool {
        let Some(current) = self.current_room else { return false };
        let tree = self.base().get_tree();
        for node in tree.get_nodes_in_group(groups::ENEMIES).iter_shared() {
            let Ok(enemy) = node.try_cast::<Node3D>() else { continue };
            if !enemy.is_visible() || enemy.is_queued_for_deletion() {
                continue;
            }
            let p = enemy.get_global_position();
            if level_assembly::room_at([p.x, p.y, p.z], &self.room_bounds) == Some(current) {
                return true;
            }
        }
        false
    }

    /// World-space center of room `i` (midpoint of its bounds). Drives room
    /// culling from a known interior point. Returns `ZERO` for an out-of-range
    /// index. NOTE: the Y is the vertical *midpoint*, not the floor — use
    /// [`room_floor_center`](Self::room_floor_center) to place a camera.
    #[func]
    pub fn room_center(&self, room: i64) -> Vector3 {
        match self.room_bounds.get(room as usize) {
            Some(b) => Vector3::new(
                (b.min[0] + b.max[0]) * 0.5,
                (b.min[1] + b.max[1]) * 0.5,
                (b.min[2] + b.max[2]) * 0.5,
            ),
            None => Vector3::ZERO,
        }
    }

    /// Horizontally-centered point at eye height above room `i`'s floor — where
    /// to park the camera for the loadout/briefing backdrop. Uses the floor
    /// (`min_y`), not the vertical midpoint, so the camera sits inside the room
    /// rather than up at the ceiling. `ZERO` for an out-of-range index.
    #[func]
    pub fn room_floor_center(&self, room: i64) -> Vector3 {
        match self.room_bounds.get(room as usize) {
            Some(b) => Vector3::new(
                (b.min[0] + b.max[0]) * 0.5,
                b.min[1] + 1.5,
                (b.min[2] + b.max[2]) * 0.5,
            ),
            None => Vector3::ZERO,
        }
    }

    /// Test seam: run room culling as if the player were at `point`.
    #[func]
    pub fn cull_for_position(&mut self, point: Vector3) {
        self.update_room_culling(arr(point));
    }

    /// Test seam: how many blinking light fixtures the host is tracking. Lets a
    /// test confirm there are handles to strand before exercising the per-frame
    /// flicker against freed lights.
    #[func]
    pub fn blinking_light_count(&self) -> i64 {
        self.blinking_lights.live_count() as i64
    }

    /// Build the quiet one-room backdrop shown behind the loadout/briefing
    /// screens. Structure-only: the same generator with populace (props, loot,
    /// enemies, and the exit portal) skipped, so no live enemy fires at the
    /// parked player and no exit gate floats in the loadout room.
    #[func]
    pub fn generate_backdrop(&mut self, seed: i64) {
        let mut spec = self.door_spec(seed);
        spec.room_budget = 1;
        self.build_from_spec(spec, seed, true);
    }

    /// Dev/test door (GUT structural suites, the editor): builds a fresh-
    /// profile spec for the property-set `current_level`, with the caller's
    /// explicit room budget. Production goes through GameManager's typed
    /// `build_from_spec` — BOTH converge on the one spec constructor.
    #[func]
    pub fn generate_level(&mut self, seed: i64, target_rooms: u32) {
        let mut spec = self.door_spec(seed);
        spec.room_budget = target_rooms as usize;
        self.build_from_spec(spec, seed, false);
    }

    fn door_spec(&self, seed: i64) -> LevelSpec {
        LevelSpec::for_level(
            void_logic::roster::roster(),
            Seed::from_i64(seed),
            self.current_level.max(1) as u32,
            &void_logic::unlocks::PermanentUnlocks::new(),
        )
    }
}

impl LevelManager {
    /// The single level-build pathway. `structure_only` builds just the room
    /// shells + lights — used for the loadout/briefing backdrop, where populace
    /// would mean a live enemy firing at the parked player and the exit gate
    /// floating in the menu. The full build additionally furnishes props and
    /// containers, spawns enemies, and places the exit portal. Both drive the
    /// same per-room steps, so the backdrop room is identical to the one the
    /// player would fly through.
    pub(crate) fn build_from_spec(&mut self, spec: LevelSpec, seed: i64, structure_only: bool) {
        let seed = Seed::from_i64(seed);
        self.current_level = spec.level as i32;
        self.spec = Some(spec.clone());

        // THE generation door (void-logic): sweeps + arena post-process for
        // generated paradigms, the authored zone lookup for fixed ones. A
        // staged fight whose arena failed to attach comes back with no
        // boss_room — the shell's business is only how loudly to complain.
        let graph = match generator::generate_for_spec(&spec, seed, structure_only) {
            Ok(g) => g,
            Err(e) => {
                godot_error!("Level generation failed: {e:?}");
                return;
            }
        };
        if !structure_only && spec.boss.is_some() && graph.boss_room().is_none() {
            godot_warn!("Boss arena failed to attach; level runs bossless");
        }

        // Assemble each room's content, grouped into the three build steps the
        // shell mirrors: structure, then non-enemy inhabitants (props +
        // containers), then enemies.
        let rooms = level_assembly::spawn_list_full(
            &graph,
            &spec,
            seed,
            &void_logic::roster::roster().catalog,
        );

        // The level manifest (Faucet Principle, tier-1 model): resolves each
        // enemy's type, expands its death-spawn minions, and binds one blue
        // cache per enemy — all seed-deterministic, all derived in void-logic. The
        // shell only places what the manifest enumerates; the enemy-type roll
        // that used to live inline here now lives in the manifest. Its rooms
        // align one-for-one with `rooms` (both from `spawn_list_full`).
        let level = self.current_level;
        let manifest =
            level_assembly::manifest(void_logic::roster::roster(), &graph, &spec, seed);

        // Drop any room nodes from a previous level before rebuilding.
        self.room_nodes.for_each_live(|_, node, _| node.queue_free());
        self.room_nodes.clear();
        self.room_bounds.clear();
        self.current_room = None;
        // Blinking lights are children of the freed room nodes.
        self.blinking_lights.clear();
        // Blue caches are direct children of the LevelManager (not under a
        // room), so they aren't freed with the room nodes — free them
        // explicitly. They are one-life-per-level (tier 1), rebuilt fresh below,
        // unlike the reused bolt ring. (Green caches were freed with their rooms.)
        self.caches.for_each_live(|_, node, _| node.queue_free());
        self.caches.clear();
        // The subdrone squad is one-life-per-level too.
        self.player_drones.for_each_live(|_, node, _| node.queue_free());
        self.player_drones.clear();
        // Boss fixtures (gate + trigger are LM children freed below with a
        // fresh handle set; the portal is freed with its room; the boss is
        // freed with its room like every enemy).
        self.boss_gates.clear();
        self.boss_portal = None;
        self.staged_boss = None;

        // The bolt ring survives the rebuild: build it once, then re-dormant it
        // so any bolt from the previous level is cleared. It is a sibling of the
        // room containers, so per-room culling never touches it.
        self.ensure_bolt_pool();
        // The dust-cloud ring rides the same lifecycle as the bolt ring: built
        // once, re-dormanted on rebuild, a sibling of the rooms.
        self.ensure_cloud_pool();

        // Pre-build the player's subdrone squad, dormant, as siblings of the
        // rooms (they escort the player across rooms, so per-room culling must
        // never hide them). The backdrop build skips them with the populace.
        if !structure_only {
            for _ in 0..subdrone::SQUAD_SIZE {
                let drone = PlayerDrone::new_alloc();
                self.base_mut().add_child(&drone);
                self.player_drones.push(&drone, ());
            }
        }

        let mut loader = ResourceLoader::singleton();
        let mut loose_rng = SmallRng::seed_from_u64(seed.value());
        let mut mesh_count = 0;
        let mut light_count = 0;
        let mut enemy_count = 0;

        // One container Node3D per room (identity transform; children keep world
        // positions). Hiding a container culls that room's geometry, lights, AND
        // inhabitants in a single visibility toggle.
        for (room_index, room) in rooms.iter().enumerate() {
            let mut room_node = Node3D::new_alloc();
            room_node.set_name(&format!("Room{}", self.room_nodes.len()));
            self.base_mut().add_child(&room_node);

            // --- Step 1: structure — the room's shell. Static pieces (here
            // and step 2's surface-mounted props) are collected and fused into
            // ONE merged collider per room instead of a body per tile (keeps
            // Jolt's broadphase and gen time sane).
            let mut static_meshes: Vec<Gd<Node3D>> = Vec::new();
            let mut solid_meshes: Vec<Gd<Node3D>> = Vec::new();
            for entry in &room.structure {
                if Self::place_mesh(&mut loader, &mut room_node, entry, &mut static_meshes, &mut solid_meshes, &mut loose_rng) {
                    mesh_count += 1;
                }
            }
            // Light fixtures. Most are dead in an abandoned base, so an Off
            // fixture gets no light node at all (the mesh stays, dark) — that
            // absence is the real GPU saving. Hidden with their room when culled.
            for ls in &room.lights {
                if ls.state == LightState::Off {
                    continue;
                }
                let energy = match ls.state {
                    LightState::Dim | LightState::Blinking => ls.energy * DIM_ENERGY_FACTOR,
                    LightState::On => ls.energy,
                    LightState::Off => ls.energy, // unreachable (Off skipped above)
                };
                let mut light = OmniLight3D::new_alloc();
                light.set_position(vec3(ls.position));
                light.set_param(godot::classes::light_3d::Param::RANGE, ls.range);
                light.set_param(godot::classes::light_3d::Param::ENERGY, energy);
                light.set_color(Color::from_rgb(ls.color[0], ls.color[1], ls.color[2]));
                room_node.add_child(&light);
                if ls.state == LightState::Blinking {
                    self.blinking_lights.push(&light, energy);
                }
                light_count += 1;
            }

            // Populace — props, loot containers, and enemies. Skipped entirely
            // for the structure-only backdrop so the loadout room stays quiet.
            if !structure_only {
                // --- Step 2: non-enemy inhabitants — furnished props (cell-rolled)
                // and organics containers (template loot spawns).
                for entry in &room.props {
                    if Self::place_mesh(&mut loader, &mut room_node, entry, &mut static_meshes, &mut solid_meshes, &mut loose_rng) {
                        mesh_count += 1;
                    }
                }
                for pos in &room.containers {
                    Self::spawn_organic_cache(&mut loader, &mut room_node, *pos);
                }

                // --- Step 3: enemies — each a self-driving RigidBody3D parented
                // under the room so it culls/hides with it. Type, death-spawn
                // minions, and the bound cache all come from the manifest: the
                // parent is placed live, its minions pre-built dormant under the
                // SAME room, its cache pre-built dormant under the level. On
                // the parent's death it activates them — no death-path or
                // per-drop instantiate remains.
                let room_anchor = manifest.rooms[room_index].anchor;
                for (enemy_index, spawn) in
                    manifest.rooms[room_index].enemies.iter().enumerate()
                {
                    // The room's dormant SEAL ANCHOR — a staged boss OR a
                    // miniboss, the manifest's call — spawns DORMANT and rises
                    // on room entry, not at build, so it can't be sniped from a
                    // corridor before the fight begins (playtest 2026-07-06).
                    // Every other enemy in the room spawns live.
                    let is_anchor = room_anchor == Some(enemy_index);
                    // The staged-boss anchor (vs a miniboss) additionally
                    // stages its counted drop set below — a miniboss has no
                    // reward ritual, so its bound cache stays ordinary.
                    let is_boss = is_anchor && spec.boss.is_some();
                    let Some(mut parent) = Self::spawn_enemy(
                        &mut loader, &mut room_node, spawn.enemy_type, level, spawn.position, is_anchor,
                    ) else { continue };
                    enemy_count += 1;
                    if is_anchor {
                        self.staged_boss = Some(LiveRef::new(&parent));
                    }

                    for minion in &spawn.minions {
                        if let Some(minion_node) = Self::spawn_enemy(
                            &mut loader, &mut room_node, minion.enemy_type, level, spawn.position, true,
                        ) {
                            parent.bind_mut().bind_minion(&minion_node, minion.trigger);
                        }
                    }
                    let mut level_mgr: Gd<Node3D> = self.base().clone().cast();
                    if let Some(mut cache_node) = Self::build_cache(&mut loader, &mut level_mgr) {
                        parent.bind_mut().bind_cache(&cache_node);
                        // A boss's bound cache is part of its staged drop
                        // set — the stamp crosses with cache_collected so
                        // the fight FSM counts exactly these.
                        cache_node.bind_mut().set_boss_loot(is_boss);
                        // Track it so a rebuild frees it (tier-1, one life/level).
                        self.caches.push(&cache_node, ());
                    }

                    // A boss's drop is special-cased at BUILD time (Faucet:
                    // everything it can shed exists before the fight):
                    // the red hull container when GameManager staged one,
                    // otherwise the consolation pile as extra bound caches.
                    if is_boss {
                        if let Some(staging) = &spec.boss {
                            if staging.hull_reward.is_some() {
                                parent.bind_mut().set_cache_kind(CurrencyKind::HullReward);
                            } else {
                                for (kind, amount) in staging.pile.iter().copied() {
                                    if let Some(mut bonus) = Self::build_cache(&mut loader, &mut level_mgr) {
                                        parent.bind_mut().bind_bonus_cache(&bonus, kind, amount);
                                        bonus.bind_mut().set_boss_loot(true);
                                        self.caches.push(&bonus, ());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // The room's PHYSICS: ONE static body carrying the watertight
            // shell (a solid box per sealed cell face, doorways open —
            // derived and unit-tested in void-logic) plus a snug convex
            // hull per protruding corner piece. Built after populace so a
            // late ConvexSolid placement can never be dropped. The skin
            // meshes are the room's LOOK; this body is the wall the
            // engine knows.
            if !room.shell.is_empty() || !solid_meshes.is_empty() {
                let mut shell_body = StaticBody3D::new_alloc();
                shell_body.set_name("RoomShell");
                for slab in &room.shell {
                    let mut boxy = BoxShape3D::new_gd();
                    boxy.set_size(vec3(slab.size));
                    let mut col = CollisionShape3D::new_alloc();
                    col.set_shape(&boxy);
                    col.set_position(vec3(slab.center));
                    shell_body.add_child(&col);
                }
                let mut shell_node = shell_body.clone().upcast::<Node>();
                for solid in &solid_meshes {
                    godot_util::add_convex_collision(&mut shell_node, solid, solid.get_transform());
                }
                room_node.add_child(&shell_body);
            }
            // Fuse the room's statics — surface-mounted props only now
            // (structure is Skin/ConvexSolid) — after every Static
            // placement has been collected.
            Self::build_merged_collision(&mut room_node, &static_meshes);

            self.room_bounds.push(room.bounds.clone());
            self.room_nodes.push(&room_node, ());
        }

        // End-of-level portal: parent it under the room that contains it so it
        // culls and hides with that room like every other inhabitant. Skipped
        // for the structure-only backdrop (no exit gate behind the loadout).
        // On a boss level it is pre-built DORMANT (Faucet: built with the
        // level, flipped live when the fight's reward is collected).
        // The ambient override: fixed environments author the bounce
        // stand-in; every other build restores the scene's own ambient.
        self.apply_ambient_override(&spec);

        // Authored daylight (fixed environments), parented under room 0 so
        // it frees with the level. ALL of an interior's light comes from
        // outside (owner 2026-07-12): a shadowed sun for the hard stripes,
        // and each authored window panel as a wide shadowless spot shining
        // inward — the real-time stand-in for the sheers-as-area-lights
        // look (the lightmap bake later makes them true emissive panels).
        if let void_logic::level_spec::Paradigm::Fixed(env) = &spec.paradigm {
            if let Some(mut room0) = self.room_nodes.get_live(0) {
                let scale = spec.pitch.tile;
                if let Some(sun) = env.sun {
                    let az = sun.azimuth_deg.to_radians();
                    let el = sun.elevation_deg.to_radians();
                    // Bearing 0 = model north (-Z), 90 = east (+X); the sun
                    // SITS at that bearing and shines back into the scene.
                    let toward_sun = Vector3::new(
                        az.sin() * el.cos(),
                        el.sin(),
                        -az.cos() * el.cos(),
                    );
                    let mut sun_light = godot::classes::DirectionalLight3D::new_alloc();
                    sun_light.set_param(godot::classes::light_3d::Param::ENERGY, sun.energy);
                    sun_light.set_color(Color::from_rgb(1.0, 0.96, 0.88));
                    sun_light.set_shadow(true);
                    room0.add_child(&sun_light);
                    sun_light.set_global_position(toward_sun * 50.0);
                    sun_light.look_at(Vector3::ZERO);
                }
                for window in &env.windows {
                    let mut spot = godot::classes::SpotLight3D::new_alloc();
                    spot.set_param(godot::classes::light_3d::Param::ENERGY, window.energy);
                    spot.set_param(
                        godot::classes::light_3d::Param::RANGE,
                        window.range * scale,
                    );
                    // Wide cone: a panel's wash, not a flashlight beam. The
                    // spot sits half a panel-height OUTSIDE the opening so
                    // the cone covers the whole window by the time it
                    // enters — daylight, already diffused by the sheers.
                    spot.set_param(godot::classes::light_3d::Param::SPOT_ANGLE, 70.0);
                    // Flat falloff: a daylight panel washes its whole
                    // throw, it doesn't pool at the glass (attenuation 0
                    // holds near-constant brightness through the range —
                    // the documented area-wash setting).
                    spot.set_param(godot::classes::light_3d::Param::ATTENUATION, 0.2);
                    spot.set_color(Color::from_rgb(0.98, 0.99, 1.0));
                    let inward = vec3(window.inward);
                    let center = vec3(window.center) * scale;
                    let back_off = window.size[1] * 0.5 * scale;
                    room0.add_child(&spot);
                    spot.set_global_position(center - inward * back_off);
                    spot.look_at(center + inward * (window.range * scale));

                    // The panel itself GLOWS: an emissive quad on the
                    // opening's plane — the visible light source the spot
                    // stands in for (and, later, the LightmapGI emitter).
                    let mut quad_mesh = godot::classes::QuadMesh::new_gd();
                    quad_mesh.set_size(Vector2::new(
                        window.size[0] * scale,
                        window.size[1] * scale,
                    ));
                    let mut mat = godot::classes::StandardMaterial3D::new_gd();
                    mat.set_feature(
                        godot::classes::base_material_3d::Feature::EMISSION,
                        true,
                    );
                    mat.set_emission(Color::from_rgb(1.0, 0.98, 0.94));
                    mat.set_emission_energy_multiplier(4.0);
                    mat.set_albedo(Color::from_rgb(1.0, 1.0, 1.0));
                    mat.set_shading_mode(
                        godot::classes::base_material_3d::ShadingMode::UNSHADED,
                    );
                    quad_mesh.set_material(&mat);
                    let mut panel = godot::classes::MeshInstance3D::new_alloc();
                    panel.set_mesh(&quad_mesh);
                    panel.set_name(&format!("WindowPanel{}", room0.get_child_count()));
                    room0.add_child(&panel);
                    // QuadMesh faces its local +Z: sit ON the plane, face
                    // the inward direction (visible from inside the room).
                    panel.set_global_position(center);
                    panel.look_at(center + inward);
                }
            }
        }

        let boss_arena = if structure_only { None } else { graph.boss_room() };
        if !structure_only {
            if let Some(portal_pos) = portal_sys::portal_position(&graph, self.pitch()) {
                if let Some(portal_scene) = loader.load(scenes::PORTAL) {
                    let packed: Gd<PackedScene> = portal_scene.cast();
                    if let Some(instance) = packed.instantiate() {
                        let mut node: Gd<Node3D> = instance.cast();
                        node.set_position(vec3(portal_pos));
                        let room_idx = self.room_bounds.iter().position(|b| b.contains(portal_pos));
                        match room_idx.and_then(|i| self.room_nodes.get_live(i)) {
                            Some(mut room_node) => room_node.add_child(&node),
                            None => self.base_mut().add_child(&node),
                        }
                        if boss_arena.is_some() {
                            if let Ok(mut portal) = node.clone().try_cast::<Portal>() {
                                portal.bind_mut().set_dormant(true);
                                self.boss_portal = Some(LiveRef::new(&portal));
                            }
                        }
                        godot_print!("Portal spawned at ({}, {}, {})", portal_pos[0], portal_pos[1], portal_pos[2]);
                    }
                } else {
                    godot_warn!("Could not load portal.tscn");
                }
            }
        }

        // Room seals: every room the manifest marks with a dormant anchor — a
        // boss arena and/or a miniboss room — seals its active connectors on
        // entry, with an interior entry sensor. ONE build path: a boss arena
        // has a single doorway, a miniboss room seals however many it has.
        for room_pos in 0..manifest.rooms.len() {
            if manifest.rooms[room_pos].anchor.is_some() {
                self.build_room_seal(&graph, room_pos);
            }
        }

        // Place the player in the first room's center. The ship is a
        // RigidBody3D and drives its own motion; we only position it.
        let (spawn, spawn_yaw) = level_assembly::spawn_pose(&graph, self.pitch());
        if let Some(parent) = self.base().get_parent() {
            if let Some(player) = parent.try_get_node_as::<ShipController>(nodes::PLAYER) {
                let mut player_node = player.clone();
                player_node.set_position(vec3(spawn));
                // Centered in the start room, facing its doorway — never a
                // corner (the pose is the model's, tested in level_assembly).
                player_node.set_rotation(Vector3::new(0.0, spawn_yaw, 0.0));
                player_node.set_linear_velocity(Vector3::ZERO);
                player_node.set_angular_velocity(Vector3::ZERO);
                player_node.reset_physics_interpolation();
                self.player = Some(LiveRef::new(&player_node.upcast::<Node3D>()));
                godot_print!("Player spawned at ({}, {}, {})", spawn[0], spawn[1], spawn[2]);
            }
        }

        // Retain the graph for the level's lifetime: it's the authority for
        // cull visibility, and later for mapping and route queries. Moved in
        // last, after every generation step that borrowed it.
        self.level_graph = graph;

        godot_print!(
            "Level generated: {} rooms, {} meshes, {} lights, {} enemies{}",
            spec.room_budget,
            mesh_count,
            light_count,
            enemy_count,
            if structure_only { " (structure-only)" } else { "" },
        );
    }

    /// Free the collision bodies the glTF importer baked from `_col`/
    /// `_convcolonly` node suffixes, leaving a pure visual subtree. Our build
    /// is the single source of collision; freeing the body also frees its
    /// collision-shape children.
    fn strip_baked_colliders(node: &Gd<Node3D>) {
        let mut bodies: Vec<Gd<Node>> = Vec::new();
        Self::collect_static_bodies(&node.clone().upcast::<Node>(), &mut bodies);
        for body in bodies {
            body.free();
        }
    }

    /// Build the bolt pool on first use and re-dormant it on every rebuild. The
    /// pool is a direct child of this LevelManager (a sibling of the room
    /// containers, not inside one), so bolts are not toggled off by per-room
    /// visibility culling and the pool outlives the room nodes cleared above.
    fn ensure_bolt_pool(&mut self) {
        // A pool queue_freed THIS frame (GameManager clears LevelManager's
        // children before every rebuild; the free lands at frame end) is
        // still briefly alive — it must count as gone, or the fresh level
        // runs with no ammunition ring and nothing in it can fire. That was
        // the "enemies stopped shooting" playtest bug: every build reached
        // through the backdrop flow skipped the rebuild against a dying pool.
        let usable = self
            .bolt_pool
            .with(|p| {
                if p.is_queued_for_deletion() {
                    None
                } else {
                    p.bind_mut().reset();
                    Some(()) // already built and now re-dormanted
                }
            })
            .flatten();
        if usable.is_none() {
            let pool = BoltPool::new_alloc();
            self.base_mut().add_child(&pool);
            self.bolt_pool = Some(LiveRef::new(&pool));
        }
    }

    /// Build the dust-cloud pool on first use and re-dormant it on every
    /// rebuild. Mirrors [`ensure_bolt_pool`](Self::ensure_bolt_pool) exactly:
    /// the pool is a direct child of this LevelManager (a sibling of the room
    /// containers), so clouds escape per-room culling and the pool outlives the
    /// room nodes cleared on rebuild. A pool queue_freed this frame counts as
    /// gone, so a fresh level always has a usable ring.
    fn ensure_cloud_pool(&mut self) {
        let usable = self
            .cloud_pool
            .with(|p| {
                if p.is_queued_for_deletion() {
                    None
                } else {
                    p.bind_mut().reset();
                    Some(()) // already built and now re-dormanted
                }
            })
            .flatten();
        if usable.is_none() {
            let pool = CloudPool::new_alloc();
            self.base_mut().add_child(&pool);
            self.cloud_pool = Some(LiveRef::new(&pool));
        }
    }

    fn collect_static_bodies(node: &Gd<Node>, out: &mut Vec<Gd<Node>>) {
        for child in node.get_children().iter_shared() {
            if child.clone().try_cast::<StaticBody3D>().is_ok() {
                out.push(child); // freeing it also frees its shape children
            } else {
                Self::collect_static_bodies(&child, out);
            }
        }
    }

    /// Fuse the remaining `Static` meshes' triangles (surface-mounted props
    /// and equipment — never the room skin, whose physics is the watertight
    /// shell, nor the corner curves, which carry solid convex hulls) into
    /// one `ConcavePolygonShape3D` on a single `StaticBody3D` per room.
    /// Jolt tracks one body instead of thousands. One source: the meshes.
    fn build_merged_collision(room_node: &mut Gd<Node3D>, static_meshes: &[Gd<Node3D>]) {
        let mut faces = PackedVector3Array::new();
        for node in static_meshes {
            Self::collect_faces(node, node.get_transform(), &mut faces);
        }
        if faces.is_empty() {
            return;
        }
        let mut shape = ConcavePolygonShape3D::new_gd();
        shape.set_faces(&faces);
        let mut col = CollisionShape3D::new_alloc();
        col.set_shape(&shape);
        let mut body = StaticBody3D::new_alloc();
        // Named for diagnostics: an anonymous @StaticBody3D@N in a contact
        // report reads as an ad-hoc body; this is the container's ONE fused
        // shell (the only static collision constructor in the build).
        body.set_name("FusedShell");
        body.add_child(&col);
        room_node.add_child(&body);
    }

    /// Append every triangle of every `MeshInstance3D` under `node` to `faces`,
    /// transformed into the room's space by the accumulated `xform`.
    fn collect_faces(node: &Gd<Node3D>, xform: Transform3D, faces: &mut PackedVector3Array) {
        if let Ok(mesh_inst) = node.clone().try_cast::<MeshInstance3D>() {
            if let Some(mesh) = mesh_inst.get_mesh() {
                for v in mesh.get_faces().as_slice() {
                    faces.push(xform * *v);
                }
            }
        }
        for child in node.get_children().iter_shared() {
            if let Ok(child3d) = child.try_cast::<Node3D>() {
                let child_xform = xform * child3d.get_transform();
                Self::collect_faces(&child3d, child_xform, faces);
            }
        }
    }

    /// Give a dynamic body a convex collider per `MeshInstance3D` under
    /// `node`, each placed at the mesh's transform relative to the body so the
    /// hull hugs the rendered geometry. Convex (not trimesh) because Jolt
    /// allows concave shapes only on static bodies — one source: the mesh.
    /// Subscribe to ViewManager's active-viewport publication (so
    /// render measurement follows mode changes) and seed the current
    /// set immediately — ViewManager is the sole authority on which
    /// viewports draw the 3D world, so we never recompute that here.
    fn connect_render_viewports(&mut self) {
        let Some(main) = self.base().get_parent() else {
            godot_print!("LevelManager: no parent; render telemetry idle");
            return;
        };
        let Some(mut view_mgr) = main.try_get_node_as::<ViewManager>(nodes::VIEW_MANAGER) else {
            godot_print!("LevelManager: no ViewManager sibling; render telemetry idle");
            return;
        };
        let callable = self.base().callable(methods::ON_RENDER_VIEWPORTS_CHANGED);
        if !view_mgr.is_connected(signals::RENDER_VIEWPORTS_CHANGED, &callable) {
            view_mgr.connect(signals::RENDER_VIEWPORTS_CHANGED, &callable);
        }
        let rids = view_mgr.bind().active_viewport_rids();
        self.apply_measured_viewports(rids);
    }

    fn apply_measured_viewports(&mut self, viewports: Array<Rid>) {
        let rids: Vec<Rid> = viewports.iter_shared().collect();
        self.telemetry.measure_viewports(&rids);
    }

    /// The retained level graph — culling, the recon map, and future
    /// pathfinding all read this one authority (crate-internal, no Variant
    /// crossing).
    pub fn graph(&self) -> &LevelGraph {
        &self.level_graph
    }

    /// This level's world-space quantization — the ONE pitch source
    /// (`planet::Pitch::for_level`); every conversion in this node and every
    /// void-logic call goes through it.
    fn pitch(&self) -> void_logic::planet::Pitch {
        self.spec
            .as_ref()
            .map(|s| s.pitch)
            .unwrap_or_else(|| void_logic::planet::Pitch::for_level(1))
    }

    /// Show only the player's current room and its portal-neighbors;
    /// hide the rest. Recomputes only when the player changes rooms.
    fn update_room_culling(&mut self, player_pos: [f32; 3]) {
        if self.cull_override == 0 {
            // Capture override: the whole level stays visible (exterior
            // vantages have no "current room" to cull from).
            self.room_nodes.for_each_live(|_, node, _| node.set_visible(true));
            return;
        }
        let Some(current) = level_assembly::room_at(player_pos, &self.room_bounds) else {
            return;
        };
        if self.current_room == Some(current) {
            return;
        }
        self.current_room = Some(current);
        // Culling and the recon map share this one room-change detection. The
        // handler (GameManager) reads back through our Gd binding for the
        // graph, which must not re-enter this &mut method — its subscription
        // is CONNECT_DEFERRED (see GameManager::wire_level_signals), so the
        // emit itself stays direct and typed.
        self.base_mut().emit_signal(signals::ROOM_CHANGED, &[Variant::from(current as i64)]);
        // The current room-list index maps to the graph node at that
        // placement position; visibility is the graph's to decide.
        let Some(current_node) = self.level_graph.room_indices().nth(current) else {
            return;
        };
        // Fixed levels keep every container visible: the whole house is ONE
        // scene under room 0, so hiding containers would blank the level,
        // not cull it (v1 relies on Godot's per-object frustum culling; the
        // zone graph above still drives room_changed and the recon map).
        if matches!(
            self.spec.as_ref().map(|s| &s.paradigm),
            Some(void_logic::level_spec::Paradigm::Fixed(_))
        ) {
            return;
        }
        let visible: std::collections::HashSet<usize> = self
            .level_graph
            .visible_from(current_node, RENDER_ROOM_DEPTH)
            .into_iter()
            .map(|n| n.index())
            .collect();
        // `for_each_live` skips any room freed out from under us and keeps the
        // index aligned with room_bounds — a freed handle is structurally
        // unreachable, so this can't use-after-free.
        self.room_nodes.for_each_live(|i, node, _| {
            node.set_visible(visible.contains(&i));
        });
    }

    /// Flicker blinking fixtures: each toggles between its full energy
    /// and dark on an out-of-phase cycle, so the base reads as alive
    /// rather than uniformly lit.
    fn update_blinking_lights(&mut self, delta: f32) {
        self.blink_time += delta;
        let t = self.blink_time;
        // `for_each_live` resolves each light by id and skips freed ones, so a
        // fixture freed out from under us (a regen, or GameManager clearing our
        // children) is a no-op rather than the use-after-free that panicked here
        // every frame.
        self.blinking_lights.for_each_live(|i, light, base| {
            let phase = i as f32 * 0.7;
            let lit = (t * 1.7 + phase).sin() > -0.55;
            light.set_param(godot::classes::light_3d::Param::ENERGY, if lit { *base } else { 0.0 });
        });
    }

    /// Instantiate an enemy scene under `parent`, stamped with its def's
    /// crossing id + level, positioned. It self-drives as a RigidBody3D; the
    /// engine owns its motion. Returns the live handle so the caller can bind
    /// its minions and blue cache (Faucet Principle, tier 1). `dormant`
    /// builds it invisible, non-processing, non-colliding — that's how a
    /// reserved minion enters the tree, waiting for its trigger.
    fn spawn_enemy(
        loader: &mut Gd<ResourceLoader>,
        parent: &mut Gd<Node3D>,
        enemy_key: void_logic::roster::EnemyKey,
        level: i32,
        pos: [f32; 3],
        dormant: bool,
    ) -> Option<Gd<EnemyDrone>> {
        // Every enemy is the same node + collider scene; the def (via the key
        // stamped below) drives stats, model, and collider size.
        let scene_res = loader.load("res://scenes/enemies/enemy.tscn")?;
        let packed: Gd<PackedScene> = scene_res.cast();
        let instance = packed.instantiate()?;
        let mut enemy = instance.try_cast::<EnemyDrone>().ok()?;
        // Stamp key + level before entering the tree so ready() configures it.
        {
            let mut g = enemy.bind_mut();
            g.set_spawn_key(GString::from(enemy_key.as_str()));
            g.set_spawn_level(level);
        }
        enemy.set_position(vec3(pos));
        // Parented under the room container (a cell inhabitant), so hiding or
        // culling the room takes its enemies with it. Reserved minions parent
        // under the SAME room as their parent-to-be, so they share its culling
        // and the one recursive signal wire.
        parent.add_child(&enemy);
        // ready() has now run (add_child is synchronous): a reserved minion is
        // fully built (model, hull, health bar) but must sit dormant until its
        // trigger. An active enemy resets interpolation for its placement.
        if dormant {
            enemy.bind_mut().deactivate_dormant();
        } else {
            enemy.reset_physics_interpolation();
        }
        Some(enemy)
    }

    /// Pre-build the one blue cache reserved for an enemy (Faucet Principle,
    /// tier 1), dormant, parented under the LevelManager rather than a room
    /// container so a dropped cache persists and stays visible when the player
    /// leaves the room it dropped in (drops are collected across rooms,
    /// matching the old scene-root drop). Returns the handle for the enemy to
    /// bind and drop with its reward.
    fn build_cache(loader: &mut Gd<ResourceLoader>, level_mgr: &mut Gd<Node3D>) -> Option<Gd<CurrencyCache>> {
        let scene_res = loader.load(scenes::CURRENCY_CACHE)?;
        let packed: Gd<PackedScene> = scene_res.cast();
        let instance = packed.instantiate()?;
        let cache = instance.try_cast::<CurrencyCache>().ok()?;
        // ready() dormants it; parent under the level (sibling of rooms + the
        // bolt pool), so per-room culling never hides a dropped cache.
        level_mgr.add_child(&cache);
        Some(cache)
    }

    /// Place a green (organics) cache at `pos` under the given room container
    /// (a cell inhabitant), live from the start — the same `drop_at` activation
    /// path the blue kill-drops use, stamped with the green payload. Wired to
    /// GameManager by the recursive build-time signal pass.
    fn spawn_organic_cache(loader: &mut Gd<ResourceLoader>, parent: &mut Gd<Node3D>, pos: [f32; 3]) -> bool {
        let Some(scene_res) = loader.load(scenes::CURRENCY_CACHE) else {
            return false;
        };
        let packed: Gd<PackedScene> = scene_res.cast();
        let Some(instance) = packed.instantiate() else {
            return false;
        };
        let Ok(mut cache) = instance.try_cast::<CurrencyCache>() else {
            return false;
        };
        // ready() (run by add_child) queues the dormant flip; drop_at queues the
        // live flip after it, so the cache ends the frame active at its spawn.
        parent.add_child(&cache);
        cache.bind_mut().drop_at(vec3(pos), CurrencyKind::Organics, ORGANIC_CACHE_AMOUNT);
        true
    }

    /// Instantiate one placed mesh under `room_node`, applying its collision
    /// intent: Static renders and is collected into `statics` for the room's
    /// merged collider; Dynamic becomes a tumbling RigidBody3D with a convex
    /// hull; Passable just renders. Shared by the structure and inhabitant
    /// (prop) build steps so both honor the same collision contract.
    fn place_mesh(
        loader: &mut Gd<ResourceLoader>,
        room_node: &mut Gd<Node3D>,
        entry: &MeshPlacement,
        statics: &mut Vec<Gd<Node3D>>,
        solids: &mut Vec<Gd<Node3D>>,
        loose_rng: &mut SmallRng,
    ) -> bool {
        // Resolve at the shell's one id→path crossing (godot_util::scene_path).
        let scene_path = super::godot_util::scene_path(entry.scene);
        let Some(resource) = loader.load(scene_path) else {
            godot_warn!("Could not load: {}", scene_path);
            return false;
        };
        let packed: Gd<PackedScene> = resource.cast();
        let Some(instance) = packed.instantiate() else {
            godot_warn!("Could not instantiate: {}", scene_path);
            return false;
        };
        let mut node: Gd<Node3D> = instance.cast();
        // The asset pack bakes its own collision via `_convcolonly` node
        // suffixes; strip it so our build owns collision as the single source.
        Self::strip_baked_colliders(&node);
        // The placement's declared scale (the fixed environment rides its
        // kit's scale here) — set before any collider is derived, so physics
        // and render agree.
        if (entry.scale - 1.0).abs() > 1e-6 {
            node.set_scale(Vector3::ONE * entry.scale);
        }

        match entry.collision {
            Collision::Dynamic => {
                node.set_position(Vector3::ZERO);
                use rand::RngExt;
                let rx: f32 = loose_rng.random_range(0.0..std::f32::consts::TAU);
                let ry: f32 = loose_rng.random_range(0.0..std::f32::consts::TAU);
                let rz: f32 = loose_rng.random_range(0.0..std::f32::consts::TAU);

                let mut body = RigidBody3D::new_alloc();
                body.set_position(vec3(entry.position));
                body.set_rotation(Vector3::new(rx, ry, rz));
                body.set_gravity_scale(0.0);

                body.add_child(&node);
                let node_xform = node.get_transform();
                let mut body_node = body.clone().upcast::<Node>();
                godot_util::add_convex_collision(&mut body_node, &node, node_xform);
                room_node.add_child(&body);
                body.reset_physics_interpolation();
            }
            Collision::Static => {
                node.set_position(vec3(entry.position));
                if entry.rotation_x.abs() > 0.001 || entry.rotation_y.abs() > 0.001 {
                    node.set_rotation(Vector3::new(entry.rotation_x, entry.rotation_y, 0.0));
                }
                room_node.add_child(&node);
                statics.push(node);
            }
            Collision::Skin | Collision::Passable => {
                // Skin: render-only by design — the room's watertight shell
                // owns that plane's physics (see build's shell emission).
                node.set_position(vec3(entry.position));
                if entry.rotation_x.abs() > 0.001 || entry.rotation_y.abs() > 0.001 {
                    node.set_rotation(Vector3::new(entry.rotation_x, entry.rotation_y, 0.0));
                }
                room_node.add_child(&node);
            }
            Collision::ConvexSolid => {
                // A protruding solid (curved corner stack): rendered here,
                // hulled onto the room's ONE shell body by the caller — a
                // per-piece body would re-explode Jolt's broadphase, the
                // exact regression the merged-collision pin guards.
                node.set_position(vec3(entry.position));
                if entry.rotation_x.abs() > 0.001 || entry.rotation_y.abs() > 0.001 {
                    node.set_rotation(Vector3::new(entry.rotation_x, entry.rotation_y, 0.0));
                }
                room_node.add_child(&node);
                solids.push(node);
            }
        }
        true
    }
}
