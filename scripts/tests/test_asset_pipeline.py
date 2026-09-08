"""Asset-pipeline conservation audit (`make test-assets`).

The provider's .max/.fbx/.obj files are the AUTHORING TRUTH; these
tests hold the material plan (scripts/material_plan.py) and the built
.glb to it, for every pack the pipeline converts through the plan doors
(PACKS below: the planet-3 environments and the military-ship hull).
Every expectation is DERIVED from the generated extracts — the material
manifest — never pinned to counts, so a new provider pack rides the same
audit by adding one row: its first run NAMES what the new format breaks
instead of a playtest revealing it.

A pack may carry an explicit WAIVER for what its provider never shipped
(`unshipped`): the audit then reports the hole as an expected failure
with the reason, never as silence.

Extracts and glb are produced by `make assets`; tests skip (loudly) when
an artifact is absent rather than fail on a half-built tree.
"""
import json
import re
import struct
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
APARTMENT = ROOT / "assets" / "apartment"
MILITARY_SHIP = ROOT / "assets" / "cgtrader_ships" / "military_ship"
HILL_HOUSE = ROOT / "assets" / "hill_house"
OFFICE_BUILDING = ROOT / "assets" / "office_building"
MOUNTAIN_VILLA = ROOT / "assets" / "mountain_villa"
ENVIRONMENTS = ROOT / "godot" / "addons" / "environments"

# pack -> the pipeline's artifacts for it: the material manifest (FBX
# connection tables or MTL statements, one shape), the .max material
# table (None for packs that ship no .max), the root the archives
# extract under (walked recursively), the built .glb, the converter's
# report of what a keep box dropped (None for packs converted whole),
# and the provider-hole waiver (None when everything declared shipped).
PACKS = {
    "apartment": {
        "manifest": APARTMENT / "fbx_materials.json",
        "max": APARTMENT / "max_materials.json",
        "textures": APARTMENT / "unpacked",
        "glb": ENVIRONMENTS / "apartment.glb",
        "report": APARTMENT / "conversion.json",
        "unshipped": None,
    },
    "hill_house": {
        "manifest": HILL_HOUSE / "mtl_materials.json",
        "max": None,
        "textures": HILL_HOUSE / "unpacked",
        "glb": ENVIRONMENTS / "hill_house.glb",
        "report": HILL_HOUSE / "conversion.json",
        # SketchUp wrote one texture file name per material (ID_Decor_10.jpg,
        # ...) but the provider shipped the SOURCE maps only; those
        # materials fall to their authored Kd, the map's average color.
        "unshipped": {"files": "all",
                      "reason": "SketchUp per-material texture names; provider ships source maps only"},
    },
    "office_building": {
        "manifest": OFFICE_BUILDING / "fbx_materials.json",
        "max": None,
        "textures": OFFICE_BUILDING / "unpacked",
        "glb": ENVIRONMENTS / "office_building.glb",
        "report": OFFICE_BUILDING / "conversion.json",
        "unshipped": None,
    },
    "mountain_villa": {
        "manifest": MOUNTAIN_VILLA / "fbx_materials.json",
        "max": MOUNTAIN_VILLA / "max_materials.json",
        "textures": MOUNTAIN_VILLA / "unpacked",
        "glb": ENVIRONMENTS / "mountain_villa.glb",
        "report": MOUNTAIN_VILLA / "conversion.json",
        # One material references a downloaded photo the author never
        # packed; it falls to its declared color.
        "unshipped": {"files": ["74522314_600483210776012_1527579838368448512_n.jpg"],
                      "reason": "provider never packed the Croncreet material's photo"},
    },
    "military_ship": {
        "manifest": MILITARY_SHIP / "fbx_materials.json",
        "max": None,
        "textures": MILITARY_SHIP / "unpacked",
        "glb": ROOT / "godot" / "addons" / "ships" / "military_ship.glb",
        "report": None,
        "unshipped": None,
    },
}

sys.path.insert(0, str(ROOT / "scripts"))
import material_plan  # noqa: E402
from material_plan import (  # noqa: E402
    BUMP_CHANNELS,
    DIFFUSE_CHANNELS,
    EMISSION_CAP,
    GLOSS_CHANNELS,
    NORMAL_CHANNELS,
    OPACITY_CHANNELS,
    ROUGHNESS_CHANNELS,
    SELFILLUM_CHANNELS,
    build_plans,
    explain_texture_usage,
    find_in_inventory,
    shipped_inventory,
)


# ---- fixtures: the generated manifest artifacts, per pack ----

def _load_or_skip(path, hint):
    if not path.exists():
        pytest.skip(f"{path} missing — run `{hint}` first")
    with open(path) as f:
        return json.load(f)


@pytest.fixture(scope="session", params=sorted(PACKS))
def pack(request):
    return PACKS[request.param]


@pytest.fixture(scope="session")
def fbx(pack):
    """The pack's material manifest (FBX connection tables or MTL
    statements — one shape), named for its first source."""
    return _load_or_skip(pack["manifest"], "make assets")


@pytest.fixture(scope="session")
def max_table(pack):
    if pack["max"] is None:
        return {}
    raw = _load_or_skip(pack["max"], "make assets")
    return material_plan.normalize_max_table(raw)


