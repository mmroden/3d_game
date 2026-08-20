"""Pure-Python plan for the cockpit-shell extraction (`make assets`).

The player hull GLBs are exterior flight models, but Spacecraft_1 (the
Vanguard) also carries a fully furnished, human-scale cockpit interior:
console/keyboard cluster, seat, side pods, and an alpha-blend canopy
glass. The first-person stereo view wants exactly that furniture as a
near-field shell around the camera — and nothing else of the hull.

This module is the extraction ORACLE: it parses the source .glb directly
(no Blender) and derives WHICH parts are cockpit interior and WHERE the
pilot's eyepoint sits, from one geometric rule anchored on the canopy:

  * the canopy is the largest-volume alpha-blended (glass) part;
  * the cockpit volume is the canopy AABB inflated by fixed margins
    (sideways for consoles/frame, down for seat and floor dressing,
    along z for the instrument bulkhead and the aft deck);
  * a part is cockpit interior iff its AABB lies fully inside that
    volume — big fuselage/wing pieces always straddle the boundary and
    fall out naturally, so no part-name list exists anywhere;
  * the eyepoint sits on the canopy's x center, low in the glass and
    forward near the dash, so the pilot sees their own cockpit.

scripts/extract-cockpit.py (Blender) APPLIES this plan; the pytest audit
(scripts/tests/test_cockpit_extraction.py) holds both the plan and the
built artifact to the rule. Coordinates throughout are glTF world space
(Y up; the cockpit faces -Z, same as a Godot camera).
"""
import json
import struct

# ── The selection rule's tuning, named and reasoned ────────────────────
# Sideways inflation of the canopy half-width: the canopy glass is
# narrower than the console deck under it; 1.9x reaches the instrument
# bulkhead's full width without swallowing the wing roots.
LATERAL_INFLATE = 1.9
# Below the canopy: seat base and console floor sit ~0.6-0.9 canopy
# heights under the glass line.
DOWN_MARGIN = 0.9
# Above: a sliver for the spine/roof trim directly over the glass.
UP_MARGIN = 0.3
# Along z: the instrument bulkhead sits forward of the glass (toward the
# nose, -z for this cockpit), the aft deck behind the seat (+z).
Z_LO_MARGIN = 1.2
Z_HI_MARGIN = 0.9
# Eyepoint height as a fraction of the canopy AABB's height above its
# floor: low in the glass, at seated eye level — high enough to see out,
# low enough that the console rises into the bottom of the view.
EYE_HEIGHT_FRAC = 0.45
# Eyepoint depth as a fraction from the canopy's forward (-z) edge toward
# its aft edge: FORWARD of center, close over the dash. The dominant
# framing term is dash proximity — rig frames showed mid-glass (0.5) and
# aft (0.65) eyes both see mostly-open canopy, because the furniture is
# all below and ahead; only an eye near the instrument line has it fill
# the bottom of the view with the canopy bows at the edges.
EYE_AFT_FRAC = 0.40
# Opaque interior materials are floored to at least this roughness (and
# lose their metal-roughness maps) by the extraction: mirror-glossy
# furniture centimeters from the eyes shimmers against every pose — a
# stereo-comfort defect the rig's pose-invariance contract caught
# (2026-08-19). Canopy glass (alpha-blend) is exempt.
ROUGHNESS_FLOOR = 0.6

GLB_MAGIC = 0x46546C67
JSON_CHUNK = 0x4E4F534A
BIN_CHUNK = 0x004E4942

# Accessor componentType -> struct format / byte size (the ones player
# hulls use; a new type fails loudly in _accessor_values).
_COMP_FMT = {5121: "B", 5123: "H", 5125: "I", 5126: "f"}
_COMP_SIZE = {5121: 1, 5123: 2, 5125: 4, 5126: 4}
_NCOMP = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}

# A triangle whose cross-product magnitude (2x area) falls under this is
# degenerate: renders nothing, and exporters legitimately prune it.
_DEGENERATE_CROSS = 1e-9


