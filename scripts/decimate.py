"""Headless mesh conversion for `make assets`, run via Blender.

    blender --background --python scripts/decimate.py -- \\
        <in> <out.glb> <target_tris> <tex_dir> [base_color_file]

Imports a source mesh, ensures it carries PBR material(s), collapses geometry to
roughly <target_tris> triangles, caps every texture at TEX_CAP on its longest
side, and writes a self-contained .glb with the maps embedded. Accepted
inputs: .fbx, .obj, .glb/.gltf.

Materials: when the importer already wires textures (an OBJ whose .mtl
references its maps, a .glb with embedded textures), they are kept as-is.
When it yields untextured meshes (cgtrader FBX/OBJ drops ship their Substance
maps loose, unreferenced), one Principled-BSDF material is rebuilt from
<tex_dir>, whose files are matched to channels by suffix — the packs name
their maps freely ("initialShadingGroup_Base_color.jpg", "Sphere_drone_01
_Metallic.jpg", "Normal Map.png", "Metal.png"), so matching is
case/space/underscore-insensitive. Pass <tex_dir> as "-" when there is
nothing to rebuild from. [base_color_file] disambiguates packs that ship
multiple base coats (the apartment boss's "Base Plain"/"Base Rusted").

FBX additionally gets the importer's +90° X rotation baked out so the model
sits upright in Godot. Static: the provider's animations are stripped before
any transform bake (see below).
"""
import os
import re
import sys

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from plan_apply import cap_and_pack_images  # noqa: E402

argv = sys.argv[sys.argv.index("--") + 1:]
in_path, out_path, target_tris, tex_dir = argv[0], argv[1], int(argv[2]), argv[3]
base_color_file = argv[4] if len(argv) > 4 else None

# Longest-side cap for the props' maps, aspect-true (the old 1024 SQUARE
# scale squashed every non-square map). A prop is a couple of meters seen
# across a room, so 4k is beyond what it can show; the scenes carry no
# cap at all (install-addons.sh ENV_TEX_CAP — owner 2026-09-06: "why are
# we being precious with these resources?").
TEX_CAP = 4096

# Channel classification for loose texture maps, evaluated in order against
# the normalized (lowercase, separators stripped) file stem; first suffix
# match wins. `None` channels are recognized-but-unused maps we skip silently
# (packed ORM/AO/masks aren't worth splitting for these props).
CHANNEL_SUFFIXES = [
    ("base", ("basecolor", "baseplain", "baserusted", "albedo", "diffuse")),
    (None, ("normaldirectx",)),
    ("normal", ("normalopengl", "normalmap", "normal")),
    ("metal", ("metallic", "metal")),
    ("rough", ("roughness",)),
    ("emit", ("emissive", "emission")),
    (None, ("height", "mixedao", "ao", "ormboth", "orm", "colormasks", "masks", "mask")),
]
IMAGE_EXTS = {".jpg", ".jpeg", ".png"}

# Start from an empty scene, then import the source mesh by format.
bpy.ops.wm.read_factory_settings(use_empty=True)
ext = os.path.splitext(in_path)[1].lower()
if ext == ".fbx":
    bpy.ops.import_scene.fbx(filepath=in_path)
elif ext == ".obj":
    bpy.ops.wm.obj_import(filepath=in_path)
elif ext in (".glb", ".gltf"):
    bpy.ops.import_scene.gltf(filepath=in_path)
else:
    raise SystemExit(f"decimate: unsupported input format '{ext}' ({in_path})")

meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]

# Purge image datablocks whose backing file is gone (the apartment-boss FBX
# references a "Texture Final/" folder the provider never shipped). Removing
# them empties the broken TEX_IMAGE nodes, so the material-rebuild check
# below sees the model as untextured and rebuilds from the loose maps —
# and the pack pass can't trip over a nonexistent source path.
for img in list(bpy.data.images):
    if img.source == "FILE" and img.packed_file is None \
            and not os.path.exists(bpy.path.abspath(img.filepath)):
        print(f"decimate: dropping broken image reference: {img.filepath}")
        bpy.data.images.remove(img)


