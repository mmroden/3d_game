"""Split a multi-panel kit into per-panel game-weight .glb files.

    blender --background --python scripts/split-panels.py -- \
        <kit.glb|kit.blend> <out_dir> <target_tris>

The cgtrader "Sci-Fi Parts Kit" packs carry all pieces as sibling mesh
objects. Vol 01 ships one GLB with embedded PBR materials; Vol 03 ships a
.blend whose Principled BSDF graphs arrive fully wired to the loose PBR
maps beside it — either way, every mesh piece becomes its own .glb:
textures downscaled once (they are shared), every object decimated to
roughly <target_tris>, exported selection-by-selection under a sanitized
stable name — the asset catalog references these names, so they must never
depend on kit ordering.

Near-pitch squares are NORMALIZED: square plates within NORMALIZE_TOL of
the kit's largest square snap exactly to it (a bake, applied to the mesh),
so provider slop (2.944 vs 2.999) can't open seams between wall panels or
fail the probe's grid-agreement check. The largest square itself is never
scaled — the kit's own plates stay the pitch truth.

Every piece is also POSE-normalized into the assembler's native pose: the
plate face lying flat, thin axis up, detailed side facing up (in glTF
terms: face in XZ, thin along +Y — Blender's Z is glTF's Y). Provider
packs author plates in arbitrary axes; the room assembler's rotations
turn ONE canonical pose into all six cell faces, so orientation slop must
die here, at the conversion, never downstream (vol03's whole wall pool
arrived Z-thin and skinned rooms as open venetian blinds, 2026-07-18).
The probe's `wall_pool_pieces_arrive_in_native_pose` contract audits the
result.
"""
import math
import os
import re
import sys
import tomllib

import bpy
import mathutils

argv = sys.argv[sys.argv.index("--") + 1:]
kit_path, out_dir, target_tris = argv[0], argv[1], int(argv[2])

TEX_SIZE = 1024
# A square plate within 5% of the kit's largest square is the same plate
# with provider slop; anything farther off is a genuinely smaller piece.
NORMALIZE_TOL = 0.05
# Two plate-face extents within 1% of each other read as square.
SQUARE_TOL = 0.01

# ── Load stage: format-sniffed ─────────────────────────────────────────
if kit_path.lower().endswith(".blend"):
    bpy.ops.wm.open_mainfile(filepath=kit_path)
else:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=kit_path)

# Shared textures: downscale once before any export embeds them.
for img in bpy.data.images:
    if img.size[0] > TEX_SIZE or img.size[1] > TEX_SIZE:
        img.scale(min(img.size[0], TEX_SIZE), min(img.size[1], TEX_SIZE))

# Panels export DOUBLE-SIDED (glTF doubleSided rides Blender's backface
# toggle): a plate's back is a real surface — single-sided plates render
# rooms invisible from outside, so any sightline crossing the inter-room
# void reads as skybox (the visual suite's LEAK class, 2026-07-14).
for mat in bpy.data.materials:
    mat.use_backface_culling = False

meshes = [o for o in bpy.data.objects if o.type == "MESH"]
os.makedirs(out_dir, exist_ok=True)

# Flatten every object transform into its mesh ONCE: importers park
# provider units in object scale (vol01 arrives ×0.01 — centimeter
# authoring) and axis conversions in object rotation, so raw mesh math
# lies about world geometry (2026-07-19: every bake qualification read
# 300 m plates). After this, mesh space IS world space and stays that
# way through every later transform.
for obj in meshes:
    if obj.data.users > 1:
        obj.data = obj.data.copy()
    obj.data.transform(obj.matrix_world)
    obj.matrix_world = mathutils.Matrix.Identity(4)


def plate_face(obj):
    """The two largest mesh extents (the plate face); None if degenerate."""
    vs = obj.data.vertices
    if not vs:
        return None
    lo = [min(v.co[i] for v in vs) for i in range(3)]
    hi = [max(v.co[i] for v in vs) for i in range(3)]
    dims = sorted((hi[i] - lo[i] for i in range(3)), reverse=True)
    return (dims[0], dims[1]) if dims[0] > 0 else None


def is_square(obj):
    face = plate_face(obj)
    return face is not None and (face[0] - face[1]) <= SQUARE_TOL * face[0]


def _stem(obj):
    # 'SF PP01_A_001' -> sf_pp01_a_001 (stable, name-derived) — the one
    # identity the catalog, the census and the authored flips all key on.
    return re.sub(r"[^a-z0-9]+", "_", obj.name.lower()).strip("_")


