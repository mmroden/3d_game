"""Review-only, enforced (`make test-assets`).

The review agents under .claude/agents/ make no call that can change
anything. Two things make that true rather than merely asked. Each agent's
frontmatter `tools:` allowlist is held here to the `read_only_tools()`
registry in scripts/review_guard.py, so a tool reaches a reviewer only by
being declared read-only in one place. And the Bash guard the same
frontmatter installs as a PreToolUse hook (scripts/review-guard.sh, which
hands the hook's stdin to scripts/review_guard.py through the project venv)
is held here to its allowlist: git's reading subcommands and `gh pr`
reads, and nothing else. Owner 2026-09-15: a reviewer has Read for docs
and data and the symbol tools for code, so every file reader in Bash is a
second door and is refused; no writes, builds, runs, posts, redirects to a
file, or commands hidden inside another.

Every table here is a function, so it can be edited symbolically.
"""
import io
import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))

import review_guard  # noqa: E402


def root():
    return Path(__file__).resolve().parents[2]


def allowed_commands():
    return [
        "git diff --stat main...HEAD",
        "git diff --name-only main...HEAD",
        "git diff --name-status origin/main...origin/review-agents",
        "git show --stat HEAD",
        "git show --name-only HEAD~2",
        "git show origin/main:.claude/agents/design-review.md",
        "git show HEAD:docs/review/reviewer.md",
        "git log --oneline -S HULL_SALT --all",
        "git log --oneline -5 -- rust/void-logic/src/seed.rs",
        "git blame -L 10,20 docs/review/reviewer.md",
        "git status --short",
        "git -C /Users/x/repo diff --stat",
        "git --no-pager log -3",
        "git ls-files -- '*.rs'",
        "git rev-parse --abbrev-ref HEAD",
        "git merge-base origin/main HEAD",
        "git grep -n design-review origin/main -- docs",
        "git grep -n salt -- rosters catalog",
        "gh pr view 12 --json body --jq .body",
        "gh pr diff 12",
        "gh pr list --state open",
        "gh pr checks 12",
        "git log 2>/dev/null",
        "git log -3 2>&1",
        "git diff --stat > /dev/null",
        "PAGER=cat git log -1",
        "git diff --stat && git status",
        "git log\ngit status",
    ]


