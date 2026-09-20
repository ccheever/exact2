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
