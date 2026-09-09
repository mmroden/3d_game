"""Scene metrics for `make assets` and `make metrics` (pure Python, no Blender).

    python3 scripts/scene-metrics.py <key> <scene.glb> <out.toml> [manifest.json] [zones.toml]

Written by install-addons.sh right after every environment conversion,
one file per scene under out/metrics/: what the pipeline PRODUCED, in
one stable, human-readable place — the scene's extents in model meters
(the numbers zone authoring and the kit scale decision start from),
its parts and triangles, its exported materials (alpha modes, textured
or flat), any surviving animations, the largest parts with their AABBs,
and the SOURCE file's own geometry counts when the material manifest
carries them (the OBJ manifest's vertex counts and percentile extents —
how the hill house's millimeter units and 175 km backdrop were told
apart). Read this instead of parsing a .glb or a manifest by hand: the
metrics are reproducible, an inline parse is not.

Beside the TOML, SECTION DRAWINGS for zone authoring — the scene cut by
a plane, drawn the way an architect's drawings read (owner 2026-09-06:
zones drafted from bounding numbers walled off a hallway that exists
and a ceiling that opens into the next floor; a vertex-density raster
showed neither, a wall being four vertices):
  <key>_plan_low.png   horizontal cut at sill height: counters, sofas,
                       tables print as their footprints;
  <key>_plan.png       horizontal cut between waist and lintel: walls
                       are lines, doorways are gaps — THE floor plan;
  <key>_plan_high.png  horizontal cut above the lintels: an opening
                       still open here goes all the way up;
  <key>_long_NN.png    vertical cuts along the body's long axis at its
                       quarter points (NN = 25, 50, 75 percent across
                       the short axis): floor, ceiling, mezzanines,
                       the end walls' openings (y up);
  <key>_cross_NN.png   the same across the short axis.
The plan cuts sit inside the AUTHORED zone band (the zones' lowest
floor to their highest ceiling) when a zone roster exists, so an
edit to the boxes re-cuts where they claim the rooms are; without
zones they sit inside the vertex cloud's 5th..95th percentile band.
The frame covers the body plus the zone boxes plus a margin, and
reaches as high as what stands on the footprint — an upper storey
shows even when the ground floor holds nearly every vertex. Solid
geometry draws neutral gray, glass blue, OVER the zone boxes (start
green with its north edge tripled, boss red dotted, others yellow):
a box edge that runs along a wall shows the wall, an edge across open
floor shows its color — the difference between a box that follows
the model and one that walls off a doorway. Every image's cut, axes,
frame and cell are in the TOML's [[section]] rows: pixel (i, j) is
model (frame_lo[0] + i * cell, frame_lo[1] + j * cell) on the
section's axes, rows counted from the top, or from the bottom when
flip_v (the vertical sections draw y up).

Bounds come from the accessors' declared min/max through the node
transforms (cockpit_plan.parse_glb in bounds-only mode); the sections
walk every triangle once (a four-million triangle scene takes under a
minute).
"""
import json
import math
import os
import struct
import sys
import zlib
from array import array

try:
    import tomllib
except ImportError:  # the audit venv (python 3.9) carries tomli
    import tomli as tomllib

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cockpit_plan import (  # noqa: E402
    BIN_CHUNK, GLB_MAGIC, JSON_CHUNK, _matmul, _trs_matrix, _xform, parse_glb,
)

from material_plan import clone_stem, find_in_inventory, texture_files  # noqa: E402

LARGEST = 24  # parts listed individually, by AABB volume
SECTION_MAX_CELLS = 1600  # longest image side, pixels
SECTION_MIN_CELL = 0.025  # meters per pixel, floor
# The section frame: the body (5th..95th percentile of the vertex cloud)
# joined with the authored zone boxes, plus this margin per axis —
# enough to show what leads out of the body (a hallway, a stair)
# without the backdrop shrinking the room to a smudge.
FRAME_MARGIN_MIN = 2.0
FRAME_MARGIN_FRAC = 0.5
# Cut heights above the band's floor / below its ceiling (meters): the
# low cut at sill height prints furniture, the plan cut sits between
# waist and lintel where doorways are open, the high cut above lintels.
PLAN_LOW_ABOVE_FLOOR = 0.6
PLAN_ABOVE_FLOOR = 1.5
PLAN_HIGH_BELOW_CEILING = 0.6
# Vertical cuts at these points along the body (percent of its span).
VERTICAL_CUT_PERCENTS = (25, 50, 75)
# The vertical frame also reaches the lowest and highest vertex standing
# this far around the body's footprint (plus a meter), capped at this
# many body heights past the body's floor and ceiling — an upper storey
# shows, a sky dome does not stretch the frame.
FOOTPRINT_REACH = 2.0
REACH_STOREYS = 3.0

SOLID_COLOR = (210, 210, 210)
GLASS_COLOR = (90, 150, 230)
ZONE_COLOR = (240, 200, 60)
START_COLOR = (80, 230, 100)
BOSS_COLOR = (240, 80, 80)

AXIS_NAMES = "xyz"
# Accessor componentType -> array typecode (all little-endian, 4-byte
# alignment is glTF's own rule).
_ARRAY_CODE = {5120: "b", 5121: "B", 5122: "h", 5123: "H", 5125: "I", 5126: "f"}
_NCOMP = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}


def glb_chunks(path):
    with open(path, "rb") as f:
        magic, _version, length = struct.unpack("<III", f.read(12))
        if magic != GLB_MAGIC:
            raise ValueError(f"{path} is not a .glb container")
        chunks = {}
        while f.tell() < length:
            clen, ctype = struct.unpack("<II", f.read(8))
            chunks[ctype] = f.read(clen)
    return json.loads(chunks[JSON_CHUNK]), chunks[BIN_CHUNK]


def toml_str(s):
    return '"' + str(s).replace("\\", "\\\\").replace('"', '\\"') + '"'


def toml_vec(v):
    return "[" + ", ".join(f"{x:.3f}" for x in v) + "]"


def toml_strs(seq):
    return "[" + ", ".join(toml_str(s) for s in seq) + "]"


def toml_bool(b):
    return "true" if b else "false"


def volume(p):
    return max(0.0, (p["hi"][0] - p["lo"][0])
               * (p["hi"][1] - p["lo"][1])
               * (p["hi"][2] - p["lo"][2]))


# ---------------------------------------------------------------- mesh

def _flat_accessor(gltf, binbuf, idx):
    """Accessor `idx` as one flat array of its components (compact: a
    ten-million-vertex scene stays in float32)."""
    acc = gltf["accessors"][idx]
    view = gltf["bufferViews"][acc["bufferView"]]
    values = array(_ARRAY_CODE[acc["componentType"]])
    ncomp = _NCOMP[acc["type"]]
    tight = ncomp * values.itemsize
    stride = view.get("byteStride") or tight
    base = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
    count = acc["count"]
    if stride == tight:
        values.frombytes(binbuf[base:base + count * tight])
    else:
        fmt = "<" + values.typecode * ncomp
        for i in range(count):
            values.extend(struct.unpack_from(fmt, binbuf, base + i * stride))
    if sys.byteorder != "little":
        values.byteswap()
    return values


