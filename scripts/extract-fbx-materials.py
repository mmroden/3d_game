"""FBX material manifest extractor for `make assets`, run via Blender (parse
only, no scene import — seconds, not minutes).

    blender --background --python scripts/extract-fbx-materials.py -- \\
        <in.fbx> <out.json>

Dumps the COMPLETE material truth from the FBX connection tables into one
JSON artifact (generated, gitignored — reproducible from the committed FBX):

  - every material's class (CoronaMtl / CoronaPhysicalMtl / LightMtl /
    RaySwitchMtl / LayeredMtl / standard),
  - every texture link with its authored channel name, resolved through
    Corona's wrapper-texture chains to a real file basename,
  - the property subset the material plan needs (transparency stack,
    authored colors, emission),
  - container edges (RaySwitchMtl directMtl / LayeredMtl baseMtl / ...):
    3ds Max container materials whose sub-materials Blender's FBX importer
    NEVER instantiates — the source of the blank-grey floor/cabinets,
  - whether the material is assigned to any model,
  - which texture files the FBX EMBEDS (Video elements carrying Content
    bytes): they ship inside the file, with no loose copy, and count as
    shipped inventory exactly like the archive's files,
  - the file's declared UNITS and axes (GlobalSettings: UnitScaleFactor,
    OriginalUnitScaleFactor, up/front/coord axes) — what Blender's
    importer scales by, and the first thing to read when a scene comes
    out 1.4 m tall (the mountain villa, 2026-09-06).

scripts/material_plan.py turns this into per-material plans (pure Python,
unit-tested by `make test-assets`); scripts/convert-environment.py and
scripts/convert-hull.py apply the plans in Blender. This extractor is the
ONLY place the FBX gets parsed (mtl_materials.py is its OBJ/MTL twin).
"""
import json
import os
import sys

from io_scene_fbx import parse_fbx  # Blender's own binary-FBX parser

argv = sys.argv[sys.argv.index("--") + 1:]
in_path, out_path = argv[0], argv[1]


def fbx_name(raw):
    """FBX names are b'Name\\x00\\x01Class' — return the Name part."""
    return raw.split(b"\x00", 1)[0].decode("utf-8", "replace")


# Properties worth carrying (matched against the segment after the last
# '|', so both standard and 3dsMax|CoronaMtlPb|-prefixed spellings land).
# color-valued entries keep [r,g,b]; scalar entries keep a float.
PROP_COLORS = {"DiffuseColor", "EmissiveColor", "colorDiffuse", "baseColor"}
PROP_SCALARS = {
    "TransparencyFactor", "EmissiveFactor",
    "levelRefract", "levelOpacity", "refractGlossiness",
    "opacityLevel", "refractionAmount",
    "thin", "useThinMode", "multiplier",
    # Authored map amounts (per-slot blend fractions): Corona blends each
    # map over the material's base value at this fraction. Ignoring them
    # exaggerates — a wall authored as 95% flat plaster + 5% grunge map
    # renders as pure grunge (owner 2026-07-12).
    "mapamountBump", "baseBumpMapAmount",
    "mapamountDiffuse", "mapamountReflectGlossiness",
}

# Material class, betrayed by its property prefixes.
CLASS_PREFIXES = [
    ("3dsMax|CoronaPhysicalMtlPb|", "CoronaPhysicalMtl"),
    ("3dsMax|CoronaMtlPb|", "CoronaMtl"),
    ("3dsMax|LightMtlPb|", "LightMtl"),
]
# Container class, betrayed by its material->material edge channels.
CONTAINER_PREFIXES = [
    ("3dsMax|RaySwitchMtlPb|", "RaySwitchMtl"),
    ("3dsMax|LayeredMtlPb|", "LayeredMtl"),
]
# GlobalSettings properties worth reporting (numbers as authored).
UNIT_PROPS = {
    "UnitScaleFactor": "unit_scale_factor",
    "OriginalUnitScaleFactor": "original_unit_scale_factor",
    "UpAxis": "up_axis", "UpAxisSign": "up_axis_sign",
    "FrontAxis": "front_axis", "FrontAxisSign": "front_axis_sign",
    "CoordAxis": "coord_axis", "CoordAxisSign": "coord_axis_sign",
}

