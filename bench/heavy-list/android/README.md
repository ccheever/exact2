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

The stable-window placement path already exists in `runner/src/instance/collection/within.rs`: unchanged port/pins/sizes and retained coverage can move the native scroller without publishing another layout. Importing LazyColumn wholesale would duplicate this mechanism. The useful remaining Compose lesson is sharing text layout between measurement and drawing. C9 rich text currently builds metric-only and painted Paragraphs separately; unchanged source metric leases already survive renewal, so simply enlarging another cache does not remove that work.

Removing first-layout duplication needs an Android staging protocol: derive a prepared inline paint descriptor snapshot before layout, pass it to the native measurement owner without publishing UI state, and permit an owner-specific styled Paragraph to serve both a width measurement and a subsequent matching draw. Keep scalar measurement keys independent of paint; validate source incarnation, metric body, paint/environment, font resolution and width against the accepted draw descriptor before reuse, and fall back if motion, palette or later publication changes it. The existing public Compose Paragraph API supports creating/measuring/painting such an object, but does not accept UIKit-style cached line-break ranges or let callers replace arbitrary span paint afterward. This broader protocol change remains follow-up work; it has not been implemented or assigned a measured benefit.

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
