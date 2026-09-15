# Voice Bible — Void Scavenger

> **STATUS (2026-08-30):** This doc previously overstated its authority.
> Sections quoting Mark or stamped "(Mark, 2026-08-30)" are his design;
> everything else is unratified Claude proposal awaiting his redline.
> Invented character names were an overreach: the only permitted labels are
> role-addresses ("Control") and the `[CALLSIGN]` placeholder Mark will fill.

Who can speak, through what channel, knowing what, lying how. Every dialogue
question ("would X say this here?") is answered against this table before any
line is written.

## Cast

### CONTROL — the man in the chair (Mark, 2026-08-30)

Quoted phrases below are Mark's words.

- **Identity:** A man — Mark's word, and Mark voices the entire cast (budget
  and design agree). Mission control for a military exploration/scout
  flight. He has no name: "Control" is a radio address, not a name, and it
  stays that way.
- **Epistemic position:** "They see what the player sees, but they can't
  hear what the player says, because the Other has prevented that kind of
  signal passage." He watches the Player's own feed and gets nothing back —
  every transmission is half a conversation. He knows the mission cold;
  what he cannot work out is why the Player "can't just come back to base,"
  why they're trapped in this weird situation.
- **Register & arc:** "A pilot's control tower" — scout-mission jargon,
  dense procedure words, straight-laced. Then "increasingly off-book" as
  the repetition sets in and the befuddlement mounts. The erosion is
  countable: procedure-word density per line falls planet by planet.
  Befuddlement peaks on planet 3, watching the Player fly through prosaic
  surroundings — apartments, schools, libraries.
- **The RTB thread:** Initially he calls for the Player to return to base —
  and watches the Player fail to comply, unable to distinguish "can't hear
  me" from "can't fly it." The order decays into ritual with everything
  else.
- **Truth status:** Honest but ignorant. (Whether this channel is ever
  counterfeited is an open question — Q3.)

### THE OTHER
- **Identity:** Dormant defensive AI, remnant of the dinosaur civilization.
  Zero-g-native cognition (bible rule 8). Sincere, alien, very old.
- **Epistemic position:** Knows its entire history but cannot yet *say* it;
  knows the Player only through what it observes. It hears the whole
  channel — and it is the reason Control hears nothing: the Player's
  outbound signal terminates at the Other ("the Other has prevented that
  kind of signal passage" — Mark, 2026-08-30). Every call the Player keys
  toward base is received by the computer instead: the Player is
  "unknowingly corresponding" with it. The channel is also how it learns
  human speech — diegetically why its voice de-garbles.
- **Channel:** The simulation itself — levels, enemies, geometry are all its
  speech. Voice-over emerges at M1, heavily garbled. The end-of-level choice
  interface is its inbound channel, growing across meta-tiers (2 crude glyphs →
  4 phrased prompts). The interface's growth is visible sync progress.
- **Truth status:** Sincere but alien; its history is unknowable in full (bible
  rule 2). It does not lie; it is *incommensurable*, which reads as unreliable.
- **Arc:** "The computer's garbled voice becomes ungarbled over time and as
  the player plays further" (Mark, 2026-08-30). As the garble lifts, the
  voice underneath is revealed to be Control's — it learned speech from the
  only speech on the wire. Its early vocabulary is the channel's
  most-repeated material (radio checks, the RTB order, the callsign); its
  first wrongness is placement, not wording.

### THE PLAYER
- **Identity:** Human survey pilot, consciousness trapped in the Other's
  protocol. Amnesiac about the launch and the trap; memory returns as sync
  deepens, corroborating or contradicting the handler's account.
- **Epistemic position:** Knows only what they experience. Unreliable about
  their own past (amnesia).
- **Channel:** The Player talks — keys the mic, calls base — and none of it
  arrives where it is aimed: the Other swallows the path (Mark,
  2026-08-30). In the mix the Player is unvoiced ("basically two voices"):
  we hear the consequences of the Player speaking — Control's non-answers,
  the Other's learning — never the speech itself.
- **Arc:** The channel asymmetry inverts across the game: run one opens with
  a fluent handler and a mute Other; the endgame closes with a dark handler
  and a fluent Other — in the same voice. The Player is the still point the
  two curves cross through.

## Audibility matrix

| speaker ↓ / hearer → | Player | Control | Other |
|----------------------|--------|---------|-------|
| **Player** (mic) | — | **never arrives** — the Other blocks the path | yes — sole receiver of everything the Player sends |
| **Control** | yes | — | overheard: its entire language corpus |
| **Other** | yes (garble → clarity) | open — does Control ever hear it? | — |

Control also watches the Player's visual feed — sight down, sound none
(Mark, 2026-08-30). Dramatic irony is structural: Control describes what he
sees and never learns what the Player would say back.

## Channel physics: the lag tell

The real handler is light-seconds away. **Real handler lines always arrive with
a 2–3 second response lag. Anything answering instantly is inside the
simulation.** Never explained in-game. This is the fairness mechanism for the
counterfeit-handler beat (Q3): attentive players can catch the fake because it
doesn't obey physics. Establish the lag consistently from the first minute of
run one or the tell is worthless.

## Production

- **Casting (2026-08-30; supersedes the two-actor note and model A in
  [01_two_voice_test](script/01_two_voice_test.md)):** ONE performer voices
  everything recorded — the handler and the Other. The Player is unvoiced.
  Diegetic before it is economic: the Other's entire language corpus is the
  handler channel, so when its audio de-garbles, the voice underneath must
  be the handler's. De-garbling is a slow unmasking, performed in the
  player's ear rather than told.
- **The tell-decay ladder.** What distinguishes the two voices erodes on
  schedule: early — garble + grammar tier; mid — grammar + the lag; by T4 —
  the lag alone. A counterfeit beat, if fired (Q3), plays against a player
  whose learned tells have all expired but one. Establish the lag from
  minute one or none of this is fair.
- **`corpus:` tags.** T1–T2 Other lines cite the handler line IDs their
  vocabulary came from — nothing in its mouth before the handler put it
  there. The first un-sourced word (T3) is a beat in itself.
- **Garble is post-processing, not performance.** Every line is recorded clean,
  once. Degradation tiers are applied in the mix, keyed to within-run signal
  quality and meta-tier. The VA never records "garbled" takes.
- **Script line format:**

  ```
  [OTH-C2-B01]  speaker:OTHER  gate:runs_completed>=2  trigger:boss_death
                chapter:2  tier:garble2  variant:hostile?
  ```

  One content per line ID. Tone variants (`variant:`) exist only where tagged,
  selected by the Player's most recent choice — same content pool, different
  framing (bible rule 6). Every line traceable to exactly one gate.
