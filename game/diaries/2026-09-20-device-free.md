# Device-free worlds: K2 and K3 measurements

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
| First tick completed | 24.703 | 25.461 |
| First publication accepted by Contract | 24.863 | 25.638 |

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


## K3 (2026-09-20)

Measured after merging exactly `a5d198a` into `core/world`. No authored changes
to `world/`, `world/derive/` or `motion/`; the merge brings the kernel lane’s
fallible setup/tick contract. Tally now returns `Result<(), DataError>` and
propagates kernel errors with `?`. No pins changed.

The adapter retains a returned tick failure independently of its one-shot
`take_error()`. State reports `failed`, `error`, and `ready: false`; state/tree/logs
remain readable, clock requests refuse, and successful restore clears the failure.
Partial failed-tick publications are withheld. Diagnostics admit 4,096 UTF-8 bytes,
then explicitly mark truncation; even a returned 80 KB error does not trap the shell.
The independent `game/tests/world-failure` wasm returns an error on tick three.
Its browser proof advances the maximum 216,000-tick request, observes tick two,
reads all three inspection operations and then clicks an ordinary Contract button.
That last assertion fails if the page or wasm aborts.

The scalar reader is shared with `exact-gpu` input parsing. It admits 16,384 bytes
and 64 scalar fields; duplicate names, nested containers, malformed strings/numbers
and trailing data refuse before dispatch. Scanning costs O(bytes + fields²),
with at most 2,080 prior-name comparisons (including a refused 65th field); lookups inspect at most 64 entries.
No general JSON Value tree or hash table is constructed. Request-pressure tests
include 16 KB of nested arrays and exact boundary positive controls.

### Where the bytes go

Binaryen 131 `wasm-opt -Oz`; gzip level 9 via Bun’s `node:zlib`, consistently
with K2. Python/zlib’s compressor produces different gzip sizes on these bytes.
The original K2 stripped artifact was copied before rebuilding. Symbol attribution
uses the named **post-merge baseline**, which is 2,150 raw bytes larger than K2,
and the final artifact. Historical K2 names were not reconstructed.
The first baseline snapshot precedes the final diagnostic wire-format polish;
that small polish is included in the shared-reader step.

| Device-free `gpu_bg.wasm` step | Raw B | Gzip B | Change raw / gzip B |
|---|---:|---:|---:|
| K2 shipped | 391,681 | 164,240 | — |
| Post-merge failure-port baseline | 393,831 | 165,446 | +2,150 / +1,206 |
| Shared bounded scalar reader | 366,332 | 152,465 | −27,499 / −12,981 |
| Bounded returned diagnostics (final) | 366,615 | 152,634 | +283 / +169 |
| **Net from K2** | | | **−25,066 / −11,606** |

Final adjacent files: `app.wasm` 661,221 raw / 278,846 gzip; `gpu.js` 15,918 /
3,697. No device `Module`, wgpu device, texture or shader implementation appears
in the named wasm. The ownership-only ABI was already present in K2; it was not
counted again as a K3 saving.

`twiggy 0.8.0` was installed into task scratch. The analysis build keeps function
names through wasm-bindgen and `wasm-opt -Oz -g`; the ordinary build strips them.
The following shallow-body/data/section totals sum exactly to the shipped sizes,
excluding only names. Attribution uses the first crate in each demangled symbol;
LTO-inlined work stays with its containing function. Generic `alloc` containers
are therefore separate from the kernel that instantiates most of them. Static
strings and numeric tables are grouped as read-only data, not guessed into crates.
Per-crate gzip numbers would be misleading because compression shares a dictionary.

| Shallow raw bytes by symbol owner | Post-merge baseline | Final |
|---|---:|---:|
| exact-world | 138,688 | 138,658 |
| alloc | 81,018 | 76,394 |
| Read-only data | 44,857 | 38,810 |
| exact-world-adapter | 27,147 | 26,750 |
| core | 26,912 | 25,926 |
| GPU ABI exports | 20,302 | 16,657 |
| exact-gpu (owner / JSON) | 13,029 | 14,657 |
| serde_json | 11,062 | 0 |
| Other builtins / wasm structure | 9,167 | 9,125 |
| dlmalloc | 7,280 | 7,280 |
| Tally logic + derived Data | 6,987 | 6,987 |
| std | 2,573 | 1,396 |
| ryu | 1,581 | 1,580 |
| wasm-bindgen | 1,082 | 1,082 |
| Rust allocation glue | 990 | 990 |
| hashbrown | 833 | 0 |
| exact-plan | 214 | 214 |
| Tally registration | 109 | 109 |
| **Total shipped bytes** | **393,831** | **366,615** |

`std`/`core` formatting and panic-named functions are a **subset** of those rows:
11,774 baseline / 10,741 final raw bytes. There is no separately named `libm`
function; wasm intrinsics/builtins handle the operations retained by Tally. `ryu`
contributes 1,580 final named bytes; its constants are in read-only data. The
1,082 wasm-bindgen bytes exclude the separately listed GPU ABI exports and JS.
The 35,304-byte `Sim<Tally>::candidate` is restore validation/construction, not
startup tick work. Fifteen live entities do not remove save/restore, admission,
inspection or the derived Data machinery from this generic kernel.

