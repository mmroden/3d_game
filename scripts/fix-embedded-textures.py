#!/usr/bin/env python3
"""Fix .tres files that have empty CompressedTexture2D sub_resources.

The Quaternius asset pack ships some .tres material files with embedded
CompressedTexture2D sub_resources that have load_path pointing to
.s3tc.ctex files (Windows/Linux S3TC texture compression).  On macOS/Metal
these don't exist, and stripping the load_path lines leaves the textures
empty (rendering as white → broken emissive/detail).

This script converts those embedded sub_resources to ext_resource
references pointing to the actual .png files, which Godot can resolve
on any platform.

Usage:  fix-embedded-textures.py <materials-dir>
"""
import re
import sys
from pathlib import Path


def fix_tres_file(path: Path) -> bool:
    """Fix a single .tres file.  Returns True if changes were made."""
    text = path.read_text()

    # Find all sub_resource CompressedTexture2D blocks with load_path
    # Pattern: [sub_resource type="CompressedTexture2D" id="X"]\nload_path = "...Y.png-Z.s3tc.ctex"
    sub_pattern = re.compile(
        r'\[sub_resource type="CompressedTexture2D" id="([^"]+)"\]\n'
        r'load_path = "res://\.godot/imported/([^"]+?)(-[a-f0-9]+\.s3tc\.ctex)"',
    )

    matches = list(sub_pattern.finditer(text))
    if not matches:
        # Also catch already-stripped (empty) CompressedTexture2D blocks
        empty_pattern = re.compile(
            r'\[sub_resource type="CompressedTexture2D" id="([^"]+)"\]\n(?=\n|\[)',
        )
        empty_matches = list(empty_pattern.finditer(text))
        if not empty_matches:
            return False
        # For empty blocks, we need the source file to determine the .png name.
        # Skip these — they need the source load_path info.
        return False

    changed = False
    for m in matches:
        sub_id = m.group(1)
        png_name = m.group(2)  # e.g. "T_Trim_02_DetailMask.png"

        # Determine the materials directory from existing ext_resource paths
        mat_dir = "res://addons/quaternius/materials/"

        # Remove the sub_resource block
        text = text.replace(m.group(0) + "\n", "")
        # If there's a trailing blank line, clean it up
        text = text.replace(m.group(0), "")

        # Add ext_resource entry (after existing ext_resources)
        ext_line = f'[ext_resource type="Texture2D" path="{mat_dir}{png_name}" id="{sub_id}"]\n'

        # Insert before the first [sub_resource or [resource] line
        insert_re = re.compile(r'^(\[(?:sub_resource|resource)\b)', re.MULTILINE)
        insert_match = insert_re.search(text)
        if insert_match:
            text = text[:insert_match.start()] + ext_line + "\n" + text[insert_match.start():]

        # Replace SubResource("X") with ExtResource("X")
        text = text.replace(f'SubResource("{sub_id}")', f'ExtResource("{sub_id}")')

        changed = True

    if changed:
        # Update load_steps count (we removed sub_resources, added ext_resources — net zero)
        path.write_text(text)

    return changed


def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <materials-dir>", file=sys.stderr)
        sys.exit(1)

    materials_dir = Path(sys.argv[1])
    fixed = 0
    for tres in sorted(materials_dir.glob("*.tres")):
        if fix_tres_file(tres):
            print(f"  Fixed embedded textures: {tres.name}")
            fixed += 1

    print(f"  {fixed} files fixed.")


if __name__ == "__main__":
    main()
