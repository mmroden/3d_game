"""Subagent launches are metered (`make test-assets`).

The owner's rule (2026-09-15, after a run that spawned twenty-one review
agents he never asked for): a session gets a fixed budget of subagent
launches, seven, and the launch past it is refused. Two things make that a
mechanism rather than a request. .claude/settings.json installs
scripts/agent-budget.sh as a PreToolUse hook on Agent (and pins the
concurrency cap and an ask-rule on Agent beside it), and that hook hands the
call to scripts/agent_budget.py, which charges every launch to one ledger
per session under out/agent-budget/ and exits 2 once the budget is spent.
"""
import io
import json
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))

import agent_budget  # noqa: E402


def root():
    return Path(__file__).resolve().parents[2]


def settings():
    return json.loads((root() / ".claude" / "settings.json").read_text())


def launch(ledger, session="s1", subagent_type="mark-review", description="review", **kw):
    payload = json.dumps({
        "session_id": session,
        "tool_name": "Agent",
        "tool_input": {"subagent_type": subagent_type, "description": description},
    })
    err = io.StringIO()
    code = agent_budget.main(stdin=io.StringIO(payload), stderr=err, ledger_dir=ledger, **kw)
    return code, err.getvalue()


def test_launches_within_the_budget_are_allowed(tmp_path):
    for _ in range(7):
        assert launch(tmp_path) == (0, "")


def test_the_launch_past_the_budget_is_refused_and_says_so(tmp_path):
    for _ in range(7):
        launch(tmp_path)
    code, err = launch(tmp_path, description="one too many")
    assert code == 2
    assert "7" in err and "agent_budget" in err


def test_every_later_launch_stays_refused(tmp_path):
    for _ in range(9):
        launch(tmp_path)
    assert launch(tmp_path)[0] == 2


def test_the_budget_is_a_parameter_with_seven_as_its_default(tmp_path):
    assert launch(tmp_path, budget=1) == (0, "")
    assert launch(tmp_path, budget=1)[0] == 2


def test_sessions_are_metered_separately(tmp_path):
    for _ in range(7):
        launch(tmp_path, session="a")
    assert launch(tmp_path, session="a")[0] == 2
    assert launch(tmp_path, session="b") == (0, "")


def test_the_ledger_records_each_launch(tmp_path):
    launch(tmp_path, subagent_type="review-dry", description="dry lens")
    launch(tmp_path, subagent_type="review-done", description="done lens")
    lines = (tmp_path / "s1").read_text().splitlines()
    assert len(lines) == 2
    assert "review-dry" in lines[0] and "dry lens" in lines[0]
    assert "review-done" in lines[1]


def test_fails_closed_on_malformed_input(tmp_path):
    err = io.StringIO()
    assert agent_budget.main(stdin=io.StringIO("not json"), stderr=err, ledger_dir=tmp_path) == 2
    no_session = json.dumps({"tool_name": "Agent", "tool_input": {}})
    assert agent_budget.main(stdin=io.StringIO(no_session), stderr=err, ledger_dir=tmp_path) == 2


def test_a_call_for_any_other_tool_is_refused(tmp_path):
    """Wired to the wrong matcher, the hook refuses loudly instead of metering nothing."""
    payload = json.dumps({"session_id": "s1", "tool_name": "Bash", "tool_input": {"command": "ls"}})
    err = io.StringIO()
    assert agent_budget.main(stdin=io.StringIO(payload), stderr=err, ledger_dir=tmp_path) == 2
    assert "Bash" in err.getvalue()


def test_the_hook_wrapper_runs_the_budget_through_the_venv(tmp_path):
    """settings.json calls scripts/agent-budget.sh; it must reach the meter."""
    env = dict(os.environ, CLAUDE_PROJECT_DIR=str(root()), AGENT_BUDGET_LEDGER=str(tmp_path))
    wrapper = str(root() / "scripts" / "agent-budget.sh")
    payload = json.dumps({"session_id": "w", "tool_name": "Agent",
                          "tool_input": {"subagent_type": "Explore", "description": "x"}})
    codes = [subprocess.run(["sh", wrapper], input=payload, env=env,
                            capture_output=True, text=True).returncode for _ in range(8)]
    assert codes == [0] * 7 + [2]


# ------------------------------------------------------- the settings shell
def test_settings_install_the_budget_hook_on_agent():
    hooks = [h for h in settings()["hooks"]["PreToolUse"] if h["matcher"] == "Agent"]
    assert hooks, "no PreToolUse hook on Agent"
    assert any("scripts/agent-budget.sh" in c["command"] for h in hooks for c in h["hooks"])


def test_settings_cap_concurrent_subagents_at_the_budget():
    assert settings()["env"]["CLAUDE_CODE_MAX_CONCURRENT_SUBAGENTS"] == "7"


def test_settings_ask_before_every_agent_launch():
    assert "Agent" in settings()["permissions"]["ask"]
