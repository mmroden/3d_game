"""Headless fixed-environment conversion for `make assets`, run via Blender.

    blender --background --python scripts/convert-environment.py -- \\
        <model> <out.glb> <tex_root> <manifest.json> \\
        [--max max_materials.json] [--windows out.toml] \\
        [--cache scene_cache.blend] [--tex-cap N] [--report conversion.json] \\
        [--unit-scale S] [--obj-up Y|Z] [--obj-forward AXIS] \\
        [--keep lo_x,lo_y,lo_z,hi_x,hi_y,hi_z]

One door for every planet-3 scene (the apartment, the hill house, the
office building, the mountain villa): an archviz export — FBX from 3ds
Max/Corona or Blender, or OBJ + MTL from SketchUp — becomes one
self-contained .glb at provider scale in meters. This script is
MECHANISM only: the material POLICY lives in scripts/material_plan.py
(pure Python, audited by `make test-assets`), computed from the model's
material manifest (extract-fbx-materials.py or mtl_materials.py — one
JSON shape) and, when the pack ships a .max, its material table
(extract-max-materials.py); scripts/plan_apply.py wires the plans
(shared with the hull conversion). Here we import the scene by format,
apply the plans, derive the window panes, cap textures, and export.

Per-pack provider facts, recorded by the install script with their
provenance (the metrics name them):
  --unit-scale   the factor that makes the file's units meters (the hill
                 house's OBJ is millimeters: 0.001);
  --obj-up / --obj-forward   an OBJ's authored axes (SketchUp's is Z up;
                 the importer assumes Y up and lays the room on its side);
  --keep         a box in the glTF frame (Y up, meters — the metrics'):
                 faces whose center falls outside are clipped, objects
                 left empty are dropped. Selects one scene when the
                 provider laid several side by side (the office's four
                 color schemes), and cuts a backdrop away from a room.
The imported scene's bounds are logged in that same frame right after
import — the numbers a keep box is read from.
--report writes what --keep did (objects dropped, faces clipped, and the
materials no surviving face wears) so the audit can subtract it.

No decimation: a scene is a single environment instance, not a
many-instances enemy model (owner 2026-07-11). Textures are capped at
--tex-cap (default 2048) — room-scale surfaces are seen close-up at kit
scale, so the 1024 prop budget doesn't apply, but uncapped archviz
sources are a VRAM problem.

--cache: the multi-minute import happens once per model change; later
runs open the virgin post-import scene in seconds (derived, gitignored).
The cache is keyed on the model's mtime AND the import options (unit
scale, axes) through a sidecar — changing either re-imports. --windows:
the generated windows file (rosters/windows/<key>.toml) — pane rects
DERIVED from the scene's glass materials, consumed by the level grammar
for the authored environment of that key (the install script asks for it
only once the environment roster exists). It is always written when
asked for, empty when no pane identifies itself, so the pipeline's
freshness has a file to hold.

The conversion FAILS if fewer than MIN_WIRED of the textured plans land —
a gray scene must never ship silently.
"""
import argparse
import json
import os
import sys

import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from material_plan import (  # noqa: E402
    build_plans, loose_matches, normalize_max_table, shipped_inventory,
)
from plan_apply import apply_plans, cap_and_pack_images  # noqa: E402

parser = argparse.ArgumentParser(prog="convert-environment.py")
parser.add_argument("model")
parser.add_argument("out")
parser.add_argument("tex_root")
parser.add_argument("manifest")
parser.add_argument("--max", dest="max_table", default=None)
parser.add_argument("--windows", default=None)
parser.add_argument("--cache", default=None)
parser.add_argument("--report", default=None)
parser.add_argument("--tex-cap", dest="tex_cap", type=int, default=2048)
parser.add_argument("--unit-scale", dest="unit_scale", type=float, default=1.0)
parser.add_argument("--obj-up", dest="obj_up", default="Y", choices=["Y", "Z"])
parser.add_argument("--obj-forward", dest="obj_forward", default="NEGATIVE_Z",
                    choices=["X", "Y", "Z", "NEGATIVE_X", "NEGATIVE_Y", "NEGATIVE_Z"])
parser.add_argument("--keep", default=None,
                    help="lo_x,lo_y,lo_z,hi_x,hi_y,hi_z in the glTF frame (Y up)")
args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:])

in_path, out_path, tex_root, tex_cap = args.model, args.out, args.tex_root, args.tex_cap
PANE_EMISSION = 1.5  # glass glow strength (look knob; blinds silhouette)
MIN_WIRED = 0.80  # fraction of textured plans that must actually wire

