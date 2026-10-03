# 99 Nights in the Forest — exact2, aimed at world scale, light and AI

Brief (lead agent for Charlie, 2026-10-02, lane `roblox/forest`): clone the Roblox
game *99 Nights in the Forest* to **find where the engine breaks**, not to make a
polished demo. Two sibling lanes clone other games against other axes; this one is
**world scale + rendering/atmosphere + AI**: a big forest at 1k → 100k+ trees, a
day/night cycle with a campfire whose light radius changes, darkness, fog, a
flashlight, a Deer that stalks at night, many wolves, navigation around trees,
player collision against thousands of trunks, carrying, determinism and a mid-night
save. Change the engine only when truly blocked; a limit is a finding first.
Builder: Claude (Opus 5.5), one agent, worktree `wt/forest` on `roblox/forest` from
origin/main @ e50e3f74. Machine: Apple M5 Pro (18 cores, 64 GB), shared with the two
sibling lanes (load average 7 → 221 during this work), display locked, so every
browser number is headless Chrome WebGPU (real GPU work, no vsync: "sanity-only"
as `game/bench` calls it).

The game is `game/games/forest/`; how to play it is in its README.

## Time log (UTC, 2026-10-03)

Reading `CLAUDE.md`, the rules, `game/README.md`, the engine and render READMEs and
the tennis diary took about five minutes before the first receipt.

- **04:10:34** `bun game/new.mjs forest`: as for tennis, the type is `SmallGame`;
  renamed by hand.
- **04:10:47** First Linux proof refused in 0.5 s: *the SDK lock is stale on main*
  (`cssparser 0.37.0`, `dtoa 1.0.11`, `dtoa-short 0.3.5` resolve outside
  `game/app/shells.lock`). The error names `--update-lock`, which worked. Same
  friction as tennis's #2, three weeks later, with different crates.
- **04:10:51 → 04:18:13** Cold Linux build and starter proof: 441.9 s, 0 failures.
  **04:18 → 04:24** cold web build (`--screenshot-only`): 394.0 s.
- **04:19 → 04:27** Wrote the game: terrain, jittered tree grid, campfire and sky,
  player needs and carrying, the Deer and wolves, HUD, Contract (≈1,100 lines).
- **04:27** Adding `logic/Cargo.toml` (for `exact-game-physics`) made the game's own
  lock stale again (`--locked` refuses); a second `--update-lock`.
- **04:29:45** First compile of logic and tests: one error (a `Ref<Deer>` in a
  `{:?}`). **04:30** 5 of 7 tests green; both failures were my tests (food capped at
  100; the first day starts at t = 8 s, after dawn). **04:33** 7 of 7.
- **04:34** Linux bake: the Contract compiler reported all twelve
  `button has no attribute aria-pressed` errors in one run (good: one round, not
  twelve). Removed; the title shows the selection as text instead.
- **04:35** Linux proof 24/25: the Deer struck before the flashlight stunned it.
  Eight-way keyboard facing missed a 30° cone; widened to 40°, the beam slows the
  Deer, and the proof's bot turns toward it in two-tick steps. **04:38** 0 failures.
- **04:39** First full web proof: the day frame was white. `PointLight.intensity`
  is documented as candela but reaches the shader unscaled, while the sun's lux is
  multiplied by 0.0003 (friction 4). The run then died on a CDP 15 s timeout: a
  60-second seek is one `Runtime.evaluate` (friction 7). Seeks now go in 10 s steps.
- **04:41 → 04:52** `logic/examples/scale.rs`, the hostless scale probe; found that a
  seekable `Sim::run` observes the whole world per call and that Rapier costs
  O(trees) per tick; profiled both with `sample` (limits 1–3 below).
- **04:52 → 04:57** Lighting in the sun's units; two `--screenshot-only` rounds
  (65 s, 82 s) to tune the night. **04:57** full web proof: 0 failures, 100.9 s;
  `day.png`/`night.png` kept under `artifacts/screens/`.
- **04:58 → 05:06** `bench.mjs`, the live headless-Chrome probe, and the first
  sweeps (limits 4–6). Load rose from 12 to 221 halfway through.
- **05:07** Death by starvation emitted no `died` (the tick compared against a
  flag read after the needs step). Fixed; two more tests (9 green).
- **05:08** First baseline started, then stopped by me: the sources were not yet
  `rustfmt`ed and formatting would have changed the input digest under it.
- **05:09 → 05:33** Baseline, first attempt: Linux Off/Save/FreshGame and the Linux
  release run passed (14.3 / 310.1 / 185.1 / 356.4 s), web Off passed (135 s), web
  Save and FreshGame died on the 15 s CDP window inside a 10 s seek: under Save mode
  every tick round-trips the ~6 MB save. Proof seeks now go in 1 s steps.
