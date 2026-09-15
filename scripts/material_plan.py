"""Material plan: the POLICY half of every FBX/OBJ-sourced conversion
(the planet-3 environments, the converted player hulls), pure Python —
no Blender.

Input: a material manifest (extract-fbx-materials.py for FBX,
mtl_materials.py for OBJ/MTL — the same JSON shape), the .max material
table when the pack ships one (extract-max-materials.py), and the
shipped texture inventory. Output: one plan per material name saying
exactly what the Godot-facing material must carry — base color, normal,
roughness, alpha, emission — with a PROVENANCE tag on every field, so
"why does this surface look wrong" is a lookup, not an investigation.

scripts/plan_apply.py wires the plans inside Blender (mechanism, shared
by convert-environment.py and convert-hull.py);
scripts/tests/test_asset_pipeline.py audits them (`make test-assets`).
"""
import os
import re

# Emission ceiling for provider-authored light materials (Corona LightMtl
# and self-illuminated surfaces). Corona treats these as photometric light
# SOURCES (the apartment ships sky cards at strength 1000); Godot renders
# them as surfaces, so anything past a gentle glow clips to a white shape.
# One look knob, owner-retunable (mine: the number is not the owner's).
EMISSION_CAP = 3.0

# Channel names, canonicalized across the manifests: Corona's (3ds Max
# exports), the standard FBX set (Blender exports carry their maps
# there), and Wavefront MTL statements. Anything not mapped here and not
# WAIVED is an unknown channel — the audit flags it, never drops it
# silently.
DIFFUSE_CHANNELS = {
    "DiffuseColor",
    "3dsMax|CoronaMtlPb|texmapDiffuse",
    "3dsMax|CoronaPhysicalMtlPb|baseTexmap",
    "map_Kd",
}
# Height (greyscale bump) maps the applier converts to normals.
BUMP_CHANNELS = {
    "3dsMax|CoronaMtlPb|texmapBump",
    "3dsMax|CoronaPhysicalMtlPb|baseBumpTexmap",
    "Bump",
    "map_Bump", "map_bump", "bump",
}
# Real tangent-space normal maps (a Blender-exported FBX's NormalMap).
NORMAL_CHANNELS = {
    "NormalMap",
}
GLOSS_CHANNELS = {  # Corona glossiness = 1 - roughness: invert on wire
    "3dsMax|CoronaMtlPb|texmapReflectGlossiness",
}
ROUGHNESS_CHANNELS = {
    "3dsMax|CoronaPhysicalMtlPb|baseRoughnessTexmap",
}
# Cutout (alpha mask) maps. A Blender-exported FBX parks its alpha
# texture on TransparencyFactor (the office building's "AlfaMask").
OPACITY_CHANNELS = {
    "3dsMax|CoronaMtlPb|texmapOpacity",
    "3dsMax|LightMtlPb|opacityTexmap",
    "TransparencyFactor",
    "map_d",
}
SELFILLUM_CHANNELS = {
    "3dsMax|CoronaMtlPb|texmapSelfIllum",
    "map_Ke",
}
# Channels consciously not represented in a real-time StandardMaterial3D.
# Every shipped texture referenced ONLY through these gets a named waiver
# in the usage report instead of counting as unexplained.
WAIVED_CHANNEL_MARKERS = (
    "texmapReflect",           # reflection color mask (env-mapped IBL later)
    "texmapDisplace",          # displacement — no runtime tessellation
    "texmapTranslucency",      # thin-surface translucency
    "mixmaps",                 # LayeredMtl blend masks (base layer only)
    "TransparentColor",
    "texmapRefract",
    "ShininessExponent",       # std FBX shininess (no roughness policy yet)
    "ReflectionFactor",        # std FBX metallic (no metallic policy yet)
    "map_Ka", "map_Ks", "map_Ns", "disp", "decal", "refl",  # MTL extras
)

IMAGE_EXTS = (".jpg", ".jpeg", ".png", ".tif", ".tiff", ".bmp", ".tga", ".exr")


def texture_files(root):
    """basename (lowercased) -> path of every image under `root`,
    recursively: a pack's extracted archives may wrap their maps in a
    folder, split them across several archives, or ship them loose. The
    converters and the audit both resolve through here, so they agree on
    the inventory. First path wins a basename collision (sorted walk)."""
    files = {}
    if not root or not os.path.isdir(root):
        return files
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if not d.startswith("."))
        for f in sorted(filenames):
            if f.startswith(".") or os.path.splitext(f)[1].lower() not in IMAGE_EXTS:
                continue
            files.setdefault(f.lower(), os.path.join(dirpath, f))
    return files