| # | Largest final function bodies (types abbreviated) | Baseline B | Final B |
|---:|---|---:|---:|
| 1 | `<Sim<Tally>>::candidate` | 35304 | 35297 |
| 2 | `<Sim<Tally>>::advance_us` | 13912 | 13912 |
| 3 | `<WorldSurface<Tally> as gpu::Surface>::agent` | 11649 | 11065 |
| 4 | `<gpu::json::Parser>::value` | 7436 | 7436 |
| 5 | `<values::Published>::read_reserved` | 4929 | 4929 |
| 6 | `<Dlmalloc>::malloc` | 4869 | 4869 |
| 7 | `gpu::web_owned::bind_at` | 4184 | 4174 |
| 8 | `<Sim<Tally>>::new` | 3886 | 3886 |
| 9 | `<bin::Decoder as Reader>::field` | 3676 | 3676 |
| 10 | `gpu_input` | 7050 | 3465 |
| 11 | `<WorldSurface<Tally> as gpu::Surface>::published` | 3457 | 3457 |
| 12 | `<WorldSurface<Tally> as gpu::Surface>::bind` | 3173 | 3173 |
| 13 | `<bin::Encoder>::name` | 2821 | 2821 |
| 14 | `gpu_create_headless` | 2799 | 2794 |
| 15 | `<World>::set_parent` | 2491 | 2491 |
| 16 | `<btree::map::BTreeMap<String, f32>>::insert` | 2395 | 2395 |
| 17 | `<World>::record_change` | 2311 | 2311 |
| 18 | `<Sim<Tally>>::save` | 2294 | 2294 |
| 19 | `<btree::map::BTreeMap<Rc<str>, values::Published>>::insert` | 2250 | 2250 |
| 20 | `<Tally as sim::Game>::register` | 2190 | 2190 |
| 21 | `<World>::sample` | 2029 | 2029 |
| 22 | `<World>::write` | 2007 | 2007 |
| 23 | `<json::Encoder as Writer>::number` | 2001 | 2001 |
| 24 | `<World>::spawn_named::<&str, Tally::Owner>` | 1997 | 1997 |
| 25 | `<btree::map::entry::Entry<String, btree::set::BTreeSet<Entity>>>::or_default` | 1956 | 1956 |
| 26 | `core::slice::sort::stable::quicksort::quicksort::<(String, values::Published), <[(String, values::Published)]>::sort_by<<btree::map::BTreeMap<String, values::Published> as core:…` | 1881 | 1881 |
| 27 | `<char>::escape_debug_ext` | 1840 | 1840 |
| 28 | `<btree::map::entry::Entry<&str, Registration>>::or_insert` | 1824 | 1824 |
| 29 | `<btree::map::entry::Entry<Entity, btree::set::BTreeSet<Entity>>>::or_default` | 1794 | 1794 |
| 30 | `text::shortest::<f64, String>` | 1779 | 1778 |
| 31 | `<WorldSurface<Tally> as gpu::Surface>::input` | 1694 | 1694 |
| 32 | `<btree::map::BTreeMap<u32, btree::set_val::SetValZST>>::insert` | 1684 | 1684 |
| 33 | `<btree::map::entry::VacantEntry<&str, Box<dyn storage::Erased>>>::insert` | 1671 | 1671 |
| 34 | `<Tally::Round as Data>::read` | 1545 | 1545 |
| 35 | `<bin::Decoder as Reader>::skip` | 1539 | 1539 |
| 36 | `gpu::json::parse_fields` | — | 1522 |
| 37 | `<Sim<Tally>>::install` | 1519 | 1519 |
| 38 | `<storage::Storage<Tally::Card> as storage::Erased>::read` | 1514 | 1514 |
| 39 | `core::slice::sort::stable::drift::sort::<(String, values::Published), <[(String, values::Published)]>::sort_by<<btree::map::BTreeMap<String, values::Published> as core::iter::tr…` | 1439 | 1439 |
| 40 | `type_name::<Tally::Options>` | 1355 | 1355 |

A dash means no separate corresponding baseline symbol, not zero total cost
in its callers. Full demangled names remain in `scratch/k3/{baseline,final}/top.json`.
The removed serde Value parser alone had 11,062 directly named bytes, plus its
container, formatting and string dependencies. The shared scalar reader uses the
already retained Plan value/scalar parser, eliminating a second representation.
Serde remains a build-time declaration dependency, with no named runtime body.

Stop decision: the shell already uses `panic = "abort"`, `opt-level = "z"`, fat
LTO and one codegen unit. Dropping `exact-plan` would replace the public Surface
Value contract to save only 214 directly named bytes. A second bespoke scalar
parser or per-game ABI would duplicate working code for an estimated <5 KB gzip;
removing kernel restore/inspection functionality would violate the contract.
No feature, dependency version, generated source or kernel implementation changed.
The remaining kernel/code-generation footprint is a finding for its owner.

