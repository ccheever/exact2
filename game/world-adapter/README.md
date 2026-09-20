# Device-free world surface

`WorldSurface<G: exact_world::Game>: exact_gpu::Surface` owns a `Sim<G>`.
`module!(Game)` registers it as `world`, with typed argument defaults. An app
selects it with `game.world: true`. Tally is the nonspatial consumer: ordinary
Contract text and action buttons, with a surface carrier but no canvas drawing.

Binding constructs and takes one fixed tick immediately. Ready means bound and
at least one tick taken. Lifecycle, keys, named button controls, scalar axis
controls, clock ownership, messages, publications and exact save/restore work
without presentation. `render`, device callbacks and child composition use
Surface defaults. `sim(&self) -> Option<&Sim<G>>` exposes read-only simulation.

Agent JSON transport lives here. `state`, `tree`, `clock` and `logs` use the
kernel's bounded inspection writer; unsupported operations return an error.
Requests admit 16 KiB, entity listings 512 rows, output 65,536 bytes/visits,
clock requests 216,000 ticks, settle 3,600 ticks and input queues 1,024 events.
Kernel admission bounds still apply to each operation. Tally has exactly 15
entities and at most twelve card visits per game operation; its heartbeat makes
settle exhaust its explicit budget. The proof's `clock +1000` publication
assertion is a negative control against a frozen or absent world.

The kernel has no caller-clock getter. The adapter checkpoint contains a version,
eight bytes of caller time and the opaque Sim save. Host time is rebased on open;
queued input remains in the Sim checkpoint. No kernel files are changed.

Measured on this Linux box: Tally construction 91 allocations / 15,459 requested
bytes; first tick 6 / 672; exact restore 159 / 33,432, for a 3,623-byte Sim save.
The test reuses the kernel counting allocator source without adding unsafe code.
Artifact and host startup measurements are recorded below.

## K2 receipt (2026-09-20)

Web exports `gpu_load_headless()` and `gpu_create_headless(name)` establish
ownership immediately after wasm instantiation. Full registrations additionally
export `gpu_attach(id, canvas, width, height) -> bool`, which adds presentation to
the existing ID without constructing, binding or restoring a replacement world.
WebGPU acquisition and shaders run in the background. A rendered world continues
ticking without a device and keeps its existing `no device` readiness reason.
The headless clock does not fabricate a submitted GPU frame.

`module!(G)` uses the ownership-only web ABI; `module!(G, device)` retains the
full device ABI for measurement or explicit app-shell use. The separate
`exact_gpu::web_owned` owner has no Gpu, texture or device storage. It shares named
argument decoding with Module. It admits 256 concurrent surfaces, refuses ID
exhaustion, and bounds transport to 16 KiB / depth 64 before parsing. Asset
delivery explicitly refuses: this registration is for presentation-less,
asset-less surfaces. The native registration retains the existing Linux ABI.

The measured device cost is **52,914 gzip bytes including JS**, below the proposed
150 KB threshold. The lean registration is still the default: these games cannot
use the extra exports, and omitting them also avoids an unnecessary adapter
request. No Cargo feature, dependency pin or other game's fixture changed.

Bytes, using the ordinary `host/web/build.mjs`, Binaryen **131** `wasm-opt -Oz`
and gzip level 9:

| Artifact | Full ABI raw | Full ABI gzip | Device-free raw | Device-free gzip |
| --- | ---: | ---: | ---: | ---: |
| app.wasm | 661,197 | 278,837 | 661,197 | 278,837 |
| gpu_bg.wasm | 497,482 | 210,088 | 391,681 | 164,240 |
| gpu.js | 57,578 | 10,763 | 15,918 | 3,697 |

Binaryen 108 produced a wasm-bindgen externref table that could not grow during
initialization; those results were discarded. Version 131 artifacts passed the
actual browser proofs. The optimizer was installed in task scratch, with no
repository tool/dependency pin changes.

Linux, 20 independent process launches, `gpu-dev`, Instant measured from the
start of the Rust host `run` entry (OS exec/dynamic-loader time excluded):

| Event | Median ms | p95 ms |
| --- | ---: | ---: |
| First tick completed | 24.739 | 25.461 |
| First publication accepted by Contract | 24.898 | 25.638 |

`EXACT_WORLD_TIMING=1` enables the opt-in host trace. The first-tick timestamp is
captured immediately after binding, then verified with state; the verification
cost is therefore excluded from that timestamp and included before publication.

Browser navigation medians are **unverified**. The required 10 cold / 10 warm
runs did not complete: repeated Chromium CDP `Runtime.evaluate` and
`Target.createTarget` timeouts exhausted three harness repair rounds. No partial
samples are presented as medians. This applies to FCP, module instantiation,
bind, first tick and the first visible publication, both before and after.
Before-change activation has no device-free ownership path; the rejected and
never-resolving-device fixtures verify the new path explicitly. Timing markers
are available as `state.world[].perf.moduleInstantiatedMs`, `boundMs`,
`firstTickMs` and `firstPublicationMs`. The last marker records the Contract
commit; the browser diagnostic separately observes the visible Tick text.
The existing two-rAF lazy-load delay remains because FCP parity was not proven.

Reproduce with `bun game/world-adapter/measure.mjs --sizes --linux --web` after
baking Tally; set `K2_SCRATCH` to a scratch directory containing `full/` from a
`module!(Tally, device)` bake, and `CHROME` to Chromium. Cold trials disable the
HTTP cache and use a fresh browser/profile; warm trials discard a priming load.
Each recorded browser process group is SIGKILLed and awaited in `finally`.

Validation: 10 adapter tests pass, including maximum queued-input pressure,
explicit admission refusals, exact queued-input continuation and the frozen-world
negative control. Tally's ordinary, Save and FreshGame proofs pass on Linux and
Chromium (six runs), agreeing on both tick pins and continuation bytes. Seven
other Linux game proofs pass; Lanterns has zero assertion failures but no pins,
so remains UNVERIFIED. The game Rust suite reports 753 passed, 18 GPU-required
failures and 25 ignored. Game Bun tests: 149 passed / one skipped. Web JS fixtures:
149 passed / three skipped / four existing Swift-source assertions failed;
Swift files were not changed. Targeted GPU/web/Linux Rust: 166 passed / three
adapter-required failures / one ignored. Whole-root build/test/clippy is blocked
by the absent lean Hermes producer. Game clippy and formatting pass.

Kernel gaps: caller time is not publicly readable, requiring the adapter envelope
above. Kernel logs expose an opaque cursor, which the adapter retains per surface.
No JSON parser or presentation assumptions were added to the kernel. On Apple,
the equivalent remaining work is to load/create ownership before requesting a
Metal device, drive headless clock requests, then attach the presentation target
to the same ID. No Apple SDK or real GPU is available here; real device attachment,
rendering and Apple behavior remain unverified beyond the web upgrade fixture.