def shipped_inventory(fbx, tex_root):
    """Every texture the provider shipped, by basename: the image files
    under the extracted archives (see texture_files) plus the maps packed
    INSIDE the model file (the manifest's embedded_textures) — a packed
    diffuse is as shipped as a loose one, and a plan that cannot name it
    ships a gray hull (the military ship, 2026-09-06)."""
    loose = {os.path.basename(p) for p in texture_files(tex_root).values()}
    return loose | set(fbx.get("embedded_textures", []))


def normalize_max_table(raw):
    """The .max importer suffixes textured material names with the bitmap's
    authoring path ("ZJ-001106_E:\\CGtrader\\...") — normalize back to the
    material name; entries WITH channels win a key collision."""
    mats = raw.get("materials", raw)
    table = {}
    for key, entry in mats.items():
        name = re.sub(r"_[A-Za-z]:\\.*$", "", key)
        held = table.get(name)
        if held is None or (entry.get("channels") and not held.get("channels")):
            table[name] = entry
    return table


def clone_stem(basename):
    """The source-image name an authored reference collapses to: the
    stem, lower-cased, runs of dots/underscores/hyphens/spaces as one
    underscore, and SketchUp's clone mark dropped — a trailing number
    either wrapped in underscores ("ID_Fabric_Sofa_Grey_01_1000_") or
    glued to the stem and followed by one ("ID_Decor_Ceramic_bianco3650_"):
    each is the thousandth clone of one material, and the zip ships the
    one source image. A bare trailing number ("ID_Decor_10") is a
    different picture and stays. The audit groups unshipped references
    by this stem, so a hole reads "ceramic_bianco x5500", not 5500 lines."""
    stem = re.sub(r"[.\-_ ]+", "_", os.path.splitext(basename)[0].lower())
    return re.sub(r"_?\d+_$", "", stem)


def find_in_inventory(basename, inventory):
    """The shipped file matching an authored reference. Authors' machines
    disagree with the archive on case, sometimes on extension, and an
    exporter may sanitize the name on the way out (SketchUp writes
    "AD_W_Wall_Concrete.jpg" for the shipped "AD.W_Wall_Concrete.jpg"),
    so the match relaxes in steps: exact (case-insensitive), then stem,
    then the stem with SketchUp's clone mark dropped and its spelling
    kept, then the stem with dots/underscores/hyphens/spaces treated as
    one, then that with the clone mark dropped (`clone_stem`). A bare
    trailing number is a different picture and never collapses.

    A function of its inputs: the inventory is walked in name order, so
    two shipped files that loosen to one stem ("ID_Decor_13.jpg" and
    "ID.Decor_13.jpg", the hill house) resolve the same way in every
    process — the converter's and the audit's (run 42, 2026-09-09: they
    disagreed, and a material merged with its twin read as lost)."""
    files = sorted(inventory)
    lower = {}
    for f in files:
        lower.setdefault(f.lower(), f)
    hit = lower.get(basename.lower())
    if hit:
        return hit
    stems = {}
    for f in files:
        stems.setdefault(os.path.splitext(f)[0].lower(), f)
    stem = os.path.splitext(basename)[0].lower()
    hit = stems.get(stem)
    if hit:
        return hit
    spelled = re.sub(r"_?\d+_$", "", stem)  # the clone mark off, spelling kept
    if spelled != stem:
        hit = stems.get(spelled)
        if hit:
            return hit

    def loose(s):
        return re.sub(r"[.\-_ ]+", "_", s)
    loose_stems = {}
    for s, f in stems.items():
        loose_stems.setdefault(loose(s), f)
    hit = loose_stems.get(loose(stem))
    if hit:
        return hit
    unclone = clone_stem(basename)
    if unclone != loose(stem):
        return loose_stems.get(unclone)
    return None


