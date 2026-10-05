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

## Agent-directed rescue play (2026-10-04)

Charlie's continuing brief is to improve Garden, Forest and Rivals by playing them,
including Jev choosing player actions, and feeding the friction into the engine.
This increment, by Codex, started from main at `6274fbab3` and fast-forwarded to
`2bd0c91c9`; subsequent origin checks found no further commits during implementation.

The children existed, but a player had no direction to them and rescue changed
only a counter. Added a visible eight-direction compass with distance, switching
to the campfire while escorting, and a one-time reward of 20 fuel and two food at
camp. Rescues now require a living player and a lit fire; previously the
`safe.max(3)` fallback rescued children at an extinguished camp. The saved child
fate is the reward receipt; no new counter or save protocol.

The shared `game/proof.mjs` now exposes `decide`: a bounded Jev request over a
game-supplied observation and named choices. Credentials stay in the driver;
responses cannot turn into arbitrary commands. A rejected request stops the step,
and the JSONL contains observations, choices, probabilities, latency and token use.
Forest's `--playtest` makes at most 48 decisions using **only the visible HUD**.
The script translates “follow” into a short key hold along that compass. Jev
chooses when to move, interact, eat, wait or toggle the light. This is strategic
play on the agent clock, not visual perception or human input-latency evidence.

| host | decisions | outcome | decision p50 / p95 | tokens in / out |
|---|---:|---|---:|---:|
| web | 39 | both rescued, 87 health, 37 s until night | 269 / 539 ms | 26,377 / 1,693 |
| macOS | 40 | both rescued, 87 health, 37 s until night | 327 / 633 ms | 27,093 / 1,742 |

Both runs use seed 7, 2,000 trees and eight wolves. Different model choices are
expected; these are exploratory outcomes, explicitly `UNVERIFIED` by the proof
runner, not fixed hash evidence. Screenshots were inspected on both hosts. The
artifacts are `artifacts/web/jev-*` and `artifacts/jev-macos/jev-*` in the game.

The separate scripted rescue walks to a child with real keys, saves mid-escort,
walks home, checks the fuel and food, then repeats from the save in a new process.
The web run passed all gameplay assertions in 28.1 s, including byte-identical
rescue saves. macOS passed the same script in 78.5 s; its observed pins and all
six save digests match web. Eleven hostless tests cover the old loop plus guidance, one-time
supplies, extinguished fires, death and saved continuation. Four driver tests
cover valid decisions, invalid choices, missing keys, size bounds and error
redaction. Root build, tests, Clippy, format, caps and boot passed.

Friction and lessons:

- Main's dependency graph had moved beyond Forest's lock/cache (`chrono-tz` was
  the first refusal). Fetching the generated workspace and refreshing its existing
  lock repaired it; no authored dependency or new manifest was needed.
- `wasm-bindgen` was installed but absent from this shell's PATH. `exact setup`
  reinstalled it and still reported it missing. Explicitly including
  `~/.cargo/bin` and `~/.cache/exact/binaryen/version_132/bin` passed `setup --check`.
  The first successful web artifact was unoptimized; this is not a size result.
- The first rescue proof read a unit enum as a string; inspection showed
  `{Food:{}}`. Correcting that assertion took one host-test retry; game behavior
  and restored save equality already passed. Inspected enums need a clearer
  example in the agent reference.
- Native cold startup work dominated the development loop: 79.2 s Rust and
  173.7 s Swift, 253.7 s total build. These are cold-build costs, not a regression
  measurement or a claim about warm iteration.
- The broad driver suite took 485 s cold (477 s was the existing Beacons/skinned
  release-equivalence test), passing 101 tests and finding two stale fixtures:
  the CLI-body fixture omitted `layoutArgs`, and the phone mock still intercepted
  `build.mjs` after device discovery moved to `devices.mjs`. Updated those doubles;
  both focused reruns passed, then the full 103-test suite passed warm in 9.11 s.
  Neither failure was in the new game or Jev helper.
- Giving the player useful information also gave Jev useful information. A
  private “AI knows every child coordinate” interface would have hidden the
  missing player guidance. Keep model observations tied to the player-facing
  experience when testing discoverability; keep exact-state scripted proofs for
  reproducibility.

The cross-host repin exposed a host discrepancy the web/macOS pair did not:
Linux returned `NoHandler { event: "message" }` when the rescue emitted its
notification into a canvas without a Contract message handler. All three Linux
modes reached that same event; web's three modes completed the rescue. The web
glue explicitly checks `messageViews` before dispatch, whereas Linux dispatched
every string and turned the missing handler into a clock error. Fixed both the
ordinary tick and restore delivery paths in `host/linux/src/surfaces.rs`: dispatch
only to an authored Message handler. Registered handlers still receive every
message, and the internal audio notification is still ignored on the headless
host. The 42 surface tests pass, including a new listening/unlistening regression;
the complete Linux rescue script then passed in 8.43 s including the warm build.
No dummy message handler was added to Forest.

Main was fetched again and merged through `c7bb6ab9d` after the first verification
run finished. This avoids invalidating an in-flight baseline with a different
source tree while still following the fast-moving input/compiler work.
The root gates and 103 driver tests pass after that merge. The updated macOS
build and scripted run also pass (146.8 s including rebuild); its optional `ps`
descendant audit timed out, while every owned carrier handle exited normally.

The final repin succeeded: Linux and web agree in normal, Save and FreshGame
modes, and the native release profile agrees too. macOS's independently driven
run matches the accepted pins and save digests. Evidence is under
`artifacts/prove/run-3AZz51`; tick pins stayed unchanged, while the serialized HUD
changed the night save and the new rescue save adds coverage. The ordinary Linux
proof now reports `PASS` against those accepted pins. The next campaign batch
should exercise Garden's economy and Rivals' combat with the same decision seam,
then feed their observed problems back into gameplay and the engine.

## Supply runs after the rescue (2026-10-04)

