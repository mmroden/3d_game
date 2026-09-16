"""Reviewer Bash guard: the PreToolUse hook every review agent carries.

    tools/pyenv/bin/python3 scripts/review_guard.py   (stdin: the hook's JSON)

The review agents under .claude/agents/ are review-only: their frontmatter
`tools:` allowlist keeps every editing, publishing, and scheduling tool out
of their reach, and this guard closes the one hole an allowlist leaves,
Bash. It reads the hook input on stdin, takes `tool_input.command`, and
exits 2 (blocked, reason on stderr) unless every command in it is on the
read-only allowlist below: git's reading subcommands, `gh pr view`/`diff`,
and the file-reading utilities. No redirection to a file, no command
hidden inside another (backticks, `$(`, `<(`), no editing flag on sed, no
`-exec` on find. Anything not listed is refused, and the refusal names the
offending word so the reviewer can say what it needed. Unparseable input
is refused too: the guard fails closed.

Two registries live here so each has one home. `read_only_tools()` is the
set of tools a review agent's `tools:` line may name; the frontmatter
contract test in scripts/tests/test_review_guard.py holds every agent to
it, so a tool reaches a reviewer only by being declared read-only here.
The command allowlist, `read_commands()` and the per-command checks, is
what `verdict` applies to Bash. Every registry is a function so that it
is a symbol: code here is edited only through the symbol tools.
"""
import json
import re
import shlex
import sys


def read_only_tools():
    """The tools a review agent's frontmatter may name."""
    return frozenset({
        "Read", "Glob", "Grep", "Bash", "ToolSearch", "WebFetch",
        "mcp__serena__find_symbol", "mcp__serena__find_referencing_symbols",
        "mcp__serena__find_implementations", "mcp__serena__find_declaration",
        "mcp__serena__get_symbols_overview", "mcp__serena__get_diagnostics_for_file",
        "mcp__serena__get_diagnostics_for_symbol", "mcp__serena__get_current_config",
    })


def read_commands():
    """A reviewer's Bash is git and gh, nothing else: docs are read with Read,
    code with the symbol tools (owner 2026-09-15)."""
    return frozenset({"git", "gh"})


def git_read():
    return frozenset({
        "diff", "show", "log", "blame", "status", "grep", "ls-files", "ls-tree",
        "cat-file", "rev-parse", "merge-base", "describe", "shortlog", "name-rev",
        "rev-list", "check-ignore", "show-ref", "for-each-ref", "count-objects",
        "var", "version", "help",
    })


def git_global_with_arg():
    return frozenset({"-C", "-c", "--git-dir", "--work-tree", "--namespace"})


def gh_read():
    return frozenset({
        ("pr", "view"), ("pr", "diff"), ("pr", "list"), ("pr", "checks"), ("pr", "status"),
        ("issue", "view"), ("issue", "list"), ("repo", "view"), ("run", "view"), ("run", "list"),
    })


def code_path():
    """A code file by extension, or a path into a code root; the same shape the
    settings.json hooks refuse for everyone (owner 2026-09-15)."""
    return re.compile(r"\.(py|rs|cpp|cc|cxx|hpp|h|gd)(\b|$)"
                      r"|(^|/)(rust|scripts)(/|$)"
                      r"|(^|/)godot/(scripts|tests|tools)(/|$)")


def names_only_flags():
    """diff/show flags that print names and counts, never file content."""
    return frozenset({"--stat", "--name-only", "--name-status", "--numstat",
                      "--shortstat", "--dirstat", "-s", "--no-patch"})


def patch_flags():
    return frozenset({"-p", "--patch", "-u"})


def separators():
    return frozenset({"|", "||", "&&", ";", ";;", "&", "(", ")", "{", "}"})


def leading_keywords():
    return frozenset({"if", "elif", "while", "until", "then", "else", "do", "!", "time"})


def lone_keywords():
    return frozenset({"done", "fi", "esac"})


def hidden_command_markers():
    return ("`", "$(", "<(", ">(")


def assignment():
    return re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")


def _tokens(line):
    lexer = shlex.shlex(line, posix=True, punctuation_chars=True)
    lexer.whitespace_split = True
    return list(lexer)


def _segments(tokens):
    segment = []
    for token in tokens:
        if token in separators():
            if segment:
                yield segment
            segment = []
        else:
            segment.append(token)
    if segment:
        yield segment