def _is_identity(m):
    return all(abs(m[i][j] - (1.0 if i == j else 0.0)) < 1e-12
               for i in range(4) for j in range(4))


def _transform_flat(m, pos):
    out = array("d")
    for i in range(0, len(pos) - 2, 3):
        out.extend(_xform(m, (pos[i], pos[i + 1], pos[i + 2])))
    return out


def load_mesh(gltf, binbuf):
    """Every triangle primitive of the default scene as (positions,
    indices, blend, uvs, material): positions a flat array of world-
    space coordinates [x0, y0, z0, x1, ...] (glTF frame, node transforms
    applied), indices a flat sequence of vertex ids, three per triangle,
    uvs the flat TEXCOORD_0 array [u0, v0, u1, ...] or None, material
    the primitive's material index or None."""
    blend_mats = {
        i for i, mat in enumerate(gltf.get("materials", []))
        if mat.get("alphaMode") in ("BLEND", "MASK")
    }
    prims = []

    def walk(node_idx, parent_m):
        node = gltf["nodes"][node_idx]
        m = _matmul(parent_m, _trs_matrix(node))
        if "mesh" in node:
            for prim in gltf["meshes"][node["mesh"]]["primitives"]:
                if prim.get("mode", 4) != 4:
                    continue  # points and lines never wall anything
                pos = _flat_accessor(gltf, binbuf, prim["attributes"]["POSITION"])
                if not _is_identity(m):
                    pos = _transform_flat(m, pos)
                if "indices" in prim:
                    idx = _flat_accessor(gltf, binbuf, prim["indices"])
                else:
                    idx = range(len(pos) // 3)
                uv = None
                if "TEXCOORD_0" in prim["attributes"]:
                    uv = _flat_accessor(gltf, binbuf, prim["attributes"]["TEXCOORD_0"])
                material = prim.get("material")
                prims.append((pos, idx, material in blend_mats, uv, material))
        for child in node.get("children", []):
            walk(child, m)

    identity = [[float(i == j) for j in range(4)] for i in range(4)]
    for root in gltf["scenes"][gltf.get("scene", 0)]["nodes"]:
        walk(root, identity)
    return prims


def percentile(values, q):
    values = sorted(values)
    return values[int(q * (len(values) - 1))]


def body_percentiles(prims, q_lo=0.05, q_hi=0.95):
    lo, hi, count = [], [], 0
    for axis in range(3):
        coords = []
        for pos, *_rest in prims:
            coords.extend(pos[axis::3])
        count = len(coords)
        lo.append(percentile(coords, q_lo))
        hi.append(percentile(coords, q_hi))
    return lo, hi, count


def vertical_reach(prims, body_lo, body_hi):
    """The y range of what stands on the body's footprint: the lowest and
    highest vertex whose (x, z) lie within FOOTPRINT_REACH of the body,
    a meter beyond each — an archviz house's upper storey (a bare shell,
    a few vertices next to the furnished ground floor) shows in the
    sections, while a tree crown away from the house does not stretch
    them. Capped at REACH_STOREYS storeys past the body's floor and
    ceiling (a storey being the body's height, at least 3 m — a rug can
    own every percentile), so a sky dome over the house cannot either.
    None when nothing stands there."""
    x0, x1 = body_lo[0] - FOOTPRINT_REACH, body_hi[0] + FOOTPRINT_REACH
    z0, z1 = body_lo[2] - FOOTPRINT_REACH, body_hi[2] + FOOTPRINT_REACH
    lo = hi = None
    for pos, *_rest in prims:
        for i in range(0, len(pos) - 2, 3):
            if x0 <= pos[i] <= x1 and z0 <= pos[i + 2] <= z1:
                y = pos[i + 1]
                if lo is None or y < lo:
                    lo = y
                if hi is None or y > hi:
                    hi = y
    if lo is None:
        return None
    storey = max(3.0, body_hi[1] - body_lo[1])
    lo = max(lo, body_lo[1] - REACH_STOREYS * storey)
    hi = min(hi, body_hi[1] + REACH_STOREYS * storey)
    return lo - 1.0, hi + 1.0


def zones_bounds(zones):
    """The union AABB of the authored boxes, or None without zones."""
    if not zones:
        return None
    lo = [min(z[1][k] for z in zones) for k in range(3)]
    hi = [max(z[1][k] + z[2][k] for z in zones) for k in range(3)]
    return lo, hi


def frame_bounds(prims, body_lo, body_hi, zbox):
    """The section frame: the body joined with the zone boxes, plus a
    margin per axis; vertically also as far as what stands on the
    footprint reaches. Never clamped to the scene's extents — a margin
    past the last vertex is black, and a box drafted past the geometry
    must still draw where it was authored."""
    core_lo = [min(body_lo[k], zbox[0][k]) if zbox else body_lo[k] for k in range(3)]
    core_hi = [max(body_hi[k], zbox[1][k]) if zbox else body_hi[k] for k in range(3)]
    margin = [max(FRAME_MARGIN_MIN, FRAME_MARGIN_FRAC * (core_hi[k] - core_lo[k]))
              for k in range(3)]
    frame_lo = [core_lo[k] - margin[k] for k in range(3)]
    frame_hi = [core_hi[k] + margin[k] for k in range(3)]
    reach = vertical_reach(prims, body_lo, body_hi)
    if reach:
        frame_lo[1] = min(frame_lo[1], reach[0])
        frame_hi[1] = max(frame_hi[1], reach[1])
    return frame_lo, frame_hi


# ---------------------------------------------------------------- sections

def _cross(pos, a, b, c, axis, level):
    """The two points where the plane `axis = level` crosses triangle
    (a, b, c) (flat-array offsets), given that it does."""
    pts = []
    corners = ((a, b), (b, c), (c, a))
    for i, j in corners:
        pi, pj = pos[i + axis], pos[j + axis]
        if (pi < level) != (pj < level):
            t = (level - pi) / (pj - pi)
            pts.append((pos[i] + (pos[j] - pos[i]) * t,
                        pos[i + 1] + (pos[j + 1] - pos[i + 1]) * t,
                        pos[i + 2] + (pos[j + 2] - pos[i + 2]) * t))
    return pts[0], pts[1]


def cut_sections(prims, cuts):
    """Cut every triangle with each plane in `cuts` ([(axis, level)]).
    Returns one segment list per cut: (p, q, blend) with p and q the
    crossing points as 3-tuples. A triangle crosses a plane when some
    vertex lies below the level and some at or above it (the half-open
    rule makes a vertex exactly on the plane count once)."""
    out = [[] for _ in cuts]
    by_axis = {}
    for n, (axis, level) in enumerate(cuts):
        by_axis.setdefault(axis, []).append((level, out[n]))
    axes = list(by_axis.items())
    for pos, idx, blend, *_rest in prims:
        for k in range(0, len(idx) - 2, 3):
            a = idx[k] * 3
            b = idx[k + 1] * 3
            c = idx[k + 2] * 3
            for axis, levels in axes:
                pa = pos[a + axis]
                pb = pos[b + axis]
                pc = pos[c + axis]
                lo = pa if pa < pb else pb
                if pc < lo:
                    lo = pc
                hi = pa if pa > pb else pb
                if pc > hi:
                    hi = pc
                for level, segs in levels:
                    if lo < level <= hi:
                        p, q = _cross(pos, a, b, c, axis, level)
                        segs.append((p, q, blend))
    return out


def _clip(u0, v0, u1, v1, lo_u, lo_v, hi_u, hi_v):
    """Liang-Barsky: the parameter range of segment (u0,v0)-(u1,v1)
    inside the frame, or None."""
    t0, t1 = 0.0, 1.0
    du, dv = u1 - u0, v1 - v0
    for p, q in ((-du, u0 - lo_u), (du, hi_u - u0), (-dv, v0 - lo_v), (dv, hi_v - v0)):
        if p == 0:
            if q < 0:
                return None
            continue
        t = q / p
        if p < 0:
            if t > t1:
                return None
            if t > t0:
                t0 = t
        else:
            if t < t0:
                return None
            if t < t1:
                t1 = t
    return t0, t1


class Raster:
    """An RGB image over a model-space frame on two axes; `flip_v`
    draws increasing v upward (vertical sections, y up)."""

    def __init__(self, lo, hi, axes, flip_v):
        self.axes = axes
        self.flip_v = flip_v
        self.lo = (lo[axes[0]], lo[axes[1]])
        self.hi = (hi[axes[0]], hi[axes[1]])
        span_u = self.hi[0] - self.lo[0]
        span_v = self.hi[1] - self.lo[1]
        self.cell = max(SECTION_MIN_CELL, max(span_u, span_v) / SECTION_MAX_CELLS)
        self.w = max(1, int(math.ceil(span_u / self.cell)) + 1)
        self.h = max(1, int(math.ceil(span_v / self.cell)) + 1)
        self.px = bytearray(self.w * self.h * 3)

    def _index(self, i, j):
        if self.flip_v:
            j = self.h - 1 - j
        return (j * self.w + i) * 3

    def paint(self, i, j, color, over=True):
        if 0 <= i < self.w and 0 <= j < self.h:
            at = self._index(i, j)
            if over or not any(self.px[at:at + 3]):
                self.px[at:at + 3] = bytes(color)

    def segment(self, p, q, color, over):
        au, av = self.axes
        u0, v0, u1, v1 = p[au], p[av], q[au], q[av]
        span = _clip(u0, v0, u1, v1, self.lo[0], self.lo[1], self.hi[0], self.hi[1])
        if span is None:
            return
        t0, t1 = span
        du, dv = u1 - u0, v1 - v0
        n = int(max(abs(du), abs(dv)) * (t1 - t0) / self.cell * 2) + 1
        cell, lo_u, lo_v = self.cell, self.lo[0], self.lo[1]
        for s in range(n + 1):
            t = t0 + (t1 - t0) * s / n
            self.paint(int((u0 + du * t - lo_u) / cell),
                       int((v0 + dv * t - lo_v) / cell), color, over)

    def box(self, u0, v0, u1, v1, color, dotted=False, triple_v0=False):
        x0 = int((u0 - self.lo[0]) / self.cell)
        x1 = int((u1 - self.lo[0]) / self.cell)
        y0 = int((v0 - self.lo[1]) / self.cell)
        y1 = int((v1 - self.lo[1]) / self.cell)
        for x in range(max(0, x0), min(self.w, x1 + 1)):
            if dotted and x % 4 >= 2:
                continue
            for y in (y0, y1):
                self.paint(x, y, color)
        for y in range(max(0, y0), min(self.h, y1 + 1)):
            if dotted and y % 4 >= 2:
                continue
            for x in (x0, x1):
                self.paint(x, y, color)
        if triple_v0:
            for d in range(1, 4):
                for x in range(max(0, x0 + d), min(self.w, x1 - d + 1)):
                    self.paint(x, y0 + d, color)

    def write(self, path):
        stride = self.w * 3
        rows = [self.px[j * stride:(j + 1) * stride] for j in range(self.h)]
        write_png(path, self.w, self.h, rows)


def write_png(path, width, height, rows):
    """8-bit RGB PNG from rows of bytes (3 per pixel), stdlib only."""
    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)
    raw = b"".join(b"\x00" + bytes(row) for row in rows)
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 6))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


