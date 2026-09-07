"""OBJ/MTL material oracle for `make assets` (pure Python, no Blender).

    python3 scripts/mtl_materials.py <in.obj> <in.mtl> <out.json>

The Wavefront twin of extract-fbx-materials.py, for packs that ship no
FBX (the hill house is a SketchUp OBJ export). Emits the SAME oracle
shape the material plan and the audit consume, so an MTL pack rides the
plan doors unchanged:

  - every material's authored scalars: Kd -> DiffuseColor, d (or Tr) ->
    TransparencyFactor = 1 - d, Ke -> EmissiveColor,
  - every texture statement as a channel link (map_Kd, map_d, map_Bump /
    bump, map_Ke, ...) resolved to the file's basename, MTL option flags
    ("-bm 0.5", "-clamp on") stripped — the bump multiplier survives as
    mapamountBump, the authored strength the normal plan carries,
  - `assigned` = CARRIES A DISTINCT SOLID FACE, what survives an import:
    an area-bearing face whose vertex set no earlier face already covers.
    SketchUp writes a two-sided face as two polygons over the same
    vertices (one per side material) and Blender's mesh validation keeps
    one, so the second material loses its only face; a `usemtl` with
    only such faces, or none, is an authored leftover the audit must not
    demand (the FBX oracle and the cockpit reader draw the same line),
  - `face_stats` = per material: faces, distinct solid faces, and the
    largest triangle's cross-product magnitude (2x area, file units) —
    what says whether a material the glb lacks was ever visible,
  - `embedded_textures` = [] (OBJ embeds nothing),
  - `geometry` = the raw file's census: vertex/face/object/group counts,
    the vertex AABB, and 5th/95th-percentile extents per axis — a room
    tells itself apart from the backdrop around it (the hill house's one
    mesh spans a 175 km AABB).

Importable: parse_mtl / assigned_in_obj / obj_geometry are the parser;
`main` is the pipeline door.
"""
import json
import os
import sys

# MTL texture statements, all forwarded as channels under their own
# names (material_plan.py owns which channel means what).
TEXTURE_KEYS = {
    "map_Kd", "map_Ka", "map_Ks", "map_Ns", "map_d", "map_Ke",
    "map_Bump", "map_bump", "bump", "disp", "decal", "refl",
}
# Option flags a texture statement may carry before its filename, with
# their argument counts (the MTL spec's "-flag args..." forms).
OPTION_ARGS = {
    "-blendu": 1, "-blendv": 1, "-boost": 1, "-mm": 2, "-o": 3, "-s": 3,
    "-t": 3, "-texres": 1, "-clamp": 1, "-bm": 1, "-imfchan": 1, "-type": 1,
    "-cc": 1,
}
# A triangle whose cross-product magnitude (2x area) falls under this is
# degenerate: renders nothing, and importers legitimately prune it (the
# same bar cockpit_plan.parse_glb holds a round-trip to).
_DEGENERATE_CROSS = 1e-9


def _floats(tokens):
    out = []
    for t in tokens:
        try:
            out.append(float(t))
        except ValueError:
            break
    return out


def _texture_statement(tokens):
    """(basename, options) of a texture statement's argument tokens: the
    filename is everything after the option flags (it may contain
    spaces), reduced to its basename; options keep their float args."""
    options = {}
    i = 0
    while i < len(tokens) and tokens[i] in OPTION_ARGS:
        n = OPTION_ARGS[tokens[i]]
        args = tokens[i + 1:i + 1 + n]
        options[tokens[i]] = _floats(args) or args
        i += 1 + n
    path = " ".join(tokens[i:]).replace("\\", "/")
    return path.rsplit("/", 1)[-1], options


def parse_mtl(text):
    """Material name -> oracle record, from MTL text."""
    materials = {}
    rec = None
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        key, _, rest = line.partition(" ")
        tokens = rest.split()
        if key == "newmtl":
            rec = {"class": "mtl", "props": {}, "channels": {},
                   "children": {}, "assigned": False}
            materials[rest.strip()] = rec
            continue
        if rec is None:
            continue
        if key == "Kd":
            vals = _floats(tokens)
            if len(vals) >= 3:
                rec["props"]["DiffuseColor"] = vals[:3]
        elif key == "Ke":
            vals = _floats(tokens)
            if len(vals) >= 3 and max(vals[:3]) > 0.0:
                rec["props"]["EmissiveColor"] = vals[:3]
                rec["props"].setdefault("EmissiveFactor", 1.0)
        elif key == "d":
            vals = _floats(tokens)
            if vals and vals[0] < 1.0:
                rec["props"]["TransparencyFactor"] = 1.0 - vals[0]
        elif key == "Tr":
            vals = _floats(tokens)
            if vals and vals[0] > 0.0:
                rec["props"].setdefault("TransparencyFactor", vals[0])
        elif key in TEXTURE_KEYS:
            basename, options = _texture_statement(tokens)
            if basename:
                rec["channels"].setdefault(key, basename)
                if key in ("map_Bump", "map_bump", "bump") and "-bm" in options:
                    amount = options["-bm"]
                    if amount and isinstance(amount[0], float):
                        rec["props"]["mapamountBump"] = amount[0]
    return materials