This batch began from the validated Rivals checkpoint `f53bc97a2`. Main had
advanced twice, through `843dd48d6` (Apple's unused Canvas 2D module omission and
Ibex's Windows connection readiness); merge `16786a177` was clean. Forest's own
lock then refused the moved dependency graph. The prescribed `--update-lock`
refreshed four dependency edges, with no package-version changes.

The old Jev run stopped as soon as the two children reached safety. Extending
the goal to two nights exposed the next problem: at the 128-decision limit the
player carried **three logs while the fire was out**, had 11 hunger and 65 health,
and had survived one night. The sole objective read “All children safe · Keep
the fire burning”; it no longer gave a route home or toward supplies. The final
43 choices toggled a flashlight that eventually had no charge. The controller
had four raw walking directions, but no distance to camp to guide them.

Added player-facing Children, Fuel, Food and Camp compass buttons. Camp's bearing
remains visible beside the chosen route. Fuel chooses loose logs/scrap, then a
standing tree if no loose fuel exists; food chooses an uncollected food item.
The chosen entity is saved in `Trail` and stays fixed while walking. Selecting a
route or collecting supplies refreshes it; looking at the HUD does not scan the
forest every frame. A full pack with fuel points home. An empty food search
refreshes when dawn or a rescue supplies food. The latter was a review-found
invalidation bug: its new regression failed on the rescue branch before the fix.
The HUD also explains each supply's value.

The night score had a separate defect: a dead player's `survived` counter kept
increasing at dawn while the death screen remained open. A test reproduced 1
where 0 was expected. The calendar still advances, but the score now increments
only for a living player.

Jev still sees only the rendered HUD and available buttons. The model never
receives child/item coordinates, and the motor never writes the world. The first
new runs reached camp but oscillated east/west across it: each raw direction
walked six metres, competing with the near-target compass motor. The last policy
revision offers those directions as detours only after two compass strides fail
to reduce the visible distance; it also stops offering an empty flashlight's
unusable toggle. Its follow description now covers supplies as well as children.

| Run (128 decisions each) | Game seconds | Nights | Health / hunger | Fire | Decision p50 / p95 |
|---|---:|---:|---|---:|---|
| Baseline web | 235.7 | 1 | 65 / 11 | 0% | 304 / 629 ms |
| Compass A, web | 124.8 | 1 | 100 / 44 | 41% | 281 / 465 ms |
| Compass A, macOS | 89.5 | 0 | 100 / 88 | 87% | 332 / 666 ms |
| Compass B, web | 134.0 | 1 | 100 / 73 | 62% | 324 / 556 ms |
| Compass B, macOS | 63.0 | 0 | 100 / 99 | 97% | 310 / 585 ms |

Every run rescued both children, but **none completed the two-night goal**.
These are different decision sequences and different simulated durations, not
a survival-rate or host-parity comparison. B removed the zigzag and made supply
runs, but still spent 44/48 choices on individual interactions and six each on
eating. Its 100 ms interaction step is shorter than chopping's 350 ms cooldown;
incidental chop prompts also divert it from a chosen food route. This batch's
baseline and two revisions are closed. Interaction granularity and cooldown
feedback are the next separate target, not more tuning until a win.

Artifacts are `artifacts/jev-survival-baseline-web/`,
`jev-survival-trails-{web,macos}/`, and `jev-survival-trails-{web,macos}-b/`.
Input/output token counts respectively: 99,041/8,657; 108,646/12,655;
110,365/12,933; 103,793/9,532; 103,771/9,519. Model wall time never advances
the game clock. Every descendant audit passed except A on macOS, whose optional
`ps` audit timed out; both B audits passed. The first native rebuild took 54.9 s
Rust and 3.9 s Swift, rather than the older 173.7 s Swift cold build.

Friction from implementing the feature:

- The determinism lint rejected `f32::powi(2)` in a distance comparison. Plain
  multiplication is enough; no host-dependent transcendental function is needed.
- The supply save test initially checkpointed directly after its arranged
  `World::teleport`. Teleport invalidates the camera's follow, and restore places
  that pending follower; the immediate save roundtrip differed. Completing the
  arranged tick before the gameplay checkpoint makes the continuation agree.
  The pending-teleport checkpoint behavior is queued for an engine investigation,
  rather than silently presented as a general save guarantee.
- Rounded “2 m” could hide a food target just outside interaction reach if it
  reused the child's two-metre threshold. Supply bearings retain a direction
  until 1.5 m; a 1.75 m case is covered by the test.

The 15 simulation tests and determinism lint pass, including saved landmarks,
collection retargeting, the tree-to-log fallback, new supplies and the frozen
death score. Root build, test (2,285 enabled, nine ignored), clippy, formatting
and boot pass on the merged main. The normal web proof passes all gameplay,
rescue and new supply checks in 75.7 s, including a fresh-process supply run
with identical continuation bytes. Its screenshot shows readable controls and
the persistent camp bearing. The final macOS drive passes in 99.7 s; its screenshot
also shows readable controls. Its owned processes exit, while the optional `ps`
descendant audit times out.

The strict collector `artifacts/prove/run-Tnufcz` accepts the new baseline after
Linux and web agree in normal, Save and FreshGame modes, plus Linux release.
All seven gameplay runs have zero failures and successful descendant audits.
Their source-input digest, both tick pins, all six world snapshots and nine save
digests also match the independent macOS drive. Web's three modes take 84.1,
108.3 and 116.0 s; the release comparison takes 80.0 s including a 54.8 s rebuild.
The new `supplies` continuation pin records collecting fuel, returning home and
feeding the fire through the visible compass controls. This completes the supply
feature checkpoint; the two-night Jev objective remains unfinished.

At that clean checkpoint, main had advanced another 88 commits through
`6cfb736a8`. Merge `72722721f` retains main's visible-overflow hit testing and
this branch's passive surface-control guard. The queue conflict keeps both lanes'
entries. Sources stayed fixed through the preceding collector; fetching and
merging happen between verification batches.

The rebuilt normal Forest proofs pass on this merge: web 148.4 s and macOS
141.3 s, including their rebuilds. Their input digests, both tick pins, six world
snapshots and nine saves agree. Web's process audit passes; macOS again reports
the optional descendant audit unavailable while every owned carrier exits.
The new macOS supply screenshot shows the forest through the HUD panels after
main's capture fix. Root build, 2,313 enabled tests (nine ignored), clippy,
formatting, caps and boot pass; 34 focused native input/transform/capture tests
also pass. The pending camera-follow checkpoint investigation remains queued.

## Pending camera work is part of a checkpoint (2026-10-04)

Main advanced through `39ac858b2` before this batch; merge `01aaab7ff` was clean.
The queued teleport case reproduced in both Forest and the engine. It is more
than different save bytes: after teleporting the player to x=50, the original
camera reaches x=51 on the next tick, while the restored camera reaches about
x=50.10516. Restore initialized `Follow` before the next game tick moved its
target, turning the original snap into a smoothed step. A newly spawned follower
has the same defect, and both `restore` and `restore_bound` exhibit it.

Removed follower placement from restore. Setup and setup-argument rebuilds still
place followers; restore keeps their saved poses and pending work until the usual
scene step. No new save field, format or API is needed. The engine guide now
states that ordering explicitly. Forest's supply test no longer advances an extra
tick to normalize its arranged teleport: its immediate save roundtrip and later
collection/retarget continuation now agree directly.

The engine regression exercises newly spawned, teleported and retargeted followers
through both restore methods. A second case teleports after an explicit follow
inside a game tick, then checks the checkpoint and subsequent ticks in Save and
FreshGame modes against ordinary execution. Engine tests pass (401 enabled,
seven ignored, including the added case); Forest's 15 tests, Garden's 13 simulation tests and
Rivals' 23 enabled tests pass. Garden's stale lock needed the prescribed refresh,
adding two dependency edges without package-version changes. Cross-host app drives
follow this checkpoint.

The app sweep passes with the fix committed as `b37d46a2c`: Forest web 157.7 s
and macOS 117.5 s, including rebuilds, with matching source inputs, both existing
tick pins, six snapshots and nine save digests. Rivals also passes on web
(122.1 s) and macOS (29.7 s), with matching pins, six snapshots and six saves.
Garden's full Linux/web collector, release comparison and independent macOS run
agree too (diary 004). Forest and Garden's optional macOS descendant audits time
out, while every owned carrier exits; the other audits pass. This closes the
pending-teleport queue item without a special case in Forest.

Root build, 2,314 enabled tests (nine ignored), clippy, formatting, caps and boot
pass. The fetched main had put `kernel/src/style.rs` three lines over its source
cap; shortening comments restores the cap without changing behavior. Engine
clippy and the edited game package's formatting pass as well. The key lesson is
that restore is not another setup: pending work must keep its place relative to
the next game tick, even when initializing it early appears visually convenient.

## Axe feedback and the size of an interaction (2026-10-04)

Started by merging 51 main commits through `85bd9ba9b` as `6250a2856`. The queue
conflict preserves both lanes; the style conflict keeps main's extraction of
touch-action into its own module. This includes new Apple paint ranks and shared
host-module caching. Garden's unchanged five-order proofs pass on web/macOS
(88.9/42.0 s including rebuilds); the native optional descendant audit times out
while its owned carriers close, and web's audit passes.

The unchanged Forest policy again spent many decisions on interactions: 34 on
web, 60 on macOS, including 14/40 axe presses. The game rejects E during the
350 ms swing recovery but continued showing “E: chop”. A regression establishes
that a second press 100 ms later leaves two hits, then fails on that misleading
label. Its initial test-only borrow error was corrected before this behavioral
failure was recorded.

Revision A makes the prompt derive readiness and remaining hits from the saved
Player cooldown and Grove hit points. Recovery rounds up to tenths of a second;
the prompt does not offer E until it works. Jev receives the same label and can
wait its displayed duration. Across the two runs, all 25 observations of recovery
lead to waiting (21) or walking (4), with no E option offered during recovery.
No new engine state, event channel or agent-only command is needed.

The final revision makes **holding E repeat axe swings** at the existing cadence.
The hint says “Hold E: chop”; pickups, feeding and rescue require a fresh press.
This gives people a continuous chopping action and lets Jev use a one-second
hold through the ordinary input API. It does not collect the dropped logs.
The engine's saved held-input state already supports resuming midway through a
hold: a regression exercises that and the cooldown with both Rapier and the
game's grid collision, then compares continuation bytes.

| Run (128 decisions each) | Game seconds | Axe decisions | Health / hunger | Fire | Decision p50 / p95 |
|---|---:|---:|---|---:|---|
| Baseline web | 78.1 | 14 presses | 93 / 96 | 80% | 367 / 706 ms |
| Baseline macOS | 58.9 | 40 presses | 100 / 98 | 95% | 354 / 828 ms |
| Recovery web | 80.0 | 12 presses | 100 / 65 | 97% | 291 / 588 ms |
| Recovery macOS | 76.7 | 13 presses | 100 / 99 | 97% | 329 / 641 ms |
| Hold web | 88.7 | 5 holds | 100 / 61 | 100% | 317 / 515 ms |
| Hold macOS | 88.6 | 7 holds | 100 / 61 | 92% | 307 / 481 ms |

Every run rescues both children; none reaches even the first dawn before its
decision limit, so the two-night objective remains unfinished. The final
screenshots show five/seven trees felled, agreeing with five/seven hold decisions.
They also show readable night HUDs, and the supply policy still gathers fuel
with a nearly full fire. Late repeated E presses on web were productive food
pickups, not axe cooldown failures. The baseline and two interaction revisions
close this batch's three-round loop. Further survival work should examine the
public post-rescue objective and milestones, rather than continue this tuning.

Artifacts: `artifacts/jev-cooldown-baseline-{web,macos}/`,
`jev-cooldown-{web,macos}/`, and `jev-hold-{web,macos}/`. Input/output tokens,
in that order: 103,941/9,594; 105,110/9,685; 103,336/8,735; 105,304/9,425;
102,600/8,769; 102,421/8,728. Wall times including rebuilds: 111.3/113.4 s,
100.2/91.0 s, 81.7/86.1 s. Model wall time does not advance the game clock.
Baseline and final macOS runs report the optional descendant scan unavailable
while every owned carrier closes; the other four audits pass. The final web and
macOS screenshots and recovery macOS screenshot were inspected.

One diagnostic limitation surfaced: these post-playtest JSON files use the
default, paginated world snapshot. In Forest that page ends among the trees,
before Player, so it cannot answer how many swings were accepted. Tick/hash and
the visible HUD outcomes remain valid. Named component reads or a full snapshot
after the loop are queued; no hidden world state should enter Jev's observations.

The normal host proof now saves after the first swing, checks an ignored rapid
press, then holds E to finish the tree without collecting its logs. A fresh
process repeats from that recovery checkpoint with equal snapshots and save
bytes. The existing night, rescue and supply journeys remain. All 17 simulation
tests, determinism lint and game Clippy pass. Clippy also caught an existing
indexed tree-rotation loop; iterating its turns directly keeps its order intact.

The native sweep caught an older capture fixture missed by main's paint-rank
change: `BoxPaintMacTests` still supplied only a style's z-index, but native now
consumes the kernel's explicit rank. Adding that production wire message keeps
the same pixel, animation and restoration assertions; all 38 selected capture,
input and transform tests then pass. No capture implementation change was needed.

Root build, 2,316 enabled tests across 81 binaries (nine ignored), Clippy,
formatting and boot pass. The shared app-tool suite passes 73 tests with two
optional integration skips. Cross-host acceptance follows this checkpoint.

### Accepted verification

The strict collector `artifacts/prove/run-mm6oin` accepts feature commit
`53d2d6fb9` after Linux and web agree in normal, Save and FreshGame modes,
plus native release. All seven runs have zero failures and successful
descendant audits. Linux's three modes take 1.7/14.9/10.4 s; web takes
101.0/110.1/118.6 s including mode builds. The release comparison takes 81.2 s
including its rebuild. A subsequent ordinary Linux proof passes against the
accepted pins. The independent macOS proof takes
96.7 s, including its optional descendant scan timing out; every owned carrier
closes. It matches source inputs, all pins, seven world snapshots and twelve
save files. Both hosts' recovery screenshots were inspected. The new chopping
continuation digest is `5a79c206…`; the existing supply save stays unchanged.

The main integration sweep also passes Garden and Rivals on web and macOS.
Garden agrees on its four snapshots and six saves. Rivals takes 55.6/36.3 s
including rebuilds, agrees on six snapshots and six saves, and its two inspected
drill captures retain the same readable overlay and controls. Both web audits
pass; the native optional audits time out while every owned process closes.
## Main visibility and presentation integration (2026-10-04)

Garden's plot-visibility batch merges main through `8e003c707` as `60e9fde3c`.
Forest's complete proofs pass on web in 151.0 s and macOS in 149.4 s,
including rebuilds beside the other games and shared checks. Inputs, pins,
all seven world observations and twelve saves match across hosts. The axe
recovery screenshots were inspected on both and retain the same forest and
readable recovery countdown. Artifacts: `artifacts/main-8e-{web,macos}/`.
Web's descendant audit passes; macOS's optional scan times out while every
recorded carrier closes. No new Jev run or policy change in this integration.
Garden diary 004 records the merge conflicts, the refreshed game lockfiles,
the passing root/engine/surface checks, and the unavailable Lean oracle.

## Browser tap integration recheck (2026-10-04)

Rivals' navigation batch merges main through `3d76ccdb7` as `c04a3f0f0`.
Forest's complete web proof passes in 86.2 s including builds, retains every
tick/save pin, and passes the descendant process audit. It exercises the merged
driver's press-target verification. The chopping capture was inspected and
retains the visible recovery countdown, supply guidance and forest scene.
Artifact: `artifacts/main-3d-web/`. No new Forest Jev run or policy change here.
Rivals diary 006 records the shared gates and unavailable Lean advisory check.

## Game-shell toolchain integration (2026-10-04)

Merge `4a02f5cf2` brings main through `8c9b476fa` during Rivals' spawn/Mayhem
batch. Forest's complete proof passes on web in 89.2 s and macOS in 104.6 s,
including builds beside the other verification. Inputs, pins, seven world
observations and twelve saves agree. Web's process audit passes; macOS's
optional scan is unavailable while every owned carrier closes. Both chopping
captures were inspected. Artifacts: `artifacts/main-8c-{web,macos}/`. No Forest
gameplay or Jev-policy change here; Rivals diary 006 records the shared checks.

## Main's button and colour update (2026-10-04)

Merge `a0a6d35fa` brings 120 commits through main `c2e909694`. Forest's full
proof passes on web (168.6 s) and macOS (158.4 s), including builds beside
other verification. Inputs, every pin, seven world observations and twelve
saves agree. Both chopping screenshots were inspected; compass, recovery
prompt and survival labels remain readable. Web's descendant audit passes;
macOS's optional scan is unavailable while every owned carrier closes.
Artifacts: `artifacts/main-c2-{web,macos}/`. Forest's lock captures the new
shared colour-parser dependency. Rivals diary 006 records the shared checks;
no Forest logic or Jev policy changed.

## SVG filter extraction integration (2026-10-04)

During Garden's recovery batch, merge `a5f62346d` brings main `00d37ef9f` and
`79e52f11b` refreshes the game locks. Forest's full web/macOS proofs pass in
145.9/141.7 s, with equal inputs, seven world observations and twelve saves;
every pin stays unchanged. Both chopping captures were inspected. Web's
descendant audit passes; native's optional scan is unavailable with every
owned carrier closed. Artifacts: `artifacts/main-00d-{web,macos}/`. Garden
diary 004 records the shared passing gates. No Forest logic or policy changes.

## Knowing when camp is prepared (2026-10-04)

The last hold-to-chop runs rescued both children and finished with 100%/92%
fire, yet neither reached a dawn within 128 choices. Their public objective
said only “Keep the fire burning.” We left the compass and axe batches closed
and addressed that missing survival milestone in the game.

Two HUD lines now budget supplies to the next dawn and name the next task.
The calculation uses the same day/night burn and hunger drain as the simulation,
with ten fuel and ten hunger in reserve. Carried fuel still needs feeding;
carried food counts toward the budget, with an eating reminder at 35 hunger.
Once prepared, campers are told to return home and shelter until a visible
countdown ends. Dawn budgets the next night. The chosen compass stays under
player control. No economy, movement, clock, creature or reward changes.

Jev receives the two new visible labels. Its goal, 128-choice limit, compass
motor, legal actions and ten-second wait are unchanged. One run per host:

| Host | Decisions | Game seconds | Nights | Health / hunger | Fire | Decision p50 / p95 |
|---|---:|---:|---:|---|---:|---|
| Web | 128 (limit) | 150.7 | 1 | 100 / 56 | 97% | 291 / 507 ms |
| macOS | 122 | 255.2 | 2 | 100 / 55 | 50% | 295 / 608 ms |

Both rescue both children. Native waits at camp and completes the goal; web
reaches its first dawn, then spends its remaining decisions on more supplies,
ending 24 m from camp with three logs and two food. The first dawn is visible
at decisions 82/55; the runs make seven/nineteen wait choices. This is evidence
that the public feedback helps this task, not an estimated policy success rate.
No further controller or HUD tuning follows these outcomes; this batch is closed.

Artifacts: `artifacts/jev-dawn-{web,macos}/`. Input/output tokens are
107,995/9,178 and 102,026/8,519. Wall times including builds are 65.3/82.2 s;
both descendant audits pass. Both final screenshots were inspected and show
readable camp plans. The outcome JSON now reads the named Player component
and position after the loop, so the paginated tree-heavy snapshot no longer
hides accepted chops (five/three) or the final location. Those private diagnostic
reads never enter the decision inputs.

Twenty simulation tests pass, including sheltering through dawn at four
day/night boundaries in both collision modes, byte-identical fresh-save
continuation, feeding vs carrying fuel, new-dawn re-budgeting and consuming
reserved food. Game determinism Clippy and formatting pass. The host proof
adds a second rescue, a shelter checkpoint, the first dawn at full health,
and an identical fresh-process continuation.

Before this work, merge `e278ff4b6` brings eight main commits through
`02f53744a`. Both sides of the queue conflict are preserved. Root build,
2,348 enabled tests across 81 binaries (nine ignored), Clippy, formatting
and boot pass. The compiler/semantics changes trigger the advisory difftest
against `469740d5e`; it exits 2 because `lake` is absent. Cross-host strict
acceptance and the other games' integration sweep follow this checkpoint.

### Accepted verification

A second periodic fetch found main `e702a02e3`, a Lean-only change; merge
`02ca9c62f` brings it in after the running integration checks finish. Caps
passes, and the new advisory difftest attempt again reports the missing `lake`.
The compiler and runtime sources remain those of the passing shared checks.

Strict collector `artifacts/prove/run-qRDLfX` accepts this merged revision.
Linux normal/Save/FreshGame take 39.5/17.8/17.8 s; web takes
133.7/136.5/151.6 s; native release takes 68.0 s, including builds. All seven
checks have zero failures and successful descendant audits. The collector also
restores the ordinary web build (25.9 s). Its candidate child summaries remain
UNVERIFIED by design; acceptance comes from the successful collector and its
written pins. A subsequent ordinary Linux proof passes against those pins.

The independent web/macOS proofs (`artifacts/dawn-{web,macos}/`) take
124.3/174.7 s. All inputs, pins, nine world observations and fifteen saves
agree with every strict run. Web's process audit passes; native's optional
scan is unavailable while all recorded carriers close. Both hosts' shelter and
dawn screenshots were inspected. The old tick pins are unchanged; continuation
save digests include the new published HUD labels. The new dawn save digest
is `875e37ab…`, and its world at tick 7378 is `0x3a3a46cdf1380f23`.

Garden's complete proofs pass on web/macOS in 94.6/30.9 s with equal inputs,
pins, six worlds and nine saves. Rivals passes in 81.0/36.4 s with equal
inputs, pins, nine worlds and eleven saves. Their recovery and Mayhem captures
were inspected on both hosts. Both web audits pass; both optional native scans
are unavailable with owned carriers closed. These integration times include
builds and concurrent work, not isolated performance measurements. Artifacts:
Garden `artifacts/main-02f-{web,macos}/`, Rivals
`artifacts/main-e702-{web,macos}/`. No gameplay or Jev changes in those games.

## Main build and browser input integration during Rivals pacing (2026-10-04)

Merges `ec4375505` and `d9a33f1c8` bring main through `a98895a22`.
Forest's full web/macOS proofs pass in 149.7/172.0 s, including builds.
Inputs, pins, nine world observations and fifteen saves agree. Both shelter
screenshots were inspected; camp preparation remains readable. Web's process
audit passes; native's optional scan is unavailable with every owned carrier
closed. Artifacts: `artifacts/main-a988-{web,macos}/`. Rivals diary 006 records
the shared gates and browser-modifier fixture finding. No Forest gameplay or
Jev-policy changes.

## Contract module scoping integration (2026-10-04)

Merge `e19460756` brings main `7cdf080e2` after Rivals' strict collector
finishes. Forest passes web/macOS in 132.6/171.7 s. Both hosts agree on
inputs, all pins, nine worlds and fifteen saves. All worlds and saves also
match the previous integration; only the source-input digest changes. Both
shelter captures were inspected. Web's process audit passes; native's
optional scan is unavailable while all owned carriers close. Artifacts:
`artifacts/main-7cdf-{web,macos}/`. Rivals diary 006 records the passing root
gates and the advisory Lean check unavailable because `lake` is absent.
No gameplay or Jev-policy change; times include builds and concurrent work.

## Bake cache and paint-plane integration (2026-10-04)

During Garden's empty-plot work, merge `f59d587ac` brings main through
`e097e4cae`. Forest's complete web/macOS proofs pass in 129.8/172.6 s.
Both hosts agree on inputs, pins, nine worlds and fifteen saves; worlds and
saves also match the previous accepted integration. Both shelter screenshots
were inspected. Both optional descendant scans are unavailable while every
owned carrier closes. Artifacts: `artifacts/main-e097-{web,macos}/`. Garden
diary 004 records the shared gates. No Forest gameplay or Jev-policy changes;
timings include builds and concurrent verification.

## Shared descendant-audit correction (2026-10-04)

Garden diary 004 records the shared `ps` failure and delayed-event fixes.
Forest passes complete web/macOS proofs in 64.2/164.5 s with successful
descendant audits and no remaining recorded children. Inputs, all pins,
nine world observations and fifteen saves match `main-e097-macos` on both
hosts. Both shelter captures were inspected. Artifacts:
`artifacts/audit-{web,macos}/`. No gameplay or Jev-policy changes; timings
include builds and concurrent verification.

## Browser-cleanup integration during Rivals label work (2026-10-04)

Merge `745a66652` brings main `152cf5b17`. Forest passes complete web/macOS
proofs in 138.7/169.2 s. Inputs, all pins, nine worlds and fifteen saves
agree between hosts; worlds and saves also match the accepted audit baseline.
Only the source-input digest changes. Both descendant audits pass and both
shelter screenshots were inspected. Artifacts: `artifacts/plates-main-{web,macos}/`.
Rivals diary 006 records the shared passing checks. No Forest gameplay or
Jev-policy changes; times include builds and concurrent verification.

## Keep the physics digest with its entry (2026-10-04)

The old diary's per-tick collider rebuild is already fixed. A fresh release
measurement of the real game at 20k trees gives 0.087 ms median day ticks,
0.099 ms at night, 0.97 ms seekable ticks and a 17.73 ms first full world
hash. The remaining cost worth isolating is state capture for the agent,
not the live simulation. This run is observational, not a before/after claim.

The physics hash cached every entry's digest in a second `BTreeMap`, then
looked it up for every collider whenever the world was hashed. The digest
now lives beside its entry, excluded from Data. Sync and body writeback
clear it at the same points as before; deleting an entry deletes its cache.
The EXPHYS v3 encoding and the hash's entity order and algorithm are unchanged.

Six alternating runs of `hash_of_a_static_forest`, comparing baseline
`16baa33a8` and the new release binary, each measure one cold and four warm
hashes at each size and give these medians:

| Trees | Warm hash before | Warm hash after | Snapshot before / after |
| --- | ---: | ---: | ---: |
| 20,000 | 1.810 ms | 1.175 ms | 1.575 / 1.595 ms |
| 100,000 | 10.185 ms | 7.405 ms | 8.545 / 8.505 ms |

The warm hash itself is 35% / 27% cheaper; at 100k, snapshot plus hashing
falls from about 18.7 to 15.9 ms. Cold hashing remains expensive: 68.32 /
65.73 ms at 100k, plus about 50 ms for its first snapshot. The stripped
binary's `sample` output did not identify functions; the claim comes from
the alternating executable measurement, not that profile. Logs:
`/tmp/exact2-{forest-scale-current,physics-hash-current,physics-inline-ab}.log`.

All 47 enabled physics tests pass, including the existing continuation and
pin tests, and Clippy and formatting pass. A new lifecycle test warms the
cache before pose and material edits, adding/moving/removing a body,
reparenting and moving its parent, despawning and reusing the entity slot.
At every stage, the warmed hash equals a fresh decoded save and a clone;
hashing leaves the saved bytes unchanged. No gameplay or Jev-policy change.

Merge `16baa33a8` brings main through `e05dff0c0`, including Apple's idle
timer and deferred-surface work. All five root checks pass in 56.974 s:
build 0.272, tests 54.303 (2,351 passed, 81 binaries, nine ignored), Clippy
0.256, formatting 2.040, caps 0.087 and boot 0.016 s. The game workspace's
lockfile also catches up with main's path dependencies. Logs:
`/tmp/exact2-inline-main-{build,test,clippy,fmt,caps,boot}.log` and
`/tmp/exact2-physics-inline-{test,clippy,fmt}.log`.

Forest's complete web/macOS proofs pass in 144.6/180.5 s including builds.
Both hosts agree on source inputs, all pins, nine world observations and
fifteen saves; every world and save also matches `plates-main-macos` before
the change. Both descendant audits pass with no recorded children left.
Both shelter captures were inspected: supply guidance and countdown remain
readable. Artifacts: `artifacts/inline-{web,macos}/`. The speed measurement
is the isolated alternating diagnostic above, not these proof wall times.

## Contract modules integration from main (2026-10-04)

Merge `887ee2b78` brings main through `a73a3ae4a`, including Contract
module resolution and source-graph watching. Garden diary 004 records the
passing root checks and their 123.3 s budget miss. Forest passes complete
web/macOS proofs in 110.5/170.1 s, including builds. Both hosts agree on
source inputs, all pins, nine world observations and fifteen saves; worlds
and saves also match `inline-macos` before the integration. Both descendant
audits pass with no recorded children left, and both shelter captures were
inspected. Artifacts: `artifacts/modules-{web,macos}/`. No gameplay or
Jev-policy changes.

## Index collider verification by its arena slot (2026-10-04)

The remaining snapshot work still searched a `BTreeSet` twice for every
static collider: once while checking which colliders needed verification,
then again while encoding their holes. Rapier already gives every collider
an arena slot and generation. A derived vector now holds the verified
generation at that slot. Sync and removal clear it at the same points as
before; restore seeds it from the holes it rebuilt. Its length is bounded
by Rapier's arena high-water slot count. The exact rebuild comparison,
collider traversal, saved bytes and hash algorithm are unchanged.

Six alternating runs of the existing release `hash_of_a_static_forest`
measurement compare baseline `850a7afab` with this change:

| Trees | Warm snapshot before / after | Warm hash before / after |
| --- | ---: | ---: |
| 20,000 | 1.505 / 0.695 ms | 1.050 / 1.045 ms |
| 100,000 | 8.115 / 3.845 ms | 6.980 / 6.940 ms |

At 100k, warmed snapshot encoding is 53% cheaper; snapshot plus hash falls
from about 15.1 to 10.8 ms. Cold snapshot medians are 48.68 / 39.86 ms,
and cold hashing 63.11 / 61.13 ms. These are isolated state-capture costs,
not a live FPS claim. Logs: `/tmp/exact2-physics-holes-{baseline,candidate,ab}.log`;
individual samples: `/tmp/exact2-physics-holes-ab.json`. The first candidate
is retained; no snapshot format or gameplay change is needed.

The existing lifecycle test now compares every cached snapshot with a
capture that proves every collider hole afresh, then checks save, clone
and hash agreement. It covers pose/material edits, body changes, parent
movement, despawn and both entity and Rapier collider-slot reuse, including
collider removal/reinsertion and sensor changes. The full physics suite,
Clippy and formatting pass before main integration; host verification and
the integration results follow below.

Feature commit `ce65b166a` is followed by periodic merge `0299f571d`,
bringing main through `f708c99cf` (platform colors and Contract module
review fixes). All 47 enabled physics tests pass, with three ignored scale
measurements; the extended Rapier slot-generation assertions also pass.
All five root checks pass in 181.454 s: build 33.832, tests 122.256
(2,374 passed in 81 binaries, nine ignored), Clippy 23.179, formatting
2.086, caps 0.086 and boot 0.015 s. The test binaries report only 38.9 s
of execution in total. This is another 60 s budget miss, recorded in QUEUE;
no game proofs run concurrently with the gate. Logs:
`/tmp/exact2-holes-main-*.log` and `/tmp/exact2-physics-holes-*.log`.

Forest's complete web/macOS proofs pass in 116.0/186.1 s including builds.
Both hosts agree on source inputs, all pins, nine world observations and
fifteen saves; worlds and saves also match `modules-web` before the change.
Both descendant audits pass with no recorded children left, and both
shelter captures were inspected. Artifacts: `artifacts/holes-{web,macos}/`.
These proof wall times include builds and carrier work; the performance
claim is the isolated alternating measurement above.

## Build a lasting camp windbreak (2026-10-04)

Scrap previously had only one use: burn it for 20 fuel. The new windbreak
spends two logs and one scrap at camp to halve day and night fuel use
permanently. Those materials would otherwise yield 44 immediate fuel, so
the upgrade trades immediate safety for later nights. A metal screen on
two wooden posts appears behind the flame. R and the HUD button perform
the same validated action; the card shows the recipe, carried counts and
readiness. Separate log and scrap compasses keep chosen landmarks across
saves; the log search falls back to standing trees. The existing dawn
budget uses the reduced burn rate while retaining its ten-fuel reserve.

This uses the existing engine's resources, item entities, parented meshes,
surface fields and commands. The recipe consumes exactly its three items,
restacks any extras and records the upgrade once. No separate persistence
code, new agent operation or engine feature is needed. The new tests cover
missing ingredients, distance and death refusals, exact consumption,
duplicate commands, appearance and save continuation, material targeting,
and daylight/dusk/night/dawn budgets in both collision modes. All 23
simulation tests, Clippy and formatting pass. The first Linux candidate
has zero failures in 23.2 s including its build, with a passing descendant
audit (`artifacts/windbreak-linux/`). Eleven world observations and eighteen
saves now include gathering the recipe through the public compasses and
repeating the return/build/burn sequence in a fresh process. Pins remain
unaccepted until strict host/mode agreement.

The new Jev scenario is fixed before its first run: build the windbreak
and reach the first dawn alive within 96 choices. It sees only the visible
HUD and enabled controls. Its compass motor is unchanged; after building,
waiting advances ten seconds, as the earlier survival mode does after
rescue. The earlier two-night survival feedback batch stays closed.

Feature commit `c2f967ebd` is followed by periodic merge `dc943ed53` through
main `39018dfa4`. This main delta fixes captured TypeScript module identity
and qualifies Unix-only compiler fixtures; no game engine or host runtime
code changes. All five root checks pass in 55.429 s: build 2.759, tests
46.511 (2,374 passed in 81 binaries, nine ignored), Clippy 3.991,
formatting 2.068, caps 0.084 and boot 0.015 s. The timing returns under
60 s without removing coverage; QUEUE retains the earlier launch-overhead
question. Logs: `/tmp/exact2-windbreak-root-*.log`.

The complete web/macOS candidates have zero failures in 144.7/181.9 s,
including builds. Both agree on source inputs, all pins, eleven world
observations and eighteen save files. Their worlds and saves also match
the Linux candidate from before the main merge. All descendant audits
pass with no recorded children left. The unbuilt web recipe card and both
hosts' built-camp captures were inspected: recipe counts, disabled state,
the permanent-benefit label and the screen behind the fire are legible.
Artifacts: `artifacts/windbreak-{linux,web,macos}/`.

Jev builds the upgrade on decision 20 on web and 28 on macOS, using the
same public controls as the deterministic drive. Both runs reach the
96-decision cap before dawn: web ends with health 100, hunger 78, fire 85%
and one child following; native ends with health 100, hunger 80, fire 100%
and one child rescued. Neither chooses `wait` even once. The day has only
advanced 49.2/46.3 seconds; repeated switches between Camp and Children
consume decisions without advancing time. Both show fire and food ready,
while the general preparation hint still asks for rescue supplies.

This is evidence that the recipe and separate build action are discoverable,
not that the entire build-and-survive goal succeeded. The visible rescue
hint may compete with the assigned shelter goal, but two stochastic runs
do not isolate that cause or establish a success rate. This bounded batch
is closed without changing the controller or repeating it until it wins.
QUEUE records the unfinished shelter behavior. Both final screenshots were
inspected and both descendant audits pass. Artifacts:
`artifacts/jev-windbreak-{web,macos}/`; wall times 45.4/61.2 s, request
latency medians 326/294 ms and p95 572/528 ms. Web uses 82,513 input and
6,872 output tokens; native uses 83,299 input and 7,075 output tokens.

Strict acceptance succeeds in `artifacts/prove/run-Ij3FWb/`: Linux and
web in normal, paranoid and fresh-game modes, plus release Linux. All
seven agree with the independent macOS candidate on source inputs, every
pin, eleven world observations and eighteen save files. Every descendant
audit passes with no recorded process remaining. The collector accepts
the new `windbreak` save pin and the changed resource bytes at
`dc943ed53`; it also restores the ordinary web build. A normal Linux run
against the accepted pins then passes in 3.883 s with clean process exit
(`artifacts/windbreak-accepted-linux/`). No engine change was needed to
make this upgrade save and restore identically on these hosts.

## Reuse cold-capture comparison buffers (2026-10-04)

Periodic merge `3d8603704` brings main through `b79156175`: Contract
semantics, its generated Lean model and inline parsing. The engine follow-up
targets the remaining first-capture cost from the earlier measurements.
Verifying each static collider used `bincode::serialize` twice: two temporary
vectors and two size-counting passes before writing the bytes. Verification
now writes directly into two buffers reused across that capture. The exact
byte comparison, positive-only cache and EXPHYS v3 representation stay the
same. The existing lifecycle test adds a mesh, sphere and capsule in one
pass to exercise different encoded lengths, alongside its uncached-byte
oracle, edits, entity/handle reuse and save/clone comparisons. All 47 enabled
physics tests, Clippy and formatting pass.

Six alternating release runs per variant compare binaries copied before
and after the change, using the existing `hash_of_a_static_forest` test.
Each run has one cold capture and four warm captures after physics steps.

| Trees | Cold capture before / after | Warm capture before / after |
|---|---:|---:|
| 20,000 | 7.890 / 7.195 ms | 0.685 / 0.680 ms |
| 100,000 | 39.425 / 36.150 ms | 3.735 / 3.795 ms |

At 100k the first capture saves 3.3 ms (8.3%); all six candidate cold
samples are below their paired baseline. Warm capture has no demonstrated
improvement. Cold world hashing is 60.755 / 61.790 ms and warmed hashing
6.910 / 6.970 ms. These are isolated release measurements on this Mac,
not frame-rate or overall game-startup claims.

A second candidate streamed Rapier's collider-hole slots instead of making
a temporary list. It did not earn its added code: cold capture was 37.365 ms,
warm capture 3.930 ms and cold hashing 67.065 ms at 100k. That prototype and
its test were removed; Rapier is unchanged. It needed one compile correction
for the serde trait import; its direct dependency-test command was refused
because Rapier is outside the game workspace. The measured binaries all
compiled and ran. No benchmark or verification script was added. Samples:
`/tmp/exact2-physics-stream-ab.json`; logs:
`/tmp/exact2-physics-{scratch,stream}-*.log`.

The retained change is commit `eaf1825ad`, after main merge `3d8603704`.
All five root checks pass in 116.014 s: build 18.376, tests 83.972
(2,374 passed in 81 binaries, nine ignored), Clippy 11.486, formatting
2.081, caps 0.084 and boot 0.015 s. Test binaries report 39.6 s of execution
in total. No game proof ran concurrently with the gate. This repeats the
local budget miss, recorded in QUEUE, without establishing its cause.
Logs: `/tmp/exact2-scratch-root-*.log`.

Forest passes complete web/macOS proofs in 162.0/185.1 s including builds.
Both hosts agree on source inputs, all pins, eleven world observations and
eighteen save files; worlds and saves also match `windbreak-web` before the
optimization. Both descendant audits pass with no recorded children left.
Both built-camp captures were inspected. Artifacts:
`artifacts/scratch-{web,macos}/`. Existing pins remain unchanged, and no
Jev policy or gameplay change is made in this batch.

## Testing-browser discovery and runner integration (2026-10-04)

Merge `775231855` brings main through `9db90ce57`; Rivals diary 006 records
the testing-browser lookup change (`1a1aab53c`) and its tooling fixtures.
Forest passes complete web/macOS proofs in 131.066/184.733 s including builds,
with `CHROME` unset. The browser selects the installed pinned Chrome for
Testing, and both descendant audits pass with no recorded children left.
Inputs, pins, eleven world observations and eighteen saves agree between
hosts; worlds and saves also match `scratch-web` before the merge. Both
windbreak captures were inspected. Artifacts: `artifacts/browser-{web,macos}/`.
Gameplay and Jev's policy are unchanged.

## Update-store ownership and main integration (2026-10-04)

Merge `f95380e2f` brings main through `dfec331d5`; Rivals diary 006 records
the update-store ownership fix and passing root checks, including their
216.4 s budget miss. Forest passes complete web/macOS proofs in
162.565/196.083 s with `CHROME` unset. Source inputs, pins, eleven final
world observations, published values and eighteen saves agree between
hosts. Ticks, hashes and saves also match `browser-web` before the merge.
The existing `EXACT_PROOF_COMPARE=1` option retains the close-time public
state. Both process audits pass with no recorded children remaining, and
both windbreak captures were inspected. Artifacts:
`artifacts/owner-{web,macos}/`. No gameplay or Jev-policy changes.

## Pending-input inspection and the next main boundary (2026-10-04)

Merge `8a0bd86c9` integrates main through `b83acf98a`. Garden diary 004
records the new read-only `state` summary of queued game input and the root
checks (all pass, 192.375 s; the local timing target remains missed).
Forest passes complete web/macOS proofs in 171.527/191.513 s including builds.
Source inputs, pins, eleven final world observations, published values and
eighteen saves agree across hosts. World observations, pins and saves match
the prior `owner` results.
Both windbreak captures were inspected; both process audits leave no recorded
children. Artifacts: `artifacts/pending-{web,macos}/`. Gameplay, pins and
Jev's closed playtest policy are unchanged.

## Main's input/focus batch (2026-10-04)

Merge `b32b0eec6` brings main through `d2cb661eb`; Rivals diary 006 records
the passing tooling/root/Apple checks and the stale clipping-test fixture.
Forest passes complete web/macOS proofs in 151.199/201.281 s including builds.
Inputs, pins, eleven final world observations, published values and eighteen
saves agree across hosts; worlds and saves match the preceding `pending`
checkpoint. Both windbreak screenshots were inspected. Artifacts:
`artifacts/main-input-{web,macos}/`. The short shared-script test run overlaps
part of the native drive; these are verification wall times, not latency claims.

The screenshots still show “To dawn: fire ready · food ready” beside “Bring the
children home for rescue supplies.” `player::preparation` selects the rescue
hint whenever any child remains unsafe, before considering ready supplies or
the shelter countdown. That explains why the two visible messages coexist;
it does not establish that this caused Jev's earlier no-wait choices. The
closed windbreak pair stays closed. Separating camp readiness from the
rescue guidance remains a concrete next gameplay investigation.

## Camp guidance without a rescue gate (2026-10-04)

The preceding integration supplied a concrete contradiction: the windbreak
capture says fire and food are ready, yet the camp plan asks for rescue
supplies. `preparation` checked whether every child was safe before considering
the actual camp budget. Remove that condition and its boolean parameter. The
camp plan now names missing fuel or food, asks a prepared player to return,
or counts down sheltering to dawn independently of rescue progress. Eating
and death still take priority. The Children compass, rescue count, escort
guidance and supply reward remain available; the plan never changes the
chosen compass or the underlying survival rules.

All 23 simulation tests pass, including the expanded saved-continuation test
at four times in the cycle, with children lost or rescued and both collision
implementations. The fuel and food cases now run with children still lost.
A temporary borrow in the new objective assertion failed compilation on the
first run; keeping its returned value alive fixes it. Log:
`/tmp/exact2-camp-guidance-tests-after.log`.

The existing windbreak proof now follows the visible shelter countdown to
the first dawn and then selects the Children compass, proving that the rescue
remains available. A fresh process repeats that continuation from the saved
camp and compares its bytes. Real-host verification and pin acceptance are
pending at this implementation checkpoint.

Playtest hypothesis: giving the camp plan its own readiness message may help
Jev finish building and sheltering. Run one new 96-decision build-camp trial
per web/macOS host with the existing controller, observation fields, choices,
goal and limit unchanged. This is a new game-guidance change, not a reopening
of the previous closed pair; record failure or success and close this pair
without tuning the policy to its result.

All root checks pass in 99.859 s: build 0.305, tests 97.077, Clippy 0.293,
formatting 2.085, caps 0.085 and boot 0.015 s; 2,436 tests pass with nine
ignored across 81 binaries (40.64 s summed test execution). No game drive
overlaps this run. The 60 s budget is still missed. Forest's determinism
lint, ordinary all-target Clippy and formatting pass. The first all-target
lint invocation also applied simulation-only bans to the unchanged scale
benchmark's `Instant`; the corrected invocation leaves those bans to the
separate logic-library determinism lint. Logs:
`/tmp/exact2-camp-guidance-{checks.json,forest-clippy-after.log}`.

The strict seven-run baseline passes at
`artifacts/prove/run-eQC0wV/`: Linux Off/Save/FreshGame 25.584/23.636/23.591 s,
web Off/Save/FreshGame 150.660/183.470/177.350 s, Linux release 61.478 s.
All gameplay assertions, source inputs and observed pins agree; every process
audit is available and leaves zero recorded children. Existing tick hashes
are unchanged. The six existing save pins are regenerated for this guidance
change, and `windbreak-dawn` adds a continuation after the public countdown
and selecting the unfinished rescue. Twenty saved files are emitted per run.
The normal browser windbreak and dawn captures were inspected: the countdown
is readable, and after dawn the counter says one night survived while the
rescue compass still points to a lost child. Normal web/macOS byte comparison
and the bounded Jev pair follow this baseline.

The normal web/macOS comparison also passes, in 92.092/179.376 s (overlapping
drives, verification duration only), at `artifacts/prove/run-AsMzlY/`.
All twenty saves, twelve final world observations with published values,
source inputs and pins agree. The ten unchanged scenario endpoints retain
their prior world hashes; the built-camp restore now continues to dawn and
is compared with a twelfth fresh session. Both cleanup audits leave zero
recorded children, and the macOS windbreak/dawn captures were inspected.
Summary: `/tmp/exact2-camp-guidance-comparison.json`.

The new Jev pair is closed at 96 decisions on each host. Web builds on
decision 48, macOS on 26. Both finish at Health 100, Fire 100%, zero rescued
children and zero nights survived; hunger is 82/76. Neither chooses `wait`,
even though the shelter countdown is offered on 14/18 decisions. After
building, neither chooses the rescue compass: instead each continues
collecting fuel and feeding the fire. These two stochastic observations do
not establish a success rate or attribute the changed choices to the hint.
They do show that clearer camp readiness did not complete the assigned goal
in this pair. The controller, observation fields, goal and limits were kept
unchanged, and no further attempts were made.

The completed runs take 71.780/78.681 s including builds, have no driver
failures, and their process audits leave no recorded children. Their proof
status is correctly UNVERIFIED: exploratory decisions are not the deterministic
baseline. Both outcome images were inspected. Artifacts:
`artifacts/jev-camp-guidance-{web,macos}/`; compact analysis:
`/tmp/exact2-camp-guidance-jev-analysis.json`. Inspection of the recorded
trees also rules out an initially suspected driver mistake: the recipe
buttons disappear after construction, and no built-camp tree exposes them.

Authoring lesson: a camp plan should report the camp's actual state, with
the rescue compass continuing to report the selected goal. This removes
contradictory advice for players, but it does not make an agent stop spending
choices on surplus supplies. That remaining gameplay/policy interaction
stays open. Existing engine observations were sufficient for the diagnosis:
the old and new rescued-dawn snapshots have identical world hashes and
publications but different journal histories (1,817/1,822 lines). Saved
continuations include that history, so hash agreement alone would have missed
the changed save bytes; the exact cross-host comparison remains necessary.

## Keep spare supplies useful (2026-10-04)

The recorded Jev trials expose inventory loss beyond the failure to wait.
At web decision 82, Fire 99% plus two carried logs and one scrap becomes
Fire 100% with an empty pack. At native decision 89, Fire 98% consumes one
log and two scrap for the same capped result. The code feeds every material
and clamps after each, so a single press can destroy most of the load's value.
These are observations from the completed pair, not a new attempt. A periodic
fetch at this clean checkpoint finds no newer main commits to integrate.

Feeding now scans the pack in order and burns only whole items whose fuel
fits below 100, preserving everything else in order. A later smaller log
can fit even when an earlier scrap does not. When no carried fuel fits,
the prompt explains the capacity limit; a nearby supply or child remains
interactable. Refused feeding changes neither inventory nor journal.

G and the visible **Drop last supply** button leave the last packed item
on the ground in front of the player, avoiding trunks. Its entity, material
and kind survive; the existing Parent removal and teleport operations put
it back into the world, and the selected material compass refreshes. The
item can be collected again after saving or leaving camp. Holding G drops
only once. Empty and dead-player drops do nothing. This gives surplus fuel
and meals a place to wait without wasting them or filling every pack slot.
The exploratory driver offers the new enabled button; the previous closed
Jev pairs remain closed and are not rerun with that extra choice.

All 26 simulation tests pass in 2.06 s. New cases cover exact and near-full
fuel boundaries, mixed packs, identity/material/parent restoration, both
collision modes, held keys, the message/button path, and refused drops.
All-target Clippy passes after replacing a test's redundant Material clone
with a copy; the determinism lint and formatting also pass. The native
preflight (`artifacts/supplies-preflight/`) takes 8.098 s with no failed
assertions, proving a normal-input fill/drop/leave/retrieve/refuel sequence
and an identical fresh-process continuation. Its UNVERIFIED status correctly
means collection, with strict baseline acceptance and web/macOS checks pending.

All root checks pass in 44.839 s, within the 60 s budget for this warm run:
build 0.241, tests 42.198, Clippy 0.232, formatting 2.069, caps 0.085 and
boot 0.015 s. The 81 test binaries pass 2,436 tests with nine ignored and
report 40.64 s execution. No game drive overlaps those checks. This is a
passing observation, not evidence that prior process-launch variance is
fixed. The final Forest test source also passes all 26 cases after the
Material-copy cleanup. Logs: `/tmp/exact2-forest-supplies-checks.json` and
`/tmp/exact2-forest-supplies-logic-tests.log`.

The strict seven-run baseline is accepted at
`artifacts/prove/run-pc3WeM/`: Linux Off/Save/FreshGame takes
4.784/26.127/26.171 s, web Off/Save/FreshGame 158.317/194.231/190.863 s,
and Linux release 30.532 s. All source-input and pin comparisons agree,
all assertions pass, and every cleanup audit leaves zero recorded children.
The original three tick pins stay unchanged; save pins reflect the new
input binding/HUD and a new `cached-supply` continuation. Each run emits
23 save files across 14 final scenario observations. The normal browser's
spare-fuel and cached-supply images were inspected: the refusal is readable,
the pack retains its log, and dropping places that log visibly on the ground
with the matching pickup prompt. The exploratory observation also includes
the visible drop label, so the last item's kind is available to Jev.

The normal web/macOS sweep (`artifacts/prove/run-cboxBu/`) passes in
116.431/192.285 s, with identical source inputs, pins, all 14 final world
observations and all 23 save files. Both cleanup audits are available and
leave zero recorded children. The original three tick pins and the previous
12 world hashes remain unchanged. These overlapping runs measure verification
work, not interactive latency; no new Jev trial was run for this change.

Image review initially appeared to show missing macOS HUD text in
`cached-supply.png` and the short `artifacts/supplies-redraw/` reproduction.
That host-defect claim was wrong: the image-viewing output showed missing
text that is present in the saved PNG. Decoding its bottom-left panel and
comparing the bright text mask with a new clean capture gives 11,730 versus
11,733 lit pixels, with only three differing pixels over the whole panel.
The decoded original's cropped image contains all six lines and the drop
button. Six more pickup/drop cycles (`artifacts/hud-pairs-before/`) each
capture the agent image, window-server image and agent image again; all have
complete text masks. A focused host test of partial text invalidation and
repeated agent capture passes unchanged and was removed because it does not
reproduce a defect. No host code was changed. The earlier diagnosis also
mistook Forest's ordinary native overlay for a GPU-uploaded HUD texture;
the game's renderer uses `ChildrenMode::Overlay` without placed children.
Temporary pixel probes: `/tmp/exact2-hud-{pixels,mask,crop}.swift`.

Periodic main integration: `51fa87a60` merges four new commits through
`dfdbb1058` (documentation and an iOS grouped-list regression test), with
no conflicts or runtime changes. All five root checks pass in 47.694 s:
2,436 tests pass across 81 binaries, nine ignored, zero failures. Their
reported execution totals 40.25 s; no game proof ran concurrently.
Logs: `/tmp/exact2-supplies-main-checks.json` and matching check logs.

**Surplus-fuel Jev follow-up (2026-10-04).** A new pair keeps the existing
build-and-survive goal, choices and 96-decision limit, with the shipped drop
action and its visible label. Both build the windbreak (web decision 48,
macOS 69), then hit the cap with zero waits, no dawn and Health 100. Web
finishes with Fire 99%, Hunger 84, one log and two scrap; macOS with Fire
88%, Hunger 87 and one scrap. They select drop four/two times and observe
the no-room prompt eight/three times. The transcript verifies the intended
trade: web decision 69 feeds two logs at Fire 65%, leaving its scrap at 89%;
macOS decision 91 feeds two logs at 64%, leaving its scrap at 88%.

Each drop is followed immediately by picking the item up again. Web repeats
that loop three times around the first retained scrap and once around a log;
macOS does it twice. The action preserves supplies, but these observations
show no improvement in the complete build-and-survive task. They do not
establish that human players share the model's behavior. This pair is closed;
do not change the controller and rerun it to a win. Artifacts:
`artifacts/jev-surplus-fuel-{web,macos}/`; durations 74.473/70.344 s, no driver
failures, both cleanup audits available with zero remaining children.
Both are exploratory `UNVERIFIED` outcomes, not deterministic proof runs.

**Native canvas bitmap orientation (2026-10-04).** The false missing-text
report above led to a separate real defect in the canvas texture path, already
queued from Caltrain: a copied layer tree loses the flipped coordinate space
inherited from AppKit ancestors outside the captured subtree. A Contract label
captured by `Capture.bitmap` has only 34% glyph-mask overlap with AppKit's
direct capture and is vertically inverted in the decoded image. Forest's
ordinary overlay does not use this texture path.

`PaintCapture` now retains that inherited bitmap orientation in an image-only
sublayer; the authored child frames, transforms, masks and paint order stay on
their original copies. It does not redraw or allocate a second bitmap for each
label. The first attempted fix, preserving ancestor geometry around the entire
copy, moved the label and was removed. The second fixes the pixels without
moving geometry. The existing agent screenshot path remains separate.

The regression compares a whole canvas overlay and individual Contract/native
text children with AppKit's direct drawing, allowing one physical pixel at
antialiased glyph edges. Against the original renderer it rejects 32–37% of
the ink; with the fix less than 1% is unmatched. It also compares a native
image and an explicit image layer's colored top, bottom and border. All five
capture tests pass, including the existing current-batch rank, animation,
opacity and mask cases. The full macOS suite executes 684 tests with two
skipped and zero failures in 41.725 s; the added decorated-image assertions
then pass in the focused run. All root checks pass in 43.316 s, with 2,436
passing tests and nine ignored across 81 binaries. No game drive overlaps
those checks. Logs: `/tmp/exact2-canvas-orientation-*.log`. App drives and
cross-host game save comparisons follow this source checkpoint.

The Caltrain app rebuilds and its macOS smoke passes all three authored tests
in 3.9 s. The search drive's title card, train, `Palo` field and Back control
are upright in `/tmp/exact2-canvas-orientation-caltrain.png`; the old canvas
orientation queue entry is removed. Garden (`run-2BzIul`), Forest
(`run-XH68Ib`) and Rivals (`run-4S7b8n`) each pass on web/macOS with equal
source inputs, pins, 37 final world observations and all 55 save files.
Their paired durations are 56.344/43.109, 111.738/192.124 and
71.411/57.250 s respectively. All cleanup audits are available and empty.
The fed harvest, cached supply and bandaged drill images were inspected.

The placement fixture gives a narrower mixed result (`run-4eGe0r`): projected
geometry, hitting its placed controls, visibility, named reordering and save
restoration pass, and the 3D glyphs are now upright. The complete proof fails:
both hosts produce continuation digest `651e8484…` rather than pin `84506a97…`;
macOS also captures Pull where its ordinary HUD should cover it, and the
reorder changes its full save from 2,227 to 2,310 bytes while the world snapshot
stays equal. Running the original `PaintCapture.swift` reproduces those same
three native failures and every saved byte (`artifacts/canvas-orientation-original/`).
That capture's 3D labels are inverted; the fixed capture's are upright. The
original direct proof records two final hashes, both matching the fixed run;
the matrix run also records publications and its third session. No broader
before/after snapshot equality is claimed. All fixture cleanup audits are
available and empty. The unrelated failures are queued and its pins remain
unchanged. Comparison: `/tmp/exact2-canvas-orientation-comparison.json`.

Periodic main integration: `a1cf446e7` merges nine commits through
`e1b1ca868`, covering iOS scroll corrections and the media-session admission,
without conflicts. The root checks pass in 71.918 s, exceeding the 60 s
budget: 2,436 tests pass across 81 binaries, nine ignored, zero failures.
The test command takes 69.000 s while its reported execution totals 40.55 s;
no game proof overlaps it. This remains part of the queued root timing
variance, not evidence that the budget is repaired. The merged macOS suite
executes 684 tests with two skipped and zero failures in 41.224 s, including
the final text/image orientation regression. Logs:
`/tmp/exact2-canvas-main-{checks.json,apple-tests.log}`.

Each game then rebuilds and runs on the merged macOS host. Garden plants,
harvests and opens its one-fruit backpack; Forest selects the food compass;
Rivals fires and spends ammunition. All three advance the clock, move, pause
and resume, with zero assertion failures and available, empty cleanup audits.
Their build-and-drive durations are 14.862/14.470/14.469 s. The inspected
images are each game's `artifacts/canvas-main-macos/main-check.png`. These
short drives are deliberately `UNVERIFIED` partial runs; the complete paired
proofs above precede the merge. The placement fixture is also rebuilt with
the fixed renderer restored after its original-renderer comparison; its
queued proof failures remain open.