def _kit_policy():
    # (wall_coverage, authored per-piece detail flips) from the kit's
    # manifest — the stems whose artist front is NOT their relief-heavy
    # side, the one facing fact no measurement can recover.
    repo = os.path.abspath(os.path.join(out_dir, "..", "..", ".."))
    with open(os.path.join(repo, "catalog", "kits.toml"), "rb") as f:
        kits = tomllib.load(f)["kits"]
    rel = os.path.relpath(out_dir, repo)
    for kit in kits.values():
        if kit.get("install_dir") == rel:
            return kit.get("wall_coverage"), set(kit.get("detail_flip", []))
    return None, set()


bar, FLIP_STEMS = _kit_policy()


# ── Pose-normalize stage: thin axis up, detailed side up ───────────────
# Blender is Z-up and the glTF exporter maps Blender Z to glTF Y, so the
# native pose here is "thin along Blender Z"; the export then lands thin
# along glTF Y. All pose math mutates MESH DATA directly
# (mesh.transform) — bpy.ops transform_apply silently failed to reach
# the exports in background mode (2026-07-19: an entire bake measured
# identical poses); matrix-on-data has no operator context to lose.
def _mesh_bounds(obj):
    vs = obj.data.vertices
    lo = [min(v.co[i] for v in vs) for i in range(3)]
    hi = [max(v.co[i] for v in vs) for i in range(3)]
    return lo, hi

def _mesh_dims(obj):
    lo, hi = _mesh_bounds(obj)
    return [hi[i] - lo[i] for i in range(3)]

def _relief_split_z(obj):
    # (positive, negative) relief mass along Z: geometry weighed by its
    # distance BEYOND the base slab — the peak flat-area depth bin. The
    # heavier side is the greeble. EXACTLY the probe's relief_split
    # (asset_catalog/probe.rs); the two ends of the pipeline compute the
    # same number on the same meshes and cannot disagree by construction
    # (the winding sum this replaces read modeling conventions instead —
    # the bake 5/6/8 oscillation saga).
    BINS = 32
    lo_z = min(v.co[2] for v in obj.data.vertices)
    hi_z = max(v.co[2] for v in obj.data.vertices)
    depth = hi_z - lo_z
    if depth <= 0.0:
        return 0.0, 0.0
    flat = [0.0] * BINS
    weighed = []
    for poly in obj.data.polygons:
        if poly.area <= 0.0:
            continue
        c = poly.center[2]
        b = min(int((c - lo_z) / depth * BINS), BINS - 1)
        flat[b] += poly.area * abs(poly.normal[2])
        weighed.append((poly.area, c))
    peak = max(range(BINS), key=lambda b: flat[b])
    bin_lo = lo_z + depth * peak / BINS
    bin_hi = lo_z + depth * (peak + 1) / BINS
    pos = sum(a * max(0.0, c - bin_hi) for a, c in weighed)
    neg = sum(a * max(0.0, bin_lo - c) for a, c in weighed)
    return pos, neg

def pose_normalize(obj):
    dims = _mesh_dims(obj)
    if min(dims) <= 0.0:
        return
    thin = min(range(3), key=lambda i: dims[i])
    if thin == 0:  # thin along X: stand the face up about Y
        obj.data.transform(mathutils.Matrix.Rotation(math.radians(90.0), 4, "Y"))
    elif thin == 1:  # thin along Y: lay the face flat about X
        obj.data.transform(mathutils.Matrix.Rotation(math.radians(90.0), 4, "X"))
    if thin != 2:
        print(f"panel: {obj.name!r} pose-normalized (was thin along {'XYZ'[thin]})")
    # Artist front up: the relief-heavy side (the greeble), unless the
    # kit AUTHORS this piece as detailed on its relief-light side
    # (kits.toml detail_flip). Ties break positive, deterministically,
    # matching the probe.
    pos, neg = _relief_split_z(obj)
    if ((pos >= neg)) == (_stem(obj) in FLIP_STEMS):
        obj.data.transform(mathutils.Matrix.Rotation(math.pi, 4, "X"))
        print(f"panel: {obj.name!r} flipped front-side up")
    # Long axis conventionally on X (width-X): the v2 assembler's course
    # math and the preassembly stacker both assume it; a Z-rotation
    # preserves thin axis and detail facing.
    dims = _mesh_dims(obj)
    if dims[1] > dims[0]:
        obj.data.transform(mathutils.Matrix.Rotation(math.radians(90.0), 4, "Z"))

for obj in meshes:
    pose_normalize(obj)

# ── Normalize stage: snap near-pitch squares to the kit's largest ──────
squares = [o for o in meshes if is_square(o)]
pitch = max((plate_face(o)[0] for o in squares), default=0.0)
for obj in squares:
    side = plate_face(obj)[0]
    if side <= 0.0 or side == pitch:
        continue
    if (pitch - side) / pitch <= NORMALIZE_TOL:
        factor = pitch / side
        obj.data.transform(mathutils.Matrix.Scale(factor, 4))
        print(f"panel: {obj.name!r} normalized {side:.3f} -> {pitch:.3f}")

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

    out_path = os.path.join(out_dir, f"{_stem(obj)}.glb")

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

