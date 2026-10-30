# Heavy list: GPUI on the Mac

@ref LLP 1149 §4; G1.

**All requested sample slots are filled: five interleaved fling/rest pairs and
ten fresh launches per app. Only three pairs and three launches per app meet
the quiet rule.** The final continuation completed two pairs and five cold-only
pairs without a focus loss or retake. It ran immediately, as requested, while
other work compiled: Cargo/rustc appeared in every monitor sample, the
one-minute load reached 38.31, and all new launches had `fair` thermal endpoints.
The tables include the full set, with the quiet and loaded subsets identified
separately. This completes collection, not the quiet-machine comparison.

## Arms and provenance

- Native Exact: this checkout at `f41aaafec4`, `exact-heavylist`, built with
  `bun host/apple/build.mjs exact-heavylist-apple --release` and
  `EXACT_APP_DIR=$PWD/bench/heavy-list/exact-heavylist`.
- GPUI app source: `a0f6fe8523` on `lane/gpui-arm`. The three retained pairs
  used G1's original probe at report commit `11896dd84d`. The final continuation
  uses the focus-aware probe and instrumented release bundles from `c237d4a22a`.
  Only `measure.m` was rebuilt for this report: after a failed focus retake it
  continues to the next sample. Neither application's code nor the probe was
  rebuilt. Both instrumented bundles have the same signed probe SHA-256,
  `6cd6e36fc30a5a8c53801a881c092e1ee77bc0a9b1b3b7b3d839a347035bf235`.
- GPUI: **0.2.2**, pinned with `=0.2.2` and this directory's own `Cargo.lock`.
  The [crates.io registry index](https://index.crates.io/gp/ui/gpui), checked
  2026-10-28, lists no newer non-yanked release. Native Metal, build-compiled
  shaders; `runtime_shaders` and `macos-blade` are disabled. Published package
  metadata names upstream commit `69e2130295c2649963eb639fc70b4f2ee8ea1624`
  with `dirty: true`; the registry checksum in `Cargo.lock` identifies the
  exact published source, rather than claiming that commit alone reproduces it.
- Apple M5 Max, 18 CPU cores, 40 GPU cores, 128 GB; Mac17,6;
  macOS 27.0.1 (26A434). Built-in Liquid Retina XDR, 3456×2234 pixels,
  1728×1117 logical points, 120 Hz. Both outer windows are 800×800 points at 2×;
  GPUI reserves 32 points for its title bar, matching the native safe-area inset.
- Data: `prepare.sh`, Pillow 11.3.0, 10,000 messages and 104 JPEGs, checked
  against `data.sha256`. `messages.json` SHA-256:
  `0b63b79ca113dfc23e52fdc9f4b653890bd48a59110f917fe3cae80c730a30dd`.

## Method

Quiet means **every load average is below 30 and no `cargo`, `rustc`, `swiftc`,
`clang`, `xcodebuild` or `metal` process is running**, regardless of its owner.
Compiler executables `swift-frontend` and `clang++` count too. An idle
`codex exec` process is not a reason to refuse the series. Check `uptime` and
`ps` before launching and monitor the same conditions during measurement.
For this continuation Charlie asked to run immediately, without the 20-minute
wait, and report whatever completed. A completed sample that violates the quiet
rule is reported as loaded, not counted as clean. The desktop must be unlocked
so each window can become active and remain in front for the whole fling/rest sample;
waking the display alone does not unlock it. G1m sampled load and compiler
processes every five seconds, retaining `quiet.jsonl` beside each attempt.

The driver calls `NSRunningApplication.activateWithOptions:` with
`NSApplicationActivateIgnoringOtherApps` as soon as it launches each process,
again when its window registers, and before every fling. A signal asks the
probe to make that window key and return its window number, key-window number,
active state and `NSWorkspace.sharedWorkspace.frontmostApplication`. The driver
requires the target PID to be frontmost and its window to be key before posting.
The probe records focus losses during cold startup, fling and rest through
application/window notifications, with display-callback and one-second checks
as well. Each loss records the frontmost application's name, bundle ID and PID.
A loss discards that process's sample and permits one fresh launch, named
`-retry`; a second loss skips that sample and continues with the next one.
The rejected JSON retains the foreground application at the loss. Activation
does not prevent another application from taking focus afterward.

`measure.m` launches fresh processes for five interleaved rounds, alternating
native/GPUI and GPUI/native. Both receive the same four 600-point flings at
0, 2.5, 5 and 7.5 seconds through the unmodified `host/apple/wheel.swift` helper:
`CGEvent.postToPid`, phase-bearing trackpad events and decaying momentum
(LLP 1142 D3). The fling sample lasts ten seconds, followed by ten seconds at
rest. The probe records received wheel-event counts, summed deltas and the final
scroll position. AppKit’s asynchronous scrolling bypasses some local event
monitor calls; those counts are not an input-delivery comparison.

`probe.m` is linked into copies of both bundles with the existing
`probe/linkprobe.py`; neither process uses `DYLD_INSERT_LIBRARIES`. The start
time comes from `proc_pid_rusage`’s `ri_proc_start_abstime`, converted with
`mach_timebase_info` to the monotonic clock used by `CACurrentMediaTime`.
GPUI's first complete scene has no pending image requests; the hook on
`CAMetalLayer.nextDrawable` attaches an `MTLDrawable.addPresentedHandler` and
reads `presentedTime`. The native milestone is the first display-link layer
snapshot within mean 1/255 of the settled 100×100 snapshot at two seconds,
after verifying that a scroll view and nonblank content exist. It observes
cached layers without forcing an AppKit draw. These cold milestones differ:
the native result is an observation on a display turn, not a drawable timestamp.

Both processes run a main-run-loop display-link sampler at the screen's
maximum rate during the fling. It records both the link timestamp and callback
delivery time. This measures callback cadence, not proof that new pixels reached
the display. GPUI additionally records actual drawable presentation timestamps;
those are reported separately, within the recorded gesture windows so that
intentional idle gaps are not counted as dropped frames. Late means an interval
above 1.5 display periods.
No input-to-photon latency is inferred from either series.

Process CPU and physical footprint use `proc_pid_rusage(RUSAGE_INFO_V4)`;
main-thread CPU uses `thread_info(THREAD_BASIC_INFO)` on the main thread's Mach
port. The process CPU counters are Mach ticks, converted with
`mach_timebase_info`; see XNU’s [fill_task_rusage](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/kern/bsd_kern.c).
CPU is cumulative-counter delta divided by elapsed sample time, in ms/s,
with one-second samples retained as well as each phase’s aggregate. The retained
samples reject inactive start/end samples, inactive display callbacks during
the fling, or differing start/end thermal states; they have no continuous rest
focus record. The corrected probe also records focus changes at rest. Thermals
remain endpoint observations; a `fair` endpoint is recorded, not treated as
nominal.
Fling footprint is sampled each display callback; rest footprint once per second.
Lifetime peak and end footprint are retained separately. The display-link sampler
is paused at rest. Samples include probe overhead and exclude WindowServer's CPU.

## Parity and limits

The pilot's `Application`, `ListState` and variable-height `list` pattern are
reused. Its generated chat bubbles, command channel and programmatic scroll are
replaced by the shared heavy-list data, the specified row and native wheel input.
Images use ImageIO thumbnail decoding on GPUI's background executor, keyed by
filename and displayed pixel dimensions, then center-cropped to the tile.
The GPUI list adds the native Mac list’s 16-point outer inset, keeping SPEC’s
16-point padding inside each row. It premeasures an 800-point margin so wheel
input can advance beyond the currently measured rows. The line boxes round
the platform ascent and descent separately, as ExactCore’s CSSLineBox does.
Reaction counts are keyed by message id and emoji; numeric counts use tabular digits in the platform font.

Known differences that prevent interpreting this as a renderer ranking:

- GPUI owns its element tree, Taffy layout, virtualization, text shaping,
  glyph atlas, rendering and presentation. Native Exact owns a kernel/runner
  and AppKit views with Core Animation composition. Their caches, worker work
  and application startup paths are part of these whole-process measurements.
- Exact's initial rows are baked into its plan; GPUI parses the shared JSON
  at startup. Both retain all 10,000 messages and load the same image bytes.
- GPUI 0.2.2's `TextRun` has no per-run font size. Code runs use Menlo at
  16 pt in the 16 pt text flow instead of SPEC's 15 pt. They keep their inline
  background and participate in the same wrapping paragraph.
- Native AppKit and GPUI have different scrolling and row-retention machinery.
  Equal wheel input does not guarantee equal offsets or equal numbers of rows
  laid out. Scrollbar presentation also differs.
- Live mode is outside G1's static-feed run. GPUI refuses `BENCH_LIVE=1`;
  it does not substitute a static feed while claiming live results.
- GPUI's public element API here supplies no native row accessibility labels.
  This is a benchmark window, not a qualified accessible application.
- The two cold milestones and the cadence/presentation distinction above remain
  non-comparable. Driver and filesystem caches are not purged between launches.

The supplied brief references LLP 1149 §4.1, but this checkout's r3 has §4
followed by §5. Its requested exact list of non-comparables is unavailable here;
the list above names the differences established from the implementation.

## Whole app

**Full collection; quiet qualification incomplete.** Values are the median
(minimum–maximum) of all five completed fling/rest pairs, with ten launches per
app for cold start. Three pairs were quiet with nominal thermal endpoints; the
two new pairs and five cold-only pairs ran under compilation with fair endpoints.
These pooled summaries describe the completed slots; they are not estimates
from one controlled machine condition. The subsets below show the difference.
All launches used warm system caches and had Low Power Mode off.

| Metric | Native Apple host | GPUI 0.2.2 |
|---|---:|---:|
| Cold to complete-screen milestone, ms; **10 launches each** | 463.56 (421.71–957.02) | 373.42 (318.66–586.66) |
| Fling process CPU, ms/s | 124.16 (115.96–192.38) | 93.55 (88.60–124.59) |
| Fling main-thread CPU, ms/s | 82.69 (79.82–135.53) | 45.15 (43.14–58.42) |
| Peak footprint during fling, MiB | 218.61 (197.08–230.61) | 398.16 (396.16–409.02) |
| Lifetime peak footprint through end of rest, MiB | 220.75 (211.20–234.08) | 417.89 (403.80–427.88) |
| End-of-fling footprint, MiB | 215.05 (177.50–225.58) | 388.97 (387.58–396.80) |
| Rest process CPU, ms/s | 1.113 (1.076–1.464) | 5.113 (4.796–6.378) |
| Rest main-thread CPU, ms/s | 0.491 (0.467–0.787) | 3.301 (3.048–4.094) |
| End-of-rest footprint, MiB | 214.69 (176.81–224.97) | 209.38 (204.47–213.72) |

Across these completed slots, GPUI used 30.60 ms/s less process CPU during the
fling (24.6%) and 37.54 ms/s less main-thread CPU (45.4%). Native Exact used
179.55 MiB less peak fling footprint (45.1%), 197.14 MiB less lifetime peak
(47.2%) and 173.92 MiB less end-of-fling footprint (44.7%). Native Exact's
rest process/main-thread CPU was lower by 4.000/2.811 ms/s (78.2%/85.1%).
GPUI's end-of-rest footprint was 5.31 MiB lower (2.5%), within the native runs'
spread. GPUI's cold milestone was 90.13 ms earlier (19.4%), but the milestones
differ: this is not a startup win measured with one instrument. Percentages
use the higher median as the denominator. Compilation, thermal conditions and
different scroll work prevent a controlled performance or renderer ranking.

The subsets are kept apart here. The quiet column has three launches and three
fling/rest samples per app. The loaded column has seven launches and two
fling/rest samples per app. Values remain median (minimum–maximum). The probe
revision also changed between subsets; these tables do not isolate compilation's cost.

| Metric | Native, quiet | GPUI, quiet | Native, loaded | GPUI, loaded |
|---|---:|---:|---:|---:|
| Cold to complete-screen milestone, ms | 596.84 (441.92–621.55) | 338.49 (318.66–447.50) | 434.88 (421.71–957.02) | 377.89 (352.90–586.66) |
| Fling process CPU, ms/s | 121.41 (115.96–124.16) | 89.47 (88.60–93.55) | 178.90 (165.41–192.38) | 120.71 (116.84–124.59) |
| Fling main-thread CPU, ms/s | 81.77 (79.82–82.69) | 43.15 (43.14–45.15) | 126.40 (117.27–135.53) | 56.23 (54.05–58.42) |
| Peak footprint during fling, MiB | 224.16 (197.08–230.61) | 397.50 (396.16–398.16) | 214.81 (211.00–218.61) | 408.20 (407.38–409.02) |
| Lifetime peak footprint through end of rest, MiB | 226.38 (214.88–234.08) | 413.33 (403.80–417.89) | 215.98 (211.20–220.75) | 427.12 (426.36–427.88) |
| End-of-fling footprint, MiB | 216.86 (188.27–225.58) | 388.56 (387.58–388.97) | 196.27 (177.50–215.05) | 395.97 (395.14–396.80) |
| Rest process CPU, ms/s | 1.084 (1.076–1.113) | 5.113 (4.796–5.401) | 1.436 (1.408–1.464) | 5.610 (4.841–6.378) |
| Rest main-thread CPU, ms/s | 0.471 (0.467–0.491) | 3.301 (3.071–3.474) | 0.747 (0.708–0.787) | 3.571 (3.048–4.094) |
| End-of-rest footprint, MiB | 216.27 (187.78–224.97) | 208.52 (204.47–209.38) | 195.75 (176.81–214.69) | 212.83 (211.94–213.72) |

In the three quiet pairs, GPUI's fling process/main-thread CPU was lower by
26.3%/47.2%; native Exact's peak fling footprint was lower by 43.6% and its
rest process CPU by 78.8%. The loaded pairs have higher CPU rates in both apps.
No loaded sample contributes to the quiet columns.

Equal input produced native final offsets of 8,454, 8,318, 8,366, 7,264 and
8,344 points; GPUI ended at item 18, offset 255, in all five runs. The driver
posted four gestures per sample, each with 106 events and an 8.33 ms nominal
period. The apps do not establish equal scroll work from those inputs.

**The frame intervals use different instruments.** The native probe records
display-link cadence, not presented-frame timestamps. GPUI also records
`MTLDrawable.presentedTime`. Native cadence and GPUI presentation cannot
establish which app presents more smoothly; native presented-frame intervals
and late frames remain unmeasured. The first table covers all five pairs;
the second separates the three quiet pairs from the two loaded pairs.

| Arm and instrument | Interval p50, ms | p95, ms | p99, ms | Late intervals/s |
|---|---:|---:|---:|---:|
| Native, display-link timestamp | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–17.56) | 0.30 (0.00–3.30) |
| GPUI, display-link timestamp | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 16.67 (8.33–16.74) | 1.30 (0.10–4.70) |
| Native, presented frame | unmeasured | unmeasured | unmeasured | unmeasured |
| GPUI, presented frame within gesture windows | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–16.67) | 0.44 (0.22–1.38) |

| Subset and instrument | Interval p50, ms | p95, ms | p99, ms | Late intervals/s |
|---|---:|---:|---:|---:|
| Quiet, native display link | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 0.10 (0.00–0.30) |
| Quiet, GPUI display link | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–16.67) | 0.30 (0.10–1.30) |
| Quiet, GPUI presentation | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 0.44 (0.22–0.44) |
| Loaded, native display link | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 17.31 (17.06–17.56) | 3.10 (2.90–3.30) |
| Loaded, GPUI display link | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 16.72 (16.71–16.74) | 4.10 (3.50–4.70) |
| Loaded, GPUI presentation | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 16.67 (16.67–16.67) | 1.32 (1.25–1.38) |

