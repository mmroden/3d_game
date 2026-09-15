#!/usr/bin/env bash
set -euo pipefail

# Install Godot addons from downloaded asset packs into godot/addons/.
# Source packs live in assets/ (gitignored). This script copies only the
# files Godot needs — no Unity/Unreal projects, no duplicate root .bin
# files, no stale .import descriptors.

ASSETS_DIR="${1:?Usage: install-addons.sh <assets-dir> <godot-dir>}"
GODOT_DIR="${2:?Usage: install-addons.sh <assets-dir> <godot-dir>}"
MODE="${3:-}"
TRES_ONLY="$MODE"    # --tres-only: re-copy just .tres files (fix Godot path rewrites)
# --metrics-only (make metrics): no Blender, nothing converted — every
# stale metrics re-reads its installed product against the current
# rosters (the zone-authoring loop's stage). The cheap copy stanzas still
# run; they are idempotent.
METRICS_ONLY=""
[ "$MODE" = "--metrics-only" ] && METRICS_ONLY=1
ADDON_DIR="$GODOT_DIR/addons/quaternius"
SCRIPTS_DIR="$(cd "$(dirname "$0")" && pwd)"
OUT_DIR="$(cd "$SCRIPTS_DIR/.." && pwd)/out"

# ---------- Freshness ----------
# Door 1 is idempotent (re-runs converge) and, since 2026-09-06, also
# incremental: every Blender-heavy stanza asks `stale` first and runs only
# when an output is missing or older than one of its inputs. Inputs are
# the provider files AND the scripts the stanza invokes, so editing a
# converter rebuilds exactly what it converts; editing THIS script does
# not (FORCE=1 rebuilds everything). Cheap copy stanzas stay
# unconditional. Owner 2026-09-06: adding one hull meant sitting through
# the apartment and the panel splits — and the new scenes will be run
# many times while the audit names what each one lacks.
stale() {  # <output>... -- <input>...   (returns 0 = work needed)
    [ -n "${FORCE:-}" ] && return 0
    local outputs=() f in out
    while [ $# -gt 0 ] && [ "$1" != "--" ]; do outputs+=("$1"); shift; done
    [ $# -gt 0 ] && shift
    [ ${#outputs[@]} -gt 0 ] || return 0
    for f in "${outputs[@]}"; do [ -e "$f" ] || return 0; done
    for in in "$@"; do
        [ -n "$in" ] && [ -e "$in" ] || continue
        for out in "${outputs[@]}"; do
            [ "$in" -nt "$out" ] && return 0
        done
    done
    return 1
}

# ---------- Quaternius Modular Sci-Fi MegaKit ----------

MEGAKIT_SRC="$ASSETS_DIR/quaternius-megakit/Engine Projects/Godot/modular-sci-fi-megakit/addons/quaternius"

if [ ! -d "$MEGAKIT_SRC" ]; then
    echo "ERROR: MegaKit Godot addon not found at:"
    echo "  $MEGAKIT_SRC"
    echo "Run 'make deps' first to download asset packs."
    exit 1
fi

# Re-copy only .tres materials (Godot rewrites texture paths during import)
if [ "$TRES_ONLY" = "--tres-only" ]; then
    echo "  Re-copying .tres materials from source (fixing Godot path rewrites)..."
    find "$MEGAKIT_SRC/materials" -maxdepth 1 -name "*.tres" -exec cp {} "$ADDON_DIR/materials/" \;
    ESSENTIALS_SRC="$ASSETS_DIR/quaternius-essentials/Engine Projects/Godot/sci-fi-essentials/addons/quaternius"
    if [ -d "$ESSENTIALS_SRC/materials" ]; then
        find "$ESSENTIALS_SRC/materials" -maxdepth 1 -name "*.tres" -exec cp {} "$ADDON_DIR/materials/" \;
    fi
    chmod -R u+w "$ADDON_DIR/materials"
    # Strip Textures/ subdirectory prefix, stale UIDs, and deprecated
    # editor-only VisualShader state (graph_offset triggers a deprecation
    # error on load, which GUT treats as a test failure)
    find "$ADDON_DIR/materials" -maxdepth 1 -name "*.tres" \
        -exec sed -i '' 's|materials/Textures/|materials/|g' {} + \
        -exec sed -i '' 's| uid="uid://[^"]*"||g' {} + \
        -exec sed -i '' '/^graph_offset = /d' {} +
    # Convert embedded CompressedTexture2D (with S3TC load_paths) to ext_resource
    # references pointing to the actual .png files (works on any platform)
    python3 "$(dirname "$0")/fix-embedded-textures.py" "$ADDON_DIR/materials"
    echo "  Materials restored."
    exit 0
fi

echo "  Installing MegaKit addon..."
mkdir -p "$ADDON_DIR/modularscifimegakit"

# Import script (material assignment magic)
cp "$MEGAKIT_SRC/quaternius_import_script.gd" "$ADDON_DIR/"

# Materials — textures, shaders, .tres (skip stale .import files)
mkdir -p "$ADDON_DIR/materials"
find "$MEGAKIT_SRC/materials" -maxdepth 1 \( -name "*.tres" -o -name "*.png" -o -name "*.gdshader" -o -name "*.bin" \) \
    -exec cp {} "$ADDON_DIR/materials/" \;

# Remove any stale Textures symlink (causes infinite reimport loops)
rm -f "$ADDON_DIR/materials/Textures"

# Strip stale UIDs from .tres files (they reference the asset pack author's
# project) and deprecated editor-only VisualShader state (graph_offset
# triggers a deprecation error on load, which GUT treats as a test failure)
find "$ADDON_DIR/materials" -maxdepth 1 -name "*.tres" \
    -exec sed -i '' 's| uid="uid://[^"]*"||g' {} + \
    -exec sed -i '' '/^graph_offset = /d' {} +

# Strip stale .s3tc.ctex load_path entries (macOS/Metal doesn't generate these;
# they reference the asset pack author's S3TC-compiled textures)
find "$ADDON_DIR/materials" -maxdepth 1 -name "*.tres" \
    -exec sed -i '' '/load_path.*\.s3tc\.ctex/d' {} +

# Mesh modules — only subdirectories with .gltf + .bin (skip root dupes and .import files)
for subdir in walls platforms props columns aliens decals; do
    src="$MEGAKIT_SRC/modularscifimegakit/$subdir"
    if [ -d "$src" ]; then
        mkdir -p "$ADDON_DIR/modularscifimegakit/$subdir"
        find "$src" -maxdepth 1 \( -name "*.gltf" -o -name "*.bin" \) \
            -exec cp {} "$ADDON_DIR/modularscifimegakit/$subdir/" \;
    fi
done

# Symlink shared textures into each mesh subdirectory so bare-filename URIs
# in the .gltf files (e.g. "T_Trim_01_Normal.png") resolve correctly.
for subdir in walls platforms props columns aliens decals; do
    target="$ADDON_DIR/modularscifimegakit/$subdir"
    [ -d "$target" ] || continue
    for tex in "$ADDON_DIR/materials"/*.png; do
        [ -f "$tex" ] || continue
        base="$(basename "$tex")"
        [ -e "$target/$base" ] || ln -s "../../materials/$base" "$target/$base"
    done
done

# Make everything writable (source packs may be read-only)
chmod -R u+w "$ADDON_DIR"

echo "  MegaKit addon installed."

# ---------- Quaternius Sci-Fi Essentials ----------

ESSENTIALS_GLTF="$ASSETS_DIR/quaternius-essentials/glTF"
ESSENTIALS_TEX="$ASSETS_DIR/quaternius-essentials/Textures"

if [ -d "$ESSENTIALS_GLTF" ]; then
    echo "  Installing Essentials addon..."

    # Essentials textures (merge into materials dir for shared material references)
    if [ -d "$ESSENTIALS_TEX" ]; then
        find "$ESSENTIALS_TEX" -maxdepth 1 \( -name "*.png" -o -name "*.jpg" \) \
            -exec cp {} "$ADDON_DIR/materials/" \;

        if [ -d "$ESSENTIALS_TEX/Planet Textures" ]; then
            mkdir -p "$ADDON_DIR/materials/Planet Textures"
            find "$ESSENTIALS_TEX/Planet Textures" -maxdepth 1 \( -name "*.png" -o -name "*.jpg" \) \
                -exec cp {} "$ADDON_DIR/materials/Planet Textures/" \;
        fi
    fi

    # Essentials meshes — categorize by filename prefix from flat glTF/ directory
    for category in props enemies guns; do
        mkdir -p "$ADDON_DIR/essentials/$category"
    done
    for f in "$ESSENTIALS_GLTF"/*.gltf "$ESSENTIALS_GLTF"/*.bin "$ESSENTIALS_GLTF"/*.png "$ESSENTIALS_GLTF"/*.jpg; do
        [ -f "$f" ] || continue
        base="$(basename "$f")"
        case "$base" in
            Prop_*|T_Props_*|T_Screens*|T_Table*|T_Rings*|T_Trim_*) cp "$f" "$ADDON_DIR/essentials/props/" ;;
            Enemy_*|T_Enemies_*)                                      cp "$f" "$ADDON_DIR/essentials/enemies/" ;;
            Gun_*|T_Guns_*)                                           cp "$f" "$ADDON_DIR/essentials/guns/" ;;
        esac
    done

    # Symlink shared textures into each essentials subdirectory so bare-filename
    # URIs in the .gltf files (e.g. "T_Trim_03_Normal.png") resolve correctly.
    for category in props enemies guns; do
        target="$ADDON_DIR/essentials/$category"
        [ -d "$target" ] || continue
        for tex in "$ADDON_DIR/materials"/*.png "$ADDON_DIR/materials"/*.jpg; do
            [ -f "$tex" ] || continue
            base="$(basename "$tex")"
            [ -e "$target/$base" ] || ln -s "../../materials/$base" "$target/$base"
        done
    done

    chmod -R u+w "$ADDON_DIR"
    echo "  Essentials addon installed."
else
    echo "  Essentials glTF not found, skipping."
fi

# ---------- Quaternius Monsters (FBX-only pack) ----------

MONSTERS_SRC="$ASSETS_DIR/quaternius-monsters/FBX"

if [ -d "$MONSTERS_SRC" ]; then
    echo "  Installing Monsters addon..."
    mkdir -p "$ADDON_DIR/monsters"
    find "$MONSTERS_SRC" -maxdepth 1 -name "*.fbx" \
        -exec cp {} "$ADDON_DIR/monsters/" \;
    chmod -R u+w "$ADDON_DIR/monsters"
    echo "  Monsters addon installed."
else
    echo "  Monsters FBX not found, skipping."
fi

# ---------- Quaternius Fish (FBX-only pack) ----------

FISH_SRC="$ASSETS_DIR/quaternius-fish/FBX"

if [ -d "$FISH_SRC" ]; then
    echo "  Installing Fish addon..."
    mkdir -p "$ADDON_DIR/fish"
    find "$FISH_SRC" -maxdepth 1 -name "*.fbx" \
        -exec cp {} "$ADDON_DIR/fish/" \;
    chmod -R u+w "$ADDON_DIR/fish"
    echo "  Fish addon installed."
else
    echo "  Fish FBX not found, skipping."
fi

# ---------- Quaternius Spaceships (FBX-only pack, reserved for player ship upgrades) ----------

SPACESHIPS_SRC="$ASSETS_DIR/quaternius-spaceships/FBX"

if [ -d "$SPACESHIPS_SRC" ]; then
    echo "  Installing Spaceships addon..."
    mkdir -p "$ADDON_DIR/spaceships"
    find "$SPACESHIPS_SRC" -maxdepth 1 -name "*.fbx" \
        -exec cp {} "$ADDON_DIR/spaceships/" \;
    chmod -R u+w "$ADDON_DIR/spaceships"
    echo "  Spaceships addon installed."
else
    echo "  Spaceships FBX not found, skipping."
fi

echo "  Quaternius addon ready at: $ADDON_DIR"
du -sh "$ADDON_DIR"

# ========== Player ship models (CGTrader, royalty-free, not redistributable) ==========
# Source .glb files under assets/cgtrader_ships/ are deliberately git-tracked:
# this is a private, non-open-source repo, so the convenience of a self-contained
# checkout outweighs keeping third-party binaries out of history.

SHIPS_SRC="$ASSETS_DIR/cgtrader_ships"
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"

# Provider archives extract once into unpacked/ beside their sources (the
# raw .rar/.zip stay tracked and untouched; unpacked/ is gitignored).
extract_archive() {  # <archive> <dest-dir> — idempotent (dest exists = done)
    local archive="$1" dest="$2"
    [ -d "$dest" ] && return 0
    mkdir -p "$dest"
    # bsdtar first: the macOS system one reads these .rar files; 7z builds
    # often lack the rar codec. A failed extraction removes dest so the next
    # run retries instead of trusting a half-extracted directory.
    if ! bsdtar -xf "$archive" -C "$dest" 2>/dev/null \
        && ! 7z x -y -o"$dest" "$archive" >/dev/null 2>&1; then
        rm -rf "$dest"
        echo "  ERROR: could not extract $archive (bsdtar and 7z both failed)"
        return 1
    fi
}

# The textures archives wrap their maps in a folder (or don't) — resolve to
# whichever directory actually holds the images. (The material-plan
# conversions resolve the same way in Python: material_plan.texture_dir.)
tex_root() {  # <unpacked-dir>
    local dir
    dir="$(find "$1" -mindepth 1 -maxdepth 1 -type d | head -1)"
    if [ -n "$dir" ]; then echo "$dir"; else echo "$1"; fi
}

# Cockpit shell: a hull's interior furniture (consoles, seat, canopy
# bows) extracted per the cockpit_plan.py rule with the pilot Eyepoint
# baked in — the first-person stereo view renders it at full detail
# around the camera. One stage for every hull; audit: `make test-assets`.
extract_cockpit() {  # <hull.glb> <shell.glb>
    local metrics="$OUT_DIR/metrics/$(basename "$2" .glb).toml"
    if [ -z "$METRICS_ONLY" ]; then
        if [ ! -x "$BLENDER" ]; then
            echo "  WARNING: Blender not found at $BLENDER — run 'make deps'. Skipping cockpit shell $(basename "$2")."
            return 0
        fi
        if stale "$2" -- "$1" "$SCRIPTS_DIR/extract-cockpit.py" "$SCRIPTS_DIR/cockpit_plan.py"; then
            mkdir -p "$OUT_DIR"
            "$BLENDER" --background --python-exit-code 1 --python "$SCRIPTS_DIR/extract-cockpit.py" -- \
                "$1" "$2" > "$OUT_DIR/cockpit-$(basename "$2" .glb).log" 2>&1 \
                || echo "  ERROR: $(basename "$2") extraction failed (see out/cockpit-$(basename "$2" .glb).log)"
            grep -i "extract-cockpit:" "$OUT_DIR/cockpit-$(basename "$2" .glb).log" \
                || echo "  ($(basename "$2"): no extraction summary — see out/cockpit-$(basename "$2" .glb).log)"
        else
            echo "  $(basename "$2") is fresh."
        fi
    fi
    # The metrics of what got built (out/metrics/<shell>.toml): the
    # reproducible reading of the hull, the plan, and the shell — read
    # that, never an ad-hoc parse of the .glb.
    if [ -f "$2" ] && stale "$metrics" -- "$1" "$2" "$SCRIPTS_DIR/hull-metrics.py" "$SCRIPTS_DIR/cockpit_plan.py"; then
        python3 "$SCRIPTS_DIR/hull-metrics.py" "$1" "$2" "$metrics" \
            | grep -i "hull-metrics:" || echo "  ($(basename "$2"): no metrics — check hull-metrics.py output)"
    fi
}

# decimate_model <label> <src> <out.glb> <target_tris> <tex_dir> [base_color_file]
# The one stage for every decimated model (enemy mechs, spheres, drones,
# the jump gate): freshness-gated on the source, its texture folder, and
# decimate.py itself.
# An image's .import sidecar without the image is a stale extraction
# (the old sweep deleted Godot's extracted textures and left their
# sidecars): Godot reads the sidecar as "already extracted", never
# rewrites the file, and falls back to the embedded map uncompressed —
# 8 load errors per enemy model on every import since 2026-09-06, and
# 4096-square maps in VRAM uncompressed. A sidecar with no source is
# nobody's product; it goes, and the next import extracts once.
sweep_stale_sidecars() {
    local sidecar
    for sidecar in "$1"/*.png.import "$1"/*.jpg.import "$1"/*.jpeg.import; do
        [ -f "$sidecar" ] || continue
        [ -f "${sidecar%.import}" ] || rm -f "$sidecar"
    done
}

decimate_model() {
    local label="$1" src="$2" out="$3" target="$4" tex="$5" base="${6:-}"
    local metrics="$OUT_DIR/metrics/$(basename "$out" .glb).toml"
    if [ -z "$METRICS_ONLY" ]; then
        if stale "$out" -- "$src" "$tex" "$SCRIPTS_DIR/decimate.py"; then
            # A rebuilt glb embeds new maps: the textures Godot extracted
            # from the old one (<model>_<image>.png/.jpg and their .import
            # sidecars) would shadow them, since the importer extracts
            # only what is not already there.
            rm -f "${out%.glb}"_*.png "${out%.glb}"_*.jpg "${out%.glb}"_*.png.import "${out%.glb}"_*.jpg.import 2>/dev/null
            # Full Blender output to out/decimate-<model>.log: the stage's
            # histogram reads it (a summary grep hid every warning, 2026-09-07).
            mkdir -p "$OUT_DIR"
            "$BLENDER" --background --python-exit-code 1 --python "$SCRIPTS_DIR/decimate.py" -- \
                "$src" "$out" "$target" "$tex" ${base:+"$base"} \
                > "$OUT_DIR/decimate-$(basename "$out" .glb).log" 2>&1 \
                || echo "  ERROR: $label decimation failed (see out/decimate-$(basename "$out" .glb).log)"
            grep -i "decimate:" "$OUT_DIR/decimate-$(basename "$out" .glb).log" \
                || echo "  ($label: no decimation summary — see out/decimate-$(basename "$out" .glb).log)"
        else
            echo "  $label is fresh."
        fi
    fi
    # The metrics of the decimated product (out/metrics/<model>.toml):
    # parts, extents, materials, surviving ANIMATIONS — the apartment
    # boss shipped five provider clips whose scale channels overrode the
    # roster's size fit (2026-09-06); read this, never `strings` on a .glb.
    if [ -f "$out" ] && stale "$metrics" -- "$out" "$SCRIPTS_DIR/scene-metrics.py" "$SCRIPTS_DIR/cockpit_plan.py"; then
        mkdir -p "$OUT_DIR/metrics"
        python3 "$SCRIPTS_DIR/scene-metrics.py" "$(basename "$out" .glb)" "$out" "$metrics" \
            | grep -i "scene-metrics:" || echo "  ($label: no metrics — check scene-metrics.py output)"
    fi
}

if [ -d "$SHIPS_SRC" ]; then
    echo "  Installing player ship models..."
    SHIPS_DIR="$GODOT_DIR/addons/ships"
    mkdir -p "$SHIPS_DIR"
    # Self-contained .glb (mesh + embedded PBR textures) — Godot imports natively.
    find "$SHIPS_SRC" -maxdepth 1 -name "*.glb" -exec cp {} "$SHIPS_DIR/" \;

    # Spaceship_1 is the player ship, packaged with three swappable color styles
    # (Style_1/2/3). The .glb bakes in Style_1; we install the external PBR maps
    # for all three styles alongside so the loadout variants can re-skin the hull
    # at runtime. Style N, part P → TX_spacecraft_1_<P + (N-1)*5>_<Map>.png.
    SPACESHIP1_SRC="$SHIPS_SRC/Spaceship_1"
    if [ -d "$SPACESHIP1_SRC" ]; then
        cp "$SPACESHIP1_SRC/Spacecraft_1.glb" "$SHIPS_DIR/"
        STYLES_DST="$SHIPS_DIR/Spacecraft_1_styles"
        rm -rf "$STYLES_DST"
        mkdir -p "$STYLES_DST"
        # Flatten Texture_Base/Style_N → Spacecraft_1_styles/Style_N.
        cp -R "$SPACESHIP1_SRC/Texture_Base/"Style_* "$STYLES_DST/"
        echo "  Spaceship_1 color styles installed ($(ls -d "$STYLES_DST"/Style_* 2>/dev/null | wc -l | tr -d ' ') styles)."
        extract_cockpit "$SPACESHIP1_SRC/Spacecraft_1.glb" "$SHIPS_DIR/vanguard_cockpit.glb"
    fi
    # Military ship (provider FBX + a .rar of loose maps; the diffuse and
    # cockpit textures are EMBEDDED in the FBX): converted through the
    # same material-plan stages as the apartment — manifest
    # (extract-fbx-materials.py) -> plan (material_plan.py) -> apply
    # (convert-hull.py) — into a roster-frame hull, then its furnished
    # interior extracted as a second cockpit shell. Facing: the provider
    # modeled the nose along -Z (front landing gear forward; probed
    # 2026-09-06), the roster frame is nose +Z — a half turn.
    MILITARY_SRC="$SHIPS_SRC/military_ship"
    if [ -d "$MILITARY_SRC" ]; then
        if [ -z "$METRICS_ONLY" ] && [ ! -x "$BLENDER" ]; then
            echo "  WARNING: Blender not found at $BLENDER — run 'make deps'. Skipping military ship."
        else
            fbx="$(find "$MILITARY_SRC" -maxdepth 1 -iname "MilitaryShip_StockFlight*.fbx" | head -1)"
            tex_rar="$(find "$MILITARY_SRC" -maxdepth 1 -iname "Textures*.rar" | head -1)"
            if [ -n "$fbx" ]; then
                [ -z "$tex_rar" ] || [ -n "$METRICS_ONLY" ] || extract_archive "$tex_rar" "$MILITARY_SRC/unpacked/textures"
                if [ -n "$METRICS_ONLY" ]; then
                    :
                elif stale "$SHIPS_DIR/military_ship.glb" "$MILITARY_SRC/fbx_materials.json" -- \
                        "$fbx" "$tex_rar" "$SCRIPTS_DIR/extract-fbx-materials.py" \
                        "$SCRIPTS_DIR/convert-hull.py" "$SCRIPTS_DIR/plan_apply.py" \
                        "$SCRIPTS_DIR/material_plan.py"; then
                    mkdir -p "$OUT_DIR"
                    "$BLENDER" --background --python-exit-code 1 --python "$SCRIPTS_DIR/extract-fbx-materials.py" -- \
                        "$fbx" "$MILITARY_SRC/fbx_materials.json" > "$OUT_DIR/manifest-military_ship.log" 2>&1 \
                        || echo "  ERROR: military ship manifest extraction failed (see out/manifest-military_ship.log)"
                    grep -i "extract-fbx-materials:" "$OUT_DIR/manifest-military_ship.log" \
                        || echo "  (military ship: no manifest summary — see out/manifest-military_ship.log)"
                    "$BLENDER" --background --python-exit-code 1 --python "$SCRIPTS_DIR/convert-hull.py" -- \
                        "$fbx" "$SHIPS_DIR/military_ship.glb" "$MILITARY_SRC/unpacked/textures" \
                        "$MILITARY_SRC/fbx_materials.json" 180 > "$OUT_DIR/hull-military_ship.log" 2>&1 \
                        || echo "  ERROR: military ship conversion failed (see out/hull-military_ship.log)"
                    grep -i "convert-hull:" "$OUT_DIR/hull-military_ship.log" \
                        || echo "  (military ship: no conversion summary — see out/hull-military_ship.log)"
                else
                    echo "  military_ship.glb is fresh."
                fi
                extract_cockpit "$SHIPS_DIR/military_ship.glb" "$SHIPS_DIR/military_ship_cockpit.glb"
            fi
        fi
    fi
    # Purchasable hull roster (ship_upgrades/): installed under stable names —
    # the ShipType spec table (void-logic/src/ship_type.rs) is the single
    # source of truth for these res:// paths. The old free-floating copies
    # under their upload-artifact names are removed.
    UPGRADES_SRC="$SHIPS_SRC/ship_upgrades"
    if [ -d "$UPGRADES_SRC" ]; then
        rm -f "$SHIPS_DIR/basic_ship.glb" "$SHIPS_DIR/basic_ship.glb.import" \
              "$SHIPS_DIR/uploads_files_2828578_Dronbizimisimiz.glb" \
              "$SHIPS_DIR/uploads_files_2828578_Dronbizimisimiz.glb.import" \
              "$SHIPS_DIR/uploads_files_4774125_Alien_spacecraft_04_FBX_.glb" \
              "$SHIPS_DIR/uploads_files_4774125_Alien_spacecraft_04_FBX_.glb.import"
        cp "$UPGRADES_SRC/basic_ship.glb" "$SHIPS_DIR/talon.glb"
        cp "$UPGRADES_SRC/Dronbizimisimiz.glb" "$SHIPS_DIR/hive.glb"
        cp "$UPGRADES_SRC/Alien_spacecraft_04_FBX_.glb" "$SHIPS_DIR/reaver.glb"
        echo "  Hull roster installed (talon, hive, reaver)."
    fi
    chmod -R u+w "$SHIPS_DIR"
    echo "  Player ship models installed ($(ls "$SHIPS_DIR"/*.glb 2>/dev/null | wc -l | tr -d ' ') models)."
else
    echo "  cgtrader_ships not found, skipping player ships."
fi

# ========== Enemy models (CGTrader evil-mechs: decimated FBX -> glB) ==========
# The raw mechs are 21-26k tris each — far too heavy to render 29 of them twice
# (SBS) and to build convex hulls from. Blender (installed by `make deps`)
# collapses each to a game-weight budget and writes a self-contained .glb.

EVIL_MECHS_SRC="$SHIPS_SRC/evil_mechs"
DECIMATE_TARGET=2000

if [ -d "$EVIL_MECHS_SRC" ]; then
    ENEMIES_DIR="$GODOT_DIR/addons/enemies"
    mkdir -p "$ENEMIES_DIR"
    if [ ! -x "$BLENDER" ]; then
        echo "  WARNING: Blender not found at $BLENDER — run 'make deps'. Skipping enemy mechs."
    else
        echo "  Decimating enemy mech models (target ${DECIMATE_TARGET} tris)..."
        # Drop any stale raw-FBX install from before decimation existed.
        # The loose .png/.jpg beside the glbs are NOT ours to sweep: they
        # are the textures Godot's importer extracts from each glb (its
        # embedded_image_handling default) and imports VRAM-compressed;
        # sweeping them every run forced a re-extraction on every import
        # pass 1, with 8 "Failed loading resource" errors per model
        # (2026-09-07). decimate_model drops a model's own extractions
        # when it rebuilds that model.
        rm -f "$ENEMIES_DIR"/*.fbx 2>/dev/null
        sweep_stale_sidecars "$ENEMIES_DIR"
        for n in 01 03; do
            src="$(find "$EVIL_MECHS_SRC" -maxdepth 1 -iname "*Evil_mech_${n}*.fbx" | head -1)"
            [ -n "$src" ] || continue
            # The mech's PBR maps ship loose in a sibling "(Textures)" folder the
            # FBX doesn't reference; pass it so decimate.py rebuilds the material.
            tex="$(find "$EVIL_MECHS_SRC" -maxdepth 1 -type d -iname "*Evil_mech_${n}*Textures*" | head -1)"
            decimate_model "mech ${n}" "$src" "$ENEMIES_DIR/evil_mech_${n}.glb" "$DECIMATE_TARGET" "$tex"
        done
        chmod -R u+w "$ENEMIES_DIR"
        echo "  Enemy mech models installed ($(ls "$ENEMIES_DIR"/*.glb 2>/dev/null | wc -l | tr -d ' ') mechs)."
    fi
else
    echo "  evil_mechs not found, skipping enemy mechs."
fi

# ========== Enemy models (CGTrader spheres + alien troop: decimated FBX -> glB) ==========
# The sphere ships are the basic enemy roster (the mechs graduated to bosses);
# same FBX + loose "(Textures)" folder shape as the mechs, same Blender pass.

SPHERES_SRC="$SHIPS_SRC/spheres"

if [ -d "$SPHERES_SRC" ]; then
    ENEMIES_DIR="$GODOT_DIR/addons/enemies"
    mkdir -p "$ENEMIES_DIR"
    if [ ! -x "$BLENDER" ]; then
        echo "  WARNING: Blender not found at $BLENDER — run 'make deps'. Skipping sphere enemies."
    else
        echo "  Decimating sphere enemy models (target ${DECIMATE_TARGET} tris)..."
        for n in 01 02 03; do
            src="$(find "$SPHERES_SRC" -maxdepth 1 -iname "*Sphere_ship_${n}*.fbx" | head -1)"
            [ -n "$src" ] || continue
            tex="$(find "$SPHERES_SRC" -maxdepth 1 -type d -iname "*Sphere_ship_${n}*Textures*" | head -1)"
            decimate_model "sphere ${n}" "$src" "$ENEMIES_DIR/sphere_ship_${n}.glb" "$DECIMATE_TARGET" "$tex"
        done
        src="$(find "$SPHERES_SRC" -maxdepth 1 -iname "*Alien_troop_01*.fbx" | head -1)"
        if [ -n "$src" ]; then
            tex="$(find "$SPHERES_SRC" -maxdepth 1 -type d -iname "*Alien_troop_01*Textures*" | head -1)"
            decimate_model "alien troop" "$src" "$ENEMIES_DIR/alien_troop_01.glb" "$DECIMATE_TARGET" "$tex"
        fi
        chmod -R u+w "$ENEMIES_DIR"
        echo "  Sphere enemy models installed."
    fi
else
    echo "  spheres not found, skipping sphere enemies."
fi

# ========== Enemy models (CGTrader grey spheres: planet-3 drone roster) ==========
# Five provider drops in mixed shapes: three OBJ+MTL sphere drones with their
# PBR maps in .rar archives, the apartment boss as loose FBX + .rar textures
# (two base coats — Rusted is the game's read), and UMSFD_02 as a ready .glb
# that still rides the decimation pass (the enemy tri budget is a contract,
# probe-enforced). Archives extract once into unpacked/ beside the sources;
# raw provider files stay untouched.

GREY_SPHERES_SRC="$SHIPS_SRC/grey_spheres"

if [ -d "$GREY_SPHERES_SRC" ]; then
    ENEMIES_DIR="$GODOT_DIR/addons/enemies"
    mkdir -p "$ENEMIES_DIR"
    if [ ! -x "$BLENDER" ]; then
        echo "  WARNING: Blender not found at $BLENDER — run 'make deps'. Skipping grey spheres."
    else
        UNPACKED="$GREY_SPHERES_SRC/unpacked"
        echo "  Decimating grey-sphere drone models (target ${DECIMATE_TARGET} tris)..."
        for n in 01 02 03; do
            obj_rar="$(find "$GREY_SPHERES_SRC" -maxdepth 1 -iname "*Sphere_drone_${n}_OBJ.rar" | head -1)"
            tex_rar="$(find "$GREY_SPHERES_SRC" -maxdepth 1 -iname "*Sphere_drone_${n}_Textures.rar" | head -1)"
            [ -n "$obj_rar" ] || continue
            extract_archive "$obj_rar" "$UNPACKED/sphere_drone_${n}_obj"
            [ -z "$tex_rar" ] || extract_archive "$tex_rar" "$UNPACKED/sphere_drone_${n}_tex"
            src="$(find "$UNPACKED/sphere_drone_${n}_obj" -iname "*.obj" | head -1)"
            [ -n "$src" ] || continue
            decimate_model "sphere drone ${n}" "$src" "$ENEMIES_DIR/sphere_drone_${n}.glb" \
                "$DECIMATE_TARGET" "$(tex_root "$UNPACKED/sphere_drone_${n}_tex")"
        done

        src="$(find "$GREY_SPHERES_SRC" -maxdepth 1 -iname "*apartment_boss*.fbx" | head -1)"
        if [ -n "$src" ]; then
            tex_rar="$(find "$GREY_SPHERES_SRC" -maxdepth 1 -iname "*apartment_boss*Textures*.rar" | head -1)"
            [ -z "$tex_rar" ] || extract_archive "$tex_rar" "$UNPACKED/apartment_boss_tex"
            decimate_model "apartment boss" "$src" "$ENEMIES_DIR/apartment_boss.glb" \
                "$DECIMATE_TARGET" "$(tex_root "$UNPACKED/apartment_boss_tex")" "Base Rusted.png"
        fi

        src="$(find "$GREY_SPHERES_SRC" -maxdepth 1 -iname "*UMSFD_02*.glb" | head -1)"
        if [ -n "$src" ]; then
            tex_rar="$(find "$GREY_SPHERES_SRC" -maxdepth 1 -iname "*UMSFD_02*Textures*.rar" | head -1)"
            [ -z "$tex_rar" ] || extract_archive "$tex_rar" "$UNPACKED/umsfd_02_tex"
            decimate_model "umsfd_02" "$src" "$ENEMIES_DIR/umsfd_02.glb" \
                "$DECIMATE_TARGET" "$(tex_root "$UNPACKED/umsfd_02_tex")"
        fi
        chmod -R u+w "$ENEMIES_DIR"
        echo "  Grey-sphere drone models installed."
    fi
else
    echo "  grey_spheres not found, skipping planet-3 drones."
fi

# ========== Planet-3 fixed environments (archviz scenes -> one glB each) ==========
# One stage for every fixed-level scene. A pack is a folder under assets/
# holding what the provider shipped — the model (FBX, or OBJ + MTL), the
# texture archives (.rar/.zip), a .max source when there is one — and
# nothing else authored. The stanza extracts the archives beside the
# sources (unpacked/, gitignored), runs the manifest the model's format
# calls for (extract-fbx-materials.py / mtl_materials.py, plus
# extract-max-materials.py when a .max ships), converts through
# convert-environment.py (material_plan.py policy, plan_apply.py
# mechanism, scene cache beside the pack), and installs
# godot/addons/environments/<key>.glb. The probe catalogs it; kits.toml
# (scale) and rosters/environments/<key>.toml (zones) make it a level.
# Windows derive into rosters/windows/<key>.toml only once that roster
# exists — the level grammar consumes them by environment key. Adding a
# scene = a folder + one convert_environment line (+ its kit + zones).
ENVIRONMENTS_DIR="$GODOT_DIR/addons/environments"
# Texture cap for scenes: Godot's own maximum texture side. The earlier
# 2048 was a VRAM guess nobody asked for — the villa's 8192 ground and
# 3840 concrete came through at a quarter size and read as pixels
# (owner 2026-09-06: "why are we being precious with these resources?
# we have 64 GB of RAM"). The metrics's [textures] block carries every
# scene's shipped-versus-source megapixels; a pack that ever needs a
# cap gets it per pack through convert_environment's options.
ENV_TEX_CAP=16384

lfs_guard() {  # <file> — a git-lfs pointer stub means the clone skipped LFS
    if head -c 12 "$1" | grep -q "^version http"; then
        echo "  ERROR: $1 is a git-lfs pointer stub."
        echo "  Run 'make deps' (installs git-lfs) then 'git lfs pull'."
        exit 1
    fi
}

# convert_environment <key> <pack_dir> [model file name] [converter options...]
# The model is the first .fbx (else .obj) found in the pack or its
# extracted archives; a pack that ships several names its pick. Every
# archive at the pack's top level is a provider input (extracted, and a
# freshness input); what a seller ships for HUMANS — render galleries,
# manuals — lives under <pack>/reference/, which the stage never reads
# (the hill house's 830 MB render zip would otherwise unpack a gigabyte
# of PSDs into the texture inventory and reconvert the scene). Any
# further arguments go to convert-environment.py verbatim — the per-pack
# provider facts (--unit-scale, --obj-up/--obj-forward, --keep=box),
# each recorded here with its provenance. They are freshness inputs
# through a stamp file beside the pack: change one and only that pack
# reconverts.
# glTF-Transform (tools/node, `make deps-node`): the glb-stage optimizer
# every scene passes through after export. `join` runs dedup and flatten
# first — content-identical materials (SketchUp's thousands of clones
# export as identical glTF materials) merge under one name and their
# primitives join, keepMeshes keeping node and mesh identity — and
# `prune` drops what nothing references. The hill house's 11,571
# single-mesh surfaces (Godot keeps 256 per mesh and drops the rest)
# became 175 in ten seconds (2026-09-07). The audit's surface-cap and
# per-class conservation contracts judge the product.
GLTF_TRANSFORM="${GLTF_TRANSFORM:-tools/node/node_modules/.bin/gltf-transform}"
GLTF_TRANSFORM_VERSION="${GLTF_TRANSFORM_VERSION:-unpinned}"
# The optimizer's options are part of every pack's conversion stamp: a
# change here reconverts every scene, like any converter option.
# keepMeshes keeps node and mesh identity; keep-solid-textures keeps a
# solid single-color map a map (prune's default folds it into a color
# factor, which reads identically but contradicts the plan's word and
# the audit's textured/flat contract, 2026-09-07).
OPTIMIZE_OPTS="gltf-transform-$GLTF_TRANSFORM_VERSION join:keepMeshes prune:keep-solid-textures"
optimize_glb() {
    local glb="$1" log="$2" joined="${1%.glb}.joined.glb"
    if [ ! -x "$GLTF_TRANSFORM" ]; then
        echo "  ERROR: gltf-transform not found at $GLTF_TRANSFORM — run 'make deps'"
        return 1
    fi
    echo "  Optimizing $(basename "$glb") ($OPTIMIZE_OPTS)..."
    NODE_OPTIONS=--max-old-space-size=24576 "$GLTF_TRANSFORM" join "$glb" "$joined" --keepMeshes true >> "$log" 2>&1 \
        && NODE_OPTIONS=--max-old-space-size=24576 "$GLTF_TRANSFORM" prune "$joined" "$glb" --keep-solid-textures true >> "$log" 2>&1 \
        || {
            echo "  ERROR: gltf-transform failed on $glb (see $log)"
            rm -f "$joined" "$glb"
            return 1
        }
    rm -f "$joined"
    grep -E "^info: .*→" "$log" | tail -2 | sed 's/^info: /  /'
}

convert_environment() {
    local key="$1" pack="$2" pick="${3:-}"
    shift 3 2>/dev/null || shift $#
    # The stamp carries the model pick and every converter option, the
    # texture cap included, so a pick or a cap change reconverts like any
    # other option change (a pack's alternate export is usually OLDER
    # than the product, so mtime alone would never notice the switch).
    local stamp="$pack/convert.opts" opts="${pick:-first-model} $* --tex-cap $ENV_TEX_CAP --optimize $OPTIMIZE_OPTS"
    # Metrics-only re-reads products; it must never touch a stamp (`make
    # metrics` once ran without the optimizer version in its environment,
    # stamped every pack "unpinned", and the next real run reconverted all
    # four scenes for nothing, 2026-09-08).
    if [ -z "$METRICS_ONLY" ] && { [ ! -f "$stamp" ] || [ "$(cat "$stamp")" != "$opts" ]; }; then
        printf '%s' "$opts" > "$stamp"
    fi
    if [ ! -d "$pack" ]; then
        echo "  $key pack not found at $pack, skipping."
        return
    fi
    if [ ! -x "$BLENDER" ]; then
        echo "  WARNING: Blender not found at $BLENDER — run 'make deps'. Skipping $key."
        return
    fi
    mkdir -p "$ENVIRONMENTS_DIR"
    local unpacked="$pack/unpacked" a model manifest max_src max_table="" mtl out windows=""
    for a in "$pack"/*.rar "$pack"/*.zip; do
        [ -f "$a" ] || continue
        [ -n "$METRICS_ONLY" ] && continue
        lfs_guard "$a"
        extract_archive "$a" "$unpacked/$(basename "${a%.*}")"
    done
    if [ -n "$pick" ]; then
        model="$(find "$pack" -maxdepth 3 -name "$pick" -not -path '*/.*' | head -1)"
    else
        model="$(find "$pack" -maxdepth 3 -iname '*.fbx' -not -path '*/.*' | sort | head -1)"
        [ -n "$model" ] || model="$(find "$pack" -maxdepth 3 -iname '*.obj' -not -path '*/.*' | sort | head -1)"
    fi
    if [ -z "$model" ] || [ ! -f "$model" ]; then
        echo "  ERROR: $key: no model (.fbx / .obj${pick:+ / $pick}) under $pack"
        exit 1
    fi
    lfs_guard "$model"
    # The material manifest the model's format calls for, once per change.
    case "$(echo "${model##*.}" | tr '[:upper:]' '[:lower:]')" in
        obj)
            mtl="${model%.*}.mtl"
            [ -f "$mtl" ] || mtl="$(find "$(dirname "$model")" -maxdepth 1 -iname '*.mtl' | head -1)"
            manifest="$pack/mtl_materials.json"
            if [ -z "$METRICS_ONLY" ] && stale "$manifest" -- "$model" "$mtl" "$SCRIPTS_DIR/mtl_materials.py"; then
                echo "  Extracting $key material manifest from the MTL..."
                python3 "$SCRIPTS_DIR/mtl_materials.py" "$model" "$mtl" "$manifest" \
                    | grep -i "mtl-materials:" || echo "  (mtl-materials: no summary — check output)"
            fi ;;
        *)
            manifest="$pack/fbx_materials.json"
            if [ -z "$METRICS_ONLY" ] && stale "$manifest" -- "$model" "$SCRIPTS_DIR/extract-fbx-materials.py"; then
                echo "  Extracting $key material manifest from FBX connection tables..."
                mkdir -p "$OUT_DIR"
                "$BLENDER" --background --python-exit-code 1 \
                    --python "$SCRIPTS_DIR/extract-fbx-materials.py" -- "$model" "$manifest" \
                    > "$OUT_DIR/manifest-$key.log" 2>&1 \
                    || echo "  ERROR: $key manifest extraction failed (see out/manifest-$key.log)"
                grep -i "extract-fbx-materials:" "$OUT_DIR/manifest-$key.log" \
                    || echo "  (extract-fbx-materials: no summary — see out/manifest-$key.log)"
            fi ;;
    esac
    # A .max source is the authoring truth for Corona materials (the FBX
    # export destroys most of their bindings); its table merges over the
    # manifest in the plan.
    max_src="$(find "$pack" -maxdepth 3 -iname '*.max' -not -path '*/.*' | head -1)"
    if [ -n "$max_src" ]; then
        max_table="$pack/max_materials.json"
        if [ -z "$METRICS_ONLY" ] && stale "$max_table" -- "$max_src" "$SCRIPTS_DIR/extract-max-materials.py"; then
            echo "  Extracting $key material table from the .max source..."
            mkdir -p "$OUT_DIR"
            "$BLENDER" --background --python-exit-code 1 \
                --python "$SCRIPTS_DIR/extract-max-materials.py" -- "$max_src" "$max_table" \
                > "$OUT_DIR/max-$key.log" 2>&1 \
                || echo "  ERROR: $key .max table extraction failed (see out/max-$key.log)"
            grep -i "extract-max:" "$OUT_DIR/max-$key.log" \
                || echo "  (extract-max: no summary — see out/max-$key.log)"
        fi
    fi
    out="$ENVIRONMENTS_DIR/$key.glb"
    [ -f "rosters/environments/$key.toml" ] && windows="rosters/windows/$key.toml"
    if [ -n "$METRICS_ONLY" ]; then
        :  # metrics-only: the installed product is what there is
    elif stale "$out" ${windows:+"$windows"} -- "$model" "$manifest" ${max_table:+"$max_table"} "$stamp" \
            "$pack"/*.rar "$pack"/*.zip "$SCRIPTS_DIR/convert-environment.py" \
            "$SCRIPTS_DIR/plan_apply.py" "$SCRIPTS_DIR/material_plan.py" \
            "$SCRIPTS_DIR/retile.py"; then
        echo "  Converting $key (full detail, ${ENV_TEX_CAP}px textures; log: out/convert-$key.log)..."
        # Full Blender output to a log — a failed conversion's traceback
        # is a run artifact, not noise (the office building died silently
        # behind a summary grep, 2026-09-06).
        mkdir -p "$OUT_DIR"
        "$BLENDER" --background --python-exit-code 1 --python "$SCRIPTS_DIR/convert-environment.py" -- \
            "$model" "$out" "$unpacked" "$manifest" --tex-cap "$ENV_TEX_CAP" \
            --environment "$key" \
            --cache "$pack/scene_cache.blend" --report "$pack/conversion.json" \
            ${max_table:+--max "$max_table"} ${windows:+--windows "$windows"} \
            "$@" \
            > "$OUT_DIR/convert-$key.log" 2>&1 \
            || {
                # A failed conversion must not leave the previous product
                # standing as if current (run 9 blessed a stale office
                # building that way, 2026-09-06).
                grep -i "convert-environment:" "$OUT_DIR/convert-$key.log" || true
                echo "  ERROR: $key conversion failed (see out/convert-$key.log)"
                rm -f "$out"
                exit 1
            }
        grep -i "convert-environment:" "$OUT_DIR/convert-$key.log" || true
        optimize_glb "$out" "$OUT_DIR/convert-$key.log" || exit 1
    else
        echo "  $key.glb is fresh."
    fi
    if [ ! -f "$out" ]; then
        echo "  ERROR: $key conversion produced no glb"
        exit 1
    fi
    # The metrics of what got built (out/metrics/<key>.toml + the section
    # sections <key>_plan{,_low,_high}.png / <key>_{long,cross}.png: the
    # scene cut by planes, with the authored zone boxes drawn over the
    # walls): the reproducible reading zone authoring and the kit-scale
    # decision start from. Re-measures when the zone roster changes, so
    # a box edit shows against the walls.
    local metrics="$OUT_DIR/metrics/$key.toml" zones="rosters/environments/$key.toml"
    if stale "$metrics" -- "$out" "$manifest" "$zones" "$SCRIPTS_DIR/scene-metrics.py" "$SCRIPTS_DIR/cockpit_plan.py"; then
        mkdir -p "$OUT_DIR/metrics"
        python3 "$SCRIPTS_DIR/scene-metrics.py" "$key" "$out" "$metrics" "$manifest" "$zones" \
            | grep -i "scene-metrics:" || echo "  ($key: no metrics — check scene-metrics.py output)"
        # glTF-Transform's own inspection beside ours (meshes, primitives,
        # materials, textures as the standard tool reads them).
        if [ -x "$GLTF_TRANSFORM" ]; then
            NODE_OPTIONS=--max-old-space-size=24576 "$GLTF_TRANSFORM" inspect "$out" --format md \
                > "$OUT_DIR/metrics/$key.inspect.md" 2>&1 || echo "  ($key: gltf-transform inspect failed — see out/metrics/$key.inspect.md)"
        fi
    fi
    chmod -R u+w "$ENVIRONMENTS_DIR"
    echo "  $key environment installed."
}

convert_environment "apartment" "$ASSETS_DIR/apartment" ""
# SketchUp's OBJ is millimeters and Z up (manifest metrics 2026-09-06: the
# room body spans ~6 x 12.5 x 2.8 m once scaled — x 7.6..13.6, y
# 0.5..3.3, z -22.3..-9.8 in the metrics frame — inside the provider's
# 175 m scenery backdrop). The backdrop STAYS: it is the view through
# the glass walls (owner 2026-09-06: clipped, the outside was black).
convert_environment "hill_house" "$ASSETS_DIR/hill_house" "" \
    --unit-scale 0.001 --obj-up Z --obj-forward NEGATIVE_Y
# The office pack lays its four color schemes in a row along z, 100 m
# apart (metrics 2026-09-06: plain at z 25..75, Blue at -75..-25, "2" at
# -175..-125, "2Violet" at 125..175). One building is the level: the
# plain scheme. Pick another by moving the box (that pack reconverts).
convert_environment "office_building" "$ASSETS_DIR/office_building" "" "--keep=-45,-1,0,25,50,100"
# The villa's FBX archive ships two exports; the single-UV one is the
# game mesh (one UV set per vertex, what the glTF wants). Its file
# declares inches (metrics: unit_scale_factor 2.54) but the geometry
# reads as decimeters — at 0.1 m per unit the house is 4.7 m tall on a
# 20 x 16 m footprint (the listing's villa); at the declared inch it is
# a 1.2 m dollhouse. 0.1 / 0.0254 on top of the declared conversion.
convert_environment "mountain_villa" "$ASSETS_DIR/mountain_villa" "FBX UV.fbx" \
    --unit-scale 3.937

# ========== Skies (attributed downloads -> godot/addons/sky) ==========
# The panoramas fixed kits name (catalog/kits.toml [skies]): assets/sky/
# holds the checksum-pinned downloads (`make assets-fetch`,
# catalog/attributions.toml — the credits page renders from the same
# file) and each installs as shipped; the import stage sets its sidecar
# to VRAM-uncompressed float with mipmaps (Makefile). The audit holds
# every catalogued sky to an installed, attributed file.
SKY_DIR="$GODOT_DIR/addons/sky"
mkdir -p "$SKY_DIR"
for sky in "$ASSETS_DIR"/sky/*.exr "$ASSETS_DIR"/sky/*.hdr; do
    [ -f "$sky" ] || continue
    lfs_guard "$sky"
    if stale "$SKY_DIR/$(basename "$sky")" -- "$sky"; then
        echo "  Installing sky $(basename "$sky")..."
        cp "$sky" "$SKY_DIR/"
    fi
done

# ========== Panel-world kits (CGTrader parts kits -> per-piece glB) ==========
# B11 cubic-cell panel worlds: each "Sci-Fi Parts Kit" pack carries its
# pieces as sibling objects (Vol 01: one GLB with embedded textures; Vol 03:
# a .blend with its Principled graphs wired to the loose PBR maps beside
# it). split-panels.py sniffs the format, snaps near-pitch squares to the
# kit's largest square, decimates each piece to game weight, and exports one
# stable-named .glb per piece. Pieces serve ANY cell face — there are no
# floors or ceilings in 6DOF (see the B11 plan). A new "Vol N" pack is one
# more split_panel_kit line (+ its kits.toml entry).
WALLS_SRC="$ASSETS_DIR/more_walls"

# split_panel_kit <label> <source> <install_dir>
split_panel_kit() {
    local label="$1" src="$2" dest="$3"
    [ -z "$METRICS_ONLY" ] || return 0  # panels are measured by the probe (Door 3)
    if [ -z "$src" ] || [ ! -f "$src" ]; then
        echo "  $label source not found, skipping."
        return
    fi
    if head -c 12 "$src" | grep -q "^version http"; then
        echo "  ERROR: $src is a git-lfs pointer stub."
        echo "  Run 'make deps' (installs git-lfs) then 'git lfs pull'."
        exit 1
    fi
    if ! stale "$OUT_DIR/split-$label.log" "$dest" -- "$src" "$SCRIPTS_DIR/split-panels.py"; then
        echo "  $label pieces are fresh ($(ls "$dest" | wc -l | tr -d ' ')); see out/split-$label.log."
        return
    fi
    echo "  Splitting $label panels (target 800 tris each)..."
    mkdir -p "$dest"
    # Fresh slate for the GLBS: a stale piece or variant from a prior
    # run would be measured as if current. The manifest SURVIVES — it
    # carries the Transform's persistent flip state into the next bake
    # (the split overwrites it at the end of its run).
    rm -f "$dest"/*.glb
    # Full split output to a log — the Transform's decisions (bakes,
    # skips, stacks, flips) are run artifacts, not noise; swallowing
    # them into /dev/null bred ad-hoc out-of-band Blender runs.
    mkdir -p "$(dirname "$0")/../out"
    "$BLENDER" --background --python-exit-code 1 --python "$(dirname "$0")/split-panels.py" -- \
        "$src" "$dest" 800 > "$(dirname "$0")/../out/split-$label.log" 2>&1 || \
        echo "  WARNING: $label panel split failed (see out/split-$label.log)"
    echo "  $label pieces installed: $(ls "$dest" | wc -l | tr -d ' ')"
}

# -iname: provider archives mix "Vol+01" dirs with "vol 03" filenames.
split_panel_kit "vol01" \
    "$(find "$WALLS_SRC" -maxdepth 1 -iname "*vol*01*.glb" 2>/dev/null | head -1)" \
    "$GODOT_DIR/addons/walls"
split_panel_kit "vol03" \
    "$(find "$WALLS_SRC" -maxdepth 2 -iname "*vol*03*.blend" 2>/dev/null | head -1)" \
    "$GODOT_DIR/addons/walls_vol3"

# ========== Jump gate (CGTrader OBJ -> decimated glB) ==========
# The end-of-level exit portal model. Ships as a ~78k-tri OBJ + .mtl + loose PBR
# maps; the same headless Blender pass that decimates the enemy mechs collapses
# it to a game-weight glb (textures embedded) under godot/addons/props/. The
# .mtl references its textures, so the OBJ importer materials need no rebuild —
# decimate.py keeps them and ignores the trailing tex_dir arg.

# A single static prop — keep full detail (0 = no decimation). Decimating it
# would collapse the gate's tiny energy-field plane; the tri budget only matters
# for the many-instances enemy mechs.
JUMP_GATE_SRC="$SHIPS_SRC/jump_gate/JumpGate.obj"
JUMP_GATE_TARGET=0

if [ -f "$JUMP_GATE_SRC" ]; then
    PROPS_DIR="$GODOT_DIR/addons/props"
    mkdir -p "$PROPS_DIR"
    if [ ! -x "$BLENDER" ]; then
        echo "  WARNING: Blender not found at $BLENDER — run 'make deps'. Skipping jump gate."
    else
        decimate_model "jump gate" "$JUMP_GATE_SRC" "$PROPS_DIR/jump_gate.glb" \
            "$JUMP_GATE_TARGET" "$(dirname "$JUMP_GATE_SRC")"
        chmod -R u+w "$PROPS_DIR"
        echo "  Jump gate installed."
    fi
else
    echo "  jump_gate OBJ not found, skipping."
fi

# ========== Audio assets (music + SFX) ==========

AUDIO_DIR="$GODOT_DIR/addons/audio"
MUSIC_SRC="$ASSETS_DIR/music"
SFX_SRC="$ASSETS_DIR/sfx"

# ---------- Music ----------

if [ -d "$MUSIC_SRC" ]; then
    echo "  Installing music..."
    # Four beds (owner's layout 2026-07-05): ambient (the original wavs —
    # menu + reserve), per-level loopable backgrounds (1-30 so far), combat
    # stingers (11-19, random per fight), boss tracks (1-6). Numbered
    # sources install under stable numeric names the catalog derives.
    mkdir -p "$AUDIO_DIR/music/ambient" "$AUDIO_DIR/music/levels" \
             "$AUDIO_DIR/music/combat" "$AUDIO_DIR/music/boss"
    for wav in "$MUSIC_SRC/Ambient"/*.wav; do
        [ -f "$wav" ] || continue
        base="$(basename "$wav" .wav)"
        clean="$(echo "$base" | sed 's/^juanjo_sound - //' | tr '[:upper:]' '[:lower:]' | tr ' ' '_')"
        cp "$wav" "$AUDIO_DIR/music/ambient/${clean}.wav"
    done
    for mp3 in "$MUSIC_SRC/Level Backgrounds"/*.mp3; do
        [ -f "$mp3" ] || continue
        num="$(basename "$mp3" | sed 's/^\([0-9]*\)\..*/\1/')"
        cp "$mp3" "$AUDIO_DIR/music/levels/level_$(printf '%02d' "$num").mp3"
    done
    for mp3 in "$MUSIC_SRC/combat"/*.mp3; do
        [ -f "$mp3" ] || continue
        num="$(basename "$mp3" | sed 's/^\([0-9]*\)\..*/\1/')"
        cp "$mp3" "$AUDIO_DIR/music/combat/combat_${num}.mp3"
    done
    for mp3 in "$MUSIC_SRC/Boss Music Tracks"/*.mp3; do
        [ -f "$mp3" ] || continue
        num="$(basename "$mp3" | sed 's/^\([0-9]*\)\..*/\1/')"
        cp "$mp3" "$AUDIO_DIR/music/boss/boss_${num}.mp3"
    done
    chmod -R u+w "$AUDIO_DIR/music"
    echo "  Music installed ($(ls "$AUDIO_DIR/music" | wc -l | tr -d ' ') tracks)."
else
    echo "  Music source not found at $MUSIC_SRC, skipping."
fi

# ---------- Sound effects ----------

if [ -d "$SFX_SRC" ]; then
    echo "  Installing SFX..."
    mkdir -p "$AUDIO_DIR/sfx"
    rsync -a --exclude='.DS_Store' --exclude='*.reapeaks' "$SFX_SRC/" "$AUDIO_DIR/sfx/"
    chmod -R u+w "$AUDIO_DIR/sfx"
    echo "  SFX installed."
else
    echo "  SFX source not found at $SFX_SRC, skipping."
fi

if [ -d "$AUDIO_DIR" ]; then
    echo "  Audio addon ready at: $AUDIO_DIR"
    du -sh "$AUDIO_DIR"
fi
