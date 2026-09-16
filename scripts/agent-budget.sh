#!/bin/sh
# The PreToolUse hook .claude/settings.json installs on Agent: every
# subagent launch in a session is charged against one ledger, and the
# launch past the budget is refused (exit 2 blocks the call, reason on
# stderr). Hands the hook's stdin JSON to scripts/agent_budget.py through
# the project venv; fails closed when the venv is missing, so a fresh
# clone refuses launches rather than allowing them unmetered.
PY="${CLAUDE_PROJECT_DIR:-.}/tools/pyenv/bin/python3"
[ -x "$PY" ] || { echo "agent-budget: tools/pyenv missing; run make deps" >&2; exit 2; }
exec "$PY" "${CLAUDE_PROJECT_DIR:-.}/scripts/agent_budget.py" "$@"
