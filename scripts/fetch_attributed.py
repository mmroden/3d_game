#!/usr/bin/env python3
"""Acquisition stage for third-party files the game downloads rather than
buys (the NASA star map as the sky, owner 2026-09-09). Every download is
declared in catalog/attributions.toml beside the credit it owes:

    [[source]] ... downloads = [{ url, path, sha256 }]

`make assets` runs this before the install stage: a declared file that
is present and matches its pin is left alone; a missing one is fetched
to its path and verified; a mismatch is removed and fails the stage (a
changed upstream never ships silently); a download with no pin yet is
fetched, its checksum printed for the attributions file, and the stage
still fails — an unpinned download is not reproducible.

Usage: fetch_attributed.py <attributions.toml> [repo root]"""
import hashlib
import os
import sys
import urllib.request

try:
    import tomllib
except ImportError:  # the audit venv (python 3.9) carries tomli
    import tomli as tomllib

CHUNK = 1 << 20


def sha256_of(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(CHUNK), b""):
            digest.update(block)
    return digest.hexdigest()


def fetch(url, path):
    """Stream `url` to `path` (a partial file is written beside it and
    moved into place only when the stream ends)."""
    os.makedirs(os.path.dirname(os.path.abspath(path)) or ".", exist_ok=True)
    partial = path + ".part"
    with urllib.request.urlopen(url) as response, open(partial, "wb") as out:
        for block in iter(lambda: response.read(CHUNK), b""):
            out.write(block)
    os.replace(partial, path)


def main(attributions_path, root="."):
    with open(attributions_path, "rb") as f:
        doc = tomllib.load(f)
    failures = 0
    for source in doc.get("source", []):
        for download in source.get("downloads", []):
            rel, url, pin = download["path"], download["url"], download.get("sha256")
            path = os.path.join(root, rel)
            if os.path.isfile(path) and pin and sha256_of(path) == pin:
                print(f"fetch-attributed: {rel} is fresh")
                continue
            if os.path.isfile(path) and pin:
                print(f"fetch-attributed: {rel} does not match its pin — fetching again")
            print(f"fetch-attributed: fetching {url} -> {rel}", flush=True)
            fetch(url, path)
            got = sha256_of(path)
            size_mb = os.path.getsize(path) / 1e6
            if pin is None:
                print(f"fetch-attributed: {rel} downloaded ({size_mb:.1f} MB) but UNPINNED — "
                      f"pin it in {attributions_path} ({source.get('key', '?')}): "
                      f'sha256 = "{got}"')
                failures += 1
            elif got != pin:
                os.remove(path)
                print(f"fetch-attributed: ERROR {rel} sha256 {got} does not match the pinned "
                      f"{pin} — removed; the upstream file changed or the pin is wrong")
                failures += 1
            else:
                print(f"fetch-attributed: {rel} verified ({size_mb:.1f} MB)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:3]))
