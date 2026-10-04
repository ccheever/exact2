# Rivals — exact2, a fast first-person arena shooter

Brief (Charlie, 2026-10-02, relayed by the lead agent, lane `roblox/rivals`): clone the
Roblox game RIVALS — 1v1 or small free-for-all duels against bots in a compact arena
with cover; first-person mouse look, WASD, sprint, jump, slide; a hitscan rifle with
recoil and spread, a rocket launcher with splash and knockback, a knife; headshots,
hit markers, damage numbers, kill feed, ammo and reload; first to five kills wins the
round. The point is **to find where the engine breaks** on input latency, fast
raycasts/projectiles and combat feel, and to measure it. Greybox art. Change the
engine only when truly blocked. Builder: Claude (Opus 5.5), one agent, worktree
`wt/rivals` on `roblox/rivals` from `origin/main` @ e50e3f74.

The game is `game/games/rivals/`; how to play it is in its README.

## Time log (UTC, 2026-10-03; 21:10 local on the 2nd)

Reading `CLAUDE.md`, the rules, `game/README.md`, the engine README, the diaries
README and the tennis diary took about four minutes before the first command.

- **04:10:23** `bun install`, `bun game/new.mjs rivals`. The scaffold still names its
  type `SmallGame` (tennis friction, unchanged).
- **04:10** First starter proof refused at the Cargo graph: the template's captured
  lock is stale (tennis friction 2, unchanged; `--update-lock` fixed it, as the
  error says).
- **04:11–04:19** Cold Linux build of the untouched starter: **473 s**
  (`UNVERIFIED`, 0 failures) at load average 15–40 with two sibling lanes building.
- **04:12–04:20** Wrote the game (arena, fighter, weapons, bots, round, HUD record,
  Contract HUD): ~1,500 lines, no engine source read except `input.rs`, `motion.rs`,
  the physics crate's `character.rs`/`queries.rs`/`types.rs` and `sim.rs`'s input
  queue.
- **04:20** Adding `exact-game-physics` needs `--update-lock` again, and that
  failed offline: the shared Cargo cache had never downloaded Rapier's
  dependencies (`bincode`, `parry3d`, three `glam`s …). `cargo fetch` on the
  generated manifest (network) fixed it.
- **04:20–04:24** First `shells.mjs --test`: clippy determinism lints clean on the
  first try; **4 min 08 s** cold. 3 of 4 tests failed, all mine (below). From then on
  `cargo test -p rivals-logic` from `.shells/` took **1.5–2.5 s** per edit.
- **04:26** `bun game/dev.mjs rivals`: web build 1 min 16 s, then
  `cannot listen on 127.0.0.1:8765: EADDRINUSE` — a sibling lane's dev server — and
  the loop exits. `--port` exists on `host/web/dev.mjs`; `game/dev.mjs` forwards it.
- **04:28** All seven sim tests green. Three fixes: `tap("KeyF")` queues both edges in
  one tick, so a fire button read with `held` never fires (now `held || pressed`);
  the first pointer move of a session only places the pointer (it is coalesced and
  creates the `PointerState` with zero delta); the range's dummies stood on the
  ramp and slid off it, out of the line of fire.
- **04:30** Writing `tests/limits.rs`: a ray aimed at the centre of a strafing
  capsule hit the wall behind it on tick 219. Isolated in seven lines to **Parry's
  capsule raycast** (limit 1). Hitscan now uses an analytic ray–capsule test; the
  engine raycast stays for walls.
- **04:34** First Linux proof: 9 checks then `contact move needs exactly x/y or
  dx/dy` (my `{by:{dx}}`), then the Linux presenter retiring the canvas contact
  (friction 6). **04:38** Linux proof: **0 failures, 23 checks, 4.9 s**.
- **04:40** First web proof: refused mid-run — `web build is stale: weapons.rs
  changed` — because I edited a tuning constant while it ran. The guard is right.
- **04:41** Web proof: **0 failures, 50.6 s**. First look at pixels: the crosshair
  was missing. `layout crosshair` said `line-height: 1040px`: I wrote
  `line-height=40`, and a unitless CSS line-height is a multiplier. Contract
  followed CSS; my bug.
- **04:44** Pointer lock (limit 4): the one engine change, `3596eae1`.
- **04:48–04:55** First baseline, `bun game/prove.mjs rivals`, one attempt: Linux
  Off / Save / FreshGame 6.0 / 1.8 / 1.6 s, web 51.1 / 49.0 / 57.8 s, a cold Linux
  release build 2 min 37 s and its run (222.9 s in all); every row agreed — tick 0
  `0x43ddb86583af9a6d`, tick 1080 (mid-duel) `0x2bdee9eeafba4807`, continuation save
  `7580aadf…` — and the pins were written.
- **04:55** `bun game/prove.mjs rivals`: **PROOF PASS**, 0.76 s.
- **04:56** `feel.mjs exact --game rivals --hz 120 --play '[data-testid="range"]'`:
  **INVALID**, `Chrome foreground … frontmost:false` — the console is at
  `loginwindow` (no GUI session), so no headed run can be measured on this machine
  (friction 11). Its result files were removed.
- **04:58** Benchmarks rerun on the live clock (limit 6).
- **05:00** Viewmodel against a wall (`artifacts/web/wall.png`, limit 10).
- **05:01** `bun game/dev.mjs rivals --port 8791`: the page serves (200) after a
  2 min warm web build; stopped. `shells.mjs --test` (lints and 18 tests): 24 s warm.
  `cargo fmt` reformatted seven files; `prove.mjs rivals` still **PASS**.

**Totals.** Brief to a playable duel with all logic tests green: 04:10 → 04:28
(18 min). To the first passing real-host proof: 04:38 (28 min). To pinned, verified
PASS on Linux and web: 04:55 (45 min), with one baseline attempt. Build/run
iterations: 5 Linux proof rounds, 5 web proof rounds, 1 baseline; ~40 hostless test
runs at 1.5–2.5 s each. Pixel inspections: 5 screenshots.

## Line counts (physical lines, `rustfmt`ed, blanks and comments included)

| file | lines |
|---|---:|
| `logic/src/lib.rs` — options, actions, tick order, camera, viewmodel, HUD record | 511 |
| `logic/src/weapons.rs` — rifle, rockets, knife, splash, hitscan, effects | 473 |
| `logic/src/fighter.rs` — capsule movement, slide, knockback, ray–capsule | 344 |
| `logic/src/bots.rs` — sight, reaction, aim, strafing, cover, unsticking | 318 |
| `logic/src/round.rs` — kills, feed, damage numbers, respawn, round win | 231 |
| `logic/src/arena.rs` — greybox arena, spawns, cover points | 141 |
| **game logic** | **2,018** |
| `app.contract` — title, HUD, feed, marks, death/round screens, pause, touch | 152 |
| `logic/tests/sim.rs` — range, duel, replay, mid-fight save, rounds | 238 |
| `logic/tests/movement.rs` — ramp, crate, wall slide, slide, rocket jump | 130 |
| `logic/tests/limits.rs` — the measurements below | 465 |
| `proof.mjs` — the real-host proof | 128 |

Engine change: `host/web/gpu-glue.js` +18 −1.

## Limits found