# ---- the material plans: policy computed OUTSIDE Blender ----
if not os.path.isfile(args.manifest):
    raise SystemExit(
        f"convert-environment: material manifest {args.manifest} missing — "
        "install-addons.sh extracts the manifest first; check the pipeline order.")
with open(args.manifest) as f:
    manifest = json.load(f)
max_table = {}
if args.max_table and os.path.isfile(args.max_table):
    with open(args.max_table) as f:
        max_table = normalize_max_table(json.load(f))

inventory = shipped_inventory(manifest, tex_root)
plans = build_plans(manifest, max_table, inventory)
assigned = {n for n, r in manifest["materials"].items() if r["assigned"]}
glass_mats = {n for n in assigned if plans[n]["classification"] == "glass"}
n_textured_planned = sum(
    1 for n in assigned if "texture" in plans[n]["base_color"])
print(
    f"convert-environment: {len(plans)} material plans "
    f"({n_textured_planned} textured, {len(glass_mats)} glass, "
    f"{sum(1 for n in assigned if plans[n]['classification'] == 'light')} light, "
    f"{sum(1 for n in assigned if plans[n]['resolved_from'])} container-resolved) "
    f"from {len(inventory)} shipped textures"
)
# What the relaxed name matching decided, pair by pair — a run artifact
# for review, never a silent substitution.
relaxed = loose_matches(manifest, inventory)
print(f"convert-environment: {len(relaxed)} texture references matched by relaxed name")
for name, chan, declared, shipped in relaxed:
    print(f"convert-environment:   {name} {chan}: {declared} -> {shipped}")


def import_model(path):
    ext = os.path.splitext(path)[1].lower()
    if ext == ".fbx":
        bpy.ops.import_scene.fbx(filepath=path, global_scale=args.unit_scale)
    elif ext == ".obj":
        bpy.ops.wm.obj_import(filepath=path, global_scale=args.unit_scale,
                              up_axis=args.obj_up, forward_axis=args.obj_forward)
    else:
        raise SystemExit(f"convert-environment: unsupported model format '{ext}' ({path})")


# ---- import the scene (cached; the cache is keyed on the model's mtime
# and the import options through a sidecar) ----
cache = args.cache
cache_key = {"model_mtime": os.path.getmtime(in_path), "unit_scale": args.unit_scale,
             "obj_up": args.obj_up, "obj_forward": args.obj_forward}
cache_sidecar = f"{cache}.json" if cache else None
cache_valid = False
if cache and os.path.isfile(cache) and os.path.isfile(cache_sidecar):
    with open(cache_sidecar) as f:
        cache_valid = json.load(f) == cache_key
if cache_valid:
    print("convert-environment: loading cached scene...", flush=True)
    bpy.ops.wm.open_mainfile(filepath=cache)
else:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    print(f"convert-environment: importing model (may take minutes; unit scale "
          f"{args.unit_scale:g})...", flush=True)
    import_model(in_path)
    # Path-less texture nodes materialize as broken image datablocks
    # (named after the source's texture element, no backing file) — purge
    # them so the pack pass below can't trip over them. Images that exist
    # on disk, and the ones the model embeds (packed), survive.
    for img in list(bpy.data.images):
        if img.source == "FILE" and img.packed_file is None \
                and not os.path.exists(bpy.path.abspath(img.filepath)):
            bpy.data.images.remove(img)
    if cache:
        bpy.ops.wm.save_as_mainfile(filepath=cache)
        with open(cache_sidecar, "w") as f:
            json.dump(cache_key, f)
meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
if not meshes:
    raise SystemExit(f"convert-environment: {in_path} imported no meshes")
print(f"convert-environment: scene ready, {len(meshes)} meshes", flush=True)


def world_aabb(o):
    corners = [o.matrix_world @ Vector(c) for c in o.bound_box]
    return ([min(p[i] for p in corners) for i in range(3)],
            [max(p[i] for p in corners) for i in range(3)])


def gltf_bounds(objects):
    """The objects' joint AABB in the glTF frame (x, y up, z) from
    Blender's (x, y, z up): glTF (x, y, z) = Blender (x, z, -y)."""
    lo = [float("inf")] * 3
    hi = [float("-inf")] * 3
    for o in objects:
        blo, bhi = world_aabb(o)
        glo = (blo[0], blo[2], -bhi[1])
        ghi = (bhi[0], bhi[2], -blo[1])
        lo = [min(lo[k], glo[k]) for k in range(3)]
        hi = [max(hi[k], ghi[k]) for k in range(3)]
    return lo, hi


def fmt(v):
    return "[" + ", ".join(f"{x:.3f}" for x in v) + "]"


