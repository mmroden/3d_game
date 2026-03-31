//! Shared Godot-dependent utilities for node code.
//! Basis helpers, beam rendering, and particle material builders.

use godot::prelude::*;
use godot::classes::{
    MeshInstance3D, BoxMesh, StandardMaterial3D,
    ParticleProcessMaterial,
    particle_process_material::Parameter,
};

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
    material.set_emission(Color::from_rgba(color[0], color[1], color[2], 1.0));
    material.set_emission_energy_multiplier(5.0);
    mesh_instance.set_surface_override_material(0, &material);

    mesh_instance.set_meta("beam_age", &Variant::from(0.0_f32));

    let beam_basis = basis_from_direction(to - from);
    let transform = Transform3D { basis: beam_basis, origin: midpoint };
    mesh_instance.set_transform(transform);

    Some(mesh_instance)
}

/// Age all beams in a list, fading their alpha. Returns only those still alive.
pub fn age_beams(beams: &mut Vec<Gd<MeshInstance3D>>, delta: f32, lifetime: f32, color: &[f32]) {
    beams.retain_mut(|beam| {
        if !beam.is_instance_valid() {
            return false;
        }
        let age = beam.get_meta("beam_age").to::<f32>() + delta;
        if age >= lifetime {
            beam.queue_free();
            false
        } else {
            beam.set_meta("beam_age", &Variant::from(age));
            let alpha = 1.0 - (age / lifetime);
            if let Some(mat) = beam.get_surface_override_material(0) {
                let mut std_mat = mat.cast::<StandardMaterial3D>();
                std_mat.set_albedo(Color::from_rgba(color[0], color[1], color[2], alpha));
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
