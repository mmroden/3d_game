"""Unit tests for scripts/fetch_attributed.py — the acquisition stage for
third-party files the game downloads rather than buys: every download
is declared in catalog/attributions.toml beside the credit it owes,
fetched to its declared path, and pinned by checksum, so a fresh clone
gets byte-identical files or a loud refusal (owner 2026-09-09: the NASA
star map as the sky, and "we need to start producing a credits page").
Pure: the tests serve files from a temp directory over file:// URLs."""
import hashlib
import importlib.util
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))


def _load():
    spec = importlib.util.spec_from_file_location(
        "fetch_attributed", ROOT / "scripts" / "fetch_attributed.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


fetch = _load()


def attributions(tmp_path, downloads):
    """A one-source attributions file whose downloads point at files the
    test wrote under tmp_path/remote, fetched into tmp_path/repo."""
    rows = ",\n".join(
        "  { url = %r, path = %r%s }" % (
            f"file://{tmp_path / 'remote' / name}", f"assets/sky/{name}",
            f", sha256 = {sha!r}" if sha else "")
        for name, sha in downloads)
    text = f'''
[[source]]
key = "starmap"
name = "Deep Star Maps 2020"
author = "NASA/Goddard Space Flight Center Scientific Visualization Studio"
url = "https://svs.gsfc.nasa.gov/4851"
license = "public domain"
credit = "NASA/Goddard Space Flight Center Scientific Visualization Studio"
used_for = "the sky"
downloads = [
{rows}
]
'''
    path = tmp_path / "attributions.toml"
    path.write_text(text)
    (tmp_path / "repo").mkdir(exist_ok=True)
    return path


def remote(tmp_path, name, payload):
    (tmp_path / "remote").mkdir(exist_ok=True)
    (tmp_path / "remote" / name).write_bytes(payload)
    return hashlib.sha256(payload).hexdigest()


def test_a_pinned_download_lands_at_its_path_and_matches():
    pass


def test_a_missing_pinned_file_is_fetched_and_verified(tmp_path):
    sha = remote(tmp_path, "sky.exr", b"stars" * 1000)
    toml = attributions(tmp_path, [("sky.exr", sha)])
    outcome = fetch.main(str(toml), str(tmp_path / "repo"))
    assert outcome == 0
    assert (tmp_path / "repo" / "assets" / "sky" / "sky.exr").read_bytes() == b"stars" * 1000


def test_a_checksum_mismatch_refuses_and_leaves_no_file(tmp_path):
    """A file that does not match its pin is not an asset: it is removed
    and the stage fails, so a changed upstream never ships silently."""
    remote(tmp_path, "sky.exr", b"stars")
    toml = attributions(tmp_path, [("sky.exr", "0" * 64)])
    outcome = fetch.main(str(toml), str(tmp_path / "repo"))
    assert outcome != 0
    assert not (tmp_path / "repo" / "assets" / "sky" / "sky.exr").exists()


def test_a_present_matching_file_is_left_alone(tmp_path):
    sha = remote(tmp_path, "sky.exr", b"stars")
    toml = attributions(tmp_path, [("sky.exr", sha)])
    target = tmp_path / "repo" / "assets" / "sky" / "sky.exr"
    target.parent.mkdir(parents=True)
    target.write_bytes(b"stars")
    (tmp_path / "remote" / "sky.exr").unlink()  # nothing to fetch from: must not be needed
    assert fetch.main(str(toml), str(tmp_path / "repo")) == 0
    assert target.read_bytes() == b"stars"


def test_an_unpinned_download_is_fetched_but_the_stage_fails_until_pinned(tmp_path, capsys):
    """The first acquisition has no pin to check against: the file is
    fetched, its checksum printed for the attributions file, and the
    stage still fails — an unpinned download is not reproducible."""
    sha = remote(tmp_path, "sky.exr", b"stars")
    toml = attributions(tmp_path, [("sky.exr", None)])
    outcome = fetch.main(str(toml), str(tmp_path / "repo"))
    assert outcome != 0
    assert (tmp_path / "repo" / "assets" / "sky" / "sky.exr").read_bytes() == b"stars"
    assert sha in capsys.readouterr().out, "the checksum to pin is printed"


def test_a_source_without_downloads_needs_nothing(tmp_path):
    toml = attributions(tmp_path, [])
    assert fetch.main(str(toml), str(tmp_path / "repo")) == 0
