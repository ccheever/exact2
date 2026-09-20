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
refuse until a successful restore clears the failure. Diagnostics beyond 4,096
UTF-8 bytes are explicitly truncated. A panic can abort an entire wasm module;
never unwrap kernel errors in gameplay callbacks.

Lifecycle, keys, named button/scalar-axis controls, clock ownership, messages,
publications and exact save/restore use the ordinary Surface API.
`sim(&self) -> Option<&Sim<G>>` gives read-only access. Rendering, device callbacks
and child composition use Surface defaults. Web `gpu_load_headless()` and
`gpu_create_headless(name)` establish ownership after instantiation.
`module!(G, device)` additionally retains the device ABI, including
`gpu_attach(id, canvas, width, height) -> bool`; attachment preserves the instance.
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
The owner admits 256 surfaces and refuses ID exhaustion. Game code must bound its
own work: Tally has 15 entities, and each card traversal visits at most twelve
cards (a thirteenth child explicitly refuses). Hand reads use the ownership index,
not a scan of all cards. Its heartbeat deliberately exhausts the settle budget.

## Checkpoints

`carry(&mut self) -> Result<Option<Vec<u8>>, SurfaceError>` returns exactly
`Sim::save()`; `restore(&mut self, &[u8], Restore) -> Result<(), String>` passes
those bytes directly to the Sim. `Sim::clock_ms()` is the sole saved caller clock;
the adapter keeps only the host epoch. There is no version wrapper or duplicated
clock. Old surface envelopes refuse; queued input stays in the Sim checkpoint.
`Restore::Open` is exact; `Restore::Carry` uses Sim carry semantics. The opaque
log cursor is retained per surface and is not saved continuation state.

## Measure

Build Tally through `host/web/build.mjs` with `EXACT_APP_DIR=game/games/tally` and
`EXACT_WEB_DIST=game/games/tally/dist`. Build its native executable with
`bun game/games/tally/proof.mjs linux --build-only`. Then run
`bun game/world-adapter/measure.mjs --sizes --linux --web --compare-delay`.
The Caltrain comparator needs a built `caltrain-linux` (matching build command in the diary).
Set `CHROME` to Chromium, `CARGO_TARGET_DIR` to the shared build directory, and
optionally `TALLY_LINUX`, `CALTRAIN_LINUX`, or `K3_SCRATCH` for artifact locations.
`EXACT_WORLD_TIMING=1` enables native host boundaries and adapter bind/tick spans.
The web numbers come from `state.world[].perf` through the ordinary agent carrier.

Measured with Binaryen 131 `-Oz`, Bun gzip level 9:

| `gpu_bg.wasm` | Raw B | Gzip B |
|---|---:|---:|
| K2 device-free | 391,681 | 164,240 |
| Fallible-contract baseline | 393,831 | 165,446 |
| Shared scalar reader | 366,332 | 152,465 |
| Final, including bounded diagnostics | 366,615 | 152,634 |

Final `app.wasm`: 661,221 / 278,846 B raw/gzip; `gpu.js`: 15,918 / 3,697 B.
The kernel accounts for 138,658 named body bytes; `alloc` containers add 76,394.
`exact-plan` contributes 214; the second JSON parser is gone. Profiles already
use size optimization, fat LTO, one codegen unit and aborting panics.

| Chromium, ms (median / p95) | 10 cold | 10 warm reloads |
|---|---:|---:|
| FCP | 114 / 136 | 100 / 116 |
| Module instantiated | 154.1 / 195.5 | 158.2 / 165.8 |
| Bound | 159.0 / 208.0 | 158.85 / 166.5 |
| First tick | 159.0 / 208.0 | 158.85 / 166.5 |
| First publication accepted | 162.3 / 215.8 | 160.1 / 167.8 |

Cold uses fresh browser profiles; warm reloads follow one priming load. The server
sends full resources on both. Immediate loading versus two rAFs has median FCP
114/116 ms and publication 162.3/203.7 ms, so wholly device-free bakes start loading
on the first surface operation. Other registrations retain the delay.

| Linux, 20 launches, ms (median / p95) | Tally |
|---|---:|
| Plan decoded | 0.128 / 0.134 |
| First host frame | 13.114 / 13.307 |
| Module verified / dlopen complete | 24.336 / 24.881; 24.559 / 25.126 |
| First tick complete (bind returns) | 24.757 / 25.328 |
| First publication accepted | 24.865 / 25.437 |
| Create + bind + first tick, including ABI / trace | 0.165 / 0.169 |

Caltrain's same host reaches its first frame at 36.662 / 37.275 ms with assets
loaded. Tally construction allocates 91 times / 15,459 requested bytes; first tick
6 / 672; exact restore 159 / 33,432, for a 3,623-byte Sim save.
Full attribution, top 40 functions, resource transfers, host spans, reproduction
commands and validation history are in the [device-free diary](../diaries/2026-09-20-device-free.md).