@pytest.fixture(scope="session")
def inventory(pack, fbx):
    tex_root = pack["textures"]
    if not tex_root.is_dir():
        pytest.skip(f"{tex_root} missing — run `make assets` first")
    return shipped_inventory(fbx, tex_root)


@pytest.fixture(scope="session")
def plans(fbx, max_table, inventory):
    return build_plans(fbx, max_table, inventory)


@pytest.fixture(scope="session")
def report(pack):
    """The converter's account of what its keep box dropped, when the
    pack has one; an empty account otherwise."""
    path = pack["report"]
    if path is None or not path.exists():
        return {"dropped_materials": []}
    with open(path) as f:
        return json.load(f)


# ---- the spec for 3ds Max container materials: what camera rays see ----
# RaySwitchMtl renders its directMtl to the camera; LayeredMtl's base
# layer is baseMtl. Blender's FBX importer understands neither (it imports
# the container as a blank material and never instantiates the subs) —
# the plan must undo that.

def camera_facing(fbx, name, _depth=0):
    rec = fbx["materials"][name]
    if _depth < 8:
        if rec["class"] == "RaySwitchMtl" and "directMtl" in rec["children"]:
            return camera_facing(fbx, rec["children"]["directMtl"], _depth + 1)
        if rec["class"] == "LayeredMtl" and "baseMtl" in rec["children"]:
            return camera_facing(fbx, rec["children"]["baseMtl"], _depth + 1)
    return name


def channel_file(rec, channel_set, inventory):
    """The record's shipped file on any of the given channels, or None —
    resolved by the plan's own lookup, so the audit and the plan agree
    on what "shipped" means."""
    for chan, basename in rec["channels"].items():
        if chan in channel_set:
            hit = find_in_inventory(basename, inventory)
            if hit:
                return hit
    return None


def assigned(fbx):
    return {n: r for n, r in fbx["materials"].items() if r["assigned"]}


# ---- plan-level contracts ----

def test_every_assigned_material_has_a_plan(fbx, plans):
    missing = sorted(set(assigned(fbx)) - set(plans))
    assert not missing, f"assigned materials without a plan: {missing}"


def test_containers_resolve_to_their_camera_facing_submaterial(
        fbx, plans, inventory):
    """The blank-grey floor/cabinet bug: a container's plan must be built
    from the sub-material camera rays actually see, never from the
    container's own junk standard fields."""
    bad = []
    for name, rec in assigned(fbx).items():
        if not rec["children"]:
            continue
        sub = camera_facing(fbx, name)
        assert sub != name, f"{name} classed as container but resolves to itself"
        plan = plans[name]
        if plan.get("resolved_from", [])[-1:] != [sub]:
            bad.append((name, sub, plan.get("resolved_from")))
            continue
        sub_diffuse = channel_file(fbx["materials"][sub], DIFFUSE_CHANNELS, inventory)
        if sub_diffuse and plan["base_color"].get("texture") != sub_diffuse:
            bad.append((name, sub, plan["base_color"]))
    assert not bad, f"containers not resolved to camera-facing sub: {bad}"


def test_declared_diffuse_is_never_dropped(fbx, plans, inventory):
    """Every assigned material whose (resolved) record declares a diffuse
    map that shipped must plan exactly that texture."""
    dropped = []
    for name in assigned(fbx):
        rec = fbx["materials"][camera_facing(fbx, name)]
        f = channel_file(rec, DIFFUSE_CHANNELS, inventory)
        if f and plans[name]["base_color"].get("texture") != f:
            dropped.append((name, f, plans[name]["base_color"]))
    assert not dropped, f"declared diffuse maps dropped: {dropped}"


def test_declared_texture_references_resolve_to_shipped_files(pack, fbx, inventory):
    """The gray-scene hole the plan cannot see: a material file that names
    a texture the pack never shipped falls through to its flat color and
    the plan calls that "declared". Every declared reference on a live
    channel of an assigned material must resolve to a shipped file — or
    be a NAMED waiver (PACKS `unshipped`), reported as an expected
    failure with its reason and the SOURCE IMAGES the holes collapse to
    (a SketchUp scene clones one material thousands of times; the
    reader wants "ceramic_bianco x5500", not 5500 names), never silence."""
    waiver = pack["unshipped"] or {}
    waived = set(waiver.get("files") or []) if waiver.get("files") != "all" else None
    missing = {}
    for name in assigned(fbx):
        rec = fbx["materials"][camera_facing(fbx, name)]
        for chan, basename in rec["channels"].items():
            if any(m in chan for m in material_plan.WAIVED_CHANNEL_MARKERS):
                continue
            if waived is not None and basename in waived:
                continue
            if find_in_inventory(basename, inventory) is None:
                missing.setdefault(basename, []).append(name)
    if missing and waived is None:
        by_source = {}
        for basename, names in missing.items():
            stem = material_plan.clone_stem(basename)
            by_source[stem] = by_source.get(stem, 0) + len(names)
        top = ", ".join(f"{stem} x{n}" for stem, n in
                        sorted(by_source.items(), key=lambda kv: (-kv[1], kv[0]))[:8])
        pytest.xfail(f"{len(missing)} declared textures never shipped, "
                     f"{len(by_source)} source images ({top}) — "
                     f"waived: {waiver.get('reason')}")
    assert not missing, (
        f"{len(missing)} declared textures never shipped (first 10): "
        f"{ {k: v[:2] for k, v in sorted(missing.items())[:10]} }")


