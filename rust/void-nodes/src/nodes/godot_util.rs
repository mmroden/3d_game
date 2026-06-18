//! Shared Godot-dependent utilities for node code.
//! Basis helpers, beam rendering, and particle material builders.

use godot::prelude::*;
use godot::classes::{
    MeshInstance3D, BoxMesh, StandardMaterial3D, OmniLight3D,
    RigidBody3D, CollisionShape3D, PackedScene, ResourceLoader,
    ParticleProcessMaterial, BaseMaterial3D, Texture2D,
    particle_process_material::Parameter,
    base_material_3d,
    light_3d,
};

use super::audio_manager::AudioManager;
use super::constants::{meta_keys, nodes};
use super::live_handle::{LiveOpt, LiveRef, LiveVec};

/// Yaw applied to the imported ship model so its nose points along the ship's
/// forward (-Z); the asset is modelled facing backward. One source of truth for
/// both the player ship and the menu showcase.
pub const SHIP_MODEL_YAW: f32 = std::f32::consts::PI;

/// Get the scene tree root node from a SceneTree (or Optional SceneTree).
/// Returns `None` during early initialization or after the tree is torn down.
/// Usage: `scene_root(self.base().get_tree())`
pub fn scene_root(tree: impl Into<Option<Gd<godot::classes::SceneTree>>>) -> Option<Gd<godot::classes::Node>> {
    tree.into().and_then(|t| t.get_root()).map(|r| r.upcast())
}

/// World point `distance` metres straight ahead of the player camera. `main` is
/// the scene's Main node (the parent of the showcase/turntable and the Player).
/// Used to keep the showcase ship and the bestiary turntable in view — they call
/// this every frame so they track wherever the player camera lands, rather than
/// being placed once against a camera that then teleports out from under them.
pub fn camera_front_position(main: &Gd<godot::classes::Node>, distance: f32) -> Option<Vector3> {
    let camera = main.try_get_node_as::<Node3D>(nodes::PLAYER_CAMERA)?;
    let t = camera.get_global_transform();
    let forward = -t.basis.col_c();
    Some(t.origin + forward * distance)
}

/// Find the AudioManager node by navigating up to the scene root, then down to Main/AudioManager.
/// Works from any depth in the tree (enemies under LevelManager, portal, lootbox, etc.).
/// Accepts the result of `self.base().get_tree()` to avoid upcast ambiguity.
pub fn find_audio_manager(tree: impl Into<Option<Gd<godot::classes::SceneTree>>>) -> Option<Gd<AudioManager>> {
    let root = scene_root(tree)?;
    // Main is the first child of root; AudioManager is a direct child of Main
    let main = root.try_get_node_as::<godot::classes::Node>("Main")?;
    main.try_get_node_as::<AudioManager>(nodes::AUDIO_MANAGER)
}

/// Compute an orientation basis pointing along `forward`.
/// Falls back to `Vector3::RIGHT` as up-reference when forward is near-parallel to UP.
pub fn basis_from_direction(forward: Vector3) -> Basis {
    let dir = forward.normalized();
    let up = if dir.cross(Vector3::UP).length() > 0.001 {
        Vector3::UP
    } else {
        Vector3::RIGHT
    };
    let z_axis = -dir;
    let x_axis = up.cross(z_axis).normalized();
    let y_axis = z_axis.cross(x_axis);
    Basis::from_cols(x_axis, y_axis, z_axis)
}