def loose_matches(fbx, inventory):
    """Every declared texture reference of an assigned material that
    resolved only through the loosened (punctuation-insensitive) match —
    (material, channel, declared, shipped) — so a converter can log what
    the relaxation decided and a reviewer can check the pairs."""
    lower = {f.lower() for f in inventory}
    stems = {os.path.splitext(f)[0].lower() for f in inventory}
    found = []
    for name, rec in fbx["materials"].items():
        if not rec.get("assigned"):
            continue
        for chan, basename in rec["channels"].items():
            if basename.lower() in lower or os.path.splitext(basename)[0].lower() in stems:
                continue
            hit = find_in_inventory(basename, inventory)
            if hit:
                found.append((name, chan, basename, hit))
    return found


def _channel_file(rec, channel_set, inventory):
    for chan, basename in rec["channels"].items():
        if chan in channel_set:
            hit = find_in_inventory(basename, inventory)
            if hit:
                return hit, chan
    return None, None


def _camera_facing(materials, name):
    """Resolve 3ds Max container materials to what camera rays see:
    RaySwitchMtl renders its directMtl, LayeredMtl its base layer.
    Blender's FBX importer instantiates neither sub-material — importing
    the container as a blank default — so the plan re-derives the surface
    from the sub-material's record. Returns (resolved name, chain walked).
    """
    chain = []
    for _ in range(8):
        rec = materials[name]
        if rec["class"] == "RaySwitchMtl" and "directMtl" in rec["children"]:
            name = rec["children"]["directMtl"]
        elif rec["class"] == "LayeredMtl" and "baseMtl" in rec["children"]:
            name = rec["children"]["baseMtl"]
        else:
            break
        chain.append(name)
    return name, chain


