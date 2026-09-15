"""The OBJ/MTL material manifest extractor (scripts/mtl_materials.py)
and the plan vocabulary it feeds. Pure: the parser is production code
fed a small MTL/OBJ pair written by the test — the same statements the
hill house's SketchUp export uses — and the plan is built by the real
material_plan.
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from material_plan import (  # noqa: E402
    build_plans, clone_stem, explain_texture_usage, find_in_inventory,
)
from mtl_materials import assigned_in_obj, extract, parse_mtl  # noqa: E402

MTL = """\
# Exported from SketchUp
newmtl Paint_Grey
Ka 0.000000 0.000000 0.000000
Kd 0.427451 0.427451 0.427451
Ks 0.330000 0.330000 0.330000
map_Kd Some Folder/Paint_Grey.jpg

newmtl Glass
Kd 0.576471 0.576471 0.576471
d 0.310000

newmtl Lamp
Kd 1.000000 1.000000 1.000000
Ke 2.000000 2.000000 2.000000
map_Ke Some Folder/Lamp_Glow.png

newmtl Lace
Kd 1.000000 1.000000 1.000000
map_Kd Lace.jpg
map_d -clamp on Lace_Alpha.png

newmtl Brick
Kd 0.8 0.8 0.8
map_Kd Brick.jpg
map_Bump -bm 0.5 Brick_Bump.jpg

