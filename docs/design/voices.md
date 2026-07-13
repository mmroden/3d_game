# Voice Bible — Void Scavenger

Who can speak, through what channel, knowing what, lying how. Every dialogue
question ("would X say this here?") is answered against this table before any
line is written. Locked rules referenced here live in
[plot_bible.md](plot_bible.md); open decisions in
[plot_open_questions.md](plot_open_questions.md).

## Cast

### THE HANDLER (working name)
- **Identity:** Human mission control aboard the expedition ship, light-seconds
  from the belt. Was on the link when the Player's drone got trapped; has been
  trying to keep contact since.
- **Epistemic position:** Knows the human side completely — the expedition, the
  mind-meld drone program, who the Player is/was. Knows *nothing* about the
  Other except telemetry. Cannot see or hear what the Player experiences inside
  the simulation; reads only neural/link data ("your readings just spiked —
  what's happening in there?").
- **Channel:** The pre-existing mind-meld uplink. Full two-way language from
  minute one. Degrades across meta-progression (crossing-curves arc, below).
- **Truth status:** Honest but ignorant. Late-game: possibly counterfeit (Q3) —
  the Other, having eavesdropped on this channel to learn human language, may
  learn to imitate it.
- **Arc:** Strong → intermittent → dark (and/or compromised). The tutorial-nudge
  voice of run one is the handler doing their job.
- **Gates:** Signal strength modulated by within-run depth; content gated on
  `runs_completed` like everything else.

### THE OTHER
- **Identity:** Dormant defensive AI, remnant of the dinosaur civilization.
  Zero-g-native cognition (bible rule 8). Sincere, alien, very old.
- **Epistemic position:** Knows its entire history but cannot yet *say* it;
  knows the Player only through diagnostics. Hears the handler channel (it is
  its Rosetta stone for human language — this is diegetically why its voice
  de-garbles). The handler cannot hear it.
- **Channel:** The simulation itself — levels, enemies, geometry are all its
  speech. Voice-over emerges at M1, heavily garbled. The end-of-level choice
  interface is its inbound channel, growing across meta-tiers (2 crude glyphs →
  4 phrased prompts). The interface's growth is visible sync progress.
- **Truth status:** Sincere but alien; its history is unknowable in full (bible
  rule 2). It does not lie; it is *incommensurable*, which reads as unreliable.
- **Arc:** Garble → clarity (processing tiers); stimulus-response → something
  approaching conversation, never quite arriving.

### THE PLAYER
- **Identity:** Human survey pilot, consciousness trapped in the Other's
  protocol. Amnesiac about the launch and the trap; memory returns as sync
  deepens, corroborating or contradicting the handler's account.
- **Epistemic position:** Knows only what they experience. Unreliable about
  their own past (amnesia).
- **Channel:** Speaks ONLY on the handler channel (pending Q2). Never voiced at
  the Other — that channel doesn't carry Player speech; the choice interface is
  the only Player→Other door (bible rule 6). The Player was never mute: the
  Other just couldn't parse the channel where they'd been talking all along.
- **Arc:** Grounded wtf-surrogate early; as the handler link dies, the Player's
  voice has nowhere to go — the Player falls silent exactly when the Other
  becomes fluent.

## Audibility matrix

| speaker ↓ / hearer → | Player | Handler | Other |
|----------------------|--------|---------|-------|
| **Player** (voice, handler-link) | — | yes (lagged) | overheard, unparsed early → parsed later |
| **Player** (choice interface) | — | no | yes |
| **Handler** | yes (lagged) | — | overheard: its language-learning corpus |
| **Other** | yes | **no** — telemetry noise only | — |

The handler never hears the Other. Dramatic irony is structural: the handler
reacts to spiking readings while the player hears the cause.

## Channel physics: the lag tell

The real handler is light-seconds away. **Real handler lines always arrive with
a 2–3 second response lag. Anything answering instantly is inside the
simulation.** Never explained in-game. This is the fairness mechanism for the
counterfeit-handler beat (Q3): attentive players can catch the fake because it
doesn't obey physics. Establish the lag consistently from the first minute of
run one or the tell is worthless.

## Production

- **Casting:** two actors (Handler, Other) + Player if Q2 resolves to voiced.
  Handler-counterfeit is the handler's actor through the Other's processing
  chain — the near-match is the point.
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
