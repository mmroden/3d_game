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

Two registries live here so each has one home. READ_ONLY_TOOLS is the set
of tools a review agent's `tools:` line may name; the frontmatter contract
test in scripts/tests/test_review_guard.py holds every agent to it, so a
tool reaches a reviewer only by being declared read-only here. The command
allowlist below is what `verdict` applies to Bash.
"""
import json
import re
import shlex
import sys

READ_ONLY_TOOLS = frozenset({
    "Read", "Glob", "Grep", "Bash", "ToolSearch", "WebFetch",
    "mcp__serena__find_symbol", "mcp__serena__find_referencing_symbols",
    "mcp__serena__find_implementations", "mcp__serena__find_declaration",
    "mcp__serena__get_symbols_overview", "mcp__serena__search_for_pattern",
    "mcp__serena__read_file", "mcp__serena__list_dir", "mcp__serena__find_file",
    "mcp__serena__get_current_config",
})

READ_COMMANDS = frozenset({
    "cat", "head", "tail", "wc", "ls", "diff", "cmp", "comm", "echo", "printf",
    "cut", "tr", "uniq", "nl", "paste", "basename", "dirname", "pwd", "date",
    "file", "stat", "which", "test", "[", "true", "false", "cd", "jq", "column",
    "realpath", "grep", "egrep", "fgrep", "rg", "sed", "find", "sort",
    "git", "gh",
})
GIT_READ = frozenset({
    "diff", "show", "log", "blame", "status", "grep", "ls-files", "ls-tree",
    "cat-file", "rev-parse", "merge-base", "describe", "shortlog", "name-rev",
    "rev-list", "check-ignore", "show-ref", "for-each-ref", "count-objects",
    "var", "version", "help",
})
GIT_GLOBAL_WITH_ARG = frozenset({"-C", "-c", "--git-dir", "--work-tree", "--namespace"})
GH_READ = frozenset({
    ("pr", "view"), ("pr", "diff"), ("pr", "list"), ("pr", "checks"), ("pr", "status"),
    ("issue", "view"), ("issue", "list"), ("repo", "view"), ("run", "view"), ("run", "list"),
})
SED_FLAGS = frozenset({"-n", "-E", "-r", "-s", "-u", "-z", "--quiet", "--silent",
                       "--regexp-extended", "--posix"})
FIND_WRITES = frozenset({"-delete", "-exec", "-execdir", "-ok", "-okdir",
                         "-fprint", "-fprint0", "-fprintf", "-fls"})
SEPARATORS = frozenset({"|", "||", "&&", ";", ";;", "&", "(", ")", "{", "}"})
LEADING_KEYWORDS = frozenset({"if", "elif", "while", "until", "then", "else", "do", "!", "time"})
LONE_KEYWORDS = frozenset({"done", "fi", "esac"})
HIDDEN_COMMAND = ("`", "$(", "<(", ">(")
ASSIGNMENT = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")
SED_RANGE_PRINT = re.compile(r"^[0-9$]+(,[0-9$]+)?p$")


def _tokens(line):
    lexer = shlex.shlex(line, posix=True, punctuation_chars=True)
    lexer.whitespace_split = True
    return list(lexer)


def _segments(tokens):
    segment = []
    for token in tokens:
        if token in SEPARATORS:
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


def _sed_script_ok(script):
    for piece in script.split(";"):
        piece = piece.strip()
        if not piece or piece == "p" or SED_RANGE_PRINT.match(piece):
            continue
        if len(piece) > 1 and piece[0] == "s":
            delimiter, flags, seen, j = piece[1], "", 0, 2
            while j < len(piece):
                if piece[j] == "\\":
                    j += 2
                    continue
                if piece[j] == delimiter:
                    seen += 1
                    if seen == 2:
                        flags = piece[j + 1:]
                        break
                j += 1
            if seen == 2 and not set(flags) & set("we"):
                continue
        return False
    return True


def _check_git(args):
    i = 0
    while i < len(args) and args[i].startswith("-"):
        i += 2 if args[i] in GIT_GLOBAL_WITH_ARG else 1
    sub = args[i] if i < len(args) else ""
    if sub not in GIT_READ:
        return f"`git {sub}`".strip() + " can change the repository or its branches"
    for token in args[i + 1:]:
        if token == "-o" or token.startswith("--output"):
            return f"`git {sub} {token}` writes a file"
    return None


def _check_gh(args):
    pair = tuple(args[:2])
    if pair not in GH_READ:
        return f"`gh {' '.join(args[:2])}`".strip() + " is not a read of the pull request"
    return None


def _check_sed(args):
    scripts, files, i = [], [], 0
    while i < len(args):
        token = args[i]
        if token.startswith("-i") or token.startswith("--in-place"):
            return f"`sed {token}` edits the file in place"
        if token in ("-e", "--expression"):
            scripts.append(args[i + 1] if i + 1 < len(args) else "")
            i += 2
            continue
        if token.startswith("-"):
            if token not in SED_FLAGS:
                return f"`sed {token}` is not a reading flag"
        elif not scripts and not files:
            scripts.append(token)
        else:
            files.append(token)
        i += 1
    for script in scripts:
        if not _sed_script_ok(script):
            return f"`sed {script}` is not a print or substitute script"
    return None


def _check_segment(segment):
    segment, reason = _strip_redirections(segment)
    if reason:
        return reason
    while segment and (ASSIGNMENT.match(segment[0]) or segment[0] in LEADING_KEYWORDS):
        segment = segment[1:]
    if not segment or segment[0] in LONE_KEYWORDS or segment[0] == "for":
        return None
    command, args = segment[0], segment[1:]
    if command not in READ_COMMANDS:
        return f"`{command}` is not on the reviewer's read-only allowlist"
    if command == "git":
        return _check_git(args)
    if command == "gh":
        return _check_gh(args)
    if command == "sed":
        return _check_sed(args)
    if command == "find" and any(token in FIND_WRITES for token in args):
        return "`find` with " + ", ".join(t for t in args if t in FIND_WRITES) + " changes or runs things"
    if command == "sort" and any(token == "-o" or token.startswith("--output") for token in args):
        return "`sort -o` writes a file"
    return None


def verdict(command):
    """None when every command is read-only; otherwise the reason to block."""
    for marker in HIDDEN_COMMAND:
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
        print(f"Reviewers are read-only: {reason}. Bash is for reading "
              "(git diff/show/log/blame/status, gh pr view/diff, cat, grep...). "
              "scripts/review_guard.py", file=stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
