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
21 simulation tests and three layout tests pass; game determinism Clippy
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
