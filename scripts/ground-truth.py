"""The ground-truth reading (`make ground-truth`): do the review docs still describe the code?

    python3 scripts/ground-truth.py <repo-root> <docs-dir> <out.toml>

Every backticked repo path in the review docs and every symbol in the
canonical-sites table of ground_truth.md is a claim about the tracked
tree. Documents drift from code; a reviewer told to hold a diff to a
symbol that was renamed months ago judges it against a phantom. This
tool reads the claims and checks each against `git ls-files`: a path is
found (exactly, or by suffix when the doc wrote it relative to its
crate, `where` naming the real path), a product (a gitignored path the
pipeline makes), or missing; a symbol is found when a definition of it
exists in a tracked Rust, Python, or GDScript file, with `A::b` needing
both halves and a module counted as defined by its file. Globs, URLs,
and placeholders are not claims. Symbols in prose are not claims either:
the docs name engine and crate symbols the repo does not define, so the
table is the declared set and prose is the reviewer's to verify.

The result is written as TOML under out/metrics/ and the exit status is
the gate: any missing claim fails. The invoker runs it before a review,
like the other gates. Definitions are found by regex over the tracked
sources, the ctags approach; an LSP would be the upgrade if the regexes
ever prove too coarse.
"""
import os
import re
import subprocess
import sys
from collections import namedtuple

from toml_out import toml_str

Claim = namedtuple("Claim", "doc line token kind")
Result = namedtuple("Result", "doc line token kind status where")

BACKTICK = re.compile(r"`([^`\n]+)`")
LINE_SUFFIX = re.compile(r":\d+(-\d+)?$")
SYMBOL = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*(::[A-Za-z_][A-Za-z0-9_]*)*$")
NOT_A_CLAIM = ("://", "*", "<", ">", "{", "$", " ", "?", "[")
PATH_EXTENSIONS = (".md", ".rs", ".py", ".toml", ".sh", ".gd", ".tscn", ".tres",
                   ".cfg", ".json", ".glb", ".txt", ".yml", ".yaml")
SITE_COLUMN = "canonical site"


class Tree:
    """The tracked tree, read once: paths, directories, sources, ignore rules."""

    # How a definition of {name} reads in each source language. Rust items
    # may follow an `impl X {` on the same line; Python and GDScript
    # definitions start their line.
    DEFINITIONS = {
        ".rs": (r"(?:^|[{;])\s*(?:pub(?:\([^)]*\))?\s+)?(?:(?:unsafe|async|const)\s+)*"
                r"(?:struct|enum|trait|type|fn|const|static|mod|union)\s+{name}\b"
                r"|(?:^|[{;])\s*macro_rules!\s*{name}\b"),
        ".py": r"^\s*(?:async\s+)?(?:def|class)\s+{name}\b",
        ".gd": r"^\s*(?:static\s+)?(?:func|class_name|class|var|const|signal|enum)\s+{name}\b",
    }

    def __init__(self, root):
        self.root = root
        listing = subprocess.run(["git", "ls-files", "-z"], cwd=root, check=True,
                                 capture_output=True, text=True).stdout
        self.files = sorted(p for p in listing.split("\0") if p)
        self.dirs = set()
        for path in self.files:
            parts = path.split("/")[:-1]
            for depth in range(1, len(parts) + 1):
                self.dirs.add("/".join(parts[:depth]))
        self.root_files = {p for p in self.files if "/" not in p}
        self._sources = {}

    def source(self, path):
        if path not in self._sources:
            with open(os.path.join(self.root, path), encoding="utf-8", errors="replace") as f:
                self._sources[path] = f.read()
        return self._sources[path]

    def ignored(self, paths):
        """The subset of `paths` git ignores: products the pipeline makes.

        A directory claim (`out/metrics/`) is a product when a file inside
        it would be ignored, which also covers `dir/*` style rules. Each
        path is asked on its own, so a path git cannot place (outside the
        repo, malformed) is simply not a product rather than poisoning
        the batch."""
        products = set()
        for path in paths:
            probe = path + "x" if path.endswith("/") else path
            run = subprocess.run(["git", "check-ignore", "-q", "--", probe],
                                 cwd=self.root, capture_output=True, text=True)
            if run.returncode == 0:
                products.add(path)
        return products

    def find_path(self, token):
        """(status, where) for a path claim, before the ignore check."""
        is_dir = token.endswith("/")
        name = token.rstrip("/")
        if is_dir:
            if name in self.dirs:
                return "found", name + "/"
            hits = sorted(d for d in self.dirs if d.endswith("/" + name))
            if hits:
                return "found", hits[0] + "/"
            return None, ""
        if name in self.files:
            return "found", name
        if name in self.dirs:
            return "found", name + "/"
        hits = sorted(p for p in self.files if p.endswith("/" + name))
        if hits:
            return "found", hits[0]
        return None, ""

    def find_definition(self, name):
        """path:line of the first definition of `name`, or a module file for it."""
        escaped = re.escape(name)
        for path in self.files:
            ext = os.path.splitext(path)[1]
            if ext not in self.DEFINITIONS:
                continue
            pattern = re.compile(self.DEFINITIONS[ext].replace("{name}", escaped), re.MULTILINE)
            match = pattern.search(self.source(path))
            if match:
                # The match may open on the previous line (`impl X {`); the
                # name itself is on the definition line.
                line = self.source(path).count("\n", 0, match.end()) + 1
                return f"{path}:{line}"
        for path in self.files:
            stem, ext = os.path.splitext(path)
            if ext in self.DEFINITIONS and (os.path.basename(stem) == name
                                            or stem.endswith(f"/{name}/mod")):
                return path
        return None