def _strip_redirections(segment):
    """Drop redirection tokens; return the reason if one writes to a file."""
    kept, i = [], 0
    while i < len(segment):
        token = segment[i]
        following = segment[i + 1] if i + 1 < len(segment) else ""
        if ">" in token:
            to_null = following == "/dev/null"
            to_fd = token.endswith("&") and following in ("1", "2")
            if not (to_null or to_fd):
                return None, f"`{token} {following}`".strip() + " redirects output to a file"
            i += 2
            continue
        if token in ("<", "<<", "<<<"):
            i += 2
            continue
        kept.append(token)
        i += 1
    return kept, None


def _check_git(args):
    i = 0
    while i < len(args) and args[i].startswith("-"):
        i += 2 if args[i] in git_global_with_arg() else 1
    sub = args[i] if i < len(args) else ""
    rest = args[i + 1:]
    if sub not in git_read():
        return f"`git {sub}`".strip() + " can change the repository or its branches"
    for token in rest:
        if token == "-o" or token.startswith("--output"):
            return f"`git {sub} {token}` writes a file"
    return _check_git_content(sub, rest)


def _check_git_content(sub, rest):
    """git prints file content only as names and counts, or from a non-code
    path; code is reached only through the symbol tools (owner 2026-09-15)."""
    symbols = "; code is reached only through the symbol tools"
    code = [t for t in rest if code_path().search(t.split(":", 1)[1] if ":" in t else t)]
    if sub in ("blame", "grep") and code:
        return f"`git {sub} {code[0]}` reads code as text{symbols}"
    if sub == "grep" and "--" not in rest:
        return "`git grep` with no pathspec searches code; give `-- <docs or data path>`"
    if sub == "diff" and not any(t in names_only_flags() for t in rest):
        return f"`git diff` prints file content; use --stat, --name-only, or --name-status{symbols}"
    if sub == "show":
        if code:
            return f"`git show {code[0]}` reads code as text{symbols}"
        if not any(t in names_only_flags() for t in rest) and not any(":" in t for t in rest):
            return f"`git show` prints the patch; use --stat or --name-only, or ref:path to a docs file{symbols}"
    if sub == "log" and any(t in patch_flags() for t in rest):
        return f"`git log -p` prints file content{symbols}"
    return None


def _check_gh(args):
    pair = tuple(args[:2])
    if pair not in gh_read():
        return f"`gh {' '.join(args[:2])}`".strip() + " is not a read of the pull request"
    return None


def _check_segment(segment):
    segment, reason = _strip_redirections(segment)
    if reason:
        return reason
    while segment and (assignment().match(segment[0]) or segment[0] in leading_keywords()):
        segment = segment[1:]
    if not segment or segment[0] in lone_keywords() or segment[0] == "for":
        return None
    command, args = segment[0], segment[1:]
    if command not in read_commands():
        return (f"`{command}` is not on the reviewer's allowlist; a reviewer's Bash "
                "is git and gh reads only")
    if command == "git":
        return _check_git(args)
    return _check_gh(args)


def verdict(command):
    """None when every command is read-only; otherwise the reason to block."""
    for marker in hidden_command_markers():
        if marker in command:
            return f"`{marker}` hides a command inside another"
    for line in command.split("\n"):
        if not line.strip():
            continue
        try:
            tokens = _tokens(line)
        except ValueError as error:
            return f"could not parse the command ({error})"
        for segment in _segments(tokens):
            reason = _check_segment(segment)
            if reason:
                return reason
    return None


def main(stdin=sys.stdin, stderr=sys.stderr):
    try:
        command = json.load(stdin)["tool_input"]["command"]
    except (ValueError, KeyError, TypeError):
        print("review_guard: no command in the hook input; refusing.", file=stderr)
        return 2
    if not isinstance(command, str) or not command.strip():
        print("review_guard: empty command; refusing.", file=stderr)
        return 2
    reason = verdict(command)
    if reason:
        print(f"Reviewers are read-only: {reason}. Bash is for reading history and "
              "docs (git diff/show/log/blame/status, gh pr view/diff; cat, grep, sed "
              "on docs, TOML, Makefiles). Code is read through the symbol tools. "
              "scripts/review_guard.py", file=stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