def load_zones(zones_path):
    if not zones_path or not os.path.isfile(zones_path):
        return []
    with open(zones_path, "rb") as f:
        doc = tomllib.load(f)
    zones = []
    for z in doc.get("zone", []):
        box = z["box"]
        zones.append((z.get("key", "?"), box["min"], box["extents"],
                      bool(z.get("start")), bool(z.get("boss"))))
    return zones


def draw_zones(raster, zones, axis, level):
    """Zone boxes on a section: every box on a horizontal cut (they
    are floor-to-ceiling volumes), only the boxes the plane passes
    through on a vertical one."""
    au, av = raster.axes
    for _key, bmin, ext, start, boss in zones:
        if axis != 1 and not (bmin[axis] <= level < bmin[axis] + ext[axis]):
            continue
        color = START_COLOR if start else BOSS_COLOR if boss else ZONE_COLOR
        raster.box(bmin[au], bmin[av], bmin[au] + ext[au], bmin[av] + ext[av],
                   color, dotted=boss, triple_v0=start and axis == 1)



# ---------------------------------------------------------------- zone faces

FACE_NAMES = {(0, -1): "-x", (0, 1): "+x", (1, -1): "-y", (1, 1): "+y",
              (2, -1): "-z", (2, 1): "+z"}
# Geometry within this of a walled face plane backs it (half a cell each
# side would miss a 3.45 m ceiling under a 4 m box top; a meter reads
# "the model has a surface here").
FACE_TOLERANCE = 1.0
FACE_GRID = 10  # sub-cells per unit face edge
OPEN_CELL_BELOW = 0.5  # a unit face cell backed less than this is open
OPEN_AT_LISTED = 12  # open cells named per zone face


def unit_faces(zones):
    """Every unit-cell face of the zone union that no other cell shares —
    the faces the containment shell walls (level_assembly's rule) —
    keyed (axis, plane, u, v): the plane's integer coordinate on `axis`
    and the cell's integer coordinates on the other two axes in
    ascending order; the value is (zone key, outward sign)."""
    cells = {}
    for key, bmin, ext, _start, _boss in zones:
        for x in range(bmin[0], bmin[0] + ext[0]):
            for y in range(bmin[1], bmin[1] + ext[1]):
                for z in range(bmin[2], bmin[2] + ext[2]):
                    cells[(x, y, z)] = key
    faces = {}
    for cell, key in cells.items():
        for axis in range(3):
            for sign in (-1, 1):
                neighbour = list(cell)
                neighbour[axis] += sign
                if tuple(neighbour) in cells:
                    continue
                plane = cell[axis] + (1 if sign > 0 else 0)
                u, v = [cell[k] for k in range(3) if k != axis]
                faces[(axis, plane, u, v)] = (key, sign)
    return faces


