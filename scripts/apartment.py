"""Headless apartment-environment conversion for `make assets`, run via Blender.

    blender --background --python scripts/apartment.py -- \
        <in.fbx> <out.glb> <textures.zip> [tex_cap]

Converts the planet-3 apartment (a 3ds Max/Corona archviz export) into one
self-contained .glb at provider scale. The FBX's texture filename fields are
EMPTY (Corona materials don't survive FBX), so Blender's importer wires zero
textures — but the FBX still carries its Texture->Material->Model connection
table, and the texture element names (ZJ-001846) match the files in the
textures zip (ZJ-001846.jpg). We parse the FBX connection table ourselves
(via Blender's own io_scene_fbx parser), then rebuild each material's Base
Color from its connected diffuse map.

No decimation: this is a single environment instance, not a many-instances
enemy model (owner 2026-07-11). Textures are capped at [tex_cap]
(default 2048) — room-scale surfaces are seen close-up at 5x scale, so the
1024 prop budget doesn't apply, but uncapped archviz sources are a VRAM
problem.

The rebuild FAILS if fewer than MIN_WIRED of materials get a texture —
a gray house must never ship silently.
"""
import os
import sys
import zipfile

import bpy

argv = sys.argv[sys.argv.index("--") + 1:]
in_path, out_path, zip_path = argv[0], argv[1], argv[2]
tex_cap = int(argv[3]) if len(argv) > 3 else 2048

MIN_WIRED = 0.80  # fraction of texture-connected materials that must wire

# ---- extract the textures beside the FBX (idempotent) ----
tex_dir = os.path.join(os.path.dirname(in_path), "textures")
if not os.path.isdir(tex_dir):
    with zipfile.ZipFile(zip_path) as zf:
        zf.extractall(os.path.dirname(in_path))
    print(f"apartment: extracted {zip_path} -> {tex_dir}")

# Index the extracted files by stem for case-insensitive lookup.
tex_files = {}
for f in os.listdir(tex_dir):
    stem, ext = os.path.splitext(f)
    if ext.lower() in (".jpg", ".jpeg", ".png", ".tif", ".tiff"):
        tex_files[stem.lower()] = os.path.join(tex_dir, f)

# ---- parse the FBX connection table (texture -> material, channel) ----
from io_scene_fbx import parse_fbx  # Blender's own binary-FBX parser


def fbx_name(raw):
    """FBX names are b'Name\\x00\\x01Class' — return the Name part."""
    return raw.split(b"\x00", 1)[0].decode("utf-8", "replace")


elem_root, _ = parse_fbx.parse(in_path)
objects_elem = next(e for e in elem_root.elems if e.id == b"Objects")
connections_elem = next(e for e in elem_root.elems if e.id == b"Connections")

textures = {}   # uid -> texture element name (a node id, NOT a filename)
materials = {}  # uid -> material name (matches a Blender material, pre-dedup)
videos = {}     # uid -> file stem: Video elements carry the REAL filename
                # (an absolute path from the author's machine — only the
                # basename means anything here)
for e in objects_elem.elems:
    if e.id == b"Texture":
        textures[e.props[0]] = fbx_name(e.props[1])
    elif e.id == b"Material":
        materials[e.props[0]] = fbx_name(e.props[1])
    elif e.id == b"Video":
        raw = next(
            (c.props[0] for c in e.elems if c.id in (b"FileName", b"Filename") and c.props[0]),
            b"",
        )
        if raw:
            basename = raw.decode("utf-8", "replace").replace("\\", "/").rsplit("/", 1)[-1]
            videos[e.props[0]] = os.path.splitext(basename)[0]

# Texture --OP(property)--> Material. This export names its channels with
# 3ds Max/Corona property paths (there is no standard "DiffuseColor" texture
# connection in the file); v1 wires the diffuse/base channel only —
# glossiness/bump/opacity arrive as Corona-specific encodings that aren't
# worth translating for a fly-through environment. Flat-color materials need
# nothing here: their standard DiffuseColor property survives the normal
# Blender import. Unknown channels are counted and reported, never silent.
DIFFUSE_CHANNELS = {
    "DiffuseColor",
    "3dsMax|CoronaMtlPb|texmapDiffuse",
    "3dsMax|CoronaPhysicalMtlPb|baseTexmap",
}
# Video --OO--> Texture: resolve each texture node to its file stem.
tex_stem = {}        # texture uid -> file stem
for c in connections_elem.elems:
    if c.id == b"C" and len(c.props) >= 3 and c.props[0] == b"OO" \
            and c.props[1] in videos and c.props[2] in textures:
        tex_stem.setdefault(c.props[2], videos[c.props[1]])