def _face_indices(face_line, nverts):
    """The vertex indices of an OBJ face statement (1-based in the file,
    negative ones relative to the vertices read so far), or [] when the
    face cannot be read."""
    idx = []
    for tok in face_line.split()[1:]:
        head = tok.split("/", 1)[0]
        try:
            i = int(head)
        except ValueError:
            return []
        i = i - 1 if i > 0 else nverts + i
        if i < 0 or i >= nverts:
            return []
        idx.append(i)
    return idx if len(idx) >= 3 else []


def _largest_cross(idx, verts):
    """The largest cross-product magnitude (2x area) among a face's fan
    triangles."""
    best = 0.0
    a = verts[idx[0]]
    for k in range(1, len(idx) - 1):
        b, c = verts[idx[k]], verts[idx[k + 1]]
        u = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
        v = (c[0] - a[0], c[1] - a[1], c[2] - a[2])
        cross = (u[1] * v[2] - u[2] * v[1],
                 u[2] * v[0] - u[0] * v[2],
                 u[0] * v[1] - u[1] * v[0])
        mag = (cross[0] ** 2 + cross[1] ** 2 + cross[2] ** 2) ** 0.5
        if mag > best:
            best = mag
    return best


def _scan_obj(obj_path):
    """One line scan of the OBJ (hundreds of MB): per-material face
    statistics (from which `assigned` derives) and the geometry census.
    Vertices are kept for the face-area test and the percentile extents;
    each face's vertex set is remembered (hashed) so a later polygon over
    the same vertices counts as a duplicate, not a distinct face."""
    current = None
    counts = {"vertices": 0, "faces": 0, "objects": 0, "groups": 0}
    stats = {}
    verts = []
    seen = set()
    with open(obj_path, encoding="utf-8", errors="replace") as f:
        for line in f:
            if line.startswith("v "):
                parts = line.split()
                if len(parts) >= 4:
                    try:
                        verts.append((float(parts[1]), float(parts[2]), float(parts[3])))
                    except ValueError:
                        continue
                    counts["vertices"] += 1
            elif line.startswith("f "):
                counts["faces"] += 1
                if current is None:
                    continue
                rec = stats.setdefault(
                    current, {"faces": 0, "distinct_solid_faces": 0, "max_cross": 0.0})
                rec["faces"] += 1
                idx = _face_indices(line, len(verts))
                if not idx:
                    continue
                cross = _largest_cross(idx, verts)
                if cross > rec["max_cross"]:
                    rec["max_cross"] = cross
                key = hash(tuple(sorted(idx)))
                if cross >= _DEGENERATE_CROSS and key not in seen:
                    seen.add(key)
                    rec["distinct_solid_faces"] += 1
            elif line.startswith("usemtl"):
                current = line[6:].strip()
            elif line.startswith("o "):
                counts["objects"] += 1
            elif line.startswith("g "):
                counts["groups"] += 1
    geometry = dict(counts)
    if verts:
        lo, hi, p05, p95 = [], [], [], []
        for k in range(3):
            vals = sorted(v[k] for v in verts)
            n = len(vals)
            lo.append(vals[0])
            hi.append(vals[-1])
            p05.append(vals[int(0.05 * (n - 1))])
            p95.append(vals[int(0.95 * (n - 1))])
        geometry.update({"lo": lo, "hi": hi, "p05": p05, "p95": p95})
    return stats, geometry


def assigned_in_obj(obj_path):
    """The set of material names the OBJ's faces wear: a `usemtl` block
    with at least one distinct area-bearing face after it."""
    stats, _ = _scan_obj(obj_path)
    return {name for name, s in stats.items() if s["distinct_solid_faces"] > 0}


def obj_geometry(obj_path):
    """The raw file's geometry census (see module doc)."""
    return _scan_obj(obj_path)[1]


def extract(obj_path, mtl_path):
    """The oracle table for one OBJ/MTL pair."""
    with open(mtl_path, encoding="utf-8", errors="replace") as f:
        materials = parse_mtl(f.read())
    stats, geometry = _scan_obj(obj_path)
    for name, rec in materials.items():
        s = stats.get(name)
        rec["assigned"] = s is not None and s["distinct_solid_faces"] > 0
    return {
        "source": os.path.basename(obj_path),
        "materials": materials,
        "embedded_textures": [],
        "geometry": geometry,
        "face_stats": stats,
        "name_collisions": [],
        "unresolved_texture_links": 0,
    }


def main(obj_path, mtl_path, out_path):
    out = extract(obj_path, mtl_path)
    with open(out_path, "w") as f:
        json.dump(out, f, indent=1, sort_keys=True)
    mats = out["materials"]
    g = out["geometry"]
    print(
        f"mtl-materials: {len(mats)} materials "
        f"({sum(1 for r in mats.values() if r['channels'])} with texture links, "
        f"{sum(1 for r in mats.values() if r['assigned'])} assigned); "
        f"{g['vertices']} vertices, {g['faces']} faces, {g['objects']} objects, "
        f"{g['groups']} groups -> {out_path}"
    )


if __name__ == "__main__":
    main(*sys.argv[1:4])