def _covers(tri, px, py):
    (x0, y0), (x1, y1), (x2, y2) = tri
    d0 = (x1 - x0) * (py - y0) - (y1 - y0) * (px - x0)
    d1 = (x2 - x1) * (py - y1) - (y2 - y1) * (px - x1)
    d2 = (x0 - x2) * (py - y2) - (y0 - y2) * (px - x2)
    return (d0 >= 0 and d1 >= 0 and d2 >= 0) or (d0 <= 0 and d1 <= 0 and d2 <= 0)


def _raster_face(bits, tri, u, v):
    """Mark the FACE_GRID x FACE_GRID sub-cells of unit face cell (u, v)
    whose centers the projected triangle covers."""
    g = FACE_GRID
    xs = [p[0] for p in tri]
    ys = [p[1] for p in tri]
    i0 = max(0, int(math.floor((min(xs) - u) * g)))
    i1 = min(g - 1, int(math.floor((max(xs) - u) * g)))
    j0 = max(0, int(math.floor((min(ys) - v) * g)))
    j1 = min(g - 1, int(math.floor((max(ys) - v) * g)))
    for i in range(i0, i1 + 1):
        px = u + (i + 0.5) / g
        for j in range(j0, j1 + 1):
            if bits[j * g + i]:
                continue
            if _covers(tri, px, v + (j + 0.5) / g):
                bits[j * g + i] = 1


def face_coverage(prims, faces):
    """For every walled unit face, the fraction of its area that model
    geometry within FACE_TOLERANCE of its plane projects onto: a wall
    along the face reads 1.0, a doorway or open floor 0.0, a triangle
    edge-on to the face (a floor crossing a wall plane) nothing."""
    by_plane = {}
    for (axis, plane, u, v) in faces:
        by_plane.setdefault((axis, plane), set()).add((u, v))
    cover = {key: bytearray(FACE_GRID * FACE_GRID) for key in faces}
    tol = FACE_TOLERANCE
    for pos, idx, _blend, *_rest in prims:
        for k in range(0, len(idx) - 2, 3):
            a = idx[k] * 3
            b = idx[k + 1] * 3
            c = idx[k + 2] * 3
            for axis in range(3):
                pa = pos[a + axis]
                pb = pos[b + axis]
                pc = pos[c + axis]
                lo = pa if pa < pb else pb
                if pc < lo:
                    lo = pc
                hi = pa if pa > pb else pb
                if pc > hi:
                    hi = pc
                p0 = int(math.ceil(lo - tol))
                p1 = int(math.floor(hi + tol))
                if p1 < p0:
                    continue
                tri = None
                for plane in range(p0, p1 + 1):
                    cells_here = by_plane.get((axis, plane))
                    if not cells_here:
                        continue
                    if tri is None:
                        au, av = [k2 for k2 in range(3) if k2 != axis]
                        tri = [(pos[i + au], pos[i + av]) for i in (a, b, c)]
                        us = [p[0] for p in tri]
                        vs = [p[1] for p in tri]
                        u0, u1 = int(math.floor(min(us))), int(math.floor(max(us)))
                        v0, v1 = int(math.floor(min(vs))), int(math.floor(max(vs)))
                    for u in range(u0, u1 + 1):
                        for v in range(v0, v1 + 1):
                            if (u, v) in cells_here:
                                _raster_face(cover[(axis, plane, u, v)], tri, u, v)
    return cover


def zone_face_rows(zones, faces, cover):
    """One row per (zone, face direction): unit cells walled there, the
    mean backed fraction, the open cells (backed under OPEN_CELL_BELOW)
    with the first few named by their cell coordinates."""
    groups = {}
    for (axis, plane, u, v), (key, sign) in faces.items():
        bits = cover[(axis, plane, u, v)]
        backed = sum(bits) / len(bits)
        cell = [0, 0, 0]
        cell[axis] = plane - (1 if sign > 0 else 0)
        others = [k for k in range(3) if k != axis]
        cell[others[0]], cell[others[1]] = u, v
        g = groups.setdefault((key, FACE_NAMES[(axis, sign)]), {"n": 0, "sum": 0.0, "open": []})
        g["n"] += 1
        g["sum"] += backed
        if backed < OPEN_CELL_BELOW:
            g["open"].append(cell)
    order = {k: i for i, k in enumerate(z[0] for z in zones)}
    rows = []
    for (key, face), g in sorted(groups.items(), key=lambda kv: (order[kv[0][0]], kv[0][1])):
        rows.append({
            "zone": key, "face": face, "cells": g["n"],
            "backed": g["sum"] / g["n"], "open_cells": len(g["open"]),
            "open_at": sorted(g["open"])[:OPEN_AT_LISTED],
        })
    return rows


def zone_faces(prims, zones):
    """The containment audit: how much of every walled zone face the
    model backs (owner 2026-09-06: "you need a mechanism for evaluating
    void boxes" — a number per face, not an eye on a picture)."""
    if not zones:
        return []
    faces = unit_faces(zones)
    return zone_face_rows(zones, faces, face_coverage(prims, faces))


def section_plan(body_lo, body_hi, band):
    """The cuts: (name, axis, level, image axes, flip_v) — three plans
    inside `band` (floor, ceiling), and vertical cuts at the body's
    quarter points along each horizontal axis (a single center cut can
    land inside a beam or a wall and show an empty ceiling where there
    is a void; three never all do)."""
    floor, ceiling = band
    long_axis = 2 if (body_hi[2] - body_lo[2]) >= (body_hi[0] - body_lo[0]) else 0
    short_axis = 2 - long_axis
    cuts = [
        ("plan_low", 1, floor + PLAN_LOW_ABOVE_FLOOR, (0, 2), False),
        ("plan", 1, floor + PLAN_ABOVE_FLOOR, (0, 2), False),
        ("plan_high", 1, ceiling - PLAN_HIGH_BELOW_CEILING, (0, 2), False),
    ]
    for family, axis, along in (("long", short_axis, long_axis), ("cross", long_axis, short_axis)):
        for pct in VERTICAL_CUT_PERCENTS:
            level = body_lo[axis] + (body_hi[axis] - body_lo[axis]) * pct / 100.0
            cuts.append((f"{family}_{pct}", axis, level, (along, 1), True))
    return cuts


