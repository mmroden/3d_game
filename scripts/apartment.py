"""Headless apartment-environment conversion for `make assets`, run via Blender.

    blender --background --python scripts/apartment.py -- \
        <in.fbx> <out.glb> <textures.zip> [tex_cap] [max_materials.json] \
        [windows_out.toml] [fbx_materials.json]

Converts the planet-3 apartment (a 3ds Max/Corona archviz export) into one
self-contained .glb at provider scale. This script is MECHANISM only: the
material POLICY lives in scripts/material_plan.py (pure Python, audited by
`make test-assets`), computed from the FBX material oracle
(extract-fbx-materials.py) and the .max material table
(extract-max-materials.py). Here we import the scene, apply the plans to
Blender materials, derive the window panes, cap textures, and export.

No decimation: this is a single environment instance, not a many-instances
enemy model (owner 2026-07-11). Textures are capped at [tex_cap]
(default 2048) — room-scale surfaces are seen close-up at 5x scale, so the
1024 prop budget doesn't apply, but uncapped archviz sources are a VRAM
problem.

The conversion FAILS if fewer than MIN_WIRED of the textured plans land —
a gray house must never ship silently.
"""
import json
import os
import sys
import zipfile

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from material_plan import build_plans, normalize_max_table  # noqa: E402

argv = sys.argv[sys.argv.index("--") + 1:]
in_path, out_path, zip_path = argv[0], argv[1], argv[2]
tex_cap = int(argv[3]) if len(argv) > 3 else 2048
mat_table_path = argv[4] if len(argv) > 4 else None
# The generated windows file (rosters/windows/<key>.toml): pane rects
# DERIVED from the scene's glass materials, consumed by the level grammar.
windows_out_path = argv[5] if len(argv) > 5 else None
fbx_table_path = argv[6] if len(argv) > 6 else None
PANE_EMISSION = 1.5  # glass glow strength (look knob; blinds silhouette)

MIN_WIRED = 0.80  # fraction of textured plans that must actually wire

# ---- extract the textures beside the FBX (idempotent) ----
tex_dir = os.path.join(os.path.dirname(in_path), "textures")
if not os.path.isdir(tex_dir):
    with zipfile.ZipFile(zip_path) as zf:
        zf.extractall(os.path.dirname(in_path))
    print(f"apartment: extracted {zip_path} -> {tex_dir}")

inventory = {f for f in os.listdir(tex_dir) if not f.startswith(".")}

# ---- the material plans: policy computed OUTSIDE Blender ----
if not fbx_table_path or not os.path.isfile(fbx_table_path):
    raise SystemExit(
        "apartment: fbx_materials.json missing — install-addons.sh runs "
        "extract-fbx-materials.py first; check the asset pipeline order.")
with open(fbx_table_path) as f:
    fbx_table = json.load(f)
max_table = {}
if mat_table_path and os.path.isfile(mat_table_path):
    with open(mat_table_path) as f:
        max_table = normalize_max_table(json.load(f))

plans = build_plans(fbx_table, max_table, inventory)
assigned = {n for n, r in fbx_table["materials"].items() if r["assigned"]}
glass_mats = {n for n in assigned if plans[n]["classification"] == "glass"}
n_textured_planned = sum(
    1 for n in assigned if "texture" in plans[n]["base_color"])