### Browser startup

Each cold row is one fresh Chromium 1234 browser and profile; 10 samples per
policy, interleaved to reduce order bias. Warm is 10 real reloads in the same
profile after one discarded priming load. The carrier has no cache-disable
switch. The existing server uses no-store: **all resources transfer in full even
on warm reload**, as the measured sizes below show. Warm thus means a warmed
browser/profile/compilation environment, not a cache-hit transfer claim.
The carrier stamps all five requested markers; `boundMs` and `firstTickMs` share
the synchronous bind completion boundary. Publication means the Contract commit,
not scanout. A passive 200 ms grace after carrier readiness keeps the first state
read from forcing lazy loading ahead of the automatic policy. No custom CDP
harness or separate visible-text observer is used.

| Browser marker, ms (median / p95) | Immediate cold | Immediate warm | Two-rAF cold |
|---|---:|---:|---:|
| FCP | 114.000 / 136.000 | 100.000 / 116.000 | 116.000 / 240.000 |
| Module instantiated | 154.100 / 195.500 | 158.200 / 165.800 | 183.900 / 307.600 |
| Bound | 159.000 / 208.000 | 158.850 / 166.500 | 195.850 / 319.300 |
| First tick | 159.000 / 208.000 | 158.850 / 166.500 | 195.850 / 319.300 |
| First publication accepted | 162.300 / 215.800 | 160.100 / 167.800 | 203.700 / 326.800 |

| Resource | Transfer / encoded / decoded B, each launch | Immediate cold duration ms (median / p95) | Immediate warm duration ms (median / p95) | Delayed cold duration ms (median / p95) |
|---|---:|---:|---:|---:|
| `app.wasm` | 661,521 / 661,221 / 661,221 | 55.450 / 59.100 | 57.850 / 58.300 | 33.850 / 58.800 |
| `gpu.js` | 16,218 / 15,918 / 15,918 | 4.050 / 5.100 | 4.700 / 6.200 | 4.900 / 5.900 |
| `gpu_bg.wasm` | 366,915 / 366,615 / 366,615 | 25.500 / 36.100 | 32.400 / 35.100 | 32.950 / 38.000 |

Immediate loading keeps median FCP within noise (114 versus 116 ms) and improves
median first publication by 41.4 ms. It is now selected only by bakes whose
`game.world: true` registration is wholly device-free; other apps keep two rAFs.
An earlier exploratory carrier batch also favored immediate loading: FCP 94/100
ms and publication 128/166.5 ms (immediate/delayed medians). Shared-machine timing
varies; these finite samples are not a significance claim. Nearest-rank p95 with
10 observations is the maximum.

The no-adapter case is intentional. Device-free registration does not request a
WebGPU device. Existing rejected, never-resolving and absent-device fixtures pass.
A `f418821` worktree was not built: before K2, no world is created without a device;
there can be no finite bind/tick/publication baseline for that condition.
Every carrier browser process group is SIGKILLed and its parent exit awaited;
final measurement recorded 21 browser PIDs, leaked PIDs = none. Exploratory
measurement and failure-proof browsers were likewise closed. The replaced K2
custom CDP harness was deleted, not repaired. Two completed exploratory Bun
drivers retained filesystem-reader helpers; they were terminated and awaited,
and both new entry points now close that reader explicitly.

### Linux startup

20 independent Tally launches and 20 Caltrain launches. Epoch is the Rust host’s
`run` entry; exec/OS-loader time is excluded. Both use `gpu-dev`, native dependency
optimization level 3, aborting panics, disabled overflow/debug assertions and
noncontracting float math. Root Caltrain workspace member optimization is
explicitly matched to Tally’s external host dependencies. Default host viewport,
system fonts, CPU fallback, and each app’s real asset root are used. Ready replies
and loaded assets are checked before retaining a sample.

| Linux event from Rust host entry, ms | Median | p95 |
|---|---:|---:|
| plan_ready | 0.128 | 0.134 |
| fonts_ready | 6.913 | 7.027 |
| painter_ready | 7.683 | 7.810 |
| first_frame | 13.114 | 13.307 |
| module_verified | 24.336 | 24.881 |
| module_dlopen | 24.559 | 25.126 |
| load_headless | 24.594 | 25.161 |
| create | 24.629 | 25.197 |
| bind_start | 24.642 | 25.209 |
| bind_done | 24.757 | 25.328 |
| first_tick | 24.757 | 25.328 |
| first_publication | 24.865 | 25.437 |
| bind_work | 0.058 | 0.059 |
| tick_work | 0.009 | 0.009 |
| world_work | 0.165 | 0.169 |
| Caltrain first frame (assets loaded) | 36.662 | 37.275 |