def draw_sections(prims, frame_lo, frame_hi, body_lo, body_hi, band, zones, stem):
    """Draw every section image; returns the [[section]] rows."""
    plan = section_plan(body_lo, body_hi, band)
    segments = cut_sections(prims, [(axis, level) for _n, axis, level, _a, _f in plan])
    rows = []
    for (name, axis, level, axes, flip_v), segs in zip(plan, segments):
        raster = Raster(frame_lo, frame_hi, axes, flip_v)
        # Zones first, geometry over them: a box edge along a wall shows
        # the wall; solid over glass: a wall behind a pane reads as a wall.
        draw_zones(raster, zones, axis, level)
        for p, q, blend in segs:
            if blend:
                raster.segment(p, q, GLASS_COLOR, over=True)
        for p, q, blend in segs:
            if not blend:
                raster.segment(p, q, SOLID_COLOR, over=True)
        image = f"{os.path.basename(stem)}_{name}.png"
        raster.write(os.path.join(os.path.dirname(stem), image))
        rows.append({
            "name": name, "image": image, "axis": AXIS_NAMES[axis], "level": level,
            "axes": [AXIS_NAMES[axes[0]], AXIS_NAMES[axes[1]]], "flip_v": flip_v,
            "frame_lo": list(raster.lo), "frame_hi": list(raster.hi),
            "cell": raster.cell, "size": [raster.w, raster.h], "segments": len(segs),
        })
    return rows


def material_lines(rows, table):
    lines = []
    for r in rows:
        lines += [
            f"[[{table}]]",
            f"name = {toml_str(r['name'])}",
            f"alpha_mode = {toml_str(r['alpha_mode'])}",
            f"double_sided = {toml_bool(r['double_sided'])}",
            f"base_texture = {toml_bool(r['base_texture'])}",
            f"metal_rough_texture = {toml_bool(r['metal_rough_texture'])}",
            f"normal_texture = {toml_bool(r['normal_texture'])}",
            f"roughness = {r['roughness']:.3f}",
            f"metallic = {r['metallic']:.3f}",
            f"emissive_strength = {r['emissive_strength']:.3f}",
        ]
        if "alpha" in r:
            lines.append(f"alpha = {r['alpha']:.3f}")
        if "tris" in r:
            lines.append(f"tris = {r['tris']}")
        if r.get("texels_per_m") is not None:
            lines.append(f"texels_per_m = {r['texels_per_m']:.1f}")
        lines.append("")
    return lines


def triangles_by_material(gltf):
    """Triangles each material index carries across the default scene's
    primitives (a material with none is a name in the file, not a
    surface in the level)."""
    counts = {}
    for mesh in gltf.get("meshes", []):
        for prim in mesh.get("primitives", []):
            if prim.get("mode", 4) != 4:
                continue
            acc = prim.get("indices", prim["attributes"]["POSITION"])
            n = gltf["accessors"][acc]["count"] // 3
            counts[prim.get("material")] = counts.get(prim.get("material"), 0) + n
    return counts



INVISIBLE_ALPHA = 0.05  # a blended surface under this alpha renders nothing


def material_alpha(mat):
    """The material's base color alpha (1.0 when the file says nothing)."""
    factor = mat.get("pbrMetallicRoughness", {}).get("baseColorFactor")
    return float(factor[3]) if factor and len(factor) > 3 else 1.0


def invisible_rows(materials, tris_by_material):
    """Materials that carry faces yet render nothing: blended or masked
    with a base alpha under INVISIBLE_ALPHA (owner 2026-09-06: lamp
    chains and seat cushions missing from the hill house — a surface
    that shipped and cannot be seen is told apart here from one that
    never shipped)."""
    rows = []
    for index, mat in enumerate(materials):
        tris = tris_by_material.get(index, 0)
        if not tris or mat.get("alphaMode") not in ("BLEND", "MASK"):
            continue
        alpha = material_alpha(mat)
        if alpha < INVISIBLE_ALPHA:
            rows.append({"name": mat.get("name", "?"), "tris": tris, "alpha": alpha,
                         "alpha_mode": mat.get("alphaMode")})
    return rows


def invisible_lines(rows):
    lines = []
    for r in rows:
        lines += [
            "[[invisible]]",
            f"name = {toml_str(r['name'])}",
            f"alpha_mode = {toml_str(r['alpha_mode'])}",
            f"alpha = {r['alpha']:.3f}",
            f"tris = {r['tris']}",
            "",
        ]
    return lines



def texel_density(prims, gltf, image_sizes):
    """Texels per MODEL meter each textured material lays on its
    surfaces: the square root of (base-color texture pixels x UV area)
    over world area, summed over the material's triangles. A 4096 map
    stretched once across a 10 m wall is 400 texels/m; tiled ten times
    it is 4000. This is the number "low pixel textures" (owner
    2026-09-06) resolves to — the map's size and its mapping together
    — divided by the kit scale for texels per world meter.
    `image_sizes` maps image index -> (width, height). Returns
    {material index: texels per meter} for materials with a base color
    texture, UVs, and area."""
    image_of = {}
    textures = gltf.get("textures", [])
    for i, mat in enumerate(gltf.get("materials", [])):
        base = mat.get("pbrMetallicRoughness", {}).get("baseColorTexture")
        if base is not None and base["index"] < len(textures):
            image_of[i] = textures[base["index"]].get("source")
    world = {}
    uv_area = {}
    for pos, idx, _blend, uv, material in prims:
        if uv is None or material not in image_of:
            continue
        w_sum = world.get(material, 0.0)
        u_sum = uv_area.get(material, 0.0)
        for k in range(0, len(idx) - 2, 3):
            a, b, c = idx[k], idx[k + 1], idx[k + 2]
            a3, b3, c3 = a * 3, b * 3, c * 3
            ux, uy, uz = pos[b3] - pos[a3], pos[b3 + 1] - pos[a3 + 1], pos[b3 + 2] - pos[a3 + 2]
            vx, vy, vz = pos[c3] - pos[a3], pos[c3 + 1] - pos[a3 + 1], pos[c3 + 2] - pos[a3 + 2]
            cx, cy, cz = uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx
            w_sum += math.sqrt(cx * cx + cy * cy + cz * cz) * 0.5
            a2, b2, c2 = a * 2, b * 2, c * 2
            du1, dv1 = uv[b2] - uv[a2], uv[b2 + 1] - uv[a2 + 1]
            du2, dv2 = uv[c2] - uv[a2], uv[c2 + 1] - uv[a2 + 1]
            u_sum += abs(du1 * dv2 - du2 * dv1) * 0.5
        world[material] = w_sum
        uv_area[material] = u_sum
    density = {}
    for material, image in image_of.items():
        size = image_sizes.get(image)
        if size and world.get(material, 0.0) > 0.0:
            density[material] = math.sqrt(size[0] * size[1] * uv_area[material] / world[material])
    return density


