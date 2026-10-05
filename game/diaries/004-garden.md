# Grow a Garden — exact2, aimed at simulation scale, UI/economy and persistence

Brief (Charlie, via the lead agent, 2026-10-02, lane `roblox/garden`): clone the
Roblox game **Grow a Garden** to find where the engine breaks, not to polish a
demo. Two sibling lanes clone other Roblox games aimed at other axes; this one is
**simulation scale + UI/economy + persistence**: fill the garden and ramp the
entity count, put a real shop/backpack/sell/timers HUD in Contract, save and
restore a large garden with offline growth, simulate hours with the seekable
clock. Change the engine only when truly blocked. Builder: Claude (Opus 5.5),
one agent, worktree `wt/garden` on `roblox/garden` from `origin/main` @ e50e3f74.

The game is `game/games/garden/`; how to play and how it is built is in its
README. Run it with `bun game/dev.mjs garden` from the worktree.

## Time log (UTC, 2026-10-03)

Reading `CLAUDE.md`, the rules, `game/README.md`, the engine README, the
diaries README, the tennis diary and the starter began about 04:05 and is not in
the totals.

- **04:10:52** `bun game/new.mjs garden`. The type is still `SmallGame` (the
  tennis diary's first friction, unchanged) — renamed with `sed`.
- **04:11** `shells.mjs --test` refused: *the SDK lock is stale on main*
  (`cssparser 0.37.0`, `dtoa`, `dtoa-short` resolve outside
  `game/app/shells.lock`). `--update-lock` captured a game-owned `Cargo.lock`;
  the starter's tests then passed in 78 s. A cold Linux proof of the untouched
  starter ran in the background: **440 s**, 0 failures.
- **04:13–04:22** Wrote the game: catalogue and values, the schedule and
  growth, the farm and its commands, the shop, the HUD records (~1,250 lines
  before `rustfmt`). First compile 18 s with no error; the determinism lints
  passed first time.
- **04:22–04:24** Tests, two rounds: `Game::register` takes
  `&BTreeMap<&str, Value>`, not the game's `Args` (my guess); restore refused
  `unregistered component Plant`, then `Fruit`, then **`Parent`** — the engine's
  own component must be registered by the game that first spawns one mid-game.
  Two tests of mine assumed whole-millisecond ticks at 30 Hz. 11 green.
- **04:24:54** First Linux proof of the garden: **10 s including the build** (GPU
  1.2 s, host 1.0 s incremental), 2 failures. The prompt is empty at tick 0 (my
  proof); and *continuation saves differ in bytes* although world hashes match:
  the save carries the bound live arguments, and the HUD's command channel
  (`cmd`, `cmd_id`) is HUD state a fresh process does not have (friction 4).
- **04:25–04:26** Three rounds on the channel: the world acknowledges with
  `w.emit("done <id>")`, Contract's `message=` returns the HUD to id 0. Round 2
  lost every first command after a sale: the acknowledgement arrives *after*
  the next press, so an id that restarted at 1 matched the stale ack. Round 3:
  ids keep counting (`sent`), only a matching ack resets. **0 failures**.
  Commit `68280d3d`.
- **04:27–04:31** First web build and screenshot: 272 s cold, `UNVERIFIED`
  (pixels only). The plants were pale: `Material::rgb` is linear and I wrote
  sRGB-looking values; the garden lay behind the following camera.
- **04:31–04:50** The hostless ramp in release (`tests/scale.rs`). The first
  version timed one `run(33 ms)` per tick and measured the agent's observation,
  not the tick (friction 9); rewritten around `Sim::advance(.., Clock::Live)`
  at 60 Hz. **Limit 1**: an hour's seek at 50,000 plants took **483 s**; the
  per-tick cost was the transform hierarchy, not the game. Fruit unparented:
  **0.58 s**. Commit `3581fc2f`.
- **04:53** `proof.mjs linux --scale`: **Limit 2** — after "harvest all" at
  11,100 plants the HUD resource failed, `record exceeds 64 KiB`, and the agent's
  `clock` operation itself errored. The backpack now publishes pages of 200.
- **04:55–05:10** `proof.mjs web --scale`: **Limit 3** — `RuntimeError:
  unreachable` in `gpu_bind_at`, no message. Bisected with the agent: any tick
  that spawns ~1,000 entities inside a multi-tick advance. The web GPU module
  has no panic message (panic = abort, no hook), so the macOS host was built
  (401 s cold) to read it: `render/src/renderer.rs:324: slot has not been
  initialized`. The agent shows a native app's last 20 lines; with
  `RUST_BACKTRACE=1` the backtrace pushes the message out, `=0` shows it.
  Regression test first (failed), then an 11-line fix in `write_transforms`:
  **the only engine change**, its own commit `4386f390`. 135 render lib tests
  and the render integration tests green; clippy and fmt clean.
- **05:13** Paging and a HUD layout fix (the agent's default web viewport is
  420 × 900; the wrapped top bar sat under absolutely-placed panels). Commit
  `c99b05cd`. `web --scale --huge` then carried **21,100 plants / ~90,000
  entities** through fill, an hour's seek, harvest-all of 69,327 fruit, sell,
  fresh-process restore of 12.2 MB and an 8-hour offline restore.
- **05:13–05:18** No agent route measures live frames (`perf` counters are 0
  under the agent clock, friction 11), so `tests/render.rs` drives
  `WorldSurface<Garden>` through `exact_gpu::fixture::render` offscreen on this
  Mac's GPU. Commit `589fd79c`.
- **05:19–05:28** First baseline (`bun game/prove.mjs garden`): Linux rows
  passed; every web row failed 3 checks — web reports no `accessibleName` for
  a labelled `text`, Linux does (friction 13). The proof refused to write pins
  (correctly). The macOS proof had shown the same 3 failures at 05:05.
- **05:30–05:37** Second baseline, after reading the label from
  `props.accessibilityLabel` and giving the purse `role="status"`: Linux and web
  agree in Off, Save and FreshGame, plus the Linux release run; **pins written**
  in 403 s (tick 0 `0xe54bd41d2613ca74`, tick 5136 `0x51a6e97baaf63658`,
  continuation save `b6c1f1ad…`). Commit `1151be39`.
- **05:37** `bun game/prove.mjs garden`: **PROOF PASS**. `--hosts linux,web
  --compare-saves`: **PROOF PASS** in 27 s, world hashes equal and save bytes
  identical across arm64 macOS (Linux host) and Chrome wasm.
- **05:38** Screenshots refreshed: `artifacts/web/game.png` (100 plants, the
  shop), `artifacts/web/overview.png` (10,100 plants, 12,339 fruit, in rain).
  At that distance the fixed fog density washes the garden to a grey plane;
  not tuned.

**Totals.** Brief to a playable game with green tests: 04:10 → 04:24 (14 min).
To the first 0-failure real-host proof: 04:26 (16 min). To pinned, verified
PASS on Linux and web: 05:37 (1 h 26 min), including the scale work, the
renderer diagnosis and fix, and one baseline lost to friction 13. Build/run
iterations: 9 Linux proof rounds, 6 web rounds (2 baselines), 1 macOS proof,
4 hostless measurement runs, 2 offscreen render runs.

## Proof status

- Linux (GPU-less host): **PASS**, pinned. 34 checks: title and focus, plant →
  grow (stage 2 at half time, countdown) → harvest → backpack → sell → buy,
  disabled unaffordable seeds, the two-press observation, fill 100, a mid-game
  pin, save → fresh-process restore → identical continuation and
  byte-identical saves, the same save an hour later grown offline, harvest all,
  the virtualized backpack.
- Web (Chrome): **PASS**, pinned; hashes and save bytes equal to Linux.
- macOS: the same proof ran at 05:05 before friction 13's fix — every
  gameplay, restore and offline check passed, 3 failed on the text-name gap.
  Not rerun after the fix.
- `--scale` on Linux and web: completes at 21,100 plants (`--huge`).

## Line counts (physical lines, `rustfmt`ed)

| file | lines |
|---|---:|
| `logic/src/garden.rs` — clock, schedule heap, plants, fruit, weather, smooth growth | 527 |
| `logic/src/crops.rs` — 14 crops (rustfmt spreads each to 18 lines), values, formats, 3 tests | 487 |
| `logic/src/farm.rs` — tiles, purse, backpack, commands, offline catch-up, prompt | 421 |
| `logic/src/lib.rs` — arguments, setup, tick, command channel | 205 |
| `logic/src/hud.rs` — three published records, backpack pages | 161 |
| `logic/src/shop.rs` — stock and restock | 61 |
| **game logic** | **1,862** |
| `app.contract` — title, top bar, shop, backpack, tools, prompts, touch | 217 |
| `logic/tests/sim.rs` — 11 hostless game tests | 305 |
| `logic/tests/scale.rs`, `render.rs` — ignored measurements | 279 |
| `proof.mjs` — real-host proof and `--scale` | 221 |
| `logic/Cargo.toml` (renderer as a dev-dependency), `README.md` | 78 |

The engine change: `game/render/src/renderer.rs` +11 lines of fix, +27 of test.

## Limits found

Measured on an arm64 Mac shared with other lanes (load average 8–27 during
these runs), release builds unless noted. "Plants" is the stress fill; entities
are plants + fruit + 5 scene entities.

### 1. Parented entities cost a reap and a propagate every tick, changed or not

`Sim` calls `reap_orphans` (a query over every `Parent`) and `propagate` (every
parented entity) after each tick. With each fruit a child of its plant, a mature
garden in which nothing moves still paid per tick:

| plants | entities | live 60 Hz frame, mature (mean / max ms) | +1 h seek (108,000 ticks) | ms per tick in the seek |
|---:|---:|---:|---:|---:|
| 2,000 | 8,572 | 0.048 / 0.24 | 10.7 s | 0.099 |
| 10,000 | 42,862 | 0.244 / 1.02 | 86.0 s | 0.797 |
| 50,000 | 214,288 | 1.261 / 4.59 | **483.0 s** | 4.472 |

Fruit placed in world space instead (a mature plant never moves):

| plants | entities | live frame, mature | +1 h seek | ms per tick |
|---:|---:|---:|---:|---:|
| 2,000 | 8,572 | 0.000 / 0.01 | 0.10 s | 0.0009 |
| 10,000 | 42,862 | 0.000 / 0.01 | 0.18 s | 0.0016 |
| 50,000 | 214,288 | 0.000 / 0.01 | **0.58 s** | 0.0054 |

830× on the hour's seek at 50,000 plants; the hostless test suite went from
3.05 s to 0.38 s. Not changed in the engine: the fix is dirty tracking in the
hierarchy (skip the reap when nothing despawned, propagate only changed
subtrees), which is not a small change. The workaround is in `garden::bear`.

### 2. A surface's whole public record is capped at 64 KiB

`runner/src/surface_record.rs` refuses a record over 65,536 bytes, for all
the fields the world publishes together. Past it the HUD resource goes
`Unavailable("record exceeds 64 KiB")` and **the agent's `clock` operation
throws**, so a proof cannot even continue to observe the failure. With this
row shape (`id`, `label`, `weight`, `value`, `mutated`) a page of 200 fruit is
18.5 KB, so roughly 700 backpack rows fit beside the rest of the HUD. The
virtualized `list` itself is not the limit (Carousel shows 25,000 cards from a
data source); the world → Contract channel is. The backpack publishes pages
(`page next`/`page prev`). Grow a Garden's own backpack holds 200, so the game
lives inside it; a sell screen or a garden-wide list would not.

The host also re-serializes the *whole* record (`Sim::take_published` →
`published_json`) whenever any field changes; the status line changes every
garden second, so the 200-row page is re-encoded once a second.

### 3. Entities spawned inside a multi-tick advance crashed the renderer (fixed)

`World::fresh` is cleared at the start of each tick, and the presentation feeds
once per frame, so an entity spawned in an earlier tick of a multi-tick advance
never had its previous transform written. Past the history buffer's high-water
mark `set_batches` asserted `slot has not been initialized`: **web aborted
(`RuntimeError: unreachable`, no message) and macOS aborted (SIGABRT)** once
about 1,000 entities spawned in such a tick — every `fill 1000`, and every seek
in which a few hundred plants matured at once. 100 at a time never crossed the
mark. Any live game whose frame takes two ticks (a 30 Hz tick on a hitch) can
hit it. **Changed** (`4386f390`): `write_transforms` gives slots past the
history mark their current pose as history, as a fresh entity gets. Regression
test `slots_first_written_after_a_swap_take_their_current_pose_as_history`.
Below the mark, a recycled slot spawned that way still interpolates from its
previous occupant for one frame (not fixed; visual only).

### 4. The only input from Contract into a world is a live argument

There is no message *into* the world (tennis friction 4, still true). Commands
(buy, sell, harvest, expand, fill, away) are one `#[live] cmd: String` and a
`cmd_id: u32`:

- **Two presses between two ticks keep only the second** (measured: hand ×1,
  two Buy presses before a tick → ×2). Contract cannot queue into a string
  argument without unbounded growth.
- Live arguments are saved, so HUD-transient state is in every save; a restore
  into a fresh HUD (id 0) cannot byte-match. The fix is a protocol: the world
  emits `done <id>`, `message=` resets the HUD to rest — and the ack arrives
  after the next press, so ids must keep counting. 25 lines of Rust and
  Contract to say "do this once".

### 5. Seekable observation costs O(world) per advance

A seekable advance observes its final tick pair (rest detection), comparing
saved state. One observed single tick:

| plants | entities | one observed tick | with a 164,284-fruit backpack resource |
|---:|---:|---:|---:|
| 2,000 | 8,572 | 3.4 ms | — |
| 10,000 | 42,862 | 17.9 ms | — |
| 50,000 | 214,288 | **91.7 ms** | **408 ms** |

Live frames at the same sizes cost 0.000–0.059 ms. So `clock +34` steps through
a large garden are slow and long steps are cheap; a proof at scale should take
few, long steps. Rust tests that call `run(dt)` per tick measure this, not the
tick.

### 6. Saves are ~270 bytes per entity

| plants | entities | save | save ms | restore ms | hash ms |
|---:|---:|---:|---:|---:|---:|
| 2,000 | 8,572 | 2.6 MB | 7.7 | 14.6 | 6.2 |
| 10,000 | 42,862 | 11.8 MB | 40.6 | 69.1 | 29.6 |
| 50,000 | 214,288 | **58.1 MB** | 190.6 | 355.5 | 151.7 |

Each entity saves its `Transform`, `Mesh`, `Material` and the game component;
the mesh and material are presentation that a plant's kind already determines.
The 16 MiB surface-payload bound (`Request::capture_surface`) is reached at
about 60,000 entities (~14,000 plants), so an app-driven Save/Continue of a big
garden would refuse well before the 256 MiB agent bound. On real hosts
(`proof.mjs --scale`, gpu-dev profile): Linux restored a 12.2 MB save in a fresh
process in 303 ms, web in **2,929 ms**; web saving it took 1,263 ms.

### 7. 512 entities is the agent's whole-world read

`state world:*` and `snapshot()` refuse past 512 entities, so the proof's
pinned mid-game snapshot is a 100-plant garden (~300 entities). Scale runs read
the HUD instead.

### What did not break

- **The renderer.** Offscreen on this Mac's GPU at 1280 × 720, overview camera,
  all of it on screen, readback included in the wall time:

  | plants | entities drawn | wall ms/frame mean / p95 / max | feed ms | encode ms | draws | triangles |
  |---:|---:|---:|---:|---:|---:|---:|
  | 2,000 | 4,433 | 1.7 / 1.9 / 2.2 | 0.007 | 0.067 | 28 | 1.5 M |
  | 10,000 | 22,154 | 3.6 / 6.6 / 7.0 | 0.010 | 0.075 | 28 | 7.4 M |
  | 50,000 | 110,726 | **6.5 / 8.0 / 8.9** | 0.018 | 0.080 | 28 | 36.9 M |

  Instancing holds the draw count at 28. Smooth growth (every growing plant
  rescaled every tick, `smooth=true`) at 50,000: feed 0.016 → 0.42 ms, frame
  4.6 / 8.6 / 10.1 ms. The web's live frame time was not measured (friction 11).
- **The event-driven garden.** Growth, ripening, restock and weather are one
  saved min-heap, so a tick costs what is due. Linux host, gpu-dev: an hour's
  seek at 21,100 plants (90,000 entities) in 337 ms, web 759 ms; restoring that
  garden 8 hours later ran 103,195 events in 436 ms (web 3,119 ms including the
  restore). Hostless, an 8-hour catch-up at 50,000 plants: 150,242 events in
  358 ms. Harvest all of 164,284 fruit: 597 ms; sell all: 399 ms.
- **Determinism.** Same seed, same mutations; another seed, other mutations
  (`same_seed_same_mutations_other_seed_other_mutations`). The strongest check:
  20 minutes away and 20 minutes played produce the identical garden, weather
  count and shop stock (`away_is_the_same_as_playing_through`). A fresh-process
  restore continues to a byte-identical save on Linux, web and macOS.
- **Offline growth.** `exactTime().epochAtZero` as a live argument and the
  agent's `--epoch` made "restore an hour later" a one-line proof step.

## Friction log

1. `bun game/new.mjs` still names the type `SmallGame`. 1 min.
2. **The SDK lock is stale on main** (`cssparser`, `dtoa`, `dtoa-short`): every
   new game needs `--update-lock` before its first test, and keeps a lock it
   did not need. `cargo test -p exact-game-render` from `game/` rewrites
   `game/Cargo.lock` the same way (reverted, not committed). Left for the SDK.
3. `Game::register`'s signature is not the `Args` type; and `Parent`, the
   engine's own component, must be registered by a game that spawns one
   mid-game, or restore refuses. One round each.
4. The command channel (limit 4). Three rounds to make it restore-stable.
5. Contract has no modulo or zero-padding, so countdowns (`4:05`) and compact
   sheckles (`1.2K`) are strings formatted in Rust. Cheap, but it puts display
   in the world.
6. Contract checks surface arguments against the *last* GPU build's
   `.shells/surfaces.json`, so the first compile after adding a `#[live]`
   argument fails (`unknown surface argument cmd`) until a bake. Documented.
7. `Material` colours are linear (documented); authored sRGB-looking values
   wash out. A `paint()` helper squares them.
8. `World::children` scans every entity, so each plant keeps its own fruit
   slots (`Plant::fruits`). Documented ("children(e) scans").
9. `Sim::run(dt)` per tick in a timing loop measures observation (limit 5).
   Measuring the tick needs `advance(.., Clock::Live)` and a hand-kept clock.
10. **A web GPU panic has no message**: `RuntimeError: unreachable` with wasm
    offsets. Finding the cause took a 401 s macOS build; and the agent's
    "last 20 lines" of a native crash are all backtrace unless
    `RUST_BACKTRACE=0`.
11. **No live frame numbers through the agent**: `state world` has a `perf`
    block, but under the agent clock its rings are empty; the bench's `feel`
    runner is Beacons-only. Frame times came from a game-owned offscreen test
    over `WorldSurface` and `exact_gpu::fixture` (which needed a
    `logic/Cargo.toml` and a lock update).
12. The agent's default web viewport is 420 × 900 and the proof's 1280 × 720:
    a HUD tested only in the proof's size broke under `scripts/agent.mjs`
    (panels absolutely placed under a top bar that wraps). One round.
13. **Web and macOS report no `accessibleName` for a labelled `text`; Linux
    does.** Buttons are named on all three. Three failures per web/macOS proof
    until the proof read `props.accessibilityLabel`; it cost one whole
    first-baseline attempt (9 min 22 s) because the first baseline needs every
    mode on both hosts.
14. The gpu-dev profile builds game logic at opt-level 1 (dependencies at 3),
    so the Linux dev host runs the game slower than release; the scale numbers
    above say which they are.
15. Machine: cold builds shared the machine with two other lanes — Linux starter
    440 s, web 272 s, macOS 401 s. Incremental: Linux proof 3–10 s total, web
    30–65 s. `timeout` is not on macOS; the shell tool refuses `sleep`.

## What was pleasant

- **The Linux loop.** After the cold build, an edit to the game's Rust or
  Contract was a passing 34-check real-host proof (fresh-process restore
  included) in 4–10 seconds.
- **Saves and hashes were free**: a binary heap of tagged enum entries, a tile
  map of `Option<Entity>`, the backpack — all `Data` with nothing written for
  them; restore → continue matched bytes first time once the arguments matched.
- **Determinism held with no effort**, including the strong form (offline ≡
  online). The lints passed first compile.
- **The renderer carried 110,000 instanced entities under 9 ms**, and smooth
  growth of 50,000 under 11 ms, with no game-side batching.
- **Contract made the economy HUD cheap**: 217 lines for the title, top bar,
  seed shop with rarity colours and disabled buttons, virtualized backpack with
  pages, tools, prompts and touch controls, all with accessible names on the
  controls.

## After the fix lane (2026-10-03, merged `roblox/integrate` @ 347534e2)

The lead merged the engine fix lanes; this game was adapted to them and
re-measured. Release builds, hostless unless noted; the machine's load was 13–37
during these runs, so differences under ~30% are noise.

| item | before | after |
|---|---|---|
| HUD → world commands | live `cmd`/`cmd_id` + `done <id>` ack protocol; 2 presses between ticks → 1 applied | `postMessage("world", …)` / `input.messages()`; 2 presses → 2 applied (proof checks ×3). Game diff −107/+51 lines |
| +1 h seek, 50,000 plants, fruit **parented** | 483 s | **0.54 s** |
| +1 h seek, 50,000 plants, fruit in world space | 0.58 s | 0.34 s |
| mature live frame, 214,288 entities, parented | 1.261 ms | 0.001 ms |
| renderer feed per frame, 50,000 plants (offscreen) | — | parented **1.86 ms**, max frame 16.7 ms; world space 0.019 ms, max 7.5 ms |
| save, 214,288 entities (EXGAME v4) | 58.1 MB, save 191 ms, restore 356 ms, hash 152 ms | **6.4 MB**, 166 ms, 139 ms, **1.7 ms** |
| one observed tick, 214,288 entities | 91.7 ms | 40.7 ms |
| observed tick with a 164,284-fruit backpack | 408 ms | 17 ms (paged) |
| backpack field, 164,284 fruit, unpaged | refused (64 KiB cap) | 13.6 MB, harvest-one 311 ms (cap now 16 MiB) |
| backpack field, paged 200 | 18.5 KB | 18.5 KB, harvest-one 18 ms |
| harvest all / sell all / 8 h away, 50,000 plants | 597 / 399 / 358 ms | 250 / 13 / 135 ms |
| fresh-process restore of the 21,100-plant save, Linux / web | 12.2 MB: 303 / 2,929 ms | 2.2 MB: 225 / 1,876 ms |

Choices:

- **Fruit stays in world space.** The hierarchy's dirty tracking fixed the
  simulation (483 s → 0.54 s), but the renderer's feed still poses every
  parented entity on every tick (`world.rs`, the `(&Parent, &Transform)`
  overrides): 1.86 ms a frame at 50,000 plants against 0.019 ms unparented.
  That is the remaining per-tick parenting cost.
- **The backpack keeps its 200-row pages.** The 16 MiB cap would hold a 164,000
  fruit backpack (13.6 MB, ~190,000 rows at most), but each harvest rebuilds the
  field (311 ms against 18 ms), and Grow a Garden's own backpack is 200.
- Removed: the `Parent` registration, the ack protocol and its `cmd`/`cmd_id`
  arguments, the proof's label workaround (`props.accessibilityLabel`) and the
  purse's `role="status"` — `accessibleName` is checked again and passes on web.
  The proof's mid-game pin now reads every page of a 1,000-plant garden (>2,000
  entities) at one tick with `snapshot({all:true})`, instead of a 100-plant one.