def _plan_for(rec, tbl, inventory):
    props = rec["props"]
    plan = {}
    # Provenance prefix for what the MODEL FILE declared: the manifest
    # that read it (an MTL statement is not an FBX connection).
    decl = "mtl" if rec["class"] == "mtl" else "fbx"

    # ---- transparency stack: the exporter's TransparencyFactor is the
    # Corona refraction stack collapsed to one scalar (1 - TF = alpha);
    # levelRefract/levelOpacity identify glass, exactly as before.
    tf = props.get("TransparencyFactor", 0.0)
    refract = props.get("levelRefract", props.get("refractionAmount", 0.0))
    if "levelOpacity" in props or "opacityLevel" in props:
        eff_opacity = props.get("levelOpacity", props.get("opacityLevel"))
    else:
        eff_opacity = 1.0 - tf
    is_glass = refract > 0.5 or eff_opacity < 0.7
    is_light = rec["class"] == "LightMtl"

    # ---- base color: the .max table is the authoring truth where it
    # speaks; the model file's connection table where it is silent; the
    # authored Corona color for flat surfaces; the declared diffuse color
    # as the floor. "default" survives only when the source declares
    # NOTHING — the audit treats that as a hole.
    tbl_channels = (tbl or {}).get("channels", {})
    base = None
    tbl_base = tbl_channels.get("Base Color")
    if tbl_base:
        hit = find_in_inventory(tbl_base.replace("\\", "/").rsplit("/", 1)[-1],
                                inventory)
        if hit:
            base = {"texture": hit, "source": "max:Base Color"}
    if base is None:
        f, chan = _channel_file(rec, DIFFUSE_CHANNELS, inventory)
        if f:
            base = {"texture": f, "source": f"{decl}:{chan.rsplit('|', 1)[-1]}"}
    if base is None and is_light:
        emissive = props.get("EmissiveColor", [1.0, 1.0, 1.0])
        mag = max(emissive) or 1.0
        base = {"color": [c / mag for c in emissive], "source": "corona:LightMtl"}
    if base is None:
        for key, source in (("colorDiffuse", "corona:colorDiffuse"),
                            ("baseColor", "corona:baseColor")):
            if key in props:
                base = {"color": props[key], "source": source}
                break
    if base is None and (tbl or {}).get("color") is not None:
        base = {"color": tbl["color"], "source": "max:color"}
    if base is None and "DiffuseColor" in props:
        base = {"color": props["DiffuseColor"], "source": f"{decl}:DiffuseColor"}
    if base is None:
        base = {"color": [0.5, 0.5, 0.5], "source": "default"}
    # mapamountDiffuse < 1: the texture TINTS the authored color (a
    # cabinet authored 60% wood over near-black renders far too loud at
    # 100%). A fact about the material, whichever source named the file.
    if "texture" in base:
        amount = props.get("mapamountDiffuse", 1.0)
        if amount < 0.999 and "colorDiffuse" in props:
            base["blend"] = {"color": props["colorDiffuse"], "amount": amount}
    plan["base_color"] = base

    # ---- relief: a real normal map from the .max table wins; then the
    # model file's own tangent normal map (a Blender export's NormalMap);
    # otherwise a bump map ships as a height map the applier converts.
    normal = None
    tbl_normal = tbl_channels.get("Normal")
    if tbl_normal:
        hit = find_in_inventory(tbl_normal.replace("\\", "/").rsplit("/", 1)[-1],
                                inventory)
        if hit:
            normal = {"texture": hit, "kind": "normal", "source": "max:Normal"}
    if normal is None:
        f, chan = _channel_file(rec, NORMAL_CHANNELS, inventory)
        if f:
            normal = {"texture": f, "kind": "normal",
                      "source": f"{decl}:{chan.rsplit('|', 1)[-1]}"}
    if normal is None:
        f, chan = _channel_file(rec, BUMP_CHANNELS, inventory)
        if f:
            # The authored bump amount — often well under 1.0. Without it
            # the converted normal map shouts on every wall.
            authored = props.get("mapamountBump",
                                 props.get("baseBumpMapAmount", 1.0))
            normal = {"texture": f, "kind": "height",
                      "strength": max(0.05, min(1.5, authored)),
                      "source": f"{decl}:{chan.rsplit('|', 1)[-1]}"}
    plan["normal"] = normal

    # ---- roughness: direct maps wire as-is; Corona glossiness maps are
    # 1 - roughness and arrive inverted; glass falls back to the scalar.
    rough = None
    tbl_rough = tbl_channels.get("Roughness")
    if tbl_rough:
        hit = find_in_inventory(tbl_rough.replace("\\", "/").rsplit("/", 1)[-1],
                                inventory)
        if hit:
            # The .max importer files CoronaMtl's reflectGlossiness under
            # "Roughness" — same map, opposite convention. Invert whenever
            # the FBX shows the same file on a glossiness channel, or the
            # material class has no true roughness channel at all.
            gloss_twin, _ = _channel_file(rec, GLOSS_CHANNELS, inventory)
            invert = gloss_twin == hit or rec["class"] == "CoronaMtl"
            rough = {"texture": hit, "invert": invert, "source": "max:Roughness"}
    if rough is None:
        f, chan = _channel_file(rec, ROUGHNESS_CHANNELS, inventory)
        if f:
            rough = {"texture": f, "invert": False,
                     "source": f"{decl}:{chan.rsplit('|', 1)[-1]}"}
    if rough is None:
        f, chan = _channel_file(rec, GLOSS_CHANNELS, inventory)
        if f:
            rough = {"texture": f, "invert": True,
                     "source": f"{decl}:{chan.rsplit('|', 1)[-1]}"}
    if rough is None and is_glass and "refractGlossiness" in props:
        rough = {"value": 1.0 - props["refractGlossiness"],
                 "source": "corona:refractGlossiness"}
    # Corona blends the glossiness map toward the scalar base at the
    # slot's mapamount (0.15-0.2 on the walls); at 1.0 the scratch maps
    # render as mottled roughness chaos. Applies to any INVERTED (i.e.
    # glossiness-sourced) map, whichever table named the file.
    if rough is not None and rough.get("invert"):
        amount = props.get("mapamountReflectGlossiness", 1.0)
        if amount < 0.999:
            rough["blend"] = {
                "base": 1.0 - props.get("refractGlossiness", 1.0),
                "amount": amount,
            }
    plan["roughness"] = rough

    # ---- alpha: a declared cutout map (curtain lace) wins; scalar alpha
    # exists ONLY as the declared 1 - TransparencyFactor of glass — never
    # invented, never applied to opaque surfaces.
    alpha = None
    f, chan = _channel_file(rec, OPACITY_CHANNELS, inventory)
    if f:
        alpha = {"cutout_texture": f, "source": f"{decl}:{chan.rsplit('|', 1)[-1]}"}
    elif is_glass and tf > 0.0:
        alpha = {"value": 1.0 - tf, "source": f"{decl}:TransparencyFactor"}
    plan["alpha"] = alpha

    # ---- emission: self-illumination maps and LightMtl surfaces. Corona
    # treats LightMtl as a photometric SOURCE (sky cards at strength 1000);
    # Godot renders a surface, so strength is capped to a glow and the
    # authored value kept for provenance.
    emission = None
    f, chan = _channel_file(rec, SELFILLUM_CHANNELS, inventory)
    if f:
        emission = {"texture": f, "color": [1.0, 1.0, 1.0],
                    "authored_strength": 1.0,
                    "strength": min(1.0, EMISSION_CAP),
                    "source": f"{decl}:{chan.rsplit('|', 1)[-1]}"}
    else:
        emissive = props.get("EmissiveColor", [0.0, 0.0, 0.0])
        authored = max(emissive) * props.get("EmissiveFactor", 1.0)
        if authored <= 0.0 and is_light:
            # A LightMtl is emissive BY DEFINITION; when the FBX dropped
            # its color (four fixtures ship zeros), it still must glow.
            emissive, authored = [1.0, 1.0, 1.0], 1.0
        if authored > 0.0:
            mag = max(emissive)
            emission = {"texture": None,
                        "color": [c / mag for c in emissive],
                        "authored_strength": authored,
                        "strength": min(authored, EMISSION_CAP),
                        "source": "corona:LightMtl" if is_light else f"{decl}:EmissiveColor"}
    plan["emission"] = emission

    if is_light:
        plan["classification"] = "light"
    elif is_glass:
        plan["classification"] = "glass"
    elif "texture" in base:
        plan["classification"] = "textured"
    else:
        plan["classification"] = "flat"
    return plan