lo, hi = gltf_bounds(meshes)
print(f"convert-environment: imported bounds (glTF frame, meters) lo={fmt(lo)} hi={fmt(hi)}")


def single_user(objects):
    """Give every object its own mesh data — a Blender-exported scene
    instances its repeated pieces (the office building's 5,126 objects
    share mesh data), and both face clipping and the transform bake need
    per-object data."""
    bpy.ops.object.select_all(action="DESELECT")
    for o in objects:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.object.make_single_user(type="SELECTED_OBJECTS", object=True, obdata=True)


def materials_in_use(objects):
    used = set()
    for o in objects:
        for poly in o.data.polygons:
            slots = o.material_slots
            if slots:
                m = slots[min(poly.material_index, len(slots) - 1)].material
                if m is not None:
                    used.add(m.name.rsplit(".", 1)[0] if m.name[-4:-3] == "." else m.name)
    return used


# ---- --keep: clip to the pick box. The box speaks the glTF frame (Y up,
# the metrics'); Blender's world is Z up with glTF (x, y, z) =
# Blender (x, z, -y). Objects wholly outside go, objects straddling the
# box lose the faces whose center is outside, objects left empty go ----
report = {"kept_objects": len(meshes), "dropped_objects": 0, "clipped_faces": 0,
          "dropped_materials": [], "imported_bounds": {"lo": lo, "hi": hi}}
if args.keep:
    import bmesh
    box = [float(v) for v in args.keep.split(",")]
    if len(box) != 6:
        raise SystemExit("convert-environment: --keep wants 6 comma-separated numbers")

    def inside(p):  # a Blender world point
        gx, gy, gz = p.x, p.z, -p.y
        return (box[0] <= gx <= box[3] and box[1] <= gy <= box[4]
                and box[2] <= gz <= box[5])

    before = materials_in_use(meshes)
    single_user(meshes)
    dropped = clipped = 0
    for o in list(meshes):
        blo, bhi = world_aabb(o)
        corners = [Vector((x, y, z)) for x in (blo[0], bhi[0]) for y in (blo[1], bhi[1])
                   for z in (blo[2], bhi[2])]
        flags = [inside(c) for c in corners]
        if all(flags):
            continue
        # glTF-frame overlap test: no overlap at all -> the whole object goes.
        g_lo = (blo[0], blo[2], -bhi[1])
        g_hi = (bhi[0], bhi[2], -blo[1])
        overlaps = all(g_lo[k] <= box[k + 3] and g_hi[k] >= box[k] for k in range(3))
        if not overlaps:
            bpy.data.objects.remove(o, do_unlink=True)
            dropped += 1
            continue
        bm = bmesh.new()
        bm.from_mesh(o.data)
        mw = o.matrix_world
        gone = [f for f in bm.faces if not inside(mw @ f.calc_center_median())]
        if gone:
            bmesh.ops.delete(bm, geom=gone, context="FACES")
            clipped += len(gone)
        empty = len(bm.faces) == 0
        bm.to_mesh(o.data)
        bm.free()
        if empty:
            bpy.data.objects.remove(o, do_unlink=True)
            dropped += 1
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    after = materials_in_use(meshes)
    report.update({"kept_objects": len(meshes), "dropped_objects": dropped,
                   "clipped_faces": clipped,
                   "dropped_materials": sorted(before - after)})
    print(f"convert-environment: --keep box {box}: kept {len(meshes)} objects, "
          f"dropped {dropped}, clipped {clipped} faces, "
          f"{len(report['dropped_materials'])} materials left with no face")
    if not meshes:
        raise SystemExit("convert-environment: --keep box kept nothing")
    lo, hi = gltf_bounds(meshes)
    print(f"convert-environment: kept bounds (glTF frame, meters) lo={fmt(lo)} hi={fmt(hi)}")
if args.report:
    with open(args.report, "w") as f:
        json.dump(report, f, indent=1, sort_keys=True)

# Conservation diagnostic for the audit's glb test: assigned materials
# the importer failed to materialize (their surfaces are LOST, not just
# untextured) — named here so the loss is never silent.
imported_names = {m.name.rsplit(".", 1)[0] if m.name[-4:-3] == "." else m.name
                  for m in bpy.data.materials}
lost = sorted(assigned - imported_names)
if lost:
    print(f"convert-environment: WARNING {len(lost)} assigned materials not "
          f"materialized by the importer: {lost[:20]}")