`create` is the surface factory; bind constructs the Sim and takes the first tick.
`bind_work` and `tick_work` are adapter spans excluding diagnostic output.
`first_tick` uses the host timestamp at `bind_done`: binding takes the first tick,
so this is its observed completion, including ABI parsing and diagnostic writes.
`bind_start` is the entry to that operation. The two adapter spans separate
construction/binding from the inner tick without pretending to share an Instant
epoch across the dynamic-library boundary. `world_work` is the full host interval
from headless-owner loading complete to bind return, including the factory, ABI
parsing, host bookkeeping and trace output. Its 0.165 ms median remains well below
the requested 1 ms investigation threshold.

The old 24.7 ms consists principally of font discovery (~6.8 ms), painter/layout/
first-frame work (~6.2 ms), reading and SHA-256 verifying the native module
(~11.2 ms for the 9.7 MiB development dylib), then dlopen and surface activation. Verification is ordinary module
boot cost; it is deliberately retained. The prior timing path serialized complete
state just to confirm the first tick: that diagnostic-only serialization is now
removed. No startup save/restore or observation was added to gameplay.

### Reproduce and validate

```sh
export PATH=$HOME/.cargo/bin:$HOME/.local/bin:$PATH EXACT_UPDATE_TRUST=development
export K3_SCRATCH=$HOME/lanes/gamenext/scratch/k3
export CARGO_TARGET_DIR=$PWD/game/target
# Before a cold build: df -h ~; stop below 25 GiB.
cargo install twiggy --root "$K3_SCRATCH/tools"
# Put Binaryen 131 and wasm-bindgen 0.2.127 on PATH.
EXACT_APP_DIR=$PWD/game/games/tally EXACT_WEB_DIST=$PWD/game/games/tally/dist \
  CARGO_PROFILE_WEB_STRIP=none bun host/web/build.mjs
mkdir -p "$K3_SCRATCH/named"
wasm-bindgen --target web --no-typescript --out-name gpu \
  --out-dir "$K3_SCRATCH/named" \
  "$CARGO_TARGET_DIR/wasm32-unknown-unknown/web/tally_gpu.wasm"
wasm-opt -Oz -g --enable-bulk-memory --enable-nontrapping-float-to-int \
  --enable-sign-ext --enable-mutable-globals --strip-producers \
  "$K3_SCRATCH/named/gpu_bg.wasm" -o "$K3_SCRATCH/named/optimized.wasm"
"$K3_SCRATCH/tools/bin/twiggy" top -f json \
  "$K3_SCRATCH/named/optimized.wasm" -o "$K3_SCRATCH/top.json"
# Measure the stripped dist, not this named diagnostic artifact.
bun game/games/tally/proof.mjs linux --build-only
RUSTFLAGS='-C llvm-args=-fp-contract=off' cargo build -p caltrain-linux \
  --target x86_64-unknown-linux-gnu --profile gpu-dev \
  --config profile.gpu-dev.opt-level=3 \
  --config profile.gpu-dev.debug-assertions=false \
  --config profile.gpu-dev.overflow-checks=false
export CHROME=$HOME/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome
bun game/world-adapter/measure.mjs --sizes --linux --web --compare-delay
bun game/games/tally/proof.mjs linux --paranoid
bun game/games/tally/proof.mjs web --paranoid
bun game/tests/world-failure/proof.mjs
```

This run used `game/target` for native and workspace checks and the existing
`game/games/tally/target` cache for web bakes; the commands above can share one.
No scratch worktree was created. Disk was checked before cold builds and stayed
above 25 GiB. Named binaries, full twiggy JSON, sample rows and PID inventories
remain under `~/lanes/gamenext/scratch/k3/`; they are not shipping artifacts.

| Validation | Result |
|---|---|
| Adapter unit/integration tests | 13 passed, including 80 KB returned diagnostic, maximum tick request, scalar pressure, exact restore and counted allocation |
| Tally proofs | All six Linux/web ordinary, Save and FreshGame runs passed; pins unchanged |
| Independent wasm failure fixture | 10 assertions passed; page action succeeds after tick-three failure; browser killed and awaited |
| Other Linux game proofs | Seven passed; both Lanterns fixtures execute with zero assertion failures but remain unpinned / UNVERIFIED |
| Game Rust workspace | 756 passed, 18 GPU-required failures, 25 ignored |
| Game Bun | 149 passed, one skipped, zero failed |
| GPU module integration tests | 14 passed |
| GPU/web/Linux Rust sweep | 166 passed, three GPU-required failures, one ignored |
| Web JS fixtures | 149 passed, three skipped, four unchanged Swift-source assertions failed |
| Game and targeted GPU/web/Linux clippy | Passed with `-D warnings` |
| Root/game formatting, caps, boot | Passed; two allowed pre-pixel modules and one wasm reference |
| Whole-root build/test/clippy | Blocked by absent lean Hermes producer for TypeScript app bakes |

No real GPU attachment, rendering, GPU residency or Apple build/run is verified
on this box. Browser failure and startup are verified here; the old CDP timeout
limitation is resolved by using the supported carrier. No pre-K2 no-device timing
baseline exists because that path never constructs a world. No pins were moved
to accommodate any failure; the existing unpinned Lanterns status is retained.

## K4 (2026-09-20)