**New limit — a host seek now pays for every intermediate HUD publication.**
On real hosts (gpu-dev Linux; Chrome) the same `--scale --huge` run:

| | before | after |
|---|---:|---:|
| `clock +60000`, 100 plants, Linux | 7 ms | 284 ms |
| `clock +3600000`, 21,100 plants, Linux | 337 ms | **31,765 ms** |
| `clock +3600000`, 21,100 plants, web | 759 ms | **33,285 ms** |

The hostless `Sim` seek is unaffected (0.34 s at 50,000 plants). A `sample` of
the Linux host during the hour's seek is in tiny-skia painting, the text
cache and `exact_runner::surface_record` JSON decoding: the status record
changes once a garden second, and each change appears to be delivered, laid out
and painted inside the seek (~9 ms each, 3,600 of them), where the seek used to
deliver only its end. Not changed here; reported to the lead.

Proofs after the merge: logic tests 11 + 3 green; Linux and web proofs pass
every check except the three pins, which moved as expected (the world hash
and save format changed) and agree between the hosts — tick 0
`0x567fb953a3936f0f`, tick 5136 `0x84bdee160cf25b17`, continuation
`d6f75574…`. Not re-pinned, per the lead, until the engine is final.

### Second fix round (core's seek fix, render's parented-instance fix)

