"""Asset-pipeline conservation audit (`make test-assets`).

The provider's .max/.fbx/.obj files are the AUTHORING TRUTH; these
tests hold the material plan (scripts/material_plan.py) and the built
.glb to it, for every pack the pipeline converts through the plan doors
(PACKS below: the planet-3 environments and the military-ship hull).
Every expectation is DERIVED from the generated extracts — the oracle —
never pinned to counts, so a new provider pack rides the same audit by
adding one row: its first run NAMES what the new format breaks instead
of a playtest revealing it.

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

# pack -> the pipeline's artifacts for it: the material oracle (FBX
# connection tables or MTL statements, one shape), the .max material
# table (None for packs that ship no .max), the root the archives
# extract under (walked recursively), the built .glb, the converter's
# report of what a keep box dropped (None for packs converted whole),
# and the provider-hole waiver (None when everything declared shipped).
PACKS = {
    "apartment": {
        "oracle": APARTMENT / "fbx_materials.json",
        "max": APARTMENT / "max_materials.json",
        "textures": APARTMENT / "unpacked",
        "glb": ENVIRONMENTS / "apartment.glb",
        "report": APARTMENT / "conversion.json",
        "unshipped": None,
    },
    "hill_house": {
        "oracle": HILL_HOUSE / "mtl_materials.json",
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
        "oracle": OFFICE_BUILDING / "fbx_materials.json",
        "max": None,
        "textures": OFFICE_BUILDING / "unpacked",
        "glb": ENVIRONMENTS / "office_building.glb",
        "report": OFFICE_BUILDING / "conversion.json",
        "unshipped": None,
    },
    "mountain_villa": {
        "oracle": MOUNTAIN_VILLA / "fbx_materials.json",
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
        "oracle": MILITARY_SHIP / "fbx_materials.json",
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


# ---- fixtures: the generated oracle artifacts, per pack ----

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
    """The pack's material oracle (FBX connection tables or MTL statements
    — one shape), named for its first source."""
    return _load_or_skip(pack["oracle"], "make assets")


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
    oracle's embedded_textures, by basename; non-images and dotfiles
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


def test_glb_carries_every_assigned_material(glb, fbx, plans, report):
    """Conversion must not LOSE materials: every assigned material lands
    in the glb (under its own name; Blender may dedup) — except those the
    converter reports it dropped on purpose with the objects outside the
    keep box. The failure names each lost material's face statistics
    (the oracle's count and largest triangle) so a sliver the importer
    pruned is told apart from a surface that vanished."""
    shipped = {glb_base_name(m.get("name", "")) for m in glb.get("materials", [])}
    dropped = set(report.get("dropped_materials", []))
    lost = sorted(n for n in assigned(fbx) if n not in shipped and n not in dropped)
    stats = fbx.get("face_stats", {})
    detail = {n: stats.get(n) for n in lost[:8]}
    assert not lost, (
        f"{len(lost)} assigned materials absent from the glb: {lost[:12]} "
        f"(face stats: {detail})")


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