elem_root, _ = parse_fbx.parse(in_path)
objects_elem = next(e for e in elem_root.elems if e.id == b"Objects")
connections_elem = next(e for e in elem_root.elems if e.id == b"Connections")

# ---- units and axes, as the file declares them ----
units = {}
settings_elem = next((e for e in elem_root.elems if e.id == b"GlobalSettings"), None)
if settings_elem is not None:
    props70 = next((c for c in settings_elem.elems if c.id == b"Properties70"), None)
    for p in (props70.elems if props70 is not None else []):
        key = p.props[0].decode("utf-8", "replace")
        if key in UNIT_PROPS:
            vals = [x for x in p.props[4:] if isinstance(x, (int, float))]
            if vals:
                units[UNIT_PROPS[key]] = vals[0]

textures = {}   # uid -> texture element name
materials = {}  # uid -> material name
models = set()  # model uids
mesh_models = set()  # models that carry geometry — only these can SHOW a material
videos = {}     # uid -> file basename (with extension)
embedded = set()  # basenames whose Video element carries the file's bytes
mat_records = {}  # uid -> record under construction
geo_slots = {}  # geometry uid -> per-polygon material slot indices (or [0])

for e in objects_elem.elems:
    if e.id == b"Texture":
        textures[e.props[0]] = fbx_name(e.props[1])
    elif e.id == b"Model":
        models.add(e.props[0])
        klass = e.props[2].decode("utf-8", "replace") if len(e.props) > 2 else ""
        if klass == "Mesh":
            mesh_models.add(e.props[0])
    elif e.id == b"Geometry":
        lem = next((c for c in e.elems if c.id == b"LayerElementMaterial"), None)
        arr = None
        if lem is not None:
            mats_elem = next((c for c in lem.elems if c.id == b"Materials"), None)
            if mats_elem is not None and mats_elem.props:
                arr = mats_elem.props[0]
        geo_slots[e.props[0]] = arr if arr is not None else [0]
    elif e.id == b"Video":
        raw = next(
            (c.props[0] for c in e.elems
             if c.id in (b"FileName", b"Filename", b"RelativeFilename") and c.props[0]),
            b"",
        )
        if raw:
            basename = raw.decode("utf-8", "replace").replace("\\", "/").rsplit("/", 1)[-1]
            videos[e.props[0]] = basename
            content = next((c.props[0] for c in e.elems
                            if c.id == b"Content" and c.props), b"")
            if isinstance(content, (bytes, bytearray)) and len(content) > 0:
                embedded.add(basename)
    elif e.id == b"Material":
        name = fbx_name(e.props[1])
        materials[e.props[0]] = name
        props = {}
        klass = "standard"
        props70 = next((c for c in e.elems if c.id == b"Properties70"), None)
        for p in (props70.elems if props70 is not None else []):
            key = p.props[0].decode("utf-8", "replace")
            for prefix, k in CLASS_PREFIXES:
                if key.startswith(prefix) and klass == "standard":
                    klass = k
            short = key.rsplit("|", 1)[-1]
            vals = [float(x) for x in p.props[4:] if isinstance(x, (int, float))]
            if short in PROP_COLORS and len(vals) >= 3:
                props.setdefault(short, vals[:3])
            elif short in PROP_SCALARS and vals:
                props.setdefault(short, vals[0])
        mat_records[e.props[0]] = {
            "class": klass,
            "props": props,
            "channels": {},
            "children": {},
            "assigned": False,
        }