# ── Role bake (Transform v2): three surface-role variants per QUALIFYING
# piece — the census-v2 library the v2 assembler consumes. Qualification
# mirrors the linker's wall filter (square, cell-sized, covered to the
# kit's authored wall_coverage bar, read from ITS one home: catalog/
# kits.toml). Coverage here is an analytic PRE-filter (projected triangle
# area over face area; double-sided geometry counts twice, which only
# widens the solid/truss gap) — the Rust probe re-measures every baked
# file and the linker enforces the real bar, so a Python misjudgment
# fails the load, loudly.
def _face_dims(obj):
    # Mesh-data bounds, never obj.dimensions: data.transform edits do
    # not refresh the evaluated bbox without a depsgraph pass.
    dims = sorted(_mesh_dims(obj), reverse=True)
    return dims[0], dims[1]

def _census_solids():
    # {stem: (width, height)} for this kit's SOLID pieces, from the
    # previous census — the probe's measured faces and coverage, never a
    # Blender-side guess.
    repo = os.path.abspath(os.path.join(out_dir, "..", "..", ".."))
    rel = os.path.relpath(out_dir, repo)
    try:
        with open(os.path.join(repo, "catalog", "kits.generated.toml"), "rb") as f:
            kits = tomllib.load(f)["kits"]
    except FileNotFoundError:
        return {}
    with open(os.path.join(repo, "catalog", "kits.toml"), "rb") as f:
        manifest_kits = tomllib.load(f)["kits"]
    key = next((k for k, v in manifest_kits.items() if v.get("install_dir") == rel), None)
    if key is None or key not in kits:
        return {}
    coverage_bar = manifest_kits[key].get("wall_coverage", 1.0)
    out = {}
    for stem, p in kits[key].get("pieces", {}).items():
        if p.get("coverage", 0.0) >= coverage_bar:
            face = p.get("face", [0.0, 0.0])
            out[stem] = (face[0], face[1])
    return out


def _stack_pieces(obj_a, obj_b, scale_w, scale_h):
    # One combined mesh: b's plate below, a's above, scaled onto the
    # module. Pure data-API (bmesh) — no join operator, no context.
    import bmesh

    ha = sorted(_mesh_dims(obj_a), reverse=True)[1]
    hb = sorted(_mesh_dims(obj_b), reverse=True)[1]
    total = ha + hb
    mesh = bpy.data.meshes.new("stack")
    bm = bmesh.new()
    mats = []
    face_ranges = []
    for obj, y_center in [
        (obj_b, -total / 2.0 + hb / 2.0),
        (obj_a, total / 2.0 - ha / 2.0),
    ]:
        tmp = obj.data.copy()
        # Align component BACKS (z_min), not centers: components of
        # different depths (p 0.15 vs s 0.40) center-aligned leave the
        # thinner face recessed — background bled through every seam
        # (capture 2026-07-19). Backs to z=0 here; the whole stack
        # recenters below so the assembler's thick/2 seat lands the
        # common back on the face plane.
        z_min = min(v.co[2] for v in tmp.vertices)
        tmp.transform(mathutils.Matrix.Translation((0.0, y_center, -z_min)))
        start = len(bm.faces)
        bm.from_mesh(tmp)
        face_ranges.append((start, len(bm.faces), len(mats), tmp.materials[:]))
        mats.extend(tmp.materials[:])
        bpy.data.meshes.remove(tmp)
    bm.faces.ensure_lookup_table()
    for start, end, mat_offset, _ in face_ranges:
        if mat_offset:
            for i in range(start, end):
                bm.faces[i].material_index += mat_offset
    bm.to_mesh(mesh)
    bm.free()
    for m in mats:
        mesh.materials.append(m)
    mesh.transform(
        mathutils.Matrix.Diagonal((scale_w, scale_h, 1.0, 1.0))
    )
    # Recenter depth: backs sit at 0 after alignment; the census thick
    # is the max depth, and the seat math assumes a centered plate.
    depth = max(v.co[2] for v in mesh.vertices)
    mesh.transform(mathutils.Matrix.Translation((0.0, 0.0, -depth / 2.0)))
    combined = bpy.data.objects.new("stack", mesh)
    bpy.context.scene.collection.objects.link(combined)
    return combined


if bar is None:
    print("panels: no wall_coverage authored — no role bake for this kit")
