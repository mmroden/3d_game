"""UV islands: the connected pieces of a material's UV layout — pure
Python, no Blender (scripts/tests/test_uv_islands.py).

The retile rule (scripts/retile.py) asks whether a surface wears the
whole of its map or a patch of it, and the polygon is the wrong unit
to ask about: the villa's wood floor is one surface of several faces
that together wear one map laid once (run 47 held it as patches), while
an office wall is many objects each wearing a patch of a baked atlas.
Faces that share a vertex AND the same UV there are one piece of
layout — one island — and the island's extent is what a repeat shows
more of.

`island_spans(faces)` takes each face as a sequence of (vertex, u, v)
and returns the (u, v) extent of every island, in tiles.
"""


def island_spans(faces):
    """The (u_extent, v_extent) of each UV island among `faces`, each
    face a sequence of (vertex_index, u, v). Two faces belong to one
    island when they share a vertex at the same UV (to 1e-5); a face
    whose UVs touch no other face's is an island of its own (an
    exporter mapping every face 0..1 on its own — SketchUp — yields one
    island per face, each spanning a tile)."""
    parent = {}

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    keyed = []
    for face in faces:
        keys = [(v, round(u, 5), round(w, 5)) for v, u, w in face]
        for k in keys:
            parent.setdefault(k, k)
        head = find(keys[0])
        for k in keys[1:]:
            root = find(k)
            if root != head:
                parent[root] = head
        keyed.append(keys)
    bounds = {}
    for keys in keyed:
        root = find(keys[0])
        us = [k[1] for k in keys]
        vs = [k[2] for k in keys]
        lo_u, lo_v, hi_u, hi_v = min(us), min(vs), max(us), max(vs)
        b = bounds.get(root)
        if b is not None:
            lo_u, lo_v = min(lo_u, b[0]), min(lo_v, b[1])
            hi_u, hi_v = max(hi_u, b[2]), max(hi_v, b[3])
        bounds[root] = (lo_u, lo_v, hi_u, hi_v)
    return [(hi_u - lo_u, hi_v - lo_v) for (lo_u, lo_v, hi_u, hi_v) in bounds.values()]