def test_bump_maps_become_normal_plans(fbx, plans, inventory):
    dropped = []
    for name in assigned(fbx):
        rec = fbx["materials"][camera_facing(fbx, name)]
        f = channel_file(rec, BUMP_CHANNELS, inventory)
        if not f:
            continue
        normal = plans[name].get("normal")
        # The .max table's Normal channel may name a different (real
        # normal-map) file — that is fine; DROPPING relief entirely is not.
        if normal is None:
            dropped.append((name, f))
    assert not dropped, f"bump/relief maps dropped: {dropped}"


def test_declared_normal_maps_wire_as_normal_maps(fbx, plans, inventory):
    """A tangent-space normal map the model file declares (a Blender
    export's NormalMap channel) ships as a normal map — never waived,
    never mistaken for a height bump to be re-derived."""
    bad = []
    for name in assigned(fbx):
        rec = fbx["materials"][camera_facing(fbx, name)]
        f = channel_file(rec, NORMAL_CHANNELS, inventory)
        if not f:
            continue
        normal = plans[name].get("normal")
        if normal is None or (normal.get("texture") == f and normal.get("kind") != "normal"):
            bad.append((name, f, normal))
    assert not bad, f"declared normal maps dropped or misread: {bad}"


def test_bump_derived_normals_carry_the_authored_strength(fbx, plans, inventory):
    """Corona renders bump = levelBump x mapamount (typically well under
    1.0), MTL carries -bm; a converted normal map without the authored
    strength shouts on every wall. The plan must carry it, clamped to
    the sane range."""
    bad = []
    for name in assigned(fbx):
        rec = fbx["materials"][camera_facing(fbx, name)]
        f = channel_file(rec, BUMP_CHANNELS, inventory)
        normal = plans[name].get("normal")
        if not f or normal is None or normal.get("kind") != "height":
            continue
        props = rec["props"]
        authored = props.get("mapamountBump",
                             props.get("baseBumpMapAmount", 1.0))
        expected = max(0.05, min(1.5, authored))
        if abs(normal.get("strength", -1.0) - expected) > 1e-3:
            bad.append((name, normal.get("strength"), expected))
    assert not bad, f"height-normal plans without authored strength: {bad}"


def test_gloss_maps_become_inverted_roughness(fbx, plans, inventory):
    """Corona reflectGlossiness is 1 - roughness: the map must arrive
    inverted, or every polished surface renders uniformly dull."""
    bad = []
    for name in assigned(fbx):
        rec = fbx["materials"][camera_facing(fbx, name)]
        f = channel_file(rec, GLOSS_CHANNELS, inventory)
        if not f:
            continue
        rough = plans[name].get("roughness")
        if rough is None or (
                rough.get("texture") == f and not rough.get("invert")):
            bad.append((name, f, rough))
    assert not bad, f"gloss maps dropped or not inverted: {bad}"


def test_gloss_roughness_blends_at_the_authored_amount(fbx, plans, inventory):
    """Corona blends the glossiness map toward the scalar base at
    mapamountReflectGlossiness (0.15-0.2 on the walls); applied at 1.0
    the scratch maps render as mottled roughness chaos."""
    bad = []
    for name in assigned(fbx):
        rec = fbx["materials"][camera_facing(fbx, name)]
        f = channel_file(rec, GLOSS_CHANNELS, inventory)
        rough = plans[name].get("roughness")
        if not f or rough is None or rough.get("texture") != f:
            continue
        props = rec["props"]
        amount = props.get("mapamountReflectGlossiness", 1.0)
        if amount >= 0.999:
            continue
        blend = rough.get("blend")
        expected_base = 1.0 - props.get("refractGlossiness", 1.0)
        if (blend is None
                or abs(blend.get("amount", -1) - amount) > 1e-3
                or abs(blend.get("base", -1) - expected_base) > 1e-3):
            bad.append((name, amount, blend))
    assert not bad, f"gloss maps not blended at authored amounts: {bad}"


def test_partial_diffuse_maps_blend_toward_the_authored_color(
        fbx, plans, inventory):
    """mapamountDiffuse < 1 means the texture is a TINT over the authored
    color (a cabinet authored 60% wood over near-black); at 100% it
    renders as pure texture."""
    bad = []
    for name in assigned(fbx):
        rec = fbx["materials"][camera_facing(fbx, name)]
        f = channel_file(rec, DIFFUSE_CHANNELS, inventory)
        base = plans[name]["base_color"]
        if not f or base.get("texture") != f:
            continue
        amount = rec["props"].get("mapamountDiffuse", 1.0)
        if amount >= 0.999 or "colorDiffuse" not in rec["props"]:
            continue
        blend = base.get("blend")
        if (blend is None
                or abs(blend.get("amount", -1) - amount) > 1e-3
                or blend.get("color") != rec["props"]["colorDiffuse"]):
            bad.append((name, amount, blend))
    assert not bad, f"partial diffuse maps not blended: {bad}"


def test_physical_roughness_maps_wire_directly(fbx, plans, inventory):
    bad = []
    for name in assigned(fbx):
        rec = fbx["materials"][camera_facing(fbx, name)]
        f = channel_file(rec, ROUGHNESS_CHANNELS, inventory)
        if not f:
            continue
        rough = plans[name].get("roughness")
        if rough is None or (
                rough.get("texture") == f and rough.get("invert")):
            bad.append((name, f, rough))
    assert not bad, f"physical roughness maps dropped or wrongly inverted: {bad}"


