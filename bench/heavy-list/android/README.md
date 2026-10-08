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
