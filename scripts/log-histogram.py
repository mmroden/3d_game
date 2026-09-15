#!/usr/bin/env python3
"""Synopsis of a door's log — every ERROR and WARNING a step printed,
condensed into a histogram: one row per (message, source location,
asset being processed) with a count, most frequent first, and a tally
per asset. "22k errors of this type from this file, that could be a
thing to investigate first" (owner 2026-09-07): the hill house's
11,315 dropped surfaces sat in the import log for days behind a
tail -5. Written as TOML to out/metrics/log_<step>.toml by make after
each door; the audit holds every step to zero of both.

Reads Godot's headless output (ERROR:/WARNING: lines with their
"   at: function (file:line)" source, progress lines "[ 85% ] reimport
| hill_house.glb" naming the asset in flight, "[ DONE ]" ending it,
ANSI color throughout) and the Blender converters' logs (plan_apply:
WARNING ..., Blender's own Warning:/Error: lines). A message is folded
for grouping — res:// paths and 'quoted names' become wildcards — and
its first raw text is kept as the example.

Usage: log-histogram.py <step> <out.toml> <log>..."""
import json
import os
import re
import sys

try:
    import tomllib  # noqa: F401
except ImportError:  # the audit venv (python 3.9) carries tomli
    import tomli as tomllib  # noqa: F401

ANSI = re.compile(r"\x1b\[[0-9;]*m")
PROGRESS = re.compile(r"^\[\s*\d+%\s*\]\s*\S+\s*\|\s*(.+?)\s*$")
DONE = re.compile(r"^\[ DONE \]")
FILENAME = re.compile(r"^[\w.\-+@ ]+\.\w{1,5}$")
REPORT = re.compile(r"\b(ERROR|WARNING|Error|Warning)\b:?\s*(.*)$")
SOURCE = re.compile(r"^\s+at:\s*(.+?)\s*$")
RES_PATH = re.compile(r"res://\S*[^\s.]")
QUOTED = re.compile(r"'[^']*'")
KINDS = {"ERROR": "error", "Error": "error", "WARNING": "warning", "Warning": "warning"}
TOP = 8  # rows printed in the terminal synopsis


# A Python traceback in a Blender-hosted script's log: the frames between
# the header and the exception line are not reports of their own; the
# exception line is one error. A tool that catches an exception and
# prints it ("TypeError: ...", indented or not) counts the same way,
# the class name dropped from the message; a *Warning class is a warning.
TRACEBACK = re.compile(r"^Traceback \(most recent call last\):")
EXCEPTION = re.compile(r"^\s*([A-Za-z_][A-Za-z0-9_.]*?(Error|Exception|Warning))\s*:\s*(.*)$")


def toml_str(s):
    return json.dumps(s, ensure_ascii=False)


def fold(message):
    """The grouping key of a message: paths and names wildcarded."""
    return QUOTED.sub("'*'", RES_PATH.sub("res://*", message))


def read_reports(path):
    """Every report in one log: (kind, message, at, asset, line) with
    the asset the nearest preceding progress line named (cleared at
    DONE), else the log's own basename. A traceback is one report: its
    exception line."""
    log = os.path.basename(path)
    asset = None
    reports = []
    in_traceback = False
    with open(path, errors="replace") as f:
        lines = [ANSI.sub("", raw.rstrip("\n")) for raw in f]
    for number, line in enumerate(lines, start=1):
        if in_traceback:
            if not line or line[0] in " \t":
                continue  # a frame, or the source line under it
            in_traceback = False
            exc = EXCEPTION.match(line)
            message = exc.group(3).strip() if exc else line.strip()
            reports.append(("error", message, "", asset or log, number, log))
            continue
        if TRACEBACK.match(line):
            in_traceback = True
            continue
        if DONE.match(line):
            asset = None
            continue
        progress = PROGRESS.match(line)
        if progress:
            payload = progress.group(1)
            if FILENAME.match(payload):
                asset = payload
            continue
        if SOURCE.match(line):
            continue
        exc = EXCEPTION.match(line)
        if exc:
            kind = "warning" if exc.group(2) == "Warning" else "error"
            reports.append((kind, exc.group(3).strip(), "", asset or log, number, log))
            continue
        report = REPORT.search(line)
        if not report:
            continue
        at = ""
        if number < len(lines):
            source = SOURCE.match(lines[number])
            if source:
                at = source.group(1)
        reports.append((KINDS[report.group(1)], report.group(2).strip(), at,
                        asset or log, number, log))
    return reports, len(lines)


