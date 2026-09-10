"""Preview framing for `make previews` — pure Python, no Blender.

A model's rendered preview (out/metrics/<model>_preview.png, the picture
beside its metrics TOML) frames the model from its own bounds: the
camera sits on a chosen azimuth/elevation, stands far enough back that
the bounding sphere fits the narrower field of view with a margin of
air, and looks at the bounds center. scripts/render-preview.py APPLIES
this pose in Blender and decides nothing of its own. (owner 2026-09-09:
"maybe we render a chase plane view of it?" — the Talon hull's seller
was the last credit unaccounted for.)
"""
import math


def plan_preview(lo, hi, azimuth_deg=35.0, elevation_deg=25.0, fov_deg=40.0,
                 aspect=4 / 3, margin=1.15, min_radius=0.01):
    """Camera pose for the box [lo, hi] in Blender space (Z up; azimuth 0
    puts the camera on -Y, dead ahead of a nose that points -Y, positive
    swings it toward +X; elevation lifts it above). `fov_deg` is the
    VERTICAL field of view, `aspect` width/height; the bounding sphere
    (radius floored at `min_radius`, so an empty or flat import still
    frames) fills the narrower axis with `margin` of air. Returns
    {"look_at", "camera", "distance", "radius"}."""
    look_at = tuple((a + b) / 2 for a, b in zip(lo, hi))
    extent = [b - a for a, b in zip(lo, hi)]
    radius = max(math.sqrt(sum(x * x for x in extent)) / 2, min_radius)
    half_v = math.radians(fov_deg) / 2
    half_h = math.atan(math.tan(half_v) * aspect)
    distance = radius * margin / math.sin(min(half_v, half_h))
    az, el = math.radians(azimuth_deg), math.radians(elevation_deg)
    direction = (math.sin(az) * math.cos(el), -math.cos(az) * math.cos(el), math.sin(el))
    camera = tuple(c + d * distance for c, d in zip(look_at, direction))
    return {"look_at": look_at, "camera": camera, "distance": distance, "radius": radius}