newmtl Leftover
Kd 0.5 0.5 0.5
map_Kd Leftover.jpg
"""

# A strip of triangles, one DISTINCT face per material (a face shared by
# two materials is the two-sided case the manifest discounts).
OBJ = """\
# faces wear these
mtllib scene.mtl
v 0 0 0
v 1 0 0
v 0 1 0
v 1 1 0
v 2 0 0
v 2 1 0
v 3 0 0
usemtl Paint_Grey
f 1 2 3
usemtl Glass
f 2 4 3
usemtl Lamp
f 2 5 4
usemtl Lace
f 5 6 4
usemtl Brick
f 5 7 6
"""


def test_parse_mtl_reads_scalars_and_texture_channels():
    mats = parse_mtl(MTL)
    assert set(mats) == {"Paint_Grey", "Glass", "Lamp", "Lace", "Brick", "Leftover"}
    grey = mats["Paint_Grey"]
    assert grey["class"] == "mtl"
    assert grey["props"]["DiffuseColor"] == [0.427451, 0.427451, 0.427451]
    assert grey["channels"] == {"map_Kd": "Paint_Grey.jpg"}, \
        "the channel carries the file's basename, folders stripped"
    assert abs(mats["Glass"]["props"]["TransparencyFactor"] - 0.69) < 1e-6, \
        "d is opacity; the manifest speaks TransparencyFactor = 1 - d"
    assert "TransparencyFactor" not in grey["props"], "no invented transparency"
    assert mats["Lamp"]["props"]["EmissiveColor"] == [2.0, 2.0, 2.0]
    assert mats["Lamp"]["channels"]["map_Ke"] == "Lamp_Glow.png"


def test_parse_mtl_strips_option_flags_from_texture_statements():
    mats = parse_mtl(MTL)
    assert mats["Lace"]["channels"]["map_d"] == "Lace_Alpha.png"
    assert mats["Brick"]["channels"]["map_Bump"] == "Brick_Bump.jpg"
    assert mats["Brick"]["props"]["mapamountBump"] == 0.5, \
        "-bm is the authored bump amount, kept for the normal plan's strength"


def test_assigned_is_the_usemtl_set(tmp_path):
    obj = tmp_path / "scene.obj"
    obj.write_text(OBJ)
    assert assigned_in_obj(obj) == {"Paint_Grey", "Glass", "Lamp", "Lace", "Brick"}


def test_extract_matches_the_fbx_manifest_shape(tmp_path):
    obj = tmp_path / "scene.obj"
    mtl = tmp_path / "scene.mtl"
    obj.write_text(OBJ)
    mtl.write_text(MTL)
    table = extract(obj, mtl)
    assert set(table) >= {"source", "materials", "embedded_textures",
                          "name_collisions", "unresolved_texture_links"}
    assert table["materials"]["Leftover"]["assigned"] is False
    assert table["materials"]["Brick"]["assigned"] is True
    for rec in table["materials"].values():
        assert set(rec) == {"class", "props", "channels", "children", "assigned"}


def test_assigned_means_carries_faces(tmp_path):
    # Same definition as the FBX manifest's: a `usemtl` no face follows
    # is an authored leftover the exporter drops, and the audit must not
    # demand it from the glb.
    obj = tmp_path / "scene.obj"
    obj.write_text(
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nv 1 1 0\n"
        "usemtl Worn\nf 1 2 3\n"
        "usemtl Empty\n"
        "usemtl AlsoWorn\nf 2 4 3\n"
    )
    assert assigned_in_obj(obj) == {"Worn", "AlsoWorn"}


def test_assigned_means_carries_a_solid_face(tmp_path):
    # A face with no area renders nothing and importers legitimately drop
    # it: "assigned" means at least one area-bearing face, like the
    # cockpit reader's distinct_solid_tris.
    obj = tmp_path / "scene.obj"
    obj.write_text(
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nv 2 0 0\n"
        "usemtl Solid\nf 1 2 3\n"
        "usemtl Sliver\nf 1 2 4\n"      # three collinear points: zero area
        "usemtl Quad\nf 1 2 3 4\n"      # a polygon fans into triangles
    )
    assert assigned_in_obj(obj) == {"Solid", "Quad"}


def test_assigned_means_a_distinct_solid_face(tmp_path):
    # SketchUp writes a two-sided face as two polygons over the SAME
    # vertices, one per side material; Blender's mesh validation keeps
    # one polygon per vertex set, so the second material loses its only
    # face (the hill house's 39 sofa materials, 2026-09-06: one ~35 mm²
    # face each, gone at import). "assigned" means a solid face no
    # earlier face already covers — what survives an import.
    obj = tmp_path / "scene.obj"
    obj.write_text(
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nv 1 1 0\n"
        "usemtl Front\nf 1 2 3\n"
        "usemtl Back\nf 3 2 1\n"          # the same vertex set, reversed
        "usemtl Other\nf 2 4 3\n"
    )
    assert assigned_in_obj(obj) == {"Front", "Other"}


def test_extract_reports_the_obj_geometry(tmp_path):
    # The counts of the raw file: vertices, faces, the vertex AABB, and
    # the 5th/95th-percentile extents that tell a room from the backdrop
    # around it (the hill house's single mesh spans 175 km of AABB).
    obj = tmp_path / "scene.obj"
    mtl = tmp_path / "scene.mtl"
    mtl.write_text("newmtl A\nKd 1 1 1\n")
    lines = ["o Room", "g Walls"]
    for i in range(100):
        lines.append(f"v {i} 0 0")
    lines += ["v 100000 5 5", "usemtl A", "f 1 2 3", "f 2 3 4"]
    obj.write_text("\n".join(lines) + "\n")
    geometry = extract(obj, mtl)["geometry"]
    assert geometry["vertices"] == 101
    assert geometry["faces"] == 2
    assert geometry["objects"] == 1 and geometry["groups"] == 1
    assert geometry["lo"] == [0.0, 0.0, 0.0] and geometry["hi"] == [100000.0, 5.0, 5.0]
    assert geometry["p95"][0] < 200, "the far outlier vertex does not set the body's extent"
    assert geometry["p05"][0] >= 0.0


def test_extract_reports_per_material_face_stats(tmp_path):
    # For every material the OBJ wears: how many faces, how many of them
    # distinct and solid, and the largest triangle (cross-product
    # magnitude, 2x area, in the file's units) — the numbers that say
    # whether a material the glb lacks was ever visible.
    obj = tmp_path / "scene.obj"
    mtl = tmp_path / "scene.mtl"
    mtl.write_text("newmtl Big\nKd 1 1 1\nnewmtl Tiny\nKd 1 1 1\n")
    obj.write_text(
        "v 0 0 0\nv 2 0 0\nv 0 2 0\nv 0.001 0 0\nv 0 0.001 0\n"
        "usemtl Big\nf 1 2 3\nf 1 2 3\n"
        "usemtl Tiny\nf 1 4 5\n"
    )
    stats = extract(obj, mtl)["face_stats"]
    assert stats["Big"]["faces"] == 2
    assert stats["Big"]["distinct_solid_faces"] == 1, "the repeated face counts once"
    assert abs(stats["Big"]["max_cross"] - 4.0) < 1e-9
    assert stats["Tiny"]["faces"] == 1
    assert abs(stats["Tiny"]["max_cross"] - 1e-6) < 1e-12


def test_mtl_records_plan_through_the_shared_vocabulary(tmp_path):
    obj = tmp_path / "scene.obj"
    mtl = tmp_path / "scene.mtl"
    obj.write_text(OBJ)
    mtl.write_text(MTL)
    table = extract(obj, mtl)
    inventory = {"Paint_Grey.jpg", "Lamp_Glow.png", "Lace.jpg", "Lace_Alpha.png",
                 "Brick.jpg", "Brick_Bump.jpg", "Leftover.jpg", "Orphan.jpg"}
    plans = build_plans(table, {}, inventory)
    assert plans["Paint_Grey"]["base_color"] == {
        "texture": "Paint_Grey.jpg", "source": "mtl:map_Kd"}
    assert plans["Glass"]["classification"] == "glass"
    assert plans["Glass"]["alpha"] == {"value": 0.31, "source": "mtl:TransparencyFactor"} \
        or abs(plans["Glass"]["alpha"]["value"] - 0.31) < 1e-6
    assert plans["Lace"]["alpha"]["cutout_texture"] == "Lace_Alpha.png"
    brick_normal = plans["Brick"]["normal"]
    assert brick_normal["texture"] == "Brick_Bump.jpg"
    assert brick_normal["kind"] == "height"
    assert abs(brick_normal["strength"] - 0.5) < 1e-6
    assert plans["Lamp"]["emission"]["texture"] == "Lamp_Glow.png"
    fates = explain_texture_usage(table, plans, inventory)
    assert fates["Paint_Grey.jpg"].startswith("used:")
    assert fates["Leftover.jpg"] == "unreferenced" or fates["Leftover.jpg"].startswith("waived"), \
        "a texture only an unassigned material names is not a recovery hole"
    assert fates["Orphan.jpg"] == "unreferenced"


def test_standard_fbx_normal_map_channel_plans_as_a_normal_map():
    # A Blender-exported FBX (the office building) carries its normal maps
    # on the standard NormalMap channel — a REAL tangent-space normal map,
    # not a Corona height bump — and the plan must not waive it away.
    table = {
        "materials": {
            "Wall": {"class": "standard", "assigned": True, "children": {},
                     "props": {"DiffuseColor": [0.8, 0.8, 0.8]},
                     "channels": {"DiffuseColor": "Wall_Diffuse.png",
                                  "NormalMap": "Wall_Normal.png"}},
        },
        "embedded_textures": [],
    }
    inventory = {"Wall_Diffuse.png", "Wall_Normal.png"}
    plans = build_plans(table, {}, inventory)
    normal = plans["Wall"]["normal"]
    assert normal is not None and normal["texture"] == "Wall_Normal.png"
    assert normal["kind"] == "normal"
    assert explain_texture_usage(table, plans, inventory)["Wall_Normal.png"].startswith("used:")


def test_inventory_lookup_forgives_punctuation_differences():
    # SketchUp sanitizes material names into its MTL texture paths
    # ("AD_W_Wall_Concrete.jpg") while the texture zip keeps the authored
    # names ("AD.W_Wall_Concrete.jpg"); an exact stem match ships the
    # room flat. Dots, underscores, hyphens, and spaces are one class.
    inventory = {"AD.W_Wall_Concrete.jpg", "ID Carpet-03.png", "Exact.jpg"}
    assert find_in_inventory("AD_W_Wall_Concrete.jpg", inventory) == "AD.W_Wall_Concrete.jpg"
    assert find_in_inventory("ID_Carpet_03.jpg", inventory) == "ID Carpet-03.png"
    assert find_in_inventory("Exact.jpg", inventory) == "Exact.jpg"
    assert find_in_inventory("Missing.jpg", inventory) is None


def test_inventory_lookup_collapses_sketchup_clone_suffixes():
    # SketchUp numbers cloned materials ("ID.Fabric_Sofa_Grey_01" imported
    # a thousand times becomes ID_Fabric_Sofa_Grey_01_1000_ in the OBJ)
    # and names each clone's texture after the clone; the zip ships the
    # ONE source image. The clone mark is a trailing number followed by
    # an underscore — wrapped in underscores, or GLUED to the stem
    # (ID_Decor_Ceramic_bianco3650_, ID__Metal_Bronze_Satin997_: the hill
    # house's 7,400 unresolved names, 2026-09-06) — and collapses onto
    # the base name. A bare trailing number ("ID_Decor_10") is a
    # different picture and does NOT collapse.
    inventory = {"ID.Fabric_Sofa_Grey_01.jpg", "ID.Decor.jpg", "ID.Decor_10.jpg",
                 "ID.Decor_Ceramic_bianco.jpg", "ID.Metal_Bronze_Satin.jpg"}
    assert find_in_inventory("ID_Fabric_Sofa_Grey_01_1000_.jpg", inventory) \
        == "ID.Fabric_Sofa_Grey_01.jpg"
    assert find_in_inventory("ID_Fabric_Sofa_Grey_01_7_.jpg", inventory) \
        == "ID.Fabric_Sofa_Grey_01.jpg"
    assert find_in_inventory("ID_Decor_Ceramic_bianco3650_.jpg", inventory) \
        == "ID.Decor_Ceramic_bianco.jpg"
    assert find_in_inventory("ID__Metal_Bronze_Satin997_.jpg", inventory) \
        == "ID.Metal_Bronze_Satin.jpg"
    assert find_in_inventory("ID_Decor_10.jpg", inventory) == "ID.Decor_10.jpg"
    assert find_in_inventory("ID_Decor_11.jpg", inventory) is None


def test_inventory_lookup_is_the_same_answer_in_every_process():
    # The hill house ships BOTH "ID_Decor_13.jpg" and "ID.Decor_13.jpg" —
    # two pictures, one loosened stem. Run 42 (2026-09-09): the converter
    # resolved the clone reference "ID_Decor_13_1_.jpg" to one, the audit
    # (its own process, its own set order) to the other, and the audit
    # reported a material lost that had merely merged with its twin. A
    # lookup is a function of its inputs: the tie breaks by the file
    # spelled like the reference (clone mark dropped, punctuation kept)
    # before any loosening, and a tie that survives that breaks by name
    # — never by set order.
    inventory = {"ID_Decor_13.jpg", "ID.Decor_13.jpg"}
    assert find_in_inventory("ID_Decor_13_1_.jpg", inventory) == "ID_Decor_13.jpg"
    assert find_in_inventory("ID.Decor_13_1_.jpg", inventory) == "ID.Decor_13.jpg"
    assert find_in_inventory("ID_Decor_13.jpg", inventory) == "ID_Decor_13.jpg"
    assert find_in_inventory("ID.Decor_13.jpg", inventory) == "ID.Decor_13.jpg"
    # No spelling favors either: the same file, every process.
    for trial in range(20):
        assert find_in_inventory("ID Decor 13.jpg", set(sorted(inventory, reverse=bool(trial % 2)))) \
            == "ID.Decor_13.jpg"
        assert find_in_inventory("ID-Decor-13_5_.jpg", {"ID.Decor_13.jpg", "ID_Decor_13.jpg", "id decor 13.png"}) \
            == "ID.Decor_13.jpg"


def test_clone_stem_names_the_source_image():
    # The audit groups unshipped references by the source image they were
    # cloned from, so a provider hole reads "id_decor_ceramic_bianco x5500"
    # instead of 5500 names.
    assert clone_stem("ID_Decor_Ceramic_bianco3650_.jpg") == "id_decor_ceramic_bianco"
    assert clone_stem("ID_Fabric_Sofa_Grey_01_1000_.jpg") == "id_fabric_sofa_grey_01"
    assert clone_stem("ID__Metal_Bronze_Satin997_.jpg") == "id_metal_bronze_satin"
    assert clone_stem("ID.Decor_10.jpg") == "id_decor_10"
    assert clone_stem("AD.W_Wall Concrete.JPG") == "ad_w_wall_concrete"


def test_fbx_transparency_factor_map_is_a_cutout():
    # A Blender-exported FBX carries an alpha mask on the TransparencyFactor
    # channel (the office building's cable-duct "AlfaMask"); it is a
    # cutout, exactly like a Corona texmapOpacity.
    table = {
        "materials": {
            "Duct": {"class": "standard", "assigned": True, "children": {},
                     "props": {"DiffuseColor": [0.8, 0.8, 0.8]},
                     "channels": {"DiffuseColor": "Duct_Diffuse.png",
                                  "TransparencyFactor": "Duct_AlfaMask.png"}},
        },
        "embedded_textures": [],
    }
    inventory = {"Duct_Diffuse.png", "Duct_AlfaMask.png"}
    plans = build_plans(table, {}, inventory)
    assert plans["Duct"]["alpha"] == {"cutout_texture": "Duct_AlfaMask.png",
                                      "source": "fbx:TransparencyFactor"}
    assert explain_texture_usage(table, plans, inventory)["Duct_AlfaMask.png"].startswith("used:")