/// Spawn a thin laser beam mesh between two points.
/// Returns the `MeshInstance3D` node (caller must add it to the scene tree).
pub fn create_beam_mesh(from: Vector3, to: Vector3, color: &[f32]) -> Option<Gd<MeshInstance3D>> {
    let midpoint = (from + to) * 0.5;
    let length = from.distance_to(to);
    if length < 0.01 {
        return None;
    }

    let mut mesh_instance = MeshInstance3D::new_alloc();

    let mut box_mesh = BoxMesh::new_gd();
    box_mesh.set_size(Vector3::new(0.02, 0.02, length));
    mesh_instance.set_mesh(&box_mesh);

    let mut material = StandardMaterial3D::new_gd();
    material.set_albedo(Color::from_rgba(color[0], color[1], color[2], 1.0));
    material.set_feature(godot::classes::base_material_3d::Feature::EMISSION, true);
    material.set_emission(Color::from_rgba(color[0], color[1], color[2], 1.0));
    material.set_emission_energy_multiplier(8.0);
    mesh_instance.set_surface_override_material(0, &material);

    // Attach an OmniLight3D so the beam illuminates surroundings
    let mut light = OmniLight3D::new_alloc();
    light.set_color(Color::from_rgb(color[0], color[1], color[2]));
    light.set_param(light_3d::Param::ENERGY, 3.0);
    light.set_param(light_3d::Param::RANGE, 4.0);
    light.set_param(light_3d::Param::ATTENUATION, 2.0);
    mesh_instance.add_child(&light);

    mesh_instance.set_meta(meta_keys::BEAM_AGE, &Variant::from(0.0_f32));

    let beam_basis = basis_from_direction(to - from);
    let transform = Transform3D { basis: beam_basis, origin: midpoint };
    mesh_instance.set_transform(transform);

    Some(mesh_instance)
}

/// Attach a colored point light to a node so it reads as "glowing" (collectible
/// pickups, the ship's color accent). Returns the light so callers that need to
/// recolor it later can keep the handle; callers that don't can ignore it.
pub fn attach_glow_light(parent: &mut Gd<Node3D>, color: &[f32], energy: f32, range: f32) -> LiveRef<OmniLight3D> {
    let mut light = OmniLight3D::new_alloc();
    light.set_color(Color::from_rgb(color[0], color[1], color[2]));
    light.set_param(light_3d::Param::ENERGY, energy);
    light.set_param(light_3d::Param::RANGE, range);
    light.set_param(light_3d::Param::ATTENUATION, 1.5);
    parent.add_child(&light);
    LiveRef::new(&light)
}

/// A neutral white key light so a turntable subject (and the dim room around it)
/// reads against the dark backdrop a freshly-generated base spawns with. Placed
/// on the Y axis at eye height so it stays put as the turntable spins. One key
/// for both display screens: the ship showcase and the bestiary briefing.
pub fn attach_key_light(parent: &mut Gd<Node3D>, energy: f32, range: f32) {
    let mut key = OmniLight3D::new_alloc();
    key.set_color(Color::from_rgb(1.0, 1.0, 0.97));
    key.set_param(light_3d::Param::ENERGY, energy);
    key.set_param(light_3d::Param::RANGE, range);
    key.set_position(Vector3::new(0.0, 2.5, 0.0));
    parent.add_child(&key);
}

/// Convert a void-logic `[R, G, B, A]` color into a Godot `Color`. The one place
/// the boundary conversion lives.
pub fn to_color(rgba: [f32; 4]) -> Color {
    Color::from_rgba(rgba[0], rgba[1], rgba[2], rgba[3])
}

/// Re-skin the ship's painted hull to a body `style` (1/2/3) by swapping just
/// the styled material's albedo + roughness textures. Only the `SPC_Asset_4`
/// hull group varies between styles — the greeble/detail materials are identical
/// across all three, so they're left untouched. One path, shared by the
/// in-flight ship and the showcase so they always match the loadout.
pub fn apply_body_style(model: &Gd<Node3D>, style: u8, tex_index: u32) {
    let dir = format!("res://addons/ships/Spacecraft_1_styles/Style_{style}");
    let base_path = format!("{dir}/TX_spacecraft_1_{tex_index}_Base_color.png");
    let rough_path = format!("{dir}/TX_spacecraft_1_{tex_index}_Roughness.png");
    let mut loader = ResourceLoader::singleton();
    // Skip cleanly if the styles aren't imported yet (e.g. before `make assets`).
    // Loading a missing res:// path emits an engine error, so guard with exists().
    if !loader.exists(&base_path) || !loader.exists(&rough_path) {
        return;
    }
    let (Some(base), Some(rough)) = (loader.load(&base_path), loader.load(&rough_path)) else {
        return;
    };
    let base_tex: Gd<Texture2D> = base.cast();
    let rough_tex: Gd<Texture2D> = rough.cast();

    let mut restyled = 0;
    let mut stack: Vec<Gd<Node>> = vec![model.clone().upcast()];
    while let Some(node) = stack.pop() {
        for child in node.get_children().iter_shared() {
            stack.push(child);
        }
        // The painted-hull meshes are the SPC_Asset_4 group (styled material).
        if !node.get_name().to_string().starts_with("SPC_Asset_4") {
            continue;
        }
        let Ok(mut mesh) = node.try_cast::<MeshInstance3D>() else { continue };
        let Some(geom) = mesh.get_mesh() else { continue };
        for i in 0..geom.get_surface_count() {
            let Some(mat) = mesh.get_active_material(i) else { continue };
            // Duplicate so each instance owns its skin — the in-flight ship and
            // the showcase can wear different styles from the same imported glb.
            let Ok(mut bm) = mat.duplicate_resource().try_cast::<BaseMaterial3D>() else { continue };
            bm.set_texture(base_material_3d::TextureParam::ALBEDO, &base_tex);
            bm.set_texture(base_material_3d::TextureParam::ROUGHNESS, &rough_tex);
            mesh.set_surface_override_material(i, &bm);
            restyled += 1;
        }
    }
    if restyled == 0 {
        godot_warn!("body style {style}: no SPC_Asset_4 hull surfaces matched");
    }
}

