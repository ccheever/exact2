# Original Heavy List on Android

These release adapters use the original `../SPEC.md` workload: 10,000 keyed messages,
104 JPEGs, rich text, photo grids, links, quotes, reactions, and optional live updates.
Exact compiles the unchanged `../exact-heavylist/app.contract` and original `Heavy`
data provider. C9 selects `General<Heavy>`; main uses the public
`exact_linux::android::Handle` native-window GPU painter. Views and Compose reconstruct
the same rows from the published JSON and JPEGs. Views recycles the row roots but
rebuilds their inner content when binding. Reference separators follow the inherited
UIKit/SPEC overlay and right inset; the original Contract allocates a separate0.5dp
separator extending to the right edge. Platform text/emoji rendering also differs;
these are workload reconstructions, not an assertion of pixel identity.

## Build

Run `../prepare.sh` with Pillow11.3.0, then verify `data.sha256`. Only the published
bytes are comparable. Tools are pinned by `host/android/build.mjs`: Bun1.4.2, stable
Rust with its Android target, JDK17, Gradle9.3.1, SDK37.0/build-tools36.0.0,
NDK28.2.13676358, and dependencies from `bun.lock`. Set `ANDROID_HOME`, optional
`ANDROID_NDK_HOME`, `JAVA_HOME`, `EXACT_GRADLE`, `GRADLE_USER_HOME`, and Bun/Cargo PATH.
A provisioned local `target/android-env.sh` can supply paths. Keep one Cargo target
per checkout.

The main reference uses a separate checkout containing the public Android Handle.
Install only the benchmark adapter there without changing its runtime:

```sh
bun bench/heavy-list/android/prepare-main.mjs /path/to/main-checkout
CARGO_TARGET_DIR="$PWD/target" bun bench/heavy-list/android/build.mjs \
  --main-root /path/to/main-checkout --only all
```

`--only c9|main|views|compose` selects one APK. `--abi arm64-v8a|x86_64` selects its
native ABI. `--output directory` changes generated projects and receipts. Main Cargo
uses that checkout's `target/` or an external private `EXACT_MAIN_CARGO_TARGET_DIR`.
C9 uses `buildBake` with `EXACT_APP_DIR` selecting the original nested workspace.
All variants use release Rust, R8/resource shrinking, a single ABI and debug signing.
They are shell-profileable and not debuggable. Main requires Android API30 because
its existing native image decoder uses `AImageDecoder`; the other APKs use API29.
The main adapter uses the stock CanvasJNI allocator with the public Handle painter.
`--reuse-native` permits a Kotlin-only rebuild only when the saved native-stage receipt
matches source revision, tracked diff, tracked/untracked native source hashes, app
inputs, compiler/settings, and library/archive bytes. A changed revision forces a
fresh native build even if the resulting source content is identical. The tracked
diff guard is conservative: edits to tracked Kotlin also prevent native reuse.
`--relink-native prior-stage.json` verifies the same inputs and prior library/archive
hashes, then links a new shared library from the existing Rust archive. The new
receipt identifies its parent and states that Rust was not recompiled.

## Collect

Packages are `dev.exact.heavybench.c9`, `.main`, `.views`, and `.compose`; exported
Activities are `dev.exact.heavybench.C9Activity`, `MainActivity`, `ViewsActivity`, and
`ComposeActivity`. The shared harness accepts `BENCH_TOKEN`, `BENCH_LIVE`,
`BENCH_SPEED_DP_S`, `BENCH_DURATION_MS`, `BENCH_WARMUP_MS`, and `BENCH_START_INDEX`
(main supports index0 only). All four use the same immersive viewport.

`HeavyBench:Startup` reports renderer readiness observed after a Window draw and
`reportFullyDrawn` timing; metadata states each readiness definition. Main reads
public `first_frame` after scene submission acknowledgment and reports the submission
counter; these precede independent GPU presentation.
`HeavyBench:Measure` reports paced requests, process/main-thread CPU, Window metrics,
and memory after the interval. Window frames and Choreographer callback cadence do
not measure the Vulkan Surface's presented FPS. FrameTimeline can supply presentation
data only when its content-layer coverage and loss checks pass. The public main
Vulkan path may have no tracked SurfaceFrames; use the separate SurfaceFlinger
cohort below for all four renderers. Main's public API has no scroll acknowledgment, row identity or
mounted-row geometry; these remain unavailable instead of echoing requested motion.
A real main Surface resize restarts its Handle because the public API has no resize
command. First-install image extraction is included in startup; later launches check
a dataset stamp. Report cold-process and first-install startup separately.

Receipts bind APK/native hashes, ZIP footprint, release flags, source revision and
canonical dataset hashes. Native variants preserve original `assets/...` image paths;
references load the same JPEG bytes under `images/...`. Native bakes validate the
complete image inventory. The build script neither publishes nor installs APKs.

## Run and analyze

From the repository root, select a physical device explicitly and use the directory
containing the four `c9/main/views/compose` build receipts. The default build output
is `target/heavy-list-android`. Both output directories below must be new.

```sh
python3 bench/heavy-list/android/run.py \
  --products target/heavy-list-android --serial YOUR_DEVICE_SERIAL \
  --out target/heavy-list-capture-r1 \
  --rounds 4 --startup-reps 4 \
  --speeds 1000,3000,6000,12000,24000 --duration-ms 8000

python3 bench/heavy-list/android/analyze.py \
  --capture target/heavy-list-capture-r1 \
  --out target/heavy-list-analysis-r1 \
  --processor /path/to/trace_processor_shell
```