def test_opacity_cutouts_carry_their_texture(fbx, plans, inventory):
    """Curtain lace / leaf cutouts: a declared opacity map must arrive as
    an alpha texture, not collapse to a scalar."""
    dropped = []
    for name in assigned(fbx):
        rec = fbx["materials"][camera_facing(fbx, name)]
        f = channel_file(rec, OPACITY_CHANNELS, inventory)
        if not f:
            continue
        alpha = plans[name].get("alpha")
        if alpha is None or alpha.get("cutout_texture") != f:
            dropped.append((name, f, alpha))
    assert not dropped, f"opacity cutout maps dropped: {dropped}"


def test_selfillum_maps_become_emission(fbx, plans, inventory):
    dropped = []
    for name in assigned(fbx):
        rec = fbx["materials"][camera_facing(fbx, name)]
        f = channel_file(rec, SELFILLUM_CHANNELS, inventory)
        if not f:
            continue
        emission = plans[name].get("emission")
        if emission is None or emission.get("texture") != f:
            dropped.append((name, f, emission))
    assert not dropped, f"self-illumination maps dropped: {dropped}"


def test_light_materials_are_policy_mapped(fbx, plans):
    """Corona LightMtl surfaces are photometric light SOURCES (the pack
    ships sky cards at strength 1000); rendered raw they clip to white
    shapes. Policy: classified as light, emission kept, strength capped,
    the authored strength preserved for provenance."""
    bad = []
    for name, rec in assigned(fbx).items():
        if fbx["materials"][camera_facing(fbx, name)]["class"] != "LightMtl":
            continue
        plan = plans[name]
        emission = plan.get("emission")
        if (plan["classification"] != "light"
                or emission is None
                or not (0.0 < emission["strength"] <= EMISSION_CAP)
                or "authored_strength" not in emission):
            bad.append((name, plan.get("classification"), emission))
    assert not bad, f"LightMtl materials not policy-mapped: {bad}"


def test_no_plan_exceeds_the_emission_cap(plans):
    hot = [(n, p["emission"]) for n, p in plans.items()
           if p.get("emission") and p["emission"]["strength"] > EMISSION_CAP]
    assert not hot, f"plans exceeding EMISSION_CAP={EMISSION_CAP}: {hot}"


def test_scalar_alpha_is_declared_transparency_never_invention(fbx, plans):
    """A plan may carry partial alpha ONLY as the declared
    1 - TransparencyFactor of the resolved record (the value the model's
    exporter computed from its refraction stack, or an MTL's d)."""
    bad = []
    for name, plan in plans.items():
        alpha = plan.get("alpha")
        if not alpha or "value" not in alpha:
            continue
        rec = fbx["materials"][camera_facing(fbx, name)] \
            if name in fbx["materials"] else None
        tf = (rec or {}).get("props", {}).get("TransparencyFactor", 0.0)
        if abs(alpha["value"] - (1.0 - tf)) > 1e-3:
            bad.append((name, alpha, tf))
    assert not bad, f"alpha values that trace to no declared transparency: {bad}"


def test_no_assigned_material_defaults_silently(fbx, plans):
    """The white-surface bug class: every assigned material must source
    its base color from SOMETHING declared (a texture, the .max table, a
    Corona color, the declared diffuse) — never a silent importer default."""
    silent = [n for n in assigned(fbx)
              if plans[n]["base_color"].get("source", "default") == "default"]
    assert not silent, f"materials shipping importer defaults: {silent}"


def test_embedded_textures_count_as_shipped(fbx, inventory):
    """A map the provider packed INSIDE the FBX (the military ship's
    diffuse and cockpit textures ship that way — no loose file at all)
    is as shipped as one in the archive: it is in the inventory, so the
    plan can name it and the fate report can account for it. Left out,
    every material wearing one plans a flat color and ships gray."""
    embedded = set(fbx.get("embedded_textures", []))
    assert embedded <= inventory, (
        f"embedded textures missing from the inventory: "
        f"{sorted(embedded - inventory)}")


def test_shipped_inventory_unions_loose_and_embedded(tmp_path):
    """The inventory door itself: image files anywhere under the
    extracted archives (wrapper folders, several archives) plus the
    manifest's embedded_textures, by basename; non-images and dotfiles
    are not textures."""
    wrapped = tmp_path / "Textures.rar" / "Textures"
    wrapped.mkdir(parents=True)
    (wrapped / "Loose.png").write_bytes(b"")
    (wrapped / ".DS_Store").write_bytes(b"")
    (wrapped / "Loose.png.meta").write_bytes(b"")
    other = tmp_path / "part_2"
    other.mkdir()
    (other / "Other.jpg").write_bytes(b"")
    fbx = {"materials": {}, "embedded_textures": ["Packed.png", "Loose.png"]}
    assert shipped_inventory(fbx, tmp_path) == {"Loose.png", "Other.jpg", "Packed.png"}
    assert shipped_inventory({"materials": {}}, tmp_path / "absent") == set()


