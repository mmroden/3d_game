"""Headless cockpit-shell extraction for `make assets`, run via Blender.

    blender --background --python scripts/extract-cockpit.py -- \\
        <hull.glb> <out.glb>

APPLIES the plan derived by scripts/cockpit_plan.py (the pure-Python
reader; see its module doc for the rule): imports the hull, deletes every
mesh part the plan drops (canopy PANES included — the canopy anchors the
plan but its glass never ships), mattes the surviving materials, links an
"Eyepoint" empty at the planned pilot eye position, and re-exports a
self-contained .glb with the surviving parts at FULL detail — the shell
sits close to the camera, so it is the one model that must never be
decimated. Textures stay embedded at source resolution for the same
reason.

The pytest audit (`make test-assets`) holds the built shell to the plan;
this script computes nothing of its own.
"""
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cockpit_plan import ROUGHNESS_FLOOR, parse_glb, plan_cockpit  # noqa: E402

argv = sys.argv[sys.argv.index("--") + 1:]
in_path, out_path = argv[0], argv[1]

plan = plan_cockpit(parse_glb(in_path))
keep = set(plan["keep"])

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=in_path)


def stem(name):
    """Blender's dedup suffix (\".001\") stripped — the importer parks each
    glTF node's name on an EMPTY and gives the mesh child the suffixed
    name, so the part's identity is the suffix-free stem (with the mesh
    datablock name as the fallback witness)."""
    base, dot, suffix = name.rpartition(".")
    return base if dot and suffix.isdigit() else name


dropped = 0
for o in list(bpy.context.scene.objects):
    if o.type == "MESH" and stem(o.name) not in keep and o.data.name not in keep:
        bpy.data.objects.remove(o, do_unlink=True)
        dropped += 1

# Flatten: the surviving meshes take their world transforms, lose their
# importer-empty parents, and get their clean part names back (only
# possible once the name-hogging empties are gone), so the export carries
# exactly the planned parts as top-level named nodes.
survivors = [o for o in bpy.context.scene.objects if o.type == "MESH"]
for o in survivors:
    world = o.matrix_world.copy()
    o.parent = None
    o.matrix_world = world
for o in list(bpy.context.scene.objects):
    if o.type != "MESH":
        bpy.data.objects.remove(o, do_unlink=True)
for o in survivors:
    o.name = stem(o.name) if stem(o.name) in keep else o.data.name

if len(survivors) != len(keep):
    raise SystemExit(
        f"extract-cockpit: plan keeps {len(keep)} parts but {len(survivors)} "
        f"survived import matching — importer naming drifted, refusing to "
        f"ship a partial shell"
    )

# Matte the interior. Mirror-glossy furniture centimeters from the eyes
# shimmers against every pose (the rig's pose-invariance contract caught
# it, 2026-08-19), so the roughness is floored, the metal-roughness maps
# dropped, and metallic zeroed. The plan ships NO blend materials (the
# panes flared cache glows — owner, 2026-08-20), so every survivor takes
# the matte pass; a blend survivor here is a plan violation.
matted = 0
materials = {slot.material
             for o in survivors for slot in o.material_slots if slot.material}
for mat in materials:
    if mat.blend_method == 'BLEND':
        raise SystemExit(
            f"extract-cockpit: blend material {mat.name!r} survived a plan "
            f"that ships no glass — keep rule and artifact disagree"
        )
    if not mat.use_nodes:
        continue
    bsdf = next((n for n in mat.node_tree.nodes
                 if n.type == 'BSDF_PRINCIPLED'), None)
    if bsdf is None:
        continue
    for input_name in ("Roughness", "Metallic"):
        socket = bsdf.inputs[input_name]
        for link in list(socket.links):
            mat.node_tree.links.remove(link)
    rough = bsdf.inputs["Roughness"]
    rough.default_value = max(rough.default_value, ROUGHNESS_FLOOR)
    bsdf.inputs["Metallic"].default_value = 0.0
    matted += 1

# The pilot's eye position, as a plain empty the runtime can look up by
# name. The plan speaks glTF world space (Y up); Blender's scene is Z up,
# so map (x, y, z) -> (x, -z, y) here and let the exporter's inverse
# conversion restore the glTF coordinates on the way out.
gx, gy, gz = plan["eyepoint"]
eyepoint = bpy.data.objects.new("Eyepoint", None)
eyepoint.location = (gx, -gz, gy)
bpy.context.scene.collection.objects.link(eyepoint)

bpy.ops.export_scene.gltf(filepath=out_path, export_format="GLB")

print(
    f"extract-cockpit: {len(survivors)} parts survive (canopy {plan['canopy']} "
    f"anchors, panes dropped), dropped {dropped}, matted {matted} materials, "
    f"eyepoint ({gx:.3f}, {gy:.3f}, {gz:.3f}) -> {out_path}"
)
