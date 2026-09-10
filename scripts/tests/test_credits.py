"""Unit tests for scripts/credits.py — the credits page rendered from
catalog/attributions.toml, the one place every third-party source's
license and required credit line live (owner 2026-09-09: "we need to
start producing a credits page"). The page is generated, never
hand-edited: `make credits` writes docs/CREDITS.md and the audit holds
the committed page to the render. The same catalog feeds the in-game
crawl (void_logic::credits): every shipped source carries one short
`contribution` line for it, and the render refuses a shipped source
without one — the two views share one schema."""
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
contribution = "Star field"
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
contribution = "Modular kits"
'''

# The full page (2026-09-09): the team leads it, and a pack in the
# repository that nothing installs from (ships = false) is credited apart
# from the shipped work — on the page, off the in-game crawl, so it needs
# no crawl line.
TEAM = '''
[[team]]
name = "A. Director"
role = "Design, direction"

[[team]]
name = "B. Builder"
role = "Code"
'''

PARKED = '''
[[source]]
key = "parked"
name = "Parked textures"
author = "Parked"
license = "free for games; credit appreciated"
credit = "Parked"
used_for = "nothing yet — in the repository, not installed"
ships = false
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


def test_the_team_leads_the_page_and_the_third_party_work_follows():
    page = credits.render(credits.load(TEAM + ATTRIBUTIONS))
    assert "A. Director" in page and "B. Builder" in page, "the team is on the page"
    assert page.index("A. Director") < page.index("B. Builder") < page.index("Deep Star Maps 2020"), \
        "the team, in declared order, before any third-party source"
    assert "Design, direction" in page and "Code" in page, "each member with their role"


def test_a_pack_nothing_installs_from_is_credited_apart_from_the_shipped_work():
    page = credits.render(credits.load(TEAM + ATTRIBUTIONS + PARKED))
    assert "Parked textures" in page, "still credited — it is in the repository"
    assert page.index("Quaternius Sci-Fi kits") < page.index("Parked textures"), \
        "set apart AFTER every shipped source, not sorted among them"
    assert "not shipped" in page.lower(), "and said so"


def test_a_team_entry_without_a_role_is_a_hole_the_render_refuses():
    broken = (TEAM + ATTRIBUTIONS).replace('role = "Code"\n', "")
    try:
        credits.render(credits.load(broken))
    except ValueError as e:
        assert "B. Builder" in str(e)
    else:
        raise AssertionError("a team entry without a role rendered")


def test_a_shipped_source_without_a_crawl_line_is_a_hole_the_render_refuses():
    """The in-game crawl shows one line per source above its author; a
    shipped source with nothing to say there cannot ship. A parked pack
    is off the crawl and needs none."""
    broken = ATTRIBUTIONS.replace('contribution = "Modular kits"\n', "")
    try:
        credits.render(credits.load(broken))
    except ValueError as e:
        assert "quaternius" in str(e) and "contribution" in str(e)
    else:
        raise AssertionError("a shipped source without a crawl line rendered")
    credits.render(credits.load(ATTRIBUTIONS + PARKED))  # parked: fine without one


def test_the_page_says_what_the_crawl_will():
    page = credits.render(credits.load(ATTRIBUTIONS))
    assert "Star field" in page and "Modular kits" in page, "the crawl line is on the page too"