# ---- connections ----
# Corona wraps bitmaps in adapter nodes (normal/mix/color-correct), and
# the adapters are NOT always Texture elements — so the file search walks
# the connection graph type-agnostically: any feeding node may be, or may
# lead to, the Video element that carries the real filename.
node_children = {}  # dst uid -> [feeding src uids], every connection kind
mat_mat_edges = []  # (child uid, parent uid, channel)
model_slots = {}    # mesh model uid -> [material uids, slot order = connection order]
model_geo = {}      # mesh model uid -> geometry uid
for c in connections_elem.elems:
    if c.id != b"C" or len(c.props) < 3:
        continue
    kind, src, dst = c.props[0], c.props[1], c.props[2]
    chan = c.props[3].decode("utf-8", "replace") if len(c.props) > 3 else ""
    if src in mat_records and dst in mat_records and kind == b"OP":
        mat_mat_edges.append((src, dst, chan))
    elif src in mat_records and dst in mesh_models:
        model_slots.setdefault(dst, []).append(src)
    elif src in geo_slots and dst in mesh_models:
        model_geo.setdefault(dst, src)
    elif dst in mat_records and kind == b"OP":
        mat_records[dst]["channels"].setdefault(chan, src)  # resolve below
    else:
        node_children.setdefault(dst, []).append(src)

# "assigned" means CARRIES FACES: a material in a slot no polygon indexes
# is an authored leftover — the glTF exporter (rightly) drops it, and the
# audit must not demand it ship. The geometry's per-polygon slot indices
# are the truth (a single-entry array means every face wears slot 0).
for model_uid, slots in model_slots.items():
    geo_uid = model_geo.get(model_uid)
    indices = geo_slots.get(geo_uid, [0])
    used = set(indices) if len(indices) > 1 else {indices[0] if len(indices) else 0}
    for slot_idx in used:
        if 0 <= slot_idx < len(slots):
            mat_records[slots[slot_idx]]["assigned"] = True
        elif slots:
            mat_records[slots[0]]["assigned"] = True  # out-of-range: FBX clamps

_resolved = {}


def resolve_file(uid, depth=0):
    """The link's file basename, walking feeding nodes until a Video."""
    if uid in videos:
        return videos[uid]
    if uid in _resolved:
        return _resolved[uid]
    if depth >= 12:
        return None
    for child in node_children.get(uid, []):
        if child in mat_records or child in models:
            continue
        f = resolve_file(child, depth + 1)
        if f is not None:
            _resolved[uid] = f  # memoize
            return f
    return None


unresolved_links = 0
for rec in mat_records.values():
    resolved = {}
    for chan, tex_uid in rec["channels"].items():
        f = resolve_file(tex_uid)
        if f is None:
            unresolved_links += 1
        else:
            resolved[chan] = f
    rec["channels"] = resolved

for child_uid, parent_uid, chan in mat_mat_edges:
    parent = mat_records[parent_uid]
    role = chan.rsplit("|", 1)[-1]  # directMtl / baseMtl / giMtl / layers[N]
    parent["children"].setdefault(role, materials[child_uid])
    for prefix, k in CONTAINER_PREFIXES:
        if chan.startswith(prefix):
            parent["class"] = k

# ---- merge by display name (distinct FBX materials can share one) ----
# Blender's imported materials carry the display name, so the plan is
# keyed by it. On collision the record with more information wins
# (assigned > channels > first); collisions are reported, never silent.
by_name = {}
collisions = []


def richness(rec):
    return (rec["assigned"], len(rec["channels"]) + len(rec["children"]), len(rec["props"]))


for uid, rec in mat_records.items():
    name = materials[uid]
    held = by_name.get(name)
    if held is None:
        by_name[name] = rec
    else:
        collisions.append(name)
        if richness(rec) > richness(held):
            by_name[name] = rec

out = {
    "source": os.path.basename(in_path),
    "materials": by_name,
    "embedded_textures": sorted(embedded),
    "units": units,
    "name_collisions": sorted(set(collisions)),
    "unresolved_texture_links": unresolved_links,
}
with open(out_path, "w") as f:
    json.dump(out, f, indent=1, sort_keys=True)

n_container = sum(1 for r in by_name.values() if r["children"])
n_textured = sum(1 for r in by_name.values() if r["channels"])
print(
    f"extract-fbx-materials: {len(by_name)} materials "
    f"({n_textured} with texture links, {n_container} containers, "
    f"{len(embedded)} embedded textures, "
    f"{len(set(collisions))} name collisions, "
    f"{unresolved_links} unresolvable texture links; "
    f"unit scale {units.get('unit_scale_factor', '?')}) -> {out_path}"
)
