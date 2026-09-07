"""Scene-census recon audit (`make test-assets`).

The census (scripts/scene-census.py) draws the SECTIONS zone authoring
reads: cut a scene with a horizontal plane and every wall shows as a
line, every doorway as a gap, every cabinet as its footprint — the way
an architect's floor plan reads (owner 2026-09-06: zones drafted from
bounding numbers walled off a hallway that exists and a ceiling that
opens into the next floor; a vertex-density raster showed neither, a
wall being four vertices).

These tests build a small synthetic room as a .glb through the same
glTF layout Blender exports (one mesh, one primitive per material) and
hold the census to the section contract: a doorway is open at door
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


def _load_census():
    spec = importlib.util.spec_from_file_location(
        "scene_census", ROOT / "scripts" / "scene-census.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


census = _load_census()
tomllib = census.tomllib  # the census resolves tomllib/tomli for the venv


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
    on: whichever percentile the census reads, this is where it lands."""
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


def write_glb(path, primitives, node_translation=None):
    """A minimal .glb: one buffer, float32 positions + uint16 indices per
    primitive with declared min/max, one mesh under one node."""
    blob = bytearray()
    views, accessors, prims, materials = [], [], [], []
    for name, tris, alpha in primitives:
        verts, index = [], {}
        indices = []
        for tri in tris:
            for p in tri:
                if p not in index:
                    index[p] = len(verts)
                    verts.append(p)
                indices.append(index[p])
        pos_off = len(blob)
        for p in verts:
            blob += struct.pack("<fff", *p)
        while len(blob) % 4:
            blob += b"\0"
        idx_off = len(blob)
        for i in indices:
            blob += struct.pack("<H", i)
        while len(blob) % 4:
            blob += b"\0"
        views.append({"buffer": 0, "byteOffset": pos_off, "byteLength": len(verts) * 12})
        views.append({"buffer": 0, "byteOffset": idx_off, "byteLength": len(indices) * 2})
        accessors.append({
            "bufferView": len(views) - 2, "componentType": 5126, "count": len(verts),
            "type": "VEC3",
            "min": [min(v[k] for v in verts) for k in range(3)],
            "max": [max(v[k] for v in verts) for k in range(3)],
        })
        accessors.append({"bufferView": len(views) - 1, "componentType": 5123,
                          "count": len(indices), "type": "SCALAR"})
        materials.append({"name": name, "alphaMode": alpha, "doubleSided": True})
        prims.append({"attributes": {"POSITION": len(accessors) - 2},
                      "indices": len(accessors) - 1, "material": len(materials) - 1})
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
    js = json.dumps(gltf).encode()
    while len(js) % 4:
        js += b" "
    body = struct.pack("<II", len(js), 0x4E4F534A) + js
    body += struct.pack("<II", len(blob), 0x004E4942) + bytes(blob)
    path.write_bytes(b"glTF" + struct.pack("<II", 2, 12 + len(body)) + body)


def read_png(path):
    """(width, height, pixels[y][x] = (r, g, b)) — the census writes
    filter-0 rows only."""
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    pos, idat, width = 8, b"", None
    while pos < len(data):
        length, tag = struct.unpack(">I4s", data[pos:pos + 8])
        chunk = data[pos + 8:pos + 8 + length]
        if tag == b"IHDR":
            width, height, depth, ctype = struct.unpack(">IIBB", chunk[:10])
            assert (depth, ctype) == (8, 2), "census recon is 8-bit RGB"
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


def run_census(tmp, key, primitives, zones_text=None, node_translation=None):
    glb = tmp / f"{key}.glb"
    write_glb(glb, primitives, node_translation)
    zones = None
    if zones_text:
        zones = tmp / f"{key}_zones.toml"
        zones.write_text(zones_text)
    out = tmp / "census" / f"{key}.toml"
    census.main(key, str(glb), str(out), None, str(zones) if zones else None)
    with open(out, "rb") as f:
        return tmp, tomllib.load(f)


@pytest.fixture(scope="module")
def built(tmp_path_factory):
    return run_census(tmp_path_factory.mktemp("census"), "room",
                      room_primitives(), zones_toml())


def section(built, name):
    tmp, doc = built
    rows = [s for s in doc["recon"]["section"] if s["name"] == name]
    assert len(rows) == 1, f"the census names exactly one '{name}' section"
    meta = rows[0]
    width, height, pixels = read_png(tmp / "census" / meta["image"])
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


