"""Unit tests for scripts/uv_islands.py — the connected pieces of a
material's UV layout, the unit the retile rule judges "wears the whole
map or a patch of it" on (run 47, 2026-09-09: judged per polygon, the
villa's one-map wood floor read as patches and lost its repeat)."""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from uv_islands import island_spans  # noqa: E402


def quad(vertices, uvs):
    return [(v, u, w) for v, (u, w) in zip(vertices, uvs)]


def spans(faces):
    return sorted(tuple(round(x, 3) for x in s) for s in island_spans(faces))


def test_faces_sharing_a_vertex_at_one_uv_are_one_island():
    """The villa floor: two quads share an edge (vertices 1 and 2) and
    agree on the UVs there — one island, spanning both quads' UVs."""
    left = quad([0, 1, 2, 3], [(0.0, 0.0), (0.5, 0.0), (0.5, 1.0), (0.0, 1.0)])
    right = quad([1, 4, 5, 2], [(0.5, 0.0), (1.0, 0.0), (1.0, 1.0), (0.5, 1.0)])
    assert spans([left, right]) == [(1.0, 1.0)]


def test_faces_with_their_own_uvs_are_their_own_islands():
    """SketchUp maps every face 0..1 on its own: no vertex shared, so
    each face is an island — each spanning a whole tile."""
    a = quad([0, 1, 2, 3], [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
    b = quad([4, 5, 6, 7], [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
    assert spans([a, b]) == [(1.0, 1.0), (1.0, 1.0)]


def test_a_shared_vertex_at_different_uvs_is_a_seam_not_a_join():
    """An atlas: two objects' patches may share a vertex in 3D (a
    corner) yet sit on different regions of the map — separate islands,
    each spanning its patch."""
    wall = quad([0, 1, 2, 3], [(0.0, 0.0), (0.7, 0.0), (0.7, 0.3), (0.0, 0.3)])
    trim = quad([2, 4, 5, 6], [(0.8, 0.8), (1.0, 0.8), (1.0, 1.0), (0.8, 1.0)])
    assert spans([wall, trim]) == [(0.2, 0.2), (0.7, 0.3)]


def test_islands_chain_through_intermediate_faces():
    """Three quads in a row: the first and last share nothing directly,
    but the middle joins them — one island."""
    a = quad([0, 1, 2, 3], [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)])
    b = quad([1, 4, 5, 2], [(1.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0)])
    c = quad([4, 6, 7, 5], [(2.0, 0.0), (3.0, 0.0), (3.0, 1.0), (2.0, 1.0)])
    assert spans([a, b, c]) == [(3.0, 1.0)]


def test_uvs_agree_to_a_tolerance_not_bitwise():
    """Exporters round: a shared vertex whose UVs differ in the sixth
    decimal is the same UV."""
    a = quad([0, 1, 2, 3], [(0.0, 0.0), (0.5, 0.0), (0.5, 1.0), (0.0, 1.0)])
    b = quad([1, 4, 5, 2], [(0.500001, 0.0), (1.0, 0.0), (1.0, 1.0), (0.5, 0.999999)])
    assert spans([a, b]) == [(1.0, 1.0)]


def test_no_faces_no_islands():
    assert island_spans([]) == []
