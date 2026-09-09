"""Unit tests for scripts/credits.py — the credits page rendered from
catalog/attributions.toml, the one place every third-party source's
license and required credit line live (owner 2026-09-09: "we need to
start producing a credits page"). The page is generated, never
hand-edited: `make credits` writes docs/CREDITS.md and the audit holds
the committed page to the render."""
import importlib.util
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))


def _load():
    spec = importlib.util.spec_from_file_location(
        "credits", ROOT / "scripts" / "credits.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


credits = _load()

ATTRIBUTIONS = '''
[[source]]
key = "starmap"
name = "Deep Star Maps 2020"
author = "NASA/Goddard Space Flight Center Scientific Visualization Studio"
url = "https://svs.gsfc.nasa.gov/4851"
license = "public domain (NASA imagery); credit requested"
credit = "NASA/Goddard Space Flight Center Scientific Visualization Studio. Gaia DR2: ESA/Gaia/DPAC."
used_for = "the sky of the fixed environments"
downloads = [
  { url = "https://svs.gsfc.nasa.gov/vis/x/starmap_2020_8k_gal.exr", path = "assets/sky/starmap_2020_8k_gal.exr", sha256 = "abc" },
]

[[source]]
key = "quaternius"
name = "Quaternius Sci-Fi kits"
author = "Quaternius"
url = "https://quaternius.com"
license = "CC0 1.0"
credit = "Quaternius"
used_for = "the modular panel kits"
'''


def test_the_page_names_every_source_with_its_credit_and_license(tmp_path):
    page = credits.render(credits.load(ATTRIBUTIONS))
    assert "Deep Star Maps 2020" in page
    assert "NASA/Goddard Space Flight Center Scientific Visualization Studio. Gaia DR2: ESA/Gaia/DPAC." in page
    assert "public domain (NASA imagery); credit requested" in page
    assert "https://svs.gsfc.nasa.gov/4851" in page
    assert "Quaternius Sci-Fi kits" in page
    assert "CC0 1.0" in page
    assert "the modular panel kits" in page


def test_the_page_is_ordered_by_name_and_marked_generated():
    page = credits.render(credits.load(ATTRIBUTIONS))
    assert page.index("Deep Star Maps 2020") < page.index("Quaternius Sci-Fi kits")
    assert "GENERATED" in page.splitlines()[0], "the first line says it is generated"


def test_a_source_without_a_credit_line_is_a_hole_the_render_refuses():
    """A third-party source we cannot credit is a source we cannot ship."""
    broken = ATTRIBUTIONS.replace('credit = "Quaternius"\n', "")
    try:
        credits.render(credits.load(broken))
    except ValueError as e:
        assert "quaternius" in str(e)
    else:
        raise AssertionError("a source without a credit line rendered")


def test_the_cli_writes_the_page(tmp_path):
    src = tmp_path / "attributions.toml"
    src.write_text(ATTRIBUTIONS)
    out = tmp_path / "CREDITS.md"
    assert credits.main(str(src), str(out)) == 0
    assert "Deep Star Maps 2020" in out.read_text()