| hour's seek, 21,100 plants (real hosts) | before core's fix | fruit in world space | fruit parented |
|---|---:|---:|---:|
| Linux (gpu-dev) | 31,765 ms | 199 ms | 428 ms |
| web (Chrome) | 33,285 ms | 571 ms | 2,991–3,124 ms (3 runs) |

Offscreen at 50,000 plants (110,728 drawn), the renderer's feed is now on par:
parented 0.018–0.019 ms a frame, world space 0.019 ms (was 1.86 against 0.019),
mean frame 5.6–5.7 ms both. A web frame time could not be read: under the
agent's clock `perf frames` is virtual and the world's `perf` rings stay empty.
The seek still favours world space (5× on web), so fruit stays unparented.

"NaN sheckles" in the web scale run was the proof's read, not the game: the
proof parsed the purse from `accessibleName`, which web did not report for a
labelled `text` before the host fix, so `Number(undefined)`. The game's value
was right (`379392554 sheckles`, `379M¢` on web and Linux). The scale run now
checks that the purse after "sell all" is finite and positive.

## Agent-directed market play (2026-10-04)

Started this batch by merging main through `880ae483b`. Forest's shared
`decide` helper now also drives Garden: Jev sees the HUD text, the first three
shop rows and enabled buttons, and chooses a button, E, or a 20-second wait.
It cannot write the world or use the stress controls. The world snapshot is
saved after the run for diagnosis, never passed into the decision request.

The baseline web run exposed a real progression bug. At decision 23 Jev sold
three carrots for 67¢. At decision 25 the shop had four strawberries at 50¢,
but its buy button was still disabled. The published shop also offered to
equip a carrot already planted. Jev waited six 20-second turns for the next
restock to refresh the record before it could buy the strawberry. The run
ended at the 48-decision limit with four strawberry harvests still unsold.
Some wasted turns were the policy's own decisions, not engine faults.
Evidence: `games/garden/artifacts/jev-baseline-web/jev-decisions.jsonl`.

Fixed the stale dependencies: planting refreshes seed ownership; selling,
equipping, expanding and successful HUD commands refresh shop availability.
Successful commands now leave visible feedback, including the bought seed.
A malformed `sell` command also refuses instead of accidentally selling the
whole backpack. The scripted host proof checks the state immediately after
these actions, before a restock can hide a missed publication.

Added five market orders (carrot → strawberry → blueberry → tomato → corn).
Each consumes its requested quantity, leaves unrelated fruit, and pays full
fruit value plus a bonus once. `Farm.orders` is both progress and the saved
receipt; there is no second reward ledger. The visible order gives a reason
to plant another crop, and shop rows now explain first-fruit and regrowth
times. The existing freeform farming, mutations and scale controls remain.

On the next web run, Jev completed the first two orders in 13 decisions,
ending at garden time 1:41 with 475¢ and the blueberry order visible. It
still bought an unnecessary carrot while waiting for strawberries, an honest
policy weakness. This is not a controlled timing comparison: the objective
and game both changed. Evidence: `games/garden/artifacts/jev-orders-web/`.
The screenshot has readable order, reward, growth times and enabled shop
controls. These are strategic decisions on a seekable clock, not vision,
physical input or frame-latency measurements.