def denied_commands():
    return [
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
        "git diff",
        "git diff main...HEAD",
        "git diff --stat main...HEAD -- rust/ && git diff",
        "git show",
        "git show HEAD",
        "git show HEAD:rust/void-logic/src/seed.rs",
        "git show origin/main:scripts/credits.py",
        "git blame -L 10,20 rust/void-logic/src/seed.rs",
        "git blame godot/scripts/main.gd",
        "git grep -n salt",
        "git grep -n salt -- rust/",
        "git grep -n salt origin/main -- scripts/credits.py",
        "git log -p -3",
        "git log --patch -- docs",
        "git log -u",
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
        "echo done",
        "cat a >> b",
        "cat a | tee b",
        "cat -n docs/review/doctrine.md",
        "cat rust/void-logic/src/seed.rs",
        "cat f | head -5",
        "head -20 scripts/credits.py",
        "tail -5 godot/scripts/main.gd",
        "sed -n '1,60p' docs/review/doctrine.md",
        "sed -n 120,140p Makefile",
        "sed -i 's/a/b/' f",
        "sed -i.bak 's/a/b/' f",
        "sed 's/a/b/w out' f",
        "sed -n 'w out' f",
        "grep -rn 'salt' docs/",
        "grep -rn 'fn for_level' rust/",
        "grep -n TODO godot/tests/test_combat.gd",
        "rg --no-heading toml_str scripts",
        "git ls-files | grep 'rs$'",
        "git log 2>&1 | tail -3",
        "ls -la .claude/agents",
        "ls rust/void-logic/src",
        "wc -l docs/*.md",
        "wc -l scripts/*.py",
        "find docs -name '*.md' | wc -l",
        "find rust -name '*.rs' | wc -l",
        "find . -delete",
        "find . -exec rm {} \\;",
        "sort docs/x | uniq -c",
        "sort -o out in",
        "diff -rq a b",
        "jq '.env' .claude/settings.json",
        "cd /repo && git diff --stat",
        'for f in a b; do cat -n "$f"; done',
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


@pytest.mark.parametrize("command", allowed_commands())
def test_git_and_gh_reads_pass(command):
    assert review_guard.verdict(command) is None


@pytest.mark.parametrize("command", denied_commands())
def test_everything_else_is_refused(command):
    assert review_guard.verdict(command), command


def test_refusal_names_the_offending_word():
    assert "make" in review_guard.verdict("git log && make check")
    assert "commit" in review_guard.verdict("git commit -m x")
    assert "cat" in review_guard.verdict("cat Makefile")
    assert "--output" in review_guard.verdict("git diff --output=x.patch")


def hook(payload):
    err = io.StringIO()
    code = review_guard.main(stdin=io.StringIO(payload), stderr=err)
    return code, err.getvalue()


def test_hook_allows_a_reading_command():
    payload = json.dumps({"tool_name": "Bash", "tool_input": {"command": "git diff --stat"}})
    assert hook(payload) == (0, "")


def test_hook_blocks_with_the_reason_on_stderr():
    payload = json.dumps({"tool_name": "Bash", "tool_input": {"command": "make check"}})
    code, err = hook(payload)
    assert code == 2
    assert "make" in err
    assert "symbol tools" in err


def test_hook_fails_closed_on_malformed_input():
    assert hook("not json")[0] == 2
    assert hook(json.dumps({"tool_input": {}}))[0] == 2


def test_the_hook_wrapper_runs_the_guard_through_the_venv():
    """The frontmatter calls scripts/review-guard.sh; it must reach the guard."""
    env = dict(os.environ, CLAUDE_PROJECT_DIR=str(root()))
    wrapper = str(root() / "scripts" / "review-guard.sh")
    for command, expected in (("git diff --stat", 0), ("make check", 2)):
        payload = json.dumps({"tool_name": "Bash", "tool_input": {"command": command}})
        run = subprocess.run(["sh", wrapper], input=payload, env=env,
                             capture_output=True, text=True)
        assert run.returncode == expected, (command, run.stderr)


# ------------------------------------------------- the agents' frontmatter
def agents():
    return sorted((root() / ".claude" / "agents").glob("*review*.md"))


def frontmatter(path):
    lines = path.read_text().splitlines()
    assert lines[0] == "---", f"{path.name}: no frontmatter"
    end = lines.index("---", 1)
    return "\n".join(lines[1:end])


def tools_of(path):
    for line in frontmatter(path).splitlines():
        if line.startswith("tools:"):
            return {t.strip() for t in line[len("tools:"):].split(",") if t.strip()}
    return None


def model_of(path):
    for line in frontmatter(path).splitlines():
        if line.startswith("model:"):
            return line[len("model:"):].strip()
    return None


def test_there_are_review_agents():
    assert agents()


@pytest.mark.parametrize("agent", agents(), ids=lambda p: p.stem)
def test_agent_tools_are_an_allowlist_of_read_only_tools(agent):
    tools = tools_of(agent)
    assert tools, f"{agent.name}: no `tools:` allowlist; a reviewer inherits nothing"
    orchestrator_only = {"Agent"} if agent.stem == "mark-review" else set()
    extra = tools - review_guard.read_only_tools() - orchestrator_only
    assert not extra, f"{agent.name} may use {sorted(extra)}: not in read_only_tools()"


@pytest.mark.parametrize("agent", agents(), ids=lambda p: p.stem)
def test_agent_installs_the_bash_guard(agent):
    text = frontmatter(agent)
    assert "PreToolUse" in text, f"{agent.name}: no PreToolUse hook"
    assert '"Bash"' in text, f"{agent.name}: the guard is not on Bash"
    assert "scripts/review-guard.sh" in text, f"{agent.name}: the guard is not review-guard.sh"


@pytest.mark.parametrize("agent", agents(), ids=lambda p: p.stem)
def test_review_agents_run_on_opus(agent):
    """Owner 2026-09-15: reviewing is Opus work, never inherited from a Fable invoker."""
    assert model_of(agent) == "opus", f"{agent.name}: model is {model_of(agent)!r}, not opus"