def zone_share(prims, zones):
    """Per material, the share of its surface area whose triangles sit
    inside the authored zone union (a triangle counts where its centroid
    lies): 1.0 for a wall the player walks past, 0.0 for a backdrop hill
    past every box. The audit's texel-density floor reaches the surfaces
    whose share is high — a rule about where the player goes, the same
    for every scene, in place of waivers naming levels (owner
    2026-09-07). Empty without zones. Returns {material index: share}."""
    if not zones:
        return {}
    boxes = [(mn, [mn[k] + ex[k] for k in range(3)]) for _key, mn, ex, _start, _boss in zones]
    total, inside = {}, {}
    for pos, idx, _blend, _uv, material in prims:
        t_sum = total.get(material, 0.0)
        i_sum = inside.get(material, 0.0)
        for k in range(0, len(idx) - 2, 3):
            a3, b3, c3 = idx[k] * 3, idx[k + 1] * 3, idx[k + 2] * 3
            ux, uy, uz = pos[b3] - pos[a3], pos[b3 + 1] - pos[a3 + 1], pos[b3 + 2] - pos[a3 + 2]
            vx, vy, vz = pos[c3] - pos[a3], pos[c3 + 1] - pos[a3 + 1], pos[c3 + 2] - pos[a3 + 2]
            cx, cy, cz = uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx
            area = math.sqrt(cx * cx + cy * cy + cz * cz) * 0.5
            t_sum += area
            gx = (pos[a3] + pos[b3] + pos[c3]) / 3.0
            gy = (pos[a3 + 1] + pos[b3 + 1] + pos[c3 + 1]) / 3.0
            gz = (pos[a3 + 2] + pos[b3 + 2] + pos[c3 + 2]) / 3.0
            for lo, hi in boxes:
                if lo[0] <= gx < hi[0] and lo[1] <= gy < hi[1] and lo[2] <= gz < hi[2]:
                    i_sum += area
                    break
        total[material] = t_sum
        inside[material] = i_sum
    return {m: inside[m] / total[m] for m in total if total[m] > 0.0}



# ---------------------------------------------------------------- textures

def image_size(data):
    """(width, height) from a PNG or JPEG header, or None. JPEG: walk the
    marker segments to the first frame header (SOF), skipping fill bytes
    (0xFF runs — the hill house's tile maps carry them and read as size
    0 until 2026-09-07) and stopping at the scan (SOS)."""
    if data[:8] == b"\x89PNG\r\n\x1a\n" and len(data) >= 24:
        return struct.unpack(">II", data[16:24])
    if data[:2] == b"\xff\xd8":
        pos = 2
        while pos + 9 < len(data):
            if data[pos] != 0xFF:
                pos += 1
                continue
            marker = data[pos + 1]
            if marker == 0xFF:
                pos += 1  # fill byte: the next 0xFF starts the marker
                continue
            if marker in (0xD8, 0x01) or 0xD0 <= marker <= 0xD7:
                pos += 2
                continue
            if marker == 0xDA:
                return None  # scan data before any frame header
            length = struct.unpack(">H", data[pos + 2:pos + 4])[0]
            if marker in (0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7,
                          0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF):
                height, width = struct.unpack(">HH", data[pos + 5:pos + 9])
                return width, height
            pos += 2 + length
    return None


def source_images(manifest_path):
    """The shipped source images beside a material manifest (the pack's
    extracted archives), by name — what the converter capped from."""
    if not manifest_path:
        return {}
    unpacked = os.path.join(os.path.dirname(os.path.abspath(manifest_path)), "unpacked")
    if not os.path.isdir(unpacked):
        return {}
    return texture_files(unpacked)


def texture_rows(gltf, binbuf, manifest_path=None):
    """One row per image the scene ships: its name, format, shipped
    pixel size and bytes, the sampler filter its textures use (nearest
    reads as pixels however large the map), and — when the pack's
    extracted archives are beside the manifest — the source image's
    size, so a low-resolution surface can be told apart from a capped
    one (owner 2026-09-06: "are you decimating textures? things look
    very very low res")."""
    sources = source_images(manifest_path)
    filters = {}
    for tex in gltf.get("textures", []):
        sampler = gltf.get("samplers", [{}])[tex["sampler"]] if "sampler" in tex else {}
        mag = sampler.get("magFilter", 9729)
        filters.setdefault(tex.get("source"), "nearest" if mag == 9728 else "linear")
    rows = []
    for index, image in enumerate(gltf.get("images", [])):
        name = image.get("name") or image.get("uri") or "?"
        shipped = None
        nbytes = 0
        if "bufferView" in image:
            view = gltf["bufferViews"][image["bufferView"]]
            start = view.get("byteOffset", 0)
            data = binbuf[start:start + view["byteLength"]]
            nbytes = len(data)
            shipped = image_size(data)
        source = None
        if sources:
            hit = find_in_inventory(name if "." in name else name + ".png", sources) \
                or find_in_inventory(name if "." in name else name + ".jpg", sources)
            if hit:
                with open(sources[hit], "rb") as f:
                    source = image_size(f.read(65536))
        rows.append({
            "name": name, "mime": image.get("mimeType", "?"), "bytes": nbytes,
            "width": shipped[0] if shipped else 0, "height": shipped[1] if shipped else 0,
            "source_width": source[0] if source else 0,
            "source_height": source[1] if source else 0,
            "filter": filters.get(index, "unused"),
        })
    return rows


def texture_lines(rows):
    lines = []
    if rows:
        widest = max(max(r["width"], r["height"]) for r in rows)
        capped = sum(1 for r in rows if r["source_width"]
                     and max(r["source_width"], r["source_height"]) > max(r["width"], r["height"]))
        shipped_px = sum(r["width"] * r["height"] for r in rows)
        source_px = sum((r["source_width"] * r["source_height"]) or (r["width"] * r["height"])
                        for r in rows)
        nearest = sum(1 for r in rows if r["filter"] == "nearest")
        lines += [
            "[textures]",
            "# Every image the scene ships (the converter caps each at its",
            "# --tex-cap on the longest side and packs it); source_* is the",
            "# shipped file's size when the pack's archives sit beside the",
            "# manifest, 0 when unknown. capped = images smaller than their source;",
            "# the megapixel totals are the VRAM story (about 1 byte per pixel",
            "# once Godot compresses them, with mipmaps a third more); nearest =",
            "# images sampled without filtering, which read as pixels at any size.",
            f"count = {len(rows)}",
            f"longest_side = {widest}",
            f"capped = {capped}",
            f"nearest = {nearest}",
            f"shipped_megapixels = {shipped_px / 1e6:.1f}",
            f"source_megapixels = {source_px / 1e6:.1f}",
            f"megabytes = {sum(r['bytes'] for r in rows) / 1e6:.1f}",
            "",
        ]
    for r in rows:
        lines += [
            "[[texture]]",
            f"name = {toml_str(r['name'])}",
            f"mime = {toml_str(r['mime'])}",
            f"size = [{r['width']}, {r['height']}]",
            f"source_size = [{r['source_width']}, {r['source_height']}]",
            f"filter = {toml_str(r['filter'])}",
            f"kb = {r['bytes'] // 1024}",
            "",
        ]
    return lines