Each run's quantiles use sorted intervals at index `floor(p × count)`, capped
at the final index; the tables take medians and ranges of those per-run values.
Late means strictly greater than 12.5 ms on this 120 Hz display. Display-link
late counts are divided by the whole ten-second fling sample. GPUI presentation
intervals join timestamps only within each individual gesture window; late
counts are divided by those windows' combined durations, 4.534–5.061 seconds.
Each run supplies 481–506 such intervals. Across all five pairs, display-link
p50 and p95 tied at this precision; native p99 was 8.33 ms lower and lateness
was 1.00 interval/s lower at the median. Neither observation establishes a
presented-frame winner.

## Sample accounting

The final continuation, `target/g1/series-g1m8`, ran on 2026-10-28 from
22:29:37 to 22:31:33 PDT, without a wait. `caffeinate` held the display awake
throughout. Both drivers exited 0; all 14 new launches passed focus checks,
with no focus loss, discarded sample or retake. There is no foreground taker
to report for this continuation. Four fling/rest samples each verified focus
at startup and before all four gestures; the ten cold-only samples each
verified it at startup. All windows were 800×800 points, 2×, at 120 Hz.

| Sample slots | Raw paths under `target/g1/` | Completed pairs | Quiet pairs | Thermal endpoints |
|---|---|---:|---:|---|
| Fling 1 | `series-g1m4/fling/{native,gpui}-1.json` | 1 | 1 | nominal |
| Fling 2–3 | `series-g1m5/fling/{native,gpui}-{1,2}.json` | 2 | 2 | nominal |
| Fling 4–5 | `series-g1m8/fling/{native,gpui}-{4,5}.json` | 2 | 0 | fair |
| Cold-only 6–10 | `series-g1m8/cold/{native,gpui}-{6,…,10}.json` | 5 | 0 | fair |

The combined totals are **five completed fling/rest pairs and ten launches per
app; three quiet fling/rest pairs and three quiet launches per app**. The
original three pairs retain their earlier focus criteria, including the lack
of continuous rest focus observation noted above. Fling order was native/GPUI,
native/GPUI, GPUI/native, GPUI/native, native/GPUI; cold-only order alternated
starting with GPUI/native.

Every one of the continuation's 19 fling monitor observations and six cold
observations saw Cargo/rustc. The maximum 1/5/15-minute loads were
38.31/17.07/12.40 during fling and 28.22/17.00/12.40 during cold-only launches.
Cargo PID 9284 and rustc PID 13556 persisted throughout; additional compiler
PIDs are in each phase's `quiet.jsonl`. No process belonging to another session
was stopped. The quiet rule therefore excludes every new sample from the clean
count, even though focus held and all requested gestures and launches finished.

Earlier rejected attempts do not enter any table. Their raw directories
`series-g1m4` through `series-g1m7` remain under `target/g1/`; `series-g1m7`
identified Google Chrome (`com.google.Chrome`, PID 60411) during a fling and
Ocho (`dev.ocho.desktop`, PID 53948) during its cold retake. The earlier focus
diagnostics also identified Finder and Slack. The logs identify foreground
applications, not who activated them.

## Where the native host's fling CPU goes

@ref LLP 1149 §4.1; G1x. Measured on the same Mac at `1f473fd60344`, with
`BENCH_LIVE=0`, fresh native processes, 800×800-point windows and G1's four
600-point flings. **Sampling puts native view setup and drawing at about
31 ms/s; kernel layout and measurement at 3 ms/s; runner, row and batch work
at 6 ms/s. There are no data-source queries during the measured phases.**
Core Animation and image processing account for much of the remaining CPU.

These are new measurements: G1's earlier processes cannot be profiled
retroactively. Three release captures average **143.08 ms/s during fling and
1.269 ms/s at rest**, versus G1's earlier medians of 124.16 and 1.113.
The table gives the arithmetic mean (minimum–maximum) across runs so the
exclusive categories sum to the measured total. CPU includes all app threads
and the probe, and excludes WindowServer and GPU execution.

| Exclusive category | Fling CPU estimate, ms/s | Rest CPU estimate, ms/s |
|---|---:|---:|
| Native view creation, batch application, configuration and text setup | 22.62 (18.83–24.76) | 0 sampled |
| Native text rasterization and view drawing | 8.81 (7.47–9.59) | 0 sampled |
| Kernel layout and text measurement | 2.98 (2.78–3.12) | 0 sampled |
| Runner commit, row machinery, host batch projection and wire decoding | 5.92 (5.15–6.35) | 0.63 (0.33–1.05) |
| Data-source query | 0 asks; 0 sampled | 0 asks; 0 sampled |
| Bind evaluation | 0.42 (0.21–0.54) | 0 sampled |
| Remaining platform, image, scroll, probe and unclassified work | 102.32 (89.39–116.03) | 0.64 (0.30–0.88) |
| **Process total, measured by counter** | **143.08 (124.10–159.48)** | **1.269 (1.216–1.355)** |

The first row is broader than constructors or work on newly entering rows:
it includes applying batches to existing views, paragraph setup and retirement.
`NodeView.init` alone accounts for **0.61 ms/s (0.36–1.04)**, property
application for 1.25 and style application for 1.05. These are inclusive
subsets, not additional costs. The kernel category includes **1.90 ms/s
(1.61–2.21)** under `CallbackMeasurer`, including Core Text; the rest is
layout. Text rasterization is in the separate drawing category.

The 102.32 ms/s remainder consists of **46.59 Core Animation, 20.98 image
processing, 12.07 scroll and host scheduling, 2.06 probe, and 20.62 other**.
The last category includes AppKit, dispatch, pixel conversion and stacks without
a recognized owner. Asynchronous image and Core Animation work cannot be
assigned back to a particular entering row from these stacks. This is an
accounting of execution domains, not a measurement of the savings from removing
native rows.

Time Profiler sampled running threads at 1 ms. Each stack was assigned once;
each category's share of sampled weight was multiplied by that phase's process
CPU counter delta divided by elapsed time. The three fling captures contain
1,780, 1,400 and 1,180 samples; their ten-second rest captures contain only
11, 45 and 17. Thus the rest split is approximately **0.6 ms/s in runner/batch
work and 0.6 ms/s elsewhere**, with no sampled native apply, layout or binding
work. Zero samples do not establish zero CPU. A longer rest capture failed
the focus check before measurement; the 73-sample split is the available
estimate, not a precise decomposition of G1's earlier 1.113 ms/s.

Classification checks for probe/query frames first, then kernel layout and
measurement, bindings, remaining runner/kernel and batch frames, native apply
and text setup, native drawing, image processing, Core Animation, scroll
scheduling, and other. Examples of the matching symbols are `CallbackMeasurer`,
`exact_kernel::layout`, `taffy::compute`, `emit_bindings`, `eval_binding`,
`exact_runner`, `exact_apple::host`, `Batch.decode`, `Presenter.apply`,
`NodeView.applyProps`, `TextRaster`, `Raster`, `IIOImage`, `CA::`, and
`CollectionHost`. Earlier categories own nested work: layout called by a commit
is charged to layout, not both. Optimized inlining and incomplete stacks limit
the boundary between binding, commit and unclassified work.

Release and development runs were interleaved three times; the second release
was replaced afterward because its thermal endpoints changed. All retained
captures passed continuous focus checks, had Low Power Mode off, and met G1's
quiet rule: no compiler processes and observed one-minute loads of 8.93–19.89.
The first pair had `fair` thermal endpoints; the others were nominal. These
are not three repetitions under identical thermal or scroll-work conditions.

| Release capture under `target/g1x/` | Fling CPU, ms/s | Rest CPU, ms/s | Final offset, pt | Thermal endpoints |
|---|---:|---:|---:|---|
| `release-1b` | 159.48 | 1.216 | 9,048 | fair |
| `release-2-thermal-retry` | 145.65 | 1.355 | 8,887 | nominal |
| `release-3-retry` | 124.10 | 1.236 | 8,285 | nominal |

One accepted release control without Time Profiler used 103.07 ms/s during
fling and 1.367 at rest, ending at 6,956 points (`control-1-retry`). Its lower
scroll distance prevents isolating profiler overhead from changed work. Do not
use the profiled totals as an uninstrumented regression comparison with G1 or
as a new GPUI comparison. Focus-lost attempts and `release-2` (fair→nominal)
are excluded; failed launches and the unqualified longer-rest attempt remain
in the local logs.

Development traces supply work counts by plan site, not CPU time. The three
captures are `development-1`, `development-2`, and `development-3-retry`.
Subtracting each `before` snapshot from its `fling` snapshot gives:

| Work / plan site in `exact-heavylist/app.contract` | Run 1 | Run 2 | Run 3 |
|---|---:|---:|---:|
| Runner-clock interval bracketing fling, s | 10.749 | 10.500 | 10.750 |
| Transaction sequence advances | 176 | 161 | 146 |
| Changed-tree epochs | 77 | 69 | 57 |
| Kernel layout entries | 175 | 156 | 135 |
| List at line 92: rows constructed / retired | 47 / 39 | 39 / 35 | 33 / 28 |
| List at line 92: rows rebound | 0 | 0 | 0 |
| MessageRow root at line 102: binding evaluations | 94 | 78 | 66 |
| Inline text at line 121: instances created / binding evaluations | 634 / 5,072 | 476 / 3,808 | 408 / 3,264 |
| Reaction buttons at line 138: instances created / binding evaluations | 82 / 164 | 66 / 132 | 63 / 126 |
| All plan sites: node instances created / retired | 1,571 / 1,322 | 1,212 / 1,034 | 1,044 / 844 |
| All plan sites: binding evaluations | 5,836 | 4,422 | 3,830 |
| Data-source asks since measurement began | 0 | 0 | 0 |

These are plan-node counts, not AppKit object counts. The source map joined
without truncation, with plan digest
`589d6ae60c1b87d6e775b602ce1fecf97ff3f296577c85e57a04b9b59816e1e2`.
Most binding evaluations belong to newly constructed inline text; no retained
row was rebound. `messages()` was baked into the plan, and `config()` completed
before the baseline. There is no query-per-row problem in this static feed.

At rest the corresponding 10.001, 10.001 and 9.750-second trace intervals
advance the transaction sequence by **50, 50 and 48**, with **zero** changed
epochs, layouts, node creation/retirement, binding evaluations or queries.
The app's `every(250, tick)` and `every(1000, second)` still run with live mode
off. `SessionClock` advances the runner; `applyUnlessEmpty` avoids native
application of empty batches. Gating these tasks after the launch config says
`live=false` is a concrete candidate for eliminating otherwise idle runner
work. Its savings have not been measured. The fling's 135–175 layout entries
and 57–77 changed epochs suggest examining collection-update coalescing, but
the counters do not prove that any layout or commit was redundant.

The native trace also records scoped main-thread wall intervals for runtime,
apply, rows, text, images and scrolling. Its frame ring retains only 600
records, roughly the last five seconds at 120 Hz; its unattributed intervals
include idle time and unwrapped platform work. Neither these intervals nor
`perf` counters are a process CPU flamegraph. They cannot price individual
plan sites, attribute worker CPU, or establish presented-frame timing. Trace
export adds margins around the development count intervals. The development
sampler also keeps callbacks active at rest: those processes consumed
10.46–12.81 ms/s there, so release captures supply the CPU table.

Ink's Mac carrier (LLP 1149 S1) replaces per-node AppKit views and their
configuration, native text drawing and per-row layer materialization with
instance lists. The 22.62 + 8.81 ms/s native categories identify work it would
replace, not net savings: Ink has its own encoding, rasterization and upload
costs. Some of the Core Animation work may disappear, but window composition,
scrolling and image decoding remain, and these captures cannot separate the
removable fraction. Kernel layout/measurement, runner commits and binding
belong to Exact above the carrier. GPUI also pays for its own layout, text and
virtualization, but this app does not pay for Contract VM evaluation, Exact's
runner-to-host batch projection/wire decoding, or an AppKit view tree for the
rows. The sample categories do not isolate all of that architectural overhead
from the useful row work within them.

To reproduce, build both variants in this worktree's `target/`, copying the
release bundle before the development build replaces it:

```sh
export EXACT_APP_DIR="$PWD/bench/heavy-list/exact-heavylist"
export CARGO_TARGET_DIR="$PWD/target"
bun host/apple/build.mjs exact-heavylist-apple --release
ditto 'target/clients/2dcc39fdac58f699849328fc/dev.exact.heavybench.exact/macos/Heavy list.app' target/g1x/Release.app
bun host/apple/build.mjs exact-heavylist-apple
ditto 'target/clients/2dcc39fdac58f699849328fc/dev.exact.heavybench.exact/macos/Heavy list.app' target/g1x/Development.app
BENCH_LIVE=0 bun scripts/agent.mjs macos --size 800x800 --timing platform \
  'clock settle' 'perf messages during "tap messages wheel 0 600 fling" "clock +1500 real"' state
```

The agent command passed and reported six constructed rows. It is a separate
functional drive: agent mode changes scheduling. For the timed runs, link
G1's existing `target/g1/probe.dylib` into each bundle copy and ad-hoc sign it
as described under Reproduction. Launch the executable normally with
`BENCH_LIVE=0`, `EXACT_WINDOW_WIDTH=800`, `EXACT_WINDOW_HEIGHT=800`,
`G1_ARM=native`, and `G1_OUTPUT` naming a fresh absolute JSON path; unset
`EXACT_AGENT` and `DYLD_INSERT_LIBRARIES`. Wait for its `.window` and `.ready`
sidecars, activate the app, then send `SIGUSR1` and verify its `.focus` reply.
Monitor the quiet rule every five seconds and reject any recorded focus loss.

In one terminal, attach before the timed phases and wait for “Ctrl-C to stop”:

```sh
xcrun xctrace record --template 'Time Profiler' --attach "$g1x_pid" \
  --time-limit 30s --no-prompt --output "$g1x_capture.trace"
```

While it records, use another terminal for the development baseline:

```sh
bun scripts/agent.mjs trace --pid "$g1x_pid" --out "$g1x_capture-before.trace.json"
```

After the baseline export and a 0.5-second margin, send `SIGUSR2` to start
the probe's fling phase. Its `.fling` sidecar contains
the monotonic `time` used for the four input deadlines. At that time plus
0, 2.5, 5 and 7.5 seconds, run:

```sh
target/g1/exact-wheel "$(cat "$G1_OUTPUT.ready")"
```

Each call must report `posted: 106, periodMs: 8.33`. At start +10 seconds,
send `SIGUSR2` again to finish fling and start rest (`.rest` sidecar). Export
the development `-fling.trace.json` then; at rest +10 seconds export
`-rest.trace.json` and send `SIGUSR2` to finish. Release runs omit those
exports and finish at rest +10 seconds. Stop the profiler with SIGINT after
the app exits. Record `time.time() - time.monotonic()` before launch to align
Time Profiler's start date and nanosecond sample offsets with probe markers.
Interleave release/development three times, retaining raw phase and focus data.

```sh
xcrun xctrace export --input "$g1x_capture.trace" \
  --xpath '/trace-toc/run[@number="1"]/data/table[@schema="time-profile"]' \
  --output "$g1x_capture.xml"
```

Resolve XML `id`/`ref` entries, demangle Rust frames with `rustfilt`, and include
only samples between each probe phase's start/end times. Apply the exclusive
classification above and normalize each run separately before averaging.
The local `target/g1x/` contains `.trace`, exported `.xml`, `.samples.json`,
probe `.json`, input `.meta.json`, five-second `.machine.jsonl`, development
`-{before,fling,rest}.trace.json` and `attribution.json`. These are ignored
measurement artifacts; this change adds no scripts or instrumentation.

Both native builds and the agent drive exited 0. `git diff --check` passed.
`bun scripts/caps.mjs` passed: “All budgets within cap. 10 categories inspected.”
G1x changes only this report; no workspace test suite was rerun.

## Renderer only

| Comparison | Status |
|---|---|
| GPUI renderer versus Ink headless paint, same heavy-list screen | Follow-up in the Ink lane; not measured by G1 |

## Window inspection

