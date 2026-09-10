"""Unit tests for scripts/preview_plan.py — the framing rule behind
`make previews` (out/metrics/<model>_preview.png, the picture beside a
model's metrics TOML). The rule: the camera sits on the requested
azimuth/elevation, looks at the bounds center, and stands far enough
back that the model's bounding sphere fits the NARROWER field of view
with a margin of air. The Blender applier (render-preview.py) only
applies the pose; every number here derives from the inputs."""
import importlib.util
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))


def _load():
    spec = importlib.util.spec_from_file_location(
        "preview_plan", ROOT / "scripts" / "preview_plan.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


preview_plan = _load()

LO, HI = (-3.0, -1.0, -0.5), (1.0, 5.0, 1.5)  # an off-center, oblong box


def _sub(a, b):
    return tuple(x - y for x, y in zip(a, b))


def _norm(v):
    return math.sqrt(sum(x * x for x in v))


def test_the_camera_looks_at_the_bounds_center():
    pose = preview_plan.plan_preview(LO, HI)
    center = tuple((lo + hi) / 2 for lo, hi in zip(LO, HI))
    assert pose["look_at"] == center
    assert pose["radius"] == _norm(_sub(HI, LO)) / 2, \
        "the bounding sphere is the box's half-diagonal"


def test_the_bounding_sphere_fits_the_narrower_field_of_view():
    """Landscape frames are limited vertically; portrait frames
    horizontally. Either way the sphere just fits, margin included."""
    fov, margin = 40.0, 1.15
    for aspect in (4 / 3, 1.0, 1 / 2):
        pose = preview_plan.plan_preview(LO, HI, fov_deg=fov, aspect=aspect, margin=margin)
        half_v = math.radians(fov) / 2
        half_h = math.atan(math.tan(half_v) * aspect)
        limiting = min(half_v, half_h)
        assert pose["radius"] > 0 and pose["distance"] > 0, "a real box frames at a real distance"
        assert math.isclose(pose["distance"] * math.sin(limiting), pose["radius"] * margin,
                            rel_tol=1e-9), f"aspect {aspect}: the sphere must exactly fill the narrow axis"
        assert math.isclose(_norm(_sub(pose["camera"], pose["look_at"])), pose["distance"],
                            rel_tol=1e-9)


def test_the_camera_sits_on_the_requested_azimuth_and_elevation():
    """Azimuth 0 is dead ahead of the nose (the camera on -Y, Blender
    space), positive swings toward +X; elevation lifts the camera above."""
    pose = preview_plan.plan_preview(LO, HI, azimuth_deg=0.0, elevation_deg=0.0)
    d = _sub(pose["camera"], pose["look_at"])
    assert d[0] == 0.0 and d[2] == 0.0 and d[1] < 0, "azimuth 0, elevation 0: straight ahead on -Y"

    pose = preview_plan.plan_preview(LO, HI, azimuth_deg=90.0, elevation_deg=0.0)
    d = _sub(pose["camera"], pose["look_at"])
    assert d[0] > 0 and math.isclose(d[1], 0.0, abs_tol=1e-9), "azimuth 90 swings to +X"

    pose = preview_plan.plan_preview(LO, HI, azimuth_deg=35.0, elevation_deg=25.0)
    d = _sub(pose["camera"], pose["look_at"])
    dist = _norm(d)
    assert math.isclose(math.degrees(math.asin(d[2] / dist)), 25.0, rel_tol=1e-9)
    assert math.isclose(math.degrees(math.atan2(d[0], -d[1])), 35.0, rel_tol=1e-9)


def test_a_flat_or_empty_model_still_frames():
    """A zero-extent box (an empty import, a single point) must not put
    the camera on top of its subject: the radius has a floor."""
    pose = preview_plan.plan_preview((0.0, 0.0, 0.0), (0.0, 0.0, 0.0), min_radius=0.01)
    assert pose["radius"] == 0.01
    assert pose["distance"] > 0.0 and math.isfinite(pose["distance"])
