"""The ground-truth reading (`make ground-truth`, audited by `make test-assets`).

scripts/ground-truth.py reads the review docs' claims about the tree and
checks them against `git ls-files`. These tests build a small tracked
repo and hold the reading to its contract: a backticked repo path
anywhere in the docs is a claim (found, found by suffix when it is
written relative to its crate, resolved when written relative to the doc,
a product when the path is gitignored, missing otherwise); a symbol is a
claim only where the canonical-sites table declares it, and is found when
a Rust, Python, or GDScript definition exists, with `A::b` needing both
halves; globs and URLs are not claims; the TOML lists every claim and the
exit status is the gate.

The last test runs the reading over this repo's own docs/review: a doc
that names a symbol the code no longer has fails here, not in a review.
"""
import importlib.util
import subprocess
import sys
from pathlib import Path

import pytest

try:
    import tomllib
except ImportError:  # the audit venv (python 3.9) carries tomli
    import tomli as tomllib

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))


def _load():
    spec = importlib.util.spec_from_file_location(
        "ground_truth", ROOT / "scripts" / "ground-truth.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


gt = _load()

GROUND_TRUTH = """# Ground truth

The registry is `seed::salt` in `rust/logic/src/seed.rs`; the old `HULL_SALT`
is gone. Views live under `nodes/views/`; products land in `out/metrics/`.

## Canonical sites

| Pattern | What it is | Canonical site to verify |
|---|---|---|
| Salted entropy | one draw | `Seed::for_level`, `seed::salt` |
| Retained graph | opaque | `LevelGraph::visible_from` |
| Phantom | renamed away | `RoomBuild` |
| Half a path | wrong method | `LevelGraph::nope` |
| Stage | make | `Makefile`, `scripts/tool.py` |
| Shell | gdscript | `Thing::go` |
"""

BRIEF = """# A brief

See `scripts/plan_apply.py` and `scripts/tool.py`; products under
`out/metrics/x.toml` and the installed `addons/`; globs like
`catalog/*.generated.toml` and URLs like `https://example.com/a/b` are not
claims; `docs/review/` is a directory; `../ground_truth.md` is next door.
"""


def _write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def _git(root, *args):
    subprocess.run(["git", *args], cwd=root, check=True, capture_output=True)


@pytest.fixture
def repo(tmp_path):
    root = tmp_path
    _git(root, "init", "-q")
    _write(root / ".gitignore", "/out/\naddons/*\n")
    _write(root / "rust/logic/src/seed.rs",
           "pub struct Seed(u64);\n"
           "impl Seed {\n"
           "    pub fn for_level(self, level: u32) -> Self { self }\n"
           "}\n"
           "pub mod salt {\n"
           "    pub const HULL: u64 = 1;\n"
           "}\n")
    _write(root / "rust/logic/src/graph/mod.rs",
           "pub struct LevelGraph;\n"
           "impl LevelGraph { pub fn visible_from(&self) {} }\n")
    _write(root / "rust/nodes/src/nodes/views/view.rs", "")
    _write(root / "scripts/tool.py", "def run():\n    pass\n")
    _write(root / "godot/thing.gd", "class_name Thing\nfunc go():\n\tpass\n")
    _write(root / "Makefile", "all:\n")
    _write(root / "docs/review/ground_truth.md", GROUND_TRUTH)
    _write(root / "docs/review/principles/brief.md", BRIEF)
    _write(root / "out/metrics/x.toml", "")
    _write(root / "addons/installed.glb", "")
    _git(root, "add", "-A")
    return root


def _by_token(results):
    return {r.token: r for r in results}


def test_path_claims_are_checked_against_the_tracked_tree(repo):
    got = _by_token(gt.census(repo, repo / "docs/review"))
    assert got["rust/logic/src/seed.rs"].status == "found"
    assert got["scripts/tool.py"].status == "found"
    assert got["Makefile"].status == "found"
    assert got["docs/review/"].status == "found"
    assert got["scripts/plan_apply.py"].status == "missing"


def test_a_path_written_relative_to_its_crate_is_found_by_suffix(repo):
    got = _by_token(gt.census(repo, repo / "docs/review"))
    assert got["nodes/views/"].status == "found"
    assert got["nodes/views/"].where == "rust/nodes/src/nodes/views/"


def test_a_path_written_relative_to_the_doc_is_resolved(repo):
    got = _by_token(gt.census(repo, repo / "docs/review"))
    assert "../ground_truth.md" not in got
    assert got["docs/review/ground_truth.md"].status == "found"
    assert got["docs/review/ground_truth.md"].doc == "docs/review/principles/brief.md"


def test_gitignored_paths_are_products_not_defects(repo):
    got = _by_token(gt.census(repo, repo / "docs/review"))
    assert got["out/metrics/"].status == "product"
    assert got["out/metrics/x.toml"].status == "product"
    assert got["addons/"].status == "product", "a directory the pipeline fills is a product"


def test_globs_urls_extensions_and_markers_are_not_claims(repo):
    _write(repo / "docs/review/principles/extra.md",
           "Files ending in `.py` and `.rs`, `//` comment markers, `<key>.toml`\n"
           "placeholders, and `$(CURDIR)` are not paths; `.gitignore` at the root is.\n")
    _git(repo, "add", "-A")
    got = _by_token(gt.census(repo, repo / "docs/review"))
    for not_a_claim in ("catalog/*.generated.toml", "https://example.com/a/b",
                        ".py", ".rs", "//", "<key>.toml", "$(CURDIR)"):
        assert not_a_claim not in got, not_a_claim
    assert got[".gitignore"].status == "found"


def test_symbols_are_claims_only_from_the_canonical_sites_table(repo):
    got = _by_token(gt.census(repo, repo / "docs/review"))
    assert "HULL_SALT" not in got, "prose mentions are not symbol claims"
    assert got["Seed::for_level"].status == "found"
    assert got["Seed::for_level"].where == "rust/logic/src/seed.rs:3"
    assert got["seed::salt"].status == "found"
    assert got["LevelGraph::visible_from"].status == "found"
    assert got["LevelGraph::visible_from"].where == "rust/logic/src/graph/mod.rs:2"
    assert got["Thing::go"].status == "found"


def test_a_symbol_the_code_no_longer_has_is_missing(repo):
    got = _by_token(gt.census(repo, repo / "docs/review"))
    assert got["RoomBuild"].status == "missing"
    assert got["LevelGraph::nope"].status == "missing", "both halves of A::b must exist"


def test_every_claim_carries_its_site(repo):
    for r in gt.census(repo, repo / "docs/review"):
        assert r.doc.startswith("docs/review/"), r
        assert r.line >= 1, r
        assert r.kind in ("path", "symbol"), r


def test_the_reading_is_toml_and_the_exit_status_is_the_gate(repo, capsys):
    out = repo / "out/metrics/ground_truth.toml"
    assert gt.main(str(repo), str(repo / "docs/review"), str(out)) == 1
    doc = tomllib.loads(out.read_text())
    rows = doc["claim"]
    assert doc["summary"]["claims"] == len(rows)
    assert doc["summary"]["missing"] == 3
    assert {r["token"] for r in rows if r["status"] == "missing"} == {
        "scripts/plan_apply.py", "RoomBuild", "LevelGraph::nope"}
    assert rows == sorted(rows, key=lambda r: (r["doc"], r["line"], r["token"]))
    assert "RoomBuild" in capsys.readouterr().out


def test_docs_that_describe_the_code_pass(repo):
    fixed = GROUND_TRUTH.replace("| Phantom | renamed away | `RoomBuild` |\n", "") \
                        .replace("| Half a path | wrong method | `LevelGraph::nope` |\n", "")
    _write(repo / "docs/review/ground_truth.md", fixed)
    _write(repo / "docs/review/principles/brief.md",
           BRIEF.replace("`scripts/plan_apply.py` and ", ""))
    out = repo / "out/metrics/ground_truth.toml"
    assert gt.main(str(repo), str(repo / "docs/review"), str(out)) == 0


def test_this_repos_review_docs_describe_its_code():
    missing = [r for r in gt.census(ROOT, ROOT / "docs/review") if r.status == "missing"]
    assert not missing, "\n".join(f"{r.doc}:{r.line} `{r.token}`" for r in missing)