[Native at row zero](native-top.png) and [GPUI at row zero](gpui-top.png) are
1600×1600 captures of the production builds at 2×. Both show Ava, Ben and
Chloé with the same row widths, avatar circles, photo crops and grid gaps.
GPUI has a conventional titled window; the native host uses a unified title bar.
Core Text shaping and raster output differ in some spacing, and the code-run
size gap above remains visible. Separate GPUI captures inspected the quote at
row 18, link card at row 8 and three-photo grid at row 20. A ten-second
functional wheel drive reached row 18; it was not a qualifying timing run.
A posted mouse click changed the first reaction in row 8 from 39 to 40.
The link description ends with an ellipsis after two lines.

## Checks

For this continuation, `measure.m` compiled with `-O2 -Wall -Wextra -Werror`
and AppKit, QuartzCore and CoreGraphics. The existing probe and instrumented
bundles were reused. Both measurement drivers exited 0. All 14 new raw JSON
records were checked for focus, size, scale, display rate, Low Power Mode and
thermal endpoints; all four fling/rest records had monotonic CPU counters,
ten-second phases and four identical 106-event gestures. No focus retake was
needed, so the changed second-loss continuation branch was not exercised here.
The earlier focus-fix diagnostics in `c237d4a22a` established loss identification
and one successful retake; they tested the old stop-on-second-loss behavior.
`git diff --check` and `bun scripts/caps.mjs` passed:
“All budgets within cap. 10 categories inspected.”
No workspace test suite was rerun for this continuation.

G1 (`6ffede6aa8`) records the following commands passing on this worktree.
These are G1's results, not new G1m test runs. G1m changes the standalone
benchmark driver, probe and report; workspace source, lockfiles and the six
checks are unchanged.

- `cargo build --all-targets --keep-going`
- `cargo test --tests --no-fail-fast` (full rerun)
- `cargo clippy --all-targets --keep-going -- -D warnings`
- `cargo fmt --all -- --check`
- `bun scripts/caps.mjs`
- `bun scripts/boot.mjs`
- `cargo check --workspace --all-targets --keep-going`
- `cargo test -p exact-linux --lib --tests --no-fail-fast`

The first root test run failed four existing wall-clock limits in
`exact-svg-file` and `exact-tamber` while compilation was running. The full
warm rerun passed without changes to those sources. Caps reported
“29 crates and 1 binary” checked for wasm32 with zero errors and
“All budgets within cap. 10 categories inspected.”

The separate GPUI workspace built with `--release --locked`; its formatting
check passed. The Objective-C driver and probe compiled with
`-O2 -Wall -Wextra -Werror`. Cargo reported future incompatibility notices
for the upstream `block 0.1.6` and `proc-macro-error2 2.0.1` dependencies.

Named suites were run separately. Each line below is the command’s test
summary, verbatim. The first seven use `--test pinned`; `image` uses
`--test it`.

`paint`

```text
test result: ok. 55 passed; 0 failed; 1 ignored; 0 measured; 48 filtered out; finished in 3.46s
```

`borders`

```text
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out; finished in 0.67s
```

`tint`

```text
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out; finished in 1.43s
```

`backdrop`

```text
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 3.62s
```

`gradients`

```text
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 1.11s
```

`visual`

```text
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 96 filtered out; finished in 2.47s
```

`materials`

```text
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 2.17s
```

`image`

```text
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 7.48s
```

## Reproduction

Build from the repository root. All Cargo commands use this worktree's `target/`;
GPUI's dependency graph never enters the root lockfile.

```sh
mkdir -p target/g1
bun install --frozen-lockfile
bench/heavy-list/prepare.sh
cargo build --release --locked --manifest-path bench/heavy-list/gpui/Cargo.toml --target-dir target
EXACT_APP_DIR="$PWD/bench/heavy-list/exact-heavylist" CARGO_TARGET_DIR="$PWD/target" \
  bun host/apple/build.mjs exact-heavylist-apple --release
xcrun swiftc -O host/apple/wheel.swift -o target/g1/exact-wheel
xcrun clang -fobjc-arc -O2 -Wall -Wextra -Werror -framework AppKit \
  -framework QuartzCore -framework CoreGraphics bench/heavy-list/gpui/measure.m -o target/g1/measure
xcrun clang -dynamiclib -fobjc-arc -O2 -Wall -Wextra -Werror -Wno-unused-parameter \
  -mmacosx-version-min=14.0 -framework AppKit -framework QuartzCore -framework Metal \
  bench/heavy-list/gpui/probe.m -o target/g1/probe.dylib \
  -install_name @executable_path/../Frameworks/probe.dylib
```

Bundle `target/release/gpui-heavy-list` as a Mac application with its executable
under `Contents/MacOS` and the generated `data/` under `Contents/Resources/data`.
Its bundle metadata uses identifier `dev.exact.heavybench.gpui`, executable
`gpui-heavy-list`, package type `APPL`, minimum macOS `14.0` and
`NSHighResolutionCapable=true`.
Copy the native release bundle into `target/g1/`. In each copy put the probe in
`Contents/Frameworks/probe.dylib`, append its load command with
`python3 bench/heavy-list/probe/linkprobe.py <executable> @executable_path/../Frameworks/probe.dylib`,
and ad-hoc sign the copy with `codesign --force --deep --sign - <bundle>`.
Run `target/g1/measure <fresh-absolute-results-dir> <native-executable>
<gpui-executable> <absolute-path-to-exact-wheel>` when nothing is building.
The terminal needs Accessibility access for the existing wheel helper. Raw JSON
and logs go to the result directory. Capture parity windows separately with
`screencapture -x -o -l <window-number> <image.png>`; the ready sidecar names
the window. Screenshot capture needs Screen Recording access and is excluded
from the timed series.

The instrumented release copies, corrected driver/probe and wheel helper are
already built in this worktree. The final continuation used these commands
with five-second load and compiler monitoring saved to each phase's `quiet.jsonl`.
Do not overwrite these result directories. For a fresh full series, use a new
result directory with `5 1`, followed by `5 6 cold-only`; use quiet-machine
results for a controlled comparison.

```sh
target/g1/measure "$PWD/target/g1/series-g1m8/fling" \
  "$PWD/target/g1/Native.app/Contents/MacOS/Heavy list" \
  "$PWD/target/g1/GPUI.app/Contents/MacOS/gpui-heavy-list" \
  "$PWD/target/g1/exact-wheel" 2 4
target/g1/measure "$PWD/target/g1/series-g1m8/cold" \
  "$PWD/target/g1/Native.app/Contents/MacOS/Heavy list" \
  "$PWD/target/g1/GPUI.app/Contents/MacOS/gpui-heavy-list" \
  "$PWD/target/g1/exact-wheel" 5 6 cold-only
```

## G3: window to window

@ref LLP 1149 §4.1, D7, D12; measured 2026-10-29 at `52e617043c` on
`lane/ink-s1`. These are whole-window measurements of the native Apple host,
GPUI 0.2.2 and S1.1's Ink benchmark carrier presenting through direct Metal.
The native and GPUI release builds were made in this worktree; Ink used S1.1's
release bundle. The renderer-only G2a run is superseded by this comparison.
G1's tables above remain their own measurements.

The machine is the same M5 Max, Mac17,6, 18 CPU cores and 128 GiB,
macOS 27.0.1 (26A434), with the built-in 120 Hz display. Every qualified
sample verified an **800×800-point outer window at 2×**. The native content
view includes its 32-point title-bar safe area; GPUI and Ink have 800×768
content beneath their title bars. The S1.1 driver set Ink's content height
to 800, making its outer window 832 points tall. For this run only, the
driver was compiled with Ink's height set to 768; the exact adjustment is
below. No carrier, renderer, app or tracked driver source changed.

All three bundles contain the same linked probe, signed SHA-256
`b05ec540cc55014785584e922d0dbf43ec310fb817ad49585d22d01c99a22709`.
The data passed `data.sha256`. Live mode was off. G1's process CPU,
main-thread CPU, footprint and wheel instruments are unchanged: four
600-point flings at 0, 2.5, 5 and 7.5 seconds, ten seconds of fling and ten
at rest. Each gesture reported 106 events at an 8.33 ms nominal period.
The probe is included in CPU and footprint; WindowServer and GPU execution
are outside the process CPU counters.

### Three-arm results

Values are **median (minimum–maximum)** across five qualified fling/rest
samples and ten qualified fresh-process launches per arm. The first five
launches also supply the fling/rest samples; slots 6–10 are cold-only.
Application and driver caches were retained, as in G1: “cold” means a new
process, not an empty driver or filesystem cache. Startup failures below
are excluded from these successful-launch timing distributions.

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal |
|---|---:|---:|---:|
| Cold to first complete frame, ms; native snapshot / GPUI and Ink presented drawable | 458.57 (386.23–493.05) | 386.03 (336.84–506.50) | 375.42 (343.21–439.01) |
| Fling process CPU, ms/s | 119.01 (113.99–124.02) | 92.35 (90.02–94.82) | 569.06 (557.31–584.39) |
| Fling main-thread CPU, ms/s | 82.47 (78.57–86.02) | 44.47 (42.67–46.32) | 531.09 (516.72–539.77) |
| Rest process CPU, ms/s | 1.315 (1.170–1.395) | 5.485 (5.370–5.767) | 859.811 (855.368–861.521) |
| Rest main-thread CPU, ms/s | 0.644 (0.556–0.707) | 3.586 (3.525–3.760) | 859.744 (855.301–861.454) |
| Peak footprint during fling, MiB | 226.81 (215.85–234.47) | 399.34 (394.70–413.19) | 575.59 (573.03–577.53) |
| End-of-fling footprint, MiB | 220.03 (212.33–228.72) | 387.77 (386.50–395.19) | 225.95 (223.72–230.31) |
| End-of-rest footprint, MiB | 219.35 (211.64–227.97) | 207.64 (203.84–215.03) | 218.59 (216.39–223.02) |

Ink and GPUI use the **same `CAMetalLayer.nextDrawable` hook and
`MTLDrawable.presentedTime`**. In every qualified launch here, their first
presented drawable was also their first complete scene with requested images
loaded. The native milestone remains G1's layer-snapshot observation on a
display turn. Its cold number is not measured with the same instrument.

| Arm and instrument | Fling interval p50, ms | p95, ms | p99, ms | Late intervals/s |
|---|---:|---:|---:|---:|
| Native, display-link cadence | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 0.30 (0.10–0.50) |
| GPUI, presented drawables | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–16.67) | 0.66 (0.22–1.77) |
| Ink Metal, presented drawables | 8.33 (8.33–8.33) | 16.67 (16.67–16.67) | 16.67 (16.67–25.00) | 12.12 (11.24–16.98) |

Native presented-frame intervals remain unmeasured. Its cadence cannot rank
presentation smoothness against the other two. Quantiles use G1's
`floor(p × count)` index and are summarized across runs. Late means strictly
over 12.5 ms. Drawable intervals join timestamps only inside each recorded
gesture window, excluding intentional idle gaps between gestures; their
late counts are divided by those windows' combined durations. GPUI supplies
464–490 intervals over 4.530–4.545 seconds per run; Ink supplies 300–339 over
4.535–4.538 seconds. Native cadence covers the whole ten-second fling.
No input-to-photon latency is inferred.

Equal input still produces different scroll work. Native's five final
offsets are 8,267, 8,622, 7,838, 8,689 and 8,326 points. GPUI ends at item 18,
within-item offsets 55, 255, 255, 205 and 255. Ink ends at 6,172 points in
every run; its probe's `withinItem` field carries the whole-list offset.
These measurements do not normalize CPU by rows entered or distance traveled.

### Qualification and failed attempts

The primary series ran from **07:28:37 to 07:41:31 PDT**. All 123 five-second
machine observations met G1m's quiet rule: no compiler processes, with maximum
1/5/15-minute loads **16.59/24.87/18.07**, all below 30. Thus all 15 qualified
fling/rest samples and all 30 qualified launches are quiet. Low Power Mode
was off. Thermal endpoints were `fair` except native/GPUI fling slot 5 and
Ink slots 4–5, which were `nominal`; every retained sample had matching
endpoints. Quiet does not mean one thermal state throughout the series.

The driver verified activation, the frontmost PID and the key window at
startup and before every gesture. **No focus loss or foreground taker was
recorded**, including during the failed startups. There were no focus retakes.
The retained raw attempts, including failures, are under
`target/g3/series-20261029/`:

| Slots | Directory | Disposition |
|---|---|---|
| Fling 1–2, all arms | `fling` | retained |
| Ink 3 | `fling`, then `round3-ink-retry` | complete-frame timeout; one successful retake |
| Native/GPUI 3 | `round3-native-gpui` | retained |
| Native/GPUI 4 | `round4-5` | retained |
| Ink 4 | `round4-5`, then `round4-ink-thermal-retry` | fair→nominal; excluded and replaced once |
| Fling 5, all arms | `round5` | retained |
| Ink cold 6 | `cold`, then `cold6-ink-retry` | complete-frame timeout; one successful retake |
| Native/GPUI cold 6 | `cold6-pair` | retained |
| Cold 7–10 | `cold7-10` | retained except Ink 10 |
| Ink cold 10 | `cold7-10`, then `cold10-ink-retry` | complete-frame timeout; one successful retake |

Each timeout was the probe's 30-second wait for the first complete presented
frame; the app had submitted a frame and passed the focus handshake. It is
not evidence that nothing was visible. The driver exits on this error, so
collection resumed in fresh directories after one retake for the failed slot.
There were **three startup timeouts among 14 direct-Metal process attempts**:
ten qualified launches, one thermal-changing launch and those three failures.
The median cold number therefore does not establish reliable startup.

Round order was native/GPUI/Ink, GPUI/Ink/native, Ink/native/GPUI,
native/GPUI/Ink, GPUI/Ink/native, with the replacements above. Cold-only
order was Ink/GPUI/native for slot 6, then native/GPUI/Ink,
GPUI/Ink/native, Ink/native/GPUI and native/GPUI/Ink. Every attempt keeps
its driver log, focus sidecars and five-second `quiet.jsonl`; completed
attempts also keep the probe JSON. `summary.json` lists the qualified paths
and derived metrics.

### Optional Ink arm: wgpu

The same carrier and probe ran with `EXACT_INK_DEVICE=wgpu` after the primary
series, from **07:41:31 to 07:47:44 PDT**. This arm is not interleaved with
the primary arms. **Four of five fling/rest slots and nine of ten launch
slots qualified.** Values summarize those completed slots only.

| Metric | Ink, wgpu on Metal |
|---|---:|
| Cold to first complete presented drawable, ms; 9 launches | 450.72 (376.91–632.46) |
| Fling process CPU, ms/s; 4 samples | 584.91 (539.95–595.83) |
| Fling main-thread CPU, ms/s | 541.13 (494.49–550.44) |
| Rest process CPU, ms/s | 862.404 (855.976–911.027) |
| Rest main-thread CPU, ms/s | 862.333 (855.085–910.938) |
| Peak footprint during fling, MiB | 590.20 (584.61–596.61) |
| End-of-fling footprint, MiB | 231.28 (229.95–236.41) |
| End-of-rest footprint, MiB | 222.64 (221.08–229.00) |
| Presented interval p50, ms | 16.67 (16.67–16.67) |
| Presented interval p95, ms | 25.00 (25.00–25.00) |
| Presented interval p99, ms | 41.67 (33.33–41.67) |
| Late presented intervals/s | 34.72 (34.15–35.48) |

All 89 machine observations were quiet, with maximum 1/5/15-minute loads
13.73/11.29/13.23. All retained thermal endpoints were `fair`, Low Power
Mode was off, and there were no recorded focus losses or foreground takers.
The same 30-second complete-frame timeout occurred on slots 2, 3, 4, 7 and
8. One retake recovered each except slot 4, whose retake also timed out;
collection continued without a third attempt. That is **six startup timeouts
among 15 process attempts**, with nine qualified launches. All six failed
launches passed the focus handshake. The raw records are under
`target/g3/wgpu-20261029/`: `fling` holds slot 1 and slot 2's failed launch;
`roundN` and `roundN-retry` hold the continuation. `summary.json` lists the
qualified records. Each retained first drawable was also the first complete
scene. The four fling samples provide 186–197 presentation intervals over
4.535–4.539 gesture seconds and end at 6,171–6,172 scroll points.

