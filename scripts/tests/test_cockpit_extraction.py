"""Cockpit-extraction audit (`make test-assets`).

Every hull with a furnished interior yields a cockpit shell through ONE
rule (scripts/cockpit_plan.py); these tests hold the plan and the built
shell (godot/addons/ships/<hull>_cockpit.glb) to that rule, per hull.
The hull source is the AUTHORING TRUTH — a provider .glb (the Vanguard)
or the pipeline's own conversion of a provider FBX (the military ship,
scripts/convert-hull.py). Every expectation is DERIVED from the source
through the rule — no part names, no counts, no coordinates are pinned,
so retuning the rule's margins, a provider update, or a new hull
re-audits itself instead of breaking here.

Plan contracts run against the source directly; artifact contracts skip
(loudly) until `make assets` has built the shell — and a converted
source skips the same way until the pipeline has produced it.
"""
import json
import struct
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SHIPS = ROOT / "godot" / "addons" / "ships"

# hull -> (source hull .glb the rule reads, the shell `make assets` builds)
HULLS = {
    "vanguard": (
        ROOT / "assets" / "cgtrader_ships" / "Spaceship_1" / "Spacecraft_1.glb",
        SHIPS / "vanguard_cockpit.glb",
    ),
    "military_ship": (
        SHIPS / "military_ship.glb",
        SHIPS / "military_ship_cockpit.glb",
    ),
}

sys.path.insert(0, str(ROOT / "scripts"))
from cockpit_plan import (  # noqa: E402
    EYE_AFT_FRAC,
    EYE_HEIGHT_FRAC,
    ROUGHNESS_FLOOR,
    SEAT_DEPTH_UNDER_SILL,
    parse_glb,
    plan_cockpit,
)


@pytest.fixture(scope="session", params=sorted(HULLS))
def hull(request):
    return request.param


@pytest.fixture(scope="session")
def parts(hull):
    source, _shell = HULLS[hull]
    if not source.exists():
        pytest.skip(f"{source} missing — download the paid asset packs / run `make assets`")
    return parse_glb(source)


@pytest.fixture(scope="session")
def plan(parts):
    return plan_cockpit(parts)


def by_name(parts):
    return {p["name"]: p for p in parts}


def inside(lo, hi, part):
    return all(lo[k] <= part["lo"][k] and part["hi"][k] <= hi[k] for k in range(3))


def glb_json(path):
    with open(path, "rb") as f:
        _magic, _version, _length = struct.unpack("<III", f.read(12))
        clen, _ctype = struct.unpack("<II", f.read(8))
        return json.loads(f.read(clen))


# ---- plan-level contracts ----

def test_canopy_is_the_largest_glass_part(parts, plan):
    glass = [p for p in parts if p["blend"]]
    assert glass, "source hull has no alpha-blended part to anchor the rule on"
    largest = max(
        glass,
        key=lambda p: (p["hi"][0] - p["lo"][0])
        * (p["hi"][1] - p["lo"][1])
        * (p["hi"][2] - p["lo"][2]),
    )
    assert plan["canopy"] == largest["name"]


def test_keep_and_drop_partition_the_hull(parts, plan):
    names = {p["name"] for p in parts}
    kept, dropped = set(plan["keep"]), set(plan["drop"])
    assert kept | dropped == names
    assert not kept & dropped
    assert kept, "empty cockpit — the rule selected nothing"
    assert dropped, "rule swallowed the whole hull — margins are wrong"


def test_selection_is_exactly_volume_containment(parts, plan):
    # The rule: keep = opaque AND fully inside the volume. A dropped part
    # must fail at least one of the two (glass panes fail opacity even
    # when they sit inside — see test_cockpit_keeps_no_glass).
    lo, hi = plan["volume"]
    table = by_name(parts)
    for name in plan["keep"]:
        part = table[name]
        assert not part["blend"], f"kept part {name} is glass"
        assert inside(lo, hi, part), f"kept part {name} leaves the volume"
    for name in plan["drop"]:
        part = table[name]
        assert part["blend"] or not inside(lo, hi, part), \
            f"dropped part {name} is opaque and fits the volume"


def test_volume_contains_the_canopy(parts, plan):
    lo, hi = plan["volume"]
    assert inside(lo, hi, by_name(parts)[plan["canopy"]])


def test_cockpit_keeps_no_glass(parts, plan):
    # The canopy PANES leave the shell entirely (owner, 2026-08-20): at
    # neutral near-clear alpha they contributed nothing but specular —
    # a cache's glow focused into a distracting flare on the pane. The
    # canopy still ANCHORS the selection volume and eyepoint; its glass
    # just doesn't ship. The floating-window frame is the opaque bows.
    table = by_name(parts)
    assert not any(table[n]["blend"] for n in plan["keep"]), \
        "a blend (glass) part survived the keep rule — panes must not ship"


