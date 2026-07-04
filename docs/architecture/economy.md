# The Economy

> **Status:** implemented. This doc is ground truth for the purchase/persistence
> design; the code's single sources are named per section.

## The one rule

**Nothing is given away, and nothing is auto-awarded.** The only in-level
reward is a dropped currency cache the player must fly through. Everything
else is bought at the between-level shop.

## Currencies

| | Blue — Components | Green — Organics |
|---|---|---|
| Type | `Account<Component>` | `Account<Organic>` |
| Earned | Blue caches dropped by kills (tiered per `EnemyType::reward`) | Green caches placed at loot spawns (`ORGANIC_CACHE_AMOUNT` each) |
| Buys | Stat upgrades, laser levels, extra lives | Permanent unlocks: radar, recon map, Valkyrie cannon, hulls |
| Lifetime | The run — zeroed on run-over | Forever — profile-persistent |

One pickup mechanism serves both: `CurrencyCache` (void-nodes), kind-tinted
by `CurrencyKind::glow_color`. Blue caches are Faucet tier-1 pooled one per
enemy under the LevelManager; green caches are placed live under their rooms
by the same `drop_at` activation path.

## The shop (`void-logic/src/shop.rs`)

The purchase authority. `offers(&RunState)` prices the catalog; `purchase`
validates, spends through the typed accounts, mutates, and returns a
`Receipt` the shell routes. The shell never mutates prices or balances.

One screen, two sections (blue then green), typed item ids
(`ShopItemId::from_id`): stat kinds 0–4, Laser 5, ExtraLife 6, unlocks 7+.
Every offer carries a `detail` line derived in void-logic — the stat rows
show the owned multiplier now→next, the laser its damage jump, the life its
count, the unlocks their capability blurb — so the shell only renders what a
purchase does; it never computes it.

**Price shapes** (tests pin the shape — monotonic growth, ratchets, tier
ordering — never absolute balance; the numbers live in one consts block per
file for retuning):

- Stat upgrades: fixed +10% multiplier; cost `BASE[kind] × 1.6^owned`
  (Damage 5k, FireRate 4k, MaxHealth 3k, Thrust 2k, Rotation 1.5k). The
  growth curve is the soft cap. Damping ("Stability") and projectile speed
  ("Beam Focus") are retired — fixed base values, no upgrade kind.
- Laser: the existing `LaserLevel::upgrade_cost` ROYGBIV table, unchanged.
- Extra life: `10k + 5k × lives_purchased` (linear — owner's call
  2026-07-04: a life stays reachable) — keyed to lives *bought* this run,
  so dying never discounts the next one.
- Surge charge: 5k flat (blue), stocked only once the green Shield Surge
  item is owned; rack caps at 9. The item arrives with three charges, and
  restocks to three wherever it would be handed over fresh (grant,
  run-over, profile-only load).
- Unlocks: flat organics prices — Radar 300, Recon Map 500, Valkyrie 800,
  hulls from the `ShipType` spec (Talon 3 000, Hive 4 000, Reaver 5 000 —
  repriced 2026-07-03; hulls are long-arc purchases).

**The green ladder** (`PermanentUnlocks::available`): the green section is a
tutorial spine, not a menu. Radar is always offered; the Recon Map needs the
radar; the Valkyrie needs the map; no hull is offered (or purchasable — the
shop refuses `NotPurchasable` even with the balance) until the Valkyrie is
owned. Early game therefore shows exactly one next capability. The Valkyrie
itself is the Vanguard's heavy second cannon: a fan of aim-assisted homing
bolts on a slow clock (`armament::valkyrie` — the cannon's job is
CONNECTING when raw aim can't), damage scaled off the equipped laser,
pushed to the ship as `set_valkyrie_owned` on the purchase receipt and on
every player sync.

**Branches** hang off the spine without gating it: the Route Scanner (600)
and Threat Tracker (1 200) open with the Recon Map; the Shield Surge
(1 000) opens with the radar. Every green purchase confirms through an
info screen first — `Unlock::blurb()` (what it does) plus
`Unlock::trigger_hint()` (how to use it), carried on the offer's detail
line as `blurb | trigger` and rendered by ShopUI before the buy.

**Save & Exit** (shop row after Continue): banks the run and returns to
the menu. The save IS the level start, so a between-levels exit advances
first (Continue resumes at the next level, purchases included); a
between-lives exit keeps the current level.

## Lives

`RunState.lives` starts at 1; extra lives are blue, per-run.

| | Life loss (`apply_life_loss`, lives > 1) | Run over (`apply_death_penalty`) |
|---|---|---|
| Lives | −1 | reset to 1 (ratchet resets too) |
| Ship | health + shield restored | restored |
| Components | **kept** | zeroed |
| Bought upgrades | **kept** | cleared ("salvage lost") |
| Laser | kept | halved |
| Level | same level restarts (same seed) | back to level 1 |
| Organics / unlocks / hull / bestiary | kept | **kept** |

Flow on a spare-life death: `Death → Shop → Playing` — the death screen's
respawn re-arms at the shop (buy another life with banked blue), and the
shop's Continue restarts the current level. The same-seed restart is
deliberate: the life already paid for the retry.

## Persistence (`void-logic/src/save_game.rs`)

Split by lifetime:

- **`Profile`** — organics, permanent unlocks, bestiary. Written the moment
  a permanent thing changes (green pickup, unlock purchase, bestiary growth)
  and on run-over. Survives run-over and quits — but NOT New Game: choosing
  New Game from a menu is the explicit clean slate and wipes the whole save,
  profile included (owner's call, 2026-07-03).
- **`RunSnapshot`** (`Option`) — the continuable run, frozen at the START of
  its current level. Written only when entering Playing at level ≥ 2 (the
  definition of "finished a level"); cleared on run-over. Main-menu Continue
  exists exactly while a snapshot does (GameManager pushes
  `set_continue_available`; the menu never reads disk).

The save is the level start: a mid-level quit (or mid-shop, after a life
loss) rewinds to it — purchases and prices rewind together, nothing is
lostable that matters (green wrote through immediately).

There is **no legacy migration by design** (owner decision, 2026-07-03 —
this overhaul is the first time saves matter): `SaveGame::from_json` parses
the current shape or returns `None`, and an unparseable save is simply a
fresh start.