### Window parity and non-comparables

The new [native](g3-native-top.png), [GPUI](g3-gpui-top.png) and
[Ink Metal](g3-ink-top.png) screenshots are unscaled 1600×1600 window captures,
all at row zero, taken outside the timed series and visually inspected.
They show the same Ava, Ben and Chloé messages, avatars, styled runs and
two-photo grid, with loaded images rather than placeholders. They do not
establish pixel parity: Ink wraps Ava's first paragraph into four lines
where native and GPUI use five, its emoji and code face differ, its row
separators span the window, and the photos' visible crops differ. Native
and GPUI also differ in some text spacing and title-bar treatment, including
GPUI's 16-point code runs against the specified 15 points. The differing row
heights also change how much content fits in the viewport.

| Difference | Consequence for this comparison |
|---|---|
| Ink is S1.1's benchmark carrier, with no accessibility, IME or menus yet | Its numbers do not qualify a shipping Apple presenter or price those missing features. |
| Ink uses Parley shaping and swash rasterization; GPUI and the native host use platform text | Font metrics, wrapping, glyph appearance and text work differ; Core Text's Ink audit has not been substituted into this run. |
| GPUI owns its element tree, Taffy layout, virtualization and platform layer, with no Exact kernel or runner | Whole-process differences cannot be assigned to the GPU renderer alone. |
| The native Apple host carries AppKit views and Core Animation on this Mac; the Apple host uses UIKit on iOS | G1x's split above identifies native setup/drawing, kernel, runner and platform work. G3 does not reprofile or subtract those categories. |
| Native and Ink use the baked Contract app; GPUI parses the shared JSON at startup | Boot includes different application work despite identical source data. |
| Scroll distance, row geometry, caches and scheduling differ | Equal wheel events are a reproducible input, not proof of equal work. |
| Native startup and cadence have different instruments | Only GPUI and Ink's drawable timestamps support a direct presentation comparison. |

**Finding.** GPUI beats the current Ink carrier on fling CPU, presentation
pacing and peak footprint on this screen: direct Metal uses about 6.2 times
GPUI's fling CPU, twice its p95 frame interval and 44% more peak memory.
Ink's end-of-fling footprint is close to native's and below GPUI's, but its
rest CPU is nearly one core, almost entirely on the main thread. That rest
cost and the complete-frame timeouts occur on both Ink device arms; their
cause was not localized in this measurement. Direct Metal presents more
smoothly than the later, incomplete wgpu set. Successful direct-Metal and
GPUI cold medians are close with overlapping ranges; Ink's failed launches
prevent claiming reliable startup superiority. These are findings about
the complete current windows, including their differing work and carrier
features, rather than an isolated renderer ranking.

### Reproduction and validation

G1's build, bundling and probe-linking steps above apply. The native release
build used `EXACT_APP_DIR` and `CARGO_TARGET_DIR` pointing inside this
worktree; GPUI used `--release --locked --target-dir target`. S1.1's Ink
release carrier can be rebuilt with:

```sh
EXACT_APP_DIR="$PWD/bench/heavy-list/exact-heavylist" \
EXACT_PAINTED_PROBE="$PWD/bench/heavy-list/gpui/probe.m" \
  bun host/apple/build.mjs exact-heavylist-apple --painted-benchmark --release
```

The instrumented copies used here are `target/g3/{Native,GPUI,Ink}.app`.
Their probes live at `Contents/Resources/benchmark-probe.dylib`, linked as
`@executable_path/../Resources/benchmark-probe.dylib`, then ad-hoc signed.
To reproduce the outer-window size adjustment without editing the driver:

```sh
python3 - <<'PY'
from pathlib import Path
import subprocess
source = Path('bench/heavy-list/gpui/measure.m').read_text()
old = 'env[@"EXACT_WINDOW_HEIGHT"] = @"800";'
assert source.count(old) == 1
source = source.replace(old,
    'env[@"EXACT_WINDOW_HEIGHT"] = [arm isEqual:@"ink"] ? @"768" : @"800";')
subprocess.run(['xcrun', 'clang', '-x', 'objective-c', '-fobjc-arc', '-O2',
    '-Wall', '-Wextra', '-Werror', '-framework', 'AppKit', '-framework',
    'QuartzCore', '-framework', 'CoreGraphics', '-', '-o', 'target/g3/measure'],
    input=source, text=True, check=True)
PY
```

With fresh result directories, no compiler processes and all loads below 30:

```sh
G3_PAINTED_BIN="$PWD/target/g3/Ink.app/Contents/MacOS/Heavy list" \
EXACT_INK_DEVICE=metal target/g3/measure "$PWD/target/g3/new-series/fling" \
  "$PWD/target/g3/Native.app/Contents/MacOS/Heavy list" \
  "$PWD/target/g3/GPUI.app/Contents/MacOS/gpui-heavy-list" \
  "$PWD/target/g3/exact-wheel" 5 1
G3_PAINTED_BIN="$PWD/target/g3/Ink.app/Contents/MacOS/Heavy list" \
EXACT_INK_DEVICE=metal target/g3/measure "$PWD/target/g3/new-series/cold" \
  "$PWD/target/g3/Native.app/Contents/MacOS/Heavy list" \
  "$PWD/target/g3/GPUI.app/Contents/MacOS/gpui-heavy-list" \
  "$PWD/target/g3/exact-wheel" 5 6 cold-only
```

Use `G3_ONLY_INK=1 EXACT_INK_DEVICE=wgpu` for the optional continuation.
Keep `EXACT_AGENT`, `EXACT_AGENT_SOCKET`, `DYLD_INSERT_LIBRARIES`,
`BENCH_START_INDEX`, `EXACT_FONT`, `EXACT_ROWS` and `EXACT_ASSETS` unset.
Monitor load and compiler processes every five seconds into `quiet.jsonl`;
hold the display awake. On a startup or thermal failure, retain the failed
attempt, use a fresh directory for one retake, and continue remaining slots.
The per-directory `command.json` records each actual continuation command
and selected device. Screenshots use G1's window capture procedure separately.

Both new release builds passed. The driver and probe compiled with
`-O2 -Wall -Wextra -Werror` (the probe also uses `-Wno-unused-parameter`).
Validation checked all 39 qualified launches and 19 fling/rest samples for
size, scale, display rate, focus, Low Power Mode and stable thermal endpoints;
all 19 had four 106-event gestures, ten-second phases and monotonic CPU
counters. Three new screenshots were inspected. No workspace or pinned test
suite was rerun for this report-only change.

`git diff --check` passed. `bun scripts/caps.mjs` passed:

```text
All budgets within cap. 10 categories inspected.
```

## S1.2: idle and startup fixes; cadence and footprint remain open

@ref LLP 1149 D7, D11, S1.2; 2026-10-29, `lane/ink-s1`, following
`17f6bbccbc`. **S1.2 is incomplete.** The expired-timer spin is fixed and
ten launches of the retained direct-Metal carrier presented a complete scene
without a timeout. Presentation cadence and peak footprint still miss the
requested targets. A quiet measurement of the final retained code remains
outstanding; the quiet trial below used a pacing variant that was rejected.
No three-arm G3 rerun was made.

### Changes and evidence

`host/apple-painted/src/window.rs` now delivers due live timers before
choosing its next wait. Previously, a disabled periodic timer retained an
expired deadline: `WaitUntil` repeatedly returned immediately without
advancing the timer. The regression test checks that delivering this timer
advances its deadline without damage or a frame, and leaves an agent's frozen
clock alone. The display-link callback posts only when there is frame demand;
idle turns use `Wait`, or `WaitUntil` for a future timer.

The carrier leaves damage pending while a window frame is in flight.
`host/ink/src/device/apple.rs` owns a single frame lease; the Metal and wgpu
presentation workers release it after GPU completion and wake the carrier.
Its unit test checks exclusivity, release on the worker and the wake.
`device/metal.rs` no longer drains a window submission when waiting for an
already completed submission, including submission zero. Drawable acquisition,
presentation and completion waits remain on the render worker. The existing
`finish_for(Window)` path already avoided pixel readback; screenshots still
request `Pixels`. Both device arms passed a functional agent drive with a
scroll and screenshot; direct Metal's screenshot was visually inspected.