def test_every_shipped_texture_has_a_named_fate(fbx, plans, inventory):
    """The sniff test, inverted (owner 2026-07-12): start from the files
    the provider shipped and demand a named fate for each — used by a
    plan, waived by channel policy, or unreferenced by the source. An
    unexplained texture is a recovery hole, exactly like the orphaned
    oak floor."""
    report = explain_texture_usage(fbx, plans, inventory)
    assert set(report) == set(inventory), (
        f"usage report must cover the whole inventory; missing: "
        f"{sorted(set(inventory) - set(report))[:10]}")
    allowed = ("used:", "waived:", "unreferenced")
    unexplained = {f: fate for f, fate in report.items()
                   if not fate.startswith(allowed)}
    assert not unexplained, f"textures with no named fate: {unexplained}"


# ---- glb-level contracts: what actually shipped ----

@pytest.fixture(scope="session")
def glb(pack):
    path = pack["glb"]
    if not path.exists():
        pytest.skip(f"{path} missing — run `make assets` first")
    with open(path, "rb") as f:
        f.seek(12)
        length, _ = struct.unpack("<II", f.read(8))
        return json.loads(f.read(length))


def glb_base_name(name):
    """Blender dedup suffixes (.001) don't exist in the source's name table."""
    return re.sub(r"\.\d{3}$", "", name)


def test_glb_textured_plans_actually_ship_textured(glb, plans):
    bad = []
    for mat in glb.get("materials", []):
        plan = plans.get(glb_base_name(mat.get("name", "")))
        if plan is None or plan["classification"] != "textured":
            continue
        if "baseColorTexture" not in mat.get("pbrMetallicRoughness", {}):
            bad.append(mat.get("name"))
    assert not bad, f"planned-textured materials shipping flat: {bad}"


def test_glb_has_no_single_sided_materials(glb):
    single = [m.get("name") for m in glb.get("materials", [])
              if not m.get("doubleSided", False)]
    assert not single, f"single-sided materials (invisible from behind): {single}"


def test_glb_emission_respects_the_cap(glb):
    # Window panes glow by design (convert-environment.py PANE_EMISSION);
    # everything else obeys the plan cap. The looser of the two bounds
    # all of it.
    ceiling = max(EMISSION_CAP, 1.5) + 1e-3
    hot = []
    for m in glb.get("materials", []):
        strength = (m.get("extensions", {})
                    .get("KHR_materials_emissive_strength", {})
                    .get("emissiveStrength", 1.0))
        if strength > ceiling:
            hot.append((m.get("name"), strength))
    assert not hot, f"materials past the emission ceiling (white shapes): {hot}"


def test_plan_key_is_the_exported_content_only():
    """Two plans that export the same glTF material are one class,
    whatever their provenance: the `source` notes ("mtl:map_Kd",
    "fbx:DiffuseColor"), the resolution chain and an emission's authored
    strength never reach the glb, so dedup merges across them — the
    apartment lost four classes to that on the first optimized run
    (2026-09-07). Content that does export keeps classes apart."""
    a = {"classification": "textured",
         "base_color": {"texture": "Wall.png", "source": "fbx:DiffuseColor"},
         "roughness": {"value": 0.5, "source": "max:reflectGlossiness"},
         "emission": {"color": [1, 1, 1], "strength": 1.0, "authored_strength": 1000.0},
         "resolved_from": ["Container", "Wall"]}
    b = {"classification": "textured",
         "base_color": {"texture": "Wall.png", "source": "mtl:map_Kd"},
         "roughness": {"value": 0.5, "source": "fbx:Shininess"},
         "emission": {"color": [1, 1, 1], "strength": 1.0, "authored_strength": 2.0}}
    c = dict(a, roughness={"value": 0.7, "source": "max:reflectGlossiness"})
    assert plan_key(a) == plan_key(b), "provenance is not content"
    assert plan_key(a) != plan_key(c), "a different roughness is a different material"


def plan_key(plan):
    """A plan's exported content: two source materials with equal keys
    export as identical glTF materials, and the door's optimizer
    (glTF-Transform dedup) merges them under one surviving name —
    SketchUp's thousands of clones are one material. Provenance never
    reaches the glb and is left out: the `source` notes, the resolution
    chain, an emission's authored strength."""
    def content(value):
        if isinstance(value, dict):
            return {k: content(v) for k, v in value.items()
                    if k not in ("source", "resolved_from", "authored_strength")}
        return value
    return json.dumps(content(plan), sort_keys=True)


def plan_classes(fbx, plans, report):
    """Assigned source materials grouped by plan content, the converter's
    deliberate drops (objects outside the keep box) left out:
    {plan key: [names]}. Conservation is judged per class."""
    dropped = set(report.get("dropped_materials", []))
    classes = {}
    for name in assigned(fbx):
        if name in dropped or name not in plans:
            continue
        classes.setdefault(plan_key(plans[name]), []).append(name)
    return classes


def test_glb_carries_every_assigned_material(glb, fbx, plans, report):
    """Conversion must not LOSE materials. The door merges source
    materials whose plans are identical (glTF-Transform's dedup keeps one
    name per content class), so the contract is per class: at least one
    of its names lands in the glb — except classes the converter reports
    it dropped on purpose with the objects outside the keep box. The
    failure names each lost class's members and face statistics (the
    manifest's count and largest triangle) so a sliver the importer
    pruned is told apart from a surface that vanished."""
    shipped = {glb_base_name(m.get("name", "")) for m in glb.get("materials", [])}
    stats = fbx.get("face_stats", {})
    lost = sorted((names for names in plan_classes(fbx, plans, report).values()
                   if not any(n in shipped for n in names)), key=lambda names: names[0])
    detail = {names[0]: stats.get(names[0]) for names in lost[:8]}
    assert not lost, (
        f"{len(lost)} material classes absent from the glb ({sum(map(len, lost))} source "
        f"materials): {[names[:3] for names in lost[:12]]} (face stats: {detail})")


