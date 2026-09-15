"""Headless player-hull conversion for `make assets`, run via Blender.

    blender --background --python scripts/convert-hull.py -- \\
        <in.fbx> <out.glb> <tex_dir> <fbx_materials.json> <facing_deg> [tex_cap]

Converts a provider hull FBX into the self-contained .glb the ship roster
installs (godot/addons/ships/<hull>.glb) and the cockpit extraction
(extract-cockpit.py) reads. MECHANISM only: the material policy is
scripts/material_plan.py, computed from the FBX material manifest
(extract-fbx-materials.py) exactly as for the apartment, and wired by
scripts/plan_apply.py. Here: import, plan, apply, put the hull in the
roster frame, cap + pack textures, export.

Roster frame: installed hull models face +Z (the cgtrader convention;
ShipSpec's model_yaw_offset = PI turns every hull — and its cockpit
shell — to Godot's -Z forward), Y up, meters. That is the frame the
cockpit rule reads (aft = min z) and the runtime's shell placement
relies on. <facing_deg> is the yaw that takes THIS provider's authored
nose direction to +Z: a per-pack fact the install script records with
its provenance (0 when the provider already models the nose along +Z).
The FBX importer's +90 deg X rotation and centimeter scale are baked into
the mesh data, as decimate.py does.

Static: the game plays no provider animation (landing gear, canopy
hinges), and an animated object exports its keyed transform ON TOP of
the baked mesh — the military ship's gear and canopy frame shipped as
centimeter-sized boxes at the wrong place (2026-09-06) — so every action
is stripped before the bake and nothing animates in the export.

Full detail, no decimation: one instance flies, and the cockpit shell
extracted from this file must never be decimated (near-field). Textures
are capped at [tex_cap] (default 2048 — the resolution every other hull
ships at).

The conversion FAILS if any textured plan fails to wire — a gray hull
must never ship silently.
"""
import json
import math
import os
import sys

import bpy
from mathutils import Matrix

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from material_plan import build_plans, shipped_inventory  # noqa: E402
from plan_apply import apply_plans, cap_and_pack_images  # noqa: E402

argv = sys.argv[sys.argv.index("--") + 1:]
in_path, out_path, tex_dir, fbx_table_path = argv[0], argv[1], argv[2], argv[3]
facing_deg = float(argv[4])
tex_cap = int(argv[5]) if len(argv) > 5 else 2048

# ---- the material plans: policy computed OUTSIDE Blender ----
if not os.path.isfile(fbx_table_path):
    raise SystemExit(
        "convert-hull: fbx_materials.json missing — install-addons.sh runs "
        "extract-fbx-materials.py first; check the asset pipeline order.")
with open(fbx_table_path) as f:
    fbx_table = json.load(f)
inventory = shipped_inventory(fbx_table, tex_dir)
plans = build_plans(fbx_table, {}, inventory)
assigned = {n for n, r in fbx_table["materials"].items() if r["assigned"]}
n_textured_planned = sum(
    1 for n in assigned if "texture" in plans[n]["base_color"])
n_glass = sum(1 for n in assigned if plans[n]["classification"] == "glass")
print(
    f"convert-hull: {len(plans)} material plans "
    f"({n_textured_planned} textured, {n_glass} glass) from "
    f"{len(inventory)} shipped textures"
)

# ---- import ----
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.fbx(filepath=in_path)
# Path-less texture nodes materialize as broken image datablocks (named
# after the FBX texture element, no backing file) — purge them so the
# pack pass can't trip over them. Images the FBX embeds are packed and
# survive; so do loose ones whose files exist.
for img in list(bpy.data.images):
    if img.source == "FILE" and img.packed_file is None \
            and not os.path.exists(bpy.path.abspath(img.filepath)):
        bpy.data.images.remove(img)
meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
if not meshes:
    raise SystemExit(f"convert-hull: {in_path} imported no meshes")

# ---- static: strip the provider's animations before any transform bake ----
stripped = 0
for o in bpy.context.scene.objects:
    if o.animation_data is not None:
        o.animation_data_clear()
        stripped += 1
for action in list(bpy.data.actions):
    bpy.data.actions.remove(action)

# ---- apply the plans (mechanism only) ----
applied, wired, missing_file = apply_plans(plans, tex_dir, tex_cap)
print(
    f"convert-hull: applied {applied} plans, wired {wired}/{n_textured_planned} "
    f"textured ({missing_file} maps missing from the shipped textures)"
)
if wired < n_textured_planned:
    raise SystemExit(
        f"convert-hull: only {wired}/{n_textured_planned} textured plans "
        f"wired — refusing to ship a gray hull. Check that the textures "
        f"archive and the FBX's embedded maps cover the plan.")

# ---- roster frame: bake the importer's +90 deg X rotation and cm scale
# into the mesh data (as decimate.py does), with the provider's nose
# swung to +Z about the world up axis in the same bake ----
facing = Matrix.Rotation(math.radians(facing_deg), 4, "Z")
for o in bpy.context.scene.objects:
    if o.parent is None:
        o.matrix_world = facing @ o.matrix_world
bpy.ops.object.select_all(action="DESELECT")
for o in bpy.context.scene.objects:
    o.select_set(True)
bpy.context.view_layer.objects.active = meshes[0]
bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)

# ---- cap + pack textures so the exporter embeds the capped versions ----
cap_and_pack_images(tex_cap)

total = 0
for o in meshes:
    o.data.calc_loop_triangles()
    total += len(o.data.loop_triangles)

# No vertex colors: no plan reads them, and the exporter warns per mesh
# about an active color layer its material never uses.
bpy.ops.export_scene.gltf(
    filepath=out_path, export_format="GLB", export_apply=True,
    export_animations=False, export_vertex_color="NONE",
)
size_mb = os.path.getsize(out_path) / 1e6
print(
    f"convert-hull: {total} tris kept (no decimation), {len(meshes)} parts, "
    f"{wired} textured materials, {stripped} animated objects made static, "
    f"facing {facing_deg:g} deg -> {out_path} ({size_mb:.0f} MB)"
)