The startup failure was reproduced with images loaded and the last drawable's
`presentedTime` equal to zero: GPU completion did not mean the drawable reached
the screen. The presenter then had no damage to submit again. The direct-Metal
path now observes each drawable's presented callback and requests another frame
when it was discarded. This uses Apple's
[presented-time contract](https://developer.apple.com/documentation/metal/mtldrawable/presentedtime).
The unchanged probe observed the first complete presented scene in **10/10
fresh launches**, with no timeout and no main-thread drawable acquisition.
Those final launches ran during other compilation, so their durations are
not qualified startup performance measurements. Their records are
`target/s1-2/cold-retained/ink-{1..10}.json`.

### Ink-only measurements and their limits

The G3 instrument and 800×800-point outer window at 2× were retained, with
`G3_ONLY_INK=1` and direct Metal. The following pair is one quiet baseline
and one quiet trial, not a distribution. Four gestures and the ten-second
fling/rest phases are unchanged. All 14 machine observations had no compilers;
maximum 1/5/15-minute loads were 10.81/10.79/10.21. Both samples had nominal
thermal endpoints, Low Power Mode off and no recorded focus loss.

**The trial column is discarded code**, using a view CADisplayLink and drawing
directly on its event. Its lower CPU does not establish the final carrier's
cost: its pacing was worse. The retained code uses CVDisplayLink and the
window's redraw event. Raw records are `target/s1-2/before-final/ink-1.json`,
`after-final/ink-1.json` and `final-series-quiet.jsonl`; the directory name
`after-final` does not denote the retained implementation.

| Metric | S1.1 baseline | Discarded S1.2 trial |
|---|---:|---:|
| Rest process / main-thread CPU, ms/s | 858.854 / 858.811 | 1.578 / 1.486 |
| Fling process / main-thread CPU, ms/s | 555.955 / 520.138 | 73.062 / 42.243 |
| Presented interval p50 / p95 / p99, ms | 8.33 / 16.67 / 16.67 | 16.67 / 25.00 / 33.33 |
| Late presented intervals/s | 11.00 | 36.13 |
| Peak / end-of-fling footprint, MiB | 564.59 / 219.95 | 576.30 / 231.22 |
| End-of-rest footprint, MiB | 212.89 | 222.67 |

Intervals use G3's within-gesture calculation: 345 intervals over 4.545
seconds for the baseline, 190 over 4.540 seconds for the rejected trial.
Neither sample acquired drawables on the main thread.

An earlier diagnostic of the retained CVDisplayLink scheduling approach
reported rest CPU 1.618/1.489 ms/s, fling CPU 105.016/64.561 ms/s, presented
p95 16.67 ms, 12.33 late intervals/s and peak footprint 566.50 MiB
(`target/s1-2/flight-diagnostic-2/ink-1.json`). That build had diagnostic
logging and preceded the discarded-drawable retry and final gesture guard;
it is **not a qualified measurement of the final code**.

Three pacing variants were examined: keeping CVDisplayLink running through
the gesture and completion, drawing directly on the tick, and a view
CADisplayLink requesting 120 Hz. The first was retained after the other two
failed to improve cadence. Work stopped at the requested three-round limit.
The next investigation is the completion/display-tick handoff: pending damage
can wait for another pulse after the in-flight frame completes. Other builds
subsequently drove machine load above 240; a final quiet before/after pair
could not be collected. The requested 8.33 ms p95 is not established.

### Peak footprint is unresolved

The existing ImageIO path already decodes thumbnails with
`kCGImageSourceThumbnailMaxPixelSize`. Temporary diagnostics found accounted
image ownership within the roughly 64 MiB painter budget, about 34 MiB of
current bitmap pixels, about 0.5 MiB of glyphs and no pooled layers. Metal's
reported current resource allocation reached about 96 MiB. These counters
do not explain the process peak: a live `vmmap` showed 341 MiB of owned,
unmapped graphics memory during the transient.

Isolated diagnostics also showed that a 100×100 Metal clear increased process
footprint from about 4 MiB to 181 MiB. This identifies substantial driver cost,
but does not attribute the whole carrier transient. Separately, decoding the
large photos to 1536-pixel ImageIO thumbnails peaked near 100 MiB; the
768-pixel case stayed near 16 MiB. Thus thumbnail decoding is not exonerated
by its bounded retained pixels. Autorelease scoping and a simpler presentation
shader did not reduce the measured carrier peak; the shader change was
discarded. No decode-policy change is included, and no peak-memory improvement
is claimed. Diagnostic logs, source and `vmmap` captures remain under
`target/s1-2/`; temporary instrumentation is absent from the commit.

### Validation

The six repository checks passed: root build, tests, clippy and fmt, caps,
boot, and workspace all-target compilation. Caps includes the wasm32 check.
Ink and carrier all-target clippy also passed. Both release and development
carrier builds passed. The agent drove the development carrier through
`clock settle`, a 600-point wheel input, state and screenshot on direct Metal
and wgpu (`target/s1-2/drive-metal-dev.log` and `drive-wgpu.log`).

The final full Linux run used `RUST_TEST_THREADS=2 EXACT_INK_DEVICE=metal
cargo test -p exact-linux --lib --tests --no-fail-fast`. Two preceding runs
at default parallelism failed different wall-clock timeouts in networking
and worker startup during heavy machine load. Those failures are not
classified as known upstream failures. The third run passed, including every
pinned suite (`paint`, `borders`, `tint`, `backdrop`, `gradients`, `visual`,
`materials`) and `it::image`; no additional filtered runs are claimed.
Verbatim summaries for Linux lib, `it`, `pinned`, Ink and carrier lib:

```text
test result: ok. 1575 passed; 0 failed; 20 ignored; 0 measured; 0 filtered out; finished in 130.71s
test result: ok. 130 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.64s
test result: ok. 104 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 17.27s
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.09s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.96s
```

Complete logs are `target/s1-2/checks-*.log`; `checks-linux-final.log` is the
passing Linux run, and `checks-ink.log` and `checks-carrier-all.log` supply
the last two summaries. `QUEUE.md` records the unresolved cadence and memory
work. The final quiet Ink-only comparison and the performance targets remain
required before S1.2 can be called complete.


## S1.3: overlap the walk with rendering; size thumbnails from their content boxes

@ref LLP 1149 D7, D11, S1.3; 2026-10-29, `lane/ink-s1`, following
`fe7a8c55fb`. The implementation is committed in `221593d2f8`, `a1b3a93560`
and `1cc26abd77`. **S1.3 is not performance-qualified:** S1.3m collected
the Ink-only series below, but no sample met G3's quiet rule. The quiet
comparison and allocation-level attribution remain open.

### Handoff and image ownership

`host/apple-painted/src/window.rs` and `host/ink/src/device/apple.rs` no
longer hold the next walk behind a completion lease. CVDisplayLink and the
existing idle-timer handling are retained. `host/ink/src/present.rs` hands
work to the render thread without waiting. Only its newest queued
presentation remains eligible to show; an older pending presentation resolves
its retirement fence without acquiring a drawable. Ordered uploads and
render commands remain queued because later frames can reference their
textures. The worker waits for the current presentation's GPU completion;
the main thread can walk and encode the next frame meanwhile. Instance,
atlas and layer reuse still follows completed submissions.

Both `device/metal.rs` and `device/wgpu.rs`, and their `present.rs` modules,
use the handoff. Metal uploads use two reusable staging buffers, each written
only after its preceding blit completes. Further uploads queue their owned
bytes on the worker. Tests hold the worker behind a channel and verify that
handoffs return, only the newest pending presentation shows, and resource
work stays ordered. A Metal test copies five odd-stride rows through two
staging buffers and checks the resulting pixels.

`host/linux/src/image.rs` resolves raster demand from the content box after
padding and borders, the painter's object-fit calculation, and device scale.
The existing 128-pixel size classes provide resize slack; the previous fit
and scale are retained for deciding when a request needs replacement. The
new test in `image/control_tests.rs` covers all five fits, padding, borders,
scale and resizing. A 380×100-point content box at 2× requests a 768-pixel
cover thumbnail, while contain requests 384 pixels for a 1600×900 source.
The wider G3 fixture can still require larger thumbnails: a full-width
photo has 716 points available after the row padding, avatar and gap,
or 1432 pixels at 2× before fitting. A universal 768-pixel cap would lose
resolution there.

Benchmark completeness and scroll metadata travel with the encoded frame
and are published immediately before drawable acquisition. `probe.m`'s
scroll fields are atomic because the presentation worker now writes them.
The wgpu functional drive exposed a readback race: mapping could precede the
worker's copy submission. `map_buffer_on_submit` now orders mapping after
that copy; the rebuilt wgpu drive passes.

### Measurement

**S1.3m, 2026-10-29: collected immediately, without a quiet wait.**
Direct Metal ran from 10:51:42 to 10:54:36 PDT, then wgpu on Metal from
10:54:36 to 10:57:30. Each device attempted five fling/rest samples and
five additional cold-only launches, with one retake for a lost slot.
Metal retained five fling/rest samples and four cold-only launches; wgpu
retained four and five. Both therefore have nine retained startup timings.

**None is quiet under G3's rule.** All 107 five-second machine observations
had no compiler processes, and the maximum one- and five-minute loads were
17.97 and 28.38. The fifteen-minute load remained above 30 throughout,
falling from 84.11 to 59.84. The new columns are **loaded diagnostics**.
Every retained process had `fair` thermal endpoints, Low Power Mode off,
successful activation/frontmost-PID/key-window checks, and no recorded
focus loss. Failed attempts are accounted for below.

The measured release bundle is `target/s1-3/After-final.app`, built from
`1cc26abd77` and unchanged at report commit `d4a699d10c`. Its executable
SHA-256 is `3c93e0a2842a443163de0d6f827ff0de3788b1f21a8c819a033021ed331ae60f`;
the linked probe's is
`beda56484e995e07991eb931c3530235f10a8b0d1fa96c38e4714c291532ae04`.
`target/s1-3/measure` uses G3's 800×768-point content viewport and one
attempt per invocation; a fresh directory holds each retake. Every retained
window measured 800×800 outer points at 2× on the same 120 Hz display.
`G3_ONLY_INK=1` selected Ink alone, with `EXACT_INK_DEVICE=metal`, then
`wgpu`. No app, renderer, driver or probe was rebuilt or changed.

The new columns are **median (minimum–maximum)**, with an identical rounded
range omitted. S1.1 gives the qualified direct-Metal G3 medians above.
S1.2 is its single retained-scheduling diagnostic, which predates its final
startup fixes and includes diagnostic logging. Neither earlier binary, nor the
native or GPUI arm, was rerun. These columns are not a paired comparison.

| Metric | S1.1 Metal baseline | S1.2 Metal diagnostic | After S1.3: Metal | After S1.3: wgpu |
|---|---:|---:|---:|---:|
| Presented interval p50, ms | 8.33 | 8.33 | 8.33 | 8.33 |
| Presented interval p95, ms | 16.67 | 16.67 | 16.67 | 16.67 |
| Presented interval p99, ms | 16.67 | 25.00 | 25.00 | 25.00 |
| Late presented intervals/s | 12.12 | 12.33 | 11.34 (10.86–12.37) | 11.93 (10.78–12.59) |
| Fling process CPU, ms/s | 569.06 | 105.016 | 105.34 (91.37–111.38) | 139.16 (128.31–155.98) |
| Fling main-thread CPU, ms/s | 531.09 | 64.561 | 52.57 (44.18–54.57) | 61.24 (56.79–66.30) |
| Rest process CPU, ms/s | 859.811 | 1.618 | 1.513 (1.438–12.294) | 1.930 (1.410–11.528) |
| Rest main-thread CPU, ms/s | 859.744 | 1.489 | 1.429 (1.365–9.177) | 1.689 (1.297–8.851) |
| Peak footprint during fling, MiB | 575.59 | 566.50 | 591.64 (589.24–597.39) | 594.45 (590.49–596.39) |
| End-of-fling footprint, MiB | 225.95 | 223.91 | 245.75 (240.14–248.14) | 239.37 (237.58–241.80) |
| End-of-rest footprint, MiB | 218.59 | 214.09 | 238.22 (235.80–239.94) | 231.56 (229.24–233.50) |
| Cold to first presented drawable, ms | 375.42; 10 launches | — | 539.76 (437.61–985.03); 9 launches | 517.38 (460.58–587.84); 9 launches |

Cold includes each retained fling process's startup, as G3 does. The
additional cold-only slots alone measured **533.01 (437.61–591.34) ms**
on Metal (four of five) and **538.11 (460.58–587.84) ms** on wgpu (five
of five). In every retained process, the first presented drawable was also
the first complete scene. Caches were retained: these are fresh processes,
not empty application- or driver-cache launches. Failed startups are excluded
from these timing distributions.

The same G3 instrument reads `MTLDrawable.presentedTime`. Intervals join
timestamps only within each recorded gesture window; quantiles use
`floor(p × count)` on those intervals. Late means strictly over 12.5 ms,
divided by the windows' combined durations. Metal supplied
325–336 intervals over 4.541–4.684 gesture seconds per sample; wgpu supplied
318–334 over 4.542–4.675 seconds. Each retained sample posted four
106-event flings at the helper's nominal 8.33 ms period, with ten seconds
of fling and ten at rest. Metal ended at 6172 points except one sample at
6186; wgpu ended at 6172 throughout. CPU is not normalized by distance.

**The cadence still does not hold 8.33 ms:** both devices have a 16.67 ms
p95 and a 25.00 ms p99. The fling peak did not fall: Metal's median is
16.05 MiB above S1.1 and 25.14 MiB above the S1.2 diagnostic; wgpu's is
4.26 MiB above its own S1.1 median of 590.20 MiB. Unequal load conditions
prevent attributing these differences to S1.3. Rest CPU medians remain
low, with the high samples retained in the ranges. The quiet comparison
remains outstanding.

### S1.3m sample accounting

All 26 process attempts remain under `target/s1-3m-20261029/`, including
the eight rejected attempts. Slots 1–5 are fling/rest; 6–10 are cold-only.
Activation and the key-window handshake preceded every launch sample and
every posted gesture. There was one retake per failed slot, with collection
continuing after a second failure.

| Device and slot | First attempt | One retake |
|---|---|---|
| Metal fling 3 | Focus lost to Slack during rest | Retained |
| Metal fling 5 | Focus lost to Zoom during rest | Retained |
| Metal cold 10 | Focus lost to Slack at startup | Same loss; slot skipped |
| wgpu fling 1 | Focus lost to Slack at startup | Retained |
| wgpu fling 2 | Complete-frame timeout; focus losses also recorded | Retained |
| wgpu fling 4 | Focus lost to Slack during fling | Complete-frame timeout with focus loss to Zoom; slot skipped |

The two wgpu timeouts occurred after a first submission, at the probe's
30-second complete-frame deadline. Their focus sidecars also record losses
(Slack, Heavy list and cmux for slot 2; Zoom for slot 4's retake), so this
series does not isolate a startup failure while continuously foreground.
Metal had no complete-frame timeout. The missing Metal cold slot and wgpu
fling slot were not replaced by further launches.

`provenance.json`, per-attempt `command.json`, driver/app logs, focus
sidecars, probe JSON and `quiet.jsonl` retain the inputs and failures;
`summary.json` records the accepted paths, per-sample metrics and aggregates.

### Memory attribution and the next chunk

A separate development-agent diagnostic at 800×768 points scrolled eight
600-point steps. This frozen-clock drive is not the timed G3 workload.
`target/s1-3/vmmap-scrolled.txt` reports 693.9 MiB current and 846.2 MiB
lifetime peak footprint, with 227.8 MiB resident owned-unmapped graphics,
63.2 MiB mapped IOAccelerator memory and 19.0 MiB IOSurfaces. The raster
state held 28.85 MiB of pixels and reported a 51.86 MiB ownership peak.
These are different counters, not an allocation decomposition of the process
peak. The drive's screenshot also requests readback; window presentation
does not.

This snapshot does not identify the allocator behind the owned-unmapped
graphics category. It cannot establish that this is the upload textures,
the driver's heaps, or drawable backing. Three 1600×1536 BGRA surfaces
contain 28.125 MiB of pixels before alignment and driver overhead; that
calculation alone cannot explain hundreds of MiB. Allocation-level Metal
profiling is still required to attribute that transient.

Decoded CPU pixels remain retained after GPU upload. Releasing that copy for
the GPU painter, and re-decoding when its texture is evicted, is the next
chunk; this change does not implement it.

### Validation

The six repository checks pass: root all-target build, root tests, clippy
and fmt, caps, boot, and workspace all-target compilation. Caps checked
29 crates and one binary for wasm32. Ink and carrier all-target clippy pass.
The release and development carrier builds pass. Agent drives on direct
Metal and wgpu settled, scrolled 600 points, and captured screenshots;
both screenshots were visually inspected. The additional Metal drive
accepted eight 600-point wheel inputs at G3's content size.

The first root test run failed `workers_start_with_the_first_request` at
its five-second completion bound. The full rerun with `RUST_TEST_THREADS=2`
passed. The first full Linux run passed; the run after the readback fix
failed two `build_channel` polling timeouts. Both failure logs are retained
and unclassified by `known.mjs`; they are not claimed as known upstream
failures. The full Linux rerun with `RUST_TEST_THREADS=2` passed.

Verbatim final full Linux summaries for lib, `it` and `pinned`:

```text
test result: ok. 1576 passed; 0 failed; 20 ignored; 0 measured; 0 filtered out; finished in 102.86s
test result: ok. 130 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.52s
test result: ok. 104 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 20.40s
```

The requested filtered suites ran with `EXACT_INK_DEVICE=metal EXACT_REQUIRED=1`.
Verbatim summaries, in order: pinned `paint`, `borders`, `tint`, `backdrop`,
`gradients`, `visual`, `materials`, then `it::image`:

```text
test result: ok. 56 passed; 0 failed; 1 ignored; 0 measured; 48 filtered out; finished in 6.38s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 4.84s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 4.17s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out; finished in 7.28s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 8.78s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 6.39s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 6.41s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 4.87s
```

Final Ink and carrier library summaries:

```text
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.10s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.14s
```

Full logs and screenshots are under `target/s1-3/`. `checks-test-retry.log`
is the passing root run; `checks-linux-retry.log` is the controlled Linux
rerun; `checks-ink-readback.log` follows the readback fix. Filtered-suite
logs are named `checks-<suite>-metal.log`.

S1.3m changes only this report. All 18 retained records passed the window,
focus, thermal and power checks; the nine fling/rest records also passed
gesture-count, phase-duration and monotonic-counter checks. The fixture's
`messages.json` and all 104 bundled images match `data.sha256`.
`git diff --check` and `bun scripts/caps.mjs` pass; the latter reports:

```text
All budgets within cap. 10 categories inspected.
```

The caps log is `target/s1-3m-20261029/caps.log`. The implementation test
summaries above belong to S1.3; no code test suite was rerun for S1.3m.

## S1.4 — trace the late frames (2026-10-29)

**No pacing or memory-policy change.** Image removal retains the late cadence.
Most late intervals have little main-thread work and a GPU completion before
the next nominal refresh. The missing evidence is which drawable Core Animation
selected, and who owns the unstacked graphics charge. Neither supports a
one-change prediction. This chunk stops at that boundary; S1 is not qualified.

### Recording and sample limits

The painted carrier did not implement the native host's trace exporter. It now
exposes opt-in intervals to the existing benchmark probe with `G1_TRACE=1`.
`probe.m` records Core Animation clock timestamps, thread IDs, presented times,
and dropped-event counts in fixed storage. It exports after the fling, on a
live-agent `SIGUSR1`, or when a rejected traced process receives `SIGTERM`.
The existing reader accepts the result:

```sh
bun scripts/agent.mjs trace target/s1-4/g3-before/ink-1.json.trace.json
```

`EXACT_AGENT=1 EXACT_AGENT_LIVE=1` keeps the wall clock and display link running.
A socket request such as `{"op":"tap","at":[400,400],"wheel":[0,11.3],"phase":"changed"}`
leaves damage for the window instead of the headless agent's synchronous pixel
readback. Use `began` and `ended` at the gesture boundaries. A regression test
checks that input changes scroll position, preserves pending damage and does
not advance the clock. Occlusion still suspends display work.

The ten-second socket drives sent 1,200 requests. Early drives included headless
readback; later ones omitted gesture phases or became occluded. They are not G3
cadence samples. Their fractional scrolling created oversized clip masks that
the posted G3 fling did not reproduce. Raw records remain under `target/s1-4/`.

G3's existing `target/s1-3/measure` and `target/g3/exact-wheel` then drove Ink
alone at 800×768 points, 2×, on the 120 Hz display, with direct Metal and
`BENCH_LIVE=0`. Four recorded gesture windows completed before the first focus
loss in both retained **fling diagnostics** below. Neither entire process was
accepted by G3: the normal run lost focus during rest; the control lost focus
78 ms after its fourth gesture, before the fling phase ended. No quiet-run
qualification or comparison with a freshly measured GPUI arm is claimed.

| Gesture-window measurement | Normal images | Images disabled |
|---|---:|---:|
| Presented interval p50 / p95 / p99, ms | 8.33 / 16.67 / 16.67 | 8.33 / 16.67 / 25.00 |
| Late intervals / gesture seconds | 54 / 4.5413 | 55 / 4.6081 |
| Late intervals/s | 11.891 | 11.936 |
| Presented intervals | 335 | 328 |
| Trace events dropped | 0 | 0 |
| Image decodes / uploads during the fling | 32 / 32 | 0 / 0 |

These are `g3-before/ink-1.json` and `g3-control/ink-1.json`, with sibling
`.trace.json` files. The control uses an empty `EXACT_ASSETS` directory;
`G1_ALLOW_IMAGE_REFUSALS=1` lets its probe start with empty image boxes. Its
startup timing is not a complete-scene measurement. The trace additions between
these runs add demand, wheel and encode markers; no pacing policy changed.
The control executable SHA-256 is
`8f30fb9c420dfe254946d556519e8434adbd1274cb26926baedf6b92156893c2`;
its probe is `35b8a2fe7d02bc6d32bf1aa75ef5990570576be339a2cad4ae34b96b5179ef2e`.
The final code also factors the live-wheel handler for its regression test and
removes an unused text-instance marker. `g3-no-images` lost focus during its
second gesture; `g3-traced` lost focus before any gesture. Neither supplies a
cadence result. No failed attempt was silently replaced.

### Where the time went

Normal-run inclusive spans during the ten-second fling, in milliseconds:

| Phase | Calls | Total | p99 | Maximum |
|---|---:|---:|---:|---:|
| Main-thread walk, including nested work | 391 | 70.852 | 1.101 | 5.039 |
| Glyph rasterization on cache misses | 481 | 9.501 | 0.178 | 0.277 |
| Path rasterization on cache misses | 1 | 0.144 | 0.144 | 0.144 |
| Image decode, off main thread | 32 | 142.622 | 15.313 | 15.313 |
| Image upload preparation, main thread | 32 | 6.518 | 0.496 | 0.496 |
| Mip encoding, nested in upload preparation | 32 | 2.524 | 0.179 | 0.179 |
| Finish, including scene encoding | 391 | 30.402 | 0.181 | 0.225 |
| Handoff queue delay | 356 | 57.575 | 3.148 | 5.815 |
| Worker upload, including staging wait | 506 | 48.527 | 0.400 | 2.672 |
| Staging-ring completion wait | 506 | 37.542 | 0.372 | 2.645 |
| Worker `nextDrawable` | 355 | 15.790 | 0.206 | 0.725 |
| Presentation GPU command | 355 | 518.799 | 2.614 | 2.815 |

The walk can also run for input geometry; its count is not a presentation count.
CPU spans nest; worker waits overlap GPU execution. The table must not be summed.
The control additionally separates scene encoding and drawable-copy encoding.
The display callback to main-thread wake delay was p50 **0.016 ms**, p99
**0.062 ms**, maximum **0.070 ms** during the normal gesture windows.

`g3-before-late-frames.json` and `g3-control-late-frames.json` attribute **every**
late interval in those windows: main-thread spans are charged to the innermost
recorded phase; worker/GPU overlap is reported separately. Normal-run exclusive
main-thread work per late interval was p50 **0.842 ms**, maximum **2.447 ms**.
Only five of 54 late intervals overlap an image upload, seven overlap a decode,
five overlap glyph rasterization, and one overlaps path rasterization.

Of 54 late intervals, 51 contain two presentation commits. Independently, 51
contain a presentation GPU completion before the next nominal 8.33 ms boundary.
This is temporal attribution, not a drawable-to-frame join: the trace does not
yet connect a drawable ID to the compositor's selection. It rules out a long
walk, synchronous decode or three-drawable exhaustion as the common observed
stall. It does not prove that finishing before vblank meets Core Animation's
presentation deadline. The image-disabled control still has 55 late intervals;
45 contain two display callbacks with pending display demand at both callbacks.
A fifth scheduling change without that missing correlation would be a guess.

### The memory transient, with stacks

`g3-stacks` runs the posted fling with `MallocStackLogging=full`. On this OS,
`MallocStackLogging=1` selects lite logging and `malloc_history -highWaterMark`
refuses it. `vmmap --verbose -fullStacks` and `malloc_history -allBySize
-fullStacks` captured **584.7 MiB** live footprint and **610.5 MiB** lifetime
peak. Focus was lost during the first gesture and stack collection perturbs
execution; this is allocation evidence, not a cadence sample.

The largest captured allocation stacks include the following. Sizes are live
allocations grouped by stack, not quantities to add to the footprint again.
Release frames in the app are addresses because the executable is stripped;
the complete stacks and addresses are retained in `g3-stacks/malloc.txt`.

| Live size / allocations | Allocator stack, caller → allocation |
|---|---|
| 22.53 MiB / 15 | `CGContextDrawImage → ripc_DrawImage → img_data_lock → create_image_data_handle → calloc` |
| 21.49 MiB / 7 | `CGImageSourceCreateThumbnailAtIndexEx → IIOImageSource::createThumbnailAtIndex → CGImageCreateCopyWithParametersNew → img_data_lock_depth → calloc` |
| 17.92 MiB / 17 | App decode-worker frames → `calloc` (the separate symbolized development capture reaches `exact_ink::image::apple::decode → Pixmap::new`) |
| 12.81 MiB / 4; 10.02 MiB / 3 | App texture-creation frames → `AGXTexture → IOGPUMetalTexture → IOGPUResourceCreate` |
| 19.00 MiB / 2 | `CAMetalLayer nextDrawable → get_unused_drawable → CAIOSurfaceCreate → IOSurfaceCreate → IOSurfaceClientCreateChild` |
| 4.77 MiB / 2 | Render-worker frames → `AGXBuffer → IOGPUMetalBuffer → IOGPUResourceCreate` (the staging ring) |

The vmmap totals are **51.1 MiB resident IOAccelerator graphics**, **28.5 MiB
resident IOSurface**, and **339.8 MiB resident owned-unmapped graphics**. The last
category supplies object IDs but **no allocation stacks**. It is not accounted
for by the two staging buffers or the three drawables. In the normal unstacked
trace, 38 image texture allocations, including mip chains, total **50.094 MiB**;
all tracked textures total **60.656 MiB**. None retires during this short drive.
This drive does not exercise bitmap eviction, so it does not establish whether
texture retirement follows the painter's eviction in operation.
The image-disabled fling still samples **474.345 MiB** peak versus **138.876 MiB**
at its end: removing images leaves a **335.469 MiB** transient.

A 15-second `xctrace` Metal System Trace attached to the carrier succeeded
(`metal-system/capture.trace`). It captured resource allocation stacks, but its
kernel-resource-allocation table is empty. Its app Metal allocation counter
ranges from **70.078 to 96.859 MiB**. Texture views share backing storage and
must not be counted again. This run lost focus before its drive and is not a
performance comparison. It does not identify the owned-unmapped charge.
A launch-time retry (`metal-launch`) never reached the probe's ready state;
Instruments left the carrier stopped and failed to finish saving after its
20-second limit and a further 90-second wait. Export reported `Document Missing
Template Error`. The recorded processes were stopped; this attempt contributes
no allocation result.
The development socket stack capture's 224 MiB of oversized path/clip textures
is a different workload; G3's single path rasterization rejects it as an
explanation of this peak. No atlas eviction policy was changed.

**Prediction and decision:** no supported one-change prediction can be made
for the remaining presentation deadline or the unidentified graphics charge.
No optimization was applied, so the predicted improvement from this chunk is
**zero late intervals/s**. The required next evidence is a launch-time graphics
allocation capture and a drawable-ID join through GPU completion, compositor
selection and presentation. The existing traces narrow those questions but do
not answer them; no after-fix measurement or G3 qualification is claimed.

### Validation

All six checks pass, including the workspace check and caps' wasm32 check.
The first default-member test run timed out in `workers_start_with_the_first_request`
inside the Apple executor's nested async suite; the full command passed on retry
with `RUST_TEST_THREADS=2`. No test or timeout was changed. The full Linux run
passed: 1,576 library tests and 293 integration tests, excluding child reruns.
`exact-ink`: 33 passed; carrier: 2 passed; trace reader: 24 passed.
The named suites used `EXACT_INK_DEVICE=metal EXACT_REQUIRED=1 RUST_TEST_THREADS=2`:

```text
paint: test result: ok. 56 passed; 0 failed; 1 ignored; 0 measured; 48 filtered out; finished in 23.20s
borders: test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 6.81s
tint: test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 7.19s
backdrop: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out; finished in 8.59s
gradients: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 8.95s
visual: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 9.33s
materials: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 10.13s
image: test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 8.09s
```

Logs are in `target/s1-4/checks/`; carrier and reader logs are in `target/s1-4/`.

## S1.5 — the input's gaps and GPUI's re-presentation

**Prediction recorded before the rerun, 2026-10-29.** The driver supplies input
slower than the display refreshes. Ink presents changed content; GPUI also
re-presents unchanged content between inputs. The roughly 12 versus 0.66 late
intervals/s therefore compares different kinds of cadence. This chunk changes
the wheel driver and leaves the carrier's pacing and presentation untouched.

### Trace join and counts

`target/s1-5/analyze.py` re-derives the counts from
`target/s1-4/g3-control/ink-1.json.trace.json` and
`target/s1-4/g3-before/ink-1.json.trace.json`; its full output and per-pair
attribution are `target/s1-5/analysis.txt` and `analysis.json`. Both traces
report zero dropped events. Their SHA-256 hashes are, respectively,
`4e6556449889bbc9bb0999ab7ad3190d02df88e03c2ebf22ab04cdc13f5665e2` and
`8fa2243afbe3440ffa66dbebe878b6abce11d2d3e98dd6671f9034b405fe173d`.

The k-th `presented`, `present committed` and `gpu present` events are joined
**in file order**, including `presented.ts == 0` (never shown). No timestamp
sort precedes the join. CAMetalLayer presents in commit order; GPU completion
is `gpu present.ts + dur`. After joining, never-shown frames are removed from
the displayed sequence. An interval counts only when both endpoints are inside
one `inputWindows` entry in the sibling `ink-1.json`; late means over 12.5 ms.

| Measurement | Control, newer marks | Before, normal images |
|---|---:|---:|
| Joined / shown / never shown | 348 / 333 / 15 | 357 / 340 / 17 |
| Gesture seconds | 4.608089 | 4.541267 |
| Shown intervals / late intervals | 328 / 55 | 335 / 54 |
| Late intervals/s | 11.936 | 11.891 |
| GPU end to shown time, minimum–maximum, ms | 15.828–55.116 | 15.659–27.166 |
| GPU end to shown time in [15, 20) ms | 297 | 315 |
| GPU end to shown time in [26, 28) ms | 21 | 16 |
| GPU end to shown time ≥28 ms | 10 | 0 |

These files do **not** reproduce the brief's 50 late control intervals or its
35-frame 26–27 ms cluster. Every joined GPU command finishes before both its
shown vsync and the preceding nominal refresh; GPU completion missing those
vsyncs is not the cause. The control's longer presentation-latency tail remains
unexplained. These diagnostic runs retain S1.4's focus-loss limitations.

The control records 422 `wheel input` events and 421 gaps. Half-millisecond
bins contain **340** gaps in [8.0, 8.5) ms and **64** in [16.5, 17.0) ms,
not the brief's 328 and 63. Excluding the three between-gesture gaps leaves
418 gaps averaging **9.573 ms**; all 421 average 19.956 ms. The driver's log
reports `{"posted":106,"periodMs":8.33}` for each of four gestures (424 posts;
422 input marks). It sleeps a full nominal period **after** each post, so
posting cost and sleep overshoot accumulate. The delivered 8.33/16.67 ms grid
is consistent with window-server delivery on the 120 Hz refresh: roughly one
slot in seven has no new input. The trace measures delivery, not the posting
cost and sleep overshoot separately.

To attribute a late pair, its commit is associated with the latest handoff
whose wait ended before that commit. The handoff's start is queue insertion
after the main walk; the last input before that insertion identifies the
frame's input. Using the last input before the commit instead can charge the
next overlapping walk's input to this frame. **46 of 55** late pairs contain
at least one ≥12 ms consecutive input gap between those frame inputs. The
remaining **9 / 4.608089 s = 1.953/s** replace the brief's 6 / 50 and ~1.3/s
as this prediction's empirical basis. This is input-gap attribution, not proof
that every residual is a renderer stall.

Of **122** `display demand` callbacks with nothing pending, **56** follow a
draw after the latest input; the other **66** follow an input whose
`wheel dirty` mark is zero. Those inputs required no content frame. This
also differs from attributing all 66 callbacks solely to input gaps.

### Why GPUI hides the input gaps

In pinned **gpui 0.2.2**, `src/window.rs`'s `on_request_frame` closure
(lines 1018–1067) sets `needs_present` while the window is active and its
`last_input_timestamp` is less than one second old. Even with a clean
invalidator, it calls `present()` (line 2007), which draws
`rendered_frame.scene` again. Its comment gives the reason: “to prevent the
display from underclocking the refresh rate.”

The shared `MTLDrawable.presentedTime` instrument therefore counts GPUI's
re-presentations during input gaps, whereas Ink's row counts changed content.
The same timestamp instrument does not make those two rows content-cadence
comparisons.

### Driver and prediction

`host/apple/wheel.swift` now selects the display under `request.screen` and
posts at most one event from each `CVDisplayLink` output callback. The first
event is queued before the link starts; the producer waits for its delivery
before queuing another, so callback credits cannot accumulate into bursts.
The phases, event fields, 100 ms travel, 0.95 momentum decay, half-point end
and two-second cap stay. `periodMs` reports
[`CVDisplayLinkGetActualOutputVideoRefreshPeriod`](https://developer.apple.com/documentation/corevideo/cvdisplaylinkgetactualoutputvideorefreshperiod(_:)),
or the measured mean callback interval when Core Video supplies no period.
The existing source-hash helper build needs no change.

**Prediction, before the G3 rerun:**

- With the driver paced by the display, ink's late intervals per gesture second fall from
  ~12 to at most ~2 (the 9 unexplained of 55 is ~2.0/s; the display-link restart cases
  S1.4 could not join are included); its presented intervals per ~4.5 s of gesture windows
  rise from 300–339 to about GPUI's 464–490; p95 falls to 8.33 ms.
- GPUI's and the native host's rows do not move beyond their ranges.
- Ink's fling CPU per second rises in proportion to the frames it now draws (about
  +40%); the comparison with GPUI must be read per frame as well as per second.

### What remains

The rerun cannot settle input-to-photon latency: the 26–27 ms cluster and
the longer control tail are not a G3 column. There is no content-cadence
instrument for GPUI. The rerun's cadence rows are comparable only during a
gesture, when every tick has new input; unchanged re-presentations after
input ends remain a different measurement.

The rerun must report actual gesture-window duration alongside interval count:
four fixed 106-event gestures get shorter when their input period falls from
9.57 to 8.33 ms. The interval-count prediction is stated per ~4.5 seconds.

One residual candidate is `about_to_wait`: when the gesture's events carry
no phase, `scrolling` stays false and the carrier stops `CVDisplayLink`
after each drawn frame. The before trace has **5 of 54** late pairs with
only one display callback between commits (the brief said 5 of 53); the
control has 8 of 55. This is consistent with a restart missing a period,
and remains a candidate for after the rerun. No carrier change is made here.

Posting requires an unlocked session. No fling or G3 rerun is performed in
this chunk, and no dry-run mode is added. The orchestrator will run the posted
fling with `G1_TRACE=1` when the display is free and check each input gap.

### Validation

All six checks pass, including fmt and caps' wasm32 check. The default-member
tests and Linux tests used `RUST_TEST_THREADS=2`; Linux also used
`EXACT_INK_DEVICE=metal EXACT_REQUIRED=1`. The full Linux run passed 1,576
library tests and 293 integration tests, excluding child reruns. The existing
`wheelHelper()` export in `scripts/agent-wheel.mjs` compiled the Swift helper
successfully into `target/wheel-helper/3cb494cefbaf78cd/exact-wheel`.
No posted-event timing is claimed from that build. Logs are in
`target/s1-5/checks/`.

```text
paint: test result: ok. 56 passed; 0 failed; 1 ignored; 0 measured; 48 filtered out; finished in 8.20s
borders: test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 3.54s
tint: test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 3.26s
backdrop: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out; finished in 4.64s
gradients: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 3.35s
visual: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 6.09s
materials: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 4.34s
image: test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 3.56s
```

## G3b: window to window, the driver paced by the display

@ref LLP 1149 §4.1; 2026-10-29, `lane/ink-s1` at `26646bb58e`.
**Driver verification failed; the three-arm series was not run.** It
recorded three within-gesture input gaps at or above 12 ms; the gate
requires none. No carrier, renderer, probe or tracked driver source changed, and
no further measurement attempts or screenshots followed this result.

### Preparation and verification

All three release arms were rebuilt at the lane tip, with the same linked
probe, into `target/g3b/{Native,GPUI,Ink}.app`; Ink uses direct Metal.
`target/g3/exact-wheel` was rebuilt from `host/apple/wheel.swift`, and
`wheelHelper()` independently compiled the source-hash helper at
`target/wheel-helper/3cb494cefbaf78cd/exact-wheel`. The measurement driver
compiled with G3's temporary 768-point Ink content-height adjustment.
`data.sha256` passed. The launch commands are in `target/g3b/plan.txt`;
`provenance.json` records executable and source hashes. All three signed
probes have SHA-256
`e1843d9a2c7cb810b91d55469b2a5cd70af105ae2297c4243d35f176c8e3c225`.

The orchestrator's `GO` arrived at 12:46:18 PDT while builds were still
running. No window was launched or input posted until preparation finished
and the quiet rule passed. The verification ran at 12:52:36–12:53:02 PDT
with `G1_TRACE=1 G3_ONLY_INK=1`: one G3 round, four gestures. All six
five-second machine observations were quiet, with no compiler processes
and maximum 1/5/15-minute loads of **24.88/23.59/22.06**.

The first launch lost focus before any gesture: the sidecar names **Slack**
(`com.tinyspeck.slackmacgap`, PID 36534), followed by a transition with
Heavy list frontmost but no key window. The driver's single permitted
retake completed with no focus loss, `fair` thermal endpoints and Low Power
Mode off. Its window was 800×800 outer points, 800×768 content, at 2× and
120 Hz. Both attempts and their focus records remain in
`target/g3b/verification/`.

The trace join follows S1.5: presented, committed and GPU spans in file
order, including never-shown drawables; only then remove zero presented
timestamps. There are **341 joined frames, 325 shown, 16 never shown and
zero dropped trace events**. Input gaps count consecutive `wheel input`
events only when both endpoints lie in the same recorded `inputWindows`
entry. The three between-gesture gaps are excluded.

| Verification measurement | Result |
|---|---:|
| Driver posts | 424; 106 per gesture |
| Probe `wheelEvents` / trace `wheel input` events | 419 / 419 |
| Within-gesture input gaps | 415 |
| Gaps in [8.0, 8.7) ms | 390 |
| Gaps at or above 12 ms; required 0 | **3: 16.672, 41.629, 12.481 ms** |
| Gesture-window duration | 4.069416 s |
| Presented intervals inside those windows | 320 |
| Late intervals, over 12.5 ms | 6; 1.474/s |
| Presented interval p95 | 8.333 ms |

All three long input gaps occur in the third gesture, which received 101
events against 106 posted; the other gestures received 106 each. The
helper reports 8.33, 8.33, 8.35 and 8.33 ms periods. These delivery marks
do not identify whether the remaining gaps arose in posting, window-server
delivery or application dispatch. The saved analysis is
`verification/analysis.json`; the successful retake's trace SHA-256 is
`ba6cc224d6a7675c122fa2f842f3bbb7dea2c4b067ce2809860f066ff986e0b8`.

### Prediction disposition

The verification's lower late count does not override its failed input
gate. The prescribed five interleaved rounds and ten launches per arm,
their medians and ranges, and the three screenshots were therefore not
collected. There is no new native or GPUI result to compare with G3.

| S1.5 prediction | Verification observation; series verdict |
|---|---|
| Ink late intervals ≤ about 2/gesture second | 1.474/s in this diagnostic; met in this sample, unqualified for the series |
| About 464–490 intervals per about 4.5 gesture seconds | 320 in 4.069416 s, equivalent to 353.86 per 4.5 s; not met in this sample |
| Ink p95 8.33 ms | 8.333 ms in this diagnostic; met in this sample, unqualified for the series |
| GPUI and native rows within G3's ranges | Not tested: neither arm launched |
| Ink fling CPU/s rises about 40%, read per frame too | Not tested by the series; the traced diagnostic alone cannot establish the change |

For accounting only, the traced retake used **78.96 ms/s** of fling process
CPU over 10.00243 s and presented **324 frames** in that same interval:
32.392 frames/s, or **2.438 ms/frame**. GPUI's corresponding value was not
measured. Trace overhead and the failed delivery gate prevent treating
this as the requested CPU comparison. No D15 verdict changes.

G3's window-parity and non-comparables paragraph and table above still
apply. During a gesture with one input per refresh, both painted rows can
represent content cadence; that premise was not established here. At rest,
GPUI's one-second re-presentation runs and Ink's does not, which the
rest-CPU row of a completed series must reflect. S1.4's roughly 340 MiB
graphics transient remains unresolved.


### Validation

All six repository checks passed, including clippy, fmt, the workspace
all-target check and caps' wasm32 check. The default-member tests used
`RUST_TEST_THREADS=2`. The full Linux run also used
`EXACT_INK_DEVICE=metal EXACT_REQUIRED=1` and passed 1,576 library tests
and 293 integration tests, excluding child reruns. No retries were needed.
The filtered suites used the same environment; their summaries are verbatim:

```text
paint: test result: ok. 56 passed; 0 failed; 1 ignored; 0 measured; 48 filtered out; finished in 5.71s
borders: test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 3.21s
tint: test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 2.98s
backdrop: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out; finished in 3.36s
gradients: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 2.97s
visual: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 4.38s
materials: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 3.72s
image: test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 2.90s
```

The logs and command exit codes are in `target/g3b/checks/`.

G3c below replaces the received-gap gate with posting measurements; the orchestrator diagnosed
the gaps in G3b's third gesture as a roughly 30 ms window-server stall, with input absent for
41.6 ms and a drawable presented 29 ms after commit instead of the usual 18 ms, around
1253670.011, 1253670.028 and 1253670.070 s in
`target/g3b/verification/ink-1-retry.json.trace.json`.

## G3c: window to window, the driver paced by the display

@ref LLP 1149 §4.1, D7, D12, D15; measured 2026-10-29 on `lane/ink-s1`,
starting at `ab2455a502`. The three release bundles, linked probes and measurement
binary in `target/g3b/` are unchanged from G3b, verified by SHA-256.
Only `host/apple/wheel.swift` changed: `maxGapMs` reports the longest host-clock
interval between consecutive posts in one gesture. Both `target/g3/exact-wheel`
and the source-hash helper (`7313757cdcce07dd`) compiled from that source.
`target/g3c/plan.txt` holds the launch commands; `provenance.json` holds the hashes.

The machine and instruments are G3's: M5 Max, 18 CPU cores, 128 GiB,
macOS 27.0.1 (26A434), built-in 120 Hz display, 800×800-point outer windows
at 2×. Native's 800×800 content includes the title-bar safe area; GPUI and
Ink have 800×768 content. Ink uses direct Metal; live mode and tracing are off.
Four 600-point flings start at 0, 2.5, 5 and 7.5 seconds, followed by ten
seconds at rest. Each retained sample posted four sets of 106 events.

### Three-arm results

Values are **median (minimum–maximum)** across five qualified fling/rest
samples and ten qualified fresh-process launches per arm. The first five
launches supply the fling/rest samples; slots 6–10 are cold-only. Application,
filesystem and driver caches were retained: these are new processes, not
empty-cache launches. **Every retained sample was quiet.**

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal |
|---|---:|---:|---:|
| Cold to first complete frame, ms; native snapshot / GPUI and Ink presented drawable | 370.26 (340.06–452.06) | 312.17 (293.53–483.95) | 343.83 (314.39–452.01) |
| Fling process CPU, ms/s | 133.41 (116.98–140.29) | 87.31 (77.78–89.03) | 82.02 (78.19–93.23) |
| Fling process CPU per presented frame, ms | — | 0.965 (0.881–1.005) | 2.540 (2.429–2.834) |
| Fling main-thread CPU, ms/s | 89.29 (80.85–94.19) | 41.35 (36.54–42.49) | 38.77 (38.16–44.88) |
| Rest process CPU, ms/s | 1.200 (1.115–1.370) | 4.917 (4.476–5.698) | 1.393 (1.193–3.628) |
| Rest main-thread CPU, ms/s | 0.582 (0.552–0.687) | 3.147 (2.905–3.718) | 1.266 (1.105–2.906) |
| Peak footprint during fling, MiB | 228.70 (222.94–235.72) | 409.20 (396.78–414.75) | 591.52 (590.41–592.25) |
| End-of-fling footprint, MiB | 212.03 (207.02–225.81) | 389.34 (386.24–396.50) | 245.97 (244.92–246.69) |
| End-of-rest footprint, MiB | 211.35 (206.28–225.05) | 208.95 (203.58–213.39) | 238.59 (237.59–239.39) |

GPUI and Ink use the same drawable hook and `MTLDrawable.presentedTime`;
their first presented drawable was the first complete scene in all twenty
retained launches. Native's layer-snapshot milestone remains a different
instrument. CPU per frame divides the process CPU used in the full ten-second
fling phase by presented frames in that same phase: GPUI **886 (883–923)**,
Ink **326 (322–330)**. GPUI's count includes unchanged re-presentations, so
its lower CPU per presented frame is not a cost per newly drawn content frame.

Native's cadence below is the display link over the ten-second fling;
GPUI's and Ink's are presented drawables inside the gesture windows. As in
G3, quantiles use `floor(p × count)`, late means strictly over 12.5 ms, and
only adjacent timestamps with both endpoints inside one gesture window form
a drawable interval. Late counts are divided by those windows' total duration.

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal |
|---|---:|---:|---:|
| Fling interval p50, ms | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) |
| Fling interval p95, ms | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) |
| Fling interval p99, ms | 8.33 (8.33–13.37) | 16.67 (8.33–16.67) | 16.67 (8.33–16.67) |
| Late intervals/s | 0.800 (0.600–1.300) | 1.249 (0.750–1.720) | 1.249 (0.250–2.247) |
| Gesture-window duration, s | 4.004 (3.996–4.070) | 4.003 (4.002–4.070) | 4.004 (4.000–4.006) |
| Presented frames/s inside gesture windows | — | 103.69 (103.16–104.43) | 81.45 (80.38–82.42) |
| Inputs received / posted per sample | 8/424 (8–8/424) | 416/424 (412–421/424) | 420/424 (406–424/424) |
| Driver maxGapMs; maximum of four gestures | 17.212 (16.635–32.586) | 16.849 (16.654–16.936) | 17.017 (16.765–24.811) |

