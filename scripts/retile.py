"""Retile: the rule that lifts a map laid too thin across a surface up
to the texel floor by repeating it — pure Python, no Blender.

A provider lays a 4096-wide wood map once across a 30 m floor and the
player, flying close, sees 22 texels per meter (the villa, 2026-09-08).
The map has the pixels; the mapping spends them too thinly. Repeating
it N times across the same surface multiplies the density by N with no
new pixels — planks 30 m long become planks 1.3 m long, which is also
what wood looks like. The owner asked for it (2026-09-09: "I'd rather
you just increase the repetition first").

THE ONE GUARD IS FIDELITY (owner 2026-09-09: the vendor's presentation
is the spec): a repeat may only ever show more of the surface's OWN
map. Scaling UVs by N shows N times the map along each axis from the
layout's own region outward, so a UV ISLAND (scripts/uv_islands.py: the
faces joined by shared vertices at shared UVs — one surface's layout)
that does not wear the whole map along an axis is a patch of an ATLAS,
and a repeat would show the atlas' other patches: the office is baked
per object into 2048 atlases, and its posters became grids of nine
(run 45), Wall_B x13 would show the atlas thirteen times (run 46).
Judged per island, not per polygon: the villa's wood floor is several
faces that together wear one map laid once (run 47 held it by mistake).

`repeat_verdict` says what a repeat costs, or why there is none:
  - "held": an atlas island (`atlas_island`: some axis along which no
    island wears COVERED_TILE of the map — this also covers a terrain
    photograph sampled in slivers); a PICTURE (`picture`: an island
    wearing the whole map once whose edges do not continue — a poster
    on its own map, a backdrop photograph); an ALPHA silhouette (a
    plant card's map is its outline); a COLLAPSED mapping (the map
    sampled as a color); a mapping so far under the floor that no sane
    repeat reaches it (`retile_factor` past RETILE_MAX). Held surfaces
    stay as shipped; their resolution is the source's ceiling. Lifting
    an atlas patch faithfully needs a different step (crop the patch
    into its own map, then repeat) — not done, the owner's call;
  - "seamless": on EACH axis the author already repeats the map (some
    island spans AUTHOR_REPEATS_SPAN tiles or more) or its edges
    continue (`edge_continuity_axes` at TILEABLE_ABOVE or above);
  - "seamed": the repeat shows the map's edges as seams.

Every number here is MINE (not the owner's) except the floor; each is
inspectable per surface in the scene's conversion report (`retile`).

Pure functions, unit-tested (scripts/tests/test_retile.py):
`edge_continuity_axes` (and `edge_continuity`, their mean), `seamless`,
`atlas_island`, `picture`, `repeat_verdict`, `collapsed_mapping`,
`retile_factor`. scripts/convert-environment.py applies them inside
Blender (pixels from bpy; island spans and areas per material; the
plan's alpha); the audit reads TEXEL_FLOOR_PER_WORLD_M, which lives
here, as its yardstick — one home for the converter's target and the
audit's number.
"""
import math

# Texels per WORLD meter a textured surface the player walks past should
# lay down (owner 2026-09-06: "could we enforce something like 500
# texels/meter as a minimum?"; 2026-09-08: "a high resolution target when
# the player flies close to objects"). The converter's target; the
# audit's YARDSTICK (reported, not a gate — owner 2026-09-09).
TEXEL_FLOOR_PER_WORLD_M = 500.0

# Mine: edge continuity above this reads as a tiling map along that
# axis. A periodic texture scores near 1.0 (its opposite edges are the
# same pixels); a photograph or a baked atlas scores by how far its
# opposite edges differ, on a 0..1 scale of the map's own contrast — a
# hard border or unrelated islands land well under the bar.
TILEABLE_ABOVE = 0.85

# Mine: an island whose UVs span at least this many tiles along an axis
# is laid out by an author who repeats the map along it: a tiled layout
# on that axis, whatever the map's edges do.
AUTHOR_REPEATS_SPAN = 2.0

# Mine: an island wears the whole map along an axis when its UV span
# there reaches this much of a tile; under it the island is a patch of
# a larger map, and a repeat would show the rest of that map.
COVERED_TILE = 0.9

# Mine: an island wearing the whole map once — both UV spans within
# this tolerance of one tile — is a picture pinned to a surface when
# the map's edges do not continue (a poster, a screen); when they do,
# it is a tiling material an exporter mapped per face (SketchUp).
FULL_TILE_TOLERANCE = 0.1

# Pixels sampled per edge (evenly spaced), enough for a verdict on an
# 8192-wide map without walking every pixel of it.
EDGE_SAMPLES = 256

# Mine: the largest repeat the rule will apply. A mapping that needs
# more than this to reach the floor is not a tiling layout laid thin;
# it is a collapsed or decal mapping that samples the map as a color,
# and repeating it further paints a picture the author never made (the
# apartment's first run asked for x1215, 2026-09-09).
RETILE_MAX = 64

# Mine: a material whose whole UV footprint covers less than this
# fraction of one tile samples its map as a color, whatever its density
# reads.
COLLAPSED_UV_AREA = 0.01


