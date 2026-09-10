"""The audio credits — the first reading of the ingestion pipeline: every
sound file in assets/ says who made it, in its own tags. Pure Python,
no ffprobe.

    python3 scripts/audio_credits.py <repo root> <out.toml>

Provider audio carries its own provenance: RIFF INFO and Broadcast Wave
(bext) chunks in the wavs, ID3v2 frames in the mp3s (and inside the wavs
that embed them) — artist, album, copyright line, originator. `make
credits` reads them into catalog/audio_credits.generated.toml before
anything installs (owner 2026-09-09: "if we add more audio files, we
will want to have that new data carried forward"), and the audit joins
that reading to catalog/attributions.toml: a track whose tags name
someone no claiming source credits fails the stage, naming the artist
and the file. Nothing is transcribed by hand twice: the files are the
oracle, this reading is the record, the attributions are the policy.
"""
import os
import struct
import sys

AUDIO_SUFFIXES = (".wav", ".mp3", ".ogg", ".flac")
# A provider archive's extraction scratch beside it — never a source.
SKIP_DIRS = ("unpacked",)

# Container tag ids -> the record's field names. The first reading of a
# field wins (INFO before bext before an embedded ID3 tag, in chunk order).
INFO_FIELDS = {
    "IART": "artist", "IPRD": "album", "INAM": "title", "ICOP": "copyright",
    "ICRD": "date", "ISFT": "software", "IARL": "archive",
}
ID3_FIELDS = {
    "TPE1": "artist", "TALB": "album", "TIT2": "title", "TCOP": "copyright",
    "TYER": "date", "TDRC": "date", "TSSE": "software", "TENC": "software",
    "TOWN": "owner", "TPUB": "publisher",
}
ID3_CODECS = {0: "latin-1", 1: "utf-16", 2: "utf-16-be", 3: "utf-8"}


def _text(raw, codec="utf-8"):
    """A tag's text: decoded, NUL-terminated, multi-valued (ID3v2.4)
    values joined."""
    values = [v.strip() for v in raw.decode(codec, errors="replace").split("\0") if v.strip()]
    return " / ".join(values)


def _merge(tags, more):
    for field, value in more.items():
        tags.setdefault(field, value)


def _syncsafe(raw):
    return (raw[0] << 21) | (raw[1] << 14) | (raw[2] << 7) | raw[3]


def _info_tags(payload):
    """The sub-chunks of a LIST/INFO chunk (id, size, NUL-terminated text)."""
    tags, pos = {}, 0
    while pos + 8 <= len(payload):
        cid = payload[pos:pos + 4].decode("ascii", errors="replace")
        size = struct.unpack("<I", payload[pos + 4:pos + 8])[0]
        field = INFO_FIELDS.get(cid)
        if field:
            value = _text(payload[pos + 8:pos + 8 + size])
            if value:
                tags.setdefault(field, value)
        pos += 8 + size + (size % 2)
    return tags


def _bext_tags(payload):
    """EBU 3285 Broadcast Wave: the originator and its reference (fixed
    fields after the 256-byte description)."""
    tags = {}
    for field, start, length in (("originator", 256, 32), ("originator_reference", 288, 32)):
        value = _text(payload[start:start + length])
        if value:
            tags[field] = value
    return tags


def riff_tags(handle):
    """INFO, bext and embedded-ID3 tags of an open RIFF/WAVE file, walking
    chunks by size so the data chunk is skipped, never read."""
    tags = {}
    header = handle.read(12)
    if len(header) < 12 or header[:4] != b"RIFF" or header[8:12] != b"WAVE":
        return tags
    while True:
        head = handle.read(8)
        if len(head) < 8:
            break
        cid, size = head[:4], struct.unpack("<I", head[4:8])[0]
        if cid == b"LIST":
            payload = handle.read(size)
            if payload[:4] == b"INFO":
                _merge(tags, _info_tags(payload[4:]))
        elif cid == b"bext":
            _merge(tags, _bext_tags(handle.read(size)))
        elif cid in (b"id3 ", b"ID3 "):
            _merge(tags, id3_tags(handle.read(size)))
        else:
            handle.seek(size, os.SEEK_CUR)
        if size % 2:
            handle.seek(1, os.SEEK_CUR)
    return tags