mat_diffuse = {}     # material uid -> texture file stem
skipped_channels = {}
unresolved_diffuse = 0
for c in connections_elem.elems:
    if c.id != b"C" or len(c.props) < 3 or c.props[0] != b"OP":
        continue
    src, dst = c.props[1], c.props[2]
    if src not in textures or dst not in materials:
        continue
    channel = c.props[3].decode("utf-8", "replace") if len(c.props) > 3 else ""
    if channel in DIFFUSE_CHANNELS:
        if src in tex_stem:
            mat_diffuse.setdefault(dst, tex_stem[src])
        else:
            unresolved_diffuse += 1
    else:
        skipped_channels[channel] = skipped_channels.get(channel, 0) + 1

# Distinct FBX materials can share a display name; group the diffuse choice
# by name and track collisions (same name, different maps) for the report.
diffuse_by_name = {}
name_collisions = set()
for uid, tex_name in mat_diffuse.items():
    name = materials[uid]
    if diffuse_by_name.setdefault(name, tex_name) != tex_name:
        name_collisions.add(name)

print(
    f"apartment: FBX declares {len(materials)} materials, "
    f"{len(mat_diffuse)} with a diffuse map "
    f"({len(name_collisions)} name collisions, "
    f"{unresolved_diffuse} diffuse links without a file); "
    f"skipped channels: {skipped_channels}"
)

# ---- import the scene ----
bpy.ops.wm.read_factory_settings(use_empty=True)
print("apartment: importing FBX (several minutes)...", flush=True)
bpy.ops.import_scene.fbx(filepath=in_path)
meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
print(f"apartment: imported {len(meshes)} meshes", flush=True)

# The path-less texture nodes still materialize as broken image datablocks
# (named after the FBX texture element, no backing file) — purge them so the
# pack pass below can't trip over them. The wired images loaded from the
# extracted zip survive: their files exist.
for img in list(bpy.data.images):
    if img.source == "FILE" and img.packed_file is None \
            and not os.path.exists(bpy.path.abspath(img.filepath)):
        bpy.data.images.remove(img)

# ---- rebuild materials: Base Color from the connected diffuse map ----
loaded = {}  # texture stem -> bpy image


def load_tex(name):
    if name in loaded:
        return loaded[name]
    path = tex_files.get(name.lower())
    img = bpy.data.images.load(path) if path else None
    loaded[name] = img
    return img


wired = missing_file = 0
total_textured = 0
for mat in bpy.data.materials:
    # Importer dedup suffixes (.001) don't exist in the FBX name table.
    base_name = mat.name.rsplit(".", 1)[0] if mat.name[-4:-3] == "." else mat.name
    tex_name = diffuse_by_name.get(mat.name) or diffuse_by_name.get(base_name)
    if tex_name is None:
        continue
    total_textured += 1
    img = load_tex(tex_name)
    if img is None:
        missing_file += 1
        continue
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    if bsdf is None:
        continue
    tex_node = mat.node_tree.nodes.new("ShaderNodeTexImage")
    tex_node.image = img
    mat.node_tree.links.new(tex_node.outputs["Color"], bsdf.inputs["Base Color"])
    wired += 1

print(
    f"apartment: wired {wired}/{total_textured} texture-connected materials "
    f"({missing_file} maps missing from the zip)"
)
if total_textured == 0 or wired / total_textured < MIN_WIRED:
    raise SystemExit(
        f"apartment: only {wired}/{total_textured} materials wired "
        f"(< {MIN_WIRED:.0%}) — refusing to ship a gray house. Check that "
        f"the textures zip matches the FBX."
    )

# ---- bake the FBX importer's +90 deg X rotation (same fix as decimate.py) ----
bpy.ops.object.select_all(action="DESELECT")
for o in meshes:
    o.select_set(True)
bpy.context.view_layer.objects.active = meshes[0]
bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)

# ---- cap + pack textures so the exporter embeds the capped versions ----
for img in bpy.data.images:
    if img.source != "FILE":
        continue
    if max(img.size) > tex_cap:
        img.scale(tex_cap, tex_cap)
    img.pack()

total = 0
for o in meshes:
    o.data.calc_loop_triangles()
    total += len(o.data.loop_triangles)

print(f"apartment: exporting {total} tris (no decimation)...", flush=True)
bpy.ops.export_scene.gltf(filepath=out_path, export_format="GLB", export_apply=True)
size_mb = os.path.getsize(out_path) / 1e6
print(f"apartment: {total} tris, {wired} textured materials -> {out_path} ({size_mb:.0f} MB)")
