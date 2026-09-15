# Review log

One file per collated pre-merge review, `<YYYY-MM-DD>-<short sha>.md`, filed
verbatim by the invoker after `mark-review` returns, with an `## Outcomes`
section appended after the owner's triage: one line per finding id,
`fixed <commit>`, `won't fix: <reason>`, or `deferred: <where tracked>`.

The next review reads this log (`docs/review/evidence.md`, consolidation
rule 3), so a decision is made once. The log is also the record of what a
real finding in this repo looks like; an entry worth teaching from is
promoted by the owner into a brief's "Repo history to hold against", never
automatically.