Merged exactly `f20210f`; the sole conflict was `QUEUE.md`. No authored changes
to `world/`, `world/derive/` or `motion/`. All eleven review inputs were considered:
K3 already handled fallible callbacks and moved the receipt here; K4 handles the
beacon, plain surfaces, canonical save/clock, children, preload and measurement.
Native threaded loading stays deferred (0.161 ms activation; web is this brief's
priority). Kernel size experiments stay with the kernel owner pending KL's verdict.

The beacon and `gpuMs` again mean elapsed time from injection through device
acquisition and shader installation, emitted once. `worldModuleMs` remains the
navigation-relative ownership timestamp. Absent/rejected/pending devices emit no
beacon. The delayed-shader fixture proves first plain render follows installation,
with no headless clock or world stamps. Removing the beacon in a scratch copy
makes the fixture fail. Plain recovery retains IDs/bindings; plan/swap fixtures
check successful commits, failed-render rollback and a missing render ABI.

Every changed line in the requested functions against `f418821` is listed below;
all other lines in those functions match it. Numbers name `host/web/gpu-glue.js`.

| Function / lines | Justification |
|---|---|
| `recoverDevice`, 182 | `!e.headless` selects attached targets. Plain attached IDs still recover together with replacement canvases and retained bindings. |
| `validateStage`, 1022 | Refuse a headless stateless candidate, restoring plain surfaces' pre-device refusal. |
| `validateStage`, 1023 | Skip render only for headless stateful worlds. Plain candidates still execute the original render/error check. |
| `stagePlan`, 1119 | Ownership readiness permits device-free staging; plain pending candidates refuse in validation. The error names module readiness. |
| `swap`, 1178–1179 | Choose ownership ABI: device modules still require `load/create/render` and every original shared export; headless modules require `load_headless/create_headless`. K2 dropped the device checks. |
| `swap`, 1180–1181 | Await device load and mark membership for device-backed creation, or load ownership only. Plain shaders still install before candidate creation. |
| `swap`, 1234 | Set ownership and derive device readiness separately; for plain modules `loaded` is true at the original commit boundary. |

The rest of K2's diff was audited: headless clock and bind/publication/frame
stamps are stateful-only; optional shader exports serve ownership-only modules;
plain attachment/render stays behind shader installation. Existing fixtures cover
asset pressure, rollback and overlapping reloads. The cosmetic-failure fixture
needed its explicit stateful flag after this gating change.

Surface checkpoints are now exactly `Sim::save()`, with input time from
`Sim::clock_ms()`. A fractional-microsecond / 3,600-tick exhausted-settle fixture
checks subsequent queued-input continuation. Tally's hand uses indexed children
in slot order; its existing `Round.deck` retains shuffled draw order. The adverse
fixture interleaves 12,000 unrelated Cards and reparents in reverse order, including
remove/readd. Reads inspect at most thirteen children and explicitly refuse a
thirteenth or a non-Card child; normal reads/holds visit at most twelve.

The normal repin machinery compared Linux/Chromium continuous, Save and FreshGame,
plus Linux release. Tick pins `1: 0x32a9f7776ceb5052` and
`85: 0x3d943e409a0493be` are unchanged. Continuation SHA-256:

- Old: `5e48162a4e3c959e83b205d74a78642b052de101033214e6099042026173f0de`
- New: `694d95cceda9f88d5c9234ccdd2c53b01bcde155ff3f2ff87203e3bda44adb08`

6,228 → 6,212 bytes; **old `[16..]` equals new exactly**. All seven repin runs
agree. Subsequent Linux/web paranoid proofs pass all six pinned modes.

The first, two-link preload improved publication but regressed median FCP
102 → 120 ms cold, 100 → 120 ms warm. Resource timing showed warm `app.wasm`
starting at 54.65 rather than 28.8 ms. Rejected it. The retained version also
preloads `app.wasm` at high priority; GPU hints have low priority. Their warm
starts are now 9.4 / 9.6 / 9.7 ms. Only manifest-declared game modules get hints.
Ordinary HTML matches frozen K3 page bytes, including metadata escaping. The dev
server strips static hints because its verified loader fetches JS as bytes with
no-store, rather than importing it; a response fixture checks this separately.

