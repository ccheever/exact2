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

Pick **Duel** (one bot), **Free for all** (seven bots) or **Range** (three
standing dummies). First to five kills takes the round; the dead respawn after
two seconds at the spawn farthest from living enemies.

| | |
|---|---|
| Look | mouse (a click captures it; Esc releases), or the arrow keys |
| Move · sprint · jump · slide | WASD · Shift · Space · C (while running) |
| Fire · reload | click or F · R |
| Weapons | 1 assault rifle · 2 rocket launcher · 3 knife (or the wheel) |

The rifle is hitscan: 20 a hit, ×1.8 to the head (the capsule's top 0.4 m),
with bloom from sustained fire and movement and a view kick per shot. Rockets
fly at 42 m/s as swept rays (they cannot tunnel), deal 35 on a direct hit plus
up to 75 splash in 4.5 m, are blocked by cover, and knock everyone back — you
included, at 35% damage, so rocket jumps work. The knife does 45, or 100 from
behind.

Bots see with rays (every sixth tick, staggered), react after 0.2–0.3 s, aim
with an error that shrinks while they track, strafe at a preferred range, take
cover to reload or when hurt, switch to the knife up close and to rockets at
mid range, and hop when the capsule stalls on an edge. A brain produces the same
`Intent` the player's input does, so bots move under the player's rules.

## Files

| file | what |
|---|---|
| `logic/src/lib.rs` | options, actions, the tick, camera, viewmodel, HUD record |
| `logic/src/fighter.rs` | the capsule fighter: movement, slide, knockback, analytic ray-capsule |
| `logic/src/weapons.rs` | rifle, rockets, knife, splash, hitscan, effects |
| `logic/src/bots.rs` | sight, aim, strafing, cover |
| `logic/src/round.rs` | kills, feed, damage numbers, respawn, round win |
| `logic/src/arena.rs` | the greybox arena, spawns and cover points |
| `logic/tests/sim.rs` | range, duel, replay determinism, mid-fight save |
| `logic/tests/limits.rs` | engine limits, measured (`--release -- --ignored` for timings) |
| `proof.mjs` | the real-host proof |

`rivals_logic::rates::{Rivals30, Rivals60, Rivals240}` are the same game at other
fixed rates for the tick-rate experiments.

![A free-for-all in progress](artifacts/web/game.png)
