#!/usr/bin/env bash
set -euo pipefail

# Install Godot addons from downloaded asset packs into godot/addons/.
# Source packs live in assets/ (gitignored). This script copies only the
# files Godot needs — no Unity/Unreal projects, no duplicate root .bin
# files, no stale .import descriptors.

ASSETS_DIR="${1:?Usage: install-addons.sh <assets-dir> <godot-dir>}"
GODOT_DIR="${2:?Usage: install-addons.sh <assets-dir> <godot-dir>}"
TRES_ONLY="${3:-}"   # pass --tres-only to re-copy just .tres files (fix Godot path rewrites)
ADDON_DIR="$GODOT_DIR/addons/quaternius"

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
BLENDER="${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}"
DECIMATE_TARGET=2000

if [ -d "$EVIL_MECHS_SRC" ]; then
    ENEMIES_DIR="$GODOT_DIR/addons/enemies"
    mkdir -p "$ENEMIES_DIR"
    if [ ! -x "$BLENDER" ]; then
        echo "  WARNING: Blender not found at $BLENDER — run 'make deps'. Skipping enemy mechs."
    else
        echo "  Decimating enemy mech models (target ${DECIMATE_TARGET} tris)..."
        # Drop any stale raw-FBX install from before decimation existed.
        rm -f "$ENEMIES_DIR"/*.fbx "$ENEMIES_DIR"/*.png "$ENEMIES_DIR"/*.jpg 2>/dev/null
        for n in 01 03; do
            src="$(find "$EVIL_MECHS_SRC" -maxdepth 1 -iname "*Evil_mech_${n}*.fbx" | head -1)"
            [ -n "$src" ] || continue
            # The mech's PBR maps ship loose in a sibling "(Textures)" folder the
            # FBX doesn't reference; pass it so decimate.py rebuilds the material.
            tex="$(find "$EVIL_MECHS_SRC" -maxdepth 1 -type d -iname "*Evil_mech_${n}*Textures*" | head -1)"
            "$BLENDER" --background --python-exit-code 1 --python "$(dirname "$0")/decimate.py" -- \
                "$src" "$ENEMIES_DIR/evil_mech_${n}.glb" "$DECIMATE_TARGET" "$tex" 2>&1 \
                | grep -i "decimate:" || echo "  (mech ${n}: no decimation summary — check Blender output)"
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
            "$BLENDER" --background --python-exit-code 1 --python "$(dirname "$0")/decimate.py" -- \
                "$src" "$ENEMIES_DIR/sphere_ship_${n}.glb" "$DECIMATE_TARGET" "$tex" 2>&1 \
                | grep -i "decimate:" || echo "  (sphere ${n}: no decimation summary — check Blender output)"
        done
        src="$(find "$SPHERES_SRC" -maxdepth 1 -iname "*Alien_troop_01*.fbx" | head -1)"
        if [ -n "$src" ]; then
            tex="$(find "$SPHERES_SRC" -maxdepth 1 -type d -iname "*Alien_troop_01*Textures*" | head -1)"
            "$BLENDER" --background --python-exit-code 1 --python "$(dirname "$0")/decimate.py" -- \
                "$src" "$ENEMIES_DIR/alien_troop_01.glb" "$DECIMATE_TARGET" "$tex" 2>&1 \
                | grep -i "decimate:" || echo "  (alien troop: no decimation summary — check Blender output)"
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
# whichever directory actually holds the images.
tex_root() {  # <unpacked-dir>
    local dir
    dir="$(find "$1" -mindepth 1 -maxdepth 1 -type d | head -1)"
    if [ -n "$dir" ]; then echo "$dir"; else echo "$1"; fi
}

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
            "$BLENDER" --background --python-exit-code 1 --python "$(dirname "$0")/decimate.py" -- \
                "$src" "$ENEMIES_DIR/sphere_drone_${n}.glb" "$DECIMATE_TARGET" \
                "$(tex_root "$UNPACKED/sphere_drone_${n}_tex")" 2>&1 \
                | grep -i "decimate:" || echo "  (sphere drone ${n}: no decimation summary — check Blender output)"
        done

        src="$(find "$GREY_SPHERES_SRC" -maxdepth 1 -iname "*apartment_boss*.fbx" | head -1)"
        if [ -n "$src" ]; then
            tex_rar="$(find "$GREY_SPHERES_SRC" -maxdepth 1 -iname "*apartment_boss*Textures*.rar" | head -1)"
            [ -z "$tex_rar" ] || extract_archive "$tex_rar" "$UNPACKED/apartment_boss_tex"
            "$BLENDER" --background --python-exit-code 1 --python "$(dirname "$0")/decimate.py" -- \
                "$src" "$ENEMIES_DIR/apartment_boss.glb" "$DECIMATE_TARGET" \
                "$(tex_root "$UNPACKED/apartment_boss_tex")" "Base Rusted.png" 2>&1 \
                | grep -i "decimate:" || echo "  (apartment boss: no decimation summary — check Blender output)"
        fi

        src="$(find "$GREY_SPHERES_SRC" -maxdepth 1 -iname "*UMSFD_02*.glb" | head -1)"
        if [ -n "$src" ]; then
            tex_rar="$(find "$GREY_SPHERES_SRC" -maxdepth 1 -iname "*UMSFD_02*Textures*.rar" | head -1)"
            [ -z "$tex_rar" ] || extract_archive "$tex_rar" "$UNPACKED/umsfd_02_tex"
            "$BLENDER" --background --python-exit-code 1 --python "$(dirname "$0")/decimate.py" -- \
                "$src" "$ENEMIES_DIR/umsfd_02.glb" "$DECIMATE_TARGET" \
                "$(tex_root "$UNPACKED/umsfd_02_tex")" 2>&1 \
                | grep -i "decimate:" || echo "  (umsfd_02: no decimation summary — check Blender output)"
        fi
        chmod -R u+w "$ENEMIES_DIR"
        echo "  Grey-sphere drone models installed."
    fi
else
    echo "  grey_spheres not found, skipping planet-3 drones."
fi

# ========== Planet-3 apartment environment (CGTrader archviz FBX -> glB) ==========
# The fixed level geometry for levels 13-15: one archviz apartment exported
# from 3ds Max/Corona whose texture references arrive broken (empty filename
# fields). apartment.py parses the FBX's own connection table to rewire the
# maps from the textures zip, keeps full geometry (a single environment
# instance — no decimation), caps textures, and writes one .glb. The probe
# catalogs addons/environments/ into catalog/environments.generated.toml.

APARTMENT_SRC="$ASSETS_DIR/apartment"
APARTMENT_FBX="$(find "$APARTMENT_SRC" -maxdepth 1 -iname "*apartment*.fbx" 2>/dev/null | head -1)"
APARTMENT_MAX="$(find "$APARTMENT_SRC" -maxdepth 1 -iname "*apartment*.max" 2>/dev/null | head -1)"
APARTMENT_TEX_ZIP="$(find "$APARTMENT_SRC" -maxdepth 1 -iname "*textures*.zip" 2>/dev/null | head -1)"
APARTMENT_TEX_CAP=2048
APARTMENT_MAT_TABLE="$APARTMENT_SRC/max_materials.json"

if [ -n "$APARTMENT_FBX" ] && [ -n "$APARTMENT_TEX_ZIP" ]; then
    # LFS pointer stub instead of the real file = the clone skipped LFS.
    if head -c 12 "$APARTMENT_FBX" | grep -q "^version http"; then
        echo "  ERROR: $APARTMENT_FBX is a git-lfs pointer stub."
        echo "  Run 'make deps' (installs git-lfs) then 'git lfs pull'."
        exit 1
    fi
    ENVIRONMENTS_DIR="$GODOT_DIR/addons/environments"
    mkdir -p "$ENVIRONMENTS_DIR"
    if [ ! -x "$BLENDER" ]; then
        echo "  WARNING: Blender not found at $BLENDER — run 'make deps'. Skipping apartment."
    else
        # The .max scene is the authoring truth for materials (the FBX
        # export destroyed most Corona bindings). Extract its material
        # table once per .max change; the material plan merges it over the
        # FBX-derived recovery.
        if [ -n "$APARTMENT_MAX" ] && { [ ! -f "$APARTMENT_MAT_TABLE" ] || [ "$APARTMENT_MAX" -nt "$APARTMENT_MAT_TABLE" ]; }; then
            echo "  Extracting material table from .max source..."
            "$BLENDER" --background --python-exit-code 1 \
                --python "$(dirname "$0")/extract-max-materials.py" -- \
                "$APARTMENT_MAX" "$APARTMENT_MAT_TABLE" 2>&1 \
                | grep -i "extract-max:" || echo "  (extract-max: no summary — check Blender output)"
        fi
        # The FBX material oracle: classes, all texture-channel links,
        # container (RaySwitch/Layered) edges, transparency/emission props.
        # The ONE place the FBX connection tables get parsed; material_plan.py
        # turns it into per-material plans, `make test-assets` audits it.
        APARTMENT_FBX_TABLE="$APARTMENT_SRC/fbx_materials.json"
        if [ ! -f "$APARTMENT_FBX_TABLE" ] || [ "$APARTMENT_FBX" -nt "$APARTMENT_FBX_TABLE" ]; then
            echo "  Extracting material oracle from FBX connection tables..."
            "$BLENDER" --background --python-exit-code 1 \
                --python "$(dirname "$0")/extract-fbx-materials.py" -- \
                "$APARTMENT_FBX" "$APARTMENT_FBX_TABLE" 2>&1 \
                | grep -i "extract-fbx-materials:" || echo "  (extract-fbx-materials: no summary — check Blender output)"
        fi
        echo "  Converting apartment environment (full detail, ${APARTMENT_TEX_CAP}px textures)..."
        "$BLENDER" --background --python-exit-code 1 --python "$(dirname "$0")/apartment.py" -- \
            "$APARTMENT_FBX" "$ENVIRONMENTS_DIR/apartment.glb" \
            "$APARTMENT_TEX_ZIP" "$APARTMENT_TEX_CAP" "$APARTMENT_MAT_TABLE" \
            "rosters/windows/apartment.toml" "$APARTMENT_FBX_TABLE" 2>&1 \
            | grep -i "apartment:" || echo "  (apartment: no summary — check Blender output)"
        if [ ! -f "$ENVIRONMENTS_DIR/apartment.glb" ]; then
            echo "  ERROR: apartment conversion produced no glb"
            exit 1
        fi
        chmod -R u+w "$ENVIRONMENTS_DIR"
        echo "  Apartment environment installed."
    fi
else
    echo "  apartment FBX/textures not found, skipping planet-3 environment."
fi

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
    if [ -z "$src" ] || [ ! -f "$src" ]; then
        echo "  $label source not found, skipping."
        return
    fi
    if head -c 12 "$src" | grep -q "^version http"; then
        echo "  ERROR: $src is a git-lfs pointer stub."
        echo "  Run 'make deps' (installs git-lfs) then 'git lfs pull'."
        exit 1
    fi
    echo "  Splitting $label panels (target 800 tris each)..."
    mkdir -p "$dest"
    # Fresh slate for the GLBS: a stale piece or variant from a prior
    # run would be censused as if current. The manifest SURVIVES — it
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
        echo "  Converting jump gate (target ${JUMP_GATE_TARGET} tris)..."
        "$BLENDER" --background --python-exit-code 1 --python "$(dirname "$0")/decimate.py" -- \
            "$JUMP_GATE_SRC" "$PROPS_DIR/jump_gate.glb" "$JUMP_GATE_TARGET" \
            "$(dirname "$JUMP_GATE_SRC")" 2>&1 \
            | grep -i "decimate:" || echo "  (jump gate: no decimation summary — check Blender output)"
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