else:
    pitch = max((plate_face(o)[0] for o in meshes if is_square(o)), default=0.0)
    # Blender Z is glTF Y: canonical (thin Z, detail +Z) IS the floor
    # pose; ceiling flips π about X; wall stands the plate up (+90° X),
    # detail landing on glTF +Z. The probe measures what actually landed.
    ROLE_ROTATIONS = [
        ("floor", 0.0),
        ("ceiling", math.pi),
        ("wall", math.radians(90.0)),
    ]
    manifest = []
    baked = 0

    def bake_roles(obj, stem, sources, stretch):
        # Pieces arrive ARTIST-FRONT UP from pose_normalize (relief
        # measure + authored flips); the role rotation is all that's
        # left to apply.
        global baked
        for role, angle in ROLE_ROTATIONS:
            # Rotate the MESH, export, rotate back — same no-ops-context
            # rule as pose_normalize; the inverse restores exactly.
            rot = mathutils.Matrix.Rotation(angle, 4, "X")
            obj.data.transform(rot)
            out_path = os.path.join(out_dir, f"{stem}_{role}.glb")
            bpy.ops.object.select_all(action="DESELECT")
            obj.select_set(True)
            bpy.ops.export_scene.gltf(
                filepath=out_path,
                use_selection=True,
                export_format="GLB",
            )
            obj.data.transform(rot.inverted())
            srcs = ", ".join(f'"{s}"' for s in sources)
            manifest.append(
                f'[variants.{stem}_{role}]\n'
                f'sources = [{srcs}]\nrole = "{role}"\nstretch = {stretch:.4f}\n'
            )
            baked += 1

    by_stem = {}
    for obj in sorted(meshes, key=lambda o: o.name):
        stem = _stem(obj)
        by_stem[stem] = obj
        w, h = _face_dims(obj)
        # Bake every ON-MODULE piece (shorter face extent on the pitch):
        # coverage judgment belongs to the probe's rasterizer alone — the
        # linker's census × policy bar filters truss overbakes into
        # unused files (a Python analytic first over- then under-baked,
        # 2026-07-19; it does not get a third chance).
        if abs(h - pitch) > 0.05:
            print(f"panel: {obj.name!r} not on module ({h:.3f} vs pitch {pitch:.3f}) — no bake")
            continue
        bake_roles(obj, stem, [stem], 0.0)
    # ── Preassembly stage (census-driven combinatorics, Mark's metric-
    # completion design): same-width SOLID pieces whose heights STACK to
    # the module become one baked plate — e.g. vol03's q (5.6×1.16) on
    # r (5.6×1.84) is exactly 3.0 tall; width stretches onto the nearest
    # integer meter within the cap (5.6→6.0, +7%). Widths/heights/
    # coverage come from the PREVIOUS census (the probe's measurements,
    # the one authority); a fresh checkout converges over two `make
    # assets` runs and ships converged.
    STRETCH_TOL = 0.10
    census_pieces = _census_solids()
    import itertools
    for (s1, f1), (s2, f2) in itertools.combinations(sorted(census_pieces.items()), 2):
        w1, h1 = f1
        w2, h2 = f2
        if s1 not in by_stem or s2 not in by_stem:
            continue
        if abs(w1 - w2) > 0.02 * max(w1, w2):
            continue
        stacked_h = h1 + h2
        target_w = float(round(w1))
        if target_w < pitch - 0.05 or target_w <= 0.0:
            continue
        stretch_w = abs(target_w / w1 - 1.0)
        stretch_h = abs(pitch / stacked_h - 1.0)
        if stretch_w > STRETCH_TOL or stretch_h > STRETCH_TOL:
            continue
        if (s1 in FLIP_STEMS) != (s2 in FLIP_STEMS):
            # A stack mixing an authored-flip piece with a plain one has
            # no single artist front — the probe refuses to census it,
            # so the Transform refuses to build it.
            print(f"panel: not stacking {s1!r}+{s2!r} — mixed authored detail_flip")
            continue
        stem = f"{s1}_{s2}_stack"
        combined = _stack_pieces(by_stem[s1], by_stem[s2], target_w / w1, pitch / stacked_h)
        print(
            f"panel: stacked {s1!r}+{s2!r} -> {stem} "
            f"({target_w:.1f}x{pitch:.1f}, stretch {max(stretch_w, stretch_h):.3f})"
        )
        bake_roles(combined, stem, [s1, s2], max(stretch_w, stretch_h))
        mesh = combined.data
        bpy.data.objects.remove(combined)
        bpy.data.meshes.remove(mesh)

    with open(os.path.join(out_dir, "manifest.toml"), "w") as f:
        f.write(
            "# GENERATED by scripts/split-panels.py (the Transform) — do not edit.\n"
            "# Declares each baked role variant; the probe measures the same\n"
            "# files and the linker cross-checks declaration against measurement.\n\n"
            + "\n".join(manifest)
        )
    print(f"panels: {baked} role variants baked (bar {bar})")