# ---- apply the plans (mechanism only) ----
applied, wired, missing_file = apply_plans(plans, tex_root, tex_cap)
print(
    f"convert-environment: applied {applied} plans, wired {wired}/{n_textured_planned} "
    f"textured ({missing_file} maps missing from the shipped textures)"
)
if n_textured_planned and wired / n_textured_planned < MIN_WIRED:
    raise SystemExit(
        f"convert-environment: only {wired}/{n_textured_planned} textured plans "
        f"wired (< {MIN_WIRED:.0%}) — refusing to ship a gray scene. Check that "
        f"the texture archives match the model.")

# ---- windows, DERIVED from the scene: the panes identify themselves
# (glass materials + big thin vertical sheets). Two outputs: (a) the
# generated windows file the level grammar consumes (probe-style:
# derived, committed, never hand-edited) — pane light energy scales with
# pane area from one authored knob; (b) glass-only emission, so the pane
# glows while blinds/sheers in front keep their silhouette (owner
# 2026-07-12: full-covering emission erased the blinds).
panes = []
for o in meshes:
    if not any((m is not None and m.name.split(".")[0] in glass_mats)
               for m in o.data.materials):
        continue
    blo, bhi = world_aabb(o)
    ext = [bhi[i] - blo[i] for i in range(3)]
    thin = min(range(2), key=lambda i: ext[i])  # thin axis is horizontal
    if ext[thin] > 0.15:
        continue
    span, height = ext[1 - thin], ext[2]
    if span * height < 0.8 or height < 0.5:
        continue  # decor glass, not a window
    panes.append((o, blo, bhi, thin))


def emit_windows_toml(path):
    """Write the generated [[window]] set, glb-space (x, y-up, z=-blender-y).
    Empty when no pane identified itself — the file still exists, and
    says so."""
    lines = [
        "# GENERATED by scripts/convert-environment.py (make assets) — do not edit.",
        "# Window panes DERIVED from the scene's glass materials: centers,",
        "# sizes, and facings are the model's own; light energy scales with",
        "# pane area via the environment's authored window_energy_per_m2.",
        "",
        f'environment = "{os.path.splitext(os.path.basename(path))[0]}"',
        "",
    ]
    if not panes:
        lines.append("# No pane identified itself in this scene (no glass material on a")
        lines.append("# thin vertical sheet of window size).")
        lines.append("")
    else:
        all_lo = [min(p[1][i] for p in panes) for i in range(2)]
        all_hi = [max(p[2][i] for p in panes) for i in range(2)]
        # Interior center for inward facing: the panes' own bounding centroid.
        cx = (all_lo[0] + all_hi[0]) / 2.0
        cy = (all_lo[1] + all_hi[1]) / 2.0
        for o, blo, bhi, thin in panes:
            center_b = [(blo[i] + bhi[i]) / 2.0 for i in range(3)]
            span = (bhi[1 - thin] - blo[1 - thin])
            height = bhi[2] - blo[2]
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


if args.windows:
    emit_windows_toml(args.windows)
    print(f"convert-environment: {len(panes)} window panes derived -> {args.windows}")
else:
    print(f"convert-environment: {len(panes)} window panes derived (not written)")

# Glass-only glow: the pane radiates, everything in front silhouettes.
glowed = set()
for o, blo, bhi, thin in panes:
    for m in o.data.materials:
        if m is None or m.name in glowed or not m.use_nodes:
            continue
        bsdf = m.node_tree.nodes.get("Principled BSDF")
        if bsdf is None:
            continue
        bsdf.inputs["Emission Color"].default_value = (1.0, 0.98, 0.94, 1.0)
        bsdf.inputs["Emission Strength"].default_value = PANE_EMISSION
        glowed.add(m.name)
print(f"convert-environment: {len(glowed)} glass pane materials set emissive")

# ---- bake the importer's object transforms (the FBX importer's +90 deg X;
# an OBJ import already sits upright and passes through unchanged) into
# the mesh data, as decimate.py does; single-user first because the bake
# refuses shared data ----
single_user(meshes)
bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)

# ---- cap + pack textures so the exporter embeds the capped versions ----
cap_and_pack_images(tex_cap)

total = 0
for o in meshes:
    o.data.calc_loop_triangles()
    total += len(o.data.loop_triangles)

print(f"convert-environment: exporting {total} tris (no decimation)...", flush=True)
# No vertex colors: no plan reads them, and the exporter warns per mesh
# about an active color layer its material never uses (the apartment's
# four, 2026-09-07).
bpy.ops.export_scene.gltf(filepath=out_path, export_format="GLB", export_apply=True,
                          export_vertex_color="NONE")
size_mb = os.path.getsize(out_path) / 1e6
print(f"convert-environment: {total} tris, {wired} textured materials -> {out_path} ({size_mb:.0f} MB)")