The retained comparison uses K3's carrier and 200 ms passive grace: ten cold and
ten warm samples per policy, cold policies interleaved, identical modules with
only the three hints removed for the serial control. Current median/p95 tables
are in the [README](../world-adapter/README.md#current-measurements-k4). FCP is
128/216 → 122/148 ms cold, 104/116 → 100/120 warm; the 4 ms warm p95 increase
is within one frame. Median tick/publication is 199.90/207.35 → 130.50/133.60 cold,
157.55/158.95 → 114.95/116.15 warm. K3 historical cold was 159.0/162.3 (warm
158.85/160.1); shared-machine variation makes the interleaved control preferable.
Ten observations do not establish statistical significance.

All 42 navigations (including two discarded primes) show exactly one transfer per
resource. Transfer/encoded bytes: app 661,521/661,221; GPU JS 16,218/15,918; GPU
wasm 366,377/366,077. The no-store server transfers full bytes even when warm.
Fetch hints use matching same-origin credentials. Each trial's 22 recorded browser
groups was SIGKILLed and awaited; both PID inventories report zero leaks.

Wasm is 366,077 raw / 152,719 gzip versus K3 366,615 / 152,634: −538 / +85 bytes.
This comparison uses K3's Bun 1.3.14 compressor. Builds/checks use pinned 1.3.12,
which reports 153,138 gzip on identical wasm. Unchanged app bytes control that
tool difference: 278,846 versus 280,487 gzip. Linux's 20-launch first tick is
24.780/25.044 ms and publication 24.893/25.175, versus K3 24.757/25.328 and
24.865/25.437. Activation is 0.161/0.174; bind/tick work 0.056/0.006 median.
The README has the complete breakdown; no native startup optimization was made.

Reproduce with the K3 commands above, using `K4_SCRATCH`, pinned Bun 1.3.12,
one `game/games/tally/target` cache, and `--compare-preload`. Measurements ran
outside local builds. Disk stayed above 25 GiB. Evidence is in
`scratch/k4/{web,linux,sizes,checkpoint-equality}.json`, `web-processes.json`,
`preload-round1/` and per-command logs.

| Validation | K4 result |
|---|---|
| Adapter | 15 passed |
| Tally | Seven repin runs agree; six subsequent Linux/web paranoid modes pass |
| Other Linux games | Seven pass; Lanterns executes with zero failures but remains unpinned / UNVERIFIED |
| Independent wasm failure proof | 10 assertions pass; browser killed and awaited |
| Game Rust | 758 passed, 18 GPU-required failures, 25 ignored |
| Game Bun | 148 passed, one skipped; literal-digest scan failed, then passed after replacing the HTML hash with frozen page bytes (149 tests covered) |
| Web JS | 160 passed, three skipped, four unchanged Swift-source assertion failures; final byte-fixture retest also passes |
| Root world/GPU/web/Linux Rust | 334 passed, three GPU-required failures, three ignored |
| Game and targeted root clippy | Pass, `-D warnings` |
| Root/game formatting, caps, boot | Pass; two pre-pixel modules, one wasm reference |
| Full root build/test/clippy | Blocked by absent lean Hermes producer for TypeScript app bakes |

Real WebGPU Chrome must still verify device acquisition, WGSL, attachment/recovery
and pixels. No Apple SDK or GPU residency result is claimed. On that machine:

```sh
export PATH=$HOME/.cargo/bin:$HOME/.local/bin:$PATH EXACT_UPDATE_TRUST=development
export CHROME=/path/to/WebGPU-capable/Chrome
bun host/web/build.mjs caltrain-web
bun scripts/smoke.mjs web --shot /tmp/k4-caltrain.png
```

Smoke includes the beacon assertions and step 10's clock-zero canvas screenshot /
crop comparison against `scripts/fixtures/canvas-sky.web.png`; do not repin that
reference. For a retained diagnostic screenshot of the isolated canvas fixture:

```sh
cargo run -q --release -p contract -- build contract/corpus/canvas.contract -o /tmp/k4-canvas.plan
bun scripts/agent.mjs web --plan /tmp/k4-canvas.plan "clock 0" "screenshot /tmp/k4-canvas.png"
```

The smoke's beacon-waited readback is the parity proof; the last screenshot alone
is diagnostic.

## K6 (2026-09-20)

Started on `core/world` at `2e0f07a8`. Read JUDGE-7 and both H7 reviews in full.
No edits to `world/`, `world/derive/`, or `motion/`; no subagents, pushes, remote
commands, worktrees, dependency-version changes, or determinism repins.

### The two ABIs

`git diff f418821 --numstat -- host/web/gpu-glue.js` is **51 additions, 0 deletions**.
No pre-existing line was edited, so there are no edited-line exceptions to justify.
The additions are ownership-only branches selected by absence of `gpu_load`, plus
performance stamps. Device module load/shader order, ready/settled, recovery,
staging, swap, beacon timing and `dataset.gpuMs` execute their original lines.
`gpu_attach`, in-place upgrades, background device promises, `deviceModules`, and
the device ABI's web headless exports were deleted. Native headless exports retain
the original API, with `gpu_advance` added. The original required `Surface::render`
contract was restored; `advance` is the only new defaulted Surface method.

An owned module loads, creates, binds/restores, and joins the frame loop immediately.
It never requests a device or shaders and sends no GPU beacon. Smoke checks the
beacon only when state does not identify an ownership-only module. Mixed apps
containing a shader canvas use the device ABI and wait for their device; the adapter
README declares this limit. Ordinary app and ordinary device-game HTML were both
executed through the `f418821` and current bake fragments and compared byte-for-byte;
they match, including the existing frozen fixture. Preload hints are world-only.

### API and failure behavior

- `Surface::advance(&mut self, _now_ms: f64) -> bool { false }`;
  `WorldSurface::advance` calls `Sim::advance_to` and retains pending publication
  delivery. The ownership web ABI and native ABI export `gpu_advance(id, now_ms)
  returning bool. Owned web frames and Linux commits use it. Agent clock replies
  remain available; the old device-game placement drive is unchanged.
- `publication::publish_record(&World, &impl Data) -> Result<(), DataError>`
  refuses shape, range, traversal, and batch admission without mutation or panic.
- `emit_declaration::<G>(app_dir) -> Result<(), Box<dyn Error>>` reports declaration
  errors to the bake. `exact-app-shell` contains the generic Contract shell code
  moved out of `exact-game-app`; no engine implementation was copied.
  Existing `exact-game-app::NoData` remains reexported for callers.
- Failed state/tree do not hash. The original error survives dead ownership;
  state uses `world.error`, and a readable tree uses `world.error` plus top-level
  `failed`. Top-level `error` remains the host's refused-operation channel. Failed
  tick messages and publications are withheld. Timestamped setup rebinding reaches
  Sim restart recovery; exact restore still works.
- Reload supplies plain values and an array of setup indices, and `releaseInput`
  clears held Sim keys. Actual Tally wasm exercises production `stagePlan`, including
  pressing the same key again after reload. Three live frames advance a real world
  with no agent clock operation.
- Logs carry string lines with line-index cursors, retaining at most 512 lines /
  48 KiB encoded text. A read consumes at most 512 kernel events / 64 KiB. The
  adapter splits already encoded kernel events rather than deserializing objects.
  Escapes, delimiters and Unicode round-trip through an independent JSON reader.
- Owned requests admit 16 KiB / depth 64; returned text 64 KiB, buffered messages
  1,024 / 64 KiB, saves 256 MiB. The regular and owned IDs refuse exhaustion and
  more than 256 live instances. Regular scene bindings retain their previous range:
  Lanterns exposed a mistakenly shared 16 KiB check, which was removed from shared
  binding and retained at the owned ABI boundary. A 20 KiB regression covers both.
- Adapter and `web_owned` runtime sources have no explicit panic/unwrap/assert path.
  Reentrant owned calls now refuse through `try_borrow_mut`; a callback cannot make
  the owner panic by requesting unload. Remaining abort possibilities are arbitrary
  user callbacks, allocation failure, and kernel/internal library assertions. Generic
  shell bake panics and the generated declaration `expect` are build-time diagnostics,
  not wasm runtime paths. No wasm panic is claimed recoverable.

The adapter uses only `exact-world` in its comparison tests. All four Tally shell
`cargo tree` outputs contain no `exact-game*` package. GPU, web and Linux shells were
baked; Apple dependency resolution is proven, but Apple execution needs the SDK.
`world: true` with audio/assets true refuses at manifest validation instead of
producing an invalid macro invocation. Ordinary audio/assets combinations remain.

### Validation

Failing-first evidence is under `scratch/k6`: `web-red.log`, `adapter-red2.log`,
`gpu-red2.log`, `gpu-bounds-red2.log`, `adapter-input-red2.log`, `manifest-red.log`,
`log-lines-red.log`, `reentrant-red.log`, `failure-web-final.log`, and
`binding-limit-red.log`. The last two caught actual host-carrier/ordinary-app
failures after the narrower tests had passed; both were fixed and rerun.

| Check | Result |
|---|---|
| Adapter | 23 passed: 8 unit + 15 integration |
| Idle drive, unfavorable input | Normal Off mode: 1,000 advances, 1,000 components: 0 allocations, 0 bytes, 0 Data writes; real ticks advance, explicit state writes 1,000 components |
| Failed ownership | Adapter and native ABI retain original error, suppress messages, restore/restart; browser proof 17 assertions passes |
| Reload/live ticking | Actual baked Tally + production staging; held-key release and three automatic frames pass |
| Publication/bounds/logs/IDs | Atomic refusal, numerical limits, nested/large data, 5,000-event log churn, encoded output bounds, exhaustion, reentrancy, regular large-binding control pass |
| Tally | All six Linux/web Off, Save and FreshGame proofs pass; original tick and continuation pins unchanged |
| Other Linux games | Seven pass; Lanterns runs with zero failures but is UNVERIFIED because it has no pins |
| Tally browser smoke | `smoke web --app-only` passes, ownership-only beacon exemption exercised |
| Game Rust workspace | 765 passed, 18 GPU-required failures, 25 ignored; final adapter rerun has one additional log test, 23 passed |
| App-owned Rust workspaces | `bun app/shells.mjs --test`: 49 passed, 3 failures explicitly report no suitable graphics adapter, 1 ignored |
| Generic shell / compatibility API | 4 tests pass; both crates pass clippy after preserving the existing NoData reexport |
| Game Bun | Full run 149 passed / 1 skipped / 1 disk-guard refusal; isolated refused fixture then passed after cleanup, giving 150 passed / 1 skipped |
| Web JS | 156 passed / 3 skipped / 4 Swift execution fixtures fail because `xcrun` is absent |
| Root world/GPU/web/Linux | Initial run 345 passed / 4 failed / 4 ignored; ordinary agent-refusal regression corrected, module tests 14/14; remaining three require GPU. One later owned-binding regression added and passes |
| Owned ABI units | 3/3: returned bounds, reentrant callback, large regular binding versus owned refusal |
| Clippy | Game workspace and targeted root GPU/Linux/web `-D warnings` pass; final adapter/GPU reruns pass |
| Formatting/caps/boot | Pass; 806 source files, two pre-pixel modules / one wasm reference |
| Caltrain wasm | `cargo build --offline -p caltrain-gpu --target wasm32-unknown-unknown` passes on final Rust sources |
| Whole-root build/test/clippy | Refuse because the lean Hermes producer is absent for TypeScript app bakes |

All external lockfile package versions/sources that remain are unchanged; the
Tally/failure locks each drop one unused external package with the engine edge.
No hash, position, continuation byte pin, or canvas reference moved.

Smoke printed success but retained its existing `exact-filesystem --serve-reads`
child. Its browser had exited. Recorded helper PID 3416118 was terminated; its
Bun parent 3415612 then exited and was awaited by the shell. The existing queue
entry now identifies that handle. No unrelated process was signaled.
An explicit caller cleanup (`k6_smoke` below) then reran Tally smoke successfully
and exited 0 without manual process cleanup.

### Measurements and limits

[The adapter README](../world-adapter/README.md#current-measurements-k6-2026-09-20)
contains all four browser policy/cache columns, raw/gzip sizes, and the Linux
breakdown. Raw evidence is `scratch/k6/{web,linux,sizes}.json`, with
`web-processes.json` recording 22 browser PIDs and no leaks. There are 20 cold and
20 warm observations plus two discarded priming loads. Each navigation transfers
app wasm, module JS and module wasm exactly once. Sampling ran without local builds.

Bun 1.3.12 / wasm-bindgen 0.2.127; Binaryen 131 is unavailable and no remote install
was attempted. These normal-bake outputs are unoptimized: app wasm 806,232 raw /
285,946 gzip; owned module 836,870 / 181,225; module JS 16,259 / 3,746. This is not
an optimized apples-to-apples size comparison with K4.

The 100 ms target remains red. Preload cold median FCP / instantiated / first tick /
publication: **164 / 168.70 / 176.20 / 181.20 ms**. Warm: **112 / 124.85 / 125.65 /
126.60 ms**. Preload saves 52.3 ms of cold publication versus serial loading while
costing 26 ms FCP in this unoptimized sample. Median app/world transfer completion
is 72.25 / 110.45 ms; paired world-response-to-instantiation interval is 61.8 ms,
then 9.9 ms for create/bind/first tick, then 6.4 ms to publication. The first
interval includes browser compilation and app startup coordination; no CPU profiler
attribution is claimed. The owned path has no device wait. Linux publication is
25.270 / 25.670 ms median/p95; create/bind/first tick is 0.163 / 0.170 ms.

Matched Linux comparator build:

```sh
RUSTFLAGS='-C llvm-args=-fp-contract=off' cargo build -p caltrain-linux -p caltrain-gpu \
  --target x86_64-unknown-linux-gnu --profile gpu-dev \
  --config profile.gpu-dev.opt-level=3 \
  --config profile.gpu-dev.debug-assertions=false \
  --config profile.gpu-dev.overflow-checks=false
K6_SCRATCH=$HOME/lanes/gamenext/scratch/k6 \
  bun game/world-adapter/measure.mjs --sizes --linux --web --compare-preload
```

Disk was checked before cold builds. One integration test's own 25 GiB guard
stopped its nested bake; clearing only this clone's completed Cargo profiles let
it pass. Final free space is about 26 GiB. No extra worktree/build matrix was added.

### Commands still required on the Mac

Use the pinned Bun and Binaryen 131. Real WebGPU, pixels, actual device loss and
recovery, and Apple execution were not available here; JS fixtures do not replace
them. From this checkout (do not repin references):

```sh
export PATH=$HOME/.cargo/bin:$HOME/.local/bin:$PATH EXACT_UPDATE_TRUST=development
export CHROME='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'
k6_smoke() {
  bun -e 'process.argv=["bun","scripts/smoke.mjs",...process.argv.slice(1)]; try { await import("./scripts/smoke.mjs"); } finally { (await import("./scripts/filesystem.mjs")).closeFilesystemReader(); }' "$@"
}
df -h ~
bun install --frozen-lockfile
bun host/web/build.mjs caltrain-web
k6_smoke web --shot /tmp/k6-caltrain-web.png
bun game/games/asset-fixture/proof.mjs web
bun test host/web/tests
bun game/games/tally/proof.mjs web --paranoid
bun game/tests/world-failure/proof.mjs
bun host/apple/build.mjs
k6_smoke macos --shot /tmp/k6-caltrain-macos.png
bun host/apple/build.mjs --ios
k6_smoke ios --shot /tmp/k6-caltrain-ios.png
bun game/games/tally/proof.mjs macos
bun game/games/tally/proof.mjs ios
cargo test --manifest-path game/Cargo.toml -p exact-game-render surface_lifecycle -- --ignored
```