def histogram(reports):
    rows = {}
    for kind, message, at, asset, number, log in reports:
        key = (kind, fold(message), at, asset, log)
        row = rows.get(key)
        if row is None:
            rows[key] = {"kind": kind, "message": key[1], "at": at, "asset": asset,
                         "log": log, "count": 1, "first_line": number, "example": message}
        else:
            row["count"] += 1
    return sorted(rows.values(), key=lambda r: (-r["count"], r["first_line"]))


def tally(reports):
    per_asset = {}
    for kind, _message, _at, asset, _number, _log in reports:
        counts = per_asset.setdefault(asset, {"errors": 0, "warnings": 0})
        counts["error" == kind and "errors" or "warnings"] += 1
    return sorted(({"asset": a, **c} for a, c in per_asset.items()),
                  key=lambda r: (-r["errors"], -r["warnings"], r["asset"]))


def main(step, out_path, *log_paths):
    if not log_paths:
        raise SystemExit(f"log-histogram: {step}: no logs to read — a histogram of nothing "
                         "is not a clean bill; run the door first")
    reports, total_lines = [], 0
    for path in log_paths:
        found, count = read_reports(path)
        reports += found
        total_lines += count
    rows = histogram(reports)
    errors = [r for r in rows if r["kind"] == "error"]
    warnings = [r for r in rows if r["kind"] == "warning"]
    by_asset = tally(reports)
    lines = [
        "# GENERATED by scripts/log-histogram.py (make assets / make metrics) — do not edit.",
        "# What one door printed as errors and warnings, condensed: a row per",
        "# message x source location x asset in flight, most frequent first",
        "# ([[error]], [[warning]]), and a tally per asset ([[by_asset]]).",
        "# `message` is folded for grouping (res:// paths and 'names' are *);",
        "# `example` is the first raw text; `asset` is the file Godot named in",
        "# its progress line when the report fired, else the log itself.",
        "",
        "[summary]",
        f"step = {toml_str(step)}",
        f"logs = [{', '.join(toml_str(os.path.basename(p)) for p in log_paths)}]",
        f"lines = {total_lines}",
        f"errors = {sum(r['count'] for r in errors)}",
        f"warnings = {sum(r['count'] for r in warnings)}",
        f"distinct_errors = {len(errors)}",
        f"distinct_warnings = {len(warnings)}",
        "",
    ]
    for row in by_asset:
        lines += [
            "[[by_asset]]",
            f"asset = {toml_str(row['asset'])}",
            f"errors = {row['errors']}",
            f"warnings = {row['warnings']}",
            "",
        ]
    for table, table_rows in (("error", errors), ("warning", warnings)):
        for row in table_rows:
            lines += [
                f"[[{table}]]",
                f"message = {toml_str(row['message'])}",
                f"at = {toml_str(row['at'])}",
                f"asset = {toml_str(row['asset'])}",
                f"log = {toml_str(row['log'])}",
                f"count = {row['count']}",
                f"first_line = {row['first_line']}",
                f"example = {toml_str(row['example'])}",
                "",
            ]
    os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
    with open(out_path, "w") as f:
        f.write("\n".join(lines))
    print(
        f"log-histogram: {step} {sum(r['count'] for r in errors)} errors "
        f"({len(errors)} distinct), {sum(r['count'] for r in warnings)} warnings "
        f"({len(warnings)} distinct) over {len(log_paths)} logs, {total_lines} lines "
        f"-> {out_path}"
    )
    for row in rows[:TOP]:
        where = f" at {row['at']}" if row["at"] else ""
        print(f"  {row['count']:>7} x {row['kind'].upper():7} {row['message'][:110]} "
              f"[{row['asset']}]{where}")


if __name__ == "__main__":
    main(*sys.argv[1:])
