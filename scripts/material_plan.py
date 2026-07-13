"""Material plan: the POLICY half of the apartment (fixed-environment)
conversion, pure Python — no Blender.

Input: the FBX material oracle (extract-fbx-materials.py), the .max
material table (extract-max-materials.py), and the shipped texture
inventory. Output: one plan per material name saying exactly what the
Godot-facing material must carry — base color, normal, roughness, alpha,
emission — with a PROVENANCE tag on every field, so "why does this
surface look wrong" is a lookup, not an investigation.

scripts/apartment.py applies the plans inside Blender (mechanism);
scripts/tests/test_asset_pipeline.py audits them (`make test-assets`).
"""

# Emission ceiling for provider-authored light materials (Corona LightMtl
# and self-illuminated surfaces). Corona treats these as photometric light
# SOURCES (the apartment ships sky cards at strength 1000); Godot renders
# them as surfaces, so anything past a gentle glow clips to a white shape.
# One look knob, owner-retunable.
EMISSION_CAP = 3.0

# Corona channel names, canonicalized. Anything not mapped here and not
# WAIVED is an unknown channel — the audit flags it, never drops it
# silently.
DIFFUSE_CHANNELS = {
    "DiffuseColor",
    "3dsMax|CoronaMtlPb|texmapDiffuse",
    "3dsMax|CoronaPhysicalMtlPb|baseTexmap",
}
BUMP_CHANNELS = {
    "3dsMax|CoronaMtlPb|texmapBump",
    "3dsMax|CoronaPhysicalMtlPb|baseBumpTexmap",
}
GLOSS_CHANNELS = {  # Corona glossiness = 1 - roughness: invert on wire
    "3dsMax|CoronaMtlPb|texmapReflectGlossiness",
}
ROUGHNESS_CHANNELS = {
    "3dsMax|CoronaPhysicalMtlPb|baseRoughnessTexmap",
}
OPACITY_CHANNELS = {
    "3dsMax|CoronaMtlPb|texmapOpacity",
    "3dsMax|LightMtlPb|opacityTexmap",
}
SELFILLUM_CHANNELS = {
    "3dsMax|CoronaMtlPb|texmapSelfIllum",
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
    "bump",                    # std FBX Bump/NormalMap duplicates
    "NormalMap",
    "ShininessExponent",
)


def normalize_max_table(raw):
    """The .max importer suffixes textured material names with the bitmap's
    authoring path ("ZJ-001106_E:\\CGtrader\\...") — normalize back to the
    material name; entries WITH channels win a key collision."""
    import re
    mats = raw.get("materials", raw)
    table = {}
    for key, entry in mats.items():
        name = re.sub(r"_[A-Za-z]:\\.*$", "", key)
        held = table.get(name)
        if held is None or (entry.get("channels") and not held.get("channels")):
            table[name] = entry
    return table


def find_in_inventory(basename, inventory):
    """The shipped file matching an authored reference (authors' machines
    disagree with the zip on case, and sometimes on extension)."""
    lower = {f.lower(): f for f in inventory}
    hit = lower.get(basename.lower())
    if hit:
        return hit
    import os
    stems = {os.path.splitext(f)[0].lower(): f for f in inventory}
    return stems.get(os.path.splitext(basename)[0].lower())


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
    # speaks; the FBX connection table where it is silent; the authored
    # Corona color for flat surfaces; the std diffuse as the floor.
    # "default" survives only when the source declares NOTHING — the
    # audit treats that as a hole.
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
            base = {"texture": f, "source": f"fbx:{chan.rsplit('|', 1)[-1]}"}
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
        base = {"color": props["DiffuseColor"], "source": "fbx:DiffuseColor"}
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

    # ---- relief: a real normal map from the .max table wins; otherwise
    # the Corona bump map ships as a height map the applier converts.
    normal = None
    tbl_normal = tbl_channels.get("Normal")
    if tbl_normal:
        hit = find_in_inventory(tbl_normal.replace("\\", "/").rsplit("/", 1)[-1],
                                inventory)
        if hit:
            normal = {"texture": hit, "kind": "normal", "source": "max:Normal"}
    if normal is None:
        f, chan = _channel_file(rec, BUMP_CHANNELS, inventory)
        if f:
            # The authored Corona bump amount — often well under 1.0.
            # Without it the converted normal map shouts on every wall.
            authored = props.get("mapamountBump",
                                 props.get("baseBumpMapAmount", 1.0))
            normal = {"texture": f, "kind": "height",
                      "strength": max(0.05, min(1.5, authored)),
                      "source": f"fbx:{chan.rsplit('|', 1)[-1]}"}
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
                     "source": f"fbx:{chan.rsplit('|', 1)[-1]}"}
    if rough is None:
        f, chan = _channel_file(rec, GLOSS_CHANNELS, inventory)
        if f:
            rough = {"texture": f, "invert": True,
                     "source": f"fbx:{chan.rsplit('|', 1)[-1]}"}
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
        alpha = {"cutout_texture": f, "source": f"fbx:{chan.rsplit('|', 1)[-1]}"}
    elif is_glass and tf > 0.0:
        alpha = {"value": 1.0 - tf, "source": "fbx:TransparencyFactor"}
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
                    "source": f"fbx:{chan.rsplit('|', 1)[-1]}"}
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
                        "source": "corona:LightMtl" if is_light else "fbx:EmissiveColor"}
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
