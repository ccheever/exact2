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

Reloading shows a moving meter. Press R again, or tap its HUD button, while the
meter is green to fill the magazine immediately. The green window spans 45–65%
of the normal reload: 1.6 seconds for the rifle, 2.2 for rockets. You get one
timing attempt per reload; an early or late press keeps the normal deadline.
Holding R does not trigger the second press. Switching weapons cancels the
reload. The window and a missed attempt survive saving and restoring.

The first-person rifle has a stock, barrel, sights, magazine and gloved hands;
the launcher has a collared tube and sight, and the knife has a guard and blade.
Firing briefly lights the muzzle, and reloading tilts the weapon and lowers its
magazine before it returns. Rifle, launcher and knife have distinct sounds;
hits, headshots and eliminations give different confirmation cues. Other fighters'
shots and explosions are positional. These models and gestures follow saved
world time, including a save during a shot or reload. The manifest's `game.audio`
selects the optional output; the gameplay aim, spread, damage and reload windows
remain the same.

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

## Looks

The title's **Look** button (and the pause menu's) cycles the setup argument
`art`: `""` **Classic**, the greybox; `"pass"` **Art pass**, a dressed dusk arena;
`"night"` the art pass at night, lit by moonlight, four 2.5 Mcd floodlights and
glowing trim. It is a setup argument, so changing it starts a new match. Every look
fights the same fight: the same inputs give the same fighters, hits, kills and
rounds (`logic/tests/art.rs`), because the art pass keeps the classic colliders,
never draws from the world RNG and allocates no entity mid-fight. Only the classic
look's tick hashes are pinned.

The art pass draws the arena as one model built from the tables' layout (the
same blocks the colliders are), with trimmed walls, glowing team lines,
banners and floodlight pylons under a painted sky that both lights the arena and
is drawn as its sky. Bots are soldiers in their team's armour, assembled from rigid
parts that run, flinch when hit, fall when killed and fade out before respawning.
The first-person rifle, launcher and knife have gloved hands; they bob with your
stride, kick, tilt and drop the magazine to reload, come to the centre to aim and
turn over to inspect (**T**). Bullets throw sparks (red off a fighter), rockets fly
as models with a light and a smoke trail, and blasts are a fireball, smoke and a
flash of light. The HUD's crosshair, hit marker, score plates, damage vignette and
round screens are restyled.

`art-src/gen.mjs` (Bun, about 3 s) writes every model, texture and sky under `art/`
from code; nothing is downloaded. Rerun it after editing `art-src/` or the arena in `assets/rivals.level.json`.
Textures under `art/textures/` and `art/data/` are shared by name, so each bakes
once however many models sample it. How it is built on the engine:

- What a tick saves is only when something happened: a soldier's last hit and
  shot, a blast, an inspect (`art.rs`). Everything that follows from that and the
  clock — weapon motion, which weapon shows, muzzle flashes, the soldiers' gait,
  facing, flinch and fall, team colours and hit flashes — is rebuilt by
  `Game::present` (`art_present.rs`) as `Offset`, `Opacity` and
  `MaterialOverrides`, outside saves and hashes.
- One set of soldier parts serves every bot: `MaterialOverrides` give the armour
  each team's colour and the visor its glow.
- Lingering particles (sparks, fire, smoke, rocket trails) are pooled
  `WorldSpace` emitters spawned in setup, moved to each impact or rocket and
  fired there; smoke uses a `ParticleLook` texture and soft particles, sparks
  stretch along their motion.
- `Game::STREAMED` fetches the rocket model and the sky and smoke textures
  without awaiting them; the art models load when the art pass's setup names
  them. The classic look waits for nothing new.

## Files

| file | what |
|---|---|
| `logic/src/lib.rs` | options, actions, the tick, camera, viewmodel, HUD record |
| `logic/src/fighter.rs` | the capsule fighter: movement, slide, knockback, aiming |
| `logic/src/weapons.rs` | rifle, rockets, knife, splash, hitscan, effects |
| `logic/src/presentation.rs` | first-person models, saved gestures and firing/hit sounds |
| `logic/src/bots.rs` | sight, aim, strafing, cover |
| `logic/src/round.rs` | kills, feed, damage numbers, respawn, round win |
| `logic/src/training.rs` | target order, combo scoring and fixed respawn lanes |
| `logic/src/arena.rs` | the arena's colliders (greybox-drawn in the classic look), spawns and cover points |
| `logic/src/art.rs` | the art pass: sky, lights, models, soldiers, weapons and effect pools |
| `logic/src/art_present.rs` | the art pass's derived motion, flashes and colours (`Game::present`) |
| `assets/rivals.level.json` | the tables: the arena and every number the fight is tuned by ([below](#tables)) |
| `logic/src/tables.rs` | the tables' types and the check the bake and every delivery run |
| `art-src/`, `art/` | the art generator and its output |
| `logic/tests/tables.rs` | what the check refuses; a reload replacing the tables mid-match |
| `logic/tests/sim.rs` | range, duel, replay determinism, mid-fight save |
| `logic/tests/art.rs` | every look fights the same fight; the art pass saves and draws from saved causes |
| `logic/tests/limits.rs` | engine limits, measured (`--release -- --ignored` for timings) |
| `proof.mjs` | the real-host proof |

`rivals_logic::rates::{Rivals30, Rivals60, Rivals240}` are the same game at other
fixed rates for the tick-rate experiments.

## Tables

`assets/rivals.level.json` is the one place the arena (blocks, spawns, pylons) and the
fight's numbers (movement, health, bandage, each weapon's damage, rate, range and
reload, the quick-reload window, kills to win, respawn and round waits) are
written. `logic/src/tables.rs` declares its types; `Game::LEVELS` delivers it
before setup and the bake refuses one that fails `Tables::check` (a missing
number, an inverted window, a spawn inside a block, too few spawns). The tick and
`Game::present` read it where they use a value, never copying one at spawn; the
HUD reads `max_hp`, `bandage_heal` and the window from the published record;
`art-src/gen.mjs` dresses its layout and `proof.mjs` reads its numbers.

In `bun game/dev.mjs rivals`, saving the file replaces the tables in the running
match (LLP 1046.009 G2): the next tick reads the new numbers and rebuilds the
arena's colliders (and the classic look's boxes) from the new layout; fighters,
scores and the clock carry on. What a fighter saved from an old value keeps it
until next set: a reload's deadline, a magazine already filled, a slide under
way. The art pass's arena model is the generator's: rerun `gen.mjs`, and the
loop bakes and delivers the changed model the same way. A file that fails the
check is refused by name and the match keeps its last good tables.

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
Add `--reload-drill` for a separate 96-choice target drill with timed reloads:
Jev reads the visible reload label and can wait 0.1 seconds or press R again.
The outcome counts quick reloads and missed attempts. The clock stays still
while the model decides, so this tests the control's clarity, not human reflexes.
Without a gateway key, `bun game/games/rivals/proof.mjs web --playtest --motor-check`
(or `macos`) checks three pointer corrections against the training target.

![A free-for-all in progress](artifacts/web/game.png)