print(
    f"apartment: {len(plans)} material plans "
    f"({n_textured_planned} textured, {len(glass_mats)} glass, "
    f"{sum(1 for n in assigned if plans[n]['classification'] == 'light')} light, "
    f"{sum(1 for n in assigned if plans[n]['resolved_from'])} container-resolved)"
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

# Conservation diagnostic for the audit's glb test: assigned FBX materials
# Blender's importer failed to materialize (their surfaces are LOST, not
# just untextured) — named here so the loss is never silent.
imported_names = {m.name.rsplit(".", 1)[0] if m.name[-4:-3] == "." else m.name
                  for m in bpy.data.materials}
lost = sorted(assigned - imported_names)
if lost:
    print(f"apartment: WARNING {len(lost)} assigned materials not "
          f"materialized by the importer: {lost}")

# ---- apply the plans (mechanism only) ----
tex_files = {}
for f in inventory:
    stem, ext = os.path.splitext(f)
    if ext.lower() in (".jpg", ".jpeg", ".png", ".tif", ".tiff"):
        tex_files[f.lower()] = os.path.join(tex_dir, f)

loaded = {}


def load_tex(basename, non_color=False):
    key = basename.lower()
    if key in loaded:
        img = loaded[key]
    else:
        path = tex_files.get(key)
        img = bpy.data.images.load(path) if path else None
        if img is not None and max(img.size) > tex_cap:
            img.scale(tex_cap, tex_cap)  # cap before any pixel math
        loaded[key] = img
    if img is not None and non_color:
        img.colorspace_settings.name = "Non-Color"
    return img


def _pixel_op(img, new_name, fn):
    """A derived image from numpy pixel math, packed, Non-Color."""
    import numpy as np
    existing = bpy.data.images.get(new_name)
    if existing is not None:
        return existing
    w, h = img.size
    px = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    out_px = fn(px.reshape(h, w, 4))
    out = bpy.data.images.new(new_name, width=w, height=h)
    out.colorspace_settings.name = "Non-Color"
    out.pixels.foreach_set(out_px.astype(np.float32).ravel())
    out.pack()
    return out


def inverted(img):
    """Corona glossiness -> roughness: 1 - x."""
    def fn(px):
        px[:, :, :3] = 1.0 - px[:, :, :3]
        return px
    return _pixel_op(img, img.name + ".rough", fn)


def height_to_normal(img):
    """Corona bump maps are greyscale height; glTF speaks normal maps.
    Neutral slope here — the AUTHORED per-material bump amount rides the
    NormalMap node instead, so one derived image serves every material
    wearing the map, each at its own strength."""
    import numpy as np

    def fn(px):
        hmap = px[:, :, 0]
        gx = np.roll(hmap, -1, axis=1) - np.roll(hmap, 1, axis=1)
        gy = np.roll(hmap, -1, axis=0) - np.roll(hmap, 1, axis=0)
        nz = np.ones_like(gx)
        length = np.sqrt(gx * gx + gy * gy + nz * nz)
        return np.stack(
            [(-gx / length + 1) / 2, (-gy / length + 1) / 2,
             (nz / length + 1) / 2, np.ones_like(gx)], axis=-1)
    return _pixel_op(img, img.name + ".normal", fn)


def image_node(mat, img):
    node = mat.node_tree.nodes.new("ShaderNodeTexImage")
    node.image = img
    return node


applied = wired = missing_file = 0
for mat in bpy.data.materials:
    base_name = mat.name.rsplit(".", 1)[0] if mat.name[-4:-3] == "." else mat.name
    plan = plans.get(mat.name) or plans.get(base_name)
    if plan is None:
        print(f"apartment: WARNING no plan for material '{mat.name}'")
        continue
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    if bsdf is None:
        continue
    applied += 1

    # The plan has FULL authority over these sockets: whatever the FBX
    # importer wired (junk transparency, strength-1000 emission) is
    # overridden by plan-or-neutral, never merely supplemented.
    for link in list(mat.node_tree.links):
        if link.to_node == bsdf and link.to_socket.name in (
                "Base Color", "Alpha", "Roughness", "Normal",
                "Emission Color", "Emission Strength"):
            mat.node_tree.links.remove(link)

    base = plan["base_color"]
    if "texture" in base:
        img = load_tex(base["texture"])
        if img is not None:
            node = image_node(mat, img)
            mat.node_tree.links.new(node.outputs["Color"],
                                    bsdf.inputs["Base Color"])
            wired += 1
        else:
            missing_file += 1
    else:
        c = base["color"]
        bsdf.inputs["Base Color"].default_value = (c[0], c[1], c[2], 1.0)

    normal = plan.get("normal")
    if normal is not None:
        img = load_tex(normal["texture"], non_color=True)
        if img is not None:
            if normal["kind"] == "height":
                img = height_to_normal(img)
            node = image_node(mat, img)
            nmap = mat.node_tree.nodes.new("ShaderNodeNormalMap")
            # The authored Corona bump amount, exported as normalTexture
            # scale — never a guessed constant.
            nmap.inputs["Strength"].default_value = normal.get("strength", 1.0)
            mat.node_tree.links.new(node.outputs["Color"], nmap.inputs["Color"])
            mat.node_tree.links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])

    rough = plan.get("roughness")
    if rough is not None:
        if "texture" in rough:
            img = load_tex(rough["texture"], non_color=True)
            if img is not None:
                if rough.get("invert"):
                    img = inverted(img)
                node = image_node(mat, img)
                mat.node_tree.links.new(node.outputs["Color"],
                                        bsdf.inputs["Roughness"])
        else:
            bsdf.inputs["Roughness"].default_value = max(
                0.0, min(1.0, rough["value"]))

    alpha = plan.get("alpha")
    if alpha is None:
        bsdf.inputs["Alpha"].default_value = 1.0
        mat.blend_method = "OPAQUE"
    elif "cutout_texture" in alpha:
        img = load_tex(alpha["cutout_texture"], non_color=True)
        if img is not None:
            node = image_node(mat, img)
            mat.node_tree.links.new(node.outputs["Color"], bsdf.inputs["Alpha"])
            mat.blend_method = "CLIP"
            mat.alpha_threshold = 0.5
    else:
        bsdf.inputs["Alpha"].default_value = alpha["value"]
        mat.blend_method = "BLEND"

    emission = plan.get("emission")
    if emission is None:
        bsdf.inputs["Emission Strength"].default_value = 0.0
    else:
        if emission.get("texture"):
            img = load_tex(emission["texture"])
            if img is not None:
                node = image_node(mat, img)
                mat.node_tree.links.new(node.outputs["Color"],
                                        bsdf.inputs["Emission Color"])
        else:
            c = emission["color"]
            bsdf.inputs["Emission Color"].default_value = (c[0], c[1], c[2], 1.0)
        bsdf.inputs["Emission Strength"].default_value = emission["strength"]

