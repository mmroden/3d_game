"""Unit tests for scripts/retile.py — the rule that lifts a map laid too
thin across a surface to the texel floor by repeating it (owner
2026-09-08: "repeated tiling to increase texture resolution so we can
hit a high resolution target when the player flies close"; 2026-09-09:
"I'd rather you just increase the repetition first"). Pure: edge
continuity is judged on pixel rows the test writes, the verdict and
the factor on numbers; the converter applies them inside Blender.

The one fidelity guard (owner 2026-09-09: the vendor's presentation is
the spec): a repeat may only ever show more of the surface's OWN map.
A face that does not wear the whole map along an axis is a patch of an
atlas, and repeating it paints the vendor's neighbors."""
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from retile import (  # noqa: E402
    AUTHOR_REPEATS_SPAN, COLLAPSED_UV_AREA, COVERED_TILE, FULL_TILE_TOLERANCE,
    RETILE_MAX, TEXEL_FLOOR_PER_WORLD_M, TILEABLE_ABOVE, atlas_island,
    collapsed_mapping, edge_continuity, edge_continuity_axes, picture,
    repeat_verdict, retile_factor, seamless,
)

BAR = TILEABLE_ABOVE
SPAN = AUTHOR_REPEATS_SPAN


def image(width, height, pixel):
    """A flat RGB pixel list [r, g, b, ...] row-major, from pixel(x, y)."""
    out = []
    for y in range(height):
        for x in range(width):
            out.extend(pixel(x, y))
    return out


def test_a_seamless_map_reads_as_continuous_across_both_edges():
    """A periodic pattern's last column matches its first, its last row
    its first: continuity near 1.0 on both axes."""
    periodic = image(32, 32, lambda x, y: (
        int(127 + 120 * math.sin(2 * math.pi * x / 32)),
        int(127 + 120 * math.cos(2 * math.pi * y / 32)),
        90))
    u, v = edge_continuity_axes(periodic, 32, 32)
    assert u > BAR and v > BAR
    assert edge_continuity(periodic, 32, 32) > BAR


def test_a_photograph_with_a_hard_edge_reads_as_discontinuous():
    """A gradient that runs dark to bright across the image: the two
    vertical edges are as different as the map gets, so u fails; the
    rows all match, so v passes — and the mean stays under the bar."""
    ramp = image(32, 32, lambda x, y: (int(255 * x / 31), int(255 * x / 31), int(255 * x / 31)))
    u, v = edge_continuity_axes(ramp, 32, 32)
    assert u < BAR and v > BAR
    assert edge_continuity(ramp, 32, 32) < BAR


def test_a_wainscot_band_continues_sideways_but_not_up():
    """The office's wall map (run 41, 2026-09-09): a band that runs the
    width of the map (periodic in x) over a field that is dark at the
    top and bright at the bottom. Left continues into right; top does
    not continue into bottom."""
    band = image(32, 32, lambda x, y: (
        int(127 + 100 * math.sin(2 * math.pi * x / 32)) if 12 <= y < 20 else int(255 * y / 31),
        int(255 * y / 31), 60))
    u, v = edge_continuity_axes(band, 32, 32)
    assert u > BAR and v < BAR


def test_a_baked_atlas_with_a_border_is_not_seamless():
    """A prop atlas: islands on a mid-gray field with a bright border
    on one side (the classic padding artifact) — discontinuous along u."""
    atlas = image(32, 32, lambda x, y: (250, 250, 250) if x == 31 else (120, 120, 120))
    u, v = edge_continuity_axes(atlas, 32, 32)
    assert u < BAR
    assert edge_continuity(atlas, 32, 32) < BAR


