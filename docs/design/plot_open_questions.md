# Plot — Open Questions

Pending narrative-design decisions. Each entry: the question, the live options,
a recommendation. These are Mark's calls. When one is decided, its answer moves
into [plot_bible.md](plot_bible.md) or [voices.md](voices.md) and the entry is
deleted here.

## Q1 — What does one run traverse?

- **A. Full ladder:** a run can traverse all four chapters; first clear = all 40
  levels. Players see the childhood house *before* the trap reveal, which the
  house itself already implies — the M2 reveal lands redundant. First clear may
  be many hours in, all of it silent (bible rule 4).
- **B. Frontier:** run one's frontier is chapters 1–2; first clear = ch-2 boss,
  triggering M2. Each milestone extends the frontier (house in run 2+, ship
  later). Diegetic fit with run-as-session (rule 7): each session re-traverses
  earlier calibration faster, then pushes deeper. Hook lands in the first hour.
- **Recommendation: B.** The ordering (learn you're trapped → *then* it goes
  into your memories) only works under B, and B structurally motivates replays.
- Sub-question under B: can a maxed-frontier player full-clear 1→4 in one run
  (they should be able to — that's M5)?
- Note: compression, room sequencing, and the ch-4 reveal ladder (bible,
  meta-topology) all assume repeated arrivals across runs and work under
  either model — the discriminator remains reveal *ordering* (house before or
  after the trap reveal).

## Q2 — Is the Player voiced?

- **A. Silent + choice glyphs only.** Cheapest; maximal player projection;
  weakest grounding — no voice owns the "wtf" the player is feeling.
- **B. Voiced, handler channel only** (see voices.md). Sparse reactive lines;
  never speaks at the Other. Buys the grounded surrogate and the late-game
  falling-silent inversion. Costs: third VA role; fixes protagonist identity.
- **C. Unvoiced, but the Other quotes the Player's thoughts back** ("You are
  thinking: *is this real*."). Player 'voice' with zero casting; deeply creepy;
  gimmicky if overused. Composable with B as an early-tier device.
- **Recommendation: B**, with C sprinkled as an Other-side device regardless.

## Q3 — Handler authenticity arc

- **A. Real throughout.** Simplest; keeps the anchor pure; loses the paranoia
  beat entirely.
- **B. Real early, counterfeited late.** The Other learns the channel by
  eavesdropping (its de-garbling mechanism anyway), and once fluent can imitate
  it. Grounding is established *then* threatened; "is this still them?" never
  fully resolves (consistent with bible rule 2). The lag tell (voices.md) keeps
  it fair.
- **C. Mimicry from the start** (handler was never real). Strongest single
  twist; sacrifices the grounded human anchor, which is the reason the handler
  exists.
- **Recommendation: B.**

## Q4 — Chapter 4 = the Other's memory?

Proposal: sync leakage becomes bidirectional; ch3 is the Player's memory
(house), ch4 the Other's (the war ship — launch bays, the trajectory plot with
a familiar peninsula). The Chicxulub confession is walked through as level
content, not delivered as monologue; the Other's VO becomes reluctant
commentary. Alternative: ch4 is just a fourth arena and the confession stays
in VO. **Recommendation: adopt the proposal** — it converts the biggest lore
dump into environmental storytelling and gives ch3/ch4 a mirrored meaning.

## Q5 — Does the Other jam or eavesdrop the handler link?

- **Eavesdrop:** it *wants* the handler talking — training data. Jamming/
  counterfeiting begins only once fluent (feeds Q3-B). Its treatment of the
  link becomes a legible measure of its progress.
- **Jam from the start:** simpler, but severs the mechanism by which its voice
  plausibly de-garbles, and wastes the crossing-curves arc.
- **Recommendation: eavesdrop.** (If Q3-B is adopted, Q5 is effectively decided.)

## Q6 — Do organics ever appear?

Bible rule 1 says any organic is plot, not variety. Is that gun ever fired —
e.g. a single organic manifestation at M5 as the Other's maximum-effort act of
self-portraiture — or does the constraint hold absolutely (the Other *cannot*,
ever, and its inability is the point)? No recommendation yet; both are strong.
Decide before anyone ships a "cool organic enemy" casually.

## Q8 — Compression vs. the economy

Compression (bible, meta-topology) means veteran runs reach the house with
fewer levels behind them — fewer caches, fewer shop visits, thinner builds.
Someone has to own this contract:

- **A. Compensate to parity:** denser caches / a currency multiplier keyed to
  compression tier, so expected resources at each frontier point stay
  constant. Diegetic: the Other has learned what the subject needs and wastes
  less of its time.
- **B. Deliberately leaner:** compressed runs are faster *and* poorer;
  veterans are expected to need less. Sharper skill curve, riskier tuning.
- **Recommendation: A**, at least as the default until playtesting says
  otherwise — B silently couples narrative progression to difficulty spikes.
  Whichever wins, the rule belongs next to SpawnParams/economy config, keyed
  to the same `runs_completed` gate as compression itself.

## Q9 — Room order and the hangar's meaning (ch 3)

The room pool (living room, bedroom, classroom, hangar, …) needs an unlock
order and a reason for it.

- **Proposal:** the sequence runs chronologically through the Player's life —
  the Other indexing the biography from childhood forward. The hangar is then
  not a childhood room at all but the *newest* memory: the expedition, the
  launch, the trap. Making it the terminal room means the amnesia arc
  resolves environmentally — the Player fights through the place they forgot
  and remembers — mirroring ch 4's walked confession. Handler Pool C lines
  coordinate (Maya reacting to the Player suddenly remembering).
- Alternative: authored thematic order with no biographical logic; hangar is
  just a cool room. Cheaper to think about, wastes the arc.
- Texture note regardless: rendered text (classroom!) garbles exactly like
  the Other's voice — same fluency ladder, same failure mode.
- **Recommendation: the proposal.**

## Q10 — What does "one of many sites" set up?

The ch-4 reveal ladder ends on: this installation is one of many
meteor-throwing sites. Decide what that's *for* before any T4 lines are
written, because it constrains M5 (the ask):

- **M5 as warning:** the Other wants humanity told — other sites are still
  armed, still dormant, and human belt-mining is tripping them. The Player's
  own capture becomes proof of concept.
- **M5 as plea:** it wants the sites found and disarmed — it has had 65
  million years to regret the system it was part of.
- **M5 as inventory:** darker — it is checking whether the network still
  answers, and the Player can't tell which of the above it is (fits bible
  rule 2).
- Cross-channel corroboration regardless of choice: Maya reports other
  survey drones going dark across the belt. The Player assembles the
  many-sites truth from two voices that cannot hear each other.
- **Recommendation:** decide the M5 *shape* now (warning/plea/inventory or
  deliberately indistinguishable); the deliberately-indistinguishable option
  is the only one that preserves rule 2 all the way to the last line.