The receive counts are the probe's `wheelEvents`; all denominators are the
four driver `posted` counts summed. Native's asynchronous AppKit scrolling
bypasses the local event monitor, so **8/424 is not 416 lost inputs**. GPUI
recorded 97.17–99.29% of posts and Ink 95.75–100%; those deficits and the
posting gaps remain beside the raw cadence. Every sample had a driver gap
longer than one refresh; the reported `periodMs` values were 8.33–8.83 ms. Neither that period
estimate nor the new maximum establishes one received input on every refresh,
and this untraced series cannot assign each late interval to posting, delivery
or rendering.

### S1.5 prediction, checked line by line

| Prediction | Result |
|---|---|
| Ink late intervals fall from about 12 to at most about 2/gesture second | **Met at the median:** 1.249/s (0.250–2.247), against G3's 12.124/s; one of five samples exceeded 2/s. |
| Ink p95 falls to 8.33 ms | **Met:** 8.33 ms in all five samples, against G3's 16.67 ms. |
| GPUI's rows remain inside G3's ranges | **Not met:** cold 312.17 ms is below 336.84–506.50; fling process/main CPU 87.31/41.35 ms/s is below 90.02–94.82/42.67–46.32; rest process/main CPU 4.917/3.147 ms/s is below 5.370–5.767/3.525–3.760. |
| Native's rows remain inside G3's ranges | **Not met:** cold 370.26 ms is below 386.23–493.05; fling process/main CPU 133.41/89.29 ms/s exceeds 113.99–124.02/78.57–86.02; late cadence is 0.800/s against 0.100–0.500; end-of-fling/rest footprints 212.03/211.35 MiB are just below 212.33–228.72/211.64–227.97. |
| Ink fling CPU/s rises about 40%, read per frame too | **Not met:** 82.02 against S1.3m's 105.34 ms/s, **−22.1%**; 2.540 against 3.164 ms/presented frame, **−19.7%**. Presented frames over the ten-second phase fell from 335 (329–340) to 326 (322–330); phase frames/s fell 2.7%. S1.3m was loaded, so this is not a controlled attribution to pacing. |