def seamless(continuity_uv, island_span_uv):
    """Whether a repeat shows no new seam: on EACH axis the author
    already repeats the map (an island spanning AUTHOR_REPEATS_SPAN
    tiles or more along it) or its edges continue along it (continuity
    at TILEABLE_ABOVE or above). `continuity_uv` is (u, v) from
    `edge_continuity_axes`; `island_span_uv` the largest (u, v) extent,
    in tiles, of any UV island wearing the material."""
    return all(span >= AUTHOR_REPEATS_SPAN or continuity >= TILEABLE_ABOVE
               for continuity, span in zip(continuity_uv, island_span_uv))


def atlas_island(island_span_uv, continuity_uv):
    """Whether the material wears patches of a larger map: along some
    axis no island spans COVERED_TILE of the tile AND the map's edges do
    not continue (a seamless map is one material through and through —
    the villa's wood floor, 3,457 slivers of it — and a repeat shows
    only more of it; an atlas is many things, and its edges say so)."""
    return any(span < COVERED_TILE and continuity < TILEABLE_ABOVE
               for span, continuity in zip(island_span_uv, continuity_uv))


def picture(island_span_uv, continuity_uv):
    """Whether the map is a picture pinned to its surface rather than a
    material: an island wears the whole of it once (both spans within
    FULL_TILE_TOLERANCE of a tile) and its edges do not continue on
    both axes (a poster, a backdrop photograph; a per-face-mapped
    tiling material continues, and is not one)."""
    whole = all(abs(s - 1.0) <= FULL_TILE_TOLERANCE for s in island_span_uv)
    continuous = all(c >= TILEABLE_ABOVE for c in continuity_uv)
    return whole and not continuous


def repeat_verdict(continuity_uv, island_span_uv, uv_area, alpha):
    """"seamless", "seamed" or "held" — what repeating this surface's
    map would cost, or why it must not be repeated. `alpha` is the
    material plan's alpha entry (a cutout texture or a scalar) or
    None."""
    if (alpha or collapsed_mapping(uv_area) or atlas_island(island_span_uv, continuity_uv)
            or picture(island_span_uv, continuity_uv)):
        return "held"
    return "seamless" if seamless(continuity_uv, island_span_uv) else "seamed"


def collapsed_mapping(uv_area):
    """Whether a material's UV footprint (in tiles, summed over its
    faces) is too small to be a picture: under COLLAPSED_UV_AREA it
    samples the map as a color, and no repeat should touch it."""
    return uv_area < COLLAPSED_UV_AREA


def _channel_range(pixels, width, height, channels):
    """(min, max) over a sparse sample of the map — the contrast the edge
    differences are measured against."""
    step = max(1, (width * height) // (EDGE_SAMPLES * EDGE_SAMPLES))
    lo, hi = 255.0, 0.0
    for i in range(0, width * height, step):
        for c in range(min(3, channels)):
            v = pixels[i * channels + c]
            if v < lo:
                lo = v
            if v > hi:
                hi = v
    return lo, hi


def edge_continuity_axes(pixels, width, height, channels=3):
    """(u, v) continuity, each 0..1: u is how well the last column
    continues into the first (the map repeats left to right), v how well
    the last row continues into the first (top to bottom), lower as
    they differ relative to the map's own contrast. `pixels` is a flat
    row-major list, `channels` values per pixel (RGB or RGBA; alpha is
    ignored). A map of no contrast (a solid color) is trivially
    continuous both ways."""
    if width < 2 or height < 2:
        return (1.0, 1.0)
    lo, hi = _channel_range(pixels, width, height, channels)
    contrast = hi - lo
    if contrast <= 0.0:
        return (1.0, 1.0)
    use = min(3, channels)

    def at(x, y, c):
        return pixels[(y * width + x) * channels + c]

    total_u = total_v = 0.0
    n = 0
    for k in range(EDGE_SAMPLES):
        y = (k * (height - 1)) // max(1, EDGE_SAMPLES - 1)
        x = (k * (width - 1)) // max(1, EDGE_SAMPLES - 1)
        for c in range(use):
            total_u += abs(at(0, y, c) - at(width - 1, y, c))
            total_v += abs(at(x, 0, c) - at(x, height - 1, c))
            n += 1
    return (max(0.0, 1.0 - (total_u / n) / contrast),
            max(0.0, 1.0 - (total_v / n) / contrast))


def edge_continuity(pixels, width, height, channels=3):
    """The mean of the two axes' continuity (`edge_continuity_axes`)."""
    u, v = edge_continuity_axes(pixels, width, height, channels)
    return (u + v) / 2.0


def retile_factor(density_per_world_m, floor=TEXEL_FLOOR_PER_WORLD_M):
    """The integer repeat that lifts a map's measured density (texels
    per world meter) to the floor: ceil(floor / density), never under 1 —
    a map already past the floor keeps its mapping. None when the
    surface has no density to lift (no map, no area), and None past
    RETILE_MAX: a mapping that far under the floor is not a tiling
    layout, and the rule refuses to repaint it."""
    if not density_per_world_m or density_per_world_m <= 0.0:
        return None
    factor = max(1, int(math.ceil(floor / density_per_world_m)))
    return factor if factor <= RETILE_MAX else None