def glb_triangles_by_material(glb):
    """Triangles each material index carries in the glb's primitives."""
    counts = {}
    for mesh in glb.get("meshes", []):
        for prim in mesh.get("primitives", []):
            if prim.get("mode", 4) != 4:
                continue
            acc = prim.get("indices", prim["attributes"]["POSITION"])
            n = glb["accessors"][acc]["count"] // 3
            counts[prim.get("material")] = counts.get(prim.get("material"), 0) + n
    return counts


def test_every_assigned_material_keeps_its_faces(glb, fbx, plans, report):
    """A material name surviving is not the surface surviving: every
    class of assigned materials (equal plans merge under one name) with
    solid faces at the source must carry at least as many triangles in
    the glb as its members had distinct solid faces (a quad is two;
    nothing lawful makes fewer) — owner 2026-09-06: the hill house lost
    lamp chains and seat cushions, small parts whose names came through.
    Packs whose manifest counts faces (the MTL manifest's face_stats)
    are held to it; the others have no source count."""
    stats = fbx.get("face_stats")
    if not stats:
        pytest.skip("the material manifest carries no per-material face counts")
    tris = glb_triangles_by_material(glb)
    by_name = {}
    for index, mat in enumerate(glb.get("materials", [])):
        name = glb_base_name(mat.get("name", ""))
        by_name[name] = by_name.get(name, 0) + tris.get(index, 0)
    short = []
    for names in plan_classes(fbx, plans, report).values():
        faces = sum(stats.get(n, {}).get("distinct_solid_faces", 0) for n in names)
        carried = sum(by_name.get(n, 0) for n in names)
        if faces > 0 and carried < faces:
            label = names[0] + (f" (+{len(names) - 1} merged)" if len(names) > 1 else "")
            short.append((label, faces, carried))
    short.sort(key=lambda t: t[2] / t[1])
    lost_faces = sum(f - c for _n, f, c in short)
    assert not short, (
        f"{len(short)} material classes carry fewer triangles than their source "
        f"faces ({lost_faces} faces short in all; sparsest first as "
        f"(class, source faces, glb tris)): {short[:12]}")


def test_no_assigned_material_ships_invisible(glb, fbx):
    """A surface that shipped but renders nothing — blended or masked at
    an alpha under 0.05 — is a hole with a name. Every such material
    is listed; none is expected (a pack that wants one waives it)."""
    tris = glb_triangles_by_material(glb)
    invisible = []
    for index, mat in enumerate(glb.get("materials", [])):
        if not tris.get(index) or mat.get("alphaMode") not in ("BLEND", "MASK"):
            continue
        factor = mat.get("pbrMetallicRoughness", {}).get("baseColorFactor")
        alpha = float(factor[3]) if factor and len(factor) > 3 else 1.0
        if alpha < 0.05:
            invisible.append((mat.get("name"), mat.get("alphaMode"), round(alpha, 3), tris[index]))
    assert not invisible, (
        f"{len(invisible)} materials ship invisible (name, mode, alpha, tris): {invisible[:10]}")


def test_glb_ships_no_animations(glb):
    """Converted packs are STATIC: the game plays no provider animation
    (landing gear, canopy hinges), and an animated node exports its keyed
    transform ON TOP of the baked mesh — the military ship's gear and
    canopy frame shipped as centimeter-sized boxes at the wrong place
    (2026-09-06) until the conversion stripped the actions."""
    animated = [a.get("name") for a in glb.get("animations", [])]
    assert not animated, f"provider animations survived the conversion: {animated}"


# ---- the decimate door's products: every installed enemy model ----

ENEMY_MODELS = ROOT / "godot" / "addons" / "enemies"


@pytest.fixture(scope="session",
                params=sorted(p.name for p in ENEMY_MODELS.glob("*.glb")) or ["(none)"])
def enemy_glb(request):
    path = ENEMY_MODELS / request.param
    if not path.is_file():
        pytest.skip(f"no decimated enemy models under {ENEMY_MODELS} — run `make assets` first")
    with open(path, "rb") as f:
        f.seek(12)
        length, _ = struct.unpack("<II", f.read(8))
        return json.loads(f.read(length))


def test_decimated_models_ship_no_animations(enemy_glb):
    """The decimate door's products are STATIC like the converted packs:
    the apartment boss shipped five provider "Drone" clips whose 60 scale
    channels re-applied over the roster's size fit at spawn — the boss
    stayed gigantic whatever enemies.toml said (owner 2026-09-06)."""
    animated = [a.get("name") for a in enemy_glb.get("animations", [])]
    assert not animated, f"provider animations survived decimation: {animated}"


# ---- texel density: the scene metrics against the floor ----

# Texels per WORLD meter a fixed environment's textured surfaces must lay
# down (owner 2026-09-06: "could we enforce something like 500 texels/
# meter as a minimum?" — contract first, the retile after). The
# apartment's median is about 1000 and reads well; the office's 216 is
# where the eye starts to notice; the villa's 20..40 is blocks.
MIN_WORLD_TEXELS_PER_M = 500.0