The SDK tool-path rough edge from the Forest diary is fixed too. `cargoOnPath`
used to return as soon as it found Homebrew's Cargo, leaving `~/.cargo/bin`
off PATH and hiding the already-installed wasm-bindgen. It now appends the
Cargo installation directory while preserving explicit PATH precedence and
`CARGO_HOME`. Binaryen's private pin already had shared lookup. `exact setup
--check` and the new web build both succeeded without a manual PATH override;
the regression also checks repeated calls and a custom Cargo home.

The development lesson: an inspectable state is only useful if it describes
the controls the player can actually use. Reading Farm's purse and invoking
`shop::buy` directly would have hidden the disabled-button bug completely.
The visible-state policy plus normal host input found it without a special
agent-only gameplay API. Keep that boundary when adding Rivals' playtest.

Validation before the next main merge: 13 simulation tests, all 72 enabled
shared app-tool tests, the five root gates, and Linux's full normal proof
passed. The latter includes both early orders and byte-identical continuation
from a fresh process saved before the first delivery. Cross-host re-pinning
and the macOS playtest follow after the fetched Apple build improvements.

### After merging main through `f999c65a6`

The updated dependency graph required another explicit Garden lock refresh.
All 13 simulation tests and the 2,248 enabled root Rust tests passed; build,
Clippy and formatting passed too. The combined shared tooling suite passed
175 tests, with two optional integration tests skipped.

| Jev run | Decisions | Outcome | Decision latency p50 / p95 | Input / output tokens |
|---|---:|---|---|---|
| baseline web | 48 (limit) | 4 strawberry harvests, not sold | 317 / 724 ms | 40,110 / 2,333 |
| market web | 13 | first 2 orders filled, 475¢ | 302 / 679 ms | 11,237 / 612 |
| market macOS | 13 | first 2 orders filled, 475¢ | 291 / 633 ms | 11,239 / 612 |

The two market runs chose the same unnecessary carrot at different moments.
Their screenshots were inspected on both hosts. The macOS deterministic proof
also passed in 8.44 s after the build, matching web and Linux's two world
pins and both continuation saves. Its broad `ps` descendant audit stalled;
every explicitly owned host process still closed. Artifacts:
`games/garden/artifacts/orders-macos/` and `jev-orders-macos/`.

Strict re-pinning then passed on Linux and web in normal, Save and FreshGame
modes, plus the native release-equivalence run. The release carrier's cold
build took 62 s. All hosts agree on the new market continuation digest
`1e80964f…` and the large-garden continuation `8ad353c2…`; accepted inputs are
`c875ef2e…`. Complete evidence is in
`games/garden/artifacts/prove/run-jEvdaO/`. No parity threshold was loosened.

## Rechecking the market on the repaired save path (2026-10-04)

Forest's arranged teleport exposed an engine restore bug: pending camera work
ran before the next game tick after restoring, but after it in the original
simulation. `b37d46a2c` preserves that work and its saved pose (diary 005), on
main through `39ac858b2`. Garden's lock refresh adds `serde_json` to Canvas 2D
and `exact-kernel` to the Windows host, without package-version changes. All
13 simulation tests and three crop unit tests pass.

The strict collector `artifacts/prove/run-UhwFBt` accepts Garden's refreshed
EXSIM v7 baseline after Linux and web agree in normal, Save and FreshGame modes,
plus native release. Both tick hashes stay the same; the two save digests change
from the older format. All seven runs have zero failures and passing descendant
audits. The independent macOS run (66.3 s including rebuild) matches their source
inputs, tick/save pins, four world snapshots and six saves. Its optional `ps`
audit times out while every owned process exits. The release comparison takes
75.4 s including its rebuilds. A subsequent ordinary Linux proof reports `PASS`
against the accepted pins.

Jev replayed the existing two-order objective through the HUD on both hosts.
Each run chose the same 13 actions and ended at garden time 1:41 with 475¢ and
the blueberry order visible. Both bought the same unnecessary carrot while
waiting for strawberries. This is an integration recheck, with no policy change
or claim about completing the remaining three orders.

| Run | Decisions | Decision p50 / p95 | Input / output tokens | Wall time with build |
|---|---:|---|---|---:|
| Web | 13 | 306 / 3,008 ms | 11,237 / 612 | 69.8 s |
| macOS | 13 | 311 / 589 ms | 11,237 / 612 | 8.2 s |

Both Jev process audits pass. Model wall time does not advance the game clock.
Artifacts are `artifacts/jev-follow-{web,macos}/`; both screenshots were viewed,
with readable market rewards, growth times and enabled seed controls. The next
playtest should go beyond the two-order, same-tile controller: ordinary walking,
the remaining crop controls, and all five requests. Its evidence should drive
the next navigation and progression improvements rather than another replay of
the already-working opening.

## Taking the market through all five crops (2026-10-04)

Merged main through `548631e52` before this batch (`c2cf96958`), including the
smaller production Apple bake. The existing playtest now accepts `--full-market`:
96 decisions, all five crop controls, and ordinary WASD walking. Observations
remain visible HUD text; the full world is recorded only after the run.

The baseline bought a blueberry seed at decision 14, then spent the remainder
of its budget harvesting strawberries on the same tile. It never walked or
equipped blueberry. Its 24 strawberry harvests left order three untouched.
This exposed a missing transition in the player's feedback: having a seed does
not explain that a fruiting plant keeps its occupied tile after harvest.

The first revision adds a market next-step hint, a **Hold requested seed**
shortcut using the existing equip command, and a current-plot readout. Held
seeds now have a dark background that stays readable against the sky. Plot
crossings publish immediately, even between empty tiles with identical prompts;
the remembered tile is saved with the other status dependencies. A regression
crosses two such boundaries around a save/restore, plants, and compares bytes.
The controller's short walk also changed from 350 to 500 ms: the Character's
acceleration meant the shorter press could remain in the starting tile. These
runs therefore compare the whole interaction change, not isolated HUD causality.

Guidance got Jev to order four on web and order five on macOS. Both stalled on
random seed stock. Tomato was absent through the early restocks; macOS bought
one only at decision 74. The web player spent enough money on spare seeds to
fall below tomato's price when stock finally appeared. The final revision
stocks at least one requested seed when an order unlocks and at each restock
while it is active. Its normal price still applies. Other stock keeps its
random rolls, and no extra random numbers are drawn by the guarantee.

| Run | Decisions | Orders completed | Purse | Decision p50 / p95 | Input / output tokens |
|---|---:|---:|---:|---|---|
| Full-market baseline, web | 96 (limit) | 2 | 55¢ | 282 / 590 ms | 110,795 / 9,828 |
| Guidance, web | 96 (limit) | 3 | 756¢ | 319 / 715 ms | 117,966 / 10,632 |
| Guidance, macOS | 96 (limit) | 4 | 1,640¢ | 351 / 818 ms | 116,934 / 10,349 |
| Guidance + stock, web | 65 | 5 | 3,291¢ | 386 / 958 ms | 77,261 / 6,377 |
| Guidance + stock, macOS | 63 | 5 | 3,291¢ | 426 / 783 ms | 74,787 / 6,166 |

Final runs finish at garden time 11:05 with four persistent plants. Both still
buy three unnecessary carrots; web repeats tomato and corn equip actions, and
macOS repeats tomato. No further policy tuning in this batch: baseline and two
gameplay revisions close the three-round loop. Model time does not advance the
game clock. These are text-driven strategic runs, not a vision or physical
latency benchmark. Wall times including builds were 56.5 s for baseline,
59.1/65.7 s for guidance and 50.1/54.6 s for stock (web/macOS).

Evidence: `artifacts/jev-full-baseline-web/`, `jev-guidance-{web,macos}/` and
`jev-stock-{web,macos}/`. Both revisions' web and macOS screenshots were viewed:
market text, plot coordinates and held seeds are readable; the capsule still
obscures small plants directly beneath it, now queued. The guidance macOS run's
optional descendant scan timed out while every owned carrier closed; the final
stock runs' process audits both pass.

The deterministic market proof now fills all five orders through normal host
controls, with an intentionally wrong held seed to exercise the shortcut. It
repeats that journey from a fresh process saved before the first delivery and
requires identical continuation bytes. Fifteen simulation tests and three crop
unit tests pass; active tomato stock is checked over twelve restocks without
gifting seeds or money. The new stock test initially stopped just before the
first due tick at exactly 300,000 ms; advancing 300,100 ms, as the existing
restock test does, exercises the intended event. Stable Clippy also caught an
existing modulo expression in crop formatting; it now uses `is_multiple_of`.

The lesson for agentic development is that an understandable objective also
needs a usable next action. A richer snapshot would not have fixed random stock
blocking the market. Fixing the public game loop helped both hosts without an
agent-only shortcut, and the ordinary proof can now preserve the whole journey.

### Accepted verification

Feature commit `a4dc6b67c` is accepted by the strict collector
`artifacts/prove/run-1UyQZO`: Linux and web agree in normal, Save and FreshGame
modes, plus native release. All seven runs have zero failures and passing
descendant audits. Linux takes 1.2/4.9/4.8 s; web takes 53.3/59.1/60.1 s,
including its mode-specific builds. The release comparison takes 26.3 s.
The independent macOS proof takes 18.9 s and matches the same source inputs,
tick/save pins, four world snapshots and six saves. Its process audit passes.
The normal proof's web/macOS market screenshots and web offline-growth screen
were also inspected. A subsequent ordinary Linux proof passes against the
accepted baseline. Pin changes include the saved plot dependency and the
longer market continuation, whose digest is now `c7804a76…`.

Root verification: all five gates pass, with 2,315 enabled Rust tests across
81 binaries and nine ignored tests. Garden's Clippy/format checks pass; the
shared app-tool suite has 73 passes and two optional integration skips.

### Main's paint-rank merge

The later Forest axe batch merges main through `85bd9ba9b` as `6250a2856`.
Garden's unchanged complete proofs pass on web in 88.9 s and macOS in 42.0 s,
including rebuilds. Their source inputs, tick/save pins, four world snapshots
and six saves agree. Web's descendant audit passes; macOS's optional scan times
out while every owned carrier closes. Artifacts: `artifacts/main-paint-{web,macos}/`.

## Seeing the active plot (2026-10-04)

Main moved another eight commits while Forest's axe was being verified. They
are documentation and queue updates, merged through `407cd0b5c` as `97746517c`
before this batch; source stays fixed during the cross-host collector.

The market screenshots exposed a human problem that Jev's text observations
cannot detect: the capsule and a planted crop share their tile's centre. The
player covers a seedling completely and most of a blueberry plant. Crops now
stand 0.65 m to the right and 0.35 m forward of the interaction centre. Stems
and world-space fruit share that anchor through growth and regrowth; fruit
stays unparented, preserving the measured long-seek saving above.

Four thin planes outline the current interaction tile: cyan for empty, amber
for growing or regrowing, green for ripe fruit. They hide outside the garden.
This is four entities regardless of garden size. Updating the outline reads
only the current tile and its fruit slots, runs with the existing status
change publication, and avoids writing unchanged transforms or materials.
The text prompt still names the action and countdown, so colour is additional
feedback, not the only way to tell what is ready. A player can still walk over
a crop; the offset improves the normal planting position, not all occlusion.

The first visual round is accepted without further tuning. Normal proofs on
web and macOS pass all behavioral checks, including all five orders and two
fresh-process continuations with identical saved bytes. Their planted, ripe,
blueberry and final-market screenshots are retained in `artifacts/plot-{web,macos}/`.
The small seedling, ripe carrot and blueberry are visible beside the avatar,
and the outline identifies which of the adjacent plants E will harvest.
Web takes 48.3 s and macOS 26.0 s including builds. Web's descendant audit
passes; macOS's optional scan times out while every recorded carrier closes.
These exploratory runs do not accept pins; the strict collector follows.

Seventeen simulation tests and three crop unit tests pass. New regressions
exercise the outline's growth, harvest, movement out of and back into the
garden, and save/restore continuation; all fourteen crops keep their stem and
fruit anchors through stepped and smooth growth and a second harvest cycle.
The ordinary proof now checks the outline transitions and captures those
early planting and harvest scenes on both graphical hosts.

The engine supplied the required meshes, materials, visibility and saved state
without a new feature. The development lesson is about the observation loop:
a successful text-driven playtest needs a separate visual inspection. A full
market completion had hidden this defect, and replaying the same policy alone
could never prove that it was fixed.

### Unchanged Jev replay and scale

The same full-market policy is replayed once per host, without prompt, action
or economy changes. Web completes all five orders in 69 decisions (3,064¢,
11:05 garden time). Mac completes three, steps west out of the garden after
returning to the strawberry tile, and spends most remaining decisions walking
north. "Outside the garden" tells it where it is not, but gives no direction
back. It ends at `[1.891, 0.9, -12]`, the movement bound, with 6¢ and its tomato
seed unused. The outline correctly hides outside the garden. This is a new
queued recovery-feedback case, not evidence that the visual change affected
Jev, whose inputs do not include images. No policy tuning or further replay
in this batch; the earlier market baseline+A+B batch stays closed.

Artifacts: `artifacts/jev-plot-{web,macos}/`. Decision latency p50/p95 is
290/484 ms web and 301/666 ms Mac; input/output tokens 82,517/6,965 and
109,343/9,714. Wall times are 25.2 and 62.1 s, both with cached builds and
passing descendant audits. Model time is outside the game clock.

The existing release `entity_ramp` measurement, after those runs have closed:

| Plants | Entities when mature | Stepped mean ms/frame | Smooth mean ms/frame | One-hour seek |
|---|---:|---:|---:|---:|
| 100 | 435 | 0.001 | 0.001 | 76 ms |
| 10,000 | 42,866 | 0.001 | 0.011 | 110 ms |
| 50,000 | 214,292 | 0.004 | 0.060 | 286 ms |

All three mature mean frame costs round below 0.001 ms. At 50,000 plants the
hour processes 332,168 events, saves 6,398,657 bytes in 139.7 ms, and restores
in 121.8 ms with the same hash. These are simulation measurements, not rendered
frame timings or a before/after performance comparison. Full observations
still cost O(entities): 34.0 ms for the largest garden's observed tick.

The existing offscreen GPU fixture also passes: at 150 garden seconds with
100/10,000/50,000 plants, 180 rendered frames average 1.4/2.0/3.4 ms with
p95 2.5/3.3/4.3 ms. The largest scene has 110,732 entities, 28 draws, and
0.015 ms mean scene-feed time. This is the native release renderer's overview
fixture on this Mac, not a browser or app-window latency measurement. Both
Jev screenshots were inspected as well: web's corn is visible beside the
avatar; Mac's missing outline agrees with its outside-garden state.

### Accepted plot verification

Feature `15447a888` is accepted by `artifacts/prove/run-P5FxVf`: all seven
normal/Save/FreshGame Linux and web runs plus native release agree, with zero
failures and passing descendant audits. Linux takes 18.2/4.7/4.8 s, web
64.7/61.8/52.3 s, including mode-specific builds; release takes 66.7 s. The
collector restores the normal web build. The independent macOS proof matches
the collector's source inputs, all pins, four world snapshots and six saves.
The saved outline and plant positions change tick 0 to `0x45f32f6360bbbb73`,
tick 5136 to `0x5e57e536867f9d6f`, continuation to `d7a91ec9…` and market to
`6910128d…`. The subsequent ordinary Linux proof passes the accepted pins.

All five root gates pass: 2,316 enabled tests across 81 binaries, nine ignored;
Garden's Clippy and formatting checks also pass. The core's deprecated
`fetch_update` warnings occur only with the newer pinned web nightly, not the
stable gate toolchain. No engine code or Jev policy changes in this feature.

### Second main update during this batch

Main advanced 31 more commits while the plot collector ran. After accepting
its baseline, merge `60e9fde3c` brings in `8e003c707`: inherited game visibility,
authored presentation hooks, native shader packaging, Windows grants, shared
Calendar, and the JS-target differential fixes. Conflicts retain our generated
Windows formatting, silent-surface regression and game freshness exclusions
alongside main's presentation hooks and platform-local module tracking.
All three game lockfiles needed the new `exact-grants -> serde_json` edge;
the existing `--update-lock` command captures it.

Garden's complete proof still passes with unchanged pins on web (137.3 s) and
macOS (58.7 s), with equal inputs, four world observations and six saves.
Both new market screenshots were inspected and retain the crop spacing and
outline. Artifacts: `artifacts/main-8e-{web,macos}/`. Both optional descendant
scans time out, with every recorded carrier closed. Builds and verification
ran beside the other games and checks, so these are integration wall times,
not isolated performance comparisons.

All five merged root gates pass: 2,325 enabled Rust tests across 80 binaries,
nine ignored. Engine/renderer tests pass 630 with 17 ignored; Linux surface
tests pass 56, including the retained silent-message regression and new shader
tests. The app-tool suite passes 80 initially, with two optional skips and one
new fixture expecting the old generated type spelling. Updating that fixture
to check both `type App = game_logic::Island` and the bake's `App` argument
makes all five tests in its file pass; no generated behavior changes.

The advisory semantics command's default merge-base check finds no changes
after a merge. Retrying with `--base 38631aab2` correctly selects this update,
but cannot run the oracle: this Mac has no Lean `lake` executable. That check
is unavailable, not passed; the five gates above do not depend on it.

## Browser tap integration recheck (2026-10-04)

During Rivals' bot-navigation work, merge `c04a3f0f0` brings 23 commits from
main through `3d76ccdb7`. The web agent now verifies that a press reaches its
intended target. Garden's complete web proof passes in 63.9 s including builds,
keeps every tick/save pin, and passes the descendant process audit. The market
capture was inspected: five orders filled, four visible plants, readable shop
and active-plot feedback. Artifact: `artifacts/main-3d-web/`. No Garden gameplay
or Jev-policy change in this integration sweep. Rivals diary 006 records the
passing root gates and the advisory semantics check unavailable without Lean.

## Game-shell toolchain integration (2026-10-04)

Merge `4a02f5cf2`, in the Rivals spawn/Mayhem batch, brings main through
`8c9b476fa`. Generated game shells now inherit the SDK toolchain explicitly,
including games outside the SDK tree. Garden's complete proof passes on web
in 70.0 s and macOS in 27.2 s, including builds beside other verification.
Inputs, pins, four world observations and six saves agree. Web's process audit
passes; macOS's optional scan is unavailable while every owned carrier closes.
Both market captures were inspected. Artifacts: `artifacts/main-8c-{web,macos}/`.
Rivals diary 006 records the shared checks and the correction of recent test
totals that had counted nested subprocess result lines twice.

## Main's button and colour update (2026-10-04)

At Charlie's reminder to keep current, merge `a0a6d35fa` brings 120 commits
through main `c2e909694`, before beginning the separate lost-player recovery
change. The shared CSS colour parser supersedes our older local parser;
main's button alignment removes Garden's hand-written label padding. Both
sets of queue findings are preserved. The SDK and all three game lockfiles
need the new `exact-canvas -> exact-motion` edge, captured with the existing
`--update-lock` command.

Garden's full proof passes on web (158.2 s) and macOS (66.9 s), including
builds beside other verification. Inputs, every pin, four world observations
and six saves agree. Both market screenshots were inspected: the controls
are centred and readable, with the crop spacing and active plot retained.
Both optional descendant scans are unavailable; all owned carriers close.
Artifacts: `artifacts/main-c2-{web,macos}/`. Rivals diary 006 records the shared
checks. No Garden logic or Jev policy changed; recovery guidance remains next.

## Returning from outside the garden (2026-10-04)

The previous native Jev run completed three market orders, walked west at
choice 27, then repeatedly headed north until clamped at `(1.8905318, -12)`.
The HUD said "Outside the garden"; its prompt was blank. The starting six-row
garden ends at z=-11, so north cannot get this player back. The earlier market
policy/economy batch remains closed; this is a separate public-feedback defect.

The prompt now gives an inward direction, its WASD key and approximate distance
to the nearest plot. It aims at that plot's centre, chooses the larger axis,
and recomputes while walking, so a corner can need two directions. Re-entering
restores the ordinary planting/harvesting prompt and outline. The calculation
uses the garden size and player position only, with no plant scan, navigation
resource or extra engine API. Expansion follows the same bounds as `tile_at`.

Two new regressions fail on the blank prompt before the change. The recorded
north-boundary position is reached with ordinary movement; pressing E outside
spends no seed, and a southward return plus planting continues byte-identically
after save/restore. The other test follows only the published direction from
every edge and corner of both six- and sixteen-row gardens. All 19 simulation
tests and three crop tests pass; four measurement tests stay ignored. Strict
Clippy with the determinism config passes for the library and simulation tests.
Applying that config to all targets also rejects the existing measurement
files' deliberate `Instant` use; no simulation lint is waived. Formatting passes.

The real-host proof adds the same lost-position journey and a fresh-process
return, using only keys and visible prompts. Initial web/macOS runs have zero
failures in 92.6/30.1 s, with equal inputs, pins, six world observations and
nine saves. Both process audits pass. Lost/returned captures on both hosts were
inspected; the instruction is readable and the player can plant on returning.
Artifacts: `artifacts/return-{web,macos}/`. These runs remain UNVERIFIED until
the strict collector accepts the new recovery save below.

Unchanged full-market Jev runs complete all five orders in 64 web decisions
(3,291¢, 664.7 game seconds) and 66 native decisions (3,084¢); neither leaves
the plots. They establish ordinary play but do not exercise recovery.
`--start-outside`, added to the existing playtest with `--full-market`, walks
to the recorded north-boundary trap before handing control over. It keeps the
model, goal, choices and decision budget unchanged; no world mutation or extra
model-only observation sets up the case. Both first runs buy one carrot seed,
then choose `walk_south` on decision two and plant on decision three.

### Jev's result and the limit of the improvement

The targeted runs both use all 96 decisions and deliver four orders. Web
ends with the corn seed held, 1,177¢ and three plants at 518.9 game seconds;
native has planted corn, has 353¢ and is twenty game seconds from its first
corn harvest at 796.5 seconds. The public return prompt resolves all six/five
outside episodes that the respective decision loops observe, in one or two
decisions each. Web's final decision walks north out again; the cap prevents
another observation/return. This is recovery evidence, not a complete-market
win. Ordinary unforced runs finish all five orders on both hosts above.

At the last row, "Move to an empty tile" still gives no direction. Web keeps
trying north from an occupied tile and returning south onto it, with seed
purchases interleaved; native eventually moves east to plant corn. That is a
separate queued finding. Do not tune the controller or increase its cap to
turn this batch into a win. The baseline and public-guidance change close this
recovery target without reopening the earlier market tuning rounds.

Artifacts: `artifacts/jev-return-{web,macos}/` for ordinary play and
`artifacts/jev-outside-{web,macos}/` for the controlled start. All four process
audits pass. Targeted model latency p50/p95 is 284/576 ms on web and 301/543 ms
on native; request usage totals 116,500/9,917 and 116,121/9,952 input/output
tokens. Ordinary runs are 311/540 and 323/543 ms, with 75,965/6,248 and
78,774/6,599 tokens. Whole targeted runs take 35.7/56.3 wall seconds. These
are visible-text decisions on an advanced clock, not pixel perception or
real-time play. All four final captures were inspected.

### Accepted baseline and another main update

After feature `b65d36c1f`, merge `a5f62346d` brings main `00d37ef9f`: the SVG
filter chain is now below the kernel. `79e52f11b` refreshes the shared and three
game locks for `exact-svg-filter`; all three focused lock tests pass. All five
root gates pass on this merged source: build, 2,347 enabled tests across 81
binaries (nine ignored), strict Clippy plus formatting, caps and boot. Before
that merge, one root run reproduced the already queued exclusive update-store
lock flake; the full failed binary then passed all 50 tests. Its cause is not
fixed by this game change; the additional Mac evidence is on the existing
queue item. Logs: `/tmp/exact2-main00d-{build,test,clippy,lock-tests}.log`.

With the source fixed, strict collector `artifacts/prove/run-7kIc7z/` exits zero
and accepts all seven matching runs. Linux normal/Save/Fresh Game take
39.810/6.008/5.792 s; web 97.186/90.492/86.435 s; native release 58.027 s.
The final ordinary web rebuild takes 20.007 s. Each gameplay run has zero
failures and an available, passing descendant audit. Collector child reports
say UNVERIFIED while recording the candidate; the collector's full agreement
is what accepts it. A subsequent ordinary Linux proof checks the accepted
pins and reports PASS in 1.424 s (`artifacts/return-strict-linux/`).

The independent macOS proof on the merged source matches all seven runs:
inputs, six world observations and nine saves, including the return and
planting continuation. It takes 43.312 s, with zero failures; its optional
process scan is unavailable while all owned carriers close. Lost/returned
screenshots were inspected again. Artifact: `artifacts/return-main-macos/`.
Every old tick and save pin remains unchanged. The only new save pin is
`recovery = 302e11d239c811ad4ea3f29d1a96a0098de23ee664ba2cd03837803a6cff26b0`;
the expanded proof's input digest is
`0cd9c503afd70139d3ef013cbaf70dec3e1ba81f4348ecc2c977d4914d75d96a`.

Forest and Rivals also pass both hosts after the merge, with matching states
and unchanged pins; their diaries record that sweep. These verification wall
times include builds and concurrent checks, not isolated performance data.
The useful engine lesson here is that existing publication, input and save
seams were enough: give the player the missing spatial fact, then test that
following it works. Private coordinates in Jev's input would have bypassed
the defect that a person could encounter too.

## Main compiler integration during Forest's preparation work (2026-10-04)

Merge `e278ff4b6` brings main through `02f53744a`. Garden's full web/macOS
proofs pass in 94.6/30.9 s, including builds. Inputs, pins, six world
observations and nine saves agree. Both returned-to-garden screenshots were
inspected; recovery guidance and planting remain usable. Web's process audit
passes; native's optional scan is unavailable with owned carriers closed.
Artifacts: `artifacts/main-02f-{web,macos}/`. The later merge `02ca9c62f` adds
main `e702a02e3`'s Lean-only changes. Forest diary 005 records the shared gates
and unavailable Lean advisory check. No Garden gameplay or Jev-policy change.

## Main build and browser input integration during Rivals pacing (2026-10-04)

Merges `ec4375505` and `d9a33f1c8` bring main through `a98895a22`.
Garden's full web/macOS proofs pass in 104.0/32.7 s, including builds.
Inputs, pins, six world observations and nine saves agree. Both returned
screenshots were inspected; recovery guidance and planting remain readable.
Web's process audit passes; native's optional scan is unavailable with every
owned carrier closed. Artifacts: `artifacts/main-a988-{web,macos}/`. Rivals
diary 006 records the shared gates and browser-modifier fixture finding.
No Garden gameplay or Jev-policy changes.

## Contract module scoping integration (2026-10-04)

Merge `e19460756` brings main `7cdf080e2` at the next completed proof boundary.
Garden passes web/macOS in 104.6/33.2 s. Both hosts agree on inputs, all pins,
six worlds and nine saves. Every world and save also matches the previous
main integration; only the source-input digest changes. Both returned-to-plot
captures were inspected. Web's process audit passes; native's optional scan
is unavailable while all owned carriers close. Artifacts:
`artifacts/main-7cdf-{web,macos}/`. Rivals diary 006 records the shared checks.
No gameplay or Jev-policy change; wall times include builds and concurrent work.

## A seed has a direction to an empty plot (2026-10-04)

The outside-start Jev runs recovered from the boundary but finished with only
four orders at the 96-decision cap. At an occupied last-row plot, neither
the plot readout nor the market's “find an empty tile” said where one was.
This is a new feedback gap; the earlier recovery/controller batches stay closed.

The HUD now shows the nearest empty plot's coordinates, cardinal direction,
WASD key and approximate distance while a seed is held on an occupied tile.
The existing growth/harvest prompt stays visible. Reaching an empty tile,
leaving the garden or using the last seed removes the hint. A full garden
says so and suggests expansion only below the size limit. The authoritative
tile table supplies the answer; equal distances keep row-major order, and
there is no saved navigation cache or new engine API. The lookup runs during
HUD publication, not each simulation tick.

The new north-edge regression first fails on the old code because no
planting direction is published. It now walks using only that text, plants
and produces identical saves after restoring while the hint is active. A
second test frees a full garden's opposite-corner carrot, follows the hint
there, fills it again, expands and follows the newly available space. All
21 simulation tests and three crop tests pass; game determinism Clippy
and formatting pass. The real-host proof extends boundary recovery with
buying another carrot and following the empty-plot hint to a second planting,
then repeats the whole continuation in a fresh process.

Exploratory web/macOS proofs have zero failures in 91.5/33.8 s, matching
inputs, pins, six world observations and nine saves. Both empty-direction
captures were inspected. Web's process audit passes; native's optional scan
is unavailable with every owned carrier closed. Artifacts: `artifacts/empty-{web,macos}/`.

One fresh outside-start Jev run per host, with the same goals, choices,
movement motor and 96-decision cap, completes all five orders in **53/54
decisions**. Both finish with 3,094¢ and four planted crops. The only added
observation is the new visible planting label. Web follows east at decisions
15 and 25, then south at 40; macOS follows east at 16 and 26, then south at
41. Both initially follow the return prompt south. They still sometimes
buy an extra seed or equip one twice; no policy tuning follows the outcomes.
These two exploratory text-driven runs do not establish a success rate or
visual perception. Both process audits pass, both final captures were inspected,
and this guidance batch is closed. Artifacts: `artifacts/jev-empty-{web,macos}/`;
wall times including builds are 27.5/33.0 s. Model latency never advances
game time. Decision latency p50/p95 is 349/629 ms on web and 340/533 ms
on macOS; input/output tokens are 63,258/5,166 and 64,411/5,245. The
completed worlds are at 569.3/569.4 game seconds.

The maximum-size lookup (one real plant, 65,535 empty tiles) averages
**0.0389 ms** across 1,000 release calls. Over 600 live frames, mean/max
simulation cost is 0.0004/0.0172 ms without the hint and 0.0010/0.0511 ms
with it. This isolates the tile lookup, not rendering a full garden.
The first measurement fails its setup assertion: the existing scale helper
advances `1000 / 30` ms, which rounds to 33,333 microseconds, short of a
30 Hz tick. It could report a command's timing while that command was still
queued. The helper now crosses the boundary by one microsecond and asserts
exactly one tick executed. All four ignored scale cases pass with
`GARDEN_SIZES=100`, including the new maximum-table case. Historical command
timings using that helper did not guarantee a tick; no comparison to those
old timings is claimed. Logs: `/tmp/exact2-garden-empty-{baseline,tests,scale-all}.log`.

Strict acceptance and the next periodic main integration follow this checkpoint.

### Accepted verification and another main update

Feature `374d89061` is followed by merge `f59d587ac`, bringing main
`e097e4cae`'s Cargo metadata cache key, browser shutdown evidence and Apple
paint-plane changes. All five root gates pass: build, 2,351 enabled tests
across 81 binaries (nine ignored), strict Clippy, formatting, caps and boot.
The app/build, Windows portability and surface-record suite passes 164 tests
with six optional/platform skips. Logs: `/tmp/exact2-maine097-*.log`.
The first root-test transcript used two independently opened file handles
for stdout/stderr and overwrote some output; a repeated passing run uses one
redirected stream. Counts use each Cargo-launched binary's final result once.

With sources fixed, strict collector `artifacts/prove/run-bTYZe6/` accepts
all seven matching runs. Linux normal/Save/FreshGame take 13.9/6.4/6.3 s;
web 81.4/98.6/74.8 s; native release 28.8 s. Every run has zero failures.
Six descendant audits pass; web Save's optional scan is unavailable while
all owned carriers close. The collector restores the ordinary web build
(21.0 s). Its child summaries remain UNVERIFIED while collecting candidates;
acceptance comes from the successful collector and written pins.

The independent macOS run (`artifacts/empty-main-macos/`) takes 39.4 s and
matches the inputs, every pin, six world observations and nine saves of all
seven strict runs. Its optional descendant scan is unavailable with owned
carriers closed. The empty-plot screenshot was inspected again. Both old
tick pins remain unchanged; continuation/market save digests now include
the new HUD field. Recovery additionally buys, follows the hint and plants
a second carrot; its accepted save is `048fa471…`.

Forest and Rivals pass complete web/macOS proofs after the merge, retaining
their existing world and save pins. Their diaries record those comparisons
and inspected captures. These verification times include builds and concurrent
work. The empty-plot queue item is closed; future playtesting should seek new
gameplay evidence rather than tune this successful controller batch further.

A final ordinary Linux proof checks the accepted pins and reports PASS in
1.475 s, with a successful process audit (`artifacts/empty-accepted-linux/`).
Caps passes after staging the pins and all three diary updates.

## Process-audit correctness after the guidance batch (2026-10-04)

Repeated optional native audit skips expose two shared proof defects. A
nonzero `ps` exit returns no rows without marking the audit unavailable, so
a failed scan can look like an empty successful one. Separately, synchronous
work can delay an already successful child's events past the 200 ms watchdog.
A real `ps` normally finishes in about 27 ms; blocking the caller for 400 ms
reproduces the false timeout immediately before its queued exit-zero event.
This is an audit-coverage fix, not an explanation for long game proof times.

`processInventory` now waits through `close`, including stdout drainage.
The existing deadline requests termination of that invocation's owned child;
an additional bounded grace handles a child that never closes. Exit zero
still succeeds if delivery was delayed. Spawn errors, nonzero exits, empty
or incomplete output missing the proof process, and genuine timeouts report
an unavailable audit with their specific reason in `process-cleanup.json`.
PID/start-time matching and the final recorded-child leak check are unchanged.

The real 400 ms blocked-caller reproduction now reads all 973 live rows;
an owned sleeping subprocess is killed and reported unavailable in 206 ms.
A regression covers stdout after exit, failed and empty scans, spawn errors,
delayed success, real termination and an uninterruptible child. The focused
checks pass, followed by all 117 shared proof/decision/Windows tests (four
platform skips, 204.8 s including fixture builds). To keep the proof test
under 1,500 lines, its existing Windows receipt case moves unchanged to
`scripts/windows.test.mjs`, and redundant blank spacers are removed. No
new test runner or script is added.

All six full game runs pass with successful descendant audits and no remaining
recorded children. Garden web/macOS take 67.1/29.5 s, Forest 64.2/164.5 s,
and Rivals 59.1/57.6 s. Every input digest, pin, world observation and save
matches the corresponding accepted pre-change baseline. Garden retains six
worlds and nine saves. All six representative screenshots were inspected;
the planting hint remains readable on both hosts. Artifacts for each game:
`artifacts/audit-{web,macos}/`; tooling log:
`/tmp/exact2-process-inventory-tools.log`. These are concurrent verification
times including builds, not isolated performance measurements. No gameplay
or Jev-policy changes are part of this batch.

Feature `e4086cec8` is followed by periodic main merge `ebbd6ce65`, bringing
`4e228c5d0` without conflicts. The incoming delta is only LLP 1092/1094
documentation, reviews and their DEFERRED admissions; executable inputs are
unchanged, so the six game comparisons above still apply. All five root checks
pass: build, 2,351 enabled tests across 81 binaries (nine ignored), strict
Clippy and formatting, caps and boot. This local run takes 65.2 s in total,
including 62.5 s for tests, exceeding the 60 s target. Logs:
`/tmp/exact2-auditmain-{build,test,clippy,fmt,caps,boot}.log`.

## Browser-cleanup integration during Rivals label work (2026-10-04)

Merge `745a66652` brings main `152cf5b17`. Garden passes complete web/macOS
proofs in 121.3/31.4 s, retaining all pins, six worlds and nine saves from
the accepted audit baseline. Only the source-input digest changes. Both
empty-plot captures were inspected. Native's descendant audit passes;
web's optional scan reports `ps timed out after 200 ms`, with every owned
carrier closed. Artifacts: `artifacts/plates-main-{web,macos}/`. Rivals
diary 006 records the passing root and tooling checks. No Garden gameplay
or Jev-policy changes; times include builds and concurrent work.

## Native idle-tick and physics cache integration (2026-10-04)

Merge `16baa33a8` brings main through `e05dff0c0`. Forest diary 005 records
the measured physics digest-cache change and passing root checks. Garden
passes complete web/macOS proofs in 82.5/40.8 s, including builds. Inputs,
all pins, six world observations and nine saves agree between hosts; worlds
and saves also match `plates-main-macos` before the change. Both descendant
audits pass with no recorded children left, and both empty-direction
captures were inspected. Artifacts: `artifacts/inline-{web,macos}/`.
No gameplay or Jev-policy changes.

## Watering gives waiting players something useful to do (2026-10-04)

The previous full-market Jev runs used 28 waits each, out of 53/54 decisions
(`jev-empty-{web,macos}`). This batch adds optional plant care: Q spends one of three water doses to cut the
current plot's remaining growth or fruit wait by 25%. Each growing plant
and each new unripe fruit can take one dose. R refills beside a blue barrel
west of the first plot. The HUD exposes both buttons, dose count, current
care status and a direction to the barrel; a watered outline turns blue.
Unwatered growth and the five-order economy keep their existing rules.

The implementation uses the game's saved components and event heap. A dose
shifts the effective start and deadline together, retaining smooth-growth
progress. The earlier event is inserted in the same heap; only an event
matching the current deadline may advance the plant or ripen its fruit.
Old events become harmless when reached. Work is bounded to the current
plant and its fruit slots, with no scan of the world or heap. This is a
useful authoring pattern for rescheduling saved gameplay work through the
existing engine's Data and clock, and it survives offline catch-up.

One compile attempt used a nonexistent Transform constructor; corrected to
`Transform::at`. The first Linux drive then exposed delayed barrel-range
publication: within the same plot, the refill button could lag until the
next status second, and the scripted walker overshot outside the garden.
`Shown` now watches the range boundary as well as the plot, event count and
clock. The proof also follows the public return hint before harvesting;
being close enough to a barrel does not imply standing on a crop. Original
failure artifacts remain in `artifacts/water-linux/` (four fixture failures,
with both continuations still equal). The corrected Linux candidate has
zero failures in 3.6 s; web/macOS candidates in 104.5/44.2 s also have zero.
Their inputs, eight worlds and twelve saves agree. These are candidate
observations, reported UNVERIFIED until strict acceptance updates the pins.
Both watering screenshots were inspected. A final visual adjustment extends
the ground padding so the barrel's whole base sits on it.

Tests now cover late and early watering, repeated-dose refusal, old deadline
suppression, fruit regrowth, empty cans, distance-limited refills and their
immediate HUD availability within one plot and one second. The offline
catch-up comparison also waters before going away. All 27 enabled game
tests pass (24 simulation, three library; five measurements ignored), as do
strict Clippy and formatting. The new real-host sequence saves while away
from a watered carrot, follows the barrel hint, refills, returns and harvests
at 19 seconds, before the ordinary 20-second deadline, then repeats from a
fresh process and compares the complete continuation save.


Periodic merge `869619101` integrates main `b89e55179`, a docs-only delta.
The five root checks pass in 42.409 s: build 0.253, tests 39.777 (2,351
passed in 81 binaries, nine ignored), Clippy 0.232, formatting 2.048, caps
0.083 and boot 0.016 s. Logs: `/tmp/exact2-water-root-*.log`.
Forest and Rivals' executable input digests still match their accepted
`inline-macos` proofs, so their unchanged runs are not repeated.

The first Jev pair found a real input problem that the basic care fixture
missed. Native finished all five orders in 56 decisions, using three doses
and 26 waits. Web reached its 22nd decision at a blueberry's fruit stage,
then tapping Water failed: `node #50 covers its middle`. The visible text
was clear of the button, but the bottom text column's transparent box had
grown over it after a held seed added the empty-plot hint. This is CSS hit
testing doing what it says, and the game had omitted `pointer-events="none"`
on that read-only overlay. That property now belongs to the text container;
the real-host care fixture buys a spare seed before watering to retain the
exact obstructing layout. The author guide records the pattern. Original
browser failure: `artifacts/jev-water-web/`; successful native observation:
`artifacts/jev-water-macos/`. Jev's choices and goal remain unchanged for the
browser follow-up. The input fix is tested on both hosts.