def _path_token(raw, tree):
    """The path a backticked token claims, or None when it is not a path.

    A bare extension (`.py`), a comment marker (`//`), a glob, a URL, or
    a placeholder is not a claim; a root file (`Makefile`, `.gitignore`)
    and anything with a directory separator or a file extension is."""
    if any(marker in raw for marker in NOT_A_CLAIM):
        return None
    if not re.search(r"[A-Za-z0-9]", raw):
        return None
    token = LINE_SUFFIX.sub("", raw)
    if token in tree.root_files:
        return token
    name = token.rstrip("/").rsplit("/", 1)[-1]
    if name in PATH_EXTENSIONS:
        return None
    if "/" in token or token.endswith(PATH_EXTENSIONS):
        return token
    return None


def _site_column(cells):
    for index, cell in enumerate(cells):
        if SITE_COLUMN in cell.lower():
            return index
    return None


def find_claims(docs_dir, tree=None):
    """Every (doc, line, token, kind) the review docs assert about the tree.

    A path written relative to the doc (`../doctrine.md`) is resolved
    against the doc's directory and reported resolved."""
    tree = tree or Tree(_root_of(docs_dir))
    claims, seen = [], set()

    def add(doc, line, token, kind):
        key = (doc, line, token)
        if key not in seen:
            seen.add(key)
            claims.append(Claim(doc, line, token, kind))

    for md in sorted(_docs(docs_dir)):
        doc = os.path.relpath(md, tree.root)
        with open(md, encoding="utf-8") as f:
            lines = f.read().split("\n")
        site_column = None
        for number, text in enumerate(lines, start=1):
            for raw in BACKTICK.findall(text):
                token = _path_token(raw, tree)
                if token:
                    add(doc, number, _resolve(token, doc), "path")
            if not text.lstrip().startswith("|"):
                site_column = None
                continue
            cells = [c.strip() for c in text.strip().strip("|").split("|")]
            if site_column is None:
                site_column = _site_column(cells)
                continue
            if all(set(c) <= set("-: ") for c in cells):
                continue
            if site_column < len(cells):
                for raw in BACKTICK.findall(cells[site_column]):
                    if _path_token(raw, tree) is None and SYMBOL.match(raw.rstrip("()")):
                        add(doc, number, raw.rstrip("()"), "symbol")
    return claims


def _resolve(token, doc):
    if not token.startswith(("./", "../")):
        return token
    resolved = os.path.normpath(os.path.join(os.path.dirname(doc), token))
    return resolved + "/" if token.endswith("/") else resolved


def _docs(docs_dir):
    for dirpath, _, filenames in os.walk(docs_dir):
        for name in filenames:
            if name.endswith(".md"):
                yield os.path.join(dirpath, name)


def _root_of(docs_dir):
    return subprocess.run(["git", "rev-parse", "--show-toplevel"], cwd=docs_dir, check=True,
                          capture_output=True, text=True).stdout.strip()


def check_symbol(token, tree):
    where = None
    for part in token.split("::"):
        where = tree.find_definition(part)
        if where is None:
            return "missing", ""
    return "found", where


def census(root, docs_dir):
    """Every claim with its status: found, product, or missing."""
    tree = Tree(str(root))
    claims = find_claims(str(docs_dir), tree)
    resolved = {}
    unresolved = []
    for claim in claims:
        if claim.kind == "path":
            status, where = tree.find_path(claim.token)
            if status is None:
                unresolved.append(claim.token)
            resolved[claim] = (status, where)
        else:
            resolved[claim] = check_symbol(claim.token, tree)
    products = tree.ignored(sorted(set(unresolved)))
    results = []
    for claim, (status, where) in resolved.items():
        if status is None:
            status = "product" if claim.token in products else "missing"
        results.append(Result(claim.doc, claim.line, claim.token, claim.kind, status, where))
    return sorted(results, key=lambda r: (r.doc, r.line, r.token))


def render_toml(results):
    docs = sorted({r.doc for r in results})
    counts = {status: sum(1 for r in results if r.status == status)
              for status in ("found", "product", "missing")}
    lines = [
        "# GENERATED by scripts/ground-truth.py (make ground-truth) — do not edit.",
        "# The review docs' claims about the tracked tree: every backticked repo",
        "# path under docs/review/ and every symbol in ground_truth.md's canonical-",
        "# sites table. status is found | product | missing; where is the tracked",
        "# path (a suffix match names the real one) or the definition site.",
        "# Missing is the gate.",
        "",
        "[summary]",
        f"docs = {len(docs)}",
        f"claims = {len(results)}",
        f"found = {counts['found']}",
        f"product = {counts['product']}",
        f"missing = {counts['missing']}",
    ]
    for r in results:
        lines += [
            "",
            "[[claim]]",
            f"doc = {toml_str(r.doc)}",
            f"line = {r.line}",
            f"token = {toml_str(r.token)}",
            f"kind = {toml_str(r.kind)}",
            f"status = {toml_str(r.status)}",
            f"where = {toml_str(r.where)}",
        ]
    return "\n".join(lines) + "\n"


def main(root, docs_dir, out_path):
    results = census(root, docs_dir)
    os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(render_toml(results))
    missing = [r for r in results if r.status == "missing"]
    print(f"ground-truth: {len(results)} claims, {len(missing)} missing "
          f"({os.path.relpath(out_path, root)})")
    for r in missing:
        print(f"  {r.doc}:{r.line} `{r.token}` ({r.kind}) not in the tracked tree")
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:4]))
