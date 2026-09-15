"""Recover a scene's material->texture wiring from its .max source, run
via Blender:

    blender --background --python-exit-code 1 \\
        --python scripts/extract-max-materials.py -- <in.max> <out.json>

An archviz provider's FBX export destroys most Corona material bindings
(FBX has no representation for them); the .max scene is the authoring
truth. The io_scene_max extension (installed by `make deps` through
Blender's extension system) parses the scene INCLUDING Corona materials.
Its mesh/UV import is documented as unreliable, so this script uses it
purely as a MATERIAL MANIFEST: import, walk the node trees, dump one
JSON table of material name -> per-channel texture basenames + base
color. material_plan.py merges the table over the FBX-derived recovery
(the .max wins) for convert-environment.py.
"""
import json
import os
import sys

import addon_utils
import bpy

argv = sys.argv[sys.argv.index("--") + 1:]
in_path, out_path = argv[0], argv[1]

EXTENSION = "bl_ext.blender_org.io_scene_max"

# Factory settings FIRST — it resets session addons, so enabling the
# extension must come after (installed by `make deps` via Blender's
# extension system; enabling here keeps the run self-contained).
bpy.ops.wm.read_factory_settings(use_empty=True)
enabled, _ = addon_utils.check(EXTENSION)
if not enabled:
    addon_utils.enable(EXTENSION, default_set=False)
print("extract-max: importing scene (several minutes)...", flush=True)
bpy.ops.import_scene.max(filepath=in_path)
print(f"extract-max: {len(bpy.data.materials)} materials imported", flush=True)


def channel_of(node, bsdf):
    """Which BSDF input a texture node ultimately feeds, if any."""
    for link in node.id_data.links:
        if link.from_node != node:
            continue
        if link.to_node == bsdf:
            return link.to_socket.name
        # One indirection (Normal Map / separate/mix nodes).
        for link2 in node.id_data.links:
            if link2.from_node == link.to_node and link2.to_node == bsdf:
                return link2.to_socket.name
    return None


table = {}
for mat in bpy.data.materials:
    if not mat.use_nodes:
        continue
    bsdf = next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
    entry = {"channels": {}, "color": None}
    if bsdf is not None:
        c = bsdf.inputs["Base Color"].default_value
        entry["color"] = [round(c[0], 4), round(c[1], 4), round(c[2], 4)]
    for node in mat.node_tree.nodes:
        if node.type != "TEX_IMAGE" or node.image is None:
            continue
        path = node.image.filepath or node.image.name
        basename = os.path.basename(path.replace("\\", "/"))
        channel = (channel_of(node, bsdf) or "unlinked") if bsdf else "unlinked"
        entry["channels"].setdefault(channel, basename)
    if entry["channels"] or entry["color"] is not None:
        table[mat.name] = entry

with_tex = sum(1 for e in table.values() if e["channels"])
print(f"extract-max: {len(table)} materials tabled, {with_tex} with textures", flush=True)
with open(out_path, "w") as f:
    json.dump(table, f, indent=1, sort_keys=True)
print(f"extract-max: wrote {out_path}", flush=True)