The corrected tall-HUD proofs have zero failures on web/macOS in 82.8/41.5 s,
with matching inputs, eight worlds and twelve saves. Both cleanup audits pass
and both watering captures were inspected (`artifacts/water-hit-{web,macos}/`).
The browser Jev follow-up finishes all five orders in 53 decisions, using
three water doses and 26 waits, with 3,076¢ at game time 524.3 s. Native's
previous successful run finishes in 56 decisions, 3,016¢ at 524.6 s; it
predates only the pointer-hit fix. Neither chose to refill; the deterministic
proof exercises that route. Both final screenshots were inspected and both
audits pass. These text-driven exploratory runs show the action being used,
not a measured improvement in fun or success rate; the older 28-wait runs
started outside the garden and are not a controlled timing comparison.
This feedback batch is closed, with no model-policy tuning after the results.

Browser/native decision latency p50/p95 is 416/673 and 335/599 ms; gateway
input/output token totals are 68,023/5,289 and 72,447/5,783. Wall times are
29.9/33.5 s, and model latency does not advance game time. Artifacts:
`artifacts/jev-water-hit-web/` and `artifacts/jev-water-macos/`.
Strict baseline acceptance follows for the saved can, growth spans, barrel,
HUD fields and the new watering continuation pin.

Strict acceptance succeeded at feature commit `4e0b3496a`. All seven runs
agree on inputs, pins, eight world observations and twelve saves, including
the independent macOS candidate above: Linux modes 0/1/fresh in
4.234/5.588/5.630 s, web modes 0/1/fresh in 63.140/107.876/98.046 s,
and release Linux in 26.135 s. Every descendant audit passes with no
recorded children remaining. The ordinary web build is restored afterward
(19.519 s). Artifacts: `artifacts/prove/run-g5ejkv/`.
The accepted watering continuation is `07f23e64d7494044…`; the other pins
move with the saved fields and barrel scene, as expected. An ordinary Linux
proof against the accepted pins passes in 1.950 s, with zero failures and
a passing cleanup audit (`artifacts/water-accepted-linux/`).

