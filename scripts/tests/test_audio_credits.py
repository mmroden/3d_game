"""Unit tests for scripts/audio_credits.py — the reading of the credits
provider audio carries in its own tags (RIFF INFO, Broadcast Wave bext,
ID3v2 in mp3s and embedded in wavs) into
catalog/audio_credits.generated.toml. The files are built here byte by
byte from the container specs, so every expectation derives from what
was written, never from a shipped pack."""
import importlib.util
import struct
import sys
from pathlib import Path

try:
    import tomllib
except ImportError:  # the audit venv (python 3.9) carries tomli
    import tomli as tomllib

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))


def _load():
    spec = importlib.util.spec_from_file_location(
        "audio_credits", ROOT / "scripts" / "audio_credits.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


audio_credits = _load()


# ---- byte-level builders (RIFF and ID3 per their specs) ----

def _chunk(cid, payload):
    pad = b"\0" if len(payload) % 2 else b""
    return cid + struct.pack("<I", len(payload)) + payload + pad


def _wav(info=None, bext=None, id3=None):
    fmt = _chunk(b"fmt ", struct.pack("<HHIIHH", 1, 1, 44100, 88200, 2, 16))
    body = b"WAVE" + fmt + _chunk(b"data", b"\0" * 6)
    if info:
        payload = b"INFO" + b"".join(
            _chunk(k.encode("ascii"), v.encode("utf-8") + b"\0") for k, v in info.items())
        body += _chunk(b"LIST", payload)
    if bext:
        description, originator, reference = bext
        payload = (description.encode("utf-8").ljust(256, b"\0")
                   + originator.encode("utf-8").ljust(32, b"\0")
                   + reference.encode("utf-8").ljust(32, b"\0")
                   + b"\0" * 282)
        body += _chunk(b"bext", payload)
    if id3:
        body += _chunk(b"id3 ", id3)
    return b"RIFF" + struct.pack("<I", len(body)) + body


def _syncsafe(n):
    return bytes([(n >> 21) & 0x7F, (n >> 14) & 0x7F, (n >> 7) & 0x7F, n & 0x7F])


def _id3(frames, version=3):
    body = b""
    for fid, text, encoding in frames:
        if encoding == 0:
            payload = b"\0" + text.encode("latin-1")
        elif encoding == 1:
            payload = b"\1" + text.encode("utf-16")  # BOM-led
        elif encoding == 2:
            payload = b"\2" + text.encode("utf-16-be")
        else:
            payload = b"\3" + text.encode("utf-8")
        size = _syncsafe(len(payload)) if version == 4 else struct.pack(">I", len(payload))
        body += fid.encode("ascii") + size + b"\0\0" + payload
    body += b"\0" * 16  # padding, as encoders leave it
    return b"ID3" + bytes([version, 0, 0]) + _syncsafe(len(body)) + body


def _mp3(frames, version=3):
    return _id3(frames, version) + b"\xff\xfb" + b"\0" * 32


# ---- readings ----

def test_a_wav_reads_its_info_bext_and_embedded_id3_as_one_record(tmp_path):
    path = tmp_path / "blast.wav"
    path.write_bytes(_wav(
        info={"IART": "Rich Example", "ICOP": "2026 Example Audio https://example.audio",
              "IPRD": "Weapons Vol 1", "INAM": "Blast 01", "ISFT": "Soundminer",
              "IARL": "Example Audio"},
        bext=("blast, weapon", "Example Audio", "https://example.audio"),
        id3=_id3([("TALB", "Weapons Vol 1", 1), ("TOWN", "Example Audio", 0)])))
    tags = audio_credits.read_tags(path)
    assert tags.get("artist") == "Rich Example"
    assert tags.get("copyright") == "2026 Example Audio https://example.audio"
    assert tags.get("album") == "Weapons Vol 1"
    assert tags.get("title") == "Blast 01"
    assert tags.get("software") == "Soundminer"
    assert tags.get("originator") == "Example Audio"
    assert tags.get("originator_reference") == "https://example.audio"
    assert tags.get("owner") == "Example Audio"
    assert tags.get("archive") == "Example Audio"


def test_mp3_id3_text_frames_decode_in_every_encoding_and_both_versions(tmp_path):
    v23 = tmp_path / "loop.mp3"
    v23.write_bytes(_mp3([("TPE1", "alkakrab", 1), ("TALB", "Level Loops", 0),
                          ("TYER", "2025", 0), ("TSSE", "Studio One", 2)], version=3))
    tags = audio_credits.read_tags(v23)
    assert tags.get("artist") == "alkakrab"
    assert tags.get("album") == "Level Loops"
    assert tags.get("date") == "2025"
    assert tags.get("software") == "Studio One"

    v24 = tmp_path / "boss.mp3"
    v24.write_bytes(_mp3([("TPE1", "Ålkakrab", 3), ("TDRC", "2023-04", 3),
                          ("TCOP", "2023 Someone", 3)], version=4))
    tags = audio_credits.read_tags(v24)
    assert tags.get("artist") == "Ålkakrab", "UTF-8 and syncsafe frame sizes (v2.4)"
    assert tags.get("date") == "2023-04"
    assert tags.get("copyright") == "2023 Someone"


def test_a_file_without_tags_is_recorded_with_no_attribution_fields(tmp_path):
    (tmp_path / "plain.wav").write_bytes(_wav())
    records = audio_credits.collect(tmp_path, [tmp_path / "plain.wav"])
    assert records == [{"path": "plain.wav"}], "the path alone: nothing to claim, nothing invented"


def test_the_reading_renders_toml_that_reads_back_with_the_artists_coalesced(tmp_path):
    music = tmp_path / "music"
    music.mkdir()
    (music / "2. Second.mp3").write_bytes(_mp3([("TPE1", "alkakrab", 0)]))
    (music / "1. First.mp3").write_bytes(_mp3([("TPE1", "alkakrab", 0)]))
    sfx = tmp_path / "sfx"
    sfx.mkdir()
    (sfx / "hit.wav").write_bytes(_wav(info={"IART": "Rich Example"}))
    (sfx / "untagged.wav").write_bytes(_wav())
    files = sorted(list(music.iterdir()) + list(sfx.iterdir()))
    records = audio_credits.collect(tmp_path, files)
    assert [r.get("path") for r in records] == [
        "music/1. First.mp3", "music/2. Second.mp3", "sfx/hit.wav", "sfx/untagged.wav"]

    doc = tomllib.loads(audio_credits.render_toml(records))
    assert doc.get("track") == records, "every record round-trips through the TOML"
    artists = {a["name"]: a for a in doc.get("artist", [])}
    assert set(artists) == {"alkakrab", "Rich Example"}, "untagged files name no one"
    assert artists["alkakrab"]["tracks"] == 2
    assert artists["alkakrab"]["directories"] == ["music"]
    assert artists["Rich Example"]["tracks"] == 1


def test_the_cli_scans_the_tree_and_writes_the_catalog(tmp_path):
    (tmp_path / "assets" / "music").mkdir(parents=True)
    (tmp_path / "assets" / "music" / "a.mp3").write_bytes(_mp3([("TPE1", "someone", 0)]))
    (tmp_path / "assets" / "music" / "unpacked").mkdir()
    (tmp_path / "assets" / "music" / "unpacked" / "b.mp3").write_bytes(_mp3([("TPE1", "nobody", 0)]))
    (tmp_path / "assets" / "notes.txt").write_text("not audio")
    out = tmp_path / "audio_credits.generated.toml"
    assert audio_credits.main(str(tmp_path), str(out)) == 0
    assert out.exists(), "the CLI writes the catalog"
    doc = tomllib.loads(out.read_text(encoding="utf-8"))
    assert [t["path"] for t in doc.get("track", [])] == ["assets/music/a.mp3"], \
        "audio only, repo-relative, and never the unpacked/ scratch of a provider archive"
    assert "GENERATED" in out.read_text(encoding="utf-8").splitlines()[0]