def _trs_matrix(node):
    """Node-local transform as a 4x4 row-major matrix (glTF column-major
    `matrix` or TRS, whichever the node carries)."""
    if "matrix" in node:
        m = node["matrix"]
        return [[m[0], m[4], m[8], m[12]],
                [m[1], m[5], m[9], m[13]],
                [m[2], m[6], m[10], m[14]],
                [m[3], m[7], m[11], m[15]]]
    t = node.get("translation", [0.0, 0.0, 0.0])
    q = node.get("rotation", [0.0, 0.0, 0.0, 1.0])
    s = node.get("scale", [1.0, 1.0, 1.0])
    x, y, z, w = q
    r = [[1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
         [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
         [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)]]
    return [[r[i][j] * s[j] for j in range(3)] + [t[i]] for i in range(3)] \
        + [[0.0, 0.0, 0.0, 1.0]]


def _matmul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)]
            for i in range(4)]


def _xform(m, p):
    v = [p[0], p[1], p[2], 1.0]
    return [sum(m[i][k] * v[k] for k in range(4)) for i in range(3)]


def _accessor_values(gltf, binbuf, idx):
    """All values of accessor `idx` from the binary chunk, as tuples."""
    acc = gltf["accessors"][idx]
    view = gltf["bufferViews"][acc["bufferView"]]
    ncomp = _NCOMP[acc["type"]]
    csize = _COMP_SIZE[acc["componentType"]]
    stride = view.get("byteStride") or ncomp * csize
    base = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
    fmt = "<" + _COMP_FMT[acc["componentType"]] * ncomp
    return [struct.unpack_from(fmt, binbuf, base + i * stride)
            for i in range(acc["count"])]


def parse_glb(path):
    """Read a .glb and return its mesh-bearing parts as a list of dicts:
    {name, tris, distinct_solid_tris, blend, lo, hi} with world-space
    AABBs (glTF Y-up coordinates), one entry per mesh-bearing node (a
    node's primitives are merged: AABBs unioned, triangles summed, blend
    if any is). `distinct_solid_tris` counts DISTINCT area-bearing
    triangles (deduplicated by rounded vertex positions) — the geometry a
    round-trip through an exporter must preserve exactly: exact-duplicate
    faces and zero-area degenerates render nothing and exporters
    legitimately weld them away, while `tris` is the raw authored count."""
    with open(path, "rb") as f:
        magic, _version, length = struct.unpack("<III", f.read(12))
        if magic != GLB_MAGIC:
            raise ValueError(f"{path} is not a .glb container")
        chunks = {}
        while f.tell() < length:
            clen, ctype = struct.unpack("<II", f.read(8))
            chunks[ctype] = f.read(clen)
    gltf = json.loads(chunks[JSON_CHUNK])
    binbuf = chunks[BIN_CHUNK]

    blend_mats = {
        i for i, mat in enumerate(gltf.get("materials", []))
        if mat.get("alphaMode") in ("BLEND", "MASK")
    }

    def tri_count(prim):
        acc = prim.get("indices", prim["attributes"]["POSITION"])
        return gltf["accessors"][acc]["count"] // 3

    def solid_tri_set(prim, world):
        """The prim's DISTINCT area-bearing triangles in world space, as
        sorted rounded vertex triples (order- and winding-insensitive;
        rounding absorbs float32 round-trip noise)."""
        pts = [_xform(world, p)
               for p in _accessor_values(gltf, binbuf, prim["attributes"]["POSITION"])]
        if "indices" in prim:
            order = [i[0] for i in _accessor_values(gltf, binbuf, prim["indices"])]
        else:
            order = range(len(pts))
        distinct = set()
        for k in range(0, len(order) - 2, 3):
            a, b, c = pts[order[k]], pts[order[k + 1]], pts[order[k + 2]]
            u = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
            v = (c[0] - a[0], c[1] - a[1], c[2] - a[2])
            cross = (u[1] * v[2] - u[2] * v[1],
                     u[2] * v[0] - u[0] * v[2],
                     u[0] * v[1] - u[1] * v[0])
            if (cross[0] ** 2 + cross[1] ** 2 + cross[2] ** 2) ** 0.5 \
                    >= _DEGENERATE_CROSS:
                distinct.add(tuple(sorted(
                    tuple(round(coord, 5) for coord in p) for p in (a, b, c)
                )))
        return distinct

    parts = []

    def walk(node_idx, parent_m):
        node = gltf["nodes"][node_idx]
        m = _matmul(parent_m, _trs_matrix(node))
        if "mesh" in node:
            part = None
            for prim in gltf["meshes"][node["mesh"]]["primitives"]:
                acc = gltf["accessors"][prim["attributes"]["POSITION"]]
                corners = [
                    _xform(m, [acc["min"][0] if i & 1 else acc["max"][0],
                               acc["min"][1] if i & 2 else acc["max"][1],
                               acc["min"][2] if i & 4 else acc["max"][2]])
                    for i in range(8)
                ]
                lo = [min(c[k] for c in corners) for k in range(3)]
                hi = [max(c[k] for c in corners) for k in range(3)]
                blend = prim.get("material") in blend_mats
                if part is None:
                    part = {"name": node.get("name", f"node{node_idx}"),
                            "tris": tri_count(prim),
                            "_tri_set": solid_tri_set(prim, m),
                            "blend": blend, "lo": lo, "hi": hi}
                else:
                    part["tris"] += tri_count(prim)
                    part["_tri_set"] |= solid_tri_set(prim, m)
                    part["blend"] = part["blend"] or blend
                    part["lo"] = [min(part["lo"][k], lo[k]) for k in range(3)]
                    part["hi"] = [max(part["hi"][k], hi[k]) for k in range(3)]
            parts.append(part)
        for child in node.get("children", []):
            walk(child, m)

    identity = [[float(i == j) for j in range(4)] for i in range(4)]
    for root in gltf["scenes"][gltf.get("scene", 0)]["nodes"]:
        walk(root, identity)
    for part in parts:
        part["distinct_solid_tris"] = len(part.pop("_tri_set"))
    return parts


