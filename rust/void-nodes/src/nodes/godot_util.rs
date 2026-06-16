//! Shared Godot-dependent utilities for node code.
//! Basis helpers, beam rendering, and particle material builders.

use godot::prelude::*;
use godot::classes::{
    MeshInstance3D, BoxMesh, StandardMaterial3D, OmniLight3D,
    RigidBody3D, CollisionShape3D, PackedScene, ResourceLoader,
    ParticleProcessMaterial,
    particle_process_material::Parameter,
    light_3d,
};

use super::audio_manager::AudioManager;
use super::constants::{meta_keys, nodes};

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
pub fn attach_glow_light(parent: &mut Gd<Node3D>, color: &[f32], energy: f32, range: f32) -> Gd<OmniLight3D> {
    let mut light = OmniLight3D::new_alloc();
    light.set_color(Color::from_rgb(color[0], color[1], color[2]));
    light.set_param(light_3d::Param::ENERGY, energy);
    light.set_param(light_3d::Param::RANGE, range);
    light.set_param(light_3d::Param::ATTENUATION, 1.5);
    parent.add_child(&light);
    light
}

/// Recolor a glow light (e.g. when the player changes ship color). No-op if the
/// handle is absent or freed. One recolor path for the player and the showcase.
pub fn recolor_glow(glow: &mut Option<Gd<OmniLight3D>>, color: Color) {
    if let Some(light) = glow {
        if light.is_instance_valid() {
            light.set_color(Color::from_rgb(color.r, color.g, color.b));
        }
    }
}

/// Load the ship model scene at `path`, instance it under `parent`, scale it to
/// `length`, and face its nose forward. Returns the model node. The single
/// source of truth for building the ship — both the player and the menu
/// showcase call this instead of repeating the load/fit/yaw sequence.
pub fn spawn_fitted_model(parent: &mut Gd<Node3D>, path: &str, length: f32) -> Option<Gd<Node3D>> {
    let scene = ResourceLoader::singleton().load(path)?;
    let packed = scene.try_cast::<PackedScene>().ok()?;
    let instance = packed.instantiate()?;
    let mut model: Gd<Node3D> = instance.cast();
    model.set_name("Model");
    parent.add_child(&model);
    fit_model_to_length(&mut model, length);
    model.rotate_y(SHIP_MODEL_YAW);
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
            if let Some(shape) = mesh.create_convex_shape() {
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
    let inv = root.get_global_transform().affine_inverse();
    let mut result: Option<godot::builtin::Aabb> = None;
    let mut stack: Vec<Gd<Node>> = vec![root.clone().upcast()];
    while let Some(node) = stack.pop() {
        for child in node.get_children().iter_shared() {
            stack.push(child);
        }
        if let Ok(mesh) = node.try_cast::<MeshInstance3D>() {
            if mesh.get_mesh().is_some() {
                let local = inv * mesh.get_global_transform();
                let aabb = local * mesh.get_aabb();
                result = Some(match result {
                    Some(acc) => acc.merge(aabb),
                    None => aabb,
                });
            }
        }
    }
    result
}

/// Age all beams in a list, fading their alpha. Returns only those still alive.
pub fn age_beams(beams: &mut Vec<Gd<MeshInstance3D>>, delta: f32, lifetime: f32, color: &[f32]) {
    beams.retain_mut(|beam| {
        if !beam.is_instance_valid() {
            return false;
        }
        let age = beam.get_meta(meta_keys::BEAM_AGE).to::<f32>() + delta;
        if age >= lifetime {
            beam.queue_free();
            false
        } else {
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
        }
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
