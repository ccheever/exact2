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
