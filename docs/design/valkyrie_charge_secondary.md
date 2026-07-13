# Valkyrie Charge Secondary (design capture — playtest 2026-07-06)

Status: **captured, not scheduled** — revisit when the playtest backlog reaches
the Valkyrie chunk. Source: Mark's playthrough notes + follow-up spec.

## Core change

The Valkyrie cannon leaves the primary trigger entirely. Today it co-fires on
`FIRE` with its own slower cooldown (ship_controller); instead it becomes the
**left trigger** weapon with a stored-charge model.

## Charge model

- A row of charge bars (base: **3**) fills slowly over time; the fill runs
  *across* the bars — one continuous charge that fills bar 1, then bar 2, etc.
- Each filled bar holds one projectile. Pressing the left trigger fires the
  stored projectiles (one per filled bar).
- Base fill time: **1.5 s per bar** (1.0 s also floated — tune in playtest).

## Upgrades (both blue / components, run-scoped)

- **More bars**: base price **10k**, then exponential growth matching the
  existing exponentially-priced shop items.
- **Faster refill**: −0.1 s per bar per level, floored at **1.0 s/bar**;
  priced **5k** on the same exponential ladder (confirmed 2026-07-06).

## Audio

- A slowly rising charge sound as each bar fills, continuing across the bars
  (pitch/intensity tracks total charge).
- (Shield Surge use-sound is separate and ships in the quick-fixes chunk.)

## Related but undecided

- "Valkyrie blast sends furniture tumbling" — physics-impulse AoE on the shot.
  Open question: spectacle only, or do tumbled props damage drones (would need
  a physics-damage pathway that doesn't exist yet)?

## Open questions for design review when scheduled

- Fire semantics: dump all stored projectiles per press vs. one per press.
- Does the charge pause/reset while bars are being spent?
- HUD placement of the bar row (inside the SBS safe-area band).
