"""Headless mesh conversion for `make assets`, run via Blender.

    blender --background --python scripts/decimate.py -- <in> <out.glb> <target_tris> <tex_dir>

Imports a source mesh, ensures it carries PBR material(s), collapses geometry to
roughly <target_tris> triangles, downscales every texture, and writes a
self-contained .glb with the maps embedded. Two source flavors are handled:

  * FBX (cgtrader evil-mechs): ship their PBR maps as loose Substance exports the
    FBX doesn't reference, so the importer yields untextured meshes — we rebuild a
    single Principled-BSDF material from the named maps in <tex_dir>, and bake out
    the importer's +90° X rotation so the model sits upright.
  * OBJ (cgtrader jump gate): its .mtl references the textures sitting next to the
    .obj, so Blender's importer already builds textured materials — we keep them
    as-is and <tex_dir> is ignored.

Either way the shared tail downscales/packs every imported texture, decimates,
and exports.
"""
import os
import sys

import bpy

argv = sys.argv[sys.argv.index("--") + 1:]
in_path, out_path, target_tris, tex_dir = argv[0], argv[1], int(argv[2]), argv[3]

# Map size every texture is downscaled to before embedding (square). 1k is
# plenty for these props and keeps the .glb and VRAM modest.
TEX_SIZE = 1024

# Start from an empty scene, then import the source mesh by format.
bpy.ops.wm.read_factory_settings(use_empty=True)
ext = os.path.splitext(in_path)[1].lower()
if ext == ".fbx":
    bpy.ops.import_scene.fbx(filepath=in_path)
elif ext == ".obj":
    bpy.ops.wm.obj_import(filepath=in_path)
else:
    raise SystemExit(f"decimate: unsupported input format '{ext}' ({in_path})")

meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]


def load_map(filename, non_color=False):
    """Load a loose texture from <tex_dir> for the FBX material rebuild. Returns
    the image, or None if the file is absent. Downscaling/packing happens once
    for every image in the shared pass below."""
    path = os.path.join(tex_dir, filename)
    if not os.path.exists(path):
        return None
    img = bpy.data.images.load(path)
    if non_color:
        img.colorspace_settings.name = "Non-Color"
    return img


def build_mech_material():
    """Rebuild one Principled-BSDF material from the loose Substance maps the FBX
    doesn't reference, and assign it to every mesh part (the mechs use a single
    shading group, so one material covers them all)."""
    mat = bpy.data.materials.new(name="mech")
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    bsdf = nodes.get("Principled BSDF")

    def hook(filename, target_input, non_color=False):
        img = load_map(filename, non_color=non_color)
        if img is None:
            return
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = img
        links.new(tex.outputs["Color"], bsdf.inputs[target_input])

    hook("initialShadingGroup_Base_color.jpg", "Base Color")
    hook("initialShadingGroup_Metallic.jpg", "Metallic", non_color=True)
    hook("initialShadingGroup_Roughness.jpg", "Roughness", non_color=True)
    hook("initialShadingGroup_Emissive.jpg", "Emission Color")
    bsdf.inputs["Emission Strength"].default_value = 1.0

    # Normal map needs a Normal Map node between the texture and the BSDF.
    normal_img = load_map("initialShadingGroup_Normal_OpenGL.jpg", non_color=True)
    if normal_img is not None:
        tex_node = nodes.new("ShaderNodeTexImage")
        tex_node.image = normal_img
        nmap = nodes.new("ShaderNodeNormalMap")
        links.new(tex_node.outputs["Color"], nmap.inputs["Color"])
        links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])

    for o in meshes:
        o.data.materials.clear()
        o.data.materials.append(mat)


# FBX needs its importer artefacts corrected; OBJ arrives upright and textured.
if ext == ".fbx":
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
    build_mech_material()

# Downscale + pack every imported texture so the glTF exporter embeds the small
# version, not the multi-thousand-pixel originals. Covers the FBX rebuilt maps
# and the OBJ importer's auto-loaded MTL textures alike.
for img in bpy.data.images:
    if img.source != "FILE":
        continue
    if max(img.size) > TEX_SIZE:
        img.scale(TEX_SIZE, TEX_SIZE)
    img.pack()

# Current triangle total (loop_triangles resolves quads/ngons to tris).
total = 0
for o in meshes:
    o.data.calc_loop_triangles()
    total += len(o.data.loop_triangles)

# target_tris <= 0 means "keep full detail" — for a single static prop (the jump
# gate) decimation isn't needed, and a uniform COLLAPSE ratio annihilates tiny
# but load-bearing meshes (e.g. the gate's 2-triangle energy-field plane). Only
# the many-instances enemy mechs actually need the collapse.
if target_tris > 0 and total > target_tris:
    ratio = target_tris / total
    for o in meshes:
        mod = o.modifiers.new(name="decimate", type="DECIMATE")
        mod.decimate_type = "COLLAPSE"
        mod.ratio = ratio
    summary = f"{total} -> ~{int(total * ratio)} tris (ratio {ratio:.3f})"
else:
    summary = f"{total} tris kept (no decimation)"

# export_apply bakes the decimate modifier; GLB embeds the packed textures.
bpy.ops.export_scene.gltf(filepath=out_path, export_format="GLB", export_apply=True)

print(f"decimate: {summary} -> {out_path}")