The prediction of 464–490 intervals per 4.5 seconds is **withdrawn, not tested**:
GPUI re-presents through the tail of each gesture window and Ink does not, so
counts per window measure that padding as well as renderer activity.
The replacement row reports **103.69 (103.16–104.43)** presented frames/s for
GPUI and **81.45 (80.38–82.42)** for Ink during the actual gesture windows,
with that same padding caveat; these are not equivalent rates of changed content.

### Qualification, focus and window inspection

`GO` appeared at **13:20:40 PDT**. The timed series ran **13:21:05–13:27:24**;
the three screenshots finished at **13:27:43**. No window or input was launched
before GO, and no verification fling, extra arm or extra round ran. All **77**
five-second machine observations met the quiet rule, with maximum 1/5/15-minute
loads **11.40/10.42/12.02** and no compiler processes. Every retained sample
had `fair` thermal endpoints, Low Power Mode off, and successful activation,
frontmost-PID and key-window checks before measurement and each gesture.
Ink slots 1 and 2 needed another startup focus handshake before their windows
became key; no input preceded the successful handshake.

Native slot 1 lost focus during its second gesture to **Google Chrome**
(`com.google.Chrome`, PID 60411). Its single retake completed. No other sample
lost focus or required a retake; there were no startup timeouts or thermal
rejections. Thus 31 timed process attempts yielded all 30 requested launches
and 15 fling/rest samples. Raw attempts, focus sidecars, driver logs and quiet
observations are in `target/g3c/series/{fling,cold}/`; `summary.json` records
all retained paths, per-sample values and distributions. Each phase interleaves
native/GPUI/Ink, GPUI/Ink/native, Ink/native/GPUI according to the existing
driver's round number, without additional rounds.

The [native](g3c-native-top.png), [GPUI](g3c-gpui-top.png) and
[Ink](g3c-ink-top.png) screenshots are unscaled **1600×1600** captures at row
zero, taken in three separate launches outside timing and visually inspected.
All show Ava, Ben and Chloé, styled text, avatars and loaded photos. G3's
visible differences remain: Ink's wrapping, emoji and code face, full-width
separators and photo crops differ, while native and GPUI differ in text
spacing and title-bar treatment. These captures do not establish pixel parity.

G3's non-comparables remain: Ink is a benchmark carrier without the shipping
accessibility, IME and menu surface; its Parley/swash text differs from platform
text; GPUI owns its tree, layout, virtualization and platform layer, without
Exact's kernel or runner; native carries AppKit views and Core Animation;
native and Ink boot a baked plan while GPUI parses JSON. Equal input does not
mean equal work: native ended at **8,403–9,322 points**, GPUI at item **18 +
255 points**, and Ink at **6,172 points**. Row geometry, caches and scheduling
differ. Native startup and cadence use different instruments. S1.5 adds that
GPUI re-presents unchanged content between inputs and for about a second after
each gesture, while Ink submits only changed frames. The rest-CPU row measures
their different idle behavior. The posting gaps and receive deficits above limit
the one-input-per-refresh comparison. No input-to-photon latency is measured.

**Finding.** Ink's median fling CPU/s is below GPUI's, with overlapping ranges;
its CPU per presented frame is higher, with GPUI's unchanged frames counted.
Ink's median late cadence matches GPUI's here and its p95 is 8.33 ms, but the
input gaps remain part of both measurements. Ink uses less rest CPU than GPUI,
more peak footprint than either arm, and more end-of-rest footprint than either.
Its 343.83 ms fresh-process cold median is above GPUI's 312.17 ms and below
native's differently instrumented 370.26 ms; none meets D15's 100 ms boot
budget. This comparison includes neither improved vello nor Ink on wgpu, so
it cannot decide D15's C-over-B or D-over-C arms, nor qualify D12's shipping
carrier. **S1.4's roughly 340 MiB graphics transient remains open:** the fling
peak is 591.52 MiB against 245.97 MiB at its end, with no renderer change here.

### Validation

All six repository checks passed, including clippy, fmt, the workspace
all-target check and caps' wasm32 check. The default-member and Linux tests
used `RUST_TEST_THREADS=2`; Linux and the named suites also used
`EXACT_INK_DEVICE=metal EXACT_REQUIRED=1`. The full Linux run passed
1,576 library tests and 293 integration tests, excluding child reruns.
No test retries were needed. The filtered suite results are verbatim:

```text
paint: test result: ok. 56 passed; 0 failed; 1 ignored; 0 measured; 48 filtered out; finished in 6.95s
borders: test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 3.27s
tint: test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 3.00s
backdrop: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out; finished in 3.44s
gradients: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 3.05s
visual: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 4.44s
materials: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 3.78s
image: test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 127 filtered out; finished in 3.03s
```

