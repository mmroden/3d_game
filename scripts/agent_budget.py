"""Subagent launch meter: the PreToolUse hook .claude/settings.json puts on Agent.

    tools/pyenv/bin/python3 scripts/agent_budget.py [budget]   (stdin: the hook's JSON)

The owner's rule (2026-09-15): a session gets a fixed budget of subagent
launches, and the launch past it is refused. Every Agent call, the invoker's
and any subagent's, reaches this hook; it reads the hook input on stdin,
appends one line to the session's ledger (out/agent-budget/<session_id>:
timestamp, subagent type, description) while the ledger is under budget,
and exits 2 (blocked, reason on stderr) once it is not. The ledger is the
audit trail of what a session spawned. Unparseable input, a missing session
id, or a call for any tool but Agent is refused too: the meter fails closed.

The budget is the `budget` parameter of `charge` and `main`, seven by
default; the hook passes an integer argument to change it, and the
`AGENT_BUDGET_LEDGER` environment variable moves the ledger directory
(the tests use it to keep their ledgers out of out/).
"""
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path


def charge(ledger_dir, session_id, subagent_type, description, budget=7):
    """Record one launch against the session's ledger; return the refusal
    reason once the budget is spent, or None while the launch is allowed."""
    ledger_dir = Path(ledger_dir)
    ledger_dir.mkdir(parents=True, exist_ok=True)
    ledger = ledger_dir / session_id
    spent = len(ledger.read_text().splitlines()) if ledger.exists() else 0
    if spent >= budget:
        return (f"this session has launched {spent} subagents, its budget of "
                f"{budget}; refusing '{subagent_type}: {description}'. The ledger "
                f"is {ledger}. Raising the budget is the owner's call "
                f"(scripts/agent_budget.py, .claude/settings.json).")
    stamp = datetime.now(timezone.utc).isoformat(timespec="seconds")
    with ledger.open("a") as out:
        out.write(f"{stamp}\t{subagent_type}\t{description}\n")
    return None


def main(stdin=sys.stdin, stderr=sys.stderr, ledger_dir=None, budget=7):
    try:
        hook = json.load(stdin)
        session_id = hook["session_id"]
        tool_name = hook["tool_name"]
        tool_input = hook.get("tool_input") or {}
    except (ValueError, KeyError, TypeError):
        print("agent_budget: no session id or tool name in the hook input; refusing.",
              file=stderr)
        return 2
    if not isinstance(session_id, str) or not session_id.strip():
        print("agent_budget: empty session id; refusing.", file=stderr)
        return 2
    if tool_name != "Agent":
        print(f"agent_budget: wired to {tool_name!r}, meters only Agent; refusing.",
              file=stderr)
        return 2
    if ledger_dir is None:
        ledger_dir = os.environ.get("AGENT_BUDGET_LEDGER") or (
            Path(os.environ.get("CLAUDE_PROJECT_DIR", ".")) / "out" / "agent-budget")
    reason = charge(ledger_dir, session_id.strip(),
                    str(tool_input.get("subagent_type", "?")),
                    str(tool_input.get("description", "")), budget=budget)
    if reason:
        print(f"Subagent launches are metered: {reason}", file=stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(budget=int(sys.argv[1]) if len(sys.argv) > 1 else 7))