Limits 1–5, 7, 8 and 10 are fixed on `roblox/integrate`; see [after the fix lane](#after-the-fix-lane).

The headline: the simulation side held up far better than the input side. Fixed
ticks, saves and determinism were free even mid-rocket-barrage; what breaks an FPS
here is getting the mouse in (no raw delta, no lock), one geometric bug in the
physics library, and how the live clock's lookahead treats mouse motion on a
display whose rate is not a multiple of the tick.

1. **Parry 0.30.2's raycast misses capsules it passes straight through.**
   `physics::raycast` against a `Shape::Capsule` collider — the very collider
   `CapsuleController` installs on every character — returned `None` for **64 of
   16,000** rays aimed inside the capsule (0.4%; first at x = −1.734, 0.3 m above
   centre), static or kinematic, debug or release. A sphere of the same radius hits.
   Parry casts rays on capsules with GJK over the support map
   (`query/ray/ray_support_map.rs`), which gives up on some configurations. In the
   game that is one dead-centre shot in 250 passing through an enemy, and bots that
   lose sight of a target in plain view. Evidence and the analytic replacement's 0
   misses: `tests/limits.rs::parry_capsule_rays_miss` (it fails, telling you to
   delete the workaround, the day Parry is fixed). **Not changed**: the game's
   `fighter::ray_capsule` (35 lines) replaces it; the engine fix belongs in a vendored
   Parry or an analytic capsule path in `exact-game-physics::Queries::raycast`.
2. **No raw mouse input on any host.** The world gets absolute canvas positions
   only; `PointerState::delta` is a difference of positions. On the web the cursor
   stops at the canvas edge and mouse look stops with it; macOS
   (`CanvasInputMac.swift`, an `NSTrackingArea` of absolute points) is the same, with
   no cursor capture or `CGAssociateMouseAndMouseCursorPosition`. There is no right
   button either (`buttons` is sent by the web glue but `InputEvent::Pointer` has no
   field for it), so aim-down-sights cannot be bound. **Changed on the web only**
   (limit 4); native is open.
3. **Mouse look judders when the display rate is not a multiple of the tick rate.**
   `tests/limits.rs::mouse_latency_by_tick_and_display_rate` drives the engine's own
   `Clock::Live` with a frame period, delivers mouse moves between frames and blends
   the last two ticks by `Sim::alpha`, as the renderer does. Delays are from the
   event to the frame that shows it; *judder* is the coefficient of variation of the
   per-frame turn under a steady 2,000 pt/s, 1 kHz mouse (the 1 kHz quantization
   alone gives 0.03 at 60 Hz, 0.06 at 120 Hz, 0.07 at 144 Hz):

   | display | tick | first change p50 / p95 ms | whole turn p50 / p95 ms | judder |
   |---|---|---|---|---|
   | 60 | 30 | 25.5 / 32.4 | 42.2 / 49.1 | 0.014 |
   | 60 | 60 | 8.8 / 15.8 | 8.8 / 15.8 | 0.028 |
   | 60 | 120 | 8.8 / 15.8 | 8.8 / 15.8 | 0.028 |
   | 120 | 60 | 12.8 / 16.2 | 21.1 / 24.5 | 0.028 |
   | 120 | 120 | 4.4 / 7.9 | 4.4 / 7.9 | 0.057 |
   | 120 | 240 | 4.4 / 7.9 | 4.4 / 7.9 | 0.057 |
   | 144 | 60 | 3.7 / 6.6 | 17.6 / 20.5 | 0.181 |
   | **144** | **120** | **10.6 / 13.5** | **17.6 / 20.5** | **0.293** |
   | 144 | 240 | 3.7 / 6.6 | 4.6 / 8.7 | 0.173 |

   When the tick rate is the display rate or a multiple of it, the live clock's
   lookahead makes look latency exactly "the next frame" (4.4 ms p50 at 120/120:
   no interpolation delay at all).
   At 144 Hz with the default 120 Hz tick the turn takes 2.5 frames and the per-frame
   turn varies by ±29%, four times the input's own quantization. The likely cause (from
   `Sim::advance_with`, not instrumented further): the lookahead runs ticks to a horizon
   past `now`, but the mouse input inside that last tick has not happened yet, and
   with 6.9 ms frames over 8.3 ms ticks how much of each tick was known when it ran
   changes frame to frame — a sawtooth of late input. A 144 Hz gaming monitor is the
   common case for this genre. Fixing it means either a camera pose read from the
   latest input at frame time (a presentation-only look offset) or ticks locked to
   the display. Recorded, not changed.
4. **Pointer lock, added (`3596eae1`).** A canvas marked `data-pointer-lock="true"`
   (the Contract's existing `data-*` vocabulary; the app declares the word in
   `app.json`) requests pointer lock on a mouse press, and while locked sends an
   unbounded position accumulating `movementX/Y`. The world's model is unchanged —
   delta is still a position difference, saves and hashes are untouched — and after
   unlock positions keep the offset so no spurious flick. The web proof's drag still
   turns exactly 0.25 rad. **Unverified in automation**: headless Chrome's lock grant
   is not observable through the eight operations (no page eval, console lines are
   not in `logs`), so the locked path has been exercised only by reasoning; a person
   with a mouse is the test.
5. **Every Transform write invalidates the shared query scene.**
   `exact-game-physics` keys its cached BVH on `revision::<Transform>()` — any
   entity's, collider or not: a tracer, the camera, the viewmodel, a rocket. A ray on
   the cached scene costs **0.2 µs**; the first ray after any Transform write rebuilds
   the whole scene, **6.1 µs** here (about 55 colliders). So the obvious rocket loop
   (ray, then move that rocket) rebuilds once per rocket (release, live clock):

   | rockets | batched (all rays, then all writes) | ray-then-write |
   |---|---:|---:|
   | 10 | 9.0 µs | 62.4 µs |
   | 50 | 17.2 µs | 309 µs |
   | 200 | 38.4 µs | 1,237 µs |
   | 500 | 79.0 µs | 3,736 µs |

   7–47× for the same rays, and it grows with the arena's collider count. The batched
   form needs one `physics::queries` scope across the loop and every write after it,
   which is the game's `weapons::fly`; nothing in the API steers an author there. Each
   `CapsuleController` step writes its Transform too, so N characters rebuild the scene
   N times a tick (8 fighters: ~50 µs of a 127 µs tick).
