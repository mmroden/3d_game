#!/usr/bin/env bash
set -euo pipefail

ASSETS_DIR="${1:?Usage: fetch-assets.sh <assets-dir>}"

# --- Quaternius Modular Sci-Fi MegaKit (CC0, free on itch.io) ---
MEGAKIT_DIR="$ASSETS_DIR/quaternius-megakit"
if [ -d "$MEGAKIT_DIR" ] && [ "$(ls -A "$MEGAKIT_DIR" 2>/dev/null)" ]; then
    echo "  MegaKit already present, skipping."
else
    echo ""
    echo "  ============================================================"
    echo "  Quaternius Modular Sci-Fi MegaKit (CC0, free)"
    echo "  itch.io requires a browser to download — cannot automate."
    echo ""
    echo "  1. Visit: https://quaternius.itch.io/modular-sci-fi-megakit"
    echo "  2. Click 'Download Now' → 'No thanks, just take me to the downloads'"
    echo "  3. Download the Standard (free) .zip"
    echo "  4. Unzip into: $MEGAKIT_DIR"
    echo "  5. Re-run: make deps"
    echo "  ============================================================"
    echo ""
    mkdir -p "$MEGAKIT_DIR"
    exit 1
fi

# --- Quaternius Sci-Fi Essentials Kit (CC0, free on itch.io) ---
ESSENTIALS_DIR="$ASSETS_DIR/quaternius-essentials"
if [ -d "$ESSENTIALS_DIR" ] && [ "$(ls -A "$ESSENTIALS_DIR" 2>/dev/null)" ]; then
    echo "  Essentials Kit already present, skipping."
else
    echo ""
    echo "  ============================================================"
    echo "  Quaternius Sci-Fi Essentials Kit (CC0, free)"
    echo "  itch.io requires a browser to download — cannot automate."
    echo ""
    echo "  1. Visit: https://quaternius.itch.io/sci-fi-essentials-kit"
    echo "  2. Click 'Download Now' → 'No thanks, just take me to the downloads'"
    echo "  3. Download the Standard (free) .zip"
    echo "  4. Unzip into: $ESSENTIALS_DIR"
    echo "  5. Re-run: make deps"
    echo "  ============================================================"
    echo ""
    mkdir -p "$ESSENTIALS_DIR"
    exit 1
fi

# --- Kenney Space Kit (CC0, direct download) ---
KENNEY_DIR="$ASSETS_DIR/kenney-space-kit"
if [ -d "$KENNEY_DIR" ] && [ "$(ls -A "$KENNEY_DIR" 2>/dev/null)" ]; then
    echo "  Kenney Space Kit already present, skipping."
else
    echo "  Fetching Kenney Space Kit..."
    mkdir -p "$KENNEY_DIR"
    curl -L -o "$KENNEY_DIR/spacekit.zip" \
        "https://kenney.nl/media/pages/assets/space-kit/cceeafbd0c-1677698978/kenney_space-kit.zip"
    unzip -o -q "$KENNEY_DIR/spacekit.zip" -d "$KENNEY_DIR"
    rm -f "$KENNEY_DIR/spacekit.zip"
    echo "  Kenney Space Kit ready."
fi

echo "All assets fetched."