/// Recolor a glow light (e.g. when the player changes ship color). No-op if the
/// handle is absent or freed. One recolor path for the player and the showcase.
pub fn recolor_glow(glow: &Option<LiveRef<OmniLight3D>>, color: Color) {
    glow.with(|light| light.set_color(Color::from_rgb(color.r, color.g, color.b)));
}

/// Load the ship model scene at `path`, instance it under `parent`, scale it to
/// `length`, and face its nose forward. Returns the model node. The single
/// source of truth for building the ship — both the player and the menu
/// showcase call this instead of repeating the load/fit/yaw sequence.
pub fn spawn_fitted_model(parent: &mut Gd<Node3D>, path: &str, length: f32) -> Option<Gd<Node3D>> {
    let mut model = spawn_model_fitted(parent, path, length)?;
    model.rotate_y(SHIP_MODEL_YAW);
    Some(model)
}

/// Load the model at `path`, instance it under `parent` named "Model", and
/// fit-scale it to `length` — with no facing applied. Enemies use this (their
/// imported models keep their own orientation); the ship wraps it with a yaw.
pub fn spawn_model_fitted(parent: &mut Gd<Node3D>, path: &str, length: f32) -> Option<Gd<Node3D>> {
    let scene = ResourceLoader::singleton().load(path)?;
    let packed = scene.try_cast::<PackedScene>().ok()?;
    let instance = packed.instantiate()?;
    let mut model: Gd<Node3D> = instance.cast();
    model.set_name("Model");
    parent.add_child(&model);
    fit_model_to_length(&mut model, length);
    Some(model)
}

/// Uniformly scale `model` (already in the tree, currently unscaled) so its
/// longest dimension spans `target` world units. No-op if it has no meshes.
/// Robust to whatever native units the imported model uses.
pub fn fit_model_to_length(model: &mut Gd<Node3D>, target: f32) {
    if let Some(aabb) = combined_mesh_aabb(model) {
        let longest = aabb.size.x.max(aabb.size.y).max(aabb.size.z).max(0.001);
        let scale = target / longest;
        model.set_scale(Vector3::splat(scale));
        // Re-centre: imported models often sit off their own origin, which
        // makes them render off-axis and orbit when rotated. Shift so the
        // geometry centroid lands on the node origin.
        let center = aabb.position + aabb.size * 0.5;
        model.set_position(-center * scale);
    }
}

/// Add convex-hull collision shapes (one per `MeshInstance3D`) found under
/// `node` to `body`, each placed by its accumulated transform starting from
/// `xform`. The dynamic-safe, mesh-hugging collider for loose props (Jolt
/// allows convex hulls on dynamic bodies; concave trimesh is static-only). The
/// player ship deliberately uses a capsule instead — a mesh hull baked with the
/// model's extreme fit-scale confused Jolt and snagged the ship on doorways.
pub fn add_convex_collision(body: &mut Gd<RigidBody3D>, node: &Gd<Node3D>, xform: Transform3D) {
    if let Ok(mesh_inst) = node.clone().try_cast::<MeshInstance3D>() {
        if let Some(mesh) = mesh_inst.get_mesh() {
            // simplify(true) collapses near-coplanar faces, so the hull is a
            // handful of planes instead of (worst case) one per source triangle
            // — an unsimplified hull makes Jolt narrowphase crawl with 29 enemies.
            if let Some(shape) = mesh.create_convex_shape_ex().simplify(true).done() {
                let mut col = CollisionShape3D::new_alloc();
                col.set_shape(&shape);
                col.set_transform(xform);
                body.add_child(&col);
            }
        }
    }
    for child in node.get_children().iter_shared() {
        if let Ok(child3d) = child.try_cast::<Node3D>() {
            let child_xform = xform * child3d.get_transform();
            add_convex_collision(body, &child3d, child_xform);
        }
    }
}