# Share of a textured surface's area inside the zone union (the metrics'
# in_zones) for the floor to reach it: the player walks the zones, so a
# backdrop hill or a tree card past them is out of the rule by geometry
# — one rule for every scene, never a waiver naming a level (owner
# 2026-09-07: "genericise your import methodologies and apply all of the
# tricks on all of the levels").
PLAYABLE_SHARE = 0.5

CATALOG_KITS = ROOT / "catalog" / "kits.toml"
METRICS_DIR = ROOT / "out" / "metrics"

try:
    import tomllib
except ImportError:  # the audit venv (python 3.9) carries tomli
    import tomli as tomllib


@pytest.fixture(scope="session")
def metrics(pack):
    """The scene metrics `make metrics` (or `make assets`) wrote for this
    pack's product — the reproducible reading of what shipped. None for
    a pack that is not a scene (a hull has its own metrics)."""
    key = pack["glb"].stem
    if kit_scale(key) is None:
        return None
    path = METRICS_DIR / f"{key}.toml"
    if not path.exists():
        pytest.skip(f"{path} missing — run `make metrics` first")
    with open(path, "rb") as f:
        return tomllib.load(f)


def kit_scale(environment_key):
    """World units per model meter the catalog declares for the fixed kit
    joined to this environment; None when no kit joins it (a hull)."""
    with open(CATALOG_KITS, "rb") as f:
        kits = tomllib.load(f)["kits"]
    for kit in kits.values():
        if kit.get("paradigm") == "fixed" and kit.get("environment") == environment_key:
            return float(kit["scale"])
    return None


