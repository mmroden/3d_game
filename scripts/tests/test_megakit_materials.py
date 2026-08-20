"""Megakit material-mapping audit (`make test-assets`).

The megakit's import script (quaternius_import_script.gd) replaces a
mesh surface's embedded material ONLY when a same-named .tres exists in
addons/quaternius/materials/ — an unmapped name silently ships the
model's embedded material, whose external texture references do not
survive the pack install, so the surface renders as its bare albedo
factor. Worst case found: a material literally named `Collision` — the
pack's collision-helper meshes — RENDERS coplanar with the visual walls
on the AccentStrip/Band2 pieces, which is the z-fighting the owner saw
in play (2026-08-19).

This audit makes the import script's implicit contract explicit: every
material name embedded in a megakit model must have a .tres mapping. The
failure message names the unmapped materials, their albedo factors, and
the files that carry them. The pack ships .gltf (JSON) + .bin — a first
version of this audit scanned for .glb, found nothing, and passed
vacuously; the scan-count assertion exists so that can never recur.

XFAIL (owner, 2026-08-19): 22 unmapped materials are catalogued and the
triage (strip Collision surfaces at import; map or bless the rest) is
deliberately deferred — "I know we've gotta go back and fix that issue."
strict=True so the fix forces this marker's removal.
"""
import json
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
ADDON = ROOT / "godot" / "addons" / "quaternius"
MATERIALS = ADDON / "materials"


def embedded_materials(path):
    """(name, baseColorFactor) for every material in a .gltf."""
    gltf = json.loads(path.read_text())
    out = []
    for mat in gltf.get("materials", []):
        factor = mat.get("pbrMetallicRoughness", {}).get(
            "baseColorFactor", [1.0, 1.0, 1.0, 1.0])
        out.append((mat.get("name", "?"), factor))
    return out


@pytest.mark.xfail(
    strict=True,
    reason="22 unmapped megakit materials (incl. rendering `Collision` "
    "meshes = the wall z-fighting); triage deferred by owner 2026-08-19 — "
    "fixing the mappings makes this XPASS and forces the marker off",
)
def test_every_embedded_megakit_material_is_mapped():
    if not ADDON.is_dir():
        pytest.skip(f"{ADDON} missing — run `make assets` first")
    mapped = {p.stem for p in MATERIALS.glob("*.tres")}
    assert mapped, "megakit materials/ folder is empty — install is broken"
    models = sorted(ADDON.rglob("*.gltf"))
    assert len(models) > 100, (
        f"only {len(models)} megakit models found — the scan is not "
        f"reaching the pack (vacuous-pass guard)"
    )
    unmapped = {}
    for model in models:
        for name, factor in embedded_materials(model):
            if name not in mapped:
                entry = unmapped.setdefault(
                    name, {"factor": factor, "files": []})
                entry["files"].append(model.name)
    assert not unmapped, (
        "megakit materials with no .tres mapping (these ship the model's "
        "embedded material and render as bare albedo factor):\n"
        + "\n".join(
            f"  {name}: albedo {v['factor']} in {', '.join(sorted(set(v['files']))[:4])}"
            f"{' …' if len(set(v['files'])) > 4 else ''}"
            for name, v in sorted(unmapped.items())
        )
    )
