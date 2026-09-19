# Task 001 — scored

Two judges (sol `gpt-5.6-sol` and grok, both at xhigh, blind to each other, the same
frozen checkout at 96fe3d2, told to be adversarial toward the home engine) read the
brief, the three diaries and the three implementations, and scored the rubric in
[README.md](README.md). They agreed on every number.

| | Godot 4.7 | three.js r186 | exact2 |
|---|---:|---:|---:|
| Small | 4 | 5 | **2** |
| Direct | 4 | 5 | **3** |
| Provable | 5 | 5 | 4 |
| Repeatable | 4 | 4 | **5** |
| Loop | 5 | 5 | **2** |
| Feel | unverified | unverified | unverified |
| UI | 4 | 5 | **3** |
| **of 30** | **26** | **29** | **19** |

Lines, as sol counted them (physical lines, no diaries/READMEs/generated files):
Godot 583 (318 game + 265 proof), three.js 362 (198 + 164), exact2 697 (258 game + 283
proof and tests + 156 host crates and manifests) — 896 with the build adapter.

## What decided it (both judges, same three)

1. **Loop.** A logic edit costs 63–73 s here (measured again by the orchestrator with
   the repo's own dev server: 63.4 s), because the bake watches the whole app
   directory and re-links the app wasm under fat LTO; the world's module alone builds
   in 3 s. Godot and three.js reload a script. *Wanted: rebuild only the world's
   module, swap it into the live canvas, carry the world across through `Data`;
   under 2 s.*
2. **The HUD bus.** One integer travels publish → JSON string → `message=` → a state
   → a TypeScript validator → a resource → text, and drags a JS engine into the app.
   *Wanted: a world's published record read in the Contract as a typed value, no
   module.* (Being built as `exactSurface("<name>")`, a third runner-owned source.)
3. **Size and the last proof step.** Four crates and two build scripts per game that
   a game should not author; save/restore not reachable through the agent
   (LLP 1041.001 D6), so step 5 ran in two native processes instead of two browsers.

Also named: `world.busy` by hand three times and a borrow that had to be dropped
first; `Mesh`'s docs were wrong about the sphere's radius and the capsule's height
(one wasted build); shadow acne on the ground in the screenshot, where three.js's
default-ish `normalBias` is clean; no accessible names asserted; `act` bound to
Enter as well as E is a focus footgun Godot's version avoided.

What the judges said to keep: the save (every field, held input and the queue, no
code), the hash agreeing between native and Chrome, the clock in the script's hands,
and `state world:<entity>` — "the one place exact2 is already ahead".

## What happens next

Each of those is a work item in the lane, in this order: typed publications (E1),
clean shadows by default (R3), save/restore through the agent plus the engine's
paper cuts (D6), the world dev loop (E2), generated host shells (E3). Then Task 001
is built **again, from scratch, by a fresh builder**, and scored again by the same
two judges. The bar is to beat 29.


## Second score (2026-09-17, late) — Beacons rebuilt from scratch on the engine after the consolidation pass

Same two judges, same rubric, same twins; the exact2 entry is `001-beacons-exact-r2.md`
(181 game lines, 6 min 9 s to a passing proof, no engine change, no workaround). The
Feel row is scored from the probe's provisional twin numbers; exact2's own numbers
could not be taken (the display was locked), so its Feel stays unverified.

| | Godot | three.js | exact2 r2 | | Godot | three.js | exact2 r2 |
|---|---:|---:|---:|---|---:|---:|---:|
| **sol** | | | | **grok** | | | |
| Small | 2 | 5 | 3 | | 3 | 4 | 3 |
| Direct | 3 | 4 | 4 | | 2 | 4 | 3 |
| Provable | 4 | 4 | **5** | | 4 | 4 | 4 |
| Repeatable | 4 | 4 | 4 | | 4 | 4 | 4 |
| Loop | 5 | 4 | 4 | | 4 | 3 | 3 |
| UI | 3 | 5 | 4 | | 3 | 4 | 3 |
| six rows, of 30 | 21 | 26 | **24** | | 20 | 23 | **20** |
| Feel | 2 | 4 | unverified | | 2 | 4 | unverified |

Up from 19 (sol: 24, grok: 20), still behind three.js (26 / 23) on the six rows. The
gate of LLP 1041.005 §3 is not passed. What both judges ask for, again concretely:

1. **A game is two or three files.** The generator still copies three host crates and
   their manifests (91 lines) into every game; the bake should synthesize them.
2. **Motion reads like the brief.** `math::ease(v, d, 0.074690334, dt)` and
   `velocity.y = 4.852216` are the engine's fault as much as the builder's: named
   kinematics (`move_toward`-style acceleration, a jump from a height), one
   `scene::follow`, a restart that is not a dummy argument.
3. **A simulation-only snapshot, and a proof that needs no browser.** `state world:*`
   carries the carrier's clock beside the world's fields (the only r2 proof failure,
   40 s to diagnose); a sub-second headless proof that can also see the HUD would
   beat Godot's 10 s loop instead of losing to it.
4. **UI defaults.** Accessible names and a live region from the Contract, the first
   button focused, a centred overlay helper, and a look that reads as designed
   (three.js's halo, ring, grid and fog are one-liners there).
5. **Feel numbers**, and `feel.mjs` present in the game the bench expects it in.

What they said to keep: the proof through the shared agent interface (sol's 5 — "no
Beacons-specific browser API"), the save and restore, the Contract HUD at 37 lines.


## Third score (2026-09-18, early) — Beacons r3 after three-file games, named kinematics, the UI defaults

Same judges, rubric and twins; the exact2 entry is `001-beacons-exact-r3.md` (181 game
lines, no authored scaffolding, 5 min 2 s, no engine change). exact2's Feel still
unverified (display locked).

| | Godot | three.js | exact2 r3 | | Godot | three.js | exact2 r3 |
|---|---:|---:|---:|---|---:|---:|---:|
| **sol** | | | | **grok** | | | |
| Small | 3 | 5 | 4 | | 3 | 4 | 4 |
| Direct | 3 | 4 | 4 | | 3 | 4 | **5** |
| Provable | 5 | 5 | 5 | | 5 | 5 | 5 |
| Repeatable | 3 | 3 | 3 | | 4 | 4 | 4 |
| Loop | 5 | 4 | **5** | | 4 | 4 | 3 |
| UI | 4 | 5 | 4 | | 4 | 5 | 3 |
| six rows, of 30 | 23 | 26 | **25** | | 23 | 26 | **24** |
| Feel | 2 | 4 | unverified | | 2 | 4 | unverified |

19 → 20/24 → 24/25 over three builds. Ahead of Godot on both cards, one and two points
behind three.js on the six rows, Feel unmeasured. The gate is still not passed; the
asks are now small and specific:

1. **Restart is a world operation**, not an invented `round` argument (grok); or
   at least the template names the idiom.
2. **One vocabulary from native test to browser proof** — `hold`, `tap`, `run`,
   `settle`, `position` on the driver's world handle exactly as on `Sim` (grok); a
   `Character` operation that folds movement, gravity, ground and bounds, and a
   `press_near`-style proximity helper (sol).
3. **Feel**: measure it; the template carries the probe; consider 120 Hz as the
   shipped step for action games (grok); one-command feel gate (sol).
4. **UI**: a centred overlay primitive with focus-visible and hover in the Contract;
   in-world beacon pads and a nearby prompt in the template (both).
5. Collapse project/surface/save terminology (`game world=Beacons(...) as hud`;
   the `screenshot … save` spelling) (sol).

Kept, again: the proof through the product's agent, the save, the Contract HUD.

## Feel, measured (2026-09-18, 04:56–05:36) — the row every card left unverified

One sitting, display unlocked, console idle throughout, all rows provisional at load
12–29 (`game/bench/README.md`, "First full sitting"). Judder is the displacement
between consecutive submitted poses on the paced clock; latency is event to submitted
pose (a CPU pose, not a photographed frame); the twins move by their own rules.

| | Godot shipped | Godot interpolated | three.js | exact2 (60 Hz world) | exact2 (120 Hz world) |
|---|---:|---:|---:|---:|---:|
| player judder (3 runs) | 0.994–1.000 | 0.032–0.370 | 0.484–0.725 | 0.002 (0.074†) | 0.002 (2.06†) |
| repeated positions | 49.7 % | 0 % | 12–26 % | 0 % | 0 % |
| event → pose, p50 | 14.6–16.3 ms | 11.0–16.5 ms | 1.45–4.65 ms | 5.15–6.50 ms (4.25†) | 3.50–4.10 ms (4.95†) |
| hitches | 12.3 % | 12.4 % | 0 | 0 | 0–2 frames |

† contaminated first attempts (the orchestrator's own proof stole the window's focus).

Read as the rubric asks — "buttery not stuttery, as high or higher fps for the same
thing": exact2's displacement varies 0.2 % frame to frame where three.js's varies
50–70 % and shipped Godot repeats every other frame; its latency at 60 Hz ticks sits
with three.js's per-frame stepping and three times under Godot's, and at 120 Hz ticks
under both. A judge scoring the Feel row from this sitting would give exact2 5,
three.js 3 (smooth cadence, uneven displacement, best latency), Godot shipped 1 and
Godot interpolated 3. That row is not yet scored by the judges: the fourth score waits
for the skeleton slice so the game rebuilt for it can be the same brief as before.

## Fourth score (2026-09-18, 10:00) — Beacons rebuilt a fourth time, the Feel row measured

Same judges, rubric and twins; the exact2 entry is `001-beacons-exact-r4.md` (148 lines
of logic + a 39-line Contract, 0 authored scaffolding, 6 min 40 s from the brief to a
passing first browser run — 3 min 20 s strictly timestamped — no engine change). The
Feel row is scored from the first full sitting (`game/bench/README.md`) for all three.

| | Godot | three.js | exact2 r4 | | Godot | three.js | exact2 r4 |
|---|---:|---:|---:|---|---:|---:|---:|
| **sol** | | | | **grok** | | | |
| Small | 2 | 5 | 4 | | 3 | 5 | 4 |
| Direct | 3 | 5 | 4 | | 3 | 5 | 4 |
| Provable | 5 | 5 | 5 | | 5 | 5 | 5 |
| Repeatable | 4 | 4 | 4 | | 4 | 4 | **5** |
| Loop | 5 | 4 | 4 | | 5 | 4 | 4 |
| Feel | 1 | 3 | **5** | | 2 | 3 | **4** |
| UI | 4 | 5 | 4 | | 4 | 5 | 4 |
| **of 35** | **24** | **31** | **30** | | **26** | **31** | **30** |

19 → 20/24 → 24/25 → **30/30 of 35** across four builds; one point behind three.js on
both cards, four and six ahead of Godot; the historical Feel score is withdrawn pending comparable perceptual evidence.
**F3 footnote to Feel:** these historical cards used world-space displacement,
and Exact's paced intervals were compared with the twins' raw callbacks. The
[same-trace re-score](../bench/README.md#sitting-2--landmark-re-score-2026-09-18t20-36-09-882z)
recomputes world CV and event-to-drawn-pose milliseconds with every legacy drawn row retained, and withdraws a
raw-pacing or perceptual winner: Exact's archived raw rAF stamps and every
engine's camera projection are absent (`—`). Sitting #2 now records raw callbacks and cameras; its landmark re-score is in the README. All rows remain provisional, and a quiet sitting is still required.

The asks, concrete on both cards:

1. **Restart is a world operation** and the tick should not mention the engine:
   `w.character("player").step(..)`, `w.nearest_xz::<Beacon>(..)`, `World::position`
   instead of a query scope, an `unwrap` chain and a second `near_xz` pass (grok);
   named bindings and a real restart signal in the Contract (sol).
2. **A headless proof that sees the HUD in well under a second**: the native test (or
   `proof.mjs linux`) drives `hold`/`tap`/`tree`/`snapshot` including Contract text and
   accessible names on the GPU-less host; Chrome only for the screenshot (grok). And
   one standard command that runs hosts in parallel, repeats, restores and compares
   saves across hosts (sol).
3. **Ship what the twins get for free**: `Game::HZ = 120` for action games (the 120 Hz
   row is the 3.5–4.1 ms one), `Material::grid` and a background in the template, a
   centred overlay with `focus-visible`/hover on Contract buttons (grok); the author-
   owned unit down to `game.rs` + `app.contract` + `proof.mjs` with the manifests derived
   (sol); task-defining numbers visible at the call site even when they equal the
   defaults (sol — copy three.js's auditability).

Kept, again: the proof through the product's agent, the save, and the Contract HUD.
Raw pacing and landmark motion are measured provisionally; neither establishes a perceptual winner.
