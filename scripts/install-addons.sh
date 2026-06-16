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
    chmod -R u+w "$SHIPS_DIR"
    echo "  Player ship models installed ($(ls "$SHIPS_DIR"/*.glb 2>/dev/null | wc -l | tr -d ' ') models)."
else
    echo "  cgtrader_ships not found, skipping player ships."
fi

# ========== Audio assets (music + SFX) ==========

AUDIO_DIR="$GODOT_DIR/addons/audio"
MUSIC_SRC="$ASSETS_DIR/music"
SFX_SRC="$ASSETS_DIR/sfx"

# ---------- Music ----------

if [ -d "$MUSIC_SRC" ]; then
    echo "  Installing music..."
    mkdir -p "$AUDIO_DIR/music"
    for wav in "$MUSIC_SRC"/*.wav; do
        [ -f "$wav" ] || continue
        # Sanitize filename: strip "juanjo_sound - " prefix, lowercase, spaces→underscores
        base="$(basename "$wav" .wav)"
        clean="$(echo "$base" | sed 's/^juanjo_sound - //' | tr '[:upper:]' '[:lower:]' | tr ' ' '_')"
        cp "$wav" "$AUDIO_DIR/music/${clean}.wav"
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
