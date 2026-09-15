"""scripts/toml_out.py: the one TOML string quoter (`make test-assets`).

Every value the quoter writes must read back as the same value through a
TOML parser, including the characters that break a hand-rolled quoter:
quotes, backslashes, newlines, tabs, and non-ASCII.
"""
import sys
from pathlib import Path

import pytest

try:
    import tomllib
except ImportError:  # the audit venv (python 3.9) carries tomli
    import tomli as tomllib

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import toml_out  # noqa: E402


@pytest.mark.parametrize("value", [
    "plain",
    'with "quotes"',
    "back\\slash",
    "two\nlines",
    "tab\there",
    "Größe — épée",
    "",
])
def test_quoted_value_round_trips(value):
    doc = f"v = {toml_out.toml_str(value)}\n"
    assert tomllib.loads(doc)["v"] == value


def test_non_strings_are_written_as_their_str():
    assert tomllib.loads(f"v = {toml_out.toml_str(42)}\n")["v"] == "42"
