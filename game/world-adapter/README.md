# Device-free world surface

`WorldSurface<G: exact_world::Game>: exact_gpu::Surface` owns a `Sim<G>` without
presentation. `module!(G)` registers it as `world`, with typed argument defaults;
`game.world: true` selects that registration in generated app shells. Contract
text, controls and publications work without a GPU device. Tally is an example.

Use `?` in `Game::setup` and `Game::tick`, both returning `Result<(), DataError>`.
Binding constructs the simulation and takes its first fixed tick synchronously.
A returned tick error stops driving, withholds partial publications and is returned
once by `take_error()`. `state.world` retains `failed: true`, the string `error`
and `ready: false`; state/tree/log inspection remains available. Clock requests
refuse until a successful restore or setup-argument restart clears the failure. Diagnostics beyond 4,096
UTF-8 bytes are explicitly truncated. A panic can abort an entire wasm module;
never unwrap kernel errors in gameplay callbacks.

Lifecycle, keys, named button/scalar-axis controls, clock ownership, messages,
publications and exact save/restore use the ordinary Surface API.
`sim(&self) -> Option<&Sim<G>>` gives read-only access. Rendering, device callbacks
and child composition use Surface defaults. Web `gpu_load_headless()` and
`gpu_create_headless(name)` establish ownership after instantiation.
`Surface::advance(&mut self, now_ms: f64) -> bool` drives time without an agent
request or observation; `gpu_advance(id, now_ms) -> bool` exports it on the
ownership-only web and native headless ABIs. Live worlds join every frame.
`module!(G, device)` uses the original device-first lifecycle and waits for WebGPU.
A mixed app containing a device-free world and a plain shader canvas has a device
module and waits for its device.
The default web registration has no device or asset storage; asset delivery refuses.
Native registration uses the existing Linux ABI.

## Bounds

Agent `state`, `tree`, `clock` and `logs` use the kernel's bounded inspection writer.
Unsupported operations refuse. The shared JSON reader accepts a flat object:
16 KiB, 64 scalar fields, no duplicate keys, nested containers or trailing data.
Parsing is O(bytes + fields²), with at most 2,080 prior-name comparisons (including a refused 65th field).

Entity listings return at most 512 rows and mark truncation. Inspection admits
65,536 bytes/visits. Clock requests admit 216,000 ticks; settle admits 3,600;
input queues admit 1,024 events. Kernel admission bounds apply independently.
Both owners admit 256 surfaces and refuse ID exhaustion. The owned web ABI
admits 64 KiB returned text, 1,024 messages / 64 KiB buffered message text,
and 256 MiB checkpoint bytes before copying into JavaScript.
`publish_record(&World, &impl Data) -> Result<(), DataError>` refuses traversal,
range, shape and batch errors atomically. Adapter runtime code and `web_owned`
contain no explicit panic/unwrap/assert path; arbitrary user callbacks, allocation
failure and kernel-internal assertions retain their Rust behavior. No wasm panic
is caught or described as recoverable. Game code must bound its
own work: Tally has 15 entities, and each card traversal visits at most twelve
cards (a thirteenth child explicitly refuses). Hand reads use the ownership index,
not a scan of all cards. Its heartbeat deliberately exhausts the settle budget.

## Checkpoints

`carry(&mut self) -> Result<Option<Vec<u8>>, SurfaceError>` returns exactly
`Sim::save()`; `restore(&mut self, &[u8], Restore) -> Result<(), String>` passes
those bytes directly to the Sim. `Sim::clock_ms()` is the sole saved caller clock;
the adapter keeps only the host epoch. There is no version wrapper or duplicated
clock. Old surface envelopes refuse; queued input stays in the Sim checkpoint.
`Restore::Open` is exact; `Restore::Carry` uses Sim carry semantics. The host log transport returns `{from,next,lines,reset,truncated}`; numeric cursor
indices address a ring of at most 512 string lines / 48 KiB encoded text per
surface, outside saves. A read consumes at most 512 kernel events / 64 KiB;
lagging readers receive the retained suffix with `truncated: true`.

