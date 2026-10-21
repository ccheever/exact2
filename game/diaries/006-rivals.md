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