## Contract modules integration from main (2026-10-04)

Periodic merge `887ee2b78` brings main through `a73a3ae4a`, including
Contract module resolution and source-graph watching. The frozen Bun install
succeeds. All five root checks pass in 123.304 s: build 19.532, tests 82.358
(2,357 passed in 81 binaries, nine ignored), Clippy 19.255, formatting
2.058, caps 0.086 and boot 0.015 s. This exceeds the 60 s target and is
recorded in QUEUE; no game proof ran concurrently with the checks. Logs:
`/tmp/exact2-modules-main-*.log`. The first temporary command wrapper used
an unsupported Bun stdio option and exited before running any check; the
corrected wrapper uses one log descriptor for both output streams.

The rebuilt Garden passes complete web/macOS proofs in 101.5/42.9 s.
Inputs, all pins, eight world observations and twelve saves agree between
hosts; worlds and saves also match `water-hit-web` before this integration.
Both descendant audits pass with no recorded children left, and both
watering captures were inspected. Artifacts: `artifacts/modules-{web,macos}/`.
No gameplay or Jev-policy changes.

## Physics capture and platform-color integration (2026-10-04)

Forest diary 005 records the measured collider-verification cache change
(`ce65b166a`), merge `0299f571d` through main `f708c99cf`, and passing root
checks with their 181.5 s budget miss. Garden passes complete web/macOS
proofs in 114.4/57.1 s, including builds. Both hosts agree on source inputs,
all pins, eight world observations and twelve saves; worlds and saves also
match `modules-web` before the change. Both descendant audits pass with no
recorded children left, and both watering captures were inspected.
Artifacts: `artifacts/holes-{web,macos}/`. No gameplay or Jev-policy changes.

## Cold physics capture and semantics integration (2026-10-04)

Forest diary 005 records the measured comparison-buffer change (`eaf1825ad`)
and merge `3d8603704` through main `b79156175`, plus passing root checks and
their 116.0 s local budget miss. Garden passes complete web/macOS proofs in
104.5/42.7 s including builds. Inputs, all pins, eight world observations
and twelve save files agree between hosts; worlds and saves also match
`holes-web` before the change. Both descendant audits pass with no recorded
children left, and both watering captures were inspected. Artifacts:
`artifacts/scratch-{web,macos}/`. Gameplay and Jev's policy are unchanged.

## Testing-browser discovery and runner integration (2026-10-04)

Merge `775231855` brings main through `9db90ce57`; Rivals diary 006 records
the testing-browser lookup change (`1a1aab53c`) and its tooling fixtures.
Garden passes complete web/macOS proofs in 106.790/46.270 s including builds,
with `CHROME` unset. The browser selects the installed pinned Chrome for
Testing, and both descendant audits pass with no recorded children left.
Inputs, pins, eight world observations and twelve saves match between hosts;
worlds and saves also match `scratch-web` before the merge. Both watering
captures were inspected. Artifacts: `artifacts/browser-{web,macos}/`.
Gameplay and Jev's policy are unchanged.

## Update-store ownership and main integration (2026-10-04)

Merge `f95380e2f` brings main through `dfec331d5`; Rivals diary 006 records
the update-store ownership fix and passing root checks, including their
216.4 s budget miss. Garden passes complete web/macOS proofs in
119.375/57.132 s with `CHROME` unset. Inputs, pins, nine final world
observations and twelve saves agree between hosts. This run enables the
existing `EXACT_PROOF_COMPARE=1` close-time capture; its eight observations
shared with `browser-web` retain the same ticks and hashes, and every save
remains byte-identical. Both process audits pass with no recorded children
remaining, and both watering captures were inspected. Artifacts:
`artifacts/owner-{web,macos}/`. No gameplay or Jev-policy changes.

## Compost gives spare fruit another use (2026-10-04)

Watering already lets players spend a small resource to shorten a wait. This
batch adds a different choice: compost a specific backpack fruit instead of
selling it, then feed a plot for a heavier upcoming harvest. One fruit yields
one dose; the pouch holds three. F or the Feed button spends one dose on a
seedling's first fruit batch or a mature plant's current unripe fruit. Weight
is multiplied by 1.25 at ripening, with no extra RNG draws; the existing value
formula then prices it. Regrowth starts unfed. Repeated feeding, empty/ripe
plots, stale fruit IDs and a full pouch spend nothing. The UI names each
compost trade, marks the fed plot purple and labels the harvested fruit Fed.

