# Plot Bible — Void Scavenger

> **STATUS (2026-08-30):** The "locked" framing below is **rescinded** — Mark's
> review: this document was full of unratified assumptions presented as canon,
> invented names, and plot steered onto rails he didn't choose. Entries stamped
> "(Mark, …)" are his design; everything else is Claude proposal awaiting his
> redline. Build nothing new on the unstamped parts without his word. Companion
> docs (voices.md, plot_open_questions.md, script_guide.md) inherit this status.

Single source of truth for narrative design *once ratified*. Anything unsettled
lives in [plot_open_questions.md](plot_open_questions.md). Voice cast, channels,
and script format live in [voices.md](voices.md).

## Premise

The Player is a human consciousness trapped by the Other — a long-dormant AI, the
remnant of an intelligent dinosaur civilization that colonized a fifth planet (now
the asteroid belt) 65 million years ago, fought an interplanetary war with Earth,
and died in the exchange that ended with the Chicxulub impact. The Player was part
of a human expedition surveying the belt via mind-melded drone when they tripped
the Other's defensive countermeasure. The "game" the Player experiences is the
Other's communication protocol: it reads minds by presenting challenges and
observing responses. Every level is a diagnostic. Every death is data.

The Player does not remember the launch, the expedition, or the trap. Memory
returns as synchronization deepens.

## Locked invariants

These are design rules, not suggestions. A change to any of them is a plot-bible
amendment, not a local scripting decision.

1. **Drones only.** The Other can manifest only drones — the same way a chessboard
   contains no piece with an area attack. It cannot place an organic in the arena.
   Any deviation from this rule is a major plot event, never casual enemy variety.
2. **No true ending.** The full truth of the Other's history is unknowable. No
   dialogue path, meta-tier, or completion state yields a definitive account.
3. **The invariant line.** At the end of run one, the Other says:
   *"Baseline established. Continuing to develop contact. Iterating."*
   This line has zero variants, zero gating, zero flavor. It plays identically
   regardless of anything the Player did. It is the fixed anchor that makes every
   subsequent change legible.
4. **Run one is silent.** No choice interface, no Player agency in dialogue, until
   the first complete clear. The absence of interactivity in run one is
   retroactive horror, not a missing feature.
5. **First-clear reward = the trap reveal.** The reward for winning run one is
   knowledge: you are a prisoner of something distinctly alien. Not a weapon, not
   a level. The mismatch between mechanical achievement and existential payoff is
   deliberate.
6. **Dialogue is stimulus, not conversation.** Player choices at level breaks
   (2–4 fixed prompts, never open text) change the *texture* of what follows —
   enemy density, geometry mood, voice framing — not which lore unlocks. The Other
   acts on questions rather than answering them. Lore is gated on the meta-clock
   alone, so choices can never dialogue-tree their way to the truth (see rule 2).
7. **Run = session.** Each run is a fresh session for the Other: it re-derives the
   Player from persistent "memories" (the meta-state) but starts from scratch in
   working context. This is why every run begins in the chapter-1 confusion
   (floating furniture, broken geometry) regardless of meta-progress.
8. **6DOF is diegetic.** The Other's makers were zero-g-native cybernetics.
   Gravity is the alien concept *to it*; gravity-bound artifacts (furniture) are
   what it struggles to render. The script must not conflate "I am confused by
   you" with "gravity is foreign to me" — they are different sizes of
   estrangement, and both are in play.

## The reveal engine: two clocks, two counters

**Counters contract** (no shared meaning, ever):

- `deaths_total` — drives run-local difficulty/economy only. **Zero narrative
  authority.** Deaths before the first clear are narrative no-ops.
- `runs_completed` — the sole gate for story content: the choice interface, the
  invariant line, chapter unlocks, lore tiers.
- If a third clock is ever needed (e.g. post-first-clear deaths feeding a
  fragment pool), it is a **new named counter**, never a repurposed existing one.
  Every script line must be traceable to exactly one gate.

**Two nested clocks:**