def classify_channels():
    """Match <tex_dir>'s files to material channels by normalized suffix.
    Returns {channel: filename}. Multiple base coats require the explicit
    [base_color_file] pick; other collisions keep the first (sorted) file."""
    channels = {}
    for filename in sorted(os.listdir(tex_dir)):
        stem, file_ext = os.path.splitext(filename)
        if file_ext.lower() not in IMAGE_EXTS:
            continue
        norm = re.sub(r"[\s_\-]+", "", stem.lower())
        for channel, suffixes in CHANNEL_SUFFIXES:
            if not norm.endswith(suffixes):
                continue
            if channel is None:
                break
            if channel == "base" and base_color_file:
                if filename == base_color_file:
                    channels[channel] = filename
            elif channel not in channels:
                channels[channel] = filename
            elif channel == "base":
                raise SystemExit(
                    f"decimate: multiple base-color candidates in {tex_dir} "
                    f"({channels[channel]!r} vs {filename!r}) — pass the "
                    f"[base_color_file] argument to pick one"
                )
            break
        else:
            print(f"decimate: unrecognized map skipped: {filename}")
    if base_color_file and channels.get("base") != base_color_file:
        raise SystemExit(
            f"decimate: base color {base_color_file!r} not found in {tex_dir}"
        )
    return channels


def load_map(filename, non_color=False):
    """Load a loose texture from <tex_dir> for the material rebuild.
    Capping/packing happens once for every image in the shared pass
    below."""
    img = bpy.data.images.load(os.path.join(tex_dir, filename))
    if non_color:
        img.colorspace_settings.name = "Non-Color"
    return img


def build_material():
    """Rebuild one Principled-BSDF material from the loose maps the source
    doesn't reference, and assign it to every mesh part (these packs use a
    single shading group, so one material covers them all)."""
    maps = classify_channels()
    if not maps:
        print(f"decimate: no usable maps in {tex_dir} — leaving materials untextured")
        return
    mat = bpy.data.materials.new(name="rebuilt")
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    bsdf = nodes.get("Principled BSDF")

    def hook(channel, target_input, non_color=False):
        if channel not in maps:
            return
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = load_map(maps[channel], non_color=non_color)
        links.new(tex.outputs["Color"], bsdf.inputs[target_input])

    hook("base", "Base Color")
    hook("metal", "Metallic", non_color=True)
    hook("rough", "Roughness", non_color=True)
    hook("emit", "Emission Color")
    bsdf.inputs["Emission Strength"].default_value = 1.0

    # Normal map needs a Normal Map node between the texture and the BSDF.
    if "normal" in maps:
        tex_node = nodes.new("ShaderNodeTexImage")
        tex_node.image = load_map(maps["normal"], non_color=True)
        nmap = nodes.new("ShaderNodeNormalMap")
        links.new(tex_node.outputs["Color"], nmap.inputs["Color"])
        links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])

    for o in meshes:
        o.data.materials.clear()
        o.data.materials.append(mat)
    print(f"decimate: rebuilt material from {tex_dir}: {sorted(maps.values())}")


def materials_are_textured():
    """Whether any imported mesh material already carries a wired image
    texture (an OBJ with a real .mtl, a .glb with embedded maps)."""
    return any(
        node.type == "TEX_IMAGE" and node.image is not None
        for o in meshes
        for mat in o.data.materials
        if mat is not None and mat.use_nodes
        for node in mat.node_tree.nodes
    )


# ---- static: strip the provider's animations before any transform bake ----
# The game plays no provider animation; an animated node's keyed transform
# is one more thing the product would carry that the roster did not ask
# for (the military hull's gear and canopy shipped as centimeter boxes at
# the wrong place until convert-hull.py stripped its actions, 2026-09-06).
# The audit holds every decimated model to it.
stripped = 0
for o in bpy.context.scene.objects:
    if o.animation_data is not None:
        o.animation_data_clear()
        stripped += 1
for action in list(bpy.data.actions):
    bpy.data.actions.remove(action)
if stripped:
    print(f"decimate: {stripped} animated objects made static")


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

# Importer-wired textures are kept; otherwise rebuild from the loose maps.
if not materials_are_textured() and tex_dir not in ("", "-") and os.path.isdir(tex_dir):
    build_material()

# Cap (aspect-true) + pack every imported texture so the glTF exporter embeds
# it — rebuilt maps, the OBJ importer's auto-loaded MTL textures, and
# glb-embedded images alike. One capping door for scenes and props.
cap_and_pack_images(TEX_CAP)

# Current triangle total (loop_triangles resolves quads/ngons to tris).
total = 0
for o in meshes:
    o.data.calc_loop_triangles()
    total += len(o.data.loop_triangles)

# target_tris <= 0 means "keep full detail" — for a single static prop (the jump
# gate) decimation isn't needed, and a uniform COLLAPSE ratio annihilates tiny
# but load-bearing meshes (e.g. the gate's 2-triangle energy-field plane). Only
# the many-instances enemy models actually need the collapse.
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
# No vertex colors: no material reads them, and the exporter warns per
# mesh about an active color layer its material never uses.
bpy.ops.export_scene.gltf(
    filepath=out_path, export_format="GLB", export_apply=True,
    export_animations=False, export_vertex_color="NONE",
)

print(f"decimate: {summary} -> {out_path}")