- **05:41 → 06:12** Baseline, second attempt: every row agreed and the pins were
  written (24 min 13 s; Linux Save 205 s, web Save 507 s, web Off 117 s).
  Tick 0 `0x828be9d37d1f3469`, tick 4500 (mid-night) `0xcc809bc72cd80213`,
  continuation save `df2abeca…`, identical to the first attempt's Linux rows.
- **06:12** `bun game/prove.mjs forest`: **PROOF PASS**, 14.7 s.
  **06:14** `--hosts linux,web --compare-saves`: **PROOF PASS** (Linux 11.0 s, web
  98.3 s, world hashes equal, save bytes identical), 1 min 55 s.
- **06:15 → 06:24** Interleaved web sweeps at load 7–21 (the tables below).
- **06:25** `bun game/dev.mjs forest` serves (port 8765 was a sibling's dev server:
  `EADDRINUSE`, then `--port 8803`); stopped.

**Totals.** Brief to a playable game with all tests green: 04:10 → 04:33 (23 min).
To the first passing real-host proof (Linux): 04:38 (28 min). To pinned, verified
PASS on Linux and web: 06:14 (2 h 4 min, of which ~55 min were two baselines).
Build/run iterations: 9 Linux proofs, 5 web proofs, 3 `--screenshot-only` rounds,
2 baselines, 5 bench sweeps, 4 scale-probe runs, 3 `sample` profiles.

## Line counts (physical lines, `rustfmt`ed, blanks and comments included)

| file | lines |
|---|---:|
| `logic/src/lib.rs` — args, actions, setup, the tick's order, HUD record | 198 |
| `logic/src/forest.rs` — terrain, tree grid, lookups, steering, felling, pine mesh | 350 |
| `logic/src/camp.rs` — day/night cycle, sky, sun/moon, campfire, torches | 285 |
| `logic/src/creatures.rs` — the Deer's and wolves' state machines | 333 |
| `logic/src/player.rs` — movement (Rapier or grid), needs, carrying, children | 427 |
| **game logic** | **1,593** |
| `logic/tests/sim.rs` — 9 hostless tests | 289 |
| `logic/examples/scale.rs` — hostless scale probe | 127 |
| `app.contract` — title with size/wolves/torches/mode choices, HUD, death | 151 |
| `proof.mjs` — real-host proof with a key-only walker | 137 |
| `bench.mjs` — live web probe | 113 |
| manifests (`app.json`, `logic/Cargo.toml`) | 16 |

Engine source changed: none (see "Engine changes").

## What is playable

Start at a campfire in a clearing. WASD moves, E chops (three blows fell a pine and
drop two logs), picks up (logs, scrap, food, up to five, stacked on your back as
parented entities), feeds the fire, or takes a lost child by the hand; Q eats; F
toggles the flashlight. Days are 80 s, nights 50 s. The fire burns fuel; its point
light's range and the safe radius (4 m + 0.2 m per fuel point, up to 24 m) shrink
with it. Hunger drains; starving costs health; the fire heals and recharges the
flashlight. At night the Deer emerges opposite you at the edge of the light: inside
the light it circles; outside it closes in, then charges (7 s, 35 damage). Holding
the flashlight on it for 0.6 s stuns it for 3.5 s, then it flees. Wolves wander in
packs of four around dens, chase you outside the light (wider at night), bite for 8
and scatter from the flashlight. Two children are lost deep in the forest; bring
them into the light. Nights survived are counted; death shows the total and a
retry. The title picks 1k/2k/5k/20k/100k/250k trees, 8/64/512/4096 wolves, 0/16/64
extra torch lights, Rapier or grid collision, and generated pines or primitive trees.

## Limits found

Native numbers: `logic/examples/scale.rs`, release, one process per row, live clock
(`Sim::advance(.., Clock::Live)`, one tick per call, 300 day + 300 night ticks).
Web numbers: `bench.mjs`, headless Chrome WebGPU at 1280×720 CSS px, DPR 2, live
clock, `state.world.perf` after 2 s of walking; "tick" is per tick, "feed" and
"encode" per frame. Load average is given where it mattered.

### 1. Rapier trunk colliders cost O(trees) every tick — about 1.2 µs each

| trees (Rapier) | entities | live tick mean / p95 ms | save bytes | seek tick ms |
|---:|---:|---:|---:|---:|
| 1,000 | 1,095 | 0.82 / 1.13 | 3,034,986 | 2.0 |
| 5,000 | 5,242 | 5.08 / 6.80 | 14,760,968 | 12.7 |
| 20,000 | 20,792 | 24.2 / 28.9 | 58,460,177 | 59.5 |

The same forests with the game's own grid collision (`lite`): 0.021, 0.089 and
0.085 ms. Web (wasm) is worse: 3.5 ms per tick at 1k Rapier trees, 30.9 ms at 5k,
60.9 ms at 20k — so a 60 Hz Rapier forest on the web holds about **1,500 trees**,
natively about 13,000 before the tick alone fills the frame.