def plan_cockpit(parts):
    """Derive the extraction plan from a hull's parts (see module doc).
    Returns {canopy, volume: (lo, hi), keep, drop, eyepoint}; keep/drop
    partition the input part names."""
    glass = [p for p in parts if p["blend"]]
    if not glass:
        raise ValueError("hull has no alpha-blended part to anchor the canopy rule")
    canopy = max(
        glass,
        key=lambda p: (p["hi"][0] - p["lo"][0])
        * (p["hi"][1] - p["lo"][1])
        * (p["hi"][2] - p["lo"][2]),
    )
    cx = (canopy["lo"][0] + canopy["hi"][0]) / 2
    half_w = (canopy["hi"][0] - canopy["lo"][0]) / 2 * LATERAL_INFLATE
    lo = [cx - half_w, canopy["lo"][1] - DOWN_MARGIN, canopy["lo"][2] - Z_LO_MARGIN]
    hi = [cx + half_w, canopy["hi"][1] + UP_MARGIN, canopy["hi"][2] + Z_HI_MARGIN]

    keep = [p["name"] for p in parts
            if all(lo[k] <= p["lo"][k] and p["hi"][k] <= hi[k] for k in range(3))]
    drop = [p["name"] for p in parts if p["name"] not in set(keep)]

    # The pilot's eye: canopy x center, low in the glass, forward over the
    # dash — the two fractions are the framing knobs (see their comments).
    height = canopy["hi"][1] - canopy["lo"][1]
    depth = canopy["hi"][2] - canopy["lo"][2]
    eyepoint = [cx,
                canopy["lo"][1] + EYE_HEIGHT_FRAC * height,
                canopy["lo"][2] + EYE_AFT_FRAC * depth]
    return {"canopy": canopy["name"], "volume": (lo, hi),
            "keep": keep, "drop": drop, "eyepoint": eyepoint}