print(
    f"apartment: applied {applied} plans, wired {wired}/{n_textured_planned} "
    f"textured ({missing_file} maps missing from the zip)"
)
if n_textured_planned and wired / n_textured_planned < MIN_WIRED:
    raise SystemExit(
        f"apartment: only {wired}/{n_textured_planned} textured plans wired "
        f"(< {MIN_WIRED:.0%}) — refusing to ship a gray house. Check that "
        f"the textures zip matches the FBX.")

# ---- windows, DERIVED from the scene: the panes identify themselves
# (glass materials + big thin vertical sheets). Two outputs: (a) the
# generated windows file the level grammar consumes (probe-style:
# derived, committed, never hand-edited) — pane light energy scales with
# pane area from one authored knob; (b) glass-only emission, so the pane
# glows while blinds/sheers in front keep their silhouette (owner
# 2026-07-12: full-covering emission erased the blinds).
from mathutils import Vector  # noqa: E402

panes = []
for o in meshes:
    if not any((m is not None and m.name.split(".")[0] in glass_mats)
               for m in o.data.materials):
        continue
    corners = [o.matrix_world @ Vector(c) for c in o.bound_box]
    lo = [min(p[i] for p in corners) for i in range(3)]
    hi = [max(p[i] for p in corners) for i in range(3)]
    ext = [hi[i] - lo[i] for i in range(3)]
    thin = min(range(2), key=lambda i: ext[i])  # thin axis is horizontal
    if ext[thin] > 0.15:
        continue
    span, height = ext[1 - thin], ext[2]
    if span * height < 0.8 or height < 0.5:
        continue  # decor glass, not a window
    panes.append((o, lo, hi, thin))

if panes:
    all_lo = [min(p[1][i] for p in panes) for i in range(2)]
    all_hi = [max(p[2][i] for p in panes) for i in range(2)]


def emit_windows_toml(path):
    """Write the generated [[window]] set, glb-space (x, y-up, z=-blender-y)."""
    lines = [
        "# GENERATED by scripts/apartment.py (make assets) — do not edit.",
        "# Window panes DERIVED from the scene's glass materials: centers,",
        "# sizes, and facings are the model's own; light energy scales with",
        "# pane area via the environment's authored window_energy_per_m2.",
        "",
        f'environment = "{os.path.splitext(os.path.basename(path))[0]}"',
        "",
    ]
    # Interior center for inward facing: the panes' own bounding centroid.
    cx = (all_lo[0] + all_hi[0]) / 2.0
    cy = (all_lo[1] + all_hi[1]) / 2.0
    for o, lo, hi, thin in panes:
        center_b = [(lo[i] + hi[i]) / 2.0 for i in range(3)]
        span = (hi[1 - thin] - lo[1 - thin])
        height = hi[2] - lo[2]
        inward_positive = (cx, cy)[thin] > center_b[thin]
        if thin == 0:
            facing = "pos_x" if inward_positive else "neg_x"
        else:
            # blender +y = glb -z: inward toward +y means glb neg_z.
            facing = "neg_z" if inward_positive else "pos_z"
        gx, gy, gz = center_b[0], center_b[2], -center_b[1]
        lines.append("[[window]]")
        lines.append(f"center = [{gx:.2f}, {gy:.2f}, {gz:.2f}]")
        lines.append(f"size = [{span:.2f}, {height:.2f}]")
        lines.append(f'facing = "{facing}"')
        lines.append("")
    with open(path, "w") as f:
        f.write("\n".join(lines))


if windows_out_path and panes:
    emit_windows_toml(windows_out_path)
    print(f"apartment: {len(panes)} window panes derived -> {windows_out_path}")

# Glass-only glow: the pane radiates, everything in front silhouettes.
glowed = set()
for o, lo, hi, thin in panes:
    for m in o.data.materials:
        if m is None or m.name in glowed or not m.use_nodes:
            continue
        bsdf = m.node_tree.nodes.get("Principled BSDF")
        if bsdf is None:
            continue
        bsdf.inputs["Emission Color"].default_value = (1.0, 0.98, 0.94, 1.0)
        bsdf.inputs["Emission Strength"].default_value = PANE_EMISSION
        glowed.add(m.name)
print(f"apartment: {len(glowed)} glass pane materials set emissive")

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