def test_geometry_draws_over_zone_edges_so_a_wall_under_an_edge_shows(built):
    meta, _w, _h, pixels = section(built, "plan")
    # The start box's north and west edges lie on walls: the wall shows.
    assert is_gray(pixel_at(meta, pixels, 2.0, 0.0)), "north wall under the start edge"
    assert is_gray(pixel_at(meta, pixels, 0.0, 1.5)), "west wall under the start edge"
    # Its east edge crosses open floor: the box color shows there.
    assert pixel_at(meta, pixels, 4.0, 1.5) == tuple(census.START_COLOR)
    # The edge the start shares with the plain middle zone draws in the
    # plain color (the later box draws over the earlier).
    assert pixel_at(meta, pixels, 2.0, 3.0) == tuple(census.ZONE_COLOR)
    # The boss box's south edge (z = 6, off any wall) is dotted: its color
    # alternates with the black beneath along the row.
    row_j = int((6.0 - meta["frame_lo"][1]) / meta["cell"])
    row = pixels[row_j]
    assert tuple(census.BOSS_COLOR) in row
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
    tmp, doc = run_census(tmp_path, "storeys", two_storey_primitives())
    assert doc["recon"]["ceiling"] < 4.0, \
        "the fixture's body ceiling is the ground floor's (the rug wins the percentile)"
    for name in sorted(VERTICAL_SECTIONS):
        meta = [s for s in doc["recon"]["section"] if s["name"] == name][0]
        assert meta["frame_hi"][1] > 6.0, f"{name}: the frame reaches the roof"
        _w, _h, pixels = read_png(tmp / "census" / meta["image"])
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
    tmp, doc = run_census(tmp_path, "hill", lifted + terrain, zones)
    assert doc["recon"]["floor"] < 1.0, "the terrain owns the percentile floor"
    assert doc["recon"]["cut_band_source"] == "zones"
    assert doc["recon"]["cut_band"] == pytest.approx([3.0, 6.0])
    meta = [s for s in doc["recon"]["section"] if s["name"] == "plan"][0]
    assert 4.0 < meta["level"] < 5.1, "the plan cut sits at door height in the lifted room"
    _w, _h, pixels = read_png(tmp / "census" / meta["image"])
    assert is_gray(pixel_at(meta, pixels, 0.0, 1.0)), "the lifted west wall prints"
    assert is_gray(pixel_at(meta, pixels, 6.0, 1.0)), "the lifted east wall prints"
    assert not lit(pixel_at(meta, pixels, 3.0, 4.0)), "open floor, no terrain fill"


def test_the_frame_covers_authored_zones_beyond_the_body(tmp_path):
    """A corridor box drafted past the frame's edge must pull the frame
    out with it (the hill house hallway ran off the first sections), so
    the next census shows what the box encloses."""
    zones = zones_toml() + """
[[zone]]
key = "corridor"
box = { min = [2, 0, 8], extents = [2, 3, 12] }
"""
    tmp, doc = run_census(tmp_path, "reach", room_primitives(), zones)
    plan = [s for s in doc["recon"]["section"] if s["name"] == "plan"][0]
    assert plan["frame_hi"][1] >= 20.0, "the plan frame reaches the corridor's far end"
    long_50 = [s for s in doc["recon"]["section"] if s["name"] == "long_50"][0]
    assert long_50["axes"] == ["z", "y"]
    assert long_50["frame_hi"][0] >= 20.0, "so do the long sections"
    _w, _h, pixels = read_png(tmp / "census" / plan["image"])
    assert pixel_at(plan, pixels, 2.0, 14.0) == tuple(census.ZONE_COLOR), \
        "the corridor box draws where it was authored"


def test_the_toml_maps_every_image_back_to_meters(built):
    tmp, doc = built
    names = {s["name"] for s in doc["recon"]["section"]}
    assert names == {"plan_low", "plan", "plan_high"} | VERTICAL_SECTIONS
    for s in doc["recon"]["section"]:
        assert (tmp / "census" / s["image"]).is_file()
        assert s["cell"] > 0
        assert len(s["frame_lo"]) == 2 and len(s["frame_hi"]) == 2
        assert s["frame_lo"][0] < s["frame_hi"][0]
        assert s["frame_lo"][1] < s["frame_hi"][1]
    assert doc["recon"]["zones_drawn"] == 3


def test_node_transforms_reach_the_sections(tmp_path):
    """A translated node must cut where it lands, not where it was
    authored — the census walks node transforms like the probe does."""
    tmp, doc = run_census(tmp_path, "moved", room_primitives(),
                          node_translation=(10.0, 0.0, -20.0))
    meta = [s for s in doc["recon"]["section"] if s["name"] == "plan"][0]
    _w, _h, pixels = read_png(tmp / "census" / meta["image"])
    assert lit(pixel_at(meta, pixels, 10.0, -20.0 + 1.0)), "west wall, moved"
    assert not lit(pixel_at(meta, pixels, 10.0 + 3.0, -20.0 + 4.0)), "open floor, moved"
    assert doc["recon"]["zones_drawn"] == 0


# ---------------------------------------------------------------- zone faces

def zone_face(doc, zone, face):
    rows = [r for r in doc["recon"].get("zone_face", [])
            if r["zone"] == zone and r["face"] == face]
    assert len(rows) == 1, f"one row per zone face, got {rows} for {zone} {face}"
    return rows[0]


def test_zone_faces_report_how_much_geometry_backs_them(built):
    """The containment shell walls every box face no other box shares;
    the census measures how much of each such face lies on model
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
    assert not [r for r in doc["recon"]["zone_face"]
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
    _tmp, doc = run_census(tmp_path, "door", room_primitives(), zones)
    south = zone_face(doc, "room", "+z")
    assert south["cells"] == 18, "6 wide x 3 tall"
    # The doorway column (x 2..3) is open in its two lower cells and
    # mostly backed in the lintel cell; everything else is wall.
    assert 0.8 < south["backed"] < 0.95
    assert south["open_cells"] == 2
    east = zone_face(doc, "room", "+x")
    assert east["backed"] > 0.95, "glass backs a face like a wall does"
