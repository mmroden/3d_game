#!/bin/sh
# The PreToolUse hook every review agent's frontmatter installs on Bash.
# Hands the hook's stdin JSON to scripts/review_guard.py through the
# project venv; fails closed (exit 2 blocks the call) when the venv is
# missing, so a fresh clone refuses rather than allows.
PY="${CLAUDE_PROJECT_DIR:-.}/tools/pyenv/bin/python3"
[ -x "$PY" ] || { echo "review-guard: tools/pyenv missing; run make deps" >&2; exit 2; }
exec "$PY" "${CLAUDE_PROJECT_DIR:-.}/scripts/review_guard.py"