Cause, from `sample` at 20k: `physics::capsule(..).step` rebuilds the whole query
scene (`Scene::new`: `insert_collider` for every collider, then a binned BVH build)
on most ticks. `queries.rs` keys its cache on `w.revision::<Transform>()` — the
revision of the *whole Transform column* — so any moving entity anywhere (a wolf,
the camera's `Follow`, the sun, the flashlight's light) invalidates the scene the
character controller needs, every tick. `move_capsule` also finds its own collider
with a linear `find_map` over all colliders, and `physics::step`'s `sync` visits
every Body/Collider row per tick. Static trunks are never actually re-read; the
cost is bookkeeping. Not fixed: it needs per-row (or per-membership) change
tracking in the query cache, which is a design change, not a small fix. The game
defaults to Rapier at 2k trees and offers grid collision on the title.

### 2. A save carries Rapier's whole world: 2.9 KB per static trunk

Saves at 5k Rapier trees are 14.8 MB, 0.3 MB short of the **16 MiB surface
payload limit** (`Request::capture_surface`, README), and 58 MB at 20k. The grid
version stores 43 bytes per tree in the `Grove` resource (bulk `Vec<f32>`/`Vec<u8>`,
structural `Vec<Entity>`) plus the entity: 270 KB at 1k, 4.2 MB at 20k, 20.2 MB at
100k (over 16 MiB at about 80k trees), 50.3 MB at 250k. So an in-app
Save/Continue through the surface protocol stops at **~5.7k Rapier trees or ~80k
grid trees**; the agent's capture limit (256 MiB) is far away.

### 3. Every seekable advance observes the whole world (~1.2 µs/entity per call)

`Sim::run` (what tests, proofs and agents use) hashes and snapshots the world after
each advance: 1.6 ms per one-tick run at 1k entities, 26.8 ms at 20k, 124.7 ms at
100k, independent of what changed (`observe_with_hash` → `Stream::bytes` and
`RawStorage::snapshot` dominate the profile). A proof that walks a bot in 17–200 ms
steps through a 100k forest pays ~0.1 s per step on top of the ticks. Live hosts do
not observe, so players never pay it.

### 4. The renderer's CPU feed scales with every model instance, every frame

| scene (grid collision) | entities | feed ms/frame (two runs) | encode ms | ticks/frame | GPU forward ms | drawn (camera / cascades) |
|---|---:|---:|---:|---:|---:|---|
| 1k pines | 1,095 | 1.1, 1.3 | 0.22 | 3–5 | 4.4 | 139 / 21, 140, 973 |
| 5k pines | 5,242 | 5.1, 5.9 | 0.44 | 6 | 4.5 | 141 / 21, 132, 1,390 |
| 20k pines | 20,792 | 57.5, 64.5 | 3.4–4.5 | 8–9 | 4.4 | 137 / 23, 143, 1,411 |
| 100k pines | 103,725 | 101.5 (379 at load 18) | 5.3 | 13–15 | 4.5 | 183 / 20, 176, 1,630 |
| 250k pines | 259,225 | 259.9 | 10.7 | 15 | 6.2 | 217 / 17, 223, 2,105 |
| 5k primitive trees | 10,242 | 6.9 | 0.18 | 2.3 | 4.4 | 176 / 19, 184, 2,470 |
| 20k primitive trees | 40,792 | 28.3 | 0.19 | 4.9 | 4.4 | 175 / 19, 189, 2,418 |
| 100k primitive trees | 203,725 | 131.1 | 0.20 | 15 | 4.8 | 231 / 19, 241, 2,756 |

GPU culling works: the forward pass stays at ~4.5 ms from 1k to 100k trees because
the camera draws ~140–210 instances and the cascades ~2,000; 38 draws throughout.
The CPU does not keep up: `feed` grows with every entity per frame although no tree
ever moves — about 1 µs per generated pine at 1k–5k and 3 µs at 20k, ~0.65 µs per
primitive entity — and slow frames then run more ticks each (up to 15), which feeds
the next frame more. So on the web about **5k generated pines** (or ~20k primitive
entities) fit a 16 ms frame, 20k pines already miss 60 Hz on feed alone, and 250k
reach 1.8 fps. Encode stays small for primitives (0.2 ms at 200k) but grows for
models (10.7 ms at 250k). Headless Chrome never presented faster than ~20–25 fps
even for the 1k scene (CPU ~1.5 ms per frame), so fps here is not a pacing number.
Cause (render/src/world.rs, models.rs): when any tick completes,
`feed_to` calls `model_poses` over *every* entity with a model mesh; per entity it
reads `Mesh`, looks the asset's name up in a `BTreeMap<String, _>` and updates its
pose history, whether or not its Transform page changed — the primitive path's
page-generation skipping does not cover models. The primitive path still scans
pages per frame (its metadata "scales with allocated pages", render README), so
two primitives per tree cost about as much as one generated pine. Not fixed: a
skip for unchanged static model instances is an engine design change in the
feed's model path, and the game is not blocked (smaller forests play well).