def _frame_text(payload):
    """A text frame's value: the encoding byte, then the text."""
    codec = ID3_CODECS.get(payload[0]) if payload else None
    return _text(payload[1:], codec) if codec else ""


def id3_tags(data):
    """The text frames of an ID3v2.3/2.4 tag (bytes starting at "ID3")."""
    tags = {}
    if len(data) < 10 or data[:3] != b"ID3":
        return tags
    version, flags = data[3], data[5]
    end = min(10 + _syncsafe(data[6:10]), len(data))
    pos = 10
    if flags & 0x40:  # extended header: syncsafe and self-inclusive in 2.4, plain and exclusive in 2.3
        pos += _syncsafe(data[10:14]) if version >= 4 else 4 + struct.unpack(">I", data[10:14])[0]
    while pos + 10 <= end:
        fid = data[pos:pos + 4]
        if fid[0] == 0:
            break  # padding
        raw_size = data[pos + 4:pos + 8]
        size = _syncsafe(raw_size) if version >= 4 else struct.unpack(">I", raw_size)[0]
        field = ID3_FIELDS.get(fid.decode("ascii", errors="replace"))
        if field:
            value = _frame_text(data[pos + 10:pos + 10 + size])
            if value:
                tags.setdefault(field, value)
        pos += 10 + size
    return tags


def read_tags(path):
    """The provenance tags of one audio file, by container."""
    with open(path, "rb") as handle:
        magic = handle.read(4)
        handle.seek(0)
        if magic == b"RIFF":
            return riff_tags(handle)
        if magic[:3] == b"ID3":
            head = handle.read(10)
            return id3_tags(head + handle.read(_syncsafe(head[6:10])))
    return {}


def audio_files(root):
    """Every audio file under <root>/assets, provider scratch skipped."""
    paths = []
    for dirpath, dirnames, filenames in os.walk(os.path.join(root, "assets")):
        dirnames[:] = sorted(d for d in dirnames if d not in SKIP_DIRS)
        paths += [os.path.join(dirpath, name) for name in sorted(filenames)
                  if name.lower().endswith(AUDIO_SUFFIXES)]
    return paths


def collect(root, paths):
    """One record per file (root-relative path plus its tags), sorted."""
    records = []
    for path in paths:
        record = {"path": os.path.relpath(path, root).replace(os.sep, "/")}
        record.update(read_tags(path))
        records.append(record)
    records.sort(key=lambda r: r["path"])
    return records


def toml_str(s):
    return '"' + str(s).replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'


def render_toml(records):
    """The generated catalog: every track, then the artists coalesced."""
    lines = [
        "# GENERATED by scripts/audio_credits.py (make credits) — do not edit.",
        "# Every audio file under assets/ with the credits its container",
        "# carries (RIFF INFO / bext, ID3v2). The audit joins this reading to",
        "# catalog/attributions.toml: a track naming an artist no claiming",
        "# source credits fails the stage.",
        "",
    ]
    for record in records:
        lines.append("[[track]]")
        lines += [f"{key} = {toml_str(value)}" for key, value in record.items()]
        lines.append("")
    artists = {}
    for record in records:
        name = record.get("artist")
        if name:
            entry = artists.setdefault(name, {"tracks": 0, "directories": set()})
            entry["tracks"] += 1
            entry["directories"].add(os.path.dirname(record["path"]))
    for name in sorted(artists, key=str.lower):
        entry = artists[name]
        lines += [
            "[[artist]]",
            f"name = {toml_str(name)}",
            f"tracks = {entry['tracks']}",
            "directories = [" + ", ".join(toml_str(d) for d in sorted(entry["directories"])) + "]",
            "",
        ]
    return "\n".join(lines)


def main(root, out_path):
    records = collect(root, audio_files(root))
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(render_toml(records))
    artists = sorted({r["artist"] for r in records if r.get("artist")}, key=str.lower)
    print(f"audio-credits: {len(records)} tracks, {len(artists)} tagged artists "
          f"({', '.join(artists)}) -> {out_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:3]))
