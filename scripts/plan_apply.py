"""Plan application: the MECHANISM half of every FBX/OBJ-sourced
conversion (`make assets`), run inside Blender.

scripts/material_plan.py decides what each material must carry (pure
Python, audited by `make test-assets`); this module wires those plans
into Blender materials with FULL authority over the base-color, alpha,
roughness, normal, and emission sockets — whatever the importer wired
(junk transparency, strength-1000 emission) is overridden by
plan-or-neutral, never merely supplemented. scripts/convert-environment.py
and scripts/convert-hull.py both apply plans through here; neither
carries socket policy of its own.

Textures resolve by basename: the pack's extracted archives first
(recursively — material_plan.texture_files, the same walk the audit
uses), then the images the model file itself embeds (the importer parks
those as packed datablocks — the military ship's diffuse and cockpit
maps ship only that way). Every image is capped ASPECT-TRUE at the
caller's cap on load (a square resize crushed the 1745x3927 wall
paneling once); numpy pixel ops derive the inverted roughness, tinted
diffuse, and height-to-normal images the plans call for.
"""
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from material_plan import texture_files  # noqa: E402


def _cap(img, tex_cap):
    """Downscale an image to fit tex_cap on its longest side, aspect-true."""
    if max(img.size) > tex_cap:
        w, h = img.size
        s = tex_cap / max(w, h)
        img.scale(max(1, round(w * s)), max(1, round(h * s)))


class _TextureStore:
    """Loads each shipped texture once, capped, by plan basename — from
    the extracted archives (recursively), or from the source's own
    embedded images."""

    def __init__(self, tex_root, tex_cap):
        self.tex_cap = tex_cap
        self.files = texture_files(tex_root)
        # The importer materializes an FBX-embedded image as a packed
        # datablock whose filepath still names the provider's file; the
        # importer may park the same file on several datablocks (one per
        # socket it wired) — the first is as good as any.
        self.packed = {}
        for img in bpy.data.images:
            if img.packed_file is not None and img.filepath:
                key = os.path.basename(img.filepath.replace("\\", "/")).lower()
                self.packed.setdefault(key, img)
        self.loaded = {}

    def load(self, basename, non_color=False):
        key = basename.lower()
        if key in self.loaded:
            img = self.loaded[key]
        else:
            path = self.files.get(key)
            if path:
                img = bpy.data.images.load(path)
            else:
                img = self.packed.get(key)
            if img is not None:
                # Cap before any pixel math.
                _cap(img, self.tex_cap)
            self.loaded[key] = img
        if img is not None and non_color:
            img.colorspace_settings.name = "Non-Color"
        return img