def test_seamless_needs_each_axis_repeated_by_the_author_or_continuous():
    """A repeat multiplies both axes, so a seamless repeat needs both:
    the office floor (faces over three tiles both ways) is seamless
    whatever its edges score; the wainscot wall (two tiles wide, one
    band tall, top not continuing into bottom) is not — x3 stacked
    three wainscots up the wall (run 41); a unique unwrap is seamless
    only when its edges continue both ways."""
    assert seamless((0.5, 0.5), (3.3, 3.3)) is True
    assert seamless((0.844, 0.844), (SPAN, SPAN)) is True
    assert seamless((0.9, 0.5), (2.15, 0.8)) is False
    assert seamless((0.5, 0.9), (2.15, 0.8)) is True
    assert seamless((0.5, 0.5), (0.9, 0.9)) is False
    assert seamless((BAR, 0.5), (0.9, 0.9)) is False
    assert seamless((BAR, BAR), (0.9, 0.9)) is True


def test_an_atlas_island_is_never_repeated():
    """Runs 44-48 (2026-09-09): the office is baked per object into
    2048 atlases — its posters, sofa, drawers, lamps, doors and its big
    Wall_B each wear patches of a map whose other regions hold other
    things — and scaling a patch's UVs shows the whole atlas across the
    face (the wall posters became grids of nine; Wall_B x13 would show
    the atlas thirteen times). The material's UNION footprint says
    nothing (Wall_B's faces together cover the map), and neither does
    the island count alone: the villa's wood floor is 3,457 slivers of
    a map whose edges continue both ways, and a repeat of a SEAMLESS
    map shows only more of that one material (run 48 held it by
    mistake). An axis is an atlas axis when no island wears
    COVERED_TILE of the map along it AND the map's edges do not
    continue along it. The wainscot wall is the one-axis case: two
    tiles wide, continuous sideways, 0.67 of the map tall with a top
    that does not continue into the bottom — x3 stacked three bands
    with a stripe of foreign content."""
    assert atlas_island((0.66, 0.66), (0.62, 0.77)) is True
    assert atlas_island((0.92, 0.33), (0.36, 0.62)) is True
    assert atlas_island((2.15, 0.67), (0.97, 0.66)) is True
    assert atlas_island((0.02, 0.02), (0.83, 0.85)) is True
    assert atlas_island((0.45, 0.36), (0.967, 0.967)) is False
    assert atlas_island((3.3, 2.7), (0.5, 0.5)) is False
    assert atlas_island((1.0, 1.0), (0.4, 0.4)) is False
    assert atlas_island((COVERED_TILE, COVERED_TILE), (0.4, 0.4)) is False
    assert repeat_verdict((0.62, 0.77), (0.66, 0.66), uv_area=6.6, alpha=None) == "held"
    assert repeat_verdict((0.36, 0.62), (0.92, 0.33), uv_area=28.4, alpha=None) == "held"
    assert repeat_verdict((0.97, 0.66), (2.15, 0.67), uv_area=139.1, alpha=None) == "held"
    assert repeat_verdict((0.967, 0.967), (0.45, 0.36), uv_area=0.17, alpha=None) == "seamless"
    assert repeat_verdict((0.826, 0.918), (0.47, 0.46), uv_area=0.34, alpha=None) == "held"
    assert COVERED_TILE == 0.9


def test_a_face_wearing_its_whole_map_repeats_seamed_or_seamless():
    """Owner 2026-09-09: "I'd rather you just increase the repetition
    first". A face that wears the whole map along both axes shows only
    its own map when repeated: seamless when each axis is either
    continuous at the edges (a tiling material SketchUp maps per face —
    2,927 of the hill house's) or already repeated by the author (the
    office floor, three tiles across both ways); seamed when some axis
    is neither (planks laid 1.5 tiles across on hard edges)."""
    assert repeat_verdict((0.95, 0.96), (1.0, 1.0), uv_area=1.4, alpha=None) == "seamless"
    assert repeat_verdict((0.5, 0.5), (3.3, 3.3), uv_area=300.0, alpha=None) == "seamless"
    assert repeat_verdict((0.99, 0.99), (17.6, 15.6), uv_area=300.0, alpha=None) == "seamless"
    assert repeat_verdict((0.5, 0.9), (2.15, 1.5), uv_area=139.1, alpha=None) == "seamless"
    assert repeat_verdict((0.5, 0.5), (1.5, 1.4), uv_area=40.0, alpha=None) == "seamed"
    assert repeat_verdict((0.5, 0.5), (2.15, 1.5), uv_area=139.1, alpha=None) == "seamed"


