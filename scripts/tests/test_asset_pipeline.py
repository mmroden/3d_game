"""Asset-pipeline conservation audit (`make test-assets`).

The provider's .max/.fbx files are the AUTHORING TRUTH; these tests hold
the material plan (scripts/material_plan.py) and the built .glb to it.
Every expectation is DERIVED from the generated extracts — the oracle —
never pinned to counts, so a new provider pack (classroom, ...) rides the
same audit: its first run NAMES what the new format breaks instead of a
playtest revealing it.

Extracts and glb are produced by `make assets`; tests skip (loudly) when
an artifact is absent rather than fail on a half-built tree.
"""
import json
import os
import re
import struct
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
APARTMENT = ROOT / "assets" / "apartment"
GLB = ROOT / "godot" / "addons" / "environments" / "apartment.glb"

sys.path.insert(0, str(ROOT / "scripts"))
import material_plan  # noqa: E402
from material_plan import (  # noqa: E402
    BUMP_CHANNELS,
    DIFFUSE_CHANNELS,
    EMISSION_CAP,
    GLOSS_CHANNELS,
    OPACITY_CHANNELS,
    ROUGHNESS_CHANNELS,
    SELFILLUM_CHANNELS,
    build_plans,
    explain_texture_usage,
)


# ---- fixtures: the generated oracle artifacts ----

def _load_or_skip(path, hint):
    if not path.exists():
        pytest.skip(f"{path} missing — run `{hint}` first")
    with open(path) as f:
        return json.load(f)


@pytest.fixture(scope="session")
def fbx():
    return _load_or_skip(APARTMENT / "fbx_materials.json", "make assets")


@pytest.fixture(scope="session")
def max_table():
    raw = _load_or_skip(APARTMENT / "max_materials.json", "make assets")
    return material_plan.normalize_max_table(raw)


@pytest.fixture(scope="session")
def inventory():
    tex_dir = APARTMENT / "textures"
    if not tex_dir.is_dir():
        pytest.skip(f"{tex_dir} missing — run `make assets` first")
    return {f for f in os.listdir(tex_dir) if not f.startswith(".")}


@pytest.fixture(scope="session")
def plans(fbx, max_table, inventory):
    return build_plans(fbx, max_table, inventory)


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


def in_inventory(basename, inventory):
    lower = {f.lower(): f for f in inventory}
    if basename.lower() in lower:
        return lower[basename.lower()]
    stems = {os.path.splitext(f)[0].lower(): f for f in inventory}
    return stems.get(os.path.splitext(basename)[0].lower())


def channel_file(rec, channel_set, inventory):
    """The record's shipped file on any of the given channels, or None."""
    for chan, basename in rec["channels"].items():
        if chan in channel_set:
            hit = in_inventory(basename, inventory)
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
    map that shipped in the zip must plan exactly that texture."""
    dropped = []
    for name in assigned(fbx):
        rec = fbx["materials"][camera_facing(fbx, name)]
        f = channel_file(rec, DIFFUSE_CHANNELS, inventory)
        if f and plans[name]["base_color"].get("texture") != f:
            dropped.append((name, f, plans[name]["base_color"]))
    assert not dropped, f"declared diffuse maps dropped: {dropped}"


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


def test_bump_derived_normals_carry_the_authored_strength(fbx, plans, inventory):
    """Corona renders bump = levelBump x mapamount (typically well under
    1.0); a converted normal map without the authored strength shouts on
    every wall. The plan must carry it, clamped to the sane range."""
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
    """A plan may carry partial alpha ONLY as the exporter's declared
    1 - TransparencyFactor of the resolved record (the value Corona's own
    exporter computed from the refraction stack)."""
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
    Corona color, the std diffuse) — never a silent importer default."""
    silent = [n for n in assigned(fbx)
              if plans[n]["base_color"].get("source", "default") == "default"]
    assert not silent, f"materials shipping importer defaults: {silent}"


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
def glb():
    if not GLB.exists():
        pytest.skip(f"{GLB} missing — run `make assets` first")
    with open(GLB, "rb") as f:
        f.seek(12)
        length, _ = struct.unpack("<II", f.read(8))
        return json.loads(f.read(length))


def glb_base_name(name):
    """Blender dedup suffixes (.001) don't exist in the FBX name table."""
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
    # Window panes glow by design (apartment.py PANE_EMISSION); everything
    # else obeys the plan cap. The looser of the two bounds all of it.
    ceiling = max(EMISSION_CAP, 1.5) + 1e-3
    hot = []
    for m in glb.get("materials", []):
        strength = (m.get("extensions", {})
                    .get("KHR_materials_emissive_strength", {})
                    .get("emissiveStrength", 1.0))
        if strength > ceiling:
            hot.append((m.get("name"), strength))
    assert not hot, f"materials past the emission ceiling (white shapes): {hot}"


def test_glb_carries_every_assigned_material(glb, fbx, plans):
    """Container flattening must not LOSE materials: every assigned FBX
    material lands in the glb (under its own name; Blender may dedup)."""
    shipped = {glb_base_name(m.get("name", "")) for m in glb.get("materials", [])}
    lost = sorted(n for n in assigned(fbx) if n not in shipped)
    assert not lost, f"assigned materials absent from the glb: {lost}"
