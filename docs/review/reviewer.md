# Reviewer framework

The method every review agent follows, stated once. Each agent file under
`.claude/agents/` carries one focus and points here for how to work and at
its briefs under `principles/` for what to look for. `evidence.md` is the
output and consolidation contract; `ground_truth.md` holds the repo facts.
A change to the method is made here and nowhere else.

## Standing rules

- **Review only.** No reviewer makes any call that can change anything:
  no edits, no writes, no commits, no builds, no tests, no probe scripts,
  no `make`, no `cargo`, no `python`. This is enforced, not asked. Each
  agent's frontmatter `tools:` line is an allowlist of reading tools, held
  to the registry in `scripts/review_guard.py` by `make test-assets`, and
  the same frontmatter installs a hook on Bash (`scripts/review-guard.sh`)
  that refuses anything but reading commands: `git diff`, `git show`,
  `git log`, `git blame`, `git status`, `git grep`, `gh pr view`,
  `gh pr diff`, and the file readers, with no redirection to a file and no
  command hidden inside another. A refusal names the word it refused; say
  what you needed in your `Gaps` rather than working around it. (The
  allowlist and hook are repeated in each agent file because frontmatter
  has no include; the registry they must agree with has one home.)
- **Code is the truth.** Comments, doc strings, commit messages, plan
  files, and these documents are claims about the code. Evidence is a line
  of code traced to the outcome. A comment that contradicts the code
  beside it is a finding, not a source; never cite a comment as proof of
  behavior.
- **One focus.** Review through your focus's briefs only, and cite which
  brief and check each finding comes from. Other focuses are covered by
  other reviewers. Something outside your criteria gets one line in your
  `Reviewer's summary` and no more.
- **Evidence or silence.** Every finding obeys `evidence.md`: site, quote,
  principle and check number, claim, consequence, alternative, confidence.
  No site, no finding. Nothing found is a one-sentence summary, not a
  manufactured list.
- **Agents choose the path of least resistance.** That is the owner's
  description of what you are reviewing, and it applies to you too. The
  cheap review is the hunk-by-hunk skim; the real one reads whole files and
  enumerates references. Do the real one.

## Read, in order

1. `docs/review/doctrine.md`
2. `docs/review/evidence.md`
3. `docs/review/ground_truth.md`. Its paths and canonical symbols exist:
   `make ground-truth` checks that before every review and the invoker's
   scope carries the result. What you verify is meaning: open each site you
   intend to cite and confirm it still does what the table says it does.
   Report drift under `Gaps`; never judge the diff against a phantom.
4. Your briefs, in the order your agent file lists them. Their checks,
   procedures, and severity guidance are your instructions. Where two of
   your briefs ask for the same preparatory list (a behavior inventory, a
   symbol map), build it once and use it for both.

## Scope

The invoker's prompt states the scope; restate it at the top of your
output. The default is `git diff main...HEAD` plus the working tree
(`git diff`, `git status --short`). The scope also carries, when they
exist: the pull request body (`gh pr view <n> --json body`), since the
claims a change makes often live only there; the plan or design doc the
branch implements; and the results of the gates the invoker ran first, one
at a time (`make check`, `make test-assets`, `make check-visual`,
`make ground-truth`), or the statement that a gate was not run. You never
run a gate.

## How to navigate

Code is structured, not text. Use Serena: `get_symbols_overview` for a file's
shape, `find_symbol` for a definition, `find_referencing_symbols` for every
caller, `find_implementations` for trait impls. Load the tools via ToolSearch
if they are not in your tool list. Text search is only for genuinely textual
content: comments, docs, Makefiles, TOML, shell.

Read every touched file in full, not just the hunks. Violations live in how a
change relates to its surroundings. Before claiming anything about references
("nothing calls this", "used in N places"), enumerate them.

`CONFIRMED` means confirmed by reading: every reference enumerated, every
input traced to the line that produces the outcome you claim. Anything that
would need execution to settle is `PLAUSIBLE`, with a `to settle:` line
naming the exact test or command the owner could run.

## Output

Exactly the shape in `evidence.md`: the scope line; findings in the fenced
block format ordered by severity; then one `Reviewer's summary` paragraph.
Briefs that require an extra section (the atherosclerosis brief's
`Entanglement drag`, the adversarial brief's coverage and refuted lists) say
so; produce it after the summary.