def density_lines(materials, tris_by_material, density, share):
    """One row per textured surface with area — EVERY such material,
    not the first LARGEST: the audit's texel-density floor reads this
    table, and a scene's sparse surface is as likely its 3000th
    material as its first. With a zone roster each row carries
    in_zones, the share of its area inside the zone union (zone_share),
    the geometry the floor's reach is read from."""
    lines = []
    if density:
        lines += [
            "# Every textured material with faces: the triangles it carries,",
            "# the texels per model meter its map lays down (the audit holds",
            "# these to a floor per world meter) and, with a roster, in_zones:",
            "# the share of its area inside the zone union — where the floor",
            "# reaches, by geometry.",
            "",
        ]
    for index in sorted(density, key=lambda i: density[i]):
        lines += [
            "[[density]]",
            f"name = {toml_str(materials[index].get('name', '?'))}",
            f"tris = {tris_by_material.get(index, 0)}",
            f"texels_per_m = {density[index]:.1f}",
        ]
        if index in share:
            lines.append(f"in_zones = {share[index]:.3f}")
        lines.append("")
    return lines



def flat_lines(materials, tris_by_material):
    """One row per material with faces and NO base color texture — what
    a surface falls back to when its map never shipped (the hill house's
    chair fabric, chains and ceramic tops, owner 2026-09-07: "materials
    not making the transition"): its mode, alpha and base color, so a
    surface that vanished can be read as it shipped."""
    rows = []
    for index, mat in enumerate(materials):
        tris = tris_by_material.get(index, 0)
        pbr = mat.get("pbrMetallicRoughness", {})
        if not tris or "baseColorTexture" in pbr:
            continue
        color = pbr.get("baseColorFactor", [1.0, 1.0, 1.0, 1.0])
        rows.append((mat.get("name", "?"), tris, mat.get("alphaMode", "OPAQUE"),
                     mat.get("doubleSided", False), color))
    lines = []
    if rows:
        lines += [
            "# Every material with faces and no base color texture: what it",
            "# renders as (mode, alpha, base color), most triangles first.",
            "",
        ]
    for name, tris, mode, two_sided, color in sorted(rows, key=lambda r: -r[1]):
        lines += [
            "[[flat]]",
            f"name = {toml_str(name)}",
            f"tris = {tris}",
            f"alpha_mode = {toml_str(mode)}",
            f"double_sided = {toml_bool(two_sided)}",
            f"base_color = {toml_vec(color[:3])}",
            f"alpha = {float(color[3]) if len(color) > 3 else 1.0:.3f}",
            "",
        ]
    return lines