def build_plans(fbx, max_table, inventory):
    """name -> plan. Containers plan as their camera-facing sub-material
    (`resolved_from` records the chain); everything else plans as itself.
    """
    materials = fbx["materials"]
    plans = {}
    for name in materials:
        resolved, chain = _camera_facing(materials, name)
        tbl = (max_table or {}).get(resolved) or ((max_table or {}).get(name)
                                                  if not chain else None)
        plan = _plan_for(materials[resolved], tbl, inventory)
        plan["resolved_from"] = chain
        plans[name] = plan
    return plans


def explain_texture_usage(fbx, plans, inventory):
    """shipped filename -> fate. `used:<material>:<slot>` when a plan of an
    ASSIGNED material ships it; `waived:<channel>` when the source only
    references it through channels policy leaves behind; `unreferenced`
    when nothing in the source names it. Anything else is a recovery hole
    the audit fails on."""
    materials = fbx["materials"]
    fates = {}

    used = {}
    for name, plan in plans.items():
        if not materials.get(name, {}).get("assigned"):
            continue
        slots = {
            "base_color": (plan.get("base_color") or {}).get("texture"),
            "normal": (plan.get("normal") or {}).get("texture"),
            "roughness": (plan.get("roughness") or {}).get("texture"),
            "alpha": (plan.get("alpha") or {}).get("cutout_texture"),
            "emission": (plan.get("emission") or {}).get("texture"),
        }
        for slot, f in slots.items():
            if f:
                used.setdefault(f, f"used:{name}:{slot}")

    # Only materials that can SHOW: the assigned set plus what their
    # container chains resolve to. A texture referenced solely by a
    # left-behind sub-material (a LayeredMtl's non-base layer) is a
    # conscious waiver, not a hole.
    effective = set()
    for name, rec in materials.items():
        if rec["assigned"]:
            effective.add(_camera_facing(materials, name)[0])
            effective.add(name)

    referenced = {}  # shipped file -> set of (channel, is_effective)
    for name, rec in materials.items():
        for chan, basename in rec["channels"].items():
            hit = find_in_inventory(basename, inventory)
            if hit:
                referenced.setdefault(hit, set()).add(
                    (chan, name in effective))

    for f in inventory:
        if f in used:
            fates[f] = used[f]
        elif f in referenced:
            live = {c for c, eff in referenced[f]
                    if eff and not any(m in c for m in WAIVED_CHANNEL_MARKERS)}
            if live:
                fates[f] = ("referenced-but-unplanned:"
                            + sorted(c.rsplit("|", 1)[-1] for c in live)[0])
            else:
                any_chan = sorted(c.rsplit("|", 1)[-1] for c, _ in referenced[f])[0]
                fates[f] = f"waived:{any_chan}"
        else:
            fates[f] = "unreferenced"
    return fates