def test_a_picture_is_never_repeated():
    """A face wearing the whole map once whose edges do NOT continue is
    a picture pinned to a surface — a poster on its own map, a backdrop
    photograph (the apartment's ZJ-001346, 1.0 x 1.0 at 24 texels per
    meter would have gridded x21); a material with an alpha silhouette
    (the plant card: its map IS its outline); and a collapsed mapping
    (the map sampled as a color) — all stay as shipped."""
    assert picture((1.0, 0.98), (0.4, 0.5)) is True
    assert picture((1.0, 1.0), (0.95, 0.96)) is False
    assert picture((1.5, 1.4), (0.4, 0.5)) is False
    assert picture((3.3, 2.7), (0.5, 0.5)) is False
    assert repeat_verdict((0.4, 0.5), (1.0, 0.98), uv_area=6.0, alpha=None) == "held"
    assert repeat_verdict((0.79, 1.0), (1.0, 1.0), uv_area=12.0, alpha=None) == "held"
    assert repeat_verdict((0.99, 0.99), (1.0, 1.0), uv_area=0.55,
                          alpha={"cutout_texture": "plant.png"}) == "held"
    assert repeat_verdict((0.99, 0.99), (1.0, 1.0), uv_area=0.9,
                          alpha={"value": 0.3}) == "held"
    assert repeat_verdict((0.99, 0.99), (1.0, 1.0), uv_area=COLLAPSED_UV_AREA / 2,
                          alpha=None) == "held"
    assert FULL_TILE_TOLERANCE == 0.1


def test_the_factor_repeats_a_thin_map_up_to_the_floor_and_never_shrinks():
    """A map laid at 22 texels per world meter needs the floor over 22,
    rounded up so the floor is met, not approached; a map already past
    the floor is left alone (factor 1); a surface with no density (no
    map, no area) has no factor."""
    assert retile_factor(22.0, TEXEL_FLOOR_PER_WORLD_M) == 23
    assert retile_factor(250.0, 500.0) == 2
    assert retile_factor(500.0, 500.0) == 1
    assert retile_factor(1200.0, 500.0) == 1
    assert retile_factor(0.0, 500.0) is None
    assert retile_factor(None, 500.0) is None


def test_a_mapping_too_far_under_the_floor_is_not_a_tiling_layout():
    """The apartment's first retile run (2026-09-09) handed three
    materials factors of 849, 921 and 1215: maps laid at under one texel
    per meter. That is not a tiling layout spread thin, it is a mapping
    that samples the map as a color (collapsed or decal UVs), and
    repeating it a thousand times paints a picture the author never
    made. Past RETILE_MAX the factor is refused."""
    assert retile_factor(TEXEL_FLOOR_PER_WORLD_M / RETILE_MAX, TEXEL_FLOOR_PER_WORLD_M) == RETILE_MAX
    assert retile_factor(TEXEL_FLOOR_PER_WORLD_M / (RETILE_MAX + 1), TEXEL_FLOOR_PER_WORLD_M) is None
    assert retile_factor(0.4, 500.0) is None


def test_a_collapsed_mapping_is_not_retiled():
    """A material whose whole UV footprint covers less than
    COLLAPSED_UV_AREA of one tile samples the map as a color; a real
    mapping laid once across a wall covers about a tile."""
    assert collapsed_mapping(0.0001) is True
    assert collapsed_mapping(COLLAPSED_UV_AREA / 2) is True
    assert collapsed_mapping(COLLAPSED_UV_AREA * 2) is False
    assert collapsed_mapping(1.0) is False


def test_the_floor_is_the_audits_floor():
    """One home for the number the converter aims at and the audit reads
    as its yardstick (owner 2026-09-06: 500 texels per meter as the
    minimum)."""
    assert TEXEL_FLOOR_PER_WORLD_M == 500.0
