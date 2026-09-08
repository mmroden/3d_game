# Script Guide — how to write and produce the VO

Companion to [voices.md](voices.md) (who may speak, on what channel) and
[plot_bible.md](plot_bible.md) (what may be revealed, when). This doc is *how
to write the lines*. Worked examples: [script/00_voice_test.md](script/00_voice_test.md).

## What this script is (and isn't)

A game VO script is not a screenplay read top to bottom. It is:

- **Pools** — sets of interchangeable lines fired by triggers (level start,
  boss death, player death, idle), filtered by gates and tiers. 90% of the
  script by line count.
- **Scenes** — the five milestone beats (M1–M5, see bible). Play once per
  save. The only screenplay-shaped parts, and even these must survive the
  player flying away mid-line.

Everything is player-paced and interruptible. No line may require the player
to stop playing to receive it.

## Repetition classes

Every line gets exactly one class. This is the discipline that keeps a
50th-hearing from curdling.

| Class | Plays | Rules |
|---|---|---|
| SIGNATURE | Once per save, ever | The invariant line, first "I", the ask. May be long. Zero variants (bible rule 3 for the invariant line). |
| MILESTONE | Once per save | Scene lines within M1–M5. May carry exposition. |
| RARE | Weighted, seldom | Character color, lore fragments. Medium length. |
| COMMON | Constantly | ≤ 8 words. 3–5 variants each. Never a joke — jokes die by the tenth hearing. Commons are weather, not events. |

## Per-voice diction rules

### The Other — grammar IS the garble

The audio de-garbles across tiers (post-processing), but the *language*
de-garbles too. Each meta-tier has a hard grammar ruleset. Never write below
or above the current tier — the ladder is the character arc.

| Tier | Unlocks (meta-gate) | Grammar license |
|---|---|---|
| T0 | pre-M1 | No voice. The levels are the speech. |
| T1 | M1 → first clear | Nominal and imperative only. No pronouns, no articles, no questions. "Baseline established. Continuing to develop contact. Iterating." |
| T2 | runs_completed ≥ 2 | Third-person designation ("the subject"). Present tense only. No contractions, no idiom. Questions permitted. |
| T3 | house frontier | Second person. Comparatives, measurement metaphor. Still no past tense — it cannot yet speak about history. |
| T4 | ship frontier | First person — **the first "I" is a SIGNATURE beat.** Past tense unlocks, and with it, narration of its own history. Contractions appear. Its first idioms are slightly dated, learned from the handler. |

Hard rules at every tier:

- Never broken English. Always **precise English from a wrong ontology.** It
  does not make errors; it makes category mistakes, exactly stated. It treats
  emotion as instrumentation ("Your fear has a shape. It is efficient.").
- It does not lie and it does not comfort. It reports.
- It measures things no one measures and ignores things everyone measures.
- Garble survivability: T1–T2 lines must still communicate when 60% masked.
  Put the load-bearing nouns at both ends of the line; test by covering
  random halves.

### The Handler — comms discipline, eroding

- Astronaut/ATC register: procedure words (*copy, say again, stand by,
  good copy*), callsign discipline (`[CALLSIGN]` — Mark fills it; invented
  callsigns were an overreach). The tutorial is not a tutorial voice — it
  is a control tower running a sortie, and the sortie happens to teach the
  controls.
- **The lag is a writing rule, not just audio:** the handler never reacts in
  real time. Every handler line is retrospective — responding to what the
  player did three seconds ago. Never write the handler interjecting into a
  live event.
- The arc is protocol erosion. Early: checklists and confirmations. Middle:
  procedure words drop off. Late: "Command wants me to read you the
  continuity script. To hell with the script." Mark each protocol break —
  they are the handler's milestones.
- The handler is the only voice permitted humor, and only the gallows-lite
  kind that people in control rooms actually use.
- Control sees what the Player sees and hears nothing back (Mark,
  2026-08-30; voices.md). He reacts to what he watches and never gets an
  answer — every line is half a conversation. Dramatic irony lives in the
  missing half; never violate it for convenience.

### The Player — talks, but is never heard (2026-08-30)

- The Player keys the mic and calls base; the Other swallows the path
  (voices.md). In the mix the Player is unvoiced — the audience hears the
  consequences of the Player speaking, never the speech.
- Write around the silence: Control's non-answers and the Other's learned
  vocabulary are where the Player's side of the conversation shows up.

## Line format

One line per ID, one gate per line, tier tagged, variant only where earned:

```
[HND-C1-T04]  class:COMMON  trigger:level_start  gate:always  chapter:1
  "Vitals green. Take it slow in there."
  VA: routine, third time today saying this.

[OTH-M2-01]   class:SIGNATURE  trigger:run_complete  gate:runs_completed==1
  "Baseline established. Continuing to develop contact. Iterating."
  VA: no menace, no warmth. A system logging a result.  tier:T1
```

Prefix = speaker (HND/OTH/PLR). `variant:` suffix keys off the Player's most
recent choice (bible rule 6) and reuses the same slot — never new content per
branch.

Triggers are always **chapter-relative** (`ch1_boss`, `ch2_midpoint`,
`room_first_visit:bedroom`) — never absolute level numbers. Chapter lengths
compress across runs (bible, meta-topology), so an absolute level number is a
bug.

## Process

1. **Voice test** — [script/00_voice_test.md](script/00_voice_test.md).
   Mark redlines until each register sounds right. Nothing is mass-drafted
   before registers lock. This is the cheapest point to change a voice.
2. **Beat cards** — one card per milestone: what the player must *feel*,
   what information enters, what question is planted. No dialogue yet.
3. **Line budget** — count lines per pool before writing them. VO cost
   scales with word count; the budget makes scope a decision, not a
   discovery. Rough shape: handler run-one pool ~25, ambient pools ~15 per
   chapter, milestones ~10–20 each, death fragments ~20, commons ~40.
   Order 300–400 lines total — one comfortable studio session per actor.
4. **Draft pool by pool**, gated on beat-card approval. Read every line
   aloud; anything you stumble on, an actor will too.
5. **Temp VO** — TTS renders wired into the actual triggers, so pacing and
   repetition are felt in-game before paying for a studio. Half the commons
   will die here. That is the point of temp VO.
6. **Freeze IDs → record clean → post-process tiers.** Actors never perform
   garble (voices.md). After freeze, line IDs are append-only.