All of this fits existing saved components, resources, action buttons and
messages. No engine API or additional scheduler is introduced. Twenty-nine
Garden tests pass (five scale/render tests stay ignored), including exact
weight/mutation comparisons, first growth and fruit-stage feeding, save/restore,
ordinary regrowth, compost refusal and the existing offline/watering equivalence
case extended to feeding. The market fixture also accepts a Fed fruit at its
full value. Determinism lints and strict all-target Clippy pass. The first test
attempt wrongly checked feeding readiness after new fruit had already ripened;
that assertion now runs while the regrowth is unripe. Logs:
`/tmp/exact2-compost-{author-checks,clippy,tests-1,tests-2}.log`.

The initial web drive has no assertion failures. The native drive finds equal
world state but different continuation bytes: opening the restored backpack
after the last clock step leaves three pending Blur events. A temporary decode
shows every other saved field equal. Moving the UI setup before the shared
continuation fixes that comparison. Comparing hosts then finds one pending
browser Blur in the earlier growing-plot checkpoint; the fixture now advances
an equal 100 ms on every host after its HUD checks and before that save. The
input is consumed through normal simulation, never removed from the save.
`game/README.md` now explains why a matching world hash alone cannot establish
matching saves. Evidence: `/tmp/exact2-compost-{save,growing}-diff.json`.

A separate, fixed 64-decision Jev scenario uses only the visible controls and
care text to compost a harvest, feed another crop and harvest its Fed fruit.
The single web/macOS pair succeeds in 18/13 decisions and 12.135/8.898 s,
including launch work. Each chooses Compost and Feed exactly once. Neither
waters; both buy two seeds, leaving web with an extra normal carrot and native
with a spare seed. Their Fed carrots are worth 39/47 coins; the runs are
stochastic and do not establish efficiency, a success rate or visual perception.
Median/p95 decision latency is 327/702 ms on web and 399/595 ms on macOS;
input/output tokens are 14,149/834 and 9,937/559. Both final screenshots were
inspected and both descendant audits leave no recorded children. Artifacts:
`artifacts/jev-compost-{web,macos}/`. This feedback batch is closed without
retuning; earlier market policies remain unchanged.

Final candidate drives have no failures in 81.840/41.861 s on web/macOS.
Both agree on source inputs, candidate pins, eleven final world samples,
published values and fifteen saves; all process audits pass. Artifacts:
`artifacts/compost-settled-{web,macos}/`. Both feeding and harvested-fruit
captures were inspected. The existing pins are still unchanged pending the
strict collector. Root checks all pass in 86.287 s: build 0.407, tests
83.400 (2386 passed in 81 binaries, 9 ignored), Clippy 0.302, formatting
2.074, caps 0.088 and boot 0.015 s. Binary-reported test execution sums to
39.87 s; the local timing target is missed again, with no game proof
running concurrently. Logs: `/tmp/exact2-compost-root-*.log`.

Strict acceptance in `artifacts/prove/run-casLNI/` succeeds at feature commit
`48d883bdc`. Linux Off/Save/FreshGame take 20.797/6.180/6.153 s; web takes
105.926/102.780/92.845 s; release Linux takes 53.271 s including its build.
All seven runs agree with the independent macOS candidate on source inputs,
pins, ten comparable state observations and all fifteen save files. The
close-time candidate has one additional offline-session sample; reconstructing
its ordinary observations from the transcript gives the same ten as the
collector. Every descendant audit passes with no recorded children left.
Only the collector updates `pins.json`, for the saved food/fed fields and the
new compost continuation `a95c5aec8548b855…`. An ordinary Linux proof then
passes the accepted pins in 2.242 s (`artifacts/compost-accepted-linux/`).
Comparison evidence: `/tmp/exact2-compost-strict-comparison.json`.

A read-only fetch after acceptance work began finds main through `b83acf98a`,
ten commits beyond the previous integration, chiefly grouped-list cardless
sections and reviewed sound/storage designs. Those fetched changes are not
in this feature's accepted source; integrate them at the next work boundary.

## Pending input is visible at the save boundary (2026-10-04)

Merge `8a0bd86c9` integrates the ten fetched main commits through `b83acf98a`
at the next clean boundary. Formatting also normalizes the two grouped-list
Rust files brought by that merge; their behavior is unchanged by formatting.

The compost continuation's earlier debugging needed a private EXSIM decode to
find three native Blur events. World `state` now reports `input.pending`, a
fixed seven-number summary: total and the six input-event kinds. It counts the
current retained queue after coalescing/refusals; it copies no event payload,
consumes no input and adds no historical trace or operation. The existing game
and engine guides explain that counts can diagnose this difference but cannot
establish equal saves: payloads, order and timestamps still matter. The matching
QUEUE entry is removed.

The engine's 408 tests pass in 29 binaries, with seven ignored. New regression
coverage distinguishes three queued Blurs at an unchanged world hash, verifies
save bytes do not change during inspection, preserves the counts across restore
and a different host epoch, and drains them through ordinary simulation. A mixed
queue covers key-repeat rejection, pointer/wheel coalescing, controls and 64 KiB
messages; the full-queue test reports 1,024 retained messages alongside 76 refused
posts. Strict all-target Clippy and formatting pass. The first compile used
`usize`, which the portable Data format deliberately does not support; the
bounded counts now use `u32`. Logs: `/tmp/exact2-pending-engine-*.log`.

All root checks pass in 192.375 s, with no game proof running concurrently:
build 37.308, tests 128.534, Clippy 24.340, formatting 2.089, caps 0.087 and
boot 0.015 s. The 81 test binaries pass 2,388 cases with nine ignored and
report 39.83 s of execution. The 60 s local target is missed again; this
integration does not claim to fix the previously recorded launch variance.
Logs: `/tmp/exact2-pending-root-*.log`.

The first Garden web/macOS drives fail only the two new assertions: the proof
read the host's `world` array as one object. Their actual state replies already
show the feature working. Immediately after Compost, web reports one message;
macOS reports one message and six Blur events. After the shared 100 ms step,
both report zero. Both runs retain all accepted pins, eleven final world samples
and all fifteen saved files byte-for-byte against `compost-settled-{web,macos}`;
all children exit and both Fed-harvest captures were inspected. The proof now
finds the named world in untargeted state before checking its queue. The engine
implementation is unchanged by this assertion fix. Initial artifacts:
`artifacts/pending-{web,macos}/` (99.264/57.463 s including build and drive).

The corrected Garden proofs pass on web/macOS in 85.457/41.987 s. Their
message/Blur counts reproduce the earlier observation and become zero at the
next tick. Source inputs, pins, eleven final world observations, published
values and fifteen saves agree across hosts; pins, worlds and saves also match
`compost-settled-{web,macos}`. Both final feeding captures were inspected and
both process audits leave no recorded children. Artifacts:
`artifacts/pending-checked-{web,macos}/`. Forest and Rivals also pass their full
web/macOS sweeps (diaries 005/006); the three games' fifty distinct saved files
are unchanged on both hosts. Comparison: `/tmp/exact2-pending-game-comparison.json`.
No pins change and the completed Jev batches stay closed.

A periodic read-only fetch during the sweep finds main at `d2cb661eb`, another
72 commits (312 files), including host input/focus and driver changes. Those
changes are outside this verified checkpoint; integrate at the next clean
boundary. Its DEFERRED change admits local notifications, unrelated to this
queue inspection; RULES is unchanged.

## Main's input/focus batch (2026-10-04)

Merge `b32b0eec6` integrates main through `d2cb661eb`; Rivals diary 006
records the tooling, root and macOS-host checks and the stale clipping-fixture
repair. Garden passes complete web/macOS proofs in 134.433/66.167 s including
builds. Source inputs, pins, eleven final world observations, published values
and fifteen saves agree across hosts; worlds, pins and saves retain the prior
`pending-checked` results. The Compost boundary still reports one message on
web and that message plus six Blurs on macOS, then zero on both after the next
normal tick. Both feeding captures were inspected. Artifacts:
`artifacts/main-input-{web,macos}/`. Gameplay and the closed Jev policies are
unchanged.

## A visible art pass (2026-10-04)

Charlie asked whether the games had become more fun and pointed out that the
art still looked much the same. The honest distinction: today's care,
compost and progression work added mechanics, but there was no substantial
visual upgrade and no demonstrated improvement in enjoyment. The existing
world was a flat brown grid, green cylinders and a capsule player. A fixed
before capture (`artifacts/art-before-web/`) closes the shop, grows one
hundred plants and walks east; its actual screenshot is the comparison.

The first art pass replaces the capsule with a straw-hatted gardener, gives
plants leaves and woody crowns, gives carrots/strawberries/bananas/pumpkins
different silhouettes, and surrounds the pad with meadow, an orchard, a
picket fence and flower banks. One generated mesh per crop keeps full fields
instanced. Scenery adds five constant entities; it does not grow one entity
per plot. The far fence follows expansion beyond the walking boundary.
Warm light and ambient occlusion ground the models. Gameplay timings and
prices are unchanged. The extra models and entities require a new deterministic
baseline; fruit rolls use the world's RNG, not entity-seeded randomness.

The first logic suite passes, but the first browser drive renders an empty
surface. Reading the real host log finds the exact cause: the generated GPU
shell is still primitive-only, and binding `gardener.model` refuses with
“this module has no model support; declare game.assets”. Garden's small
app manifest now opts into the existing model-capable module. There is no
core feature or new executor. This is a useful authoring lesson: passing the
logic suite cannot prove a game's selected GPU module can present its models.
The ordinary HUD had empty defaults and the clock had no pending work; the
actual diagnosis required host logs. No new inspection operation is needed.

The corrected second browser capture passes its harvest/movement drive in
28.803 s, with no recorded children remaining. Both arrival and grown images
were inspected: the character, leaves, fruit, flowers, orchard and fence are
visible. Its mist and fill light were too strong, washing out the palette;
the next fixed version reduces them and scatters the flower spacing. Artifacts:
`artifacts/art-second-web/`; logs: `/tmp/exact2-garden-art-{first,second}-web.log`.
The first failed drive and diagnosis remain in `art-first-web/` and
`art-diagnose-web/`. This is an in-progress art result, not a fun or accepted
parity claim.

The final art version passes the complete candidate drive on web/macOS in
85.644/51.427 s including builds. Inputs, candidate pins, eleven final world
observations and published values, and all fifteen saved files agree; neither
host leaves a recorded child. These runs are explicitly UNVERIFIED while the
accepted baseline still predates the art. Artifacts: `artifacts/art-final-web/`
and `artifacts/art-final-macos/`; comparison:
`/tmp/exact2-garden-art-comparison.json`. The matching grown-field browser
capture (`artifacts/art-final-view-web/grown.png`) and native ripe-crop capture
were inspected: leaves, fruit silhouettes, hat, flowers and fence now read
clearly, with stronger color than the second version. The extra browser
capture's movement/harvest check passes in 11.311 s with no recorded children.
This changes the scene's appearance, not its still-basic HUD or character
animation. It is not evidence of increased enjoyment.

The Garden suite passes 29 enabled tests (three unit, 26 simulation), with five
scale/render tests ignored; strict Clippy passes. The model-capable GPU module
is 1,257 KiB / 519 KiB gzip, against 1,034 / 437 KiB for the primitive module.
The application wasm is 955 / 431 KiB. Those extra bytes and the ambient
occlusion pass are costs of the art; no performance improvement is claimed.

Periodic merge `6c40122fd` brings main through `536a054e1`: web inspection of
live input values, storage-refusal precedence, and stable native smoke reload.
The five root checks pass in 49.510 s: build 0.300, tests 46.784 (2,436 passed,
nine ignored in 81 binaries), Clippy 0.292, format 2.034, caps 0.085, boot 0.015.
The web inspection/storage regression files pass 92 tests and 2,918 assertions
in 8.67 s. Logs: `/tmp/exact2-art-main-*.log`. Garden's final candidate drives
above include this merge. Source is frozen while the strict seven-mode baseline
runs, avoiding the source-drift mistake recorded in Rivals diary 006.

The strict baseline is now accepted at `6c40122fd`, with input digest
`d9be9459e5ab3ddf4b886368175e2327a6aba2716db2d69856010be1fddc8f40`.
All seven required drives agree: Linux Off/Save/FreshGame in
22.169/6.207/7.050 s, web in 74.977/105.542/97.494 s, and native release
in 54.613 s including its build. Restoring the ordinary web build takes
20.967 s. Every drive has zero failures and an available process audit with
zero remaining children. Artifacts: `artifacts/prove/run-7jUDIS/`; log:
`/tmp/exact2-garden-art-repin.log`. A direct comparison confirms that both
earlier full web/macOS candidate runs have these exact accepted pins and
source inputs, so those playthroughs were not repeated just to change a status
label. Summary: `/tmp/exact2-garden-art-repin-summary.json`.

