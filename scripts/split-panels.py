"""Split a multi-panel kit GLB into per-panel game-weight .glb files.

    blender --background --python scripts/split-panels.py -- <kit.glb> <out_dir> <target_tris>

The cgtrader "Sci-Fi Parts Kit" GLBs carry all panels as sibling mesh objects
with embedded PBR materials. For the panel worlds (B11) each panel becomes its
own .glb: textures downscaled once (they are shared), every object decimated
to roughly <target_tris>, exported selection-by-selection under a sanitized
stable name — the asset catalog references these names, so they must never
depend on kit ordering.
"""
import os
import re
import sys

import bpy

argv = sys.argv[sys.argv.index("--") + 1:]
kit_path, out_dir, target_tris = argv[0], argv[1], int(argv[2])

TEX_SIZE = 1024

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=kit_path)

# Shared textures: downscale once before any export embeds them.
for img in bpy.data.images:
    if img.size[0] > TEX_SIZE or img.size[1] > TEX_SIZE:
        img.scale(min(img.size[0], TEX_SIZE), min(img.size[1], TEX_SIZE))

meshes = [o for o in bpy.data.objects if o.type == "MESH"]
os.makedirs(out_dir, exist_ok=True)

for obj in sorted(meshes, key=lambda o: o.name):
    # Kit objects sit spread out in a showroom grid; each panel must export
    # origin-centered so a placement position IS the panel center.
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.origin_set(type="ORIGIN_GEOMETRY", center="BOUNDS")
    obj.location = (0.0, 0.0, 0.0)

    tris = sum(len(p.vertices) - 2 for p in obj.data.polygons)
    if tris > target_tris:
        mod = obj.modifiers.new("decimate", "DECIMATE")
        mod.ratio = target_tris / tris
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.modifier_apply(modifier=mod.name)

    # 'SF PP01_A_001' -> sf_pp01_a_001.glb (stable, name-derived).
    stem = re.sub(r"[^a-z0-9]+", "_", obj.name.lower()).strip("_")
    out_path = os.path.join(out_dir, f"{stem}.glb")

    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.ops.export_scene.gltf(
        filepath=out_path,
        use_selection=True,
        export_format="GLB",
    )
    remaining = sum(len(p.vertices) - 2 for p in obj.data.polygons)
    print(f"panel: {obj.name!r} {tris} -> ~{remaining} tris -> {out_path}")

print(f"panels: {len(meshes)} exported to {out_dir}")