- **Within-run** progress controls *signal quality* — how garbled the Other
  sounds, how coherent the geometry is. Resets each run (rule 7).
- **Across-run** progress (`runs_completed` and milestone flags) controls
  *content depth* — what the Other is able and willing to say at all.

A line's content is authored once; its garble level is a processing tier applied
in the mix (see voices.md), so the script stays linear.

## Environments / chapters

Maps onto the existing four-chapter scaffold
([game_plan.md](../architecture/game_plan.md) Phase 6); chapter identities
**and level counts** are redefined by this document (base 6 per calibration
chapter with compression — see meta-topology; game_plan's 10-per-chapter/40
total is superseded and that doc needs amending when Phase 6 is built).

| Ch | Environment | What it is diegetically |
|----|-------------|------------------------|
| 1 | Broken sci-fi geometry, floating furniture, drone enemies | The Other's native cognition, crudely furnished with unparsed human artifacts |
| 2 | New cell geometry (3m cubic cells), environment-matched enemies | The Other's cognition, calibrating — first fruits of synchronization |
| 3 | Prosaic human surroundings — apartments, schools, libraries (Mark, 2026-08-30) | Why these exist out here is Mark's to reveal; Control's befuddlement peaks in this chapter |
| 4 | An actual asteroid surface; drones protecting the last meteor launcher still aimed at Earth (Mark, 2026-08-30) | The endgame board; how it relates to the simulation chapters is Mark's call |

Chapter gating model (frontier vs. full ladder) is open (Q1).

## Meta-topology — how the map changes across runs

- **Compression (ch 1–2).** Base 6 levels per chapter on run one; as
  `runs_completed` grows, the calibration chapters shrink — 6 → 5 → 4 → 3
  (floor 3; economy interaction is Q8). Diegetic: protocol overhead falls as
  sync improves. The shrinking level count is a UI-free sync gauge the player
  reads directly. Gated on `runs_completed` only (counters contract).
- **Relative anchors (authoring rule).** Because level counts vary, no script
  trigger, beat, or design note may reference an absolute level number.
  Anchors are chapter-relative: `ch1_boss`, `ch2_midpoint`. M1 is "the
  chapter-1 boss falls," not "level 3."
- **Rooms (ch 3).** The house chapter draws from a pool of memory-places
  (living room, bedroom, classroom, hangar, …). Rooms unlock in a fixed
  sequence — each first visit is a MILESTONE beat — and only once the whole
  pool has been indexed does level selection randomize (diegetic: the Other
  now draws freely from a completed index). Sequence order and the hangar's
  meaning are Q9.
- **Sites (ch 4).** The ship reveals in stages across visits: interior → war
  evidence → launch bays → the targeting system and the trajectory plot (the
  peninsula) → the reveal that this installation is **one of many
  meteor-throwing sites**. The many-sites reveal feeds M5 and the
  outside-the-simulation stakes (Q10).

## Beat skeleton (proposed shape, content TBD)

Five milestone beats plus three ambient pools:

- **M1** — Chapter-1 boss falls: first contact ping. *"Initial synchronization
  established; further challenges necessary."* Heavily garbled.
- **M2** — First complete clear: the invariant line (rule 3). Trap reveal.
- **M3** — First entry into the house: escalation. It is inside your memories
  now. Each subsequent room's first visit is its own sub-milestone (see
  meta-topology); the terminal room closes the amnesia arc (Q9).
- **M4** — First entry into the ship: inversion. You are inside *its* memories.
  The site reveal ladder (meta-topology) stages across return visits.
- **M5** — First full clear at max frontier: the ask — whatever the Other actually
  wants (content TBD; constrained by rule 2).
- **Pool A** — per-chapter ambient lines (tone-variant tagged per last choice).
- **Pool B** — death fragments, post-first-clear only (needs its own counter).
- **Pool C** — handler channel: the human side, the forgotten expedition, the
  mind-meld program (see voices.md).

NG+ frame: the Other archives this model of the Player and instantiates a fresh
session. Its line is the invariant line, verbatim (rule 3) — the bookend costs
nothing and lands only because the line never varied.