def _pixel_op(img, new_name, fn):
    """A derived image from numpy pixel math, packed, Non-Color."""
    import numpy as np
    existing = bpy.data.images.get(new_name)
    if existing is not None:
        return existing
    w, h = img.size
    px = np.empty(w * h * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    out_px = fn(px.reshape(h, w, 4))
    out = bpy.data.images.new(new_name, width=w, height=h)
    out.colorspace_settings.name = "Non-Color"
    out.pixels.foreach_set(out_px.astype(np.float32).ravel())
    out.pack()
    return out


def inverted(img, blend=None):
    """Corona glossiness -> roughness: 1 - x, optionally blended toward
    the scalar base at the AUTHORED map amount (Corona's mapamount)."""
    base = (blend or {}).get("base", 0.0)
    amount = (blend or {}).get("amount", 1.0)

    def fn(px):
        px[:, :, :3] = 1.0 - px[:, :, :3]
        if amount < 0.999:
            px[:, :, :3] = base * (1.0 - amount) + px[:, :, :3] * amount
        return px
    return _pixel_op(img, f"{img.name}.rough[{base:.2f},{amount:.2f}]", fn)


def tinted(img, blend):
    """A diffuse map at partial mapamount: the texture TINTS the authored
    color. Blended in the stored (sRGB-encoded) space against the
    gamma-encoded color — visually faithful for look purposes."""
    amount = blend["amount"]
    c = [pow(max(v, 0.0), 1.0 / 2.2) for v in blend["color"]]

    def fn(px):
        import numpy as np
        base = np.array(c, dtype=np.float32)
        px[:, :, :3] = base * (1.0 - amount) + px[:, :, :3] * amount
        return px
    return _pixel_op(
        img, f"{img.name}.tint[{c[0]:.2f},{c[1]:.2f},{c[2]:.2f},{amount:.2f}]", fn)


def height_to_normal(img):
    """Bump maps are greyscale height; glTF speaks normal maps. Neutral
    slope here — the AUTHORED per-material bump amount rides the
    NormalMap node instead, so one derived image serves every material
    wearing the map, each at its own strength."""
    import numpy as np

    def fn(px):
        hmap = px[:, :, 0]
        gx = np.roll(hmap, -1, axis=1) - np.roll(hmap, 1, axis=1)
        gy = np.roll(hmap, -1, axis=0) - np.roll(hmap, 1, axis=0)
        nz = np.ones_like(gx)
        length = np.sqrt(gx * gx + gy * gy + nz * nz)
        return np.stack(
            [(-gx / length + 1) / 2, (-gy / length + 1) / 2,
             (nz / length + 1) / 2, np.ones_like(gx)], axis=-1)
    return _pixel_op(img, img.name + ".normal", fn)


def image_node(mat, img):
    node = mat.node_tree.nodes.new("ShaderNodeTexImage")
    node.image = img
    return node


def plan_name(mat):
    """The plan key for a Blender material: its name with the importer's
    dedup suffix (\".001\") stripped."""
    return mat.name.rsplit(".", 1)[0] if mat.name[-4:-3] == "." else mat.name


def apply_plans(plans, tex_root, tex_cap):
    """Wire `plans` (material name -> plan) into every material in the
    scene. Returns (applied, wired, missing_file): materials that had a
    plan, textured base colors that landed, and base textures a plan
    named but the shipped inventory could not supply."""
    store = _TextureStore(tex_root, tex_cap)
    applied = wired = missing_file = 0
    for mat in bpy.data.materials:
        plan = plans.get(mat.name) or plans.get(plan_name(mat))
        if plan is None:
            print(f"plan_apply: WARNING no plan for material '{mat.name}'")
            continue
        mat.use_nodes = True
        bsdf = mat.node_tree.nodes.get("Principled BSDF")
        if bsdf is None:
            continue
        applied += 1

        # The plan has FULL authority over these sockets: whatever the
        # importer wired (junk transparency, strength-1000 emission) is
        # overridden by plan-or-neutral, never merely supplemented.
        for link in list(mat.node_tree.links):
            if link.to_node == bsdf and link.to_socket.name in (
                    "Base Color", "Alpha", "Roughness", "Normal",
                    "Emission Color", "Emission Strength"):
                mat.node_tree.links.remove(link)

        base = plan["base_color"]
        if "texture" in base:
            img = store.load(base["texture"])
            if img is not None:
                if base.get("blend"):
                    img = tinted(img, base["blend"])
                node = image_node(mat, img)
                mat.node_tree.links.new(node.outputs["Color"],
                                        bsdf.inputs["Base Color"])
                wired += 1
            else:
                missing_file += 1
        else:
            c = base["color"]
            bsdf.inputs["Base Color"].default_value = (c[0], c[1], c[2], 1.0)

        normal = plan.get("normal")
        if normal is not None:
            img = store.load(normal["texture"], non_color=True)
            if img is not None:
                if normal["kind"] == "height":
                    img = height_to_normal(img)
                node = image_node(mat, img)
                nmap = mat.node_tree.nodes.new("ShaderNodeNormalMap")
                # The authored bump amount, exported as normalTexture
                # scale — never a guessed constant.
                nmap.inputs["Strength"].default_value = normal.get("strength", 1.0)
                mat.node_tree.links.new(node.outputs["Color"], nmap.inputs["Color"])
                mat.node_tree.links.new(nmap.outputs["Normal"], bsdf.inputs["Normal"])

        rough = plan.get("roughness")
        if rough is not None:
            if "texture" in rough:
                img = store.load(rough["texture"], non_color=True)
                if img is not None:
                    if rough.get("invert"):
                        img = inverted(img, rough.get("blend"))
                    node = image_node(mat, img)
                    mat.node_tree.links.new(node.outputs["Color"],
                                            bsdf.inputs["Roughness"])
            else:
                bsdf.inputs["Roughness"].default_value = max(
                    0.0, min(1.0, rough["value"]))

        alpha = plan.get("alpha")
        if alpha is None:
            bsdf.inputs["Alpha"].default_value = 1.0
            mat.blend_method = "OPAQUE"
        elif "cutout_texture" in alpha:
            img = store.load(alpha["cutout_texture"], non_color=True)
            if img is not None:
                node = image_node(mat, img)
                mat.node_tree.links.new(node.outputs["Color"], bsdf.inputs["Alpha"])
                mat.blend_method = "CLIP"
                mat.alpha_threshold = 0.5
        else:
            bsdf.inputs["Alpha"].default_value = alpha["value"]
            mat.blend_method = "BLEND"

        emission = plan.get("emission")
        if emission is None:
            bsdf.inputs["Emission Strength"].default_value = 0.0
        else:
            if emission.get("texture"):
                img = store.load(emission["texture"])
                if img is not None:
                    node = image_node(mat, img)
                    mat.node_tree.links.new(node.outputs["Color"],
                                            bsdf.inputs["Emission Color"])
            else:
                c = emission["color"]
                bsdf.inputs["Emission Color"].default_value = (c[0], c[1], c[2], 1.0)
            bsdf.inputs["Emission Strength"].default_value = emission["strength"]
    return applied, wired, missing_file


def cap_and_pack_images(tex_cap):
    """Cap every file-backed image at tex_cap (aspect-true) and pack it,
    so the glTF exporter embeds the capped version — plan-wired maps,
    importer-loaded ones, and the source's own embedded images alike."""
    for img in bpy.data.images:
        if img.source != "FILE":
            continue
        _cap(img, tex_cap)
        img.pack()