Logs and exit codes are in `target/g3c/checks/`. Both wheel-helper builds
passed, and the series exercised the new `maxGapMs` output in all sixty
retained gestures. The aggregation reproduced G3's existing cadence, CPU
and footprint distributions before applying the same formulas to G3c;
the per-frame CPU uses matching time bounds, and every measured phase was
ten seconds within 50 ms. All three screenshots were inspected at 1600×1600.
`git diff --check` passed; the RFC diff is confined to the requested paragraph.

## G4: four arms, egui beside GPUI, ink and the native host

@ref LLP 1149 §4.1; measured 2026-10-29 on `lane/egui-arm`.
Charlie asked for egui beside GPUI. The orchestrator ran the prepared G4b
instrument from `994b003388`, based on `0312b493fc` (S1.6), with the display
held. The four release bundles in `target/g4b/` share probe SHA-256
`6319ba0878f7f1df2e1917a74c71c1280a2fbabca30c6187e069266ba3dd0660`;
`provenance.json` records their executable hashes and the checked data hash.
Egui and eframe are **0.37.0**, wgpu **30.0.1**, pinned published packages in
`../egui/Cargo.lock`. Eframe uses wgpu over Metal; SF Pro, SF Pro Italic and
SF Mono loaded from the system in every retained egui launch. All four use
the same 10,000 messages and images, screen, linked probe and, for the painted
arms, `g1_ready` complete-scene signal. No renderer or app code changed for
this report.

The machine is the same M5 Max (18 CPU cores, 128 GiB), macOS 27.0.1
(26A434), built-in 120 Hz display. All outer windows are 800×800 points at
2×; native includes its title-bar safe area in its content, while GPUI, Ink
and egui have 800×768 content. Ink uses direct Metal. Live mode, tracing and
egui diagnostics are off. Five interleaved rounds rotate native/GPUI/Ink/egui
by one place each round. Each process receives four phase-bearing 600-point
flings at 0, 2.5, 5 and 7.5 seconds, then ten seconds at rest. The driver
reported no focus losses or focus retakes.

### Four-arm results

Values are **median (minimum–maximum)** over five fling/rest samples and ten
successful fresh-process launches per arm. The cold column takes `fling/`
rounds **1–5**, `cold/` rounds **6–7**, then `cold2/` rounds **8–10**, in round
order. Cold2's rounds **11–12** are retained but unused. The failed egui
round-8 attempt is counted below; it has no completion time and contributes
no numeric cold value. Caches were retained, so these are fresh processes,
not empty-cache launches.

**All twenty fling/rest samples and the first seven successful launches per
arm were quiet under the five-second monitor.** The series started at
**20:15:54 PDT**, after the quiet rule passed: all load averages below 30,
no compiler processes. All **98** observations in `series/quiet.jsonl` were
quiet, with maximum 1/5/15-minute loads **10.18/17.10/29.77**. The monitor
ended with the failed launch at 20:24:13; cold2 ran about 20:24:45–20:25:31
without a retained monitor. Its launches are **unmonitored, not qualified as
quiet or classified as loaded**. There are **zero observed loaded samples**.
Every successful launch had nominal thermal endpoints, Low Power Mode off,
and successful frontmost/key-window checks; those endpoints do not establish
cold2's load conditions.

For the seven quiet successful cold launches, native/GPUI/Ink/egui medians
and ranges are **356.72 (330.80–862.31) / 324.85 (305.87–484.67) /
352.29 (340.68–1019.34) / 337.97 (310.81–445.11) ms**. For the three selected
unmonitored launches they are **345.25 (343.39–414.32) / 310.56
(301.71–312.51) / 326.31 (311.68–373.95) / 336.94 (318.34–421.95) ms**.
The cold row below combines those two groups; the other rows are all quiet.

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Cold to first complete frame, ms; native snapshot / painted drawable | 354.14 (330.80–862.31) | 313.13 (301.71–484.67) | 352.13 (311.68–1019.34) | 337.45 (310.81–445.11) |
| Fling process CPU, ms/s | 131.91 (112.39–143.12) | 84.92 (82.05–89.05) | 86.41 (84.98–87.07) | 66.84 (64.93–71.54) |
| Fling process CPU per presented frame, ms | — | 0.961 (0.922–1.005) | 2.615 (2.591–2.696) | 1.642 (1.592–1.733) |
| Fling main-thread CPU, ms/s | 84.51 (76.30–92.87) | 39.56 (38.97–41.52) | 43.83 (41.98–44.65) | 31.33 (30.28–33.69) |
| Rest process CPU, ms/s | 1.288 (1.108–1.440) | 5.631 (4.911–5.678) | 1.469 (1.241–1.544) | 0.359 (0.307–0.494) |
| Rest main-thread CPU, ms/s | 0.636 (0.536–0.692) | 3.495 (3.134–3.598) | 1.382 (1.158–1.466) | 0.288 (0.238–0.413) |
| Peak footprint during fling, MiB | 228.39 (211.35–232.45) | 406.42 (391.64–413.84) | 591.56 (587.67–593.42) | 507.81 (504.14–515.89) |
| End-of-fling footprint, MiB | 216.61 (198.06–220.39) | 389.38 (388.77–391.05) | 246.08 (242.61–247.64) | 186.49 (179.99–188.63) |
| End-of-rest footprint, MiB | 215.85 (197.42–219.58) | 208.55 (206.05–210.47) | 238.64 (235.56–240.38) | 178.28 (171.80–180.50) |

The three painted arms use `MTLDrawable.presentedTime` from the same
`CAMetalLayer.nextDrawable` hook. In all thirty selected successful painted
launches, the first recorded drawable was also the first complete scene.
Native's settled layer-snapshot milestone is a different instrument. CPU per
presented frame divides process CPU over the entire ten-second fling phase
by the number of presented frames in that phase. GPUI re-presents unchanged
content between inputs and through the gesture tail: its low CPU per
presented frame is not a cost per newly drawn content frame. Egui has no
application frame timer; input, window/font events and image completion wake
it. It presented **405–413** frames in the whole phase, only **1–9** outside
the gesture windows. Its **40.90 frames/s** over the phase versus **101.02**
inside the windows shows that it does not sustain GPUI's re-presentation
through the between-gesture idle periods. This does not prove every egui
presentation contains changed pixels.

Native's cadence is the display link over the full ten-second fling, not
presented pixels. The other arms use adjacent drawable timestamps whose two
endpoints are inside the same gesture window. Quantiles use
`floor(p × count)`; late means strictly above 12.5 ms. Late counts are divided
by the full phase for native and by gesture-window duration for painted arms.

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Fling interval p50, ms | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) |
| Fling interval p95, ms | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) |
| Fling interval p99, ms | 8.33 (8.33–8.33) | 16.67 (12.50–16.67) | 8.33 (8.33–16.67) | 8.33 (8.33–8.33) |
| Late intervals/s | 0.700 (0.300–0.900) | 1.248 (0.996–1.499) | 0.749 (0.500–1.999) | 0.738 (0.000–0.998) |
| Gesture-window duration, s | 4.011 (4.003–4.085) | 4.007 (4.002–4.016) | 4.006 (4.001–4.011) | 4.009 (4.000–4.067) |
| Presented frames/s inside gesture windows | — | 102.82 (101.35–103.05) | 81.38 (80.57–82.99) | 101.02 (98.60–102.45) |
| Presented frames/s over the full fling phase | — | 88.59 (88.38–89.00) | 32.60 (32.29–33.20) | 40.90 (40.49–41.28) |
| Inputs received / posted per sample | 8/424 (8–8/424) | 410/424 (394–415/424) | 417/424 (405–423/424) | 410/424 (403–414/424) |
| Driver maxGapMs; maximum of four gestures | 24.270 (16.214–24.997) | 17.949 (16.712–24.829) | 16.704 (16.344–25.766) | 24.631 (16.665–25.281) |
| Driver empty callbacks per sample | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

Receive counts are the probe's `wheelEvents`; posted counts sum the four
wheel-helper reports. Native's AppKit asynchronous scrolling bypasses the
local monitor, so 8/424 does not mean 416 lost inputs. GPUI recorded
92.92–97.88% of posts, Ink 95.52–99.76%, egui 95.05–97.64%. Every sample's
maximum posting gap exceeds one 8.33 ms refresh despite zero empty callbacks.
Raw cadence includes posting and delivery effects; this untraced series
cannot attribute each late interval to the driver, event delivery or rendering.

### Cold attempts and intermittent egui readiness

Across all timed phases, **49 process attempts produced 48 successful
launches**: twelve each for native, GPUI and Ink; **twelve of thirteen for
egui**. Forty successful launches supply the cold table, eight are retained
extras, and twenty of the successes supply fling/rest samples. Diagnostic
readiness probes and the four separate parity captures are outside these counts.
The first cold phase completed rounds 6–7, then stopped at egui round 8.
The orchestrator resumed with cold2 rounds 8–12: all twenty launches succeeded.

`series/cold/egui-8.log` records the three SF font loads, the M5 Max Metal
renderer, then **“probe: first complete frame timed out”** at the probe's
30-second deadline. PID 81690 was frontmost and its window key in the retained
sidecars; no focus loss was recorded during the wait. The process exited 2,
leaving no `.ready` or result JSON. The aggregator retains its log in
`rejected`, rather than silently losing an attempt without JSON. It is a
readiness failure, not a measured 30,000 ms completion.

G4a's first functional launch took **14,668.34 ms** to both its first recorded
drawable and first complete scene. G4b's four diagnostic launches did not
reproduce that delay: complete presentation followed window attachment in
**103.40, 82.80, 36.10 and 41.29 ms**, with logged image completions and repaint
requests. The G4a system-log analysis in `bb8e9a37b0` put all seventeen logged
Metal compilations before 14:58:55.921, well before its first drawable around
14:59:09; they did not explain the remaining delay. The same scoped system-log
method for the new failure places PID 81690 frontmost at **20:23:43.488** and
visible at **20:23:43.543**, then exiting at **20:24:13.30**. Neither the app
log nor the retained system and Metal logs identify the cause. With diagnostics
off, this attempt has no frame/worker timeline or retained drawable timestamps.
The symptom is **unexplained and intermittent**; a shared cause with G4a is
not established. `QUEUE.md` tracks investigation in the egui benchmark, with
no scheduling workaround or Exact change inferred from these measurements.

### Parity and non-comparables

The new [native](g4-native-top.png), [GPUI](g4-gpui-top.png),
[Ink](g4-ink-top.png) and [egui](g4-egui-top.png) captures were taken after
timing, at row zero, in separate launches. All four **1600×1600** images were
visually inspected; the new egui capture supersedes G4a's. They show Ava,
Ben and Chloé, loaded avatars and photos, links, bold, italic, inline code,
mentions and non-Latin text. Native and GPUI wrap Ava's first paragraph onto
five lines; Ink and egui use four, with different breaks from each other.
GPUI visibly compresses spaces at some run boundaries; egui's SF text is
narrower and its line metrics differ from the platform rendering. Ben's emoji
are smaller in Ink and egui than in native and GPUI. Egui's photos and avatars
look more saturated, especially the greens and magentas; the shared ImageIO
source does not establish equivalent color presentation. Ink's photo crop
differs. Native, GPUI and egui show inset hairlines; Ink's row separators span
the window. Title-bar appearance also differs. **No quote box is visible in
any supplied capture**, so its left bar, rounded background and two-line
truncation remain visually unqualified here. These are content checks, not
pixel parity, and they do not cover every row type or script.

G3c's non-comparables remain. Ink's benchmark carrier lacks the shipping
accessibility, IME and menu surface and uses Parley/swash text; native uses
AppKit views and Core Animation. GPUI owns its element tree, Taffy layout,
virtualization and platform layer without Exact's kernel or runner. Egui
likewise owns immediate-mode layout, HarfRust text, tessellation, caches and
eframe/winit/wgpu presentation, with viewport-only row construction and
estimated heights; GPUI premeasures an 800-point margin. Egui has no native
row accessibility labels, uses 15 pt SF Mono code against GPUI's 16 pt Menlo,
and equal-width digit cells instead of an OpenType `tnum` selector. Its font,
emoji and image-color differences above remain unqualified. Native and Ink
boot a baked plan while GPUI and egui parse JSON; egui also loads system font
bytes. Equal input does not mean equal work: native ended at **8,480–11,451
points**, GPUI at **item 18 + 205–255 points**, Ink at **6,172 points**, and
egui at **item 18 + 82.5 points**. Caches, row geometry, scrolling, startup
milestones and re-presentation behavior differ. No live mode, input-to-photon
latency, energy attribution or isolated renderer cost is measured.

**Finding.** Egui has the lowest fling process/main CPU and rest process/main
CPU, with non-overlapping ranges against the other three, and the lowest
end-of-fling and end-of-rest footprint. Its peak footprint is above native
and GPUI and below Ink. Its successful cold median is above GPUI and below
Ink and native, with overlapping ranges, three unmonitored launches per arm
and a readiness failure outside the numeric distribution. All arms have
8.33 ms p50/p95; egui's p99 is 8.33 ms in every round, while its late-rate
median is just below Ink's and below GPUI's, with overlapping ranges and input
gaps. Native's cadence uses a different instrument. Egui presents at about
GPUI's rate inside gesture windows, above Ink's; across the full phase its
rate falls between GPUI and Ink, as does its CPU per presented frame, whose
GPUI denominator includes unchanged frames. Zero empty callbacks does not
remove the posting gaps. This run compares whole applications on visibly
different text and color paths; it cannot rank the renderers, qualify a
shipping carrier, decide D15's improved-vello or wgpu-Ink comparisons, or
establish reliable startup from twelve successes and one timeout.

### Reproduction and validation

`target/g4b/plan.txt` and `run-series.py` record the prepared commands;
`series/{fling,cold,cold2}/` and their driver logs retain every attempt.
`aggregate.mjs` reads optional `cold2/`, orders G4's selected launches by round,
and lists successful rounds above ten as `unused`. It does not overwrite
raw samples. Reproduce the report without launching a window:

```sh
bun bench/heavy-list/gpui/aggregate.mjs target/g4b --json > target/g4b/summary.json
bun bench/heavy-list/gpui/aggregate.mjs target/g4b > target/g4b/tables.md
bun bench/heavy-list/gpui/aggregate.mjs target/g4b --verify
```

Two medians were checked directly against the raw files. Egui's ten sorted
cold values put `cold2/egui-10.json` (**336.937084 ms**) and
`fling/egui-2.json` (**337.970667 ms**) in positions five and six; their mean
is **337.453875 ms**, printed as **337.45**. Its five sorted fling CPU rates
are **64.929709, 66.782693, 66.841047, 67.140407, 71.543464 ms/s**. The middle
one is round 3: `(1.0029674583333332 − 0.33437912499999994) × 1000 /
(1268659.345665917 − 1268649.343004917) = 66.841047 ms/s`, printed as **66.84**.
Both Markdown and JSON output for the retained three-arm aggregation fixture
match the pre-edit outputs byte for byte; G3c's published section is unchanged.

The timeout's additional read-only log queries are retained in
`target/g4b/readiness-timeout-{system,metal}.log`. They use
`/usr/bin/log show --start '2026-10-29 20:23:40' --end '2026-10-29 20:24:15'
--info --debug --style compact`, with predicates
`processIdentifier == 81690 OR eventMessage CONTAINS "81690"` and
`process == "MTLCompilerService" OR subsystem CONTAINS "Metal"`, respectively.

All six repository checks passed, including clippy, fmt and the workspace
all-target check, as did the egui workspace's clippy (`-D warnings`) and fmt.
Tests used `RUST_TEST_THREADS=2`; Cargo used this worktree's `target/`.
No check needed a retry. All passed again after rebasing onto `origin/main`
`7a5dbbd996`; the queue conflict kept main's entries and added the egui item.
Commands, exits and logs are in `target/g4c/{checks,checks-rebased}/`.
The retained executables, probes, wheel helper, driver and message data all
match G4b's provenance hashes. `git diff --check` passed. This reporting
chunk opened no windows and posted no input.