## Measure

Use the pinned Bun from `package.json`, Binaryen 131 and wasm-bindgen 0.2.127.
Set `EXACT_UPDATE_TRUST=development`, `CARGO_TARGET_DIR` to one shared cache,
`CHROME` to Chromium and `K4_SCRATCH` to the diagnostic output directory.
Check `df -h ~` before cold builds; stop below 25 GiB free.

```sh
EXACT_APP_DIR=$PWD/game/games/tally EXACT_WEB_DIST=$PWD/game/games/tally/dist \
  bun host/web/build.mjs
bun game/games/tally/proof.mjs linux --build-only
bun game/world-adapter/measure.mjs --sizes --linux --web --compare-preload
```

The Linux comparator also needs `caltrain-linux` with the matched `gpu-dev`
settings in the [diary](../diaries/2026-09-20-device-free.md#k4-2026-09-20).
`EXACT_WORLD_TIMING=1` enables native spans; the measurement command sets it.
Web markers come from `state.world[].perf` through the normal agent carrier.
Ten fresh profiles and ten reloads per policy use identical modules; only the
preload links vary. The no-store server transfers full resources on warm reloads.
Each sampled navigation must show exactly one resource transfer per module.

## Current measurements (K4)

Binaryen 131 `-Oz`; gzip level 9, normalized to K3's Bun 1.3.14 compressor:

| Device-free module | Raw B | Gzip B |
|---|---:|---:|
| K3 `gpu_bg.wasm` | 366,615 | 152,634 |
| K4 `gpu_bg.wasm` | 366,077 | 152,719 |
| K4 `app.wasm` | 661,221 | 278,846 |
| K4 `gpu.js` | 15,918 | 3,697 |

The pinned Bun 1.3.12 compressor reports 153,138 / 280,487 / 3,664 gzip bytes
for those three K4 files respectively. Raw bytes are identical. Construction,
first tick and restore retain 91 / 6 / 159 allocations and 15,459 / 672 / 33,432
requested bytes; the sampled Sim save is 3,623 bytes.

| Chromium ms, median / p95 | Serial cold | Preload cold | Serial warm | Preload warm |
|---|---:|---:|---:|---:|
| FCP | 128 / 216 | 122 / 148 | 104 / 116 | 100 / 120 |
| Module instantiated | 188.35 / 246.9 | 125.7 / 157.6 | 156.95 / 166.7 | 114.35 / 131.8 |
| Bound / first tick | 199.90 / 258.5 | 130.50 / 167.0 | 157.55 / 167.3 | 114.95 / 132.4 |
| Publication accepted | 207.35 / 265.5 | 133.60 / 173.4 | 158.95 / 168.8 | 116.15 / 133.4 |

| Linux, 20 launches, ms from host entry | Median / p95 |
|---|---:|
| Plan decoded | 0.125 / 0.142 |
| Fonts ready | 6.924 / 6.975 |
| Painter ready | 7.706 / 7.785 |
| First host frame | 13.154 / 13.304 |
| Module verified | 24.357 / 24.606 |
| Module dlopen complete | 24.584 / 24.832 |
| Bind work / first tick work | 0.056 / 0.060; 0.006 / 0.007 |
| First tick complete | 24.780 / 25.044 |
| Publication accepted | 24.893 / 25.175 |
| Create + bind + first tick, including ABI / trace | 0.161 / 0.174 |
| Caltrain first frame, assets loaded | 36.376 / 37.038 |

These browser measurements require no GPU; device attachment, shaders and pixels
still need real WebGPU Chrome. API limits are above; experiment decisions, exact
pin changes, the plain-surface audit and validation results are in the diary.

World shells use `exact-app-shell` for the Contract bake and this adapter for
argument declarations. Neither the four generated Tally shells nor the adapter's
tests depend on `exact-game`, `exact-game-app` or `exact-game-render`.
`game.world: true` with `game.audio: true` or `game.assets: true` refuses during
manifest resolution; these require a device game module.