6. **Tick cost by scale: far inside budget on native.** Release, arm64 macOS, the live
   clock (one tick per advance), every fighter holding fire, load average ~16 from
   sibling lanes; the 120 Hz budget is 8,333 µs:

   | free-for-all | tick mean | p50 | p99 | max |
   |---|---:|---:|---:|---:|
   | 1 bot | 37.5 µs | 37.0 | 66.0 | 163 |
   | 3 bots | 73.0 µs | 68.5 | 162 | 286 |
   | 7 bots (the brief's 8-fighter FFA) | 127 µs | 120 | 252 | 391 |
   | 11 bots | 250 µs | 252 | 388 | 715 |
   | 15 bots | 352 µs | 354 | 518 | 732 |
   | 23 bots | 545 µs | 573 | 725 | 964 |

   | 7 bots plus rockets in flight | tick mean | p99 |
   |---|---:|---:|
   | 0 | 127 µs | 250 |
   | 50 | 158 µs | 251 |
   | **200** | **188 µs** | 247 |
   | 500 | 210 µs | 315 |
   | 1,000 | 347 µs | 462 |

   Growth is roughly quadratic in fighters (every bot's sight rays at every other,
   every capsule step rebuilding the scene) and linear in rockets. **The seekable
   clock costs as much again**: `Sim::run`, proofs and agents observe the final tick
   pair of each advance for `settle`, so one tick per advance is 301 µs against 136 µs
   live at 7 bots and 807 against 527 at 23. My first benchmark used `run` and
   reported means of ~1 ms and p99s of 5–25 ms (also under heavier load); that was the
   observer, not the game. Web (wasm) tick cost is **not measured**: the frame-pacing
   bench could not run (friction 11).
7. **The Linux presenter cannot drive mouse look.** A canvas contact (`tap world
   down`) is retired by the presenter at its first `pointer move` (`contact:false`
   in the reply) and the world sees no pointer delta, so the Linux proof cannot
   check mouse look; the web proof does (`delivery: platform`, exactly 0.25 rad for
   100 points). Logic tests inject `InputEvent::Pointer` directly.
8. **Characters pass through each other** — by construction, read in
   `physics/src/character.rs` rather than measured: `CapsuleController` forces every
   character's own collider to a sensor and its movement filter excludes sensors, so
   two fighters walk through one another and bodies cannot block a doorway. Fine for
   this game, wrong for most.
9. **The renderer interpolates rotation along the shorter arc** (nlerp,
   `render/src/world/scene.rs::interpolate`), so a flick of more than 180° inside one
   tick is drawn turning the wrong way for that frame. At the default 0.0025 rad/pt
   that needs 1,257 points within 8.3 ms (151,000 pt/s) at 120 Hz — out of reach for a
   person, inside reach at high sensitivity or 30 Hz ticks.

10. **The viewmodel clips into walls.** Weapon models are ordinary children of the
    camera, drawn in the same depth range and field of view as the world; there is no
    viewmodel pass. Standing against the north wall (0.36 m away) the rifle's far
    0.45 m is inside it and only its stub shows (`artifacts/web/wall.png`).

What did **not** break:

- **No tunneling anywhere.** Rockets are swept rays, so a 2 km/s rocket (17 m per
  tick) stops at a 0.2 m wall (`swept_rockets_do_not_tunnel`). More surprisingly, the
  engine's `Body` has no CCD switch, yet Rapier dynamic spheres at 20, 200, 1,000 and
  5,000 m/s (up to 42 m per tick) all stopped at a **5 cm** static wall
  (`dynamic_bodies_do_not_tunnel_through_thin_walls`).
- **Hitscan on a capsule strafing at 24 m/s** (twice sprint speed), a ray per tick
  for two seconds: 240/240 hits, 120/120 head/body classifications right, the
  query scene seeing each same-tick teleport (with the analytic capsule).
- **Determinism and saves under fire.** A scripted 30 s four-way fight (strafing,
  jumps, slides, sprays, mouse flicks, rocket switches) replays to the same hash and
  the same kill list; a save mid-fight, with rockets in flight and bots mid-plan,
  restores and continues byte-identically, in `Sim` and through a fresh Linux and
  Chrome process. The physics resource, the capsule controllers' supports, brains,
  rockets and RNG were all saved without a line written for it.
- **The character controller.** Walks up a 23° ramp onto a 2.6 m deck; a 1 m crate
  stops it (autostep 0.35 m) and a jump lands on it; running into a wall at 45°
  slides along it at 4.95 m/s (the full 7 cos 45°) without sticking; slides boost
  10 → 14.9 m/s and decay back; a rocket at your feet launches you to 4.0 m for 23
  self-damage (`tests/movement.rs`).

Artifacts (ignored, under `game/games/rivals/artifacts/web/` after `bun
game/games/rivals/proof.mjs web`): `game.png` (free-for-all mid-fight), `duel.png`
(the bot firing from cover), `range.png` (a headshot's marker and damage number),
`wall.png` (limit 10). Web bundle: `app.wasm` 1,278 KiB (568 KiB gzip), the GPU
module `gpu_bg.wasm` 1,424 KiB (584 KiB gzip), loaded on demand.

## Friction log

1. **Parry's capsule raycast** (limit 1): ~25 min from the failing tick to a seven-line
   reproduction, a 16,000-ray scan and the analytic workaround.
2. **Adding the physics crate needed the network.** `--update-lock` resolves offline;
   the shared Cargo cache lacked Rapier's dependencies though another fixture uses
   them. The error names `cargo fetch`; it worked.
3. **The stale template lock** (tennis friction 2) is still there.
4. **`game/dev.mjs` dies when 8765 is taken**, as it is whenever two lanes run a dev
   loop. It names the error; `--port` is undocumented in `game/README.md`.
5. **A `tap` is invisible to `held`.** Both key edges land in one tick, so a fire
   button checked with `held` ignores taps; the agent's `tap KeyF` and a quick click
   both need `pressed` too. Obvious in hindsight; it cost a test round.
6. **The first pointer move is swallowed.** Coalescing replaces the creating move, so
   the world's first `PointerState` has zero delta. Harmless live; surprising in a test.
7. **Linux contact retired on move** (limit 7).
8. **Contract checks data-* words against `app.json`** (`bake-undeclared-data`) — a
   good error, one bake to find.
9. **`game.png` is not a fight unless you stage one.** In the 7-bot free-for-all the
   passive player dies in ~4 s; the screenshot is taken at 2.6 s.
10. **Machine load.** Starter cold build 473 s at load 15–40; the first bench run
    shared the CPU with a proof build (and used the seekable clock, limit 6).
11. **The frame-pacing bench needs a logged-in desktop.** `feel.mjs` requires Chrome
    frontmost; with the console at `loginwindow` every attempt is INVALID, so neither
    keyboard-to-photon latency nor web frame pacing was measured for this game. The
    live-clock test in limit 3 is the stand-in; it measures the engine's scheduling,
    not the browser's.
12. **The agent's web staleness check counts `proof.mjs` and `pins.json`**, which the
    proof's input digest exempts: after writing pins, driving the built game with
    `scripts/agent.mjs` refused until a rebuild of unchanged game code (28 s).

## What was pleasant

- **The engine stayed out of the way of a fast game.** A 120 Hz tick of 127 µs with
  seven bots spraying, 188 µs with 200 rockets in flight as well; hostless `Sim`
  tests that play a whole round in a second; a 4.9 s Linux proof that fights a duel, saves mid-fight and restores it in a
  fresh process.
- **Determinism held without effort**, again: glam on libm, `exact_game::math` for
  `atan2`/`sin`/`cos`, the world RNG for spread, sway and bot noise, `Vec`s in entity
  order. The clippy lints passed on the first build.
- **Bots move by the player's rules.** A brain returns the same `Intent` as input, so
  sprinting, sliding, knockback and edge behaviour apply to everyone.
- **`layout <id>` answered the CSS question immediately** (the 1,040 px line-height),
  and the stale-build guard refused a web run with an edited source mid-proof.

## After the fix lane

2026-10-03, `roblox/integrate` merged into `roblox/rivals` (ef5223a3, again with
core's review fixes and the batched-ray fix). The game was
adapted, not re-pinned (the lead holds the repin until the engine is final).

- **Workarounds deleted.** `fighter::ray_capsule` and the `parry_capsule_rays_miss`
  test are gone: hitscan and bot sight are one `physics::raycast` (closed-form capsule
  rays now) and `hitscan_tracks_a_strafing_capsule` still hits 240/240 with 120/120
  head/body calls. This branch's pointer-lock glue is replaced by integrate's, which
  sends the device's motion (`dx`/`dy`).
- **Input.** Mouse look reads `pointer().delta` as device motion; the first move now
  turns (friction 6 gone: `mouse_look_turns_by_sensitivity_from_the_first_move`).
  Fire is `KeyF` or `MouseLeft`; `MouseRight` aims down sights (52° view, 35% cone,
  65% speed; `right_button_aims_down_sights`). The proof's mouse-look check runs on
  Linux too now (limit 7 gone: `delivery: presenter`, 0.25 rad for 100 points).
- **MouseLook on the camera** (yaw and pitch −0.0025 rad/pt, limit 1.45 rad), removed
  while dead and between rounds, when the tick does not turn the camera. The latency
  test now draws with and without it. Before → after (ticks only → MouseLook):

  | display / tick | whole turn p50 / p95 ms | judder |
  |---|---|---|
  | 60 / 30 | 42.2 / 49.1 → 8.8 / 15.8 | 0.014 → 0.028 |
  | 120 / 60 | 21.1 / 24.5 → 4.4 / 7.9 | 0.028 → 0.057 |
  | 120 / 120 | 4.4 / 7.9 → 4.4 / 7.9 | 0.057 → 0.057 |
  | **144 / 120** | **17.6 / 20.5 → 3.7 / 6.6** | **0.293 → 0.032** |
  | 144 / 60 | 17.6 / 20.5 → 3.7 / 6.6 | 0.181 → 0.032 |

  Every rate now shows a turn at the next frame, and judder falls to the 1 kHz
  mouse's own quantization (limit 3 fixed; the slight rise at 60/30 and 120/60 is that
  quantization replacing the ticks' smoothing).
- **Physics.** Fighters' masks now include each other, so they block one another
  (limit 8, kept: it plays better); a dead fighter's collider drops to layer 0, so
  shots, splash and bodies pass through until the respawn. `replay_gives_the_same_kills`
  asserted >10 logged hits, which the round reset can clear; it now compares every
  fighter's shots, kills, deaths, rounds and health across two runs and asks for a
  real fight (this script: 289 shots, 10 deaths, ten kills, identical twice).
- **Query scene** (limit 5, re-measured after integrate's batched-ray fix b4de29c6,
  load average 22–30 against ~16 before; two runs): ray-then-write over 500 rockets
  3,736 → 460–1,602 µs, batched 79 → ~181 µs (it was ~210 µs before b4de29c6).
  My bench's "one ray" (a `physics::raycast` free call timed alone) reads 0.5–0.7 µs
  against 0.2 µs before; the physics lane measures the cached ray at ~0.22 µs,
  twice as fast as main, so I read my per-ray numbers as load noise. A quiet-machine
  rerun would settle both.
- **Tick costs** (release, live clock, same load; two runs): 7 bots 127 → 128–164 µs,
  23 bots 545 → 398–491 µs; 7 bots + 200 rockets 188 → 194–240 µs, + 1,000 rockets
  347 → 467–578 µs. Within noise of the first post-merge run; all far inside the
  8,333 µs budget.
- **Viewmodel.** Every weapon part carries `ViewModel`: against the north wall the
  whole rifle now draws (`artifacts/web/wall.png`; limit 10 fixed).
- **Proofs, re-pinned** on the final engine (integrate b61db278a, merge e00ff9dd5):
  tick 0 `0x43ddb86583af9a6d` → `0xe4a8cfcdccd41d7a`, tick 1080 `0x2bdee9eeafba4807` →
  `0xf0f3b95bede822ab`, continuation `7580aadf…` → `63bf2ecc…` (the same save bytes as
  every post-merge run). Kills, deaths, shots, hits, health and positions are
  identical to the pre-repin runs at every snapshot both share. `--compare-saves`
  first failed on world hashes: my range session's drag (`ms:100`) and the web-only
  free-for-all made the hosts' sessions end on different ticks; the drag is now
  instant and the free-for-all runs on both, and `bun game/prove.mjs rivals --hosts
  linux,web --compare-saves` is **PROOF PASS** (world hashes equal, save bytes identical).
  Logic tests:
  18 pass with the lints (`shells.mjs --test`). Game logic 2,017 lines (−1: the
  analytic capsule out, aiming and MouseLook in).

## Agentic-development pass — 2026-10-04

The first improvement was making the range worth replaying. It is now a
thirty-second drill: follow the green target, build a multiplier up to ×5, lose
the multiplier by hitting another dummy, and retry from the results screen.
The score and target order are ordinary saved resources; combat still uses its
first-to-five round. Every visible opponent also has a nameplate and health bar.

Three concrete failures shaped the work:

- A killed range dummy respawned at `[-18, 0.91, -18]`, far from its original
  `[0, 0.91, 12]` lane. The ordinary safest-combat-spawn rule also handled dummies.
  A failing simulation test reproduced it; training respawns now use their lanes.
- There was no public world-point-to-HUD projection. `World::project(point,
  viewport)` now uses the engine camera and its current parent transforms, returns
  CSS pixels, and refuses points outside all six clip planes. Tests cover both
  perspective and orthographic cameras, invalid input and same-tick parent motion.
  Rivals separately raycasts to each head, so a nameplate cannot reveal a fighter
  behind cover. Dead and offscreen fighters publish no contact either.
- A fresh restored simulation ignored a test's mouse turn. I had stamped the raw
  event with world seconds, but a fresh host clock starts over while saved world
  time continues. The event was queued into the future. `Sim::input_now` supplies
  the host timestamp for immediate input; the existing key and message helpers
  use it too. The drill test now fires at six consecutive requested targets across
  a restore and finishes with byte-identical saves. Raw timestamped `input` remains
  available for device scheduling experiments.

The timed drill also invalidated the latency fixture's use of an endless range:
its sweep extends beyond thirty seconds. That fixture now uses an ordinary session
with dummy brains, preserving every latency bound. No threshold was relaxed.
The game's 21 enabled tests pass (11 simulation, 6 movement, 4 limits; 4 timing
experiments remain ignored). The full `exact-game` test suite and clippy pass.

### Jev observations

`proof.mjs --playtest` gives Jev the visible HUD and nameplates, plus named actions.
An authored keyboard motor aims from each rendered head anchor. Jev chooses whom
to shoot, whether to reload and, in `--duel`, movement and weapon choices. It never
receives enemy world coordinates. This is decision testing on the agent clock,
not visual perception, human aim, input latency or a live-frame performance claim.

The first web drill cleared **30 targets for 15,500 points**, finishing at ×5 with
100% displayed accuracy in 45 decisions: 36 shoot commands and 9 waits. It followed
the requested target and waited for respawns; it relied on automatic reloads.
Gateway decision latency was median **309 ms**, p95 **841 ms**. The transcript
records 35,676 input and 1,850 output tokens. Screenshots show readable labels and
a clear final score/retry screen. Artifacts: `artifacts/jev-drill-web/`.

The macOS drill made the same 45 choices and reached the same score, target count
and displayed accuracy. Gateway median **305 ms**, p95 **521 ms**. Its carrier
closed successfully; the separate descendant audit reported unavailable because
`ps` stalled, so that run does not establish descendant cleanup. Native screenshots
match the layout, though translucent dark panels appear darker than on the web;
that visual difference remains to investigate. Artifacts: `artifacts/jev-drill-macos/`.

The first web duel was much worse: **1–2 after 64 decisions**, with 55 scans,
2 shoot commands and 7 waits. With no visible contact it repeatedly spun in place,
even though the action history showed the loop and movement was available. The
run did not finish a round. Its final cleanup check also failed because Chrome
launched two GoogleUpdater descendants that remained alive after the browser
closed; that is a harness/process finding, separate from its combat result.
Gateway median **294 ms**, p95 **479 ms**; 46,685 input / 4,723 output tokens.
Artifacts: `artifacts/jev-duel-web/`. This is useful negative evidence: stationary
target selection alone does not establish a capable arena player.

The next exploration should make combat feedback legible to the player: damage
direction, useful orientation and an approach that does not get stuck repeating
the same search. The rendered-HUD boundary should remain; feeding hidden enemy
coordinates to the controller would hide the problem rather than improve the game.

### Verification and keeping current

The first collected Linux proof passed all gameplay checks, including the timed
drill, retry, fixed-lane respawn and a fresh-process saved continuation. These runs
were exploratory, so they reported `UNVERIFIED` until the full pin collector ran.
The working branch merged main through `482d12ea8` before development, then through
`ff2435041` at the next build boundary. That brought the new `host-dev` profile,
authoring tools and Apple agent-clock fixes into the actual verification batch.
The five root checks passed (2,256 enabled tests, 9 ignored), as did 106 proof and
scaffolding tests. The Linux/web collector agreed in normal, saved-restore and
fresh-game modes, and with native release; it accepted the new drill save pin.

The independent macOS proof matched all four pins and passed every new drill
check, but exposed an older driver failure: a 100-point mouse drag reported
platform delivery while the fighter turned **zero radians**. A locked canvas reads
`NSEvent.deltaX/Y`; the agent's `NSEvent.mouseEvent` factory supplies zero deltas
even when its position moves. A small SDK experiment confirmed this and confirmed
that setting the underlying CGEvent's mouse deltas preserves the event's position,
window number and timestamp. This is a driver gap, not evidence that a physical
mouse cannot turn the native game.

The driver now fills raw motion on each drag event and rounds cumulative positions
so a three-step 100-point drag still totals 100 points. Its regression failed with
zero X and Y motion before the change; all 19 `SurfaceControlTests` pass afterward.
The real macOS proof now passes the same 0.25-radian mouse turn as Linux/web and all
drill/save checks (20.9 s including the native rebuild). The Swift test needed full
Xcode's `DEVELOPER_DIR`; the command-line-tools Swift could not import XCTest.
The native archive is under the explicit `aarch64-apple-darwin/host-dev` target,
not the unqualified host-dev directory. Those two setup mistakes cost two builds.

One more main refresh brought `0762b6d3d` into the branch; only the queue's new
entries conflicted, and both sides were retained. The five root checks passed
again (2,258 enabled tests, 9 ignored), along with the 21 game tests and 57 audio
tests. The current macOS build passed the gameplay drive in 72.7 s including its
rebuild, matching Linux's input digest, world snapshots, four pins and every save
byte. Its descendant audit was unavailable (`ps` stalled); the earlier repaired
native run had also passed that audit. Main is refreshed at build boundaries, so
a moving checkout does not invalidate a replay in progress.
The final Linux/web collector passed normal, Save and FreshGame runs and native
release again with all four pins unchanged; macOS matches the accepted input
digest and pins. The warm `bun game/prove.mjs rivals` also passes.

## Combat feedback and closed-loop decisions — 2026-10-04

Main was refreshed through `2e49129cb` before this pass. The previous duel's 55
blind scans suggested two separate needs: useful game feedback, and action results
that tell the controller whether its movement happened.

The game now shows a compass and a two-second incoming-damage direction, both an
arrow around the crosshair and readable text. A hit stores its actual origin:
rifle and knife use the firing/swinging point; rockets use the explosion. The
marker turns with the player but never follows an enemy behind cover. Tests cover
all eight directions, an attacker moving afterward, a rocket blast, hiding while
dead, expiry and byte-identical continuation after save/restore. All 23 enabled
game tests pass. One test compile round needed an explicit `f32` on its expected
angle. No new engine operation was needed: ordinary saved data, `publish_record`
and Contract rotation express the feature. The full engine tests also pass after
main's input-origin changes.

The first new screenshot exposed poor compass contrast against a dark wall. Both
compass and damage text now have a dark backing panel and light text.

Three controller trials, with the same seeded duel:

| Trial | Decisions | Result | What it did |
|---|---:|---|---|
| A: heading, incoming damage, turn-left/back choices | 64 | 0–1, unfinished | 58 forward commands, walking into cover; 1 scan and 1 shoot |
| B: own movement distance and a jump option | 64 | 1–2, unfinished | 28 forward commands, then 20 jump-forward commands; still repeated blocked actions |
| C: remove twice-blocked moves until pose changes; move after a full blind turn | 86 | 1–5, round complete | 51 forward, 12 scans, 2 strafes, 5 shoot, 1 turn-back, 15 respawn waits |
| C on macOS, separate Jev decisions | 86 | 0–5, round complete | 57 forward, 7 scans, 3 strafes, 3 shoot, 1 turn-back, 15 waits |

Trial B adds proprioception: the controller reads only the player's own position
before and after an action and reports horizontal `movedMeters`. It still gets no
enemy world positions. Trial C is an explicit authored motor policy, not a hidden
model improvement: blocked choices disappear from Jev's available actions after
two observed failures. The full-turn rule prevents indefinite stationary search.
The budget grew from 64 to 128 decisions to allow a whole round; both final runs
ended naturally at 86. The final outcome now recognizes a duel's round-over screen
as `done`, as it already recognized drill completion.

Gateway latency (median / p95): A **301 / 533 ms**, B **316 / 906 ms**, C web
**356 / 645 ms**, C macOS **368 / 898 ms**. C used 85,719 / 7,455 input/output tokens
on web and 85,693 / 7,457 on macOS. These are paused-clock decisions; model wall
time does not give the bot extra simulation time. Different Jev choices are not a
cross-host determinism test.

The limit remains combat control. The motor turns with the keyboard at 2.4 rad/s.
The recorded player-state reads bracket every decision: in both final runs,
decision 24's first shoot command spent **434 ms** aiming before its first trigger
attempt. It started at 100 HP and ended at 0, with no shot recorded. Decision 57
spent **303 ms** before its first trigger, fired three hits over a **1,014 ms**
command, and also ended dead. These are simulation times from the input receipts,
not gateway latency. They support investigating the motor, though they do not
establish that faster aim alone would win. This controller is a poor stand-in for
a person's mouse flick. Three trials are enough for this tuning loop: it does not
establish a capable arena player. The next useful comparison is a real
mouse-driven motor, separately from tactical changes. Do not lower bot difficulty
just to get a green win rate.

The regular Chrome runs again left GoogleUpdater descendants after browser exit,
failing cleanup independently of their gameplay results. The existing `CHROME`
override selected the already-installed Chrome for Testing **153.0.8010.12** for
C; its descendant audit passed, as did the native run's. No cleanup assertion was
removed and no process-name kill was used. Artifacts are under
`artifacts/jev-direction-web-{a,b,c}/` and `artifacts/jev-direction-macos/`.

### Verification after the next main refresh

The final batch merged main through `82b76214e` (merge `c5f2a95d1`), including
the shared-element work, before rebuilding the hosts. The five root checks pass:
2,261 enabled tests, 9 ignored; build, Clippy, formatting, caps and boot all pass
(counting each test binary once, excluding nested subprocess result lines).
The Linux/web collector agrees in normal, Save and FreshGame modes and with the
native release build. All four pins changed with the new round data and main's
EXSIM v7 save format; the
collector accepted them after comparing hosts and modes, with every descendant
audit available and passing. Artifacts: `artifacts/prove/run-IFmcq9/`.
All 23 game tests pass again, as does formatting for `rivals-logic`. Formatting
the entire generated shell workspace finds one unrelated failure in the Windows
entry's one-line `cfg_attr`. The next batch fixes the generator's Rust string;
regenerating the Rivals shells then passes the whole workspace formatting check.

The separate macOS drive passes every gameplay check and exactly matches the
collected Linux/web input digest, world snapshots, four pins and all six save
files. Its carrier processes closed, but its descendant audit was unavailable
because `ps` stalled; this run cannot establish descendant cleanup. The earlier
macOS Jev run did pass that audit. The final web and native screenshots both show
readable compass and incoming-hit text and the same arrow position. The older
translucent-panel darkness difference remains visible in native captures and is
still queued. Artifacts: `artifacts/direction-macos/`.

The next fetch found six more main commits, through `3394b5292`; they merged
cleanly as `ae28668c6`. The new explicit mouse-click path and Apple build scheduling
now form part of this branch, rather than a future integration task. The rebuilt
native drive passes again in **25.5 s**, including the descendant audit this time,
and matches the new Linux collection's inputs, pins, world states and saves.
Artifacts: `artifacts/direction-refresh-macos/`.

The broader proof/scaffolding run passed 144 tests and skipped two Windows-only
cases, but found three failures. The formatting case first exposed the Windows
attribute, then a renamed game's imports and a long generic bake call. The
template now imports its own types together, and the generated build script uses
one short `App` alias. The existing short/long-path formatting, type naming and
dependency-graph tests pass. The SDK lock needed only dependency edges refreshed
for main's host changes, with no package-version changes; its test also passes.
The remaining profile-proof failure is a Beacons continuation pin still from
EXSIM v6: main's pointer-origin work explicitly changed saves to v7, while its
world hashes stayed fixed. Refreshing that baseline requires the full cross-host
collector, not copying the failed assertion's hash. The 33 Linux surface-control
tests, including main's new explicit primary-mouse path, pass.

The final collectors pass for Rivals (`artifacts/prove/run-diDbro/`), Beacons
(`artifacts/prove/run-tGn42W/`) and the skinned fixture
(`artifacts/prove/run-mReegD/`): normal, Save, FreshGame, restored ordinary web
build and native release, with all descendant audits available and passing.
Rivals keeps its four pin values. The two fixtures keep their world hashes and
change only the continuation-save pins for EXSIM v7. The previously failing
fast-profile/release comparison then passes in 1.18 s. All three failures from
the broader suite are resolved in focused reruns.

The final Rivals macOS drive passes in **22.6 s** and matches the accepted
Linux/web inputs, world states, pins and every save byte. Its optional descendant
audit times out again; the preceding 25.5 s run established that audit for this
main revision, before the generator-only corrections. Artifacts:
`artifacts/direction-final-macos/`. Root checks pass on this main revision, and the
final root and generated-Rivals formatting checks pass. The checkpoint stays on
main `3394b5292`; another fetch belongs at the next work boundary.

## Pointer aiming and a passive HUD (2026-10-04)

The next fetch brought sixteen commits through `d6f7daf02`: shared animation
clocks, an opaque-canvas compositing fast path and the sleeping-display capture
fix. Merge `b5d6c4f81` preserves both branches' queue entries; that was its only
conflict. Main updates remain part of each work boundary.

The new motor uses the rendered nameplate's head point, the authored camera FOV
and sensitivity to calculate an integer pointer correction. It queues that
correction and KeyF before the next simulation tick. Jev still chooses the target
and tactics; the navigation policy and bot difficulty are unchanged. This is
authored aim assistance, not model vision or a human reaction-time claim. Each
action now records its simulation duration and trigger offsets separately from
the gateway's wall time.

The first probe hit a JavaScript initialization error: a helper declared below
the top-level `await` was not initialized when called. Making it a function
declaration fixed the ordering. The second probe landed all three headshots on
web, but native aim stayed unchanged. Clicking the crosshair had exposed a host
bug: macOS never consulted `pointer-events: none` in `NodeView.hitTest`. The HUD
intercepted the canvas. Native regression tests failed on the passive crosshair,
surface controls, a native text widget and a placed canvas's fallback box. The
fix excludes the passive node's own content while preserving a descendant that
explicitly sets `auto`. A test-fixture correction gave the otherwise zero-sized
document its actual frame; 29 native input and transform tests then passed.

The third and final motor probe passed on both hosts: left, right and vertical
corrections each took no simulation time and produced exactly one headshot.
Both finished with three shots, three hits, three headshots and 150 drill points;
the final player entities compare exactly. The complete saves differ in pointer
identity (the browser synthesizes touch contacts; AppKit uses mouse id 1), so
this is not byte-identical save parity. Artifacts:
`artifacts/pointer-motor-web-3/`, `artifacts/pointer-motor-macos-3/`.
Web's descendant audit passed; native's optional audit timed out in `ps`, while
the carrier still closed its recorded processes.

Another obstacle was a false stale-build rejection after editing `proof.mjs`
and `pins.json`: proof caching excluded them, but native launch's fallback scan
did not. Both now use the existing game-input exclusion predicate. The new
freshness regression also proves that Contract, Rust and assets still invalidate
the app, ordinary app scripts remain inputs, and a compiler receipt overrides
the helper exclusion when it actually links one. The native Jev launch reused
the cached build successfully.

The unchanged-policy Jev duels finished **4–5 on web** (100 decisions) and
**4–4 at the 128-decision cap on macOS**. Both descendant audits passed.
Web fired 13 shots for 13 hits and 12 headshots; native fired 13 for 12 hits and
12 headshots. Every shooting action attempted its first trigger at **0 ms** of
simulation time, versus the keyboard helper's 434 ms on the previous first
encounter. That encounter is still decision 24: web now lands two shots and
ends at 100 HP; native lands three and ends at 64 HP. Previously each ended
dead without firing. The fixed three-correction probe establishes motor behavior;
these two different decision sequences are exploratory outcomes, not a controlled
win-rate comparison or host-parity proof. Native's final jumps and strafes still
fail to finish the tied round. No further navigation tuning belongs in this
batch. Artifacts: `artifacts/jev-pointer-web/`, `artifacts/jev-pointer-macos/`.

Gateway decision latency was 335 / 751 ms p50/p95 on web and 338 / 706 ms on
macOS; usage was 101,704 input / 9,324 output tokens and 130,780 / 12,033
respectively. Model wall time does not advance the paused simulation clock.
While inspecting the receipts, I found that the proof wrapper logged contact
down (`tap`) but omitted the helper's `pointer` moves and releases. It now
records those existing tap-protocol phases too, so a lost move is inspectable.

Root build, test, clippy, formatting and boot checks pass after this main merge:
2,285 enabled tests and nine ignored tests across 80 binaries. The focused
freshness regression and 29 native input/transform tests pass.

The normal macOS proof passes in **16.4 s** and matches Linux's input digest,
all six world observations, every pin and all six save files. Its optional
descendant audit times out; the earlier Jev run on this same host source passed
that audit. The full collector (`artifacts/prove/run-ccf18X/`) passes normal,
Save and FreshGame modes on Linux and web, restores the ordinary web build and
checks the native release profile. Every collector audit is available and
passes. Both world pins and both continuation-save pins stay unchanged; only
their input provenance advances to `f7adbe2b9`. Native and web receipts now both
contain the pointer move and release.

The game's 23 enabled tests pass (four timing experiments remain ignored).
All 99 proof-tooling tests pass, including the Beacons and skinned-fixture
fast-profile/release comparisons. Those rebuilds make the tooling run **238 s**;
the collector's web modes and native release each take **61–80 s** including
their builds. These are the slower verification lane, not an under-one-minute
edit-loop claim. The native HUD screenshot remains darker than the web capture;
input parity did not fix that separate queued rendering issue.

## Main integration and the capture fix (2026-10-04)

After Forest's supply baseline was accepted, main brought 88 more commits through
`6cfb736a8`. Merge `72722721f` combines main's visible-overflow hit testing with
the passive surface-control guard from this branch. Both sets of queue entries
survive the merge. Main also fixes macOS capture painting a translucent box twice.

The complete Rivals proof passes on web in 98.5 s and macOS in 40.2 s, including
rebuilds. Both hosts agree on source inputs, tick/save pins, all six world
snapshots and six saves. Both descendant audits pass. Evidence is in
`artifacts/main-6cf-{web,macos}/`.

The separate pointer motor checks pass too: each of three corrections advances
zero simulation time, fires once and lands a headshot on each host. They take
11.0 s web and 3.1 s macOS, with successful process audits; artifacts are
`main-6cf-motor-{web,macos}/`. These focused checks report `UNVERIFIED` because
they do not exercise the whole baseline; the complete proofs above report `PASS`.
No further Jev policy tuning was done in this integration batch.

Viewed both new `drill.png` captures: the macOS panels and full-screen results
shade now visually match the web's, closing the darker-HUD queue item. The
upstream translucent-fill and translucent-image regressions pass alongside our
passive-HUD/control tests: 34 focused Swift tests, zero failures.

Two manual test invocations failed before the successful third: this Mac's
selected Command Line Tools do not provide XCTest, and the app archive lives
under `target/aarch64-apple-darwin/host-dev`, not `target/host-dev`. The working
invocation sets `DEVELOPER_DIR` to Xcode and `EXACT_LIB_DIR` to the built archive's
actual directory. These were invocation errors; the normal app builder already
selects Xcode and its captured archive. Root's five checks pass on the merge,
including 2,313 enabled tests with nine ignored.

## Paint-rank integration recheck (2026-10-04)

Main through `85bd9ba9b` is merged as `6250a2856` during Forest's axe work.
The complete Rivals proofs pass on web in 55.6 s and macOS in 36.3 s, including
rebuilds. Source inputs, tick/save pins, all six world snapshots and six saves
agree. Both `drill.png` captures were viewed: the result overlay and controls
retain their matching appearance. Artifacts: `artifacts/main-paint-{web,macos}/`.
Web's process audit passes; macOS reports the optional descendant scan
unavailable while every owned carrier closes. No Rivals policy change or new
Jev duel is claimed by this integration sweep.

An older native capture test needed the explicit sibling-rank message now sent
by the kernel, instead of relying on the view to interpret a style's z-index.
With that fixture corrected, all 38 selected native capture, surface-control
and transform tests pass; the pixel assertions are unchanged. Forest diary 005
records the root gates and the independent gameplay changes in this batch.

## Visibility and presentation integration (2026-10-04)

Merge `60e9fde3c`, during Garden's plot-visibility work, updates main through
`8e003c707`. Rivals' complete proofs pass on web in 101.4 s and macOS in 45.6 s,
including builds beside the other games and shared checks. Both descendant
audits pass. Source inputs, all pins, six world observations and six saved files
agree across hosts. Both new `drill.png` captures were inspected: the result
overlay, controls and scene retain their matching appearance. Artifacts:
`artifacts/main-8e-{web,macos}/`. No new Jev duel or policy change here.

Garden diary 004 records the integration conflicts, the three games' refreshed
grants dependency locks, passing root/engine/surface verification, the corrected
generated-type fixture, and the advisory semantics check blocked by missing Lean.

## A hunting bot stuck beside the ramp (2026-10-04)

The last native Jev duel stopped at 4–4. Its bot was pressing into the central
ramp at `[1.650, 0.910, 3.870]`, hunting a remembered player position across the
ramp while the player searched along the arena wall. Recreating those positions
in a fresh encounter with seed 7 reproduces the obstruction: the bot moves only
4 mm in ten seconds. This isolates the geometry; it is not an exact replay of
the old fight's score, recoil and random state.

The first implementation fixes that case. Hunting and cover movement sweep the
fighter's capsule against static arena geometry, compare seven detour directions,
and hold the selected direction for 0.3 seconds while it remains clear. A short
capsule-foot clearance excludes the support floor. The bot clears the ramp and
sees the player within one second, then fires and damages the player within
three. Its skill, reaction, accuracy, weapons and Jev controller are unchanged.

Inspection also found that reaching a last sighting selected a search destination
only to overwrite it on the next tick. A searched or five-second-old sighting
now releases the target and commits to a new destination away from the bot.
Arrival, timeout and a byte-identical save/restore during the detour are tested.
All 25 game tests pass (four timing experiments ignored); clippy passes after
three pre-existing warnings were corrected for the current stable toolchain.

The normal Linux proof, extended to save an ordinary duel during a detour and
continue it in a fresh process, has zero behavioral failures in 33.2 s including
builds. Its process audit passes. It reports `UNVERIFIED` pending the intentional
baseline collection; no old pin was silently accepted. Artifact: `ramp-linux/`.

The existing release timing experiment now includes the supported maximum of
24 bots. In a separate run after compilation finished, 1/7/24 bots average
18.3/66.8/291.8 microseconds per live tick; their p99s are 30.2/111.1/939.8.
The maximum with 24 bots is **8.63 ms**, slightly over the 8.33 ms tick budget;
the first run also hit 8.75 ms. This is a repeatable tail to investigate, not a
claim that every frame meets budget or a before/after performance comparison.
Logs are `/tmp/exact2-rivals-ramp-{tests,clippy,bench-isolated}.log`.

### Main update, real hosts and the accepted baseline

Feature `2844e5298` is followed by merge `c04a3f0f0`: 23 commits from main
through `3d76ccdb7`. The sole conflict is the top of `QUEUE.md`; both branches'
entries are preserved. Upstream changes the web agent's tap verification, the
dev server's port selection, compiler diagnostics and the formal number model.
Root build, test, clippy, formatting, caps and boot pass: **2,325 enabled tests,
80 binaries, nine ignored**. Rivals' 25 tests and its clippy/format checks pass
again. The requested advisory `difftest quick --base 2844e5298` cannot run:
`lake` is absent. Rust checks do not verify the new Lean proofs.

The complete Rivals proofs pass their behavior checks on web in **71.4 s** and
macOS in **35.6 s**, with matching inputs, pins, seven world observations and
eight saves. Those exploratory runs report `UNVERIFIED` until collection. Web's
process audit passes; macOS's optional descendant scan times out while every
owned carrier closes. Both `incoming.png` captures were inspected and agree
visually. Artifacts: `artifacts/ramp-{web,macos}/`.

One unchanged-controller Jev duel on each host, without further tuning:

| | web | macOS |
|---|---:|---:|
| Final player–bot score | 4–4 | 2–5 |
| Decisions | 128 (cap) | 118 (round ended) |
| Game seconds | 63.6 | 58.47 |
| Player shots / headshots | 14 / 12 | 8 / 6 |
| Bot shots | 76 | 55 |
| Final player HP | 80 | 0 |
| Model latency p50 / p95 | 319 / 660 ms | 341 / 682 ms |
| Model input / output tokens | 130,139 / 11,810 | 119,756 / 10,811 |

The native duel now ends; the bot is no longer left pressing into the ramp.
Web scores its fourth kill at decision 126, then reaches the cap two decisions
later. Neither is proof of general navigation quality or balanced difficulty.
Bot skill remains 0.55. The sweeps query only static geometry, and Jev still sees
only the public HUD, visible nameplates and the player's own movement feedback.
Wall times are 65.2 s web and 91.9 s macOS. Web's audit passes; macOS's optional
scan is unavailable. Both final screenshots were inspected. Artifacts:
`artifacts/jev-ramp-{web,macos}/`.

Strict collector `artifacts/prove/run-Rgteti/` succeeds with the source held at
the merge: normal/Save/FreshGame on Linux (**3.67/1.20/1.19 s**) and web
(**83.23/60.49/61.29 s**), plus native release (**82.66 s**, including a
55.64 s host build). All seven process audits pass. Every run matches the
independent macOS proof's inputs, pins, seven worlds and eight saves. Restoring
the ordinary web build takes 23.09 s. The collector accepts the new Brain schema
and duel behavior in `pins.json`; a final normal Linux run reports `PASS`.

Garden and Forest's complete web proofs also pass on the merged tap driver
(63.9/86.2 s including builds), retain their accepted pins, and pass their process
audits. Their market/chopping captures were inspected. Artifacts:
`game/games/{garden,forest}/artifacts/main-3d-web/`. Those runs and the shared
checks overlapped, so their elapsed times are not isolated performance measures.
No engine API was needed for the bot fix: existing capsule sweeps, seekable
simulation, named state reads and fresh-process saves sufficed to reproduce,
correct and verify the game-side failure.

## Full arenas need distinct spawns (2026-10-04)

The 24-bot tail was reproducible at tick 1311, the first tick of round two.
The same timing experiment now reports the slow tick and its round transition.
Startup was worse: 9.29 ms at tick two. Temporarily restoring the previous bot
implementation while holding every other source fixed showed a 9.83 ms startup
peak too. Its match did not reset within the measured ten seconds. The ramp fix
exposed an existing cost during a round reset; it did not introduce the bad starts.

Rivals wrapped 25 fighters around ten spawn coordinates, placing two or three
capsules at the same point. Simultaneous respawns independently used the same
snapshot of living enemies and also chose the same point. Two regression tests
fail on those exact overlaps: player/bot-10 at `[0, .92, 18]` on setup, and
player/bot-1 at `[8, .92, -18]` when everyone is due to respawn.

The first fix supplies 26 clear arena positions, removes wrapping on setup and
round reset, and makes each respawn visible to the next placement that tick.
All 27 game tests pass, including static-collider clearance of every candidate,
distinct starts/restarts for the maximum roster, and byte-identical continuation
after restoring immediately before simultaneous respawns. Clippy passes too.

For the before/after timing comparison, only `arena.rs`, `lib.rs` and `round.rs`
were temporarily restored from the preceding commit; the same instrumented
release benchmark ran both implementations sequentially. It now forces a round
transition after the ten-second measurement so a later victory cannot omit that
path. At 24 bots, startup's maximum drops **9.348 → 0.274 ms**; the forced reset
and its first second drop **2.343 → 0.303 ms**. The ordinary measured interval's
mean/p99/max change **0.285/0.938/8.313 → 0.169/0.262/0.417 ms**. Its before run
also includes a natural round reset; gameplay diverges with the new spawns, so
these are observed workload timings, not a per-operation microbenchmark.
Logs: `/tmp/exact2-rivals-spawn-forced-{before,after}.log`.

The maximum roster is now playable from the title as **Mayhem · 24 bots**.
Its proof checks 25 separated starts, combat, and identical continuation from
a fresh-process save. Jev can play it with the existing controller's `--mayhem`
option; the duel controller is unchanged. The complete Linux proof has zero
behavioral failures in 4.37 s and a passing process audit. It remains
`UNVERIFIED` until the new baseline is collected. Artifact: `spawn-linux/`.

### Mayhem on real hosts and the accepted baseline

Feature `4e468b848` is followed by merge `4a02f5cf2`, eight main commits through
`8c9b476fa`. The only conflict is `QUEUE.md`; both branches' entries survive.
Main makes generated shells inherit the SDK toolchain and includes that pin in
metadata freshness. Three focused tooling tests pass: external-shell toolchain
refresh, the app-path test CLI, and the level-only game bake. Root's five checks
pass with **2,325 enabled tests across 80 binaries, nine ignored**. Counting each
Cargo-launched binary's final summary once fixes earlier overcounts from child
test processes; the recent `8e003c707` and `3d76ccdb7` diary totals are corrected.

Normal Rivals proofs pass every behavior check on web (**56.0 s**) and macOS
(**34.0 s**), including builds. Inputs, all pins, nine world observations and
eleven saves agree. Both optional descendant scans are unavailable; all owned
carriers close. The title and Mayhem captures on both hosts were inspected:
four readable mode buttons, separated fighters, moving combat, and legible
health/nameplates and kill feed. Artifacts: `artifacts/spawn-{web,macos}/`.

One Jev match per host, using the existing tactical controller with Mayhem's
mode/goal selected, produces:

| | web | macOS |
|---|---:|---:|
| Final player–leader score | 5–3 (win) | 4–5 (loss) |
| Decisions / game seconds | 11 / 5.74 | 12 / 6.52 |
| Player shots / headshots / deaths | 13 / 13 / 1 | 10 / 10 / 2 |
| Final HP | 80 | 0 |
| Model latency p50 / p95 | 372 / 2,391 ms | 398 / 2,986 ms |
| Model input / output tokens | 11,209 / 974 | 12,522 / 1,109 |

Both runs finish, have successful process audits, and have inspected final
screenshots. Wall times are 17.7/16.4 s. Artifacts: `artifacts/jev-mayhem-{web,macos}/`.
The perfect headshot counts are the authored pointer motor, not Jev's visual
perception or human aim. The first-to-five rule now produces six-second rounds
with little time to reposition; a roster-appropriate score target is queued as
a separate pacing experiment. No further tactical-policy tuning was done.

Strict collector `artifacts/prove/run-k9OqAq/` succeeds with sources fixed at
the merge. Normal/Save/FreshGame take **3.25/1.55/1.57 s** on Linux and
**76.81/64.95/71.65 s** on web; the native release comparison takes **26.84 s**.
All seven process audits pass, and all seven runs agree with the independent
macOS proof on inputs, pins, nine worlds and eleven saves. Restoring the ordinary
web build takes 23.07 s. Existing duel/drill hashes remain unchanged; the new
`mayhem` continuation pin is `5a98457b07f3e1140aa78c2286613409aa063a6567da1ed5f2dbd60e3b0fd863`.
A final normal Linux proof reports `PASS` in 1.23 s. The measured spawn-timing
queue item is closed. Garden and Forest also pass their complete proofs on both
hosts after this SDK update, as their diaries record.

## Main's button and colour update (2026-10-04)

Charlie asked to keep updating from fast-moving main. Merge `a0a6d35fa`
brings 120 commits through `c2e909694` at the next clean boundary, before the
next gameplay change. Conflicts preserve both sets of queued findings and
take main's shared CSS colour parser. Main also carries native input fixes,
button content alignment and Apple build caching. Continue syncing between
verified batches, keeping the source fixed during a proof collector.

All five root gates pass: build, 2,347 enabled tests across 80 binaries (nine
ignored), strict Clippy, formatting, caps and boot. Counts use each Cargo
binary's final result once, excluding nested test subprocess summaries.
The broader app/shell/generator suite runs 111 tests: 108 pass, two optional
skips, one failure because the SDK's shared game lock omitted the new
`exact-canvas -> exact-motion` edge. Refreshing it and the three game locks
with the existing command makes all three focused lock tests pass, including
the failed case. The generated-game test also builds and drives its new game
on Linux and web. Logs: `/tmp/exact2-mainc2-{build,test,clippy,tools}.log` and
`/tmp/exact2-mainc2-sdk-lock-test.log`. Advisory semantics verification with
`quick --base a3322d7a2` still cannot run: `lake` is absent, not a pass.

Rivals' full proof passes on web (147.0 s) and macOS (69.8 s). Inputs, every
existing pin, nine world observations and eleven saves agree, including
Mayhem, the range and the restored bot detour. Both title and Mayhem captures
were inspected: all four modes and the gameplay HUD remain readable. Web's
descendant audit passes; macOS's optional scan is unavailable while every
owned carrier closes. Artifacts: `artifacts/main-c2-{web,macos}/`. Garden and
Forest also pass both hosts with unchanged pins; their diaries record the
comparisons. These wall times include builds and concurrent checks, not
isolated performance measurements. No gameplay or Jev policy changed.

## SVG filter extraction integration (2026-10-04)

During Garden's recovery batch, merge `a5f62346d` brings main `00d37ef9f` and
`79e52f11b` refreshes the game locks. Rivals' full web/macOS proofs pass in
112.5/61.4 s, with equal inputs, nine world observations and eleven saves;
every duel, range and Mayhem pin stays unchanged. Both Mayhem captures were
inspected. Web's descendant audit passes; native's optional scan is unavailable
with every owned carrier closed. Artifacts: `artifacts/main-00d-{web,macos}/`.
Garden diary 004 records the shared passing gates. No Rivals logic or policy
changes. Wall times include builds and concurrent verification.

## Main compiler integration during Forest's preparation work (2026-10-04)

Merges `e278ff4b6` and `02ca9c62f` bring main through `e702a02e3`. Rivals'
full web/macOS proofs pass in 81.0/36.4 s, including builds. Inputs, pins,
nine world observations and eleven saves agree. Both Mayhem screenshots were
inspected. Web's process audit passes; native's optional scan is unavailable
with owned carriers closed. Artifacts: `artifacts/main-e702-{web,macos}/`.
Forest diary 005 records the shared checks and unavailable Lean oracle.
No Rivals gameplay or Jev-policy change in this integration.

## A Mayhem round lasts long enough to recover (2026-10-04)

The maximum-roster playtests ended in 5.74/6.52 game seconds at five kills,
a duel-sized target with 25 fighters trading shots. Mayhem now needs 25 kills;
the smaller modes keep five. The target is derived from the existing roster
argument, so saved arguments also retain the rule. The title and scoreboard
name it, and Jev's observations now include that visible scoreboard label.
No controller goals, actions, aim motor or decision limits changed.

A new regression fails on the old code when the passive-player fight reaches
five at 7.9 s. With the change, the same fight reaches 25 at 76.6 s. Saving
after five, restoring, finishing and restarting produce identical bytes.
The existing five-kill test now also plays the seven-bot free-for-all to its
actual win and restart. All 28 game tests pass (four timing tests ignored),
including the input-latency and movement tests; game determinism lint and
formatting pass.

One fresh Jev run per host:

| Host | Decisions | Game seconds | Result | Deaths | Health | Decision p50 / p95 |
|---|---:|---:|---|---:|---:|---|
| Web | 45 | 21.85 | Win 25–10 | 4 | 80 | 327 / 461 ms |
| macOS | 62 | 27.38 | Win 25–10 | 6 | 100 | 304 / 681 ms |

Web makes 31 shooting choices, eleven respawn waits, two turns and one reload;
macOS makes 33 shooting choices, 24 waits, four turns and one forward move.
The longer target leaves room for repeated deaths and recovery rather than
ending during the opening exchange. It does not by itself induce tactical
movement. The pointer motor still lands every shot as a headshot (63/53), so
these wins do not establish human difficulty, aiming skill or a success rate.
The model still chooses what to do; precise aim comes from the authored motor.
This pacing batch is closed without further tuning.

Artifacts: `artifacts/jev-pacing-{web,macos}/`; both final captures were
inspected and both process audits pass. Input/output tokens: 54,706/5,057 and
73,568/6,240. Wall times including builds: 42.5/69.0 s. Model latency does not
advance game time.

The host proof also plays a complete Mayhem round using normal keys and the
clock: save after five kills, reach the 25-kill win screen, restart into round
two, then repeat from a fresh process. Exploratory web/macOS checks pass in
92.2/63.6 s, agreeing on inputs, pins, ten world observations and fourteen
saves. Both optional descendant scans are unavailable while all owned
carriers close. The screenshots after five kills were inspected on both hosts.

Before this work, merge `ec4375505` brings main `1544d8abb`'s development-bake
and Apple relinking optimizations. The shared app-tool suite passes 82 tests
(two optional integration skips). Root build, 2,348 enabled tests across
81 binaries (nine ignored), Clippy, formatting and boot pass. Strict
cross-mode acceptance and the other games' integration sweep follow.

### Accepted pacing and main's browser input changes

Merge `d9a33f1c8` brings main `a98895a22` after the exploratory playtests:
physical browser modifiers, focused game shortcuts and Apple build
parallelism. An independent macOS proof passes in 63.6 s. Its inputs, pins,
ten world observations and fourteen saves match all seven strict runs in
`artifacts/prove/run-OgzNCH/`: Linux normal/Save/FreshGame take
44.0/15.1/13.6 s, web 109.0/92.7/109.5 s, and native release 68.3 s.
Every strict run has zero failures and an available, passing process audit.
The collector accepts the pins and restores the ordinary web build (22.8 s).
Its child reports remain UNVERIFIED while collecting the candidate. A final
ordinary Linux proof checks the accepted pins and reports PASS in 7.1 s
(`artifacts/pacing-accepted-linux/`).

Existing duel tick, continuation and drill pins stay unchanged. The Mayhem
save changes to `87bd6aef…`, including its new published score target; the
new complete-round continuation is `22a476c0…`. The independent macOS proof
uses `artifacts/pacing-main-macos/`; its optional descendant scan is
unavailable while every owned carrier closes. The title capture was inspected.

Main's new browser modifier test initially fails on this Mac, and a focused
Chrome for Testing rerun reproduces it: `contextmenu` arrives between
`pointerdown` and `pointerup`. All held-modifier flags and trusted events are
correct. The fixture now accepts either placement relative to pointer-up,
while requiring exactly one trusted context-menu event after pointer-down
and before ControlRight's release. Keyboard and pointer-edge order remain
strict. This changes only the test, not input delivery.

Garden and Forest also pass their complete web/macOS proofs on this merged
source; their diaries record the captures and matching states. These timings
include builds and concurrent verification, not isolated performance data.

The final app/build, Windows portability and surface-record suite passes
163 tests with four optional/platform skips in 33.4 s, closing the fixture's
three-round fix loop. Log: `/tmp/exact2-maina988-tools-final.log`. The root
Rust sources are unchanged since the passing gates above; caps and boot pass
again with the accepted pins and diary updates.

## Periodic main update: file-local Contract names (2026-10-04)

After accepting the pacing batch, another fetch finds main `7cdf080e2`. Merge
`e19460756` brings its file-local Contract imports and named aliases without
conflicts. The games' authored Contracts need no changes. Root build, 2,351
enabled tests across 81 binaries (nine ignored), strict Clippy, formatting
and boot pass. The compiler advisory `quick --base 0991e07e3` exits 2 because
Lean's `lake` is absent; no semantics-oracle pass is claimed. Logs:
`/tmp/exact2-main7cdf-{build,test,clippy,semantics}.log`.

Rivals' complete proofs pass on web/macOS in 134.8/62.9 s. Both hosts agree
on inputs, every pin, ten world observations and fourteen saves; all worlds
and saves match the accepted pacing baseline. Only the source-input digest
changes. Both round-win screenshots were inspected, including the 25-kill
target. Web's process audit passes; native's optional scan is unavailable
while every owned carrier closes. Artifacts: `artifacts/main-7cdf-{web,macos}/`.
The Garden and Forest diaries record their corresponding integration runs.
These timings include builds and concurrent work, not isolated measurements.
No gameplay or Jev-policy changes accompany the merge.

Caps passes after staging the three diaries. All six game runs finish with
zero failures, and the working tree is committed at this verified boundary.

## Bake cache and paint-plane integration (2026-10-04)

During Garden's empty-plot work, merge `f59d587ac` brings main through
`e097e4cae`. Rivals' complete web/macOS proofs pass in 47.0/63.4 s.
Both hosts agree on inputs, pins, ten worlds and fourteen saves; worlds and
saves match the previous integration. Both five-kill Mayhem screenshots
were inspected, showing the round still live with its 25-kill target. Web's
process audit passes; native's optional scan is unavailable with owned
carriers closed. Artifacts: `artifacts/main-e097-web-recheck/` and
`artifacts/main-e097-macos/`. The first web attempt was interrupted because
I passed a mistyped Chrome executable path; the corrected run is the evidence
above. Garden diary 004 records the shared passing gates. No Rivals gameplay
or Jev-policy changes; timings include builds and concurrent verification.

## Shared descendant-audit correction (2026-10-04)

Garden diary 004 records the shared `ps` failure and delayed-event fixes.
Rivals passes complete web/macOS proofs in 59.1/57.6 s with successful
descendant audits and no remaining recorded children. Inputs, all pins,
ten world observations and fourteen saves match `main-e097-macos` on both
hosts. Both five-kill Mayhem captures were inspected; the round remains
live with its 25-kill target. Artifacts: `artifacts/audit-{web,macos}/`.
No gameplay or Jev-policy changes; timings include builds and concurrent
verification.

## Readable nameplates in a crowded fight (2026-10-04)

The five-kill Mayhem captures from the shared audit sweep show names and
health bars overlapping: bot-14 and bot-11 merge into an unreadable plate.
The crowded fight needs a bounded display policy. Plates now occupy an
explicit 128 × 48 CSS-pixel box with a six-pixel gap between neighbors.
When boxes compete, the drill's requested target wins, then the opponent
nearest the crosshair, with fighter slot breaking ties. A suppressed plate
returns as fighters separate or the player turns. A four-pixel screen inset
keeps a whole plate visible. Head anchors, line-of-sight checks and the
Jev pointer motor are unchanged; the display never shifts an aim point to
make room. No extra saved resource or engine API is needed.

A hostless regression reproduces overlapping bot-1/bot-21 plates at 3.6 s
in Mayhem, then passes with every retained anchor still equal to the
projected head. The 640 × 360 case prioritizes the requested drill target
over the central dummy, and turning reveals the newly aimed-at opponent.
My initial small-screen assertion incorrectly expected one plate: the
two outer lanes have enough space for both; correcting it retains both.
All 29 enabled Rivals tests pass (four ignored measurements), as do strict
game Clippy and formatting. Logs: `/tmp/exact2-nameplates-*.log`.

The first full web/macOS attempts fail only my added geometry check: I
read coordinates from `tree`, which exposes properties, instead of `layout`.
Those runs still agree on all ten world observations, every pin and all
fourteen saves, and both descendant audits pass. Their world observations
and tick pins equal the pre-change `audit-macos` baseline. The corrected
check uses the existing `layout` operation to verify actual plate boxes on
the hosts. Failed artifacts remain in `artifacts/plates-{web,macos}/`;
the corrected runs use `artifacts/plates-fixed-{web,macos}/`.

Both corrected proofs finish with zero failures and successful descendant
audits: web 63.4 s, macOS 54.3 s. Inputs, all pins, ten worlds and fourteen
saves agree. Both five-kill screenshots were inspected; the previously
overlapping plates are separate. Only the Mayhem and Mayhem-round save pins
change, because saved HUD contacts contain the reduced label set. Combat
world observations and the other save pins remain unchanged.

The ordinary three-correction pointer check lands all three headshots on
each host (web/macOS 11.0/3.8 s). One unchanged Jev Mayhem run per host then
wins: web **25–14 in 75 decisions**, native **25–8 in 70**. Game times are
33.425/32.375 s, deaths nine/seven, shots 79/68, headshots 75/66. Both final
captures were inspected, both audits pass, and neither controller nor goals
were tuned after the outcomes. These exploratory text-driven decisions use
the existing authored aim assist; they do not establish a human-aim result
or improved success rate. Artifacts: `artifacts/plates-motor-check-{web,macos}/`
and `artifacts/plates-mayhem-{web,macos}/`. Wall times for the Jev runs are
54.7/79.6 s including builds; gateway latency does not advance game time.
This gameplay feedback batch is closed; strict save acceptance follows.

Decision latency p50/p95 is 309/637 ms on web and 297/588 ms on macOS;
input/output token totals are 81,893/6,477 and 79,961/6,795 respectively.

Feature `41f4a86b2` is followed by periodic main merge `745a66652`, through
`152cf5b17`. Main's bounded Windows shutdown and informative proof errors
meet this branch's Jev helper and moved receipt test at two insertion sites;
the merge keeps both implementations and all their tests. The shared proof,
decision and Windows suite passes 119 tests (seven platform skips, 56.7 s).
All five root checks pass in 53.6 s: build, 2,351 enabled tests across 81
binaries (nine ignored), strict Clippy and formatting, caps and boot. The
unchanged root Rust sources now meet the 60 s target; the queue records this
counterexample beside the previous 65.2 s miss, without claiming a fix.
Logs: `/tmp/exact2-plates-main-{build,test,clippy,fmt,caps,boot,tools}.log`.

An independent post-merge macOS proof finishes in 61.7 s with zero failures,
matching the pre-merge pins, ten worlds and fourteen saves; only the source
input digest changes. Its optional descendant scan reports a genuine
`ps timed out after 200 ms`, and all owned carriers close. The five-kill
capture was inspected again. Artifacts: `artifacts/plates-main-macos/`.

Strict collector `artifacts/prove/run-rKc52H/` accepts all seven runs with
zero failures and successful process audits. Linux normal/Save/FreshGame
take 21.7/15.0/15.5 s, web 110.4/101.2/83.8 s, and native release 33.9 s.
It restores the ordinary web build in 23.2 s. Every input digest, pin, world
observation and save matches the independent post-merge macOS run. Tick
pins, duel continuation and drill saves remain unchanged; accepted Mayhem
saves are `c5b5ea36…` and `491bf158…`. Collector child summaries say
UNVERIFIED while gathering candidates; the successful collector writes the
accepted pins after all comparisons agree.

Garden and Forest also pass complete proofs on both hosts after the merge,
with their previous world/save pins intact and all four representative
captures inspected. Their diaries record timings and the one Garden web
audit timeout. Verification times include builds and concurrent work, not
isolated performance measurements.

The final ordinary Linux proof passes against the accepted pins in 6.8 s
with a successful process audit (`artifacts/plates-accepted-linux/`). Caps
passes after staging the pins, the three diaries and the gate-timing note.