Memory (Chrome `measureUserAgentSpecificMemory`, whole page): 25–55 MB at 1k,
34–64 MB at 5k, 62–79 MB at 20k, 221 MB at 100k, 488–502 MB at 250k pines; 168 MB
for 100k primitive trees (200k entities). Ready time from Play to first frame: 0.3 s at
1k, 1.05 s at 100k, 2.1 s at 250k. Natively, setup of 250k trees takes 145 ms and
113 MiB.

### 5. Lights: sixteen unshadowed point lights, no spotlight, and the units differ

- **No spotlight.** The flashlight is a point light pushed 4 m ahead along the
  facing and 1.6 m up; it lights a disc, not a cone. Its *effect* (stunning the
  Deer) is a cone test in game code.
- **Point lights cast no shadows** (render README: shadows are the sun's alone).
  In the night screenshot the flashlight lights the near side of a pine and the
  Deer behind it alike; trunks occlude nothing. The campfire therefore cannot throw
  tree shadows, which is most of the look of the original game.
- **Sixteen point lights per frame**, nearest the camera first, no hysteresis.
  `torches-64`: 66 lights authored, 16 drawn, forward pass 5.14 ms; `torches-16`:
  5.13 ms; the fire alone: 4.4 ms (load 7). Sixteen unshadowed lights cost ~0.7 ms
  on this GPU at 2560×1440; lights 17–66 cost nothing because they are dropped.
  (Under load 200 the same two scenes measured 8.1–8.6 ms.)
- **Units:** `PointLight.intensity` is documented "in candela" but reaches the
  shader unscaled, while `DirectionalLight.illuminance` (lux) is multiplied by
  0.0003. A 100 cd default point light is therefore ~33 times the noon sun at 1 m.
  The game authors point intensities × 0.0003 (`camp::CANDELA`).
- One directional light ("the first posed sun wins"): the moon is the sun moved and
  dimmed. Changing the sky's colours re-projects SH and re-prefilters the cube on
  every tick of dusk and dawn (36 small passes per change); the game writes the
  `Environment` only when the value changes, so full day and night cost nothing.
- Fog is exponential with height falloff only; the night look (black-blue fog at
  0.03/m) works well and hides the forest's far edge for free.

### 6. AI: ~0.4 µs per wolf per tick natively; navigation is the game's

Wolves over 5k grid trees, live tick mean (native release):

| wolves | 8 | 64 | 512 | 4,096 | 16,384 | 65,536 |
|---|---:|---:|---:|---:|---:|---:|
| tick ms | 0.025 | 0.048 | 0.21 | 1.68 | 7.1 | 36.7 |

So ~30k wolves fit a 60 Hz tick natively. On the web (load 8) 512 wolves cost
0.36 ms per tick and 4,096 cost 2.9 ms per tick plus 9.1 ms of feed per frame
(14.7 ms and 45 ms at load 221) — about 4k moving agents per web frame. Each wolf steers around the
trunks in its 3×3 grid cells and resolves overlaps; the engine has no navmesh,
pathfinding, avoidance or spatial index, and `near`/`nearest_xz` scan every entity
carrying the component, so a game with many agents builds its own grid (here, the
`Grove` resource) as a matter of course.

### 7. Smaller limits

- `state world:*` and `captureWorld` read at most 512 entities; a 2k-tree world
  cannot be captured or diffed (`truncated`), though `snapshot()` pins still work.
- A proof's `game.run(ms)` is one browser evaluation with a 15 s CDP timeout: a
  60 s seek of a 2k-tree Rapier world exceeded it. The proof seeks in 10 s steps.
- Resources are not readable through the agent (as tennis found): the proof finds
  a tree through a named marker entity (`first-tree`) and reads HUD text.
- **The Save proof mode multiplies cost by the save size.** With a 5.9 MB save
  (Rapier, 2k trees), Linux Save mode took 205–310 s against 15 s Off, web Save
  507 s against 117 s, and a 10 s web seek no longer fits the 15 s CDP window.
  A first baseline needs all six rows, so every gameplay change that moves a pin
  costs ~25 minutes here.

## Engine changes

None. Every limit above was hit with the engine as it is on main; none blocked the
game, and the fixes they call for (per-row change tracking for the physics query
cache, skipping unchanged static model instances in the feed, point-light units, a
spotlight and point-light shadows) are design changes rather than small repairs.
They are recorded as `QUEUE.md` lines.

## Determinism and saves

Pinned on Linux (arm64 macOS host) and Chrome wasm in all three modes plus the
Linux release build: tick 0 `0x828be9d37d1f3469`, tick 4500 — 75 s in, after
chopping, carrying and feeding the fire, with the Deer stalking at night —
`0xcc809bc72cd80213`, and the continuation save after the flashlight stun
`df2abeca…` (5,911,267 bytes). The proof saves mid-night, restores in a fresh
process, plays the same Deer encounter with the same keys and gets byte-identical
saves; `--compare-saves` confirms Linux and web saves are identical. Hostless,
`a_mid_night_save_restores_and_continues_identically` does the same with and
without Rapier, and the scale probe restores every row (to 250k trees) to the same
hash. Determinism needed nothing beyond the README's rules: the world RNG for
placement and dens, `exact_game::math` for the trigonometry, and wolves' wander
turns derived from their own state. No `HashMap`; no lint fired.

## Friction log

1. **The SDK lock is stale on main again** (`cssparser`, `dtoa`, `dtoa-short`):
   every new game must `--update-lock` before its first bake. 1 minute.
2. **Adding an SDK crate (physics) needs `logic/Cargo.toml` and re-locks.** The
   error says how; the README section is right. 1 minute.
3. **`button` has no `aria-pressed`.** A toggle button cannot expose its state to
   assistive technology; the title shows the selection as a text line instead.
   All twelve errors arrived in one bake (15 s), which is how it should be.
4. **Point-light units** (limit 5): one wasted web round (5 min) to a white frame.
5. **No spotlight, no point shadows** (limit 5): the flashlight and the fire are
   the two lights the game is about; neither can be drawn as intended.
6. **Eight-way keyboard facing vs a cone test**: the proof's bot needed an aim loop;
   a mouse-aimed flashlight would need a pointer-to-ground pick each tick.
7. **One CDP timeout per long seek** (limit 7). 3 minutes.
8. **Seekable advances cost O(world)** (limit 3): the scale probe first measured
   observation, not ticks; switching to `Clock::Live` found the real tick costs.
9. **The query scene's whole-column revision** (limit 1): found by profiling, not
   by reading docs; the physics README does say "a relevant edit still costs an O(n)
   scene rebuild", but not that any Transform write anywhere is relevant.
10. **Helper scripts are bake inputs** (tennis 10): editing `bench.mjs` changes the
    game's input digest; I stopped one baseline to format sources rather than
    invalidate it.
11. **Machine load**: the sibling lanes pushed load from 7 to 221; headless numbers
    above 20 are marked or interleaved, and the display is locked, so no vsync'd
    frame pacing could be measured at all.

## What was pleasant

- **Grid lookups made a 250k-tree world trivial to simulate**: 0.06–0.43 ms ticks
  at 100k–250k with collision, navigation and wolves; setup 145 ms.
- **GPU culling is real**: 100k pines draw ~180 instances from the camera with the
  forward pass unchanged; three shadow cascades cull to ~2,000.
- **Generated meshes** (`w.generated`) gave a flat-shaded pine and the terrain from
  ~60 lines of code; one GPU upload, one draw group for 250k instances.
- **Saves were free again**: the Grove's bulk vectors, the Deer's and wolves'
  enums, carried items as parented entities, Rapier's world — all restored
  byte-identically in a fresh process mid-night on the first try.
- **The fire's light, fog and bloom at night** look like the game with no shader
  work: see `artifacts/screens/night-2k-deer-stunned.png`.

## After the fix lane (2026-10-03, `roblox/integrate` merged at 470dc4c4)

The engine lanes landed photometric light units, `SpotLight`, `LightShadows`,
clustered local lights past 16, a second `DirectionalLight` as fill, SSAO,
incremental physics colliders with static-collider holes in saves, per-page feed
and observation, columnar saves (EXGAME v4), a sampled Save proof mode,
self-chunking seeks, paged `state`/`snapshot({all})`, readable resources and
`aria-pressed`. The game was adapted, not redesigned:

- `CANDELA` is gone; point and spot intensities are plain candela again.
- The flashlight is a `SpotLight` (inner 0.25, outer 0.5 rad, 18 m, 250,000 cd)
  from the player's chest, tipped toward the ground 12 m ahead, with
  `LightShadows`. The campfire's point light has `LightShadows` too: the player,
  logs and stones throw shadows away from the fire by day and night. The moon is a
  second, unshadowed `DirectionalLight` (900 lux at night); the sun fades out and
  drops its shadows after dusk.
- The proof no longer chunks seeks, finds the nearest tree by reading every page
  of the world (`snapshot({all: true})`, 2,000 trees) instead of a named
  `first-tree` marker entity, which is gone; title toggles carry `aria-pressed`.
  A `torches-256` choice was added.
- **A correction to limit 4.** After the merge the web feed was still 31–47 ms at
  20k pines and 330 ms at 100k–250k, though the engine's own `feed_cpu_cost`
  measures 0.008 ms for 200k static primitives. The cause was the game: the Deer's
  loop queried `&mut Visible` every tick, and a mutable borrow alone advances the
  column's revision, so the renderer rebuilt every batch every tick. Writing
  `Visible` only when it changes took the feed to 0.13–2.2 ms. Most of what limit 4
  attributed to `model_poses` before the fix lane was this; the README now says so.
  The engine lesson stands as friction: nothing warns that a `&mut` query term on a
  render-relevant column costs O(world) per tick even when no value changes.

Before → after, same machine and probes (native: `scale.rs`, release, live clock;
web: `bench.mjs`, headless Chrome, load 10–28, interleaved where it mattered):

| measurement | before | after |
|---|---|---|
| Rapier tick, native, 1k / 5k / 20k / 100k trees (mean; p50) | 0.82 / 5.1 / 24.2 / — ms | 0.25 / 0.11 / 0.91 / 5.1 ms; p50 0.07 / 0.07 / 0.17 / 0.61 |
| grid-collision tick, native, 1k / 20k / 100k / 250k | 0.021 / 0.085 / 0.43 / — ms | 0.019 / 0.093 / 1.25 / 2.4 ms (p50 0.46 / 1.12 at 100k / 250k; load 37) |
| Rapier tick, web, 1k / 5k / 20k | 1.25 / 5.75 / 24.0 ms | 0.20 / 0.21 / 0.30 ms |
| save, Rapier, 1k / 5k / 20k / 100k | 3.0 / 14.8 / 58.5 / — MB | 0.43 / 2.0 / 7.4 / 36.8 MB |
| save, grid, 1k / 5k / 20k / 100k / 250k | 0.27 / 1.19 / 4.16 / 20.2 / 50.3 MB | 0.15 / 0.62 / 1.88 / 8.7 / 21.4 MB |
| one seekable `Sim::run` tick, grid, 1k / 20k / 100k | 1.6 / 26.8 / 124.7 ms | 0.24 / 0.83 / 8.6 ms |
| web feed per frame, 5k / 20k / 100k / 250k pines | 5–6 / 58–65 / 102 / 260 ms | 0.13 / 0.21 / 0.84 / 2.2 ms |
| web feed, 100k primitive trees (204k entities) | 131 ms | 1.1 ms |
| web tick + feed + encode per frame, 250k pines | ~275 ms (15 ticks/frame) | ~13 ms (4.6 ticks × 2.2 + 2.2 + 0.75) |
| web, 5k trees + 4,096 wolves | tick 2.9, feed 9.1 ms | tick 2.6, feed 7.8 ms (before the `Visible` fix) |
| local lights, 2k trees, forward pass: fire only / 16 / 64 / 256 torches | 4.4 / 5.1 / 5.1 (16 drawn) / — ms | 5.2 / 6.3–7.3 / 6.0–7.0 / 5.5–8.6 ms, all drawn; encode 0.44 → 2.0 ms at 256 |
| Linux proof, Off / Save mode | 15 / 205–310 s | 3.3 / 19.9 s (13.2 / 23.1 s with a GPU rebuild) |
| web proof, Off / Save mode (incl. build) | 117 / 507 s | 87–101 / 149 s |
| hostless tests (9) | 8.6–30 s | 0.3–1.0 s |

Rapier now costs about what the grid does per tick (0.17 vs 0.10 ms p50 at 20k)
and stays the default; its saves remain 4× the grid's (36.8 vs 8.7 MB at 100k), so
the 16 MiB surface limit now arrives at about 45k Rapier trees or 190k grid trees.
A native full `World::hash` of a Rapier world is still slow (660 ms at 100k, 0.05 ms
without Rapier), though seeks no longer pay it. The flashlight's shadowed spot
and the fire's shadows show in `artifacts/screens/night-camp-after.png` and
`day-camp-after.png`; the stunned Deer in `night-2k-deer-stunned-after.png`. Local
shadow views have no row of their own in `world.gpuMs`, so their cost is folded
into the forward pass above. SSAO was not tried.

Pins: the merge moved every hash (paged hash, EXGAME v4, EXPHYS v3), and the game's
changes (no marker entity, the moon, the `Visible` write) moved them again. Linux
and web agree with each other in every run (tick 0 `0x9fa895b6db68d24f` in the last
web Save-mode run), and every gameplay check passes; only the three pin checks
fail. Not re-pinned, as asked.

## Art pass (2026-10-03, branch `art/forest` from `roblox/integrate` e083e8217)

Brief: draw much better graphics with the model authoring all the art itself (no
asset packs), keep the game deterministic and the proof at PASS, keep 5k+ trees
viable on the web, and log where the engine stopped the art.

### What was made and how

Everything is generated by `artgen/gen.mjs` (335 lines) over `artgen/lib.mjs`
(272 lines): a seeded RNG, tileable value noise and fBm, a PNG encoder (zlib from
`node:zlib`, CRC by hand), a mesh builder (tubes along ring paths, displaced
icospheres, crossed alpha cards) and a binary glTF writer. `bun
game/games/forest/artgen/gen.mjs` writes 22 files into `art/` in 0.7 s
(1.24 MB: 20 `.glb` with embedded PNGs, two RGBM sky PNGs); the ordinary art bake
turns them into `assets/`.

- **Trees, five kinds**: three pines (four, six and three tiers of jagged,
  drooping cones over a crooked trunk; one dead and brown) and two broadleafs (an
  oak and a birch: trunk, five branches, six to eight lumpy leaf clumps cut out of
  an alpha leaf-card texture). Textures: streaked bark ×3, birch bark with
  lenticels, needles ×3, leaves ×2. Vertex colours carry baked AO (darker toward
  the trunk base and inside the crown). The forest picks kinds by share
  (pines 76%, oak 14%, birch 10%).
- **Undergrowth**: ferns (seven alpha fronds), bushes, grass tufts and two
  mossy rocks, on about half the cells (≈3k props at 5k trees), all `Ambient`.
- **Ground**: still the generated heightfield, now sampled every 2 m (≤20k trees)
  with moss / soil / leaf-litter patches from interfering sine waves and a worn
  ring of earth round the fire, in vertex colour only (see limits).
- **Characters, part-built and animated**: the Deer (a hunched black-furred body,
  long neck, a bone skull with jaw, a crown of branching antlers with tines,
  emissive ember eyes, four long legs with backward knees), wolves (body, head,
  ears, tail, glowing eyes, four legs) and the survivor (jacketed torso, head,
  hair, backpack with straps, two arms and two legs; children reuse it at 0.62
  scale, tinted). A new 99-line `rig.rs` gives owners a distance-driven `Gait` and
  parts a `Limb` that swings about its pivot, so legs and arms walk at the speed
  the creature actually moves and stop when it stops. The survivor turns on a
  child pivot because the capsule controller must stay upright; carried logs now
  strap onto the backpack.
- **Campfire**: four charred logs with glowing end grain, eleven stones, three
  seat logs; two additive flame layers (core and outer, sphere-shaped emission,
  smooth easing), a straight-alpha smoke column that drifts with the wind, embers;
  the shadowed point light flickers on three incommensurate waves; all rates and
  the coals' size follow the fuel.
- **Sky and grading**: a day and a starry night RGBM equirect (Milky Way band,
  moon) as the `EnvironmentMap`, swapped past 60% darkness at stepped
  intensities; the procedural sky, fog and sun colour run through a warm dusk
  (strongest halfway into the dark); a low ground fog (height falloff 0.12) that
  thickens at night; exposure 1.0 → 1.4 at night; `AmbientOcclusion` on.
- **Wind**: trees within 30 m of the player lean on slow, position-shifted waves.
- **Flashlight**: the shadowed `SpotLight` plus a blended, emissive open cone
  (`beam.glb`, alpha fading with distance) that reads as a volumetric beam.
- **HUD**: day/phase with a sun or moon glyph and a progress bar for the phase,
  a fire panel with a fuel bar, health/hunger/battery rows with icons and bars,
  a Deer threat panel that turns red on a chase, a pill-shaped prompt.

Game code: `forest.rs` 437, `camp.rs` 441, `creatures.rs` 333, `player.rs` 497,
`rig.rs` 99, `app.contract` 173 lines (the diff from the pinned game is +1,500 /
−205 lines including generated art and code). Gameplay changed only where the art
needed it: creatures stand on the ground, new components, more entities, sway
and hidden crowns are in the tick; pins were moved with a reason.

### How it looks and feels

Before/after: `artifacts/art/before/{day,forest,dusk,night,deer}.png` and
`artifacts/art/after/{day,forest,dusk,night,deer,day-proof}.png`.

Honestly: a large step. By day the clearing reads as a place, with a person in a
red jacket by a smoking fire, logs with end grain, ferns, mossy rocks, a mixed
pine/oak/birch wall with long shadows. Dusk is warm and hazy. At night the fire's
shadows of the logs, stones and ferns are the best thing on screen; the smoke is
lit from below; the beam gives the flashlight a body; the Deer is a dark, tall,
antlered silhouette with two coals for eyes. It is still low-poly greybox-plus:
the pines read as stacked lampshades from above, the ground is a smooth colour
field at 2 m resolution, the Deer is mostly lost in the dark (black fur, no rim
light), animation is a sine walk with no body motion, and nothing in the sky is
ever on screen from this camera, so the stars only light the scene.

### Cost (headless Chrome WebGPU, Apple M5 Pro, load 8–14, `bench.mjs`)

| scene | build | entities | draws | triangles | tick ms | feed ms | encode ms | forward GPU ms | cascade 2 ms | ready ms | page memory |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 2k trees | before | 2,132 | 74 | 144k | 0.13 | 0.15 | 0.32 | 2.1 | 0.26 | 213 | 40 MB |
| 2k trees | after | 3,313 | 481 | 1.76M | 0.83 | 0.51 | 0.67 | 2.5–6.4 | 1.1–3.1 | 628–1,056 | 60–90 MB |
| 5k trees | before | 5,242 | 74 | 338k | 0.17 | 0.15 | 0.36 | 2.8 | 0.30 | 315 | 81 MB |
| 5k trees | after | 8,290 | 489 | 4.39M | 0.56 | 0.51 | 0.46 | 2.6 | 1.14 | 1,027 | 93 MB |
| 5k, grid collision | after | 8,290 | 493 | 4.39M | 0.12 | 0.65 | 0.58 | 3.2 | 1.42 | 416 | 78 MB |
| 20k trees | before | 20,792 | 74 | 1.31M | 0.21 | 0.13 | 0.32 | 3.6 | 0.39 | 1,496 | 218 MB |
| 20k trees | after | 32,991 | 483 | 17.0M | 0.81 | 0.82 | 0.46 | 1.8 | 0.95 | 1,186 | 235 MB |

("before" is the pinned game's dist from `wt/forest`, served by the same probe.)
5k trees stay comfortably viable: under 2 ms of CPU per tick + frame work and
about 4 ms of GPU passes, with GPU culling keeping the camera to 250–320
instances out of 15k. Draws went from 74 to ~485 because every model primitive is
its own batch per pass (camera, three cascades, the point light's six faces, the
spot), and submitted triangles 12× — culled, so the forward pass barely moved;
the shadow cascades (casters are not culled as tightly) rose most. The tick cost
went up 0.4–0.7 ms for sway (≈150 trees written per tick) and limb swings.
Sizes: the GPU module is unchanged (2,286 KiB, 927 KiB gzip); the game now loads
436 KB of `.model` (155 KB gzip) plus one texture family — 1.6 MB of BC7 on this
Mac (45 textures), 5.1 MB RGBA8 headless. Load (Play → first frame) went from
0.2–0.3 s to about 1 s. Native offscreen GPU numbers were not taken: the native
GPU path for a game is the macOS host, and a cold Apple build was not worth its
time against headless Chrome's real Metal timestamps.

### Where the engine and tooling limited the art

1. **No custom shaders in a game.** `game/app/shells.mjs` writes the GPU shell as
   `module!(Game[, audio][, assets])`; there is no `app.json` key for `hooks` or
   `shaders`, and a game's logic crate cannot implement render `Hooks` without
   depending on `exact-game-render`. So no vertex wind (sway is CPU-side, per
   tree, in the tick), no fur/foliage shading, no procedural star sky, no real
   volumetric light, no dithered fade of occluders. The render-hook machinery
   exists (LLP 1046.006, `tests/hooks.rs`) but nothing wires it to a game.
2. **Generated meshes cannot be textured.** `World::generated(name, MeshData)`
   builds a single default material with no texture, so the terrain — the one
   mesh whose size depends on the world — gets vertex colour only, at its vertex
   spacing. Everything textured had to become a baked `.glb`.
3. **Textures are per model.** A `.glb`'s embedded images bake as
   `<model>/<i>-srgb-straight.tex`; the same bark is baked, shipped and uploaded
   three times (45 textures for ~20 distinct images). glTF with external image
   URIs shared across models was not tried.
4. **The sky is never seen and never textured.** An `EnvironmentMap` lights the
   scene only ("the visible sky and background stay procedural"), so a starry
   night sky can be lit by but not shown, even from a camera that looks up.
5. **Standalone PNGs bake as sRGB, nearest-filtered sprites.** An RGBM sky has to
   be encoded around the sRGB decode (rgb stored through the sRGB curve, alpha
   linear); there is no way to mark a PNG as linear data or linear-filtered.
6. **No occluder fading.** Crowns between the camera and the player are hidden
   outright (`Visible(false)`): models have no per-instance opacity and opaque
   materials cannot fade, so the cut pops.
7. **`Visible` and mutable borrows cost a re-batch.** Writing `Visible` (or merely
   borrowing it mutably) rebuilds every batch; the hidden-crown cut reads first and
   writes only on change.
8. **Characters are rigid parts.** Skinning works (the Fox), but authoring a
   skinned, animated glTF from code means joints, inverse binds, weights and clips;
   parts under `Parent` with a tick-driven swing were the affordable path, at a
   stiff, puppet-like look.
9. **A template regression found on the way**: after LLP 1080.002 plain `tree`
   carries no `accessibleName`, but the starter's proof (`game/new/proof.mjs`)
   still checks it, so every newly scaffolded game's proof fails on main. This
   game's proof now reads `props.accessibilityLabel`.
10. **The game's lock had to be re-captured** (+233 lines) once `art/` existed:
    the bake's glTF/image crates join the game's Cargo graph.

Proof: re-pinned with a reason (tick 0 `0xdd3bb86ac949b93b` → `0x5da20806bf0c10ee`,
tick 4500 `0x1297ea42b0978dee` → `0xc3832b82d85a125c`, night save `b87f97ba…` →
`59392d3f…`; Linux and web agreed in Off/Save/FreshGame and the Linux release run,
10 min 6 s), then `bun game/prove.mjs forest --hosts linux,web --compare-saves`:
**PROOF PASS** (Linux 3.2 s, web 61.2 s, hashes equal, saves identical).