/// Union of every `MeshInstance3D` AABB under `root`, expressed in `root`'s
/// local space. `None` if there are no meshes. Used to fit a collision box to
/// an imported model regardless of how it's nested or scaled.
pub fn combined_mesh_aabb(root: &Gd<Node3D>) -> Option<godot::builtin::Aabb> {
    use godot::classes::Node;
    use godot::builtin::Transform3D;
    let mut result: Option<godot::builtin::Aabb> = None;
    // Chain LOCAL transforms down from `root` rather than reading global ones:
    // the model is measured the instant it's add_child'd, before global
    // transforms propagate, so a cold global read misses (returns the unscaled
    // mesh). Local transforms are valid immediately.
    let mut stack: Vec<(Gd<Node>, Transform3D)> =
        vec![(root.clone().upcast(), Transform3D::IDENTITY)];
    while let Some((node, to_root)) = stack.pop() {
        for child in node.get_children().iter_shared() {
            let local = child
                .clone()
                .try_cast::<Node3D>()
                .map(|n| n.get_transform())
                .unwrap_or(Transform3D::IDENTITY);
            stack.push((child, to_root * local));
        }
        if let Ok(mesh) = node.try_cast::<MeshInstance3D>() {
            if mesh.get_mesh().is_some() {
                let aabb = to_root * mesh.get_aabb();
                result = Some(match result {
                    Some(acc) => acc.merge(aabb),
                    None => aabb,
                });
            }
        }
    }
    result
}

/// Age all beams in a list, fading their alpha and dropping the ones that have
/// lived out their lifetime (or been freed). Beams are held by identity, so an
/// already-freed beam is dropped rather than touched.
pub fn age_beams(beams: &mut LiveVec<MeshInstance3D>, delta: f32, lifetime: f32, color: &[f32]) {
    beams.retain_live(|beam, _| {
        let age = beam.get_meta(meta_keys::BEAM_AGE).to::<f32>() + delta;
        if age >= lifetime {
            beam.queue_free();
            return false;
        }
        beam.set_meta(meta_keys::BEAM_AGE, &Variant::from(age));
        let alpha = 1.0 - (age / lifetime);
        if let Some(mat) = beam.get_surface_override_material(0) {
            let mut std_mat = mat.cast::<StandardMaterial3D>();
            std_mat.set_albedo(Color::from_rgba(color[0], color[1], color[2], alpha));
        }
        // Fade the attached light
        for child in beam.get_children().iter_shared() {
            if let Ok(mut light) = child.try_cast::<OmniLight3D>() {
                light.set_param(light_3d::Param::ENERGY, 3.0 * alpha);
            }
        }
        true
    });
}

/// Create a `ParticleProcessMaterial` with common burst settings.
pub fn particle_burst_material(
    spread: f32,
    color: Color,
    velocity_range: (f32, f32),
    scale_range: Option<(f32, f32)>,
) -> Gd<ParticleProcessMaterial> {
    let mut mat = ParticleProcessMaterial::new_gd();
    mat.set_spread(spread);
    mat.set_color(color);
    mat.set_gravity(Vector3::ZERO);
    mat.set_param_min(Parameter::INITIAL_LINEAR_VELOCITY, velocity_range.0);
    mat.set_param_max(Parameter::INITIAL_LINEAR_VELOCITY, velocity_range.1);
    if let Some((min_s, max_s)) = scale_range {
        mat.set_param_min(Parameter::SCALE, min_s);
        mat.set_param_max(Parameter::SCALE, max_s);
    }
    mat
}
