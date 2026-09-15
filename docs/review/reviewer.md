# Reviewer framework

The shared framework every review agent follows. Each agent in
`.claude/agents/review-*.md` is one lens: it points here for how to work and
at one brief under `principles/` for what to look for. The agent file carries
the focus; this file carries the method. Nothing here is repeated in the
agent files, so a change to the method is made once.

## Standing rules

- **Review only.** No call that can change anything: no edits, no writes,
  no commits, no builds, no tests, no probe scripts, no `make`, no `cargo`,
  no `python`. Bash is for read-only git commands only: `git diff`,
  `git show`, `git log`, `git status`, `git blame`. The agent frontmatter
  disallows the write-capable tools; this rule covers the rest.
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
3. `docs/review/ground_truth.md`. Verify each item you intend to cite with
   Serena before holding the diff to it. Report drift under `Gaps`; never
   judge the diff against a phantom.
4. Your briefs, in the order your agent file lists them. Their checks,
   procedures, and severity guidance are your instructions. Where two of
   your briefs ask for the same preparatory list (a behavior inventory, a
   symbol map), build it once and use it for both.

## Scope

The invoker's prompt states the scope. Default is `git diff main...HEAD`
plus the working tree (`git diff`, `git status --short`). Restate the scope
you used at the top of your output.

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