ADB must be on `PATH`, or pass `--adb /path/to/adb`. `--plan-only` validates local
APKs and dataset hashes without contacting the device. Add `--live` to enable the
same authored live mutations in all four apps. Four rounds and four startup
repetitions balance renderer positions; the default three are exploratory.

The runner installs and verifies APKs, requests ART speed compilation, retains its
readback, and uses a fresh process for each launch. This leaves app data and OS
caches warm; it does not automatically separate first-install asset extraction.
Startup runs are untraced. Readiness can precede completion of image decoding;
references parse the complete published JSON while Exact starts its baked plan/data
provider. With four startup runs, report the median, range and retained individual
values; a tail percentile or performance equivalence is not established.

One scroll-only Perfetto capture records FrameTimeline,
benchmark markers and process statistics, without CPU sampling or video.

Read `report.md` with the retained phase JSON and trace-health report. Ready time is
process start to renderer readiness after a Window draw, not TTI or independently
observed first presentation. Presented FPS counts unique linked actual
SurfaceFlinger display presentation timestamps. Late display gaps use the local
observed expected-display cadence; they are distinct from native jank flags.
Missing or ambiguous content-layer coverage, unresolved loss or incomplete frame
mapping leaves headline FPS unavailable; Window frames and callbacks never fill it
in. Main's unavailable realized scroll distance also limits equal-work claims.


## Independent presentation cohort

Use a new directory after `run.py` has stopped its trace. This collector polls the
selected SurfaceFlinger layer every400ms equally for all four apps, without
Perfetto, screenshots or video. APK/process/layer ancestry, clock domains, raw-dump
hashes, overlapping histories and individual pending fences are checked offline.
Do not mix its startup, CPU or memory observations with the first cohort.

```sh
python3 bench/heavy-list/android/sf_latency.py \
  --products target/heavy-list-android --serial YOUR_DEVICE_SERIAL \
  --out target/heavy-list-present-r1 --skip-install \
  --rounds 4 --speeds 1000,3000,6000,12000,24000 \
  --duration-ms 8000 --warmup-ms 1500 --cooldown-ms 1000

python3 bench/heavy-list/android/sf_latency.py \
  --analyze-only target/heavy-list-present-r1
```

Samsung's history export can initially expose an old partial prefix. The default
presentation window therefore excludes the first3s of already paced scrolling and
the final200ms: about4.8s within each8s motion interval. These limits are explicit
in every result (`--presentation-lead-ms` / `--presentation-tail-ms`). This measures
steady scrolling; it does not report the full8s presentation average. No app code
or requested motion changes for this cohort.

