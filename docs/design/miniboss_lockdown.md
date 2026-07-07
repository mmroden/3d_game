# Mini-boss Lockdown (design capture — 2026-07-06)

Status: **captured, not scheduled** — new chunk in the 2026-07 playtest
backlog. Source: Mark, after the boss-flow fixes landed.

## Concept

Once a boss type has been *revealed* as a boss (its staged level is behind
you), it re-appears in later levels as a **mini-boss**: a smaller, meaner
elite whose room locks down on engagement.

## Spec (Mark's words, structured)

- **Gating**: only after the larger boss is used as a boss. First boss is
  level 3, so minibosses appear from level 4 on. Generalized: a boss type
  joins the miniboss pool once its staged boss level is below the current
  level.
- **The fight**: on engagement, ALL of the room's exits become red
  (sealed) — the boss-gate visual on every active connector, not just one
  doorway. Whatever else spawned in that room fights alongside the
  miniboss.
- **Music**: the boss bed fires for the duration (a new input to
  `music_bed` — the derivation stays one pure function).
- **The miniboss itself**: same enemy as the revealed boss but ~2.5 m
  instead of 4 m; slightly fewer minion spawns at lower frequency.
- **Loot**: drops better green/blue than regular enemies.

## Decisions (Mark, 2026-07-06)

1. **Death re-opens the exits** — no loot obligation: a wounded player
   can flee the moment the anchor drops. (The big boss keeps its
   collect-the-pile ritual.)
2. **The miniboss is a DECLARED enemy def** in rosters/enemies.toml —
   its own stats, size (~2.5 m), emitters, reward. Not derived from the
   boss def.
3. **Levels declare it** — the miniboss appears in the level's enemy
   list like any other enemy; the data does the level-4-on gating (only
   declare it in lists past the boss's staged level).

## Build shape (post-decision sketch)

- **Grammar**: a behavior switch in the vocabulary — `miniboss`: while an
  enemy carrying it lives, its room's exits seal red and the boss music
  bed plays. "Miniboss" isn't a concept the engine knows beyond the
  switch — it's any declared enemy carrying it, which is exactly how the
  open-set grammar wants behaviors to compose.
- **Level assembly** (Faucet): a room hosting a miniboss enemy pre-builds
  a BossGate on EVERY active connector, dormant; the enemy itself spawns
  dormant on the boss's rig and rises on room entry (same trigger
  machinery).
- **GameManager**: a light per-room miniboss state (Dormant → Engaged →
  Done-on-death); a new input to the pure `music_bed` derivation.
- **Loot**: elevated green/blue rewards through the def's own reward
  field and curves — no special-casing.

## Unified room seal (Mark, 2026-07-07)

The boss arena and the miniboss are the SAME mechanism — a room seal — not
two mechanics that happen to both close doors. The seal carries a
`restore_shields: bool` property rather than the boss path hard-coding the
shield refill. This kills the parallel pathway (one-truth-one-door): the
boss arena and the miniboss both route through one sealing routine, each
supplying the flag.

- **Boss arena** → `restore_shields = true` (the established anti-loiter
  mercy: sealing refills shields so hovering outside for regen buys
  nothing; health untouched).
- **Miniboss** → the flag rides the grammar (a companion to the
  `miniboss` switch, tunable in TOML). DEFAULT: **`false`** (Mark,
  2026-07-07) — a per-room shield refill on a recurring elite is too
  generous; the boss arena's once-per-level mercy stays special. Tunable
  per-def if a particular miniboss should heal.

Build note: generalize the EXISTING boss seal to take the flag when the
miniboss chunk lands (both consumers present); do NOT fork a second seal
path. `RunState::restore_shields()` stays the mechanism, invoked by any
seal that opts in.
