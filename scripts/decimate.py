"""Headless mesh decimation + texturing, run by `make assets` via Blender.

    blender --background --python scripts/decimate.py -- <in.fbx> <out.glb> <target_tris> <textures_dir>

Imports the FBX, collapses every mesh so the model totals roughly <target_tris>
triangles, rebuilds a PBR material from the loose PBR maps in <textures_dir>
(the cgtrader mechs ship their textures as separate Substance exports the FBX
doesn't reference, so Blender imports them untextured), and writes a
self-contained .glb with the maps embedded.

The cgtrader enemy mechs ship at 21-26k tris with 2-8k textures each — far too
heavy to render 29 of them twice (SBS) and to build convex hulls from — so they
get knocked down to a game-weight budget here: a couple thousand tris and 1k
maps, which is ample for a 1-2 m drone.
"""
import os
import sys

import bpy

argv = sys.argv[sys.argv.index("--") + 1:]
in_path, out_path, target_tris, tex_dir = argv[0], argv[1], int(argv[2]), argv[3]

# Map size the maps are downscaled to before embedding (square). 1k is plenty
# for a small drone and keeps the .glb and VRAM modest.
TEX_SIZE = 1024

# Start from an empty scene, then import the FBX.
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.fbx(filepath=in_path)

meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]

# The FBX importer leaves a +90° X rotation on every object (its Z-up→Y-up
# conversion). Left as an object transform it survives the glTF round-trip and
# the model imports into Godot pitched on its back. Bake it into the mesh data
# so the exported model sits upright with an identity transform, and a runtime
# look_at only has to correct yaw.
bpy.ops.object.select_all(action="DESELECT")
for o in meshes:
    o.select_set(True)
bpy.context.view_layer.objects.active = meshes[0] if meshes else None
bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)


def load_map(filename, non_color=False):
    """Load a texture from the textures dir, downscale it, and pack the scaled
    pixels so the glTF exporter embeds the small version (not the 2-8k original).
    Returns the image, or None if the file is absent."""
    path = os.path.join(tex_dir, filename)
    if not os.path.exists(path):
        return None
    img = bpy.data.images.load(path)
    img.scale(TEX_SIZE, TEX_SIZE)
    if non_color:
        img.colorspace_settings.name = "Non-Color"
    img.pack()
    return img


# Build one Principled-BSDF material from the loose Substance maps. The cgtrader
# mechs use a single shading group, so one material covers every mesh part.
mat = bpy.data.materials.new(name="mech")
mat.use_nodes = True
nodes = mat.node_tree.nodes
links = mat.node_tree.links
bsdf = nodes.get("Principled BSDF")


def hook(filename, target_input, non_color=False):
    img = load_map(filename, non_color=non_color)
    if img is None:
        return None
    tex = nodes.new("ShaderNodeTexImage")
    tex.image = img
    links.new(tex.outputs["Color"], bsdf.inputs[target_input])
    return tex


hook("initialShadingGroup_Base_color.jpg", "Base Color")
hook("initialShadingGroup_Metallic.jpg", "Metallic", non_color=True)
hook("initialShadingGroup_Roughness.jpg", "Roughness", non_color=True)
hook("initialShadingGroup_Emissive.jpg", "Emission Color")
bsdf.inputs["Emission Strength"].default_value = 1.0

# Normal map needs a Normal Map node between the texture and the BSDF.
normal_tex = load_map("initialShadingGroup_Normal_OpenGL.jpg", non_color=True)
if normal_tex is not None:
    tex_node = nodes.new("ShaderNodeTexImage")
    tex_node.image = normal_tex
    nmap = nodes.new("ShaderNodeNormalMap")
    links.new(tex_node.outputs["Color"], nmap.inputs["Color"])
    links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])

# Replace whatever the FBX importer gave each mesh with our one material.
for o in meshes:
    o.data.materials.clear()
    o.data.materials.append(mat)

# Current triangle total (loop_triangles resolves quads/ngons to tris).
total = 0
for o in meshes:
    o.data.calc_loop_triangles()
    total += len(o.data.loop_triangles)

ratio = min(1.0, target_tris / max(total, 1))

for o in meshes:
    mod = o.modifiers.new(name="decimate", type="DECIMATE")
    mod.decimate_type = "COLLAPSE"
    mod.ratio = ratio

# export_apply bakes the decimate modifier; GLB embeds the packed textures.
bpy.ops.export_scene.gltf(filepath=out_path, export_format="GLB", export_apply=True)

print(f"decimate: {total} -> ~{int(total * ratio)} tris (ratio {ratio:.3f}) -> {out_path}")
