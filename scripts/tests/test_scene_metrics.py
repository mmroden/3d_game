"""Scene metrics section audit (`make test-assets`).

The metrics tool (scripts/scene-metrics.py) draws the SECTIONS zone
authoring reads: cut a scene with a horizontal plane and every wall
shows as a line, every doorway as a gap, every cabinet as its footprint
— the way an architect's floor plan reads (owner 2026-09-06: zones
drafted from bounding numbers walled off a hallway that exists and a
ceiling that opens into the next floor; a vertex-density raster showed
neither, a wall being four vertices).

These tests build a small synthetic room as a .glb through the same
glTF layout Blender exports (one mesh, one primitive per material) and
hold the metrics to the section contract: a doorway is open at door
height and closed above the lintel, low furniture prints on the low cut
only, glass draws distinct from walls, geometry draws over the authored
zone boxes (a box edge along a wall shows the wall), the cuts follow the
zones' height band, the frame reaches the zones and the upper storey,
and the TOML names every image with its cut so a reader can map pixels
back to meters.
"""
import importlib.util
import json
import struct
import sys
import zlib
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))


def _load_metrics():
    spec = importlib.util.spec_from_file_location(
        "scene_metrics", ROOT / "scripts" / "scene-metrics.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


metrics = _load_metrics()
tomllib = metrics.tomllib  # the metrics tool resolves tomllib/tomli for the venv


# ---------------------------------------------------------------- fixtures
# The room: x 0..6, z 0..8 (model meters, glTF Y up), floor at y 0,
# ceiling at y 3. Solid walls all round except a 1 m doorway in the
# south wall (z = 8) between x 2..3 with its lintel from y 2.1 up, a
# glass pane in the east wall (x = 6) between z 3..5 from the floor to
# the ceiling, and a 1 m square cabinet 0.75 m tall around (1.5, 1.5).
FLOOR_Y, CEIL_Y = 0.0, 3.0
DOOR = (2.0, 3.0, 2.1)  # x0, x1, lintel height
PANE = (3.0, 5.0)  # z0, z1 on the east wall
CABINET = (1.0, 2.0, 1.0, 2.0, 0.75)  # x0, x1, z0, z1, top height

VERTICAL_SECTIONS = {"long_25", "long_50", "long_75", "cross_25", "cross_50", "cross_75"}


def quad(a, b, c, d):
    """Two triangles for the quad a-b-c-d (any winding; a section does
    not care which way a wall faces)."""
    return [(a, b, c), (a, c, d)]


def wall_x(x, z0, z1, y0, y1):
    return quad((x, y0, z0), (x, y0, z1), (x, y1, z1), (x, y1, z0))


def wall_z(z, x0, x1, y0, y1):
    return quad((x0, y0, z), (x1, y0, z), (x1, y1, z), (x0, y1, z))


def slab_y(y, x0, x1, z0, z1):
    return quad((x0, y, z0), (x1, y, z0), (x1, y, z1), (x0, y, z1))


def box(x0, x1, y0, y1, z0, z1):
    return (wall_x(x0, z0, z1, y0, y1) + wall_x(x1, z0, z1, y0, y1)
            + wall_z(z0, x0, x1, y0, y1) + wall_z(z1, x0, x1, y0, y1)
            + slab_y(y0, x0, x1, z0, z1) + slab_y(y1, x0, x1, z0, z1))


def room_primitives():
    """(name, triangles, alpha_mode) per material, as Blender exports a
    scene: one primitive per material."""
    walls = []
    walls += wall_z(0.0, 0.0, 6.0, FLOOR_Y, CEIL_Y)  # north
    walls += wall_x(0.0, 0.0, 8.0, FLOOR_Y, CEIL_Y)  # west
    # East wall with the pane cut out of it.
    walls += wall_x(6.0, 0.0, PANE[0], FLOOR_Y, CEIL_Y)
    walls += wall_x(6.0, PANE[1], 8.0, FLOOR_Y, CEIL_Y)
    # South wall with the doorway: two jambs and the lintel.
    walls += wall_z(8.0, 0.0, DOOR[0], FLOOR_Y, CEIL_Y)
    walls += wall_z(8.0, DOOR[1], 6.0, FLOOR_Y, CEIL_Y)
    walls += wall_z(8.0, DOOR[0], DOOR[1], DOOR[2], CEIL_Y)
    slabs = slab_y(FLOOR_Y, 0.0, 6.0, 0.0, 8.0) + slab_y(CEIL_Y, 0.0, 6.0, 0.0, 8.0)
    x0, x1, z0, z1, top = CABINET
    cabinet = box(x0, x1, FLOOR_Y, top, z0, z1)
    pane = wall_x(6.0, PANE[0], PANE[1], FLOOR_Y, CEIL_Y)
    return [
        ("Wall", walls, "OPAQUE"),
        ("Slab", slabs, "OPAQUE"),
        ("Cabinet", cabinet, "OPAQUE"),
        ("Glass", pane, "BLEND"),
    ]


def rug_primitives(x0, x1, z0, z1, y, n=16):
    """A dense grid of little slabs at height y — the vertex-heavy
    ground floor of an archviz scene, or a terrain the house stands
    on: whichever percentile the metrics read, this is where it lands."""
    rug = []
    dx, dz = (x1 - x0) / n, (z1 - z0) / n
    for i in range(n):
        for k in range(n):
            rug += slab_y(y, x0 + i * dx, x0 + (i + 0.75) * dx,
                          z0 + k * dz, z0 + (k + 0.75) * dz)
    return [("Rug", rug, "OPAQUE")]


def two_storey_primitives():
    """The room plus an upper storey: side walls carried up to y 6 and a
    roof slab there, with a dense rug on the ground floor so the ground
    floor holds nearly every vertex — the body's y percentile stays at
    the ground ceiling, as in an archviz house whose upper floor is a
    bare shell."""
    upper = wall_x(0.0, 0.0, 8.0, CEIL_Y, 6.0) + wall_x(6.0, 0.0, 8.0, CEIL_Y, 6.0)
    upper += slab_y(6.0, 0.0, 6.0, 0.0, 8.0)
    return room_primitives() + [("Upper", upper, "OPAQUE")] \
        + rug_primitives(0.5, 5.5, 0.5, 7.5, 0.01)


def zones_toml():
    """Three zones down the room's length, inset two meters from the east
    wall (x < 4) and the south wall (z < 6) so wall probes there read the
    geometry, not a box edge, and those faces hang in open room beyond
    the face audit's reach: the start's north edge lies on the north
    wall, the plain middle shares edges with both, the boss ends short
    of the doorway wall."""
    return """
[environment]
key = "room"
model = "room"

[[zone]]
key = "north"
box = { min = [0, 0, 0], extents = [4, 3, 3] }
start = true
links = ["middle"]

[[zone]]
key = "middle"
box = { min = [0, 0, 3], extents = [4, 3, 2] }
links = ["south"]
enemy_spawns = [[2.0, 1.5, 4.0]]

[[zone]]
key = "south"
box = { min = [0, 0, 5], extents = [4, 3, 1] }
boss = true
enemy_spawns = [[2.0, 1.5, 5.5]]
"""


def write_glb(path, primitives, node_translation=None, images=None, uvs=None):
    """A minimal .glb: one buffer, float32 positions + uint16 indices per
    primitive with declared min/max, one mesh under one node. Each
    primitive is (name, triangles, alpha mode[, base alpha]); a fourth
    element writes the material's baseColorFactor alpha. `images` =
    [(name, mime, bytes, material name)]: each is embedded in the
    buffer and wired as that material's base color texture, the way the
    converter packs a scene's maps. `uvs` = {primitive name: [(u, v)
    triple per triangle]} adds a TEXCOORD_0 set to those primitives
    (vertices then dedupe by position AND uv)."""
    blob = bytearray()
    views, accessors, prims, materials = [], [], [], []
    for primitive in primitives:
        name, tris, alpha = primitive[:3]
        base_alpha = primitive[3] if len(primitive) > 3 else None
        tri_uvs = (uvs or {}).get(name)
        verts, index = [], {}
        indices = []
        for t, tri in enumerate(tris):
            for corner, p in enumerate(tri):
                uv = tuple(tri_uvs[t][corner]) if tri_uvs else None
                key = (p, uv)
                if key not in index:
                    index[key] = len(verts)
                    verts.append((p, uv))
                indices.append(index[key])
        pos_off = len(blob)
        for p, _uv in verts:
            blob += struct.pack("<fff", *p)
        while len(blob) % 4:
            blob += b"\0"
        uv_off = len(blob)
        if tri_uvs:
            for _p, uv in verts:
                blob += struct.pack("<ff", *uv)
            while len(blob) % 4:
                blob += b"\0"
        idx_off = len(blob)
        for i in indices:
            blob += struct.pack("<H", i)
        while len(blob) % 4:
            blob += b"\0"
        views.append({"buffer": 0, "byteOffset": pos_off, "byteLength": len(verts) * 12})
        accessors.append({
            "bufferView": len(views) - 1, "componentType": 5126, "count": len(verts),
            "type": "VEC3",
            "min": [min(v[0][k] for v in verts) for k in range(3)],
            "max": [max(v[0][k] for v in verts) for k in range(3)],
        })
        attributes = {"POSITION": len(accessors) - 1}
        if tri_uvs:
            views.append({"buffer": 0, "byteOffset": uv_off, "byteLength": len(verts) * 8})
            accessors.append({"bufferView": len(views) - 1, "componentType": 5126,
                              "count": len(verts), "type": "VEC2"})
            attributes["TEXCOORD_0"] = len(accessors) - 1
        views.append({"buffer": 0, "byteOffset": idx_off, "byteLength": len(indices) * 2})
        accessors.append({"bufferView": len(views) - 1, "componentType": 5123,
                          "count": len(indices), "type": "SCALAR"})
        material = {"name": name, "alphaMode": alpha, "doubleSided": True}
        if base_alpha is not None:
            material["pbrMetallicRoughness"] = {"baseColorFactor": [1.0, 1.0, 1.0, base_alpha]}
        materials.append(material)
        prims.append({"attributes": attributes,
                      "indices": len(accessors) - 1, "material": len(materials) - 1})
    gltf_images, textures = [], []
    for img_name, mime, data, material in images or []:
        off = len(blob)
        blob += data
        while len(blob) % 4:
            blob += b"\0"
        views.append({"buffer": 0, "byteOffset": off, "byteLength": len(data)})
        gltf_images.append({"name": img_name, "mimeType": mime, "bufferView": len(views) - 1})
        textures.append({"source": len(gltf_images) - 1})
        for m in materials:
            if m["name"] == material:
                m.setdefault("pbrMetallicRoughness", {})["baseColorTexture"] = {
                    "index": len(textures) - 1}
    node = {"name": "Room", "mesh": 0}
    if node_translation:
        node["translation"] = list(node_translation)
    gltf = {
        "asset": {"version": "2.0"},
        "scene": 0, "scenes": [{"nodes": [0]}], "nodes": [node],
        "meshes": [{"name": "Room", "primitives": prims}],
        "materials": materials, "accessors": accessors,
        "bufferViews": views, "buffers": [{"byteLength": len(blob)}],
    }
    if gltf_images:
        gltf["images"] = gltf_images
        gltf["textures"] = textures
    js = json.dumps(gltf).encode()
    while len(js) % 4:
        js += b" "
    body = struct.pack("<II", len(js), 0x4E4F534A) + js
    body += struct.pack("<II", len(blob), 0x004E4942) + bytes(blob)
    path.write_bytes(b"glTF" + struct.pack("<II", 2, 12 + len(body)) + body)


def read_png(path):
    """(width, height, pixels[y][x] = (r, g, b)) — the metrics tool
    writes filter-0 rows only."""
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    pos, idat, width = 8, b"", None
    while pos < len(data):
        length, tag = struct.unpack(">I4s", data[pos:pos + 8])
        chunk = data[pos + 8:pos + 8 + length]
        if tag == b"IHDR":
            width, height, depth, ctype = struct.unpack(">IIBB", chunk[:10])
            assert (depth, ctype) == (8, 2), "sections are 8-bit RGB"
        elif tag == b"IDAT":
            idat += chunk
        pos += 12 + length
    raw = zlib.decompress(idat)
    stride = 1 + width * 3
    rows = []
    for j in range(height):
        row = raw[j * stride:(j + 1) * stride]
        assert row[0] == 0, "filter type 0"
        rows.append([tuple(row[1 + 3 * i:4 + 3 * i]) for i in range(width)])
    return width, height, rows


def run_metrics(tmp, key, primitives, zones_text=None, node_translation=None):
    glb = tmp / f"{key}.glb"
    write_glb(glb, primitives, node_translation)
    zones = None
    if zones_text:
        zones = tmp / f"{key}_zones.toml"
        zones.write_text(zones_text)
    out = tmp / "metrics" / f"{key}.toml"
    metrics.main(key, str(glb), str(out), None, str(zones) if zones else None)
    with open(out, "rb") as f:
        return tmp, tomllib.load(f)


@pytest.fixture(scope="module")
def built(tmp_path_factory):
    return run_metrics(tmp_path_factory.mktemp("metrics"), "room",
                       room_primitives(), zones_toml())


def section(built, name):
    tmp, doc = built
    rows = [s for s in doc["section"] if s["name"] == name]
    assert len(rows) == 1, f"the metrics name exactly one '{name}' section"
    meta = rows[0]
    width, height, pixels = read_png(tmp / "metrics" / meta["image"])
    return meta, width, height, pixels


def pixel_at(meta, pixels, u, v):
    """The pixel holding model point (u, v) on the section's image axes
    (the TOML's frame and cell map meters to pixels; vertical sections
    draw v upward)."""
    cell = meta["cell"]
    i = int((u - meta["frame_lo"][0]) / cell)
    j = int((v - meta["frame_lo"][1]) / cell)
    if meta["flip_v"]:
        j = len(pixels) - 1 - j
    return pixels[j][i]


def lit(px):
    return max(px) > 0


def is_gray(px):
    return lit(px) and px[0] == px[1] == px[2]


# ---------------------------------------------------------------- contracts

def test_the_plan_cuts_at_door_height_and_reads_the_doorway_open(built):
    meta, _w, _h, pixels = section(built, "plan")
    assert meta["axis"] == "y"
    assert FLOOR_Y + 1.0 < meta["level"] < DOOR[2], \
        "the plan cut sits between waist and lintel — where doors are open"
    # Jambs: lit either side of the doorway on the south wall.
    assert is_gray(pixel_at(meta, pixels, 1.0, 8.0))
    assert is_gray(pixel_at(meta, pixels, 5.0, 8.0))
    # The doorway itself: dark on this cut.
    for x in (2.2, 2.5, 2.8):
        assert not lit(pixel_at(meta, pixels, x, 8.0)), \
            f"the doorway at x={x} must read open at door height"
    # The east wall is a continuous line off the pane; the room is open.
    for z in (1.0, 2.5, 7.0):
        assert is_gray(pixel_at(meta, pixels, 6.0, z)), "east wall"
    assert not lit(pixel_at(meta, pixels, 5.5, 7.5)), "open floor"


def test_the_high_cut_closes_the_doorway_at_its_lintel(built):
    meta, _w, _h, pixels = section(built, "plan_high")
    assert DOOR[2] < meta["level"] < CEIL_Y
    for x in (2.2, 2.5, 2.8):
        assert is_gray(pixel_at(meta, pixels, x, 8.0)), \
            "above the lintel the south wall is continuous"


def test_low_furniture_prints_on_the_low_cut_only(built):
    low, _w, _h, low_px = section(built, "plan_low")
    plan, _w, _h, plan_px = section(built, "plan")
    x0, x1, z0, z1, top = CABINET
    assert FLOOR_Y < low["level"] < top < plan["level"]
    assert is_gray(pixel_at(low, low_px, x0, 1.5)), "cabinet's west face, low cut"
    assert is_gray(pixel_at(low, low_px, 1.5, z1)), "cabinet's south face, low cut"
    assert not lit(pixel_at(plan, plan_px, x0, 1.5)), "nothing at door height there"
    assert not lit(pixel_at(low, low_px, 1.5, 1.5)), "the cabinet's top is not a cut"
    # Slabs lie in no cut: a section is lines on black, never a floor fill.
    dark = sum(1 for row in plan_px for px in row if not lit(px))
    assert dark > 0.8 * len(plan_px) * len(plan_px[0])


def test_glass_draws_distinct_from_walls(built):
    meta, _w, _h, pixels = section(built, "plan")
    wall = pixel_at(meta, pixels, 6.0, 1.0)
    pane = pixel_at(meta, pixels, 6.0, 4.0)
    assert lit(wall) and lit(pane)
    assert is_gray(wall), "solid geometry draws neutral"
    assert not is_gray(pane), "glass draws in its own tint"



def test_materials_report_the_triangles_they_carry(built):
    """A material in the glb with no faces is a name, not a surface: the
    villa's "Windows Glass" is listed yet its openings show the void
    (owner 2026-09-06) — the count says whether the panes ever shipped."""
    _tmp, doc = built
    by_name = {m["name"]: m for m in doc["material"]}
    assert by_name["Glass"]["tris"] == 2, "one pane, two triangles"
    assert by_name["Wall"]["tris"] == 14, "seven wall quads"
    assert by_name["Slab"]["tris"] == 4
    assert by_name["Cabinet"]["tris"] == 12
    assert doc["materials"]["unused"] == 0, "every fixture material carries faces"



def test_invisible_materials_are_listed(tmp_path):
    """A blended material under alpha 0.05 with faces renders nothing:
    the metrics name it (the hill house's wine display shipped at 0.03
    across 41,724 triangles, owner 2026-09-06). Opaque materials and
    blended ones with visible alpha are not listed."""
    ghost = wall_z(4.0, 1.0, 5.0, FLOOR_Y, CEIL_Y)
    veil = wall_z(6.0, 1.0, 5.0, FLOOR_Y, CEIL_Y)
    prims = room_primitives() + [("Ghost", ghost, "BLEND", 0.03), ("Veil", veil, "BLEND", 0.4)]
    glb = tmp_path / "ghost.glb"
    write_glb(glb, prims)
    out = tmp_path / "metrics" / "ghost.toml"
    metrics.main("ghost", str(glb), str(out), None, None)
    with open(out, "rb") as f:
        doc = tomllib.load(f)
    assert doc["materials"]["invisible"] == 1
    assert [r["name"] for r in doc["invisible"]] == ["Ghost"]
    assert doc["invisible"][0]["tris"] == 2
    assert doc["invisible"][0]["alpha"] == pytest.approx(0.03)
    by_name = {m["name"]: m for m in doc["material"]}
    assert by_name["Ghost"]["alpha"] == pytest.approx(0.03)
    assert by_name["Veil"]["alpha"] == pytest.approx(0.4), "a blended veil at 0.4 is visible"
    assert by_name["Glass"]["alpha"] == 1.0, "a blended pane at full alpha is visible"


def test_flat_materials_are_listed_as_they_render(tmp_path):
    """"Materials not making the transition, not textures" (owner
    2026-09-07): a surface whose map never shipped renders as its base
    color, so every material with faces and no base color texture is
    listed with its mode, alpha and color, most triangles first — a
    seat that vanished can be read as it shipped. Textured materials
    and materials no triangle carries stay off the table."""
    seat = box(1.0, 2.0, FLOOR_Y, FLOOR_Y + 0.5, 1.0, 2.0)
    prims = room_primitives() + [("Seat", seat, "OPAQUE", 0.9), ("Chain", [seat[0]], "BLEND")]
    glb = tmp_path / "flat.glb"
    write_glb(glb, prims, images=[
        ("Wall", "image/png", png_bytes(tmp_path, "wall", 16, 16), "Wall")])
    out = tmp_path / "metrics" / "flat.toml"
    metrics.main("flat", str(glb), str(out), None, None)
    with open(out, "rb") as f:
        doc = tomllib.load(f)
    table = doc["flat"]
    names = [r["name"] for r in table]
    assert "Wall" not in names, "a textured material is not flat"
    counts = [r["tris"] for r in table]
    assert counts == sorted(counts, reverse=True), "most triangles first"
    assert names[-1] == "Chain", "the one-triangle chain is last"
    by_name = {r["name"]: r for r in table}
    assert by_name["Seat"]["tris"] == 12
    assert by_name["Seat"]["alpha_mode"] == "OPAQUE"
    assert by_name["Seat"]["double_sided"] is True
    assert by_name["Seat"]["base_color"] == pytest.approx([1.0, 1.0, 1.0])
    assert by_name["Seat"]["alpha"] == pytest.approx(0.9)
    assert by_name["Chain"]["tris"] == 1
    assert by_name["Chain"]["alpha_mode"] == "BLEND"
    assert by_name["Chain"]["alpha"] == 1.0, "no factor means opaque white"
    assert len(table) == doc["materials"]["flat"], "the table is the flat count"


def test_the_metrics_count_surfaces_per_mesh_and_clone_stems(tmp_path):
    """Godot keeps RenderingServer.MAX_MESH_SURFACES (256) surfaces per
    mesh and drops the rest with an error — one glTF primitive is one
    surface, one primitive per material with faces. The hill house
    shipped one mesh with 11,571 materials, so 11,315 of them never
    reached the level (owner 2026-09-07: "materials not making the
    transition"). The metrics count the surfaces a scene ships and the
    most any one mesh carries, and the distinct names once the source
    exporter's clone suffix is stripped (Seat_1_, Seat_2_ -> Seat) —
    the ceiling on what merging clones would leave."""
    seat = box(1.0, 2.0, FLOOR_Y, FLOOR_Y + 0.5, 1.0, 2.0)
    prims = room_primitives() + [("Seat_1_", seat[:6], "OPAQUE"), ("Seat_2_", seat[6:], "OPAQUE")]
    glb = tmp_path / "surfaces.glb"
    write_glb(glb, prims)
    out = tmp_path / "metrics" / "surfaces.toml"
    metrics.main("surfaces", str(glb), str(out), None, None)
    with open(out, "rb") as f:
        doc = tomllib.load(f)
    assert doc["geometry"]["surfaces"] == len(prims), "one surface per primitive"
    assert doc["geometry"]["surfaces_max_per_mesh"] == len(prims), "one mesh carries them all"
    assert doc["materials"]["count"] == len(prims)
    assert doc["materials"]["clone_stems"] == len(prims) - 1, "the two seats share a stem"



def jpeg_header(width, height, fill=0, progressive=False):
    """The bytes of a JPEG up to its frame header: SOI, a JFIF APP0
    segment, `fill` padding 0xFF bytes (writers pad before markers), a
    baseline or progressive SOF, then the scan marker."""
    app0 = b"JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00"
    sof = struct.pack(">HBHHB", 8 + 3 * 3, 8, height, width, 3) + bytes(
        [1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1])
    return (b"\xff\xd8" + b"\xff\xe0" + struct.pack(">H", len(app0) + 2) + app0
            + b"\xff" * fill + (b"\xff\xc2" if progressive else b"\xff\xc0") + sof
            + b"\xff\xda" + b"\x00\x0c" + b"\x03" * 10)


def test_image_size_reads_png_and_every_jpeg_frame_shape():
    """The texture table's sizes come from the file headers alone: PNG's
    IHDR, and a JPEG's first frame header whether baseline or progressive,
    padded with fill bytes or not (84 shipped hill house maps read as
    0 x 0 before the fill-byte case was handled, 2026-09-07)."""
    png = (b"\x89PNG\r\n\x1a\n" + b"\0\0\0\rIHDR" + struct.pack(">II", 640, 480)
           + b"\x08\x02\0\0\0")
    assert metrics.image_size(png) == (640, 480)
    assert metrics.image_size(jpeg_header(1024, 768)) == (1024, 768)
    assert metrics.image_size(jpeg_header(2048, 1536, fill=3)) == (2048, 1536)
    assert metrics.image_size(jpeg_header(300, 200, progressive=True)) == (300, 200)
    assert metrics.image_size(b"\xff\xd8\xff\xda\x00\x0c" + b"\x00" * 20) is None, \
        "a scan before any frame header is not a size"
    assert metrics.image_size(b"not an image") is None



def test_textured_materials_report_texels_per_meter(tmp_path):
    """"A lot of low pixel textures" (owner 2026-09-06, the villa) is a
    number per material: texels per model meter from the map's size and
    the mapping together. One pane 2 x 3 m with a 16 x 8 map laid once
    across it carries sqrt(128 / 6) texels per meter; the same map tiled
    twice each way carries twice that; an untextured wall reports none.
    The [[density]] table lists every textured surface, sparsest first,
    for the audit's floor."""
    pane = wall_x(6.0, PANE[0], PANE[1], FLOOR_Y, CEIL_Y)  # 2 m by 3 m
    tiled = wall_x(0.0, PANE[0], PANE[1], FLOOR_Y, CEIL_Y)
    once = [[(0, 0), (1, 0), (1, 1)], [(0, 0), (1, 1), (0, 1)]]
    twice = [[(0, 0), (2, 0), (2, 2)], [(0, 0), (2, 2), (0, 2)]]
    glb = tmp_path / "density.glb"
    write_glb(glb, room_primitives() + [("Pane", pane, "OPAQUE"), ("Tiled", tiled, "OPAQUE")],
              images=[("Map", "image/png", png_bytes(tmp_path, "map", 16, 8), "Pane"),
                      ("Map2", "image/png", png_bytes(tmp_path, "map2", 16, 8), "Tiled")],
              uvs={"Pane": once, "Tiled": twice})
    out = tmp_path / "metrics" / "density.toml"
    metrics.main("density", str(glb), str(out), None, None)
    with open(out, "rb") as f:
        doc = tomllib.load(f)
    by_name = {m["name"]: m for m in doc["material"]}
    assert by_name["Pane"]["texels_per_m"] == pytest.approx((128 / 6) ** 0.5, rel=0.02)
    assert by_name["Tiled"]["texels_per_m"] == pytest.approx(2 * (128 / 6) ** 0.5, rel=0.02)
    assert "texels_per_m" not in by_name["Wall"], "no map, no density"
    assert doc["materials"]["texels_per_m_min"] == pytest.approx((128 / 6) ** 0.5, rel=0.02)
    table = doc["density"]
    assert [r["name"] for r in table] == ["Pane", "Tiled"], "every textured surface, sparsest first"
    assert table[0]["tris"] == 2
    assert table[1]["texels_per_m"] == pytest.approx(2 * (128 / 6) ** 0.5, rel=0.02)


def test_density_rows_say_how_much_of_each_surface_lies_in_the_zones(tmp_path):
    """The density floor is a rule about where the player walks: every
    [[density]] row carries in_zones, the share of the surface's area
    whose triangles sit inside the authored zone union, so a backdrop
    hill or a tree card past the rooms falls out of the floor by
    geometry — one rule for every scene, no per-level waivers (owner
    2026-09-07: "genericise your import methodologies"). Without a
    roster the key is absent."""
    y = FLOOR_Y + 1.0
    inside = slab_y(y, 1.0, 3.0, 1.0, 2.0)      # within the north box (x < 4, z < 3)
    straddle = slab_y(y, 3.0, 5.0, 1.0, 2.0)    # its east edge at x 4 splits the patch
    far = slab_y(y, 50.0, 52.0, 1.0, 2.0)       # a backdrop, past every box
    once = [[(0, 0), (1, 0), (1, 1)], [(0, 0), (1, 1), (0, 1)]]
    prims = room_primitives() + [("Inside", inside, "OPAQUE"), ("Straddle", straddle, "OPAQUE"),
                                 ("Far", far, "OPAQUE")]
    images = [(n, "image/png", png_bytes(tmp_path, n.lower(), 16, 16), n)
              for n in ("Inside", "Straddle", "Far")]
    uvs = {"Inside": once, "Straddle": once, "Far": once}
    glb = tmp_path / "share.glb"
    write_glb(glb, prims, images=images, uvs=uvs)
    zones = tmp_path / "share_zones.toml"
    zones.write_text(zones_toml())
    out = tmp_path / "metrics" / "share.toml"
    metrics.main("share", str(glb), str(out), None, str(zones))
    with open(out, "rb") as f:
        doc = tomllib.load(f)
    by_name = {r["name"]: r for r in doc["density"]}
    assert by_name["Inside"]["in_zones"] == pytest.approx(1.0)
    assert by_name["Far"]["in_zones"] == pytest.approx(0.0)
    assert by_name["Straddle"]["in_zones"] == pytest.approx(0.5), "half its area past the box edge"
    bare = tmp_path / "metrics" / "bare.toml"
    metrics.main("bare", str(glb), str(bare), None, None)
    with open(bare, "rb") as f:
        assert all("in_zones" not in r for r in tomllib.load(f)["density"]), "no roster, no share"


def test_geometry_draws_over_zone_edges_so_a_wall_under_an_edge_shows(built):
    meta, _w, _h, pixels = section(built, "plan")
    # The start box's north and west edges lie on walls: the wall shows.
    assert is_gray(pixel_at(meta, pixels, 2.0, 0.0)), "north wall under the start edge"
    assert is_gray(pixel_at(meta, pixels, 0.0, 1.5)), "west wall under the start edge"
    # Its east edge crosses open floor: the box color shows there.
    assert pixel_at(meta, pixels, 4.0, 1.5) == tuple(metrics.START_COLOR)
    # The edge the start shares with the plain middle zone draws in the
    # plain color (the later box draws over the earlier).
    assert pixel_at(meta, pixels, 2.0, 3.0) == tuple(metrics.ZONE_COLOR)
    # The boss box's south edge (z = 6, off any wall) is dotted: its color
    # alternates with the black beneath along the row.
    row_j = int((6.0 - meta["frame_lo"][1]) / meta["cell"])
    row = pixels[row_j]
    assert tuple(metrics.BOSS_COLOR) in row
    assert (0, 0, 0) in row[:int((4.0 - meta["frame_lo"][0]) / meta["cell"])]


def test_vertical_sections_show_floor_and_ceiling(built):
    levels = {}
    for name in sorted(VERTICAL_SECTIONS):
        meta, _w, _h, pixels = section(built, name)
        assert meta["axis"] in ("x", "z")
        assert meta["flip_v"], "vertical sections draw y up"
        # Probe past the zone boxes (x < 5, z < 7) so the lines are geometry.
        u = 5.5 if meta["axis"] == "z" else 7.5
        assert is_gray(pixel_at(meta, pixels, u, FLOOR_Y)), f"{name}: floor line"
        assert is_gray(pixel_at(meta, pixels, u, CEIL_Y)), f"{name}: ceiling line"
        assert not lit(pixel_at(meta, pixels, u, 1.5)), f"{name}: open room between"
        levels[name] = meta["level"]
    for family, axis, span in (("long", "x", 6.0), ("cross", "z", 8.0)):
        # Three cuts at the body's quarter points, so a beam or a wall
        # that swallows one cut never hides a whole room.
        for pct in (25, 50, 75):
            assert section(built, f"{family}_{pct}")[0]["axis"] == axis
        assert 0.0 < levels[f"{family}_25"] < levels[f"{family}_50"] \
            < levels[f"{family}_75"] < span
    assert section(built, "long_50")[0]["axis"] == "x", \
        "the long sections cut across the SHORT horizontal axis (x, 6 m) " \
        "so the room's 8 m length lies along the image"


def test_vertical_frames_reach_an_upper_storey(tmp_path):
    tmp, doc = run_metrics(tmp_path, "storeys", two_storey_primitives())
    assert doc["sections"]["ceiling"] < 4.0, \
        "the fixture's body ceiling is the ground floor's (the rug wins the percentile)"
    for name in sorted(VERTICAL_SECTIONS):
        meta = [s for s in doc["section"] if s["name"] == name][0]
        assert meta["frame_hi"][1] > 6.0, f"{name}: the frame reaches the roof"
        _w, _h, pixels = read_png(tmp / "metrics" / meta["image"])
        u = 5.5 if meta["axis"] == "z" else 7.5
        assert is_gray(pixel_at(meta, pixels, u, 6.0)), f"{name}: roof line"
        assert is_gray(pixel_at(meta, pixels, u, CEIL_Y)), f"{name}: ground ceiling"


def test_plan_cuts_follow_the_zone_band_over_a_terrain(tmp_path):
    """A house on a hill: the terrain holds the vertex cloud, so its
    percentile floor is the ground and a cut there lands inside the
    hill — empty. With a zone roster the cuts sit inside the zones'
    own band, where the rooms are claimed to be."""
    lifted = [(n, [tuple((x, y + 3.0, z) for x, y, z in tri) for tri in tris], a)
              for n, tris, a in room_primitives()]
    terrain = rug_primitives(-6.0, 12.0, -6.0, 14.0, 0.0, n=32)
    zones = """
[environment]
key = "hill"
model = "hill"

[[zone]]
key = "room"
box = { min = [0, 3, 0], extents = [5, 3, 7] }
start = true
"""
    tmp, doc = run_metrics(tmp_path, "hill", lifted + terrain, zones)
    assert doc["sections"]["floor"] < 1.0, "the terrain owns the percentile floor"
    assert doc["sections"]["cut_band_source"] == "zones"
    assert doc["sections"]["cut_band"] == pytest.approx([3.0, 6.0])
    meta = [s for s in doc["section"] if s["name"] == "plan"][0]
    assert 4.0 < meta["level"] < 5.1, "the plan cut sits at door height in the lifted room"
    _w, _h, pixels = read_png(tmp / "metrics" / meta["image"])
    assert is_gray(pixel_at(meta, pixels, 0.0, 1.0)), "the lifted west wall prints"
    assert is_gray(pixel_at(meta, pixels, 6.0, 1.0)), "the lifted east wall prints"
    assert not lit(pixel_at(meta, pixels, 3.0, 4.0)), "open floor, no terrain fill"


def test_the_frame_covers_authored_zones_beyond_the_body(tmp_path):
    """A corridor box drafted past the frame's edge must pull the frame
    out with it (the hill house hallway ran off the first sections), so
    the next run shows what the box encloses."""
    zones = zones_toml() + """
[[zone]]
key = "corridor"
box = { min = [2, 0, 8], extents = [2, 3, 12] }
"""
    tmp, doc = run_metrics(tmp_path, "reach", room_primitives(), zones)
    plan = [s for s in doc["section"] if s["name"] == "plan"][0]
    assert plan["frame_hi"][1] >= 20.0, "the plan frame reaches the corridor's far end"
    long_50 = [s for s in doc["section"] if s["name"] == "long_50"][0]
    assert long_50["axes"] == ["z", "y"]
    assert long_50["frame_hi"][0] >= 20.0, "so do the long sections"
    _w, _h, pixels = read_png(tmp / "metrics" / plan["image"])
    assert pixel_at(plan, pixels, 2.0, 14.0) == tuple(metrics.ZONE_COLOR), \
        "the corridor box draws where it was authored"


def test_the_toml_maps_every_image_back_to_meters(built):
    tmp, doc = built
    names = {s["name"] for s in doc["section"]}
    assert names == {"plan_low", "plan", "plan_high"} | VERTICAL_SECTIONS
    for s in doc["section"]:
        assert (tmp / "metrics" / s["image"]).is_file()
        assert s["cell"] > 0
        assert len(s["frame_lo"]) == 2 and len(s["frame_hi"]) == 2
        assert s["frame_lo"][0] < s["frame_hi"][0]
        assert s["frame_lo"][1] < s["frame_hi"][1]
    assert doc["sections"]["zones_drawn"] == 3


def test_node_transforms_reach_the_sections(tmp_path):
    """A translated node must cut where it lands, not where it was
    authored — the metrics walk node transforms like the probe does."""
    tmp, doc = run_metrics(tmp_path, "moved", room_primitives(),
                           node_translation=(10.0, 0.0, -20.0))
    meta = [s for s in doc["section"] if s["name"] == "plan"][0]
    _w, _h, pixels = read_png(tmp / "metrics" / meta["image"])
    assert lit(pixel_at(meta, pixels, 10.0, -20.0 + 1.0)), "west wall, moved"
    assert not lit(pixel_at(meta, pixels, 10.0 + 3.0, -20.0 + 4.0)), "open floor, moved"
    assert doc["sections"]["zones_drawn"] == 0



# ---------------------------------------------------------------- textures

def png_bytes(tmp, name, width, height):
    path = tmp / f"{name}.png"
    metrics.write_png(str(path), width, height,
                      [bytes([90, 90, 90] * width) for _ in range(height)])
    return path.read_bytes()


def test_the_metrics_list_every_shipped_texture_with_its_source_size(tmp_path):
    """"Are you decimating textures?" (owner 2026-09-06) is answered per
    image: the size the scene ships next to the size the pack shipped,
    the sampler filter, and the megapixel totals — so a surface that
    looks soft can be told capped-by-the-door from low-resolution-at-
    the-source from sampled-without-filtering."""
    pack = tmp_path / "pack"
    (pack / "unpacked").mkdir(parents=True)
    (pack / "unpacked" / "Wall.png").write_bytes(png_bytes(tmp_path, "src_wall", 1024, 512))
    (pack / "unpacked" / "Glass.png").write_bytes(png_bytes(tmp_path, "src_glass", 256, 256))
    manifest = pack / "fbx_materials.json"
    manifest.write_text("{}")
    glb = tmp_path / "tex.glb"
    write_glb(glb, room_primitives(), images=[
        ("Wall", "image/png", png_bytes(tmp_path, "wall", 512, 256), "Wall"),
        ("Glass", "image/png", png_bytes(tmp_path, "glass", 256, 256), "Glass"),
    ])
    out = tmp_path / "metrics" / "tex.toml"
    metrics.main("tex", str(glb), str(out), str(manifest), None)
    with open(out, "rb") as f:
        doc = tomllib.load(f)
    summary = doc["textures"]
    assert summary["count"] == 2
    assert summary["longest_side"] == 512
    assert summary["capped"] == 1, "the wall shipped smaller than its source"
    assert summary["nearest"] == 0, "no sampler declared means linear"
    # Totals are written to a tenth of a megapixel — the scale real scenes read at.
    assert summary["shipped_megapixels"] == round((512 * 256 + 256 * 256) / 1e6, 1)
    assert summary["source_megapixels"] == round((1024 * 512 + 256 * 256) / 1e6, 1)
    by_name = {t["name"]: t for t in doc["texture"]}
    assert by_name["Wall"]["size"] == [512, 256]
    assert by_name["Wall"]["source_size"] == [1024, 512]
    assert by_name["Wall"]["filter"] == "linear"
    assert by_name["Glass"]["size"] == [256, 256]
    assert by_name["Glass"]["source_size"] == [256, 256]
    assert doc["materials"]["textured"] == 2


# ---------------------------------------------------------------- zone faces

def zone_face(doc, zone, face):
    rows = [r for r in doc.get("zone_face", [])
            if r["zone"] == zone and r["face"] == face]
    assert len(rows) == 1, f"one row per zone face, got {rows} for {zone} {face}"
    return rows[0]


def test_zone_faces_report_how_much_geometry_backs_them(built):
    """The containment shell walls every box face no other box shares;
    the metrics measure how much of each such face lies on model
    geometry (within a meter of its plane), so 'does this box follow the
    model' is a number per face, not an eye on a picture (owner
    2026-09-06: "you need a mechanism for evaluating void boxes")."""
    _tmp, doc = built
    north = zone_face(doc, "north", "-z")
    assert north["cells"] == 12, "4 wide x 3 tall unit faces on the north wall"
    assert north["backed"] > 0.95, "the north wall backs the start box's north face"
    assert north["open_cells"] == 0
    assert zone_face(doc, "north", "-x")["backed"] > 0.95, "west wall"
    assert zone_face(doc, "north", "-y")["backed"] > 0.95, "floor slab"
    assert zone_face(doc, "north", "+y")["backed"] > 0.95, "ceiling slab"
    # The boxes stop two meters short of the east and south walls: those
    # faces hang in open room — the shell would wall the room off there.
    east = zone_face(doc, "north", "+x")
    assert east["backed"] < 0.05 and east["open_cells"] == east["cells"]
    assert east["open_at"][0] == [3, 0, 0], "open cells name their min corner"
    south = zone_face(doc, "south", "+z")
    assert south["backed"] < 0.05 and south["open_cells"] == south["cells"]
    # Shared faces are not walled and not reported.
    assert not [r for r in doc["zone_face"]
                if r["zone"] == "north" and r["face"] == "+z"]


def test_a_doorway_under_a_zone_face_shows_as_open_cells(tmp_path):
    """A box whose south face lies on the doorway wall: the jambs back it,
    the doorway does not — the open cells name where the box seals a
    passage (the hill house's walled-off hallway, draft 1)."""
    zones = """
[environment]
key = "door"
model = "door"

[[zone]]
key = "room"
box = { min = [0, 0, 0], extents = [6, 3, 8] }
start = true
"""
    _tmp, doc = run_metrics(tmp_path, "door", room_primitives(), zones)
    south = zone_face(doc, "room", "+z")
    assert south["cells"] == 18, "6 wide x 3 tall"
    # The doorway column (x 2..3) is open in its two lower cells and
    # mostly backed in the lintel cell; everything else is wall.
    assert 0.8 < south["backed"] < 0.95
    assert south["open_cells"] == 2
    east = zone_face(doc, "room", "+x")
    assert east["backed"] > 0.95, "glass backs a face like a wall does"
