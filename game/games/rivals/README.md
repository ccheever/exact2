# Rivals

A fast first-person arena duel in the style of Roblox RIVALS, built to find the
engine's limits on input latency, raycasts, fast projectiles and character
movement. The diary is [`game/diaries/006-rivals.md`](../../diaries/006-rivals.md).

```sh
bun game/dev.mjs rivals            # the dev loop (add --port N if 8765 is taken)
bun game/prove.mjs rivals          # the Linux proof against pins.json
bun game/games/rivals/proof.mjs web
bun game/app/shells.mjs game/games/rivals --test
```

## Playing

Pick **Duel** (one bot), **Free for all** (seven bots), **Mayhem** (24 bots) or
**Range** (three standing dummies). Duel and Free for all are first to five
kills; Mayhem is first to 25, leaving time to reload, respawn and come back.
The scoreboard shows the active target. All 25 fighters have distinct starts;
the dead respawn after two seconds
at the spawn farthest from living enemies, including anyone else who respawned
on that tick.

Range is a thirty-second target drill. Eliminate the green target, then switch to
the next one: each correct elimination earns 100 points times your combo (up to
×5), plus 50 for a headshot. Hitting another dummy breaks the combo. Dummies return
to their own lanes after two seconds. **Try again** starts a fresh drill. Visible
opponents show their name and health; cover hides their labels.
Crowded labels favor the drill's requested target, then the opponent nearest
the crosshair. Others reappear as you turn or the fighters separate. Each
label stays directly above its fighter's head, with space between labels and
inside the screen edges.

A compass reports the direction you face. After taking damage, a red arrow and
`Hit from …` label point toward that shot or explosion for two seconds. Turning
changes the relative direction; the marker remembers the hit's origin rather
than tracking an attacker behind cover.

| | |
|---|---|
| Look | mouse (a click captures it; Esc releases), or the arrow keys |
| Move · sprint · jump · slide | WASD · Shift · Space · C (while running) |
| Fire · aim · reload | left button or F · right button (down sights) · R |
| Weapons | 1 assault rifle · 2 rocket launcher · 3 knife (or the wheel) |
| Bandage | hold Q, or hold the health panel's button |

Each life gives you one bandage. Hold for 1.5 seconds to recover up to 40 HP;
walking slows while you dress the wound. Releasing early, taking damage,
firing, reloading, switching weapons, jumping, sprinting or sliding interrupts
the attempt without spending the bandage. It is spent only when healing
finishes. Full health cannot consume it. Respawning or starting a new round
gives you a fresh one. Hurt bots can use the same action after reaching cover.

The rifle is hitscan: 20 a hit, ×1.8 to the head (the capsule's top 0.4 m),
with bloom from sustained fire and movement and a view kick per shot. Rockets
fly at 42 m/s as swept rays (they cannot tunnel), deal 35 on a direct hit plus
up to 75 splash in 4.5 m, are blocked by cover, and knock everyone back — you
included, at 35% damage, so rocket jumps work. The knife does 45, or 100 from
behind. Aiming down sights narrows the view to 52°, tightens the rifle's cone
to 35% and slows you to 65%. Fighters block one another; a dead fighter leaves
its collision layer, so shots and bodies pass through until the respawn.

Bots see with rays (every sixth tick, staggered), react after 0.2–0.3 s, aim
with an error that shrinks while they track, strafe at a preferred range, take
cover to reload or when hurt, switch to the knife up close and to rockets at
mid range, and hop when the capsule stalls on an edge. While hunting or seeking
cover, they sweep a capsule ahead and hold a short detour around obstructions.
After searching a last sighting or losing it for five seconds, they choose a
new search destination. A brain produces the same
`Intent` the player's input does, so bots move under the player's rules.

## Files

| file | what |
|---|---|
| `logic/src/lib.rs` | options, actions, the tick, camera, viewmodel, HUD record |
| `logic/src/fighter.rs` | the capsule fighter: movement, slide, knockback, aiming |
| `logic/src/weapons.rs` | rifle, rockets, knife, splash, hitscan, effects |
| `logic/src/bots.rs` | sight, aim, strafing, cover |
| `logic/src/round.rs` | kills, feed, damage numbers, respawn, round win |
| `logic/src/training.rs` | target order, combo scoring and fixed respawn lanes |
| `logic/src/arena.rs` | the greybox arena, spawns and cover points |
| `logic/tests/sim.rs` | range, duel, replay determinism, mid-fight save |
| `logic/tests/limits.rs` | engine limits, measured (`--release -- --ignored` for timings) |
| `proof.mjs` | the real-host proof |

`rivals_logic::rates::{Rivals30, Rivals60, Rivals240}` are the same game at other
fixed rates for the tick-rate experiments.

## Jev playtesting

With `AI_GATEWAY_API_KEY` in the environment, run
`bun game/games/rivals/proof.mjs web --playtest` (or `macos`). Add `--duel` for a
moving opponent, or `--mayhem` for the 24-bot free-for-all. Jev chooses targets
and tactics from the visible HUD. An authored
pointer motor aims from rendered nameplate positions and queues the trigger on
the same simulation tick. The controller also reads
the player's own movement distance to recognize blocked steps: after two failures
it removes that move until position or heading changes, and after a full blind
turn it requires a new vantage point. Jev chooses from the remaining actions;
neither reads enemy world positions. Transcripts and screenshots land under
`artifacts/<host>/`. These are
exploratory decision tests, not deterministic proofs or measurements of human aim.
The outcome records each action's simulation duration and trigger times; model
latency remains in the decision transcript and does not advance the game clock.
Add `--recovery` for a separate 96-choice drill: ordinary rocket self-splash
first leaves the player wounded, then Jev chooses whether to bandage and
continues the target drill using the visible health panel and nameplates.
The deterministic proof also holds the actual HUD button, cancels early and
restores a half-finished keyboard hold in a fresh process.
Without a gateway key, `bun game/games/rivals/proof.mjs web --playtest --motor-check`
(or `macos`) checks three pointer corrections against the training target.

![A free-for-all in progress](artifacts/web/game.png)
