#!/usr/bin/env bash
set -euo pipefail

ASSETS_DIR="${1:?Usage: fetch-assets.sh <assets-dir>}"

# --- Quaternius Modular Sci-Fi MegaKit (free CC0 version from OpenGameArt) ---
MEGAKIT_DIR="$ASSETS_DIR/quaternius-megakit"
if [ ! -d "$MEGAKIT_DIR" ]; then
    echo "  Fetching Quaternius Modular Sci-Fi MegaKit..."
    mkdir -p "$MEGAKIT_DIR"
    curl -L -o "$MEGAKIT_DIR/megakit.zip" \
        "https://opengameart.org/sites/default/files/Modular%20Sci-Fi%20MegaKit.zip"
    unzip -o -q "$MEGAKIT_DIR/megakit.zip" -d "$MEGAKIT_DIR"
    rm -f "$MEGAKIT_DIR/megakit.zip"
    echo "  MegaKit ready."
else
    echo "  MegaKit already present, skipping."
fi

# --- Quaternius Sci-Fi Essentials Kit (enemies, props) ---
ESSENTIALS_DIR="$ASSETS_DIR/quaternius-essentials"
if [ ! -d "$ESSENTIALS_DIR" ]; then
    echo "  Fetching Quaternius Sci-Fi Essentials Kit..."
    mkdir -p "$ESSENTIALS_DIR"
    curl -L -o "$ESSENTIALS_DIR/essentials.zip" \
        "https://opengameart.org/sites/default/files/Sci-Fi%20Essentials.zip"
    unzip -o -q "$ESSENTIALS_DIR/essentials.zip" -d "$ESSENTIALS_DIR"
    rm -f "$ESSENTIALS_DIR/essentials.zip"
    echo "  Essentials Kit ready."
else
    echo "  Essentials Kit already present, skipping."
fi

# --- Kenney Space Kit (ships, drones, turrets) ---
KENNEY_DIR="$ASSETS_DIR/kenney-space-kit"
if [ ! -d "$KENNEY_DIR" ]; then
    echo "  Fetching Kenney Space Kit..."
    mkdir -p "$KENNEY_DIR"
    curl -L -o "$KENNEY_DIR/spacekit.zip" \
        "https://kenney.nl/media/13752/kenneyNL_1Bit_Pack.zip"
    # Kenney uses direct download links; if this fails, try the assets page
    if [ $? -ne 0 ]; then
        echo "  WARNING: Kenney download failed. Visit https://kenney.nl/assets/space-kit manually."
    else
        unzip -o -q "$KENNEY_DIR/spacekit.zip" -d "$KENNEY_DIR"
        rm -f "$KENNEY_DIR/spacekit.zip"
    fi
    echo "  Kenney Space Kit ready."
else
    echo "  Kenney Space Kit already present, skipping."
fi

echo "All assets fetched."