The old ignored render benchmark initially refuses `gardener.model` too:
its direct `WorldSurface<Garden>` still selected the primitive renderer.
The fixture now selects `ModelPresentation, true`, matching the shipped
module; no game or engine source changes. Its last heading also names the
actual `culled` field instead of incorrectly calling it GPU passes. The
corrected release run measures 180 frames at 1280 × 720, overview camera,
including offscreen readback, after each field grows for 150 seconds:

| Plants | Entities | Mean / p95 / max ms per frame | Draws | Triangles |
|---:|---:|---:|---:|---:|
| 100 | 239 | 1.7 / 2.3 / 3.1 | 115 | 46,645 |
| 1,000 | 2,239 | 1.7 / 1.9 / 2.3 | 115 | 351,845 |

Logs: `/tmp/exact2-garden-art-render{,-fixed}.log`. This bounded native
measurement shows the new scene stays inexpensive at these sizes; it is not
a before/after speedup, a browser FPS measurement, or a 50,000-plant claim.
The remaining visible weaknesses are the static character, basic HUD and
limited feedback when an action succeeds. Improving those is a better next
gameplay presentation task than adding another resource mechanic. Fun remains
unproven by these correctness and rendering measurements.

## Playtest-loop checkpoint (2026-10-04)

Charlie asked how to construct a loop that makes the games more fun. The next
Garden experiment should compare the first two minutes before and after a
focused action-feedback pass, with the same starting conditions and unchanged
playtest policy. Jev can expose confusion and repeated ineffective choices;
accelerated clock steps and scripted aiming cannot establish real-time feel
or human enjoyment. Actual captures and occasional human A/B play are separate
evidence. Keep, rework or revert based on the claimed experience, and record
uncertainty instead of treating another mechanic or a green proof as fun.
No new playtest framework or game change was implemented in this checkpoint.

Merge `f95a568d9` integrates main through `173a73afe` (six scripts/docs commits).
Both sides' Queue additions are retained. A few existing driver comments are
shortened to keep the merged source under the 1,500-line cap. The root gate
runs in 45.143 s: all checks pass except one documentation-example test
(2,435 pass, one fails, nine ignored in 81 binaries). Main's task snippet is
intentionally partial but fenced `contract`; changing its fence to `text`
passes that exact existing test. Its isolated Cargo invocation recompiles a
different feature selection in 27.63 s; the test itself takes 0.02 s. The web
agent tests pass 63 cases and 2,512 assertions in 5.41 s. Logs:
`/tmp/exact2-fun-loop-{main-*,docs-recheck,agent-tests}.log`.

## First action-feedback comparison (2026-10-04)

Hypothesis: seeing the gardener perform a successful care action and collect
its reward will make planting, watering and harvesting more tangible. This is
one presentation experiment, with unchanged crop times, prices and controls.
The earlier first-two-minutes proposal begins with a reproducible 19-second
slice: plant the starting carrot, water it, harvest it, then walk east. It
does not yet test two minutes of freely chosen play or establish enjoyment.

The saved gardener now turns, walks with articulated arms and legs, bends to
plant/feed, raises a blue watering can, and collects the actual crop's fruit
in a short arc into a satchel. Three fixed emitters provide dirt, water and
reward bursts, with short synthesized sounds. The character and effects add
ten fixed entities; effects replace bounded emitter state and draw no crop
or weather random numbers. Gestures use the simulation clock and their state
is saved. Successful actions trigger them; refused watering does not.

The before capture is `artifacts/feel-before-web/`. Three visual rounds fix
an obscured can, fruit passing behind the hat, and an upward-pointing spout.
Final captures are `artifacts/feel-final-{web,macos}/`, including planting,
watering, harvest and walking APNGs. Their runs take 35.593/17.676 s including
builds, with zero failures and no recorded children remaining. The actual
web and Mac harvest frames and web watering frame were inspected. Effects
remain small in the overview; this pass is not a complete art-direction or
HUD redesign. At the three identical planted/watered/harvested checkpoints,
HUD text, the purse/seed/bag/care/order fields, Census, Shop, Weather and
GardenClock match the before run exactly on both hosts. The first carrot
still weighs 0.20338216 and pays 12 cents. Comparison:
`/tmp/exact2-garden-feel-final-comparison.json`.

Two authoring mistakes surfaced. Adding `game.audio` requires updating the
game's lockfile before `--test`: `shells.mjs --update-lock --test` enters the
test branch without updating it, so the two commands must be separate. Also,
the game must call `audio::step` after game logic. Omitting it left expired
voices in saved state; the fixed tick removes them, and the existing proof
now checks that finished care sounds leave no voices. It also checks a live
can/droplets, successful sound cues, refused-water gesture continuity and
the visible picked fruit. Existing continuation saves exercise effects in
flight. The 29 enabled Garden logic tests pass (five measurements ignored),
as do strict Clippy and formatting.

A live-clock browser probe uses trusted Play/key input and measures the
actual WebAudio graph. The first attempt had a running context and plant PCM
but zero measured output; that result is retained as
`artifacts/feel-live-web/audio-first.json`. After the lifecycle correction,
the final probe measures nonzero RMS for plant/water/harvest
(0.0807/0.0275/0.0724), with the context running and its owned browser closed.
This proves signals reach the browser output graph, not that the sounds were
heard or judged pleasant; the initial zero's cause was not isolated.
The optional GPU module is 1,398 KiB / 571 KiB gzip, up from 1,257 / 519;
the application wasm remains 955 / 431 KiB. Source changes are game-local.

Jev's existing text-only policy cannot judge these gestures or sounds, so
rerunning it would not measure this hypothesis. Next human comparison: play
both versions in alternating order, ask which care/harvest is more satisfying
and what felt unclear, and observe whether the player chooses to plant again.
Record the reasons and a keep/rework/revert decision. Automated outcome and
save checks establish a stable comparison, not increased fun.

The full candidate proofs pass on web/macOS in 73.278/43.918 s. Source
inputs, all candidate pins, eleven final world observations and published
values, and fifteen saved files agree. Both process audits are available and
leave no recorded children. Artifacts: `artifacts/feel-checked-{web,macos}/`;
comparison: `/tmp/exact2-garden-feel-proof-comparison.json`. These runs remain
UNVERIFIED until the strict baseline accepts the changed saved representation.

The five root checks pass in 54.852 s with no game drive running beside
them (build 0.265, tests 52.080, Clippy 0.249, formatting 2.151, caps 0.092,
boot 0.015 s). All 81 test binaries pass 2,436 cases, with nine ignored.
Logs: `/tmp/exact2-garden-feel-root-*.log`. A periodic fetch finds nine new
main commits through `e55e2c27b`; they stay outside this frozen comparison
and can be integrated after accepting its baseline.

The strict baseline is accepted at `9731da5b5`, with input digest
`383249d067c257c22f098b3f285b0eb4ba78b1dad1d928f50b393f11b890c0da`.
All seven drives agree: Linux Off/Save/FreshGame in 6.360/7.321/7.576 s,
web in 118.666/123.463/118.571 s, and native release in 29.372 s including
its build. Restoring the ordinary web bundle takes 22.597 s. Every drive has
zero failures and an available process audit with no remaining children.
Both earlier full web/Mac candidate drives match these accepted inputs and
pins exactly, so they were not repeated just to change their status label.
Artifacts: `artifacts/prove/run-d0nirU/`; summary:
`/tmp/exact2-garden-feel-repin-summary.json`.

After the proof finishes, the existing offscreen release benchmark renders
180 frames at 1280 × 720 for 100/1,000 grown plants: mean 2.0/2.1 ms,
p95 2.4/2.6, max 2.8/2.9. There are 249/2,249 entities, 123 draws and
46,669/351,869 triangles. The prior art-only run had means of 1.7/1.7 ms
and 115 draws; these separate bounded runs show added presentation cost,
not a controlled speed regression estimate or a browser FPS result.
Log: `/tmp/exact2-garden-feel-render.log`. This candidate is mechanically
verified and visually inspected; no human preference result has been collected.

## The old art pass as a fourth look (2026-10-05)

Branch `art/garden` (2026-10-03) drew the garden from baked glTF on a much
older engine and garden. It is now `art: "pass"`, a fourth look beside
classic, golden and storybook (Garden panel → Look → Art pass), redone on
today's engine. Play is unchanged: `every_look_plays_the_same_garden`
includes it, and the classic look is byte-identical: its Linux proof reports
the same tick hashes (0 `0xd436feb13a9f57a4`, 5136 `0xdddce38bb7a444bf`)
and the same five save digests before and after this change. Those still
differ from `pins.json`, which predates main's new built-in components.

- **`art.mjs` (794 lines)** keeps the old mesh kit and writes binary glTF
  (218 files, 3.5 MB, 0.14 s, byte-identical on rerun) plus seven shared PNG
  textures in `art/textures/` (soil, wood, bark, cloth, straw, stone, grass).
  Each texture bakes once by name, so soil mounds and bark trunks are now
  textured where the old pass used vertex-colour noise. Plants are 14 crops
  × 5 stages, each with a `-far` level (mature plants 1,043 → 180 triangles
  on average); fruit is 14 shapes ripe and unripe, each with a far level;
  then the fence, a lantern, stall, barrel, can, path stones, tufts, a turf
  ground, two grass patches, and a farmer and keeper as body/arm/leg parts.
- **`logic/src/pass.rs` (953 lines)**: setup, the fence rebuilt as the
  garden grows (with a gate by the barrel facing the stall and inward
  lanterns), the day and night, weather, the close-up camera and `present`.
  `garden.rs` asks `pass::on` for a model per stage, a ground-standing pose
  at `plant_center` and fruit offsets that match the models. The farmer is
  the shared gesture rig under the classic names, so watering, planting and
  harvest gestures work unchanged.

What the current engine replaced:

| Old workaround | Now |
|---|---|
| 112 fruit models, one per mutation look | 14 shapes (+ unripe); mutations are `MaterialOverrides` on material 0 written by `Game::present` from the saved `muts`. Rainbow cycles hue, Gold glints, Shocked flickers (`p.rng`) |
| a far-off speck per model to keep 202 models resident (~1,800 empty draws) | `Game::STREAMED`; resident once loaded |
| no LOD | `ModelLod` per plant and fruit (28 m / 18 m); grass patches `hide: 80 m` |
| textures embedded per model | shared `art/textures/` PNGs, content-named |
| lantern lit/dark meshes swapped by a saved `Lantern` component | one lantern model, glass glow from `present`; constant point lights the sun drowns by day |
| keeper sway and wave in saved `Limb`/`Gait` components | `Offset` from `present` |
| rain/snow sphere moving with its particles | `Shape::Box` + `WorldSpace` + `ParticleLook.stretch` streaks |
| — | plants between a near camera and the farmer fade (`Opacity` from `present`, visiting only the tiles under the sight line) |

What stays in the simulation, and why: a plant's model per stage is a `Mesh`
swap at the stage event (present cannot change meshes); the sun, moon, sky,
fog and exposure are `DirectionalLight`s and the `Environment` resource,
which present cannot write (presentation resources are not supported); the
weather emitter. Sun, moon and `Environment` are ambient so they never hold
the clock awake. The farmer's walk swing stays the shared, saved gesture
rig: moving it to present would move the classic look's pins. Dropped: the
HUD fruit swatches (a new `ShopRow` field would change the classic look's
saved publications), and no wind sway (a `game.render` crate would also
load for every look).

Cost. The web dist grows from 3.8 MB to 11 MB; `assets/` is 6.1 MB (1.5 MB
gzipped), of which a device fetches the models and one texture family.
Overview on the web at 100 / 10,000 plants: classic 123 / 155 draws, 46,669 /
5.34 M submitted triangles; the pass 755 / 835 draws, 0.34 M / 20.9 M
(submitted counts every LOD level; the camera draws far levels). `STREAMED`
is static per game, so the classic look fetches the pass's models too. It
never waits for them, but on a local server they arrive before its first
frame and are prepared inside it: classic's first submitted frame moved from
169–410 ms to 494–503 ms over three runs each, and its ready boundary now
holds 671 mesh uploads and 45 pipelines instead of 37 and 35. The other
choice, undeclared models loaded on sight, costs classic nothing but brings
back the pop-in and save refusals the speck workaround hid.

Verified: `bun game/app/shells.mjs ./game/games/garden --test` (29 sim tests,
including the art pass's fruit placement, presented mutations surviving a
restore, and a paranoid Save run); `proof.mjs linux` as above; screenshots of
day, night, rain, close-up, overview and 10,000 plants on the web.