def main(key, glb_path, out_path, manifest_path=None, zones_path=None):
    gltf, binbuf = glb_chunks(glb_path)
    parts = parse_glb(glb_path, distinct_tris=False)
    lo = [min(p["lo"][k] for p in parts) for k in range(3)]
    hi = [max(p["hi"][k] for p in parts) for k in range(3)]
    materials = gltf.get("materials", [])
    textured = sum(1 for m in materials
                   if "baseColorTexture" in m.get("pbrMetallicRoughness", {}))
    blend = sum(1 for m in materials if m.get("alphaMode") in ("BLEND", "MASK"))
    emissive = sum(1 for m in materials
                   if m.get("extensions", {}).get("KHR_materials_emissive_strength"))
    clone_stems = len({clone_stem(m.get("name", "?")) for m in materials})
    glass_parts = sum(1 for p in parts if p["blend"])
    surfaces_per_mesh = [len(m.get("primitives", [])) for m in gltf.get("meshes", [])]
    tris_by_material = triangles_by_material(gltf)
    unused = sum(1 for i in range(len(materials)) if not tris_by_material.get(i))
    invisible = invisible_rows(materials, tris_by_material)
    textures = texture_rows(gltf, binbuf, manifest_path)
    image_sizes = {i: (t["width"], t["height"]) for i, t in enumerate(textures)
                   if t["width"] and t["height"]}

    # ---- sections: the body percentiles and the authored boxes frame the
    # cuts (a scenery backdrop — the hill house's 175 m — would shrink
    # the room to a smudge otherwise) ----
    prims = load_mesh(gltf, binbuf)
    density = texel_density(prims, gltf, image_sizes)
    body_lo, body_hi, vertex_count = body_percentiles(prims)
    zones = load_zones(zones_path)
    share = zone_share(prims, zones)
    zbox = zones_bounds(zones)
    frame_lo, frame_hi = frame_bounds(prims, body_lo, body_hi, zbox)
    if zbox:
        band, band_source = (zbox[0][1], zbox[1][1]), "zones"
    else:
        band, band_source = (body_lo[1], body_hi[1]), "body"
    stem = os.path.splitext(os.path.abspath(out_path))[0]
    os.makedirs(os.path.dirname(stem), exist_ok=True)
    sections = draw_sections(prims, frame_lo, frame_hi, body_lo, body_hi, band, zones, stem)
    faces = zone_faces(prims, zones)

    lines = [
        "# GENERATED by scripts/scene-metrics.py (make assets / make metrics) — do not edit.",
        "# What the pipeline produced for one scene: extents in model meters",
        "# (glTF Y up), parts, materials (with the triangles each carries and",
        "# the texels per meter its map lays down), the textures it ships",
        "# (shipped size against source size), surviving animations, the",
        "# largest parts by AABB volume, the source file's own geometry counts",
        "# when its material manifest carries them, the section drawings",
        "# beside this file (the scene cut by planes; see [[section]]) and,",
        "# with a zone roster, how much of every walled zone face the model",
        "# backs ([[zone_face]]).",
        "",
        f"key = {toml_str(key)}",
        f"scene = {toml_str(glb_path)}",
        f"size_mb = {os.path.getsize(glb_path) / 1e6:.1f}",
        f"animations = {toml_strs(a.get('name', '?') for a in gltf.get('animations', []))}",
        "",
        "[extents]",
        f"lo = {toml_vec(lo)}",
        f"hi = {toml_vec(hi)}",
        f"size = {toml_vec([hi[k] - lo[k] for k in range(3)])}",
        "# 5th..95th percentile of the vertex cloud per axis: the body of",
        "# the scene, backdrop and outliers aside.",
        f"body_lo = {toml_vec(body_lo)}",
        f"body_hi = {toml_vec(body_hi)}",
        "",
        "[geometry]",
        "# surfaces = glTF primitives, one per material a mesh carries — what",
        "# Godot draws. It keeps RenderingServer.MAX_MESH_SURFACES (256) per",
        "# mesh and drops the rest at import; surfaces_max_per_mesh is the",
        "# number to hold under it.",
        f"parts = {len(parts)}",
        f"surfaces = {sum(surfaces_per_mesh)}",
        f"surfaces_max_per_mesh = {max(surfaces_per_mesh, default=0)}",
        f"glass_parts = {glass_parts}",
        f"tris = {sum(p['tris'] for p in parts)}",
        f"vertices = {vertex_count}",
        "",
        "[materials]",
        "# unused = materials the file names that no triangle carries (a",
        "# pane that never shipped is a name here and a hole in the level);",
        "# invisible = materials with faces that render nothing (blended or",
        f"# masked under alpha {INVISIBLE_ALPHA:g}; listed in [[invisible]]).",
        "# clone_stems = distinct names once the source exporter's clone",
        "# suffix is stripped (material_plan.clone_stem): the ceiling on what",
        "# merging clones would leave.",
        "# texels_per_m_* = over the textured materials, texels per MODEL",
        "# meter from map size x mapping (divide by the kit scale for world",
        "# meters): a 4096 map laid once across a 10 m wall is 400.",
        f"count = {len(materials)}",
        f"textured = {textured}",
        f"flat = {len(materials) - textured}",
        f"blend = {blend}",
        f"emissive = {emissive}",
        f"unused = {unused}",
        f"invisible = {len(invisible)}",
        f"clone_stems = {clone_stems}",
    ]
    if density:
        ordered = sorted(density.values())
        lines += [
            f"texels_per_m_min = {ordered[0]:.1f}",
            f"texels_per_m_median = {ordered[len(ordered) // 2]:.1f}",
            f"texels_per_m_max = {ordered[-1]:.1f}",
        ]
    lines.append("")
    lines += invisible_lines(invisible)
    lines += texture_lines(textures)
    lines += density_lines(materials, tris_by_material, density, share)
    lines += flat_lines(materials, tris_by_material)

    if manifest_path and os.path.isfile(manifest_path):
        with open(manifest_path) as f:
            manifest = json.load(f)
        source = manifest.get("geometry")
        units = manifest.get("units")
        if source or units:
            lines += ["# The SOURCE file as its material manifest read it (raw units and",
                      "# axes, before the converter's unit scale and axis mapping).",
                      "[source]",
                      f"file = {toml_str(manifest.get('source', '?'))}"]
            if units:
                for k, v in sorted(units.items()):
                    lines.append(f"{k} = {v if isinstance(v, (int, float)) else toml_str(v)}")
            if source:
                for k in ("vertices", "faces", "objects", "groups"):
                    if k in source:
                        lines.append(f"{k} = {source[k]}")
                for k in ("lo", "hi", "p05", "p95"):
                    if k in source:
                        lines.append(f"{k} = {toml_vec(source[k])}")
            lines.append("")

    lines += [
        "[sections]",
        "# Section drawings: the scene cut by a plane, solid gray and glass",
        "# blue drawn over the authored zone boxes (start green, boss red",
        "# dotted, others yellow). The plan cuts sit inside cut_band — the",
        "# zones' floor..ceiling when a roster exists (cut_band_source =",
        "# \"zones\"), else the vertex cloud's percentile band. Pixel (i, j) of",
        "# a section image is model (frame_lo[0] + i * cell, frame_lo[1] +",
        "# j * cell) on its `axes`, rows counted from the top — from the",
        "# bottom when flip_v.",
        f"cut_band = {toml_vec(band)}",
        f"cut_band_source = {toml_str(band_source)}",
        f"floor = {body_lo[1]:.3f}",
        f"ceiling = {body_hi[1]:.3f}",
        f"zones_drawn = {len(zones)}",
        f"zone_keys = {toml_strs(z[0] for z in zones)}",
        "",
    ]
    for s in sections:
        lines += [
            "[[section]]",
            f"name = {toml_str(s['name'])}",
            f"image = {toml_str(s['image'])}",
            f"axis = {toml_str(s['axis'])}",
            f"level = {s['level']:.3f}",
            f"axes = {toml_strs(s['axes'])}",
            f"flip_v = {toml_bool(s['flip_v'])}",
            f"frame_lo = {toml_vec(s['frame_lo'])}",
            f"frame_hi = {toml_vec(s['frame_hi'])}",
            f"cell = {s['cell']:.4f}",
            f"size = [{s['size'][0]}, {s['size'][1]}]",
            f"segments = {s['segments']}",
            "",
        ]
    if faces:
        lines += [
            "# Zone faces: every unit-cell face of the zone union no other cell",
            f"# shares — what the containment shell walls — and the fraction of",
            f"# it that model geometry within {FACE_TOLERANCE:g} m of its plane covers.",
            "# A face along a wall, floor, or roof reads near 1.0; a face across",
            f"# a doorway or open floor reads low and its cells (backed under",
            f"# {OPEN_CELL_BELOW:g}) are named in open_at, min-corner cell coordinates.",
            "",
        ]
    for r in faces:
        lines += [
            "[[zone_face]]",
            f"zone = {toml_str(r['zone'])}",
            f"face = {toml_str(r['face'])}",
            f"cells = {r['cells']}",
            f"backed = {r['backed']:.3f}",
            f"open_cells = {r['open_cells']}",
            "open_at = [" + ", ".join(f"[{c[0]}, {c[1]}, {c[2]}]" for c in r["open_at"]) + "]",
            "",
        ]

    for p in sorted(parts, key=volume, reverse=True)[:LARGEST]:
        lines += [
            "[[largest_part]]",
            f"name = {toml_str(p['name'])}",
            f"tris = {p['tris']}",
            f"blend = {toml_bool(p['blend'])}",
            f"lo = {toml_vec(p['lo'])}",
            f"hi = {toml_vec(p['hi'])}",
            "",
        ]
    lines += material_lines([
        {
            "name": m.get("name", "?"),
            "alpha_mode": m.get("alphaMode", "OPAQUE"),
            "double_sided": m.get("doubleSided", False),
            "base_texture": "baseColorTexture" in m.get("pbrMetallicRoughness", {}),
            "metal_rough_texture": "metallicRoughnessTexture" in m.get("pbrMetallicRoughness", {}),
            "normal_texture": "normalTexture" in m,
            "roughness": m.get("pbrMetallicRoughness", {}).get("roughnessFactor", 1.0),
            "metallic": m.get("pbrMetallicRoughness", {}).get("metallicFactor", 1.0),
            "emissive_strength": (m.get("extensions", {})
                                  .get("KHR_materials_emissive_strength", {})
                                  .get("emissiveStrength", 1.0)),
            "alpha": material_alpha(m),
            "tris": tris_by_material.get(i, 0),
            "texels_per_m": density.get(i),
        }
        for i, m in enumerate(materials[:LARGEST])
    ], "material")
    with open(out_path, "w") as f:
        f.write("\n".join(lines))
    open_faces = sum(1 for r in faces if r["open_cells"])
    widest = max((max(t["width"], t["height"]) for t in textures), default=0)
    print(
        f"scene-metrics: {key} {len(parts)} parts, "
        f"{sum(surfaces_per_mesh)} surfaces (most in one mesh {max(surfaces_per_mesh, default=0)}), "
        f"{sum(p['tris'] for p in parts)} tris, extents "
        f"{toml_vec([hi[k] - lo[k] for k in range(3)])} m, "
        f"{len(textures)} textures (longest side {widest}), "
        f"{unused} unused / {len(invisible)} invisible materials, "
        + (f"texels/m {min(density.values()):.0f}..{max(density.values()):.0f}, " if density else "")
        + f"{len(sections)} sections {os.path.basename(stem)}_*.png"
        + (f", {len(faces)} zone faces ({open_faces} with open cells)" if faces else "")
        + f" -> {out_path}"
    )


if __name__ == "__main__":
    main(*sys.argv[1:6])