Presented updates are distinct **second-column actual-present fence timestamps**,
within the selected CLOCK_MONOTONIC interval. Pending or overwritten records,
missing boundary coverage or stale history invalidate the measurement. The collector
requires history reaching the interval's end. It retains startup partial histories
and the Galaxy's observed rolling counts up to127; this is a verified device format,
not a stable Android SDK contract. The latency header can report60Hz even for main
presenting near120Hz, so gap thresholds use the app's recorded active display rate.
See [AOSP FrameTracker](https://android.googlesource.com/platform/frameworks/native/+/refs/heads/main/services/surfaceflinger/FrameTracker.cpp)
and [FrameTimeline history export](https://android.googlesource.com/platform/frameworks/native/+/refs/heads/main/services/surfaceflinger/FrameTimeline/FrameTimeline.cpp).

Late presentation gaps exceed1.5 display periods. They are **not literal dropped
buffer counts** or native jank classifications. A failed or incomplete cohort cannot
be promoted to complete by reanalysis. Main's public Handle still has no realized
scroll acknowledgment; the same requested speed does not prove identical motion.

CPU uses process CPU time summed across threads and separately UI-thread CPU time,
normalized by the measured wall interval; a process can exceed100% by using several
cores. Memory is post-interval PSS/RSS and Java heap, reported separately at each
speed; image exposure and allocator residency vary. APK bytes include application
data and assets, so an APK comparison is not a pure framework-runtime comparison.


Known Galaxy limitation: a later balanced capture exported128 occupied records,
including a zero actual-present timestamp. The three-column dump does not identify
its PresentState. The collector conservatively rejects this unsupported/ambiguous
history; the failed32-phase plan is preserved with27 completed phases and supplies
no cohort headline. A separate four-app24k smoke passed all checks, but one sample
per app is exploratory and does not establish an FPS ranking. The startup/CPU/memory
cohort completed independently. Do not raise a count limit or classify zero as a
dropped frame without binding and verifying the device's export semantics.

## Generic row reuse and performance follow-up

C9 keeps Android Views and SDK controls as its presentation carriers. Text uses the public imperative Compose Paragraph API; collection reuse does not require the Compose composition runtime. The accepted integration changes are:

- `e733172fe`: transfer the admitted, decoded Plan into the general host instead of decoding another copy during boot.
- `c32d88bc8`: share cancellable image decode demands, limit concurrent decodes to two, and avoid retaining invisible center-cover pixels where the platform density rules permit cropping. Natural image dimensions remain separate from displayed pixel dimensions.
- `a3d85db48`: report original logical row width/height to the collection index. Android's rounded View bounds remain the drawing geometry; using them as logical measurements introduced a spacer/retained-row rounding mismatch.
- `f1471570b`: opt the Android presenter into existing shared Runner row reuse for admitted stateless row bodies. A `renew` batch resets the native incarnation before applying the new row's content, paint and motion.

Virtualization already limits the mounted collection window. Reuse reduces the work of replacing a compatible row within that window: the Runner rebinds retained node identities and renews measurement ownership, while Kotlin rotates callback/image owners and resets native interaction/presentation state. Old measurement epochs and delayed callbacks cannot update a new row. The opt-in survives fresh/prepared boots and policy changes. Stateful/native row bodies stay on fresh mounts, Android checks unsupported scroller/SVG/navigation row sites, and runtime renewal refuses live focus/contact or native navigation ownership. Enabling an accessibility service disables the Android reuse opt-in.

| Mechanism | Retention bound and lifetime |
| --- | --- |
| C9 shared Runner reuse | Up to four spare rows (`HOLD = 4`) per virtualized `Collection`, in addition to its mounted window and required pins. Ordinary nested `Each` regions do not each receive another four-row spare pool. A row can still contain many logical nodes. |
| Compose `LazyColumn` | `LazyLayout` retains up to seven inactive reusable slots **per content type**, with separately active/precomposed prefetch work. Composition/content reuse, per-pass measurement reuse and scroll-placement fast paths avoid different kinds of work; text shaping and changed layout still cost CPU. |
| Exact UIKit host | Its own `NodePool` parks compatible native trees by shape: eight per shape, 32 outer trees total; inner-list cards have separate bounds of 12 per list and 96 overall. These are tree counts, not View counts. Heavy native leaves are destroyed/recreated, and parked text/raster/image state is reset. Shared Runner row reuse remains off by default in the Apple ABI. |

C9 separately retains each live text Source's current metric/layout lease and optional paint commands, a 512-entry paragraph-offer LRU, a weak 512-entry shared-text index, and a 32 MiB image LRU. Visible image owners, worker results and posted delivery closures can hold pixels outside that image-cache bound. Renewal rotates ownership while permitting unchanged pure metric or same-source image leases to survive; UIKit parking clears its node-owned text/raster/image state.

The bounds describe distinct mechanisms and do not establish equal memory or performance. Sources: `runner/src/instance/collection/reuse.rs`, `host/android/src/row_reuse.rs`, `host/apple/src/abi_row_reuse.rs`, and `host/apple/Sources/ExactKit/IOS/NodePoolIOS.swift`. Compose findings use `LazyLayout.kt`, `LazyLayoutMeasureScope.kt`, `LazyListState.kt` and `SubcomposeLayout.kt` from the benchmark's pinned [AndroidX Compose 1.12.1 source archives](https://dl.google.com/dl/android/maven2/androidx/compose/foundation/foundation-android/1.12.1/foundation-android-1.12.1-sources.jar) ([UI sources](https://dl.google.com/dl/android/maven2/androidx/compose/ui/ui-android/1.12.1/ui-android-1.12.1-sources.jar)).

The retained `pair-final-r1` comparison measures the **four-change bundle**, rather than attributing its entire effect to row reuse. It uses the original Heavy workload on Galaxy A57 (SM-A576B), release/R8 APKs, fresh processes, unchanged workload assets, preserved app data, and four balanced A/B pairs at each scroll speed. The measured interval is eight seconds after a 1.5-second warm-up. ART speed compilation returned Success; effective compiler-filter readback was unavailable. The cohorts are untraced.

| Scroll speed | Metric | Original C9 | Accepted bundle | Change of medians |
| --- | --- | ---: | ---: | ---: |
| 1,000 | Main-thread CPU | 29.23% | 28.39% | −2.86% |
| 1,000 | Process CPU | 98.26% | 97.17% | −1.11% |
| 1,000 | Endpoint PSS | 255.93 MiB | 216.80 MiB | −15.29% |
| 24,000 | Main-thread CPU | 64.81% | 62.69% | −3.26% |
| 24,000 | Process CPU | 136.39% | 129.49% | −5.06% |
| 24,000 | Endpoint PSS | 341.51 MiB | 371.24 MiB | +8.71% |

CPU percentages are CPU time divided by elapsed time; 100% represents one fully occupied core. Values are medians of four runs. Changes use the ratio of those medians, rather than the median of paired percentages. The fast-scroll PSS increase occurred in every pair, and endpoint Java heap usage rose from 7.38 to 19.47 MiB. This remains a memory regression requiring diagnosis.

Separate heap graphs distinguish live ownership from allocation retirement pressure. Reachable Bitmap native-registration estimates were 31.55 MiB in the original and 31.79 MiB in the candidate; **unreachable** estimates were 8.87 and 34.77 MiB. Unreachable objects awaiting GC/Cleaner settlement are different from a strongly retained image cache. Registrations are allocation estimates, not total renderer/GPU memory, and multiple Cleaner registrations can belong to one Bitmap. Detailed `meminfo -a` diagnostics also perturb GC, so their later settled values are kept separate from the untraced endpoint measurements. These observations support investigating retired image/text command pressure; they do not by themselves prove a leak or explain the entire PSS delta.

Earlier source-bound profiles point to Runner/kernel layout, text shaping/paragraph work, image decoding and native traversal as useful targets. JNI entry stacks include the real work below the call, so they do not isolate boundary-transition overhead. Inclusive sample groups overlap, and the first trace had lost records; function sample counts are not an additive millisecond breakdown.

Four additional generic experiments were withdrawn after release A/B tests: resetting text command warm-up age (fast-scroll process CPU −0.05%), re-flattening renewed passive carriers (−0.71%), uniform-foreground paragraph reuse (+0.11%, PSS +1.73%), and delaying image requests until final transaction geometry (+0.33%, PSS +0.98%). Each used four balanced pairs and passed focused SDK/integration checks. Their CPU results did not justify extra production complexity; these variants are excluded from the final build. The uniform-paint differential probe compared 2,070 software-Canvas glyph images and 414 shared-owner draws with unchanged metrics and zero pixel differences (HWUI/RenderNode parity was not tested by that probe), which proves that isolated case rather than a CPU benefit.


## Collection port facts and the next CPU phase

`744c8e2d6` reads only the port offset and dimensions at transaction begin/end. Full row measurements and focus/interaction pins are still read by the unchanged feedback flush. The first available row cross dimension and all validity/range/anchor rules are preserved. On the actual minified Android SDK probe, 73 differential cases passed, including 14 identical illegal-extent rejections; a 128-row begin/end reduced row reads from 258 to 2. Full Rust/JNI/SDK renewal and text probes also passed.

The first four fast-scroll pairs showed main-thread CPU −0.60%, process CPU −0.46%, PSS −2.93%, and Java heap −16.14%. The first slow-scroll series showed process CPU +1.78%, so a second balanced four-pair series was collected at both speeds. Its process CPU change was +0.27% at 1k and −0.09% at 24k; Java heap fell by 7.92% and 18.25%, and PSS by 0.89% and 3.36%, respectively. All eight paired memory endpoints at each speed were lower. Pooled across both series (eight pairs per speed), main-thread/process CPU changed by +0.29%/+0.81% at 1k and −0.58%/−0.29% at 24k. PSS changed by −0.69%/−2.65%, and used Java heap by −7.83%/−16.08%. The slow CPU increase remains unexplained; these samples do not establish equivalence. This is accepted as a small generic allocation reduction, with a weak/mixed CPU effect. It is not a proof of CPU equivalence or lower peak memory. Fast-scroll endpoints can differ by one mounted row; the whole PSS difference cannot be assigned solely to discarded measurement objects.

The stable-window placement path already exists in `runner/src/instance/collection/within.rs`: unchanged port/pins/sizes and retained coverage can move the native scroller without publishing another layout. Importing LazyColumn wholesale would duplicate this mechanism.

The proposed rich-text measurement/paint staging protocol was implemented experimentally and withdrawn. On the actual Android SDK and pinned public Compose paragraph API, splitting a ligature-bearing string into differently colored runs changed its measured line width by 1.0667 logical pixels while preserving its line ends. A styled paint Paragraph therefore cannot substitute for the existing metric-only Paragraph without changing scalar answers. Software-Canvas experiments with mutable SDK spans also failed 20 BoringLayout ellipsis cases; they are not a ready replacement. Measurement and paint leases remain separate in the accepted implementation.

Two further generic cache experiments were tested and withdrawn on 2026-10-08. Each release A/B series used four balanced pairs at 1k and 24k dp/s, identical original Heavy assets and no profiler/video. Earlier access to the existing Rust scalar memo changed process CPU by −0.50%/−0.04% and endpoint PSS by −0.12%/+0.37%. Separate diagnostic aggregates found zero early or late memo hits during either startup-to-measure interval, so this change did not address the Heavy workload.

A second, Android-only experiment admitted exact validated request bodies up to 2 KiB under a 128 KiB additional primitive-payload budget. Actual SDK checks passed 1,672 bit-exact scalar comparisons, 288 software-Canvas image comparisons, 1,776 budget/retirement checks and 16 invalid-input checks; full Rust/JNI/SDK renewal checks also passed. The controlled release APK retained the baseline native library byte-for-byte. Process CPU changed by −0.10%/+0.18%, main-thread CPU by −0.86%/+0.21%, and endpoint PSS by +0.04%/+0.17%. All paired CPU ranges crossed zero; the fast Java heap decrease was inconsistent across pairs. These observations do not establish a material CPU or memory improvement, so the budget and cache extension are absent from the branch.

Excluded diagnostic timer brackets found 28 slow-scroll and 754–854 fast-scroll repeated parses with unchanged canonical contents above the existing 512-byte body limit. Those are potential allocation savings, not parser milliseconds or proof that the raw bodies are byte-identical. Existing font-resolution checks and separate metric/paint layouts already account for work after parsing. The following diagnostic splits that work into parsing, intrinsic resolution, Paragraph layout and drawing; overlapping sample stacks cannot provide that additive breakdown. Reusing an immutable metric String/span projection is a separate unaccepted candidate and would not remove shaping or layout.

An excluded main-owner-thread diagnostic then recorded ten disjoint text segments with thread-CPU clocks, separate nested recording/replay costs, complete timer brackets and zero invalid clock/residual observations. At 24k dp/s, measured text work consumed 1.509–1.726 s between the inner and surrounding brackets, 33.12–33.26% of the main-thread CPU measured over each respective timer bracket. Request/header work consumed only 74–84 ms (about 1.6%); intrinsic resolution consumed 357–416 ms, metric layout/scalar lookup 500–571 ms, rich preparation 243–274 ms and command recording 149–165 ms. The remaining owner CPU is outside those text segments; a separate extended capture below divides native calls and host replay. These clocks include probe/bookkeeping and owner-thread allocation costs; they exclude other threads, GPU work and blocked time, and are not production timing rankings.

The same capture counted 866–979 repeated rich metric String/span projections for the same Source/configuration. Their CPU is an unknown subset of rich preparation, which also builds styled SDK intrinsics and layouts. Those counts justify measuring projection construction separately; they do not justify attributing the whole rich-preparation cost to it or adding another cache before A/B evidence.


A separate excluded release diagnostic adds sixteen exclusive host scopes and an auxiliary projection timer. Both speeds have twenty paired inactive-owner timer snapshots, complete interval brackets, stable PID/main-thread identity, and zero invalid/off-owner/residual observations. Native, original Heavy inputs and harness remain unchanged. At 24k, matched bracket CPU fractions are 29.87–30.10% text, 58.05–58.90% host (including native call sites), and 11.23–11.85% unclassified. Native inclusive arrays overlap nested text; only the exclusive partition adds.

| Fast-scroll region | Inner–surrounding CPU ms |
| --- | ---: |
| Native call-site self | 1,099.08–1,228.62 |
| Batch decode | 206.12–230.06 |
| Batch replay self | 307.83–343.20 |
| Presenter finalization self | 315.74–349.80 |
| Box dispatch self | 460.36–509.91 |

Native call-site self includes Rust/kernel work, ABI/serialization and argument evaluation; it is not isolated JNI transition cost. Decode/replay/finalization together cost 829.70–923.06 ms, so the next host profile separates property updates, style decoding, hierarchy reconciliation and semantics. Box dispatch includes native child traversal after subtracting separately attributed text/host children. These N=1 instrumented brackets are not additive with the preceding text-only capture or production A/B samples; they do not time other threads, GPU presentation or blocked time.

All zero-record decode/replay/finalization/control-resolution passes together cost only 22.26–25.41 ms during fast scroll (at most 0.47% of exact eight-second harness UI CPU). Collection feedback produces only 169–194 of those 658–728 empty batches; its CPU subset was not timed separately. Slow empty passes cost 120.22–128.46 ms, mostly from collection feedback. A conservative Apple-inspired shortcut was prepared but remains unapplied: it must preserve pending-fill continuation, control reconciliation, layout/correction obligations, scheduling and post-transaction intrinsic drains. Its additional guards are not justified as a fast-scroll fix by these bounds.

Repeated rich metric String/span projection construction itself consumed only 7.47–8.41 ms at 24k (at most 0.16% of exact harness UI CPU), including its probes and allocations. That auxiliary timer is already inside rich preparation and must not be added again. The projection-cache proposal remains unapplied. C9 already has a bounded exact-content metric lease lookup across Source owners, so adding an id-independent text cache would duplicate existing work.

## Frozen release comparison after the integration changes

The final `four-way-final-r2` cohort uses C9 commit `744c8e2d6`, the unchanged original Heavy assets, Galaxy A57/API36, release/R8 arm64 APKs, Live OFF, four startup runs and four scroll runs per renderer/speed (48 fresh processes). Renderer positions are balanced independently at both speeds. No trace/profiler/video runs enter these medians. Main is frozen at `22bf5925d` to preserve the earlier source/profile reference; it is not the latest main.

| Renderer | Process → ready, median [range] ms | APK MiB |
| --- | ---: | ---: |
| C9 / Android Views | 190.33 [189.35–192.97] | 28.70 |
| Frozen main 22bf592 / Vello | 216.25 [200.49–225.00] | 35.48 |
| Android Views / RecyclerView | 193.52 [188.32–196.05] | 8.97 |
| Jetpack Compose / LazyColumn | 211.87 [210.30–215.66] | 10.23 |

| Requested speed dp/s | Renderer | Process CPU % | Main-thread CPU % | PSS MiB | Used Java heap MiB |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1000 | C9 / Android Views | 96.15 | 27.80 | 215.52 | 17.63 |
| 1000 | Frozen main 22bf592 / Vello | 112.64 | 6.10 | 660.67 | 4.38 |
| 1000 | Android Views / RecyclerView | 79.67 | 17.81 | 150.11 | 20.52 |
| 1000 | Jetpack Compose / LazyColumn | 89.82 | 26.93 | 154.73 | 24.20 |
| 24000 | C9 / Android Views | 129.60 | 63.11 | 367.17 | 17.30 |
| 24000 | Frozen main 22bf592 / Vello | 142.30 | 4.12 | 774.12 | 4.38 |
| 24000 | Android Views / RecyclerView | 118.71 | 36.97 | 365.08 | 40.15 |
| 24000 | Jetpack Compose / LazyColumn | 121.05 | 46.60 | 348.53 | 44.11 |

CPU percentages use one core as 100%; main moves most painting onto its separate renderer/present threads, so its main-thread percentage is not total rendering cost. At 24k, C9 process CPU remains 9.17% above Views and 7.06% above Compose, while its main thread is 70.73%/35.43% above them. Endpoint PSS is 0.57%/5.35% higher. At 1k, C9 process CPU and PSS remain 20.69%/43.57% above Views. These are descriptive contrasts for requested motion, not causal A/B effects or equivalence tests.

Ready is observed after a Window draw; it is not independently verified TTI, complete image loading or GPU presentation. Data and OS caches are warm. PSS/used Java heap are after-interval endpoints, not peaks; native allocations are outside Java heap. Requested speed does not prove equal realized motion for main, which exposes no scroll acknowledgment. Missing presentation coverage keeps headline FPS unavailable. The raw receipt retains every sample, duration, viewport/process/APK binding and source hash. Use the earlier collectors for trace/presentation investigation; keep cohorts separate.


## Generic retained styles and lazy border paths

Two small Android host changes preserve the existing imperative native-control path. A node keeps its decoded immutable geometry until its style is replaced; renewal still resets validation and preserves the latest independent PAINT pairs. Border side Paths are allocated and intersected only before a multicolor draw. Uniform borders keep the existing ring, and geometry invalidation makes the next multicolor draw rebuild its partitions. Neither change modifies the kernel, text layout or the workload.

The excluded `text-host-detail-cpu-diagnostic-device-r1` capture attributes about 1% of fast owner CPU each to repeated retained-style decoding and eager border partitions. These are upper bounds, not promised savings. `pair-style-border-r1` tests the two changes together on Galaxy A57 with release/R8 arm64 APKs: four balanced AB/BA pairs per speed, 16 fresh processes, the original Heavy workload and eight-second intervals after 1.5 seconds of warm-up. The native library, harness, assets, certificate and requested/acknowledged travel are unchanged; only DEX and its generated ART profile differ. No profiler, video, forced GC or data reset enters the cohort.

| Fast scroll (24,000 dp/s) | Baseline | Style/border bundle |
|---|---:|---:|
| UI CPU (% of one core) | 62.61 | 62.02 |
| Process CPU (% of one core) | 128.65 | 129.02 |
| Window duration p95 (ms) | 11.36 | 11.06 |
| Process-to-ready (ms) | 192.26 | 192.62 |
| PSS after interval (MiB) | 368.25 | 343.15 |
| Used Java heap after interval (MiB) | 17.73 | 9.09 |
| APK (MiB) | 28.696 | 28.696 |

Fast UI CPU falls in all four pairs (0.64–1.24% relative). Process CPU and startup show no demonstrated improvement; mixed signs and four pairs do not establish equivalence. Slow CPU shows no consistent change in either the original cohort or the separate eight-phase `pair-style-border-slow-followup-r1`. The first slow Window p95 increase is not reproduced in the follow-up, but mixed pair signs establish neither a consistent regression nor an improvement. Window timings are not presented FPS or independently verified frame drops, and ready is not TTI. Memory figures are endpoints: the large fast heap/PSS difference does not establish retained or peak savings independently of GC. The bundle cannot identify either change's individual effect and does not close the Views/Compose gap.

Actual release SDK correctness includes the original inline paint, pending intrinsic, native renewal and asynchronous renewal checks, plus 12 exact retained-versus-fresh software style comparisons covering pooled/unpooled replacement, PAINT, invalid-style refusal and destruction/new incarnation. A separate border probe passes 3,840 software cases and 48 same-origin HWUI cases against baseline and fresh owners without tolerance. Its original two-pixel mismatch at different screen positions was reproduced by identical baseline code at those positions; the failed capture is retained separately. Source, product, runtime and both untraced cohorts were independently audited.

Further excluded hierarchy attribution (`text-host-hierarchy-cpu-diagnostic-device-r1`) measures reconciliation at 287–319 ms, about 5.7–5.8% of bracket owner CPU. Flat planning is 22–25 ms; `setLeaves` is only 5–6 ms. Just 4–6 of 1,764–1,995 leaf updates retain identical object order, so a stable-membership cache is not justified. Recursive attachment timings overlap themselves; they cannot be added or treated as unique time. A further excluded attachment capture (`text-host-attachment-cpu-diagnostic-device-r1`) uses recursion-correct self accounting and twenty paired inactive snapshots per speed. Fast removal costs 106–110 ms across 3,537–3,656 calls; addition costs 113–117 ms across 5,473–5,698 calls. Acquisition is 49–51 ms; cold creation is 75–79 ms including callers outside attachment; placement is 16–17 ms. Only about 9.2% of acquisitions enter without a carrier, and roughly half of placement requests have unchanged bounds. Top-level unclaimed attachment CPU is 387–402 ms, including descendant non-attachment reconciliation and probe work. These auxiliary families overlap; do not add them or compare different instrumented captures as speedups. A subsequent excluded mutation census (`text-host-attachment-mutation-diagnostic-device-r1`) classifies all measured removal attempts in this attachment path as same-parent reorders: 106 at slow speed and 3,288–3,629 in the fast brackets, with zero pruning or cross-parent removal. Fast additions are 3,288–3,629 reorder repairs plus 1,772–2,002 previously unattached carriers. These counters describe this workload/path, not every framework removal or a speedup against earlier diagnostic builds.

Ordinary iOS/tvOS defaults to collection fills on a separate runtime owner, distinct mutable measurement/painting text engines and ordered publication (`host/apple/Sources/ExactKit/Session.swift`); agent mode and `EXACT_FILL_SYNC=1` disable async fills, and macOS defaults to synchronous filling. C9 remains main-owned. An Android port needs those ownership and coverage guarantees, and moving CPU alone would not prove lower total CPU. The inspected iOS source does not establish which mode older iOS benchmark recordings used.

The applied two-file bundle also passes the required default-member build, full tests (3,548 passed, zero failed, 34 ignored), strict Clippy, formatting, staged caps and boot. Production source hashes match the uninstrumented A/B candidate. These checks do not claim a fresh full-workspace or iOS performance run.

## Preserving native child attachment

The generic Presenter ordering path now moves retained children with the public `ViewGroup.bringChildToFront` API before descending or inserting new children. It keeps the longest prefix of the requested retained-child order already present as a subsequence, then moves the remaining children to the end in order. Genuine pruning, cross-parent moves, new insertions and indexed repair retain the original path. Carrier geometry and paint/hit ordering still update; no global membership cache is added. When an accessibility service is enabled, the original indexed path is used. This follows the attached-child ordering direction used by UIKit's `insertSubview(at:)` rather than removing and recreating window attachment during ordinary same-parent reordering.

The separate `pair-native-order-r1` cohort compares the accepted style/border APK to this ordering change: four balanced AB/BA pairs per speed, 16 fresh processes on Galaxy A57, release/R8 single-arm64, the unchanged original Heavy List (10,000 messages, 104 JPEGs), Live OFF, eight-second intervals after 1.5 seconds of warm-up. Only Presenter differs across the frozen 147-input projects; native library, text engine, borders, assets, harness, signing and build settings are identical. The APK byte size is unchanged. The unchanged collectors bind installation, process identity, clock intervals, viewport and acknowledged integer motion. All phase thermal states are zero. No tracing, profiler, video, forced GC or data reset enters this cohort.

| Fast scroll (24,000 dp/s), median | Style/border baseline | Native ordering |
|---|---:|---:|
| UI CPU (% of one core) | 62.43 | 59.43 |
| Process CPU (% of one core) | 129.63 | 127.75 |
| Window duration p95 (ms) | 11.11 | 10.86 |
| Process-to-ready (ms) | 193.29 | 196.18 |
| PSS after interval (MiB) | 346.10 | 344.75 |
| Used Java heap after interval (MiB) | 9.39 | 8.79 |
| APK (MiB) | 28.696 | 28.696 |

Fast main-thread CPU falls 4.80% relative, with all four pairs improving by 4.28–5.15%. Process CPU falls 1.45%, with all four pairs improving by 1.13–1.59%. Window p95 falls 2.25% and is lower in all four pairs; deadline-overrun counts remain sparse and mixed. Slow CPU medians are effectively unchanged (UI 28.088→28.122%; process 97.318→97.255%), with mixed paired changes. Ready medians are higher at both speeds (slow 192.51→196.76 ms; fast 193.29→196.18 ms), also with mixed pairs. This does not establish faster startup or equivalence. Memory snapshots likewise do not establish retained or peak savings independently of GC. Window timing is not presented FPS, verified frame drops or TTI. This N=4 cohort is separate from the style/border and frozen four-way cohorts; do not add their percentages or claim native parity.

The full release Rust/JNI/SDK fixture passes the original synchronous/asynchronous renewal checks, 33 ordering permutations, 34 exact software pixel comparisons and three same-origin committed HWUI pixel comparisons. It exercises two reentrant genuine add/reparent callbacks, newly promoted flattened text, native paint/hit ordering, held touch without cancellation, editor focus/selection, attached frames/LayoutParams, real collection contact/focus pins and stream retirement, pruning and new incarnations. Pixel comparisons use retained versus fresh candidate owners; they are not a separate baseline-image oracle. Actual accessibility services were disabled; the enabled-service branch is source-identical to the original indexed body and remains unverified with a running service. Source/product/SDK and complete raw/statistical audits pass independently. Post-run cleanup stops only the test applications; the final observed boot ID and installed baseline APK are recorded.

The applied native-ordering source also passes the required default-member build, full tests (3,548 passed, zero failed, 34 ignored), strict Clippy, formatting, staged caps and boot. The initial apparatus run omitted Bun from PATH and failed Bun-dependent tests/boot; its logs are preserved. After restoring the established Bun path, the entire serial suite passed with all 5,852 tracked non-Markdown source inputs unchanged. No assertion/source changes were needed. This does not claim full-workspace, fresh iOS performance or enabled-accessibility-service verification.


## Withdrawn bitmap preparation and worker text feasibility

The separate `pair-image-prepare-r1` experiment adds the public `Bitmap.prepareToDraw` call only after an accepted software bitmap enters the existing cache, before delivery. Both fixture-free release APKs use the accepted native-ordering source, unchanged native payload and original Heavy workload; only NativeImages, DEX and the generated ART profile differ. The original collectors complete four balanced AB/BA pairs per speed (16 fresh processes) with no tracing, video, forced GC or data reset. Source/product/runtime and full raw/statistical audits pass independently, including thermal status zero, integer scroll acknowledgment and final baseline APK restoration.

| Speed dp/s | Main-thread CPU, baseline → candidate | Process CPU, baseline → candidate | PSS endpoint MiB, baseline → candidate |
|---:|---:|---:|---:|
| 1000 | 27.74 → 27.78% | 96.40 → 95.97% | 215.75 → 220.39 |
| 24000 | 59.41 → 59.23% | 128.01 → 128.53% | 342.15 → 343.54 |

Slow process CPU falls 0.45% relative, with every paired process CPU change lower, but UI CPU has mixed signs and PSS rises in every pair (2.15% between medians). Fast process CPU rises 0.40%, while main-thread CPU has mixed signs and its paired-change median is effectively zero. N=4, mixed timing endpoints and normal GC do not establish a general speedup, startup benefit, equivalence, or retained/peak memory improvement. Window observations are not presented FPS or verified frame drops. The one-line change is withdrawn and absent from production. Its actual release SDK differential still passes 420 software pixel pairs, 420 same-origin committed HWUI pairs, eight lifecycle phases and the full original Rust/JNI renewal checks without tolerance; correctness alone does not justify the extra SDK queue/texture work. Full image-probe lint remains blocked by the unchanged Activity's existing GestureBackNavigation issue; assembleRelease and lintVitalRelease pass.

An independent isolated release SDK probe constructs the existing imperative Compose Paragraph engine on its owning worker Looper, with a separate main-owned painter and copied request/scalar payloads. On Galaxy it passes 1,268 valid bit-exact measurement triples, 14 matching invalid-input rejections, 50 software pixel pairs, owner-guard/wake checks and real static-font replacement across two density contexts. Its release build and full lint pass. That initial probe proves narrow construction/measurement feasibility only: main painters still seed their Source through their own measurement. Later immutable Source and ended-Picture correctness proofs are described below; real async font invalidation, complete Rust/JNI runtime migration, native-control barriers and performance benefits remain unverified. Worker termination was not observed; the probe process is stopped after recording its result. No worker or text-source-publication change is applied to production.


## Excluded ownership experiments and allocator comparison

The later immutable-source SDK probe permits independent measurement and painting owners without transferring Paragraph, TextPaint or font-resolver objects. It passes exact scalar/software comparisons, stale publication checks and observed worker termination. A separate ended-Picture probe passes software and same-origin committed HWUI comparisons. These are correctness proofs, not measured speedups. Its fresh worker painters do not prove that painting on a reused measurement engine leaves uncached metric answers unchanged.

The whole-session owner prototype is not integrated. Three actual SDK rounds exercise original JNI startup/renewal, native fling, one stable-window async publication, copied control queries and committed pixels, but the complete fixture fails its second async-admission search. The last capture identifies a conservative coverage guard rejecting a valid Float32-to-native-pixel projection: its error is 0.500103 pixels against a 0.5-pixel threshold. That fixture is stopped; its later close/barrier checks remain unexecuted. Production collection reports already use logical row heights and widths; the snapped row start appears only in the excluded coverage prototypes and does not cause demonstrated production row-height feedback churn. Normal active flings remain synchronous in the prototype, so moving ownership alone cannot establish a process CPU win.

The separate allocator cohort compares fresh System and MiMalloc release libraries built from the same frozen Rust sources, literal baked plan/compat bytes, original Heavy provider and common loader behavior. MiMalloc uses the same Android-target v2/local-dynamic-TLS configuration explored from main, with PURGE_DELAY=0 before library loading. Both apps retain the same Kotlin code, original dataset, harness, signing, viewport and requested/acknowledged movement. Four balanced AB/BA pairs per speed complete all 16 fresh-process phases at 1k and 24k dp/s; original 8s intervals and 1.5s warmup, thermal status zero, no forced GC, data/cache reset, video or profiling. Independent source/native/product/smoke/collector/raw audits pass, including observed Live ON updates and final accepted APK restoration.

| Speed dp/s | System → Mi UI CPU (% of one core) | System → Mi process CPU (% of one core) | System → Mi PSS after interval (MiB) | System → Mi process-to-ready (ms) |
|---:|---:|---:|---:|---:|
|1000|27.95 →27.73|96.82 →96.79|215.96 →208.39|192.76 →176.55|
|24000|59.64 →59.45|127.87 →128.24|344.45 →379.15|190.99 →175.42|

MiMalloc is withdrawn: process CPU is effectively unchanged, while fast endpoint PSS rises 34.70 MiB (+10.07%) and RSS rises 13.47%, with every pair higher. Fast used Java heap also rises in every pair. Ready medians improve about 16 ms, but this is the original renderer-ready proxy, not TTI or independently presented first content. APK overhead is 117.93 KiB. Separate cohorts are not additive, and endpoint memory is not peak, retained or leaked bytes.

An additional excluded public-API accounting diagnostic has one fresh fast-scroll process per allocator. Startup→Measure includes warmup and records approximately 9.5s, rather than exactly the timed 8s. It observes System 6 GC cycles/122ms against Mi 5/98ms, nearly equal Java allocation deltas (140.55M/140.39M bytes), different freed-byte deltas (139.02M/128.19M) and final Java occupancy (9.08M/19.69M). This makes GC cadence a possible contributor, not a causal explanation or a quantitative attribution of the earlier PSS gap. Unsupported public ART counters remain null/unavailable. Platform/Bionic native-heap accounting omits the private MiMalloc Rust heap, so smaller Debug native-heap values cannot establish smaller total native memory. No diagnostic CPU/ready result enters the ranking. A first partial collector attempt rejected valid optional nulls; its failure and cleanup are retained separately from the complete repaired capture.


## Updating geometry without reconciling retained containment

`Presenter.frame` now skips marking a materialized carrier's parent for containment reconciliation. Its native carrier is permanent, and moving or resizing it cannot make it eligible for flattened grouping. Unmaterialized nodes still re-evaluate their grouping geometry; creation, parenting, roots, styles, renewal, ranking and destruction retain their own invalidations. Native placement, control placement, fractional text-width offers, transforms, accessibility and layout are unchanged. This uses the distinction already present in `Presenter.present` and iOS frame handling; it adds no cache or scheduling state.

The separate `pair-frame-containment-r1` release cohort compares the accepted native-ordering APK with this one-condition change: four balanced AB/BA pairs per speed, 16 fresh processes, Galaxy A57, the original Heavy data and assets, Live OFF, eight-second intervals after 1.5 seconds of warmup. The 147-input projects differ only in Presenter; native library, harness, assets, certificate, viewport and acknowledged integer motion are identical. No profiler, video, forced GC, trace or data reset enters the cohort. All thermal states are zero, and the complete raw arithmetic and cleanup are independently checked.

| Speed dp/s | UI CPU, baseline → candidate (% of one core) | Process CPU, baseline → candidate (% of one core) | Endpoint PSS, baseline → candidate (MiB) | Process-to-ready, baseline → candidate (ms) |
|---:|---:|---:|---:|---:|
| 1000 | 28.01 → 27.97 | 97.91 → 97.17 | 215.66 → 215.42 | 192.55 → 194.66 |
| 24000 | 59.80 → 59.52 | 128.52 → 127.53 | 341.20 → 340.95 | 192.13 → 193.76 |

Fast process CPU falls 0.77% between medians, with all four paired changes lower (0.11–0.96%); UI CPU falls 0.47% with one pair higher. Slow process CPU falls 0.75%, with three of four pairs lower and mixed UI results. Endpoint memory is effectively unchanged. Ready proxies are slightly higher, and Window quality is mixed (fast duration p95 rises 1.29%, while median deadline-overrun counts fall from 5 to 4). This is a small generic removal of redundant work, not native parity, a demonstrated startup or memory improvement, presented FPS, or a statistically established speedup. The four-pair ranges are not confidence intervals, and separate cohorts are not additive.

The actual release Rust/JNI/SDK fixture passes the original synchronous/asynchronous renewal and attachment corpus (33 orders, 34 exact software comparisons and three same-origin HWUI comparisons). Nine additional software/geometry cases and three strict same-origin HWUI cases cover retained frame movement/resize, native control placement, clipped flattened-descendant accessibility geometry, promotion and renewal, fractional text-width offers and moved hit targets. Retained-versus-fresh candidate owners are the oracle; no separately compiled baseline-image oracle or enabled accessibility-service run is claimed. The original Activity also starts with Live OFF and observed Live ON mutations. These correctness probes are excluded from timing.