def test_cockpit_is_a_strict_subset_of_the_hull_geometry(parts, plan):
    table = by_name(parts)
    kept = sum(table[n]["tris"] for n in plan["keep"])
    total = sum(p["tris"] for p in parts)
    assert 0 < kept < total


def test_cockpit_is_furnished_under_the_sill(parts, plan):
    # A shell is furniture around the pilot, not just the canopy bows:
    # something kept must reach at least a seat pan's depth below the
    # glass line. The military ship (2026-09-06) is the failure this
    # pins — its whole interior is one part whose floor sat 6 cm past
    # the Vanguard-tuned DOWN_MARGIN, and the "shell" that came out was
    # the bare canopy frame.
    table = by_name(parts)
    sill = table[plan["canopy"]]["lo"][1]
    deepest = sill - min(table[n]["lo"][1] for n in plan["keep"])
    assert deepest >= SEAT_DEPTH_UNDER_SILL, (
        f"kept furniture reaches only {deepest:.3f} m under the sill "
        f"(< {SEAT_DEPTH_UNDER_SILL}) — an empty frame, not a cockpit"
    )


def test_eyepoint_sits_inside_the_canopy(parts, plan):
    canopy = by_name(parts)[plan["canopy"]]
    x, y, z = plan["eyepoint"]
    assert canopy["lo"][0] < x < canopy["hi"][0]
    span = canopy["hi"][1] - canopy["lo"][1]
    assert abs(y - (canopy["lo"][1] + EYE_HEIGHT_FRAC * span)) < 1e-6
    assert canopy["lo"][1] < y < canopy["hi"][1], \
        "eyepoint outside the glass vertically — the pilot's head is in the hull"
    depth = canopy["hi"][2] - canopy["lo"][2]
    assert abs(z - (canopy["lo"][2] + EYE_AFT_FRAC * depth)) < 1e-6, \
        "eyepoint must sit at the aft-biased seat position, not mid-glass"
    assert canopy["lo"][2] < z < canopy["hi"][2]


# ---- artifact contracts: the built shell honors the plan ----

@pytest.fixture(scope="session")
def shell_path(hull):
    _source, shell = HULLS[hull]
    if not shell.exists():
        pytest.skip(f"{shell} missing — run `make assets` first")
    return shell


@pytest.fixture(scope="session")
def shell_parts(shell_path):
    return parse_glb(shell_path)


@pytest.fixture(scope="session")
def shell_gltf(shell_path):
    return glb_json(shell_path)


def test_shell_carries_exactly_the_planned_parts(plan, shell_parts):
    assert {p["name"] for p in shell_parts} == set(plan["keep"])


def test_shell_preserves_full_part_detail(parts, plan, shell_parts):
    # Compared on DISTINCT AREA-BEARING triangles: the Blender round-trip
    # may weld exact-duplicate faces and prune zero-area degenerates —
    # both render-invisible normalizations — but one distinct real
    # triangle gone is decimation and still fails here.
    source = by_name(parts)
    for p in shell_parts:
        assert p["distinct_solid_tris"] == source[p["name"]]["distinct_solid_tris"], \
            f"{p['name']} lost geometry in extraction — the shell must not decimate"


def test_shell_carries_no_blend_materials(shell_parts):
    # The pane-free shell must not smuggle alpha-blend surfaces back in
    # (see test_cockpit_keeps_no_glass for the why).
    assert not any(p["blend"] for p in shell_parts)


def test_shell_bakes_the_eyepoint_node(plan, shell_gltf):
    empties = [n for n in shell_gltf["nodes"]
               if "mesh" not in n and n.get("name") == "Eyepoint"]
    assert len(empties) == 1, "shell must carry exactly one Eyepoint empty"
    got = empties[0].get("translation", [0.0, 0.0, 0.0])
    want = plan["eyepoint"]
    assert all(abs(g - w) < 1e-3 for g, w in zip(got, want)), \
        f"Eyepoint {got} drifted from the planned {want}"


def test_shell_interior_is_matte(shell_gltf):
    # Near-field comfort: glossy furniture centimeters from the eyes
    # mirrors the passing world and shimmers with every pose — it broke
    # the rig's pose-invariance contract (2026-08-19) before it ever
    # reached a headset. The extraction floors opaque materials'
    # roughness and drops their metal-roughness maps; canopy glass is
    # exempt (alpha IS its character), emissive screens keep their glow.
    opaque = [m for m in shell_gltf.get("materials", [])
              if m.get("alphaMode", "OPAQUE") not in ("BLEND", "MASK")]
    assert opaque, "shell carries opaque interior materials"
    for mat in opaque:
        pbr = mat.get("pbrMetallicRoughness", {})
        name = mat.get("name", "?")
        assert "metallicRoughnessTexture" not in pbr, \
            f"{name}: metal-roughness map survived — the shell must be matte"
        assert pbr.get("roughnessFactor", 1.0) >= ROUGHNESS_FLOOR - 1e-6, \
            f"{name}: roughness {pbr.get('roughnessFactor')} under the floor"
