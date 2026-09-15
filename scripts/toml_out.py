"""One TOML string quoter for every reading the pipeline writes.

A TOML basic string takes JSON's escapes for the characters that matter
(quote, backslash, control characters, newline), so `json.dumps` is the
whole implementation and a value with a newline in it round-trips instead
of breaking the file. Readings under out/metrics/ and catalog/*.generated
import this rather than declaring their own quoter.
"""
import json


def toml_str(value):
    """`value` as a TOML basic string, non-ASCII kept as written."""
    return json.dumps(str(value), ensure_ascii=False)