def test_textured_surfaces_meet_the_texel_density_floor(pack, metrics):
    """Every textured surface the player walks past lays down at least
    MIN_WORLD_TEXELS_PER_M once the kit scale is applied — the metrics'
    texels per MODEL meter (map size x mapping over surface area)
    divided by the catalog's scale, since one model meter stretches to
    `scale` world meters. The rule reaches the surfaces inside the zone
    union (in_zones >= PLAYABLE_SHARE; a scene without a roster is all
    in scope), the same rule for every scene: a backdrop hill or a tree
    card past the rooms is out by geometry, never by a waiver naming a
    level (owner 2026-09-07). The median must clear the floor and every
    surface under it is listed."""
    key = pack["glb"].stem
    scale = kit_scale(key)
    if scale is None:
        pytest.skip(f"{key}: no fixed kit joins it — a hull, not a scene")
    rows = [r for r in metrics.get("density", [])
            if r.get("in_zones", 1.0) >= PLAYABLE_SHARE]
    assert rows, f"{key}: the metrics list no textured surfaces inside the zones"
    world = sorted(r["texels_per_m"] / scale for r in rows)
    median = world[len(world) // 2]
    sparse = sorted((r["texels_per_m"] / scale, r["name"]) for r in rows
                    if r["texels_per_m"] / scale < MIN_WORLD_TEXELS_PER_M)
    listing = ", ".join(f"{name} {d:.0f}" for d, name in sparse[:10])
    assert median >= MIN_WORLD_TEXELS_PER_M, (
        f"{key}: median {median:.0f} texels per world meter (kit scale {scale:g}) "
        f"over {len(rows)} surfaces in the zones is under the "
        f"{MIN_WORLD_TEXELS_PER_M:.0f} floor — the mapping spends its maps too "
        f"thinly; sparsest: {listing}")
    assert not sparse, (
        f"{key}: {len(sparse)} of {len(rows)} textured surfaces in the zones under "
        f"{MIN_WORLD_TEXELS_PER_M:.0f} texels per world meter: {listing}")


# Surfaces one Godot mesh keeps: RenderingServer.MAX_MESH_SURFACES.
# Past it the importer drops every further surface with an error
# ('Condition "surfaces.size() == RenderingServer::MAX_MESH_SURFACES" is
# true.' — 22,630 of them in out/assets-run29-import.log, two passes over
# the hill house's 11,571 single-mesh materials). One glTF primitive is
# one surface; the converter exports one primitive per material a mesh
# uses, so a mesh's surfaces are the materials it carries.
GODOT_MAX_MESH_SURFACES = 256


def test_no_scene_mesh_exceeds_godots_surface_cap(pack, metrics):
    """Every mesh a scene ships carries at most GODOT_MAX_MESH_SURFACES
    surfaces, so every material reaches the level (owner 2026-09-07:
    "materials not making the transition, not textures" — the hill
    house's seats, chains and table tops sat past the cap)."""
    key = pack["glb"].stem
    if metrics is None:
        pytest.skip(f"{key}: a hull, not a scene")
    geometry = metrics["geometry"]
    assert geometry["surfaces_max_per_mesh"] <= GODOT_MAX_MESH_SURFACES, (
        f"{key}: a mesh ships {geometry['surfaces_max_per_mesh']} surfaces "
        f"({geometry['surfaces']} over {geometry['parts']} parts); Godot keeps "
        f"{GODOT_MAX_MESH_SURFACES} per mesh and drops the rest — "
        f"{geometry['surfaces_max_per_mesh'] - GODOT_MAX_MESH_SURFACES} materials "
        f"never reach the level (metrics: {metrics['materials']['clone_stems']} "
        f"distinct names once clone suffixes are stripped)")


# ---- door logs: every ERROR and WARNING a step printed, condensed by
# scripts/log-histogram.py into out/metrics/log_<step>.toml (a histogram
# of message x source x asset, and a tally per asset). A door that exits
# 0 while printing errors has still failed its product: Godot dropped
# 11,315 hill house surfaces past its per-mesh cap with 22,630 ERROR
# lines and exit status 0 (owner 2026-09-07: "build a histogram of
# errors ... 22k errors of this type from this file, that could be a
# thing to investigate first"). Both contracts are red until the doors
# print nothing — the reds are the work list, most frequent first.

# Engine noise the contracts do not hold the doors to — messages the
# engine prints about itself at exit, with no product behind them. Each
# entry is a message prefix (after the histogram's folding) and the
# reason; the histogram still records every occurrence, so the count is
# never hidden, only excused here by name. Nothing about a level or a
# pack is ever listed (owner 2026-09-07: no waivers naming levels).
ENGINE_NOISE = {
    "ObjectDB instances leaked at exit":
        "Godot's headless --import leaks editor objects at exit on every run "
        "(one per pass); nothing of ours allocates them",
}


# Blender's glTF exporter warns whenever one metallic-roughness map feeds
# both the Metallic and Roughness sockets (its __gather_sampler sees two
# sockets, the same image node behind each, and warns "the resulting
# glTF sampler will behave like the first shader node tex image") — the
# standard glTF PBR wiring every imported kit material has. Nothing in
# our doors can change it short of dropping a channel.
ENGINE_NOISE["More than one shader node tex image used for a texture"] = (
    "Blender's glTF exporter, metallic and roughness sockets sharing one image "
    "node (io_scene_gltf2 texture.py __gather_sampler); the export is correct")


# Blender's FBX importer reports every material->texture link it cannot
# resolve in a provider's FBX (the cgtrader mechs and sphere ships ship
# their Substance maps loose, unreferenced by the file: six links per
# model). The decimate door then rebuilds the material from those loose
# maps by design (decimate.py build_material), so the link was never
# going to be used.
ENGINE_NOISE["material link b'"] = (
    "Blender's FBX importer on a provider file whose texture links point at "
    "unshipped paths; decimate.py rebuilds the material from the loose maps")


# Third-party tools' own complaints about a provider file, scoped to the
# door log they appear in — never a bare message that could excuse one
# of ours elsewhere: (message prefix, log/asset prefix) -> reason.
TOOL_NOISE = {
    ("list index out of range", "max-"):
        "Blender's io_scene_max extension parsing 3ds Max nodes it does not "
        "model (the apartment's CoronaPhysicalMtl entries, 177 of them); its "
        "documented use here is the material table alone, which it still writes",
}


TOOL_NOISE[("bpy_struct: item.attr = val: Object.parent ID type does not support assignment to itself", "max-")] = (
    "the same extension trying to parent a 3ds Max node to itself (the villa's "
    "'mountain house' group); the object tree is not what the table reads")


TOOL_NOISE[("unpack requires a buffer of", "max-")] = (
    "the same extension reading a 3ds Max chunk shorter than its struct "
    "(eight ZJ-1xx nodes of the apartment); the material table is unaffected")


def engine_noise(row):
    if any(row["message"].startswith(prefix) for prefix in ENGINE_NOISE):
        return True
    return any(row["message"].startswith(m) and row["asset"].startswith(a)
               for m, a in TOOL_NOISE)


def door_log_histograms():
    return sorted(METRICS_DIR.glob("log_*.toml"))


def door_log_id(path):
    return path.stem[len("log_"):] if path else "no-door-logs"


def load_door_log(path):
    if path is None:
        pytest.skip("no door log histogram yet — run make assets (or make metrics)")
    with open(path, "rb") as f:
        return tomllib.load(f)


def door_log_listing(rows):
    return "; ".join(f"{r['count']} x {r['message'][:90]} [{r['asset']}]" for r in rows[:6])


@pytest.mark.parametrize("path", door_log_histograms() or [None], ids=door_log_id)

def test_a_door_prints_no_errors(path):
    doc = load_door_log(path)
    summary = doc["summary"]
    rows = [r for r in doc.get("error", []) if not engine_noise(r)]
    excused = sum(r["count"] for r in doc.get("error", []) if engine_noise(r))
    assert not rows, (
        f"{summary['step']}: {sum(r['count'] for r in rows)} errors ({len(rows)} distinct"
        f"{f', {excused} tool-noise occurrences excused' if excused else ''}) "
        f"over {', '.join(summary['logs'])}: {door_log_listing(rows)}")


@pytest.mark.parametrize("path", door_log_histograms() or [None], ids=door_log_id)

def test_a_door_prints_no_warnings(path):
    doc = load_door_log(path)
    summary = doc["summary"]
    rows = [r for r in doc.get("warning", []) if not engine_noise(r)]
    excused = sum(r["count"] for r in doc.get("warning", []) if engine_noise(r))
    assert not rows, (
        f"{summary['step']}: {sum(r['count'] for r in rows)} warnings ({len(rows)} distinct"
        f"{f', {excused} engine-noise occurrences excused' if excused else ''}) "
        f"over {', '.join(summary['logs'])}: {door_log_listing(rows)}")
