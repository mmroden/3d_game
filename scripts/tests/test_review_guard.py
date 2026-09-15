"""Review-only, enforced (`make test-assets`).

The review agents under .claude/agents/ make no call that can change
anything. Two things make that true rather than merely asked. Each agent's
frontmatter `tools:` allowlist is held here to the READ_ONLY_TOOLS registry
in scripts/review_guard.py, so a tool reaches a reviewer only by being
declared read-only in one place. And the Bash guard the same frontmatter
installs as a PreToolUse hook (scripts/review-guard.sh, which hands the
hook's stdin to scripts/review_guard.py through the project venv) is held
here to its allowlist: git's reading subcommands, `gh pr view`/`diff`, and
the file readers; nothing that writes, builds, runs, posts, or hides a
command inside another.
"""
import io
import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import review_guard  # noqa: E402

ALLOWED = [
    "git diff main...HEAD",
    "git diff --stat origin/main...origin/review-agents",
    "git show origin/main:.claude/agents/design-review.md",
    "git log --oneline -S HULL_SALT --all",
    "git blame -L 10,20 rust/void-logic/src/seed.rs",
    "git status --short",
    "git -C /Users/x/repo diff --stat",
    "git --no-pager log -3",
    "git ls-files | grep 'rs$'",
    "git rev-parse --abbrev-ref HEAD",
    "git merge-base origin/main HEAD",
    "git grep -n design-review origin/main -- docs",
    "gh pr view 12 --json body --jq .body",
    "gh pr diff 12",
    "cat -n docs/review/doctrine.md",
    "sed -n '1,60p' rust/void-logic/src/seed.rs",
    "sed -n 120,140p Makefile",
    "grep -rn 'fn for_level' rust/",
    "rg --no-heading toml_str scripts",
    "ls -la .claude/agents",
    "wc -l scripts/*.py",
    "diff -rq a b",
    "git log 2>/dev/null",
    "git log 2>&1 | tail -3",
    "cat f | head -5",
    "cat f > /dev/null",
    'for f in a b; do cat -n "$f"; done',
    "cd /repo && git diff",
    "PAGER=cat git log -1",
    "echo done",
    "find rust -name '*.rs' | wc -l",
    "sort scripts/x | uniq -c",
]

DENIED = [
    "git commit -m x",
    "git push",
    "git checkout main",
    "git switch -c x",
    "git add .",
    "git stash",
    "git reset --hard",
    "git rebase main",
    "git branch -D x",
    "git diff --output=x.patch",
    "git diff -o x",
    "gh pr comment 12 --body hi",
    "gh pr create",
    "gh api repos/x",
    "make check",
    "cargo test",
    "python3 x.py",
    "tools/pyenv/bin/python3 x.py",
    "rm -rf out",
    "mv a b",
    "cp a b",
    "mkdir x",
    "touch x",
    "echo hi > f.txt",
    "cat a >> b",
    "cat a | tee b",
    "sed -i 's/a/b/' f",
    "sed -i.bak 's/a/b/' f",
    "sed 's/a/b/w out' f",
    "sed -n 'w out' f",
    "find . -delete",
    "find . -exec rm {} \\;",
    "sort -o out in",
    "curl http://x",
    "ls; rm x",
    "git log && make",
    "git log || rm x",
    "echo `rm x`",
    "echo $(rm x)",
    "cat <(rm x)",
    "git log\nrm -rf x",
    "xargs rm",
    "bash -c 'rm x'",
    "sh -c 'ls'",
    "eval rm x",
    "godot --headless",
    "blender --background",
    "npm test",
    "chmod +x f",
    "brew install x",
    "open x",
    "kill -9 1",
]


@pytest.mark.parametrize("command", ALLOWED)
def test_read_only_commands_pass(command):
    assert review_guard.verdict(command) is None


@pytest.mark.parametrize("command", DENIED)
def test_write_capable_commands_are_refused(command):
    assert review_guard.verdict(command), command


def test_refusal_names_the_offending_word():
    assert "make" in review_guard.verdict("git log && make check")
    assert "commit" in review_guard.verdict("git commit -m x")
    assert "-i" in review_guard.verdict("sed -i 's/a/b/' f")


def _hook(payload):
    err = io.StringIO()
    code = review_guard.main(stdin=io.StringIO(payload), stderr=err)
    return code, err.getvalue()


def test_hook_allows_a_reading_command():
    payload = json.dumps({"tool_name": "Bash", "tool_input": {"command": "git diff"}})
    assert _hook(payload) == (0, "")


def test_hook_blocks_with_the_reason_on_stderr():
    payload = json.dumps({"tool_name": "Bash", "tool_input": {"command": "make check"}})
    code, err = _hook(payload)
    assert code == 2
    assert "make" in err


def test_hook_fails_closed_on_malformed_input():
    assert _hook("not json")[0] == 2
    assert _hook(json.dumps({"tool_input": {}}))[0] == 2


def test_the_hook_wrapper_runs_the_guard_through_the_venv():
    """The frontmatter calls scripts/review-guard.sh; it must reach the guard."""
    env = dict(os.environ, CLAUDE_PROJECT_DIR=str(ROOT))
    wrapper = str(ROOT / "scripts" / "review-guard.sh")
    for command, expected in (("git diff", 0), ("make check", 2)):
        payload = json.dumps({"tool_name": "Bash", "tool_input": {"command": command}})
        run = subprocess.run(["sh", wrapper], input=payload, env=env,
                             capture_output=True, text=True)
        assert run.returncode == expected, (command, run.stderr)


# ------------------------------------------------- the agents' frontmatter
AGENTS = sorted((ROOT / ".claude" / "agents").glob("*review*.md"))


def _frontmatter(path):
    lines = path.read_text().splitlines()
    assert lines[0] == "---", f"{path.name}: no frontmatter"
    end = lines.index("---", 1)
    return "\n".join(lines[1:end])


def _tools(path):
    for line in _frontmatter(path).splitlines():
        if line.startswith("tools:"):
            return {t.strip() for t in line[len("tools:"):].split(",") if t.strip()}
    return None


def test_there_are_review_agents():
    assert AGENTS


@pytest.mark.parametrize("agent", AGENTS, ids=lambda p: p.stem)
def test_agent_tools_are_an_allowlist_of_read_only_tools(agent):
    tools = _tools(agent)
    assert tools, f"{agent.name}: no `tools:` allowlist; a reviewer inherits nothing"
    orchestrator_only = {"Agent"} if agent.stem == "mark-review" else set()
    extra = tools - review_guard.READ_ONLY_TOOLS - orchestrator_only
    assert not extra, f"{agent.name} may use {sorted(extra)}: not in READ_ONLY_TOOLS"


@pytest.mark.parametrize("agent", AGENTS, ids=lambda p: p.stem)
def test_agent_installs_the_bash_guard(agent):
    text = _frontmatter(agent)
    assert "PreToolUse" in text, f"{agent.name}: no PreToolUse hook"
    assert '"Bash"' in text, f"{agent.name}: the guard is not on Bash"
    assert "scripts/review-guard.sh" in text, f"{agent.name}: the guard is not review-guard.sh"
