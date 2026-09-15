"""Unit tests for scripts/log-histogram.py — the synopsis of a door's
log: every ERROR and WARNING a step printed, condensed into a histogram
of (message, source location, asset being processed) with counts, and a
per-asset tally — "22k errors of this type from this file, investigate
that first" (owner 2026-09-07). Fixtures are log text written the way
Godot and the Blender converters write it, ANSI color included."""
import importlib.util
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))


def _load_histogram():
    spec = importlib.util.spec_from_file_location(
        "log_histogram", ROOT / "scripts" / "log-histogram.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


histogram = _load_histogram()
tomllib = histogram.tomllib

ESC = "\x1b"
GODOT_LOG = (
    f"[  85% ] {ESC}[90m{ESC}[1mreimport{ESC}[22m | apartment.glb{ESC}[39m{ESC}[0m\n"
    "[   0% ] import | Started Import Scene (104 steps)\n"
    f"[  85% ] {ESC}[90m{ESC}[1mreimport{ESC}[22m | hill_house.glb{ESC}[39m{ESC}[0m\n"
    "[   0% ] import | Importing Scene...\n"
    'ERROR: Condition "surfaces.size() == RenderingServer::MAX_MESH_SURFACES" is true.\n'
    "   at: add_surface (scene/resources/mesh.cpp:1782)\n"
    'ERROR: Condition "surfaces.size() == RenderingServer::MAX_MESH_SURFACES" is true.\n'
    "   at: add_surface (scene/resources/mesh.cpp:1782)\n"
    "[  85% ] reimport | alien_troop_01.glb\n"
    "WARNING: glTF: Image index '0' with the name 'Emissive' resolved to "
    "res://addons/enemies/alien_troop_01_Emissive.jpg couldn't be imported. "
    "It will be loaded directly instead, uncompressed.\n"
    "     at: _parse_image_save_image (modules/gltf/gltf_document.cpp:2265)\n"
    "ERROR: Failed loading resource: res://addons/enemies/alien_troop_01_Normal.jpg. "
    "The file doesn't seem to exist.\n"
    "   at: _load (core/io/resource_loader.cpp:283)\n"
    "ERROR: Failed loading resource: res://addons/enemies/alien_troop_01_Base.jpg. "
    "The file doesn't seem to exist.\n"
    "   at: _load (core/io/resource_loader.cpp:283)\n"
    f"{ESC}[92m[ DONE ]{ESC}[39m {ESC}[1mimport{ESC}[22m\n"
    "WARNING: ObjectDB instances leaked at exit (run with --verbose for details).\n"
)

CONVERT_LOG = (
    "convert-environment: loading cached scene...\n"
    "plan_apply: WARNING no plan for material 'ID_Foo'\n"
    "plan_apply: WARNING no plan for material 'ID_Bar'\n"
    "Warning: Ignoring unknown token 'usemap' in file\n"
    "convert-environment: exporting 3889899 tris (no decimation)...\n"
)


@pytest.fixture
def synopsis(tmp_path):
    godot = tmp_path / "assets-import-pass1.log"
    godot.write_text(GODOT_LOG)
    convert = tmp_path / "convert-hill_house.log"
    convert.write_text(CONVERT_LOG)
    out = tmp_path / "metrics" / "log_assets-import.toml"
    histogram.main("assets-import", str(out), str(godot), str(convert))
    with open(out, "rb") as f:
        return tomllib.load(f)


def test_the_summary_counts_every_error_and_warning(synopsis):
    summary = synopsis["summary"]
    assert summary["step"] == "assets-import"
    assert summary["errors"] == 4, "two surface drops, two failed loads"
    assert summary["warnings"] == 5, "the glTF image, the leak, two plan warnings, Blender's token"
    assert summary["distinct_errors"] == 2
    assert summary["distinct_warnings"] == 4
    assert summary["logs"] == ["assets-import-pass1.log", "convert-hill_house.log"]


def test_errors_are_a_histogram_by_message_source_and_asset(synopsis):
    """Identical messages from one source location while one asset was
    being processed are one row with a count; rows come most frequent
    first. The asset is the nearest preceding progress line naming a
    file, ANSI color stripped."""
    rows = synopsis["error"]
    assert [r["count"] for r in rows] == sorted((r["count"] for r in rows), reverse=True)
    top = rows[0]
    assert top["message"] == 'Condition "surfaces.size() == RenderingServer::MAX_MESH_SURFACES" is true.'
    assert top["at"] == "add_surface (scene/resources/mesh.cpp:1782)"
    assert top["asset"] == "hill_house.glb"
    assert top["count"] == 2
    assert top["first_line"] == 5
    assert top["log"] == "assets-import-pass1.log"


def test_resource_paths_and_quoted_names_group_into_one_row(synopsis):
    """A message that names a resource or a material is one kind of
    failure however many files it names: the res:// path and the quoted
    name are folded for grouping, and the first raw text is kept as the
    example."""
    loads = [r for r in synopsis["error"] if r["message"].startswith("Failed loading resource")]
    assert len(loads) == 1
    assert loads[0]["count"] == 2
    assert loads[0]["asset"] == "alien_troop_01.glb"
    assert loads[0]["message"] == "Failed loading resource: res://*. The file doesn't seem to exist."
    assert loads[0]["example"].startswith(
        "Failed loading resource: res://addons/enemies/alien_troop_01_Normal.jpg.")
    plans = [r for r in synopsis["warning"] if "no plan for material" in r["message"]]
    assert len(plans) == 1
    assert plans[0]["count"] == 2
    assert plans[0]["message"] == "no plan for material '*'"
    assert plans[0]["example"] == "no plan for material 'ID_Foo'"


def test_a_line_outside_any_asset_is_charged_to_its_log(synopsis):
    """Godot's leak warning fires at exit, after the last asset's DONE:
    it is charged to the log file, as is every line of a converter log,
    which is one pack's own record already."""
    leak = next(r for r in synopsis["warning"] if r["message"].startswith("ObjectDB"))
    assert leak["asset"] == "assets-import-pass1.log"
    assert leak["at"] == ""
    token = next(r for r in synopsis["warning"] if r["message"].startswith("Ignoring unknown token"))
    assert token["asset"] == "convert-hill_house.log"
    assert token["log"] == "convert-hill_house.log"


def test_the_per_asset_tally_reads_most_troubled_first(synopsis):
    """Most errors first, then most warnings; an asset's import ends at
    Godot's DONE line, so what fires after (the leak at exit) is the
    log's own."""
    tally = synopsis["by_asset"]
    assert [(r["asset"], r["errors"], r["warnings"]) for r in tally] == [
        ("alien_troop_01.glb", 2, 1),
        ("hill_house.glb", 2, 0),
        ("convert-hill_house.log", 0, 3),
        ("assets-import-pass1.log", 0, 1),
    ]
    assert "apartment.glb" not in [r["asset"] for r in tally], "a clean asset has no row"


def test_a_python_traceback_counts_as_an_error(tmp_path):
    """A Blender-hosted script that dies prints a traceback, not an
    "ERROR:" line — the .max extraction failed for days that way behind
    a summary grep (2026-09-07). The traceback is one error whose
    message is its last line, the exception; the frames between are not
    reports of their own. A caught exception the tool prints by itself
    ("TypeError: ...") counts too."""
    log = tmp_path / "max-villa.log"
    log.write_text(
        "extract-max: importing scene (several minutes)...\n"
        "Traceback (most recent call last):\n"
        '  File "/repo/scripts/extract-max-materials.py", line 37, in <module>\n'
        "    bpy.ops.import_scene.max(filepath=in_path)\n"
        "AttributeError: Calling operator \"bpy.ops.import_scene.max\" error, could not be found\n"
        "\tTypeError: bpy_struct: item.attr = val: Object.parent ID type does not support assignment to itself 'mountain house'\n"
        "Error: script failed, file: '/repo/scripts/extract-max-materials.py', exiting.\n"
    )
    out = tmp_path / "metrics" / "log_max.toml"
    histogram.main("max", str(out), str(log))
    with open(out, "rb") as f:
        doc = tomllib.load(f)
    messages = [r["message"] for r in doc["error"]]
    assert doc["summary"]["errors"] == 3
    assert 'Calling operator "bpy.ops.import_scene.max" error, could not be found' in messages, \
        "the traceback's exception line is the error"
    assert "bpy_struct: item.attr = val: Object.parent ID type does not support assignment to itself '*'" in messages, \
        "a printed exception counts, its quoted name folded"
    assert "script failed, file: '*', exiting." in messages
    assert not any("File " in m or "bpy.ops.import_scene.max(filepath" in m for m in messages), \
        "traceback frames are not errors of their own"


def test_a_clean_log_writes_an_empty_synopsis(tmp_path):
    clean = tmp_path / "quiet.log"
    clean.write_text("[  85% ] reimport | apartment.glb\n[ DONE ] import\n")
    out = tmp_path / "metrics" / "log_quiet.toml"
    histogram.main("quiet", str(out), str(clean))
    with open(out, "rb") as f:
        doc = tomllib.load(f)
    assert doc["summary"]["errors"] == 0
    assert doc["summary"]["warnings"] == 0
    assert doc.get("error", []) == []
    assert doc.get("by_asset", []) == []


def test_no_logs_is_a_refusal_not_a_clean_bill(tmp_path):
    """A histogram of nothing must not read as zero errors."""
    with pytest.raises(SystemExit):
        histogram.main("empty", str(tmp_path / "metrics" / "log_empty.toml"))
    assert not (tmp_path / "metrics" / "log_empty.toml").exists()
