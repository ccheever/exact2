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

## G6: egui against ink on Windows

**2026-10-29, NucBox; updated through G6e below.** Direct D3D11 starts fastest;
egui uses the least fling CPU. G6e restores G6c's cadence while keeping the
G6d lifecycle fixes, but misses the declared CPU range. D3D11's displayed
cadence is close to egui's and its working set is below egui's. Three release applications
render the heavy list:
Ink through direct DirectX 11 composition; the same executable with
`EXACT_INK_DEVICE=wgpu`, using DX12, readback and softbuffer; and published
egui/eframe **0.37.0**, wgpu **30.0.1**, through eframe's DX12 swap chain.
Both wgpu arms report **AMD Radeon(TM) 8060S Graphics (Dx12)**. Egui's Windows
build enables only DX12 and explicitly requests it to match the control;
this is a selected backend, not an inference about eframe's defaults.

The machine is S2's Ryzen AI MAX+ 395, 16 cores/32 logical processors,
Windows 11 Pro build **26200**, Radeon driver **32.0.23033.1002**, Balanced
power plan, signed-in **5120×1440, 60 Hz, 100%** desktop. All applications
open an **800×800 outer window**, as in G4. Windows leaves a **784×761**
client, against the Mac's 800×768 content area. Rust is **1.97.0**, Bun
**1.4.2**; the native build uses SDK **10.0.26100.0**'s x64 `fxc`.
The measured executables use source `f3bd9710aa`, based on `origin/main`
`48653eb5cb`, which includes the requested `be5413a907`. The clean NucBox mirror at
`C:\Users\Charlie Cheever\projects\exact2` advanced through local Git
bundles on `lane/g6-windows`; no branch was pushed. The spaced startup
driver is `336fa9e44b`; the executables are unchanged.

### Method and readiness

Ten fresh processes per arm run in rotating order, with application and OS
caches retained. A separate five-round series contains ten seconds of fling
and ten seconds of rest per arm. Startup-only launches are spaced five
seconds apart, and the final launch receives the same observation interval.
Four decaying gestures start at 0, 2.5, 5 and 7.5
seconds; each lasts about one second and posts 1,500 wheel units. This is
600 logical pixels at the host's three-line, 16-pixel wheel mapping.
The driver calls `DwmFlush` for each input tick and posts one `WM_MOUSEWHEEL`
per return, without timer catch-up batches. Egui applies its own wheel
conversion and smoothing; identical input does not establish equal scroll
work. The second startup series performs AMD Software's supported **Reset
Shader Cache**, confirms the dialog and minimizes AMD Software before
every launch. It retains application and OS caches. Diagnostic launches,
builds and native qualification precede the timed series.

Windows has no Unix load average. The quiet rule here is no compiler
process and less than 10% total system CPU over five seconds, checked before
collection and every five seconds throughout. All attempts remain in the
raw record. Tables use medians
**[minimum–maximum]**. The original series's rapid startup-only tail had one
11.5% CPU observation among 73. Its startup distribution is superseded by
the complete spaced series, without selecting individual fast launches. All
15 fling/rest samples overlap quiet observations only (**71** observations,
maximum **9.7%** CPU). All **34** spaced retained-driver observations pass,
with maximum **7.2%** CPU. All **38** reset-driver observations pass,
with maximum **9.4%** CPU. The cache-reset driver's first invocation stopped
before launching an application because AMD Software's window was absent;
reopening it restored the supported reset path.

The DWM proxy is process age from `GetProcessTimes` and the precise system
clock after `DwmFlush` returns for the first frame with all requested images
loaded. The Windows host also waits for data activation. Egui requests one
more repaint after its complete UI pass, then observes the preceding
paint/present in the next `App::logic`; its proxy includes that event-dispatch
delay. [DwmFlush](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmflush)
synchronizes with the compositor; it does not identify scanout of the
application's particular frame. Direct D3D11 additionally reads the
matching DXGI present's display timestamp, as in S2. Eframe's public wgpu
render state exposes its adapter, instance, device, queue and renderer,
but no surface or DXGI swap chain, so egui has no corresponding timestamp.
The Ink/wgpu readback/softbuffer path has none either.

### Startup

Ten fresh processes per arm per cache condition; all 60 reach readiness.
The original 30 launches also succeed, including the 15 fling/rest samples.
All first-screen adapters are Radeon hardware; none reports WARP.

| Arm | Retained-driver DWM proxy, ms | Retained-driver DXGI display, ms | Reset-driver DWM proxy, ms | Reset-driver DXGI display, ms |
| --- | --- | --- | --- | --- |
| Ink / D3D11 | 175.98 [169.40–190.49] | 187.14 [182.01–200.87] | 249.30 [241.31–263.67] | 261.69 [253.10–274.78] |
| Ink / wgpu DX12 | 716.21 [706.02–736.88] | — | 1007.25 [994.91–1026.16] | — |
| egui / wgpu DX12 | 248.76 [240.70–255.15] | — | 348.00 [337.97–365.33] | — |

### CPU and memory

Five ten-second phases per arm. CPU is milliseconds per second; working set
is MiB. Main-thread CPU uses the HWND owner thread. Windows CPU accounting
is quantized in 15.625 ms units; a zero rate means no accounted tick in that
phase. Peak is the process lifetime peak at the end of rest; sampled fling
peak is separate. PresentMon and the driver are outside these process totals.

| Metric | Ink / D3D11 | Ink / wgpu DX12 | egui / wgpu DX12 |
| --- | --- | --- | --- |
| Fling process CPU, ms/s | 553.92 [504.33–583.87] | 121.41 [107.35–130.74] | 24.91 [17.13–32.69] |
| Fling main-thread CPU, ms/s | 358.13 [347.11–370.30] | 112.02 [94.90–118.29] | 24.91 [15.57–32.69] |
| Rest process CPU, ms/s | 4.66 [1.55–6.21] | 1.55 [1.55–6.21] | 0.00 [0.00–0.00] |
| Rest main-thread CPU, ms/s | 3.11 [1.55–6.21] | 1.55 [1.55–6.21] | 0.00 [0.00–0.00] |
| Lifetime peak working set, MiB | 220.88 [176.90–223.93] | 324.75 [320.96–329.70] | 181.74 [181.54–181.82] |
| Sampled fling peak working set, MiB | 220.88 [176.65–223.93] | 322.48 [318.68–327.42] | 170.77 [170.66–171.54] |
| End-of-fling working set, MiB | 220.88 [176.65–223.93] | 322.48 [318.64–327.42] | 170.00 [169.03–171.54] |
| End-of-rest working set, MiB | 220.88 [176.65–223.93] | 322.48 [318.64–327.42] | 170.00 [169.03–171.54] |

### Presented cadence and input pacing

PresentMon **2.6.0** ran with the account’s existing elevated desktop
privileges, without installing a service. Its DXGI/DWM ETW trace supplies
display timestamps for D3D11 and egui. Ink/wgpu exits normally but produces
no app-owned DXGI present records on its readback/softbuffer path; cadence
there is **unmeasured**, not zero. Optional GPU/input tracking is disabled.
The retained timed traces lose **zero ETW events**. Earlier diagnostic traces
with broader tracking lost events and are excluded from the tables.

Intervals count only consecutive displays inside the same one-second gesture
window. “Late” is over 25 ms, or 1.5 refresh periods at 60 Hz, per gesture
second. Per-run quantiles are aggregated below; the full-phase rate also
includes pauses between gestures. Rest counts cover the following ten seconds.

| Metric | Ink / D3D11 | Ink / wgpu DX12 | egui / wgpu DX12 |
| --- | --- | --- | --- |
| Display interval p50, ms | 16.73 [16.68–33.32] | — | 16.67 [16.67–16.68] |
| Display interval p95, ms | 83.35 [33.35–83.37] | — | 16.77 [16.76–16.82] |
| Display interval p99, ms | 83.38 [33.36–100.06] | — | 16.88 [16.84–17.21] |
| Late intervals / gesture second | 12.41 [7.39–12.84] | — | 0.00 [0.00–0.00] |
| Displays / gesture second | 26.05 [25.73–50.51] | — | 57.86 [57.74–58.06] |
| Displays / full fling second | 11.36 [11.26–21.52] | — | 25.60 [25.42–25.70] |
| Rest displays, count | 0 [0–0] | — | 0 [0–0] |
| Posted wheel events, count | 244 [243–244] | 243 [241–244] | 243 [243–244] |
| Largest within-gesture posting gap, ms | 19.11 [18.17–22.42] | 18.94 [17.79–21.81] | 20.31 [19.08–27.22] |
| Posting gaps over 25 ms, count | 0 [0–0] | 0 [0–0] | 0 [0–1] |

**Finding, updated through G6e.** Direct D3D11's complete-screen proxy is
**160.78 ms** in G6e's ten quiet launches; egui and Ink/wgpu were not
remeasured, and egui's proxy includes another event dispatch. G6's original
D3D11/egui/Ink-wgpu medians were **175.98/248.76/716.21 ms**, rising to
**249.30/348.00/1007.25 ms** after supported driver-cache resets. G6d corrected
readiness ordering; the historical startup distributions do not isolate a
scheduling gain or close [G6c's paired-startup question](#g6c-one-paint-per-refresh).
[S2](../../../llp/1149-the-renderer.rfc.md#d12--where-it-is-the-presenter-and-what-the-same-everywhere-promises)
records Shelf first screens of **142 ms D3D11 versus 859 ms wgpu**;
[C6](../../../host/ink/MEASUREMENTS.md#device-startup-host-dev) measured
**48 versus 139 ms** device creation and **457 ms** wgpu pipelines. Those
`gpu-dev` Shelf costs do not predict egui's release whole-app startup.
G6e's full-period reservation and removal of the clean-frame skip restore
**58.15 displays/s**, **zero late intervals/s** and **16.85 ms p99**, with
**1,247 snapshots / 1,247 successful frames**. Process/main CPU are
**84.08/62.31 ms/s**, display p95 **16.74 ms**, and peak/ending working set
**153.18/153.16 MiB**. The strict CPU gate misses G6c's **84.05 ms/s** upper
bound; the final five-run maximum is **93.39 ms/s**, so the full G6e gate
is not passed. The unchanged Ink/wgpu and egui controls used
**121.41/24.91 ms/s**, with egui's **16.77 ms** p95. Egui retains the lowest
fling CPU; D3D11 retains the lowest peak and ending working-set medians,
against egui's **181.74/170.00 MiB** and Ink/wgpu's **324.75/322.48 MiB**.
G6d's Mica recovery fix remains; [G6e](#g6e-the-full-period-reservation-wait-restored)
records qualification and the stop at the CPU gate. These whole-application
measurements retain unequal layout, scrolling, text, image and presentation
paths.

### Parity and non-comparables

The 105 generated data files match `data.sha256`; Exact's baked messages
match the same JSON and its 104 bundled JPEGs match egui's image files.
The JSON SHA-256 is
`0b63b79ca113dfc23e52fdc9f4b653890bd48a59110f917fe3cae80c730a30dd`.
Both use Segoe UI at SPEC's sizes on Windows. Egui loads regular, semibold
and italic system files, with **15-point Consolas** for inline code;
SF and SF Mono are absent. Egui's four WIC workers request reduced JPEG
decodes and scale/crop to the texture size; the Exact host keeps its own
image pipeline. Equal files and font families do not equalize shaping,
line height, fallback, emoji, color management or crop behavior.

The separate row-zero captures were visually inspected after images loaded.
Both show Ava, Ben and Chloé, avatars, photos and rich text. The end
captures confirm scrolling but differ: Ink shows Ingrid and José, while
egui shows Giulia, Hiro and part of Ingrid. Segoe UI wrapping
is close but row geometry differs; Ink's separators span the window and
egui's are inset. Photo color and crop differ. These are content checks,
not pixel parity. The top captures contain no quote box, so this check does
not qualify every row type or script. Captures remain under `target/g6/`;
the desktop outside the rounded window corners is not published.

Egui owns its immediate-mode layout, HarfRust text, tessellation, texture
caches and estimated-height viewport virtualization, with **no Exact kernel
or runner**. Exact runs the baked plan and host; egui parses JSON and loads
font bytes. Egui lacks the benchmark's native row accessibility labels and
uses equal-width digit cells rather than an OpenType `tnum` selector. The
DirectX 11/wgpu Ink comparison includes the latter's GPU readback and
softbuffer presentation. It does not isolate wgpu device creation or shader
translation. No live mode, input-to-photon latency, energy, renderer-only
cost, or shipping accessibility/IME qualification is measured. Windows
working set is not G4's macOS physical footprint.

### Reproduction and validation

The [Windows README](../egui/README.md#g6-windows) gives the release build,
desktop launch, supported cache reset and summary commands. The committed
`windows-measure.py`, `windows-reset-cache.ps1` and `windows-summary.py`
retain individual launches, input QPC timestamps, CPU/memory endpoints,
PresentMon CSVs, focus observations and the quiet log. Raw evidence is in
both worktrees' `target/g6/{warm,warm-startup,cold,native}/`;
diagnostics are separate.
`target/g6/provenance-nucbox.json` records data checks and effective tools.
`target/g6/checks/` holds the Mac commands, timings and exits. The Ink
executable SHA-256 is
`47997e9150f7989c55df2b7b85dd2f42dd355d7d3aab51f19fe71d366c2bc3cf`;
egui is `880cafe46277d29a7b86892f5ebcc68d3bc469987640d7cad049aa536b194b2e`.
Both Ink arms use the same bytes.

The fling process-CPU medians were checked against raw endpoints: D3D11
round 3 is `(5796.875 − 234.375) / 10.0421222 = 553.9168 ms/s`; wgpu round 3
is `(1984.375 − 765.625) / 10.0379882 = 121.4138 ms/s`; egui round 4 is
`(421.875 − 171.875) / 10.0369905 = 24.9079 ms/s`. QPC frequency is
10,000,000 ticks/s in every sample.

All six Mac checks pass: root all-target build, tests, clippy plus fmt,
staged caps, boot, and the workspace all-target check. Root tests report
**7,257 passed, 82 ignored**, with `RUST_TEST_THREADS=2`. Egui's separate
clippy (`-D warnings`) and fmt pass; clippy first found a redundant return
in the conditional Mac decoder, which was removed. Native egui release
build and clippy also pass. Each checkout uses its own `target/`.

S2.8b's Windows qualification was rebuilt with native `fxc`: Shelf on
unmodified `gpu-dev`, native Ink/Windows libraries, and exact-linux test
executables with only that test crate's optimization set to zero. Required
GPU flags and `--test-threads=1` apply. Both devices pass **42** Ink library
tests (**27 ignored**), **35** Windows tests (**1 ignored**), pinned paint
**56** (**1 ignored**), borders **6**, tint **2**, backdrop **5**, gradients
**1**, visual **8**, materials **1**, and image **3**. The window filter with
ignored tests enabled passes **9**; composition passes **12**; all ten
S2.8b startup/fallback fault journeys pass. The six desktop journeys give:

| Journey | Ink D3D11 | Ink wgpu/DX12 |
| --- | --- | --- |
| Resize | Pass | Pass |
| Minimize/restore | Pass | Pass |
| DPI 100→150→100→150→100% | Pass with Settings operator | Fail: final image sizes differ |
| Idle caret | Pass | Pass |
| Forced device loss | Fail: recovered background tint differs | Pass |
| Sleep/wake | Pass | Pass |

The first DPI attempts timed out without the Settings operator. D3D11 then
passed. Wgpu completed all four scale transitions, but its final comparison
received **5120×1369 versus 1024×720** images; this repeated on the third
attempt, where investigation stopped. D3D11 recovered its hardware device
but lost Shelf's tinted background: comparison mean **8.8669/255**, with
**23.1606%** of pixels over 32, against limits 5 and 6%. The same failure
repeated once. G6b below fixes recovery's Mica reapplication and the DPI
fixture's accidental maximization; G6c reruns the six D3D11 journeys.
Desktop scaling was restored to 100% and every measured launch independently checked 96 DPI.


### G6b: the D3D11 fling, traced

The G6 wheel trajectory, window, desktop and device selections are unchanged.
The source is `636f1fe2ae`. The release profile keeps its optimization and adds
PDB line tables (`CARGO_PROFILE_RELEASE_DEBUG=1`,
`CARGO_PROFILE_RELEASE_STRIP=none`). A WPR `GeneralProfile` capture located
resource creation and driver work; the attribution below uses a second,
lighter ETW capture restricted to the fling: `xperf -on
PROC_THREAD+LOADER+PROFILE -stackwalk Profile -buffersize 1024 -minbuffers 128
-maxbuffers 256`. `xperf -d` stops it after the CPU endpoint. The rest interval
of this diagnostic is excluded because trace merging delays that endpoint.
Microsoft's symbol server and the executable's matching PDB resolve system
and application stacks; AMD's private driver functions remain unnamed.

One sampled fling per arm gives **532.56/365.94 ms/s** process/main CPU on
D3D11 and **129.08/115.08 ms/s** on wgpu. Sample counts are apportioned to
these independently measured thread CPU totals; they are estimates of CPU,
not elapsed GPU time. The five unprofiled G6 medians remain the before series.

| CPU attribution, ms/s | Ink D3D11 | Ink wgpu/DX12 |
| --- | ---: | ---: |
| Main: clipping, including mask targets and device calls | 173.71 | 28.43 |
| Main: remaining paint-frame work | 135.54 | 61.73 |
| Main: outside paint-frame work | 56.69 | 24.92 |
| Other threads, total | 166.62 | 14.00 |

The D3D11 worker stacks include about **146 ms/s** beneath AMD's driver
thread entry; image decode accounts for about **8 ms/s**, versus about
**11 ms/s** on wgpu. The driver stacks include condition-variable and
critical-section work. Private symbols are unavailable, so these samples
cannot establish which internal queue, spin policy or allocation routine
owns each driver cycle.

Within the main-thread totals (overlapping inclusive stacks), D3D11 texture
creation is about **58 ms/s**, completion polling **18**, rendering device
calls **78**, texture uploads **3**, and DXGI `Present` **3**. Wgpu texture
creation is about **1 ms/s** and synchronous readback **13**. The D3D11
instance/constant-buffer `UpdateSubresource` path is about **3 ms/s**;
texture uploads pass a bounded region and its row pitch. These CPU stacks
do not reveal the driver's internal copy extent. There is no ordinary
D3D11 frame readback in the stacks. The DXGI display probe reads statistics,
not pixels; its DWM synchronization runs at readiness. The latency handle is
polled with zero timeout. Composition commits occur at content attachment,
not every frame. Neither `Present`, readback nor a DWM wait explains the loss.

The Windows host has no exporter for S1.4's Apple phase marks. Its agent
`perf` runs the shared headless presenter, so it does not time this desktop
window. Four supported 600-pixel wheel operations give identical plan work
on both devices: 231 walked sites, nine constructed and four retired rows,
zero rebound rows (collection site 4, `app.contract:92`). This checks shared
plan work, not desktop pacing. The AppKit `fling`/phased-wheel forms are not
supported by this Windows agent.

A separate counter diagnostic found the D3D11 viewport-target pool retaining
**1,201 targets / 2,866,181,696 bytes**, versus **16 / 38,183,936 bytes** on
wgpu, with at most 16 targets used in one paint. These are logical texture
bytes in an instrumented run, not the process working set from G6. The
counter logging is excluded from the performance series.

**Prediction before the change.** Submit the completion query after `Present`
with an asynchronous D3D11 `Flush`. If a query left in the runtime's command
buffer prevents reuse, the target pool should stay bounded near the wgpu
arm, process CPU should fall below **250 ms/s**, and gesture cadence should
rise above **45 displays/s**. This changes neither resource retirement nor
completion requirements. It adds no completion wait. The result below must
confirm or reject that hypothesis.

**The flush hypothesis is rejected.** With the same counter probe, the trial
still uses **543.29/372.05 ms/s** process/main CPU and retains **1,320
targets / 3,150,174,720 bytes**. The flush is removed. These are diagnostic
observations, not a five-sample after series.

| D3D11 measure | G6: five quiet samples, median [range] | Predicted | Flush trial: one diagnostic |
| --- | ---: | ---: | ---: |
| Process CPU, ms/s | 553.92 [504.33–583.87] | <250 | 543.29 |
| Main CPU, ms/s | 358.13 [347.11–370.30] | Lower | 372.05 |
| Gesture displays/s | 26.05 [25.73–50.51] | >45 | 34.24 |
| Display interval p95, ms | 83.35 [33.35–83.37] | Lower | 50.04 |

The diagnostic has counter logging and no quiet-series qualification; its
numbers do not establish improvement over G6. PresentMon loses no events.
The [asynchronous flush](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11devicecontext-flush)
submits work without waiting for completion, but submission was not the cause.

The decisive counter is paint attempts: **6,337** completed painter snapshots
for **145** successful host frames before the trial, **6,408/152** with the
flush. A refused present still finishes the painter's work. The direct-window
presenter marks that picture dirty and records a refresh-period retry deadline;
`App::about_to_wait` immediately requests another redraw for the dirty flag,
and `App::draw` does not check that deadline. It rebuilds and submits the
clip masks before polling the latency handle again. Failed presents do not
increment the host's successful-frame counter. This explains the repeated
painter work, the unreusable targets and the driver's extra queue work.
The composition commit, image uploads and kernel are not the dominant cause.

#### Current build: five quiet flings

After removing the probes and flush, the release executable includes the two
qualification fixes below and the original carrier pacing. Its SHA-256 is
`94693e70bce45d7ae14d1926e68bb62ae1e888af7cc0a5bb0e6c334dcf2dfcb6`;
it retains the same PDB build settings as the sampled executable. Five runs
use `windows-measure.py --fling-only --arm d3d11 --out target/g6b/final/warm`.
All 24 quiet observations pass (maximum total CPU 9.6%, no compiler), every
window is 800×800 outer at 96 DPI on the 60 Hz display, and PresentMon loses
no events. No focus is lost and no within-gesture posting gap exceeds 25 ms.
These measure the current build, not an accepted performance fix.

| D3D11 measure, median [range] | G6 before | G6b current build |
| --- | ---: | ---: |
| Fling process CPU, ms/s | 553.92 [504.33–583.87] | 540.41 [484.16–583.85] |
| Fling main CPU, ms/s | 358.13 [347.11–370.30] | 362.61 [354.95–365.88] |
| Rest process CPU, ms/s | 4.66 [1.55–6.21] | 0.00 [0.00–3.11] |
| Rest main CPU, ms/s | 3.11 [1.55–6.21] | 0.00 [0.00–3.11] |
| Gesture displays/s | 26.05 [25.73–50.51] | 25.88 [25.23–26.66] |
| Display interval p95, ms | 83.35 [33.35–83.37] | 83.36 [83.35–83.36] |
| Late intervals/gesture second | 12.41 [7.39–12.84] | 12.86 [12.63–13.37] |
| Peak working set, MiB | 220.88 [176.90–223.93] | 219.53 [212.16–226.40] |
| Ending working set, MiB | 220.88 [176.65–223.93] | 219.28 [211.91–226.22] |

The fling loss remains: process CPU is 13.51 ms/s lower, main CPU 4.48 ms/s
higher, and cadence 0.17 displays/s lower, with p95 unchanged at 83 ms.
The qualification fixes do not close the performance issue. No improvement
is attributed to them.

Artifacts are in `target/g6b/` on the NucBox mirror: `profile/` (WPR),
`sampled/{d3d11,wgpu}/` (fling-only ETLs, xperf exports and CPU endpoints),
`release-symbols/`, `perf-plain/`, `instrument/`, `trial1/`, and `final/warm/`. They are local artifacts,
not checked-in results. The mirror remains the S2.8b mirror; no other
checkout is used.

#### Qualification failures and the requested bisect range

The preserved G6 test executable drives a freshly built Shelf at the low endpoint
`d1fa3c00dd`, in the same mirror, then the mirror returns to the lane. Both
failures reproduce there. The journey source is identical between that
endpoint and G6. There is no passing low endpoint for a bisect over
`d1fa3c00dd..48653eb5cb`; neither failure can be assigned to S1's
`270e56d396`, `0312b493fc` or `09c9086896` from this evidence.

| Failure | Low endpoint `d1fa3c00dd` | G6 source rerun | Fixed, at 100% scale |
| --- | --- | --- | --- |
| D3D11 recovery tint | Fail: mean 5.24/255, 0% over 32 | Fail: 5.62/255, 0% over 32 | Pass: 2.39/255, 0% over 32 |
| Wgpu DPI final comparison | Fail: 5120×1369 vs 1024×720 | Same failure | Pass: 1024×720; mean 0.000126/255, 0% over 32 |

The device-loss fix is `b7c35de450`: reapply the HWND's Mica settings when
recovery reuses the composition visual, and retain the new availability
result for opaque fallback. The shared-visual path comes from `8c2ed78cc1`,
before the requested range; this is code provenance, not a measured first-bad
commit. The retained visual had skipped `set_backdrop`. The recovery journey
now passes after a real `dxcap -forcetdr`, through CPU/WARP and back to
hardware. The recovered sidebar is tinted in the inspected capture.

The DPI failure is the fixture's: an independent HWND observer records
`IsZoomed=true`, outer bounds −8,−8..5128,1400 and client 5120×1369. `BitBlt`
was capturing the requested window, not the desktop. The repeated activation
clicks used the same title-bar point, allowing a double-click to maximize
Shelf. That click was introduced in `0c39c9e8c5`, before the requested range.
`cc87abf43d` clicks Shelf's empty lower-right client margin instead. The fixed
observer sees no maximized window and the final capture is 1024×720.

The first fixed DPI attempt lost its Settings UI Automation control reference
on the fourth transition. The operator now reacquires the window and control
for each selection. The final attempt passes 96→144→96→144→96 DPI. An initial
recovery check after that operator failure ran at 150% and passed, but is
excluded from the table; the table uses the subsequent 100% native suite.
The original failure artifacts, rejected trial and operator failure remain
under `target/g6b/`; no attempt is removed from the record.

#### Validation and remaining work

The final native `gpu-dev` build passes the S2.8b/G6 suites: on each device,
42 Ink tests (27 ignored), 35 Windows tests (one ignored), pinned paint 56
(one ignored), borders 6, tint 2, backdrop 5, gradients 1, visual 8, materials 1,
and image 3. All six desktop journeys pass on both devices, including the
Settings-operated DPI round trip, real TDR and sleep/wake. The nine shared
window tests, twelve composition tests and all ten startup/fallback fault
journeys pass.

The first composition suite failed the retirement fixture's second length
assertion: its three-check missing-statistics fallback could release the
first device before the assertion when WARP completed quickly. `af1627ea29`
holds the display condition at an unpresented count, independently of GPU
completion, then releases it explicitly. The unchanged product retirement
code passes all twelve composition tests with that deterministic fixture.
The first failure remains in `native/results.json`; the passing rerun is
`native/composition-fixed.log`.

Mac build, tests, clippy, formatting, staged caps, boot and full-workspace
check pass. Temporary clip/target logging and the flush trial are removed.
The accepted product change restores recovery material; the accepted journey
change prevents accidental maximization. No D3D11 fling fix is claimed.
The five current-build flings above retain the original pacing. G6b left
the carrier change outside its named `host/ink` scope. Charlie subsequently
authorized it; G6c below records the fix and remeasurement.

### G6c: one paint per refresh

**Prediction, recorded before measuring (2026-10-30).** Honoring the direct
presenter's retry deadline and coalescing dirty redraws at the monitor rate
should bring D3D11 fling process CPU below **150 ms/s**, main-thread CPU below
**100 ms/s**, cadence above **55 displays per gesture second** on this 60 Hz
desktop, display-interval p95 below **20 ms**, and late intervals below **1 per
gesture second**. Completed painter snapshots should approximately equal
successful frames. The ten-launch startup series should remain within G6's
retained-driver distribution. These predictions are checked against the five
quiet fling/rest samples and ten fresh-process launches below.

The carrier retains early redraw requests and honors a refused present's
retry deadline in both `WaitUntil` scheduling and admission to `draw`.
Before allocating paint targets, DirectX 11 waits up to one refresh period
(rounded to whole milliseconds) for its latency handle. A successful wait
reserves that frame's present, which consumes the reservation without another
poll. Resize discards the old reservation with its handle. This follows
[DXGI's wait-before-render contract](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_3/nf-dxgi1_3-idxgiswapchain2-getframelatencywaitableobject).
Continuous animation keeps the carrier's cadence; ordinary direct redraws
use the waitable without a second timer delay. Readiness/recovery polling
and device retirement keep their own deadlines. Recovery backoff does not
delay changed content on the CPU fallback.

The exit counters count completed painter snapshots and successful carrier
frames over the whole process, including startup. They write once, after the
CPU endpoints. G6b's diagnostic counted the same snapshots with per-paint
logging; its **6,337/145** is diagnostic evidence, not an unprofiled sample.

The first carrier revision (`a13a063ba1`) had one successful diagnostic:
**79.44/60.75 ms/s** process/main CPU, **58.28 displays/s**, **16.75 ms** p95
and **246 snapshots / 246 successful frames**. Its first quiet launch then
timed out after one frame: a redraw from image completion arrived before
the cadence deadline, and deferring it lost the request because image polling
had not yet marked the picture dirty. The carrier now retains an early redraw
request through the deadline, independently of the dirty flag. A windowless
scheduling test covers that wake. Both attempts remain under `target/g6c/round1/`;
neither enters the final series.

The second revision (`5f0fce2d00`) meets all six fling predictions with
**85.66/63.86 ms/s** process/main CPU, **58.28 displays/s**, **16.77 ms** p95,
and **1,240/1,235** snapshots/frames over five quiet processes. Its additional
timer pacing moves startup: a separate quiet, alternating comparison of
G6b's preserved `636f1fe2ae` executable and this revision gives DWM
**177.64 [173.65–195.50]** versus **187.01 [170.20–222.40] ms**. The median
paired increase is **9.168 ms**. All 25 observations pass, at most 6.6% CPU.
That result motivates the pre-paint waitable reservation in the final
revision. The timer trial remains under `target/g6c/round2/`; the paired
control is under `target/g6c/paired-startup/`. A prior timer-trial startup
series had an 11.4% CPU observation and is retained separately as
`round2/startup-loaded/`; none of its launches is selected for a final table.

#### Final measurement

Source **`3020f4b76c`**, release with G6b's PDB line tables retained
(`CARGO_PROFILE_RELEASE_DEBUG=1`, `CARGO_PROFILE_RELEASE_STRIP=none`).
The executable SHA-256 is
`d5eb5c060b0c528dc62e96bed36cdf41f2ae48a72f7c127371d1b40b0b52a9b2`.
Five fresh processes run the unchanged G6 four-gesture, ten-second fling and
ten-second rest. All open 800×800 outer / 784×761 client at 96 DPI on the
60 Hz desktop. The DWM-paced driver posts one wheel event per refresh.
The five flings pass all **24** quiet observations (maximum **6.6%** CPU);
the ten separate startup launches pass all **11** (maximum **9.3%**).
There are no compiler processes, focus losses or lost ETW events.

Medians **[minimum–maximum]**, five flings in each column. G6 and G6b remain
historical series; G6c replaces their D3D11 performance conclusion.

| Metric | G6 | G6b before | G6c after |
| --- | --- | --- | --- |
| Fling process CPU, ms/s | 553.92 [504.33–583.87] | 540.41 [484.16–583.85] | 82.46 [73.11–84.05] |
| Fling main-thread CPU, ms/s | 358.13 [347.11–370.30] | 362.61 [354.95–365.88] | 66.89 [59.11–70.01] |
| Rest process CPU, ms/s | 4.66 [1.55–6.21] | 0.00 [0.00–3.11] | 1.55 [0.00–4.66] |
| Rest main-thread CPU, ms/s | 3.11 [1.55–6.21] | 0.00 [0.00–3.11] | 1.55 [0.00–3.11] |
| Displays / gesture second | 26.05 [25.73–50.51] | 25.88 [25.23–26.66] | 58.25 [58.08–58.46] |
| Display interval p95, ms | 83.35 [33.35–83.37] | 83.36 [83.35–83.36] | 16.77 [16.74–16.93] |
| Late intervals / gesture second | 12.41 [7.39–12.84] | 12.86 [12.63–13.37] | 0.00 [0.00–0.00] |
| Lifetime peak working set, MiB | 220.88 [176.90–223.93] | 219.53 [212.16–226.40] | 153.04 [152.35–153.66] |
| End-of-rest working set, MiB | 220.88 [176.65–223.93] | 219.28 [211.91–226.22] | 153.04 [152.35–153.66] |

Sampled fling peak, end-of-fling and end-of-rest working sets are equal in
each final run. Display interval p50 is **16.67 [16.67–16.68] ms**; p99 is
**17.17 [16.78–17.30] ms**. The full ten-second phase, including pauses,
displays **24.59 [24.41–24.69]/s**. Every rest has zero displays. PresentMon
2.6.0 reports `Composed: Flip`. The driver posts **243 [241–244]** wheel
events, with largest within-gesture gap **20.51 [19.29–22.87] ms** and no
gap over 25 ms. These are displayed presents, not input-to-photon latency.

| Run | Completed painter snapshots | Successful carrier frames |
| --- | --- | --- |
| 1 | 252 | 252 |
| 2 | 251 | 251 |
| 3 | 249 | 249 |
| 4 | 248 | 248 |
| 5 | 251 | 251 |
| Total | **1,251** | **1,251** |

#### Startup and prediction

Ten fresh processes, application and driver caches retained. G6c does not
repeat the driver-cache reset or the two wgpu controls.

| Series | DWM complete-screen proxy, ms | DXGI display timestamp, ms |
| --- | --- | --- |
| G6 original | 175.98 [169.40–190.49] | 187.14 [182.01–200.87] |
| G6c final | 182.25 [171.22–211.84] | 194.67 [182.68–223.86] |

The contemporaneous control alternates ten before and ten after processes,
reversing order each pair, five seconds apart, at the same executable path
with the same assets and application cache. Each launch runs a freshly copied
executable image; OS prefetch and real-time scanning can differ from the
ordinary startup series. The committed `egui/windows-paired-startup.py` and
`egui/windows-paired-summary.py` reproduce this control. Before is G6b's preserved
`636f1fe2ae` release with matching PDB settings (SHA-256
`6d69997deac5ba5d8b34cf3c61ab04005937301ab22940479b445ab6af6e16e2`);
the later G6b fixes concern recovery and the DPI fixture. After is the final
G6c binary above. All **25** observations pass, maximum **6.9%** CPU.

| Alternating startup control | DWM proxy, ms | DXGI display, ms |
| --- | --- | --- |
| Before | 184.29 [175.46–194.14] | 196.22 [185.47–207.14] |
| After | 188.43 [182.65–215.32] | 199.48 [194.02–227.25] |
| Per-pair after minus before | +3.28 [−10.72–37.23] | +2.08 [−12.81–38.31] |

The median paired DWM increase is **3.281 ms**, reduced from the timer
revision's 9.168 ms. The distributions overlap, but these ten pairs do not
establish unchanged startup; the original G6 range prediction is missed.
This is the third performance revision, so further optimization stops here.
`QUEUE.md` retains the startup question. The first final paired attempt had
one 10.5% CPU observation among 25; its whole series is excluded and retained
under `paired-waitable-loaded/`. The table uses the complete quiet repeat,
without selecting individual launches.

| Prediction made before measuring | Final result |
| --- | --- |
| Process CPU <150 ms/s, at or below the 121.41 ms/s wgpu control | Pass: 82.46, maximum 84.05 |
| Main-thread CPU <100 ms/s | Pass: 66.89, maximum 70.01 |
| >55 displays / gesture second | Pass: 58.25, minimum 58.08 |
| Display p95 <20 ms | Pass: 16.77, maximum 16.93 |
| Late intervals <1 / gesture second | Pass: zero in all five |
| Snapshots approximately equal successful frames | Pass: 1,251/1,251 across the five processes |
| Startup stays within G6's retained-driver distribution | Miss: final 171.22–211.84 ms exceeds G6’s 169.40–190.49 ms; quiet paired DWM increase +3.28 ms |

#### Native qualification and remaining work

The final Shelf `gpu-dev` build passes all six D3D11 desktop journeys on
their first attempt. Pixel comparisons use S2's fixed band against a fresh
launch; these are mean channel errors out of 255.

| Journey | Final result |
| --- | --- |
| Resize | Pass; mean 0.020671, 0.0296% of pixels over 32 |
| Minimize/restore | Pass; identical pixels |
| DPI 100→150→100→150→100% | Pass with Settings operator; final 1024×720 at 96 DPI, identical pixels |
| Idle caret | Pass; three transitions, 20 changed pixels; fresh comparison identical |
| Forced device loss | Pass; CPU-to-hardware 453.445 ms, identical recovered pixels |
| Sleep/wake | Pass; sleep returned after 75.607 s, mean 0.000277, no pixels over 32 |

The recovered Shelf capture was inspected: its sidebar tint and content are
present. The timer revision's first TDR journey had a separate transient
failure: Mica reapplication returned `0x80263001` (desktop composition
disabled), leaving opaque fallback after hardware recovered. Its mean error
was **5.2427/255**, above the limit of 5; no pixel exceeded 32. An unchanged
rerun passed with identical pixels and **405.161 ms** recovery. Both attempts
remain under `round2/native/` and `round2/journeys/recovery-rerun/`. The saved
`round2/native/first-recovery-dwm-excerpt.txt` records the child-log error
read before the fixture reused that log path. The final successful journey
does not close this observed case: `QUEUE.md` now asks for retrying Mica
when DWM returns. G6b's redraw backpressure item is resolved here.

On each device, the final NucBox suites pass 42 Ink library tests (27 ignored),
38 Windows tests (one ignored), pinned paint 56 (one ignored), borders 6,
tint 2, backdrop 5, gradients 1, visual 8, materials 1 and image 3. The nine
shared-window tests, twelve composition tests and ten startup/fallback fault
journeys pass with ignored tests explicitly enabled. The three new carrier
tests exercise deadline admission and dirty/retry interaction without a
window, including the deferred image redraw. Composition tests verify a
blocked latency handle causes no paint reservation, a reservation survives
until present without a second wait, and hiding preserves an unused
reservation. Existing retirement and recovery tests pass.

All six Mac checks pass: root all-target build, tests, clippy plus fmt,
staged caps, boot and full-workspace all-target check. Root tests report
7,257 passed and 82 ignored with `RUST_TEST_THREADS=2`; the separate
`cargo test -p exact-linux --lib --tests --no-fail-fast` also passes.
Native suites use S2.8b's native `fxc`, required GPU flags and one test
thread; Shelf keeps unmodified `gpu-dev`, while only the exact-linux test
crate uses optimization level zero.

Raw evidence is retained in both worktrees: `target/g6c/final/{warm,warm-startup}/`
contains the JSON, process logs, PresentMon CSVs and quiet observations;
`native/` and `journeys/final-d3d11-dpi/` contain qualification;
`checks/` contains Mac checks. `round1/`, `round2/`, `paired-startup/`,
`paired-waitable-loaded/` and `paired-waitable/` retain the earlier trials
and startup controls. The release was built with the G6 README command
and the PDB settings above.
To repeat the final timed series on the signed-in desktop:

```powershell
python bench/heavy-list/egui/windows-measure.py --arm d3d11 --fling-only --out target/g6c/final/warm
python bench/heavy-list/egui/windows-measure.py --arm d3d11 --startup-only --out target/g6c/final/warm-startup
python bench/heavy-list/egui/windows-summary.py target/g6c/final
```

Use a new output directory for another attempt. No live mode, energy,
input-to-photon latency or isolated renderer cost was measured. G6's
whole-application and readiness non-comparables still apply. Egui and
Ink/wgpu were not remeasured; their DX12 backend selections and original
numbers remain controls, not new samples.

## G6d: the Windows reviews’ lifecycle fixes

Source **`d7fe8c32cb`**, measured before rebasing; `measured-source.bundle` retains it.
Release executable SHA-256:
`76e8eaf04a109588cddab88be65c930863b77521e516ccff9b774dfe544c5a2e`.

**Finding:** the lifecycle fixes pass native qualification. CPU, working set
and p95 remain close to G6c, but cadence is not fully preserved: displays fall
from 58.25 to 56.41 per gesture second, late intervals rise from zero to
0.993/s, and p99 rises from 17.17 to 33.33 ms. G6d stopped after its third
performance revision. G6e below restores cadence and records the remaining
CPU gate miss.

The reservation path keeps a non-device-lost error as a refused frame, logs once
per failure streak, and retries one refresh period later. Restore, focus, resize
and DPI events clear presentation backoff without shortening device recovery.
After a failed material setup, successful presents retry Mica at most three
times and request the transparent replacement frame when it returns. A clean
redraw checks device removal without reserving capacity, so TDR still reaches
CPU/WARP recovery. Capacity timeouts back off on consecutive refusals.
Each reservation wait requests at most half a refresh period (8 ms at 60 Hz).
Two bounded waits share the first period, with the carrier’s deadline giving
the event loop a turn between them; further refusals back off to one second.
Occlusion keeps its idle backoff. A clean, noncontinuous frame reserves nothing.
Geometry and CPU snapshots preserve damage still owed to the composition window.

Readiness is checked after first-pixel data activation and requires a clean
picture. Shelf’s baked screen can remain unchanged by activation; the prior
probe waited for an unnecessary second paint, which the clean-frame guard now
skips. A changed picture still needs its next successful present. The DWM proxy
and DXGI timestamp keep their definitions, but this removes that extra dispatch
from unchanged-screen startup and must be considered beside G6c.

Windowless tests exercise reservation errors, clean-frame admission, damage
retention, lifecycle reset and the carrier’s `about_to_wait` control flow with
a stubbed presentation deadline and an earlier runner timer. The native modal
fixture shares its one Winit event loop with the carrier test. Capacity and
material-retry tests cover bounded refusal and recovery. Desktop captures now
use `AttachThreadInput` for foreground activation; Settings’ protected
`ApplicationFrameHost` refuses attachment, so the fixture then requests focus
through UI Automation. Neither path sends a client click.

### Fling and rest

Five fresh-process D3D11 fling/rest samples, the same 800×800 outer window,
784×761 client, 96 DPI and 60 Hz NucBox desktop as G6c. The release retains
PDB line tables (`CARGO_PROFILE_RELEASE_DEBUG=1`, `CARGO_PROFILE_RELEASE_STRIP=none`).
The unchanged driver posts one wheel event per `DwmFlush` return. No build,
test or cache reset overlaps collection. Application, driver and OS caches
are retained. Each process runs four gestures within ten seconds, then rests
for ten seconds. All 24 fling quiet observations pass (maximum 7.6% CPU);
all 11 startup observations pass (maximum 6.6%). There are no compiler
processes, focus losses or lost ETW events. Values are median [minimum–maximum].

| Metric | G6c | G6d |
| --- | --- | --- |
| Fling process CPU, ms/s | 82.46 [73.11–84.05] | 84.07 [74.78–87.22] |
| Fling main-thread CPU, ms/s | 66.89 [59.11–70.01] | 63.89 [51.41–68.56] |
| Rest process CPU, ms/s | 1.55 [0.00–4.66] | 1.55 [0.00–3.11] |
| Rest main-thread CPU, ms/s | 1.55 [0.00–3.11] | 0.00 [0.00–1.55] |
| Displays / gesture second | 58.25 [58.08–58.46] | 56.41 [56.04–56.46] |
| Display interval p95, ms | 16.77 [16.74–16.93] | 16.75 [16.74–16.75] |
| Display interval p99, ms | 17.17 [16.78–17.30] | 33.33 [33.28–33.35] |
| Late intervals / gesture second | 0.000 [0.000–0.000] | 0.993 [0.742–0.995] |
| Lifetime peak working set, MiB | 153.04 [152.35–153.66] | 153.85 [152.94–153.91] |
| End-of-rest working set, MiB | 153.04 [152.35–153.66] | 153.85 [152.94–153.91] |

PresentMon 2.6.0 reports `Composed: Flip`. Display p50 is
16.68 [16.67–16.68] ms; the full ten-second phase, including pauses,
displays 23.02 [22.93–23.04]/s. Every rest has zero displays. The driver
posts 244 [243–244] wheel events; its largest within-gesture gap is
18.29 [18.22–23.51] ms, with no gap above 25 ms.

| Run | G6c snapshots / frames | G6d snapshots / frames |
| --- | --- | --- |
| 1 | 252 / 252 | 234 / 234 |
| 2 | 251 / 251 | 234 / 234 |
| 3 | 249 / 249 | 233 / 233 |
| 4 | 248 / 248 | 233 / 233 |
| 5 | 251 / 251 | 232 / 232 |
| Total | 1,251 / 1,251 | 1,166 / 1,166 |

### Startup

Ten separate fresh-process launches, five seconds apart, with application,
driver and OS caches retained. The egui and Ink/wgpu arms and driver-cache-cold
series were not remeasured. DWM is the complete-screen synchronization proxy;
DXGI identifies the corresponding displayed composition frame.

| Series | DWM complete-screen proxy, ms | DXGI display timestamp, ms |
| --- | --- | --- |
| G6c | 182.25 [171.22–211.84] | 194.67 [182.68–223.86] |
| G6d | 161.84 [155.63–181.27] | 176.85 [167.07–192.07] |

The complete-screen proxies are lower than G6c’s, but readiness now follows
data activation in the same draw. These historical before/after series do not
isolate the scheduling change or close G6c’s paired-startup question.
G6c’s separate paired control is reproducible with `windows-paired-startup.py`
and `windows-paired-summary.py`; each control launch runs a freshly copied image.
Its OS prefetch and real-time scanning conditions differ from ordinary launches.

### The shortened-wait trials

Two complete quiet trials precede the final scheduler. They are retained,
not mixed into the final table. Both preserve one snapshot per successful
frame, but neither preserves G6c's cadence.

| Revision | Process CPU, ms/s | Displays / gesture second | Display p95, ms | Late intervals / gesture second | Snapshots / frames |
| --- | --- | --- | --- | --- | --- |
| Half-period wait, original retry backoff (`cf02ff55f1`) | 88.83 [79.45–93.43] | 43.21 [41.04–45.50] | 50.05 [50.01–50.05] | 8.14 [6.96–8.65] | 912 / 912 |
| First retry at the end of the period (`b2fa19453d`) | 85.69 [74.76–88.82] | 55.38 [54.89–55.47] | 16.77 [16.76–16.85] | 1.74 [1.24–1.99] | 1,147 / 1,147 |

The first deadline added two periods after the partial wait. Subtracting the
elapsed wait restored p95, but still missed display slots. The final revision
splits the first reservation period across two bounded waits, returning to the
carrier between them; successive timeouts then increase the idle delay.
These are the three performance revisions of G6d; no fourth was attempted.
`timeout-trial/` and `deadline-trial/` retain both earlier series, their ten
startup launches, quiet observations and qualification. Their fling/startup
quiet maxima are respectively 8.4%/6.5% and 7.3%/6.2%, with no loaded observation.

### Native qualification

All six D3D11 desktop journeys pass on the final `gpu-dev` Shelf build.
The pixel columns use S2's fixed band against a fresh launch; mean channel
error is out of 255.

| Journey | G6c | G6d |
| --- | --- | --- |
| Resize | Pass; mean 0.020671, 0.0296% over 32 | Pass; mean 0.020690, 0.0301% over 32 |
| Minimize/restore | Pass; identical pixels | Pass; identical pixels |
| DPI 100→150→100→150→100% | Pass; final pixels identical | Pass; final 1024×720 at 96 DPI, identical pixels |
| Idle caret | Pass; three transitions, 20 changed pixels | Pass; three transitions, 20 changed pixels; fresh comparison identical |
| Forced device loss | Pass; CPU-to-hardware 453.445 ms, identical pixels | Pass; 398.810 ms, identical pixels; refused Mica restored |
| Sleep/wake | Pass; 75.607 s, mean 0.000277, no pixels over 32 | Pass; 76.606 s, mean 0.000277, no pixels over 32 |

Both 150% DPI captures pass against a fresh 150% launch: mean 0.001828,
0.002713% of pixels over 32. The recovery log records a real DWM material
refusal on WARP and the fixture's injected refusal on hardware attachment;
both are followed by `Mica retry 1/3 after present: restored`. The recovered
capture was inspected: sidebar material and content are present. This closes
G6c's queued transient-Mica case. Sleep/wake retains S2's limitation: it does
not isolate the wake source or establish a timer-only wake. Lock/unlock was
not repeated.

The NucBox passes 44 Ink library tests (27 ignored), 39 Windows tests (one
ignored), pinned paint 56 (one ignored), borders 6, tint 2, backdrop 5,
gradients 1, visual 8, materials 1 and image 3 on each of D3D11 and wgpu.
Ten shared-window tests, fourteen composition tests and all ten
startup/fallback fault journeys pass with ignored tests explicitly enabled.
Builds use S2.8b's native `fxc`, required GPU flags and one test thread.
Shelf keeps unmodified `gpu-dev`; only the exact-linux test crate uses
optimization level zero. UI Automation is a test dependency.

All six Mac checks pass on the measurement source: all-target build, root
tests, clippy plus fmt, staged caps, boot and full-workspace all-target check.
Root tests report 7,257 passed and 82 ignored with `RUST_TEST_THREADS=2`.
The separate exact-linux library and integration suites also pass.

The branch was rebased onto `origin/main` **`175bdab9d1`**. Only queue placement
needed conflict resolution; the source patches are unchanged. All six Mac
checks pass again: **7,272 tests passed, 83 ignored**. On the rebased NucBox
mirror, Shelf and the native test binaries rebuild; Ink 44, Windows 39,
shared-window 10 and composition 14 tests pass on each of D3D11 and wgpu.
The timed series and six desktop journeys above remain measurements of the
pre-rebase source; they were not repeated after rebasing.

Raw evidence is retained in both worktrees under `target/g6d/`:
`final/{warm,warm-startup}/` holds process samples, PresentMon CSVs and quiet
observations; `native/`, `preflight/` and `journeys/` hold qualification.
`device-loss-recovery.log` and `device-loss-recovered.png` retain material
recovery. `initial-native/`, `qualification3-native/`, `qualification3-journeys/`,
`qualification4-journeys/` and the trace logs retain failed qualification and
diagnostic runs, including the readiness-ordering, idle-removal and foreground
refusals. No timed sample is drawn from those runs. `checks-complete/` holds
the Mac checks before rebasing; `checks-rebased/` and `native-rebased/` hold
the post-rebase checks. The source bundle preserves the measured revision
despite rewritten commit IDs.

To repeat the timed series on the signed-in desktop after a release build:

```powershell
python bench/heavy-list/egui/windows-measure.py --arm d3d11 --fling-only --out target/g6d/final/warm
python bench/heavy-list/egui/windows-measure.py --arm d3d11 --startup-only --out target/g6d/final/warm-startup
python bench/heavy-list/egui/windows-summary.py target/g6d/final
```

Use a new directory for another attempt. Egui and Ink/wgpu, driver-cache-cold
startup, live mode, energy, input-to-photon latency and isolated renderer cost
were not remeasured. G6's whole-app and readiness non-comparables still apply.


## G6e: the full-period reservation wait restored

**Cadence is restored; the strict CPU gate is missed.** All five quiet flings
have zero late intervals, p99 below 20 ms and at least 58 displays per gesture
second. Process CPU is **84.08 [74.70–93.39] ms/s**: the median exceeds G6c's
84.05 ms/s upper bound by 0.03 ms/s, and two runs reach 91.85 and 93.39 ms/s.
The declared gate therefore fails. No third pacing policy was tried.

Source **`0b34fe1cb1`**, rebased onto `origin/main` **`4cebb42abe`** before
measurement. Release executable SHA-256:
`18fcfefe4f610cce038cea3b4076e5abf5281fe02c32c78d824e2331e208d251`.
The build retains PDB line tables as G6c and G6d did.

The waitable reservation again waits one refresh period rounded up to
milliseconds, as `77ab0960ee` did: 17 ms at this desktop's 60 Hz. Consecutive
timeouts still grow the retry period to one second, subtracting time already
spent waiting. The clean-frame reservation skip is removed after the trial
below. Reservation errors remain refusals; lifecycle deadline resets, idle
device-loss detection, damage retention, readiness ordering and bounded Mica
recovery remain. The carrier still submits no frames during rest.

Five fresh-process fling/rest samples and ten separate fresh-process startup
launches use the unchanged DWM-paced driver, 800×800 outer / 784×761 client
window, 96 DPI, 5120×1440 desktop at 60 Hz, Radeon 8060S and driver
32.0.23033.1002. Application, driver and OS caches are retained. No build or
test overlaps collection. All **24** fling quiet observations pass (maximum
**6.7%** CPU), as do all **11** startup observations (maximum **6.9%**).
There are no compiler processes, focus losses or lost ETW events.

Medians [minimum–maximum]. G6c and G6d are historical series. Startup uses ten
launches; the other distributions use five.

| Metric | G6c | G6d | G6e |
| --- | --- | --- | --- |
| Fling process CPU, ms/s | 82.46 [73.11–84.05] | 84.07 [74.78–87.22] | 84.08 [74.70–93.39] |
| Fling main-thread CPU, ms/s | 66.89 [59.11–70.01] | 63.89 [51.41–68.56] | 62.31 [57.58–70.07] |
| Rest process CPU, ms/s | 1.55 [0.00–4.66] | 1.55 [0.00–3.11] | 0.00 [0.00–6.22] |
| Rest main-thread CPU, ms/s | 1.55 [0.00–3.11] | 0.00 [0.00–1.55] | 0.00 [0.00–6.22] |
| Displays / gesture second | 58.25 [58.08–58.46] | 56.41 [56.04–56.46] | 58.15 [58.07–58.53] |
| Display interval p95, ms | 16.77 [16.74–16.93] | 16.75 [16.74–16.75] | 16.74 [16.74–16.75] |
| Display interval p99, ms | 17.17 [16.78–17.30] | 33.33 [33.28–33.35] | 16.85 [16.77–16.93] |
| Late intervals / gesture second | 0.000 [0.000–0.000] | 0.993 [0.742–0.995] | 0.00 [0.00–0.00] |
| Lifetime peak working set, MiB | 153.04 [152.35–153.66] | 153.85 [152.94–153.91] | 153.18 [152.79–153.39] |
| End-of-rest working set, MiB | 153.04 [152.35–153.66] | 153.85 [152.94–153.91] | 153.16 [152.79–153.39] |
| DWM complete-screen proxy, ms | 182.25 [171.22–211.84] | 161.84 [155.63–181.27] | 160.78 [151.81–171.37] |
| DXGI displayed complete screen, ms | 194.67 [182.68–223.86] | 176.85 [167.07–192.07] | 177.66 [170.41–182.54] |
| Painter snapshots / successful frames, total | 1,251 / 1,251 | 1,166 / 1,166 | 1,247 / 1,247 |

PresentMon 2.6.0 reports `Composed: Flip`. Display p50 is
16.67 [16.67–16.68] ms; the full ten-second phase, including pauses, displays
24.51 [24.40–24.61]/s. Every rest has zero displays. The driver posts
242 [241–243] wheel events; the largest within-gesture posting gap is
19.19 [18.83–19.91] ms, with none above 25 ms. Painter snapshots and successful
frames are equal in every run: **250, 250, 248, 250, 249**.

The CPU median is independently reproduced from run 1's endpoints:
843.75 process CPU ms / 10.0353022 wall seconds = 84.0781855 ms/s.
The restored presentation cadence meets all three cadence thresholds in every
run. These timestamps count presented frames, including redraws after zero
wheel deltas; they do not establish distinct content changes or input latency.
The retained PresentMon trace has no CPU stacks to attribute the remaining CPU
range miss. Further performance changes stop here.

### The clean-frame skip trial

The first restoration (`b61c1cdd4d`) kept G6d's clean-frame skip. It produced
56.11 [55.86–56.21] displays/s, p99 33.29 [16.90–33.37] ms, and
0.993 [0.247–0.998] late intervals/s. Joining PresentMon display timestamps
to the posted input places all **15** long intervals near the gesture tails:
33.18–33.41 ms gaps starting 946.30–979.48 ms after gesture start, while wheel
deltas alternate between zero and −1. There are no lost ETW events or focus
losses. This retained trial is **not quiet**: one of 24 observations is
10.1% CPU. Its five-run process CPU is 99.63 [77.89–102.78] ms/s, and its
1,165 snapshots equal its 1,165 successful frames.

The trial's ten startup launches are quiet (maximum 6.8% CPU): DWM
165.86 [150.89–179.68] ms and DXGI 179.06 [167.53–191.25] ms. Removing the
skip is the second and final configuration, authorized by G6e's condition
that the skip remain only if measurement preserves the row. Its complete
quiet series is the G6e column above; no sample is selected from the first
trial. G6d's corrected readiness ordering still applies, so these historical
startup distributions do not isolate a scheduling gain or close G6c's
separate paired-startup question.

### Native qualification and checks

All six Mac checks pass on the measured source: all-target build, root tests,
clippy plus fmt, staged caps, boot and full-workspace all-target check. Root
tests pass **7,331**, with **83 ignored**, using `RUST_TEST_THREADS=2`.
The separate exact-linux library and integration suites pass **2,066**, with
**38 ignored**.

All six D3D11 desktop journeys pass on the measured source and an unmodified
`gpu-dev` Shelf build. Pixel errors use S2's fixed band against a fresh launch;
mean channel error is out of 255.

| Journey | G6c | G6d | G6e |
| --- | --- | --- | --- |
| Resize | Pass | Pass | Pass; mean 0.020690, 0.0301% over 32 |
| Minimize/restore | Pass | Pass | Pass; identical pixels |
| DPI 100→150→100→150→100% | Pass | Pass | Pass; final 1024×720 at 96 DPI, identical pixels |
| Idle caret | Pass | Pass | Pass; three transitions, 20 changed pixels; fresh comparison identical |
| Forced device loss | Pass; recovery interval 453.445 ms | Pass; 398.810 ms | Pass; 1,850.191 ms; identical pixels and Mica restored |
| Sleep/wake | Pass; 75.607 s | Pass; 76.606 s | Pass; 75.692 s, mean 0.000403, no pixels over 32 |

The device-loss fixture subtracts the first CPU and recovered Ink **frame-start**
timestamps. G6e's longer interval includes a **1,446.210 ms** first fallback
paint; WARP's composition setup takes **255.336 ms**, and its three Mica
retries are refused. Hardware's injected Mica refusal then recovers on the
first post-present retry. The final capture was inspected: material and content
are present. This single qualification run establishes recovery, not unchanged
recovery latency; no cause of the long first fallback paint was isolated.
Both 150% DPI captures pass, with means 0.001830 and 0.001849 and 0.002713%
of pixels over 32. Sleep/wake retains S2's wake-source limitation.

On each of D3D11 and wgpu, the native suites pass Ink **44** (27 ignored),
Windows **39** (one ignored), pinned paint **56** (one ignored), borders **6**,
tint **2**, backdrop **5**, gradients **1**, visual **8**, materials **1** and
image **3**. The **10** shared-window tests, **14** composition tests and all
**10** startup/fallback fault journeys pass with ignored tests explicitly
enabled. Builds use native SDK `fxc`, required GPU flags and one test thread;
only the exact-linux test crate uses optimization level zero. The timeout
unit test covers the full-period bound and growing backoff at 60/120/144 Hz;
physical qualification remains 60 Hz.

The final branch is rebased onto `origin/main` **`4b06e7e4f5`**. The reservation
patches are unchanged. All six Mac checks pass again, with **7,362 tests passed
and 83 ignored**; logs are in `checks-rebased/`. The timed series and native
qualification above ran on `0b34fe1cb1`, before this final rebase, and were not
repeated afterward. The source bundle retains that revision.

Raw evidence is retained in both worktrees under `target/g6e/`: `final/`
holds the quiet fling and startup series; `skip-trial/` holds the earlier
configuration, including its loaded observation. `skip-trace.json` joins that
trial's display gaps to input. `summary.json` records the final aggregation;
`measured-source.bundle` preserves the measured source. Native logs and
captures are in `native/` and `journeys/`; Mac logs are in `checks/`.

Egui, Ink/wgpu, driver-cache-cold startup, energy, input latency and isolated
renderer cost were not remeasured. G6's fonts, whole-app paths and readiness
non-comparables remain.

## G5: the static screen and the GPU

@ref LLP 1149 §4.1; preparation on 2026-10-29, `lane/g5-static`, based on
`48653eb5cb` (includes G4 at `be5413a907`). The prepared series has three
interleaved rounds each of fling, `bar` and `caret`, using G4's four arms,
screen, data and quiet rule. That series has no cold-only launches or
cold-start comparisons. Each measured phase begins after the complete-scene
readiness handshake.

**Run status: prepared.** The series ran on the development Mac on
2026-10-30, 09:30–09:38 PDT, and was spoiled: only 44 of 91 monitor
observations were quiet, load reached 170 as other lanes built, focus-loss
sidecars accompanied nearly every fling and bar sample, and only 14 power
samples were collected in eight minutes. The raw files remain under
`target/g5/series` on that Mac. No row is taken from this run. G5 will be
measured as **G5m** on the dedicated measurement Mac, the silver M5 Max,
once it is set up.

**Expectation, stated before measurement.** In `bar`, native main-thread
CPU should be near zero because Core Animation runs the animation; here
“near zero” means below 10 ms/s, including the shared display-link probe.
Ink and GPUI should pay the frame's bookkeeping; egui should rebuild visible
rows each frame. In `caret`, every arm should be near idle between the two
visibility changes per second. Continuous frame submissions would falsify
that idle expectation; CPU totals alone cannot identify the work's stacks.

`BENCH_MOTION=bar` paints a #007AFF fill at the top of the title bar, 2 points
high, advancing across 800 points in two seconds. Contract translates the
fill inside a clipped box; GPUI requests animation frames; egui requests a
repaint each frame. `BENCH_MOTION=caret` paints a 1×20-point #007AFF caret
at (112, 10) in that bar. Contract uses `steps(2, jump-none)` so its two states
are opaque and transparent; GPUI's timer and egui's `request_repaint_after`
schedule the next 500 ms boundary. The list stays at row zero and receives no input.
Plain `steps(2)` would produce a half-opacity state. Live mode is off.

The linked probe discovers the concrete Metal queue class from a queue on
the default device, hooks its three command-buffer creation methods, then
hooks `commit` on the first returned buffer's concrete class. Each completion
adds `GPUEndTime - GPUStartTime` and a count to the phase recorded at commit.
Counts, missing timestamps and outstanding buffers are retained; a 250 ms
grace after measurement lets boundary buffers finish. An offscreen three-buffer
check matched the accumulated time to the buffers' own timestamps and kept
all three in their commit phase after the phase changed. GPU times sum buffer
spans, including possible overlaps; they are not a union of device occupancy.
**The GPU runs at whatever clock the load earns: these are busy times under
that clock, not work units.** Work in WindowServer is outside this process
hook; no observed command buffers does not mean zero GPU cost for native
Core Animation. Native's frames/s remains display-link cadence, not presented
pixels. G4's whole-app and visual non-comparables still apply.

The preparation’s power preflight,
`sudo -n /usr/bin/powermetrics --samplers cpu_power,gpu_power -i 1000 -n 1`,
returned “sudo: a password is required”.

The original preparation passed all six repository checks, including clippy
and fmt, and both GPUI's and egui's clippy (`-D warnings`) and fmt. Cargo used this
worktree's `target/`; tests used `RUST_TEST_THREADS=2`. Commands, exits and
logs are in `target/g5/checks/`. The probe's offscreen check and controlled
aggregation fixture passed; G4's two published tables still reproduce exactly.
All four signed bundle copies have probe SHA-256
`9006af6aa4f59aeac785531ee8978e38b3569ce9538700e59b4fa8a650a8208f`;
`target/g5/provenance.json` records the other hashes. All 105 data assets match
the declared hashes and their copies. The plan and run script are
`target/g5/plan.txt` and `run-series.py`. A startup readiness failure, as well
as a focus loss, retains its attempt and permits one retry for that slot;
a second failure skips the slot.

CPU, frame cadence, footprint, GPU cost and power remain unmeasured by a
qualified G5 run. The offscreen probe and aggregation checks validate the
instruments; they provide no evidence for accepting or rejecting the
workload expectations.

Rebased onto `origin/main` at `f726d13124` on 2026-10-30. The RESULTS and
§4.1 conflicts retain the Windows record before G5; the Windows driver,
PresentMon summary and per-arm heights remain intact. All six repository
checks passed: **7,373 tests passed, 83 ignored**, with root clippy and fmt;
both benchmark workspaces also passed clippy (`-D warnings`) and fmt. Logs
and command exits are in `target/g5/checks-rebased/`. The rebuilt probe's
offscreen GPU accounting check and the aggregation fixture passed, and
G4's two published tables reproduce exactly. The Exact app's ignored
lockfile was refreshed offline for main's added Windows dependencies.

All four release builds passed. Each arm launched in `bar` and `caret`,
reached a complete scene at 800×800 outer points and 2× scale, and exited
cleanly. Captures show the moving fill in all four arms and the caret's two
states in native, GPUI and Ink. Egui's caret reached a complete scene in
three attempts, but each capture sequence stayed in one state: the first
launch was inactive, and the next two recorded focus loss before readiness.
The third used the driver's normal app activation once. The three-attempt
limit ends that check; **egui's blinking remains unverified**. G5m must
verify it with the foreground held before measuring it. No app code was
changed to accommodate the shared display.

These were functional launches, with no posted input or measured workload
phase. Builds, bundle hashes, startup metadata and captures are under
`target/g5/rebased/`; `functional-results.json` separates complete scenes
from verified motion. All 398 raw files under `target/g5/series` retain
their hashes. No timing or resource row is taken from these launches.

## Stage 0b: direct Metal startup (2026-10-30)

Chunk 4 fixes the retained-launch stall: a submitted but unshown window keeps
its display link running until a positive presented timestamp, suspended while
hidden. All **30/30** corrected silver Mac launches display: ten retained Metal,
ten wgpu and ten archive misses. Complete-screen p50 is **202.050 ms** on Metal
against **230.850 ms** on wgpu, meeting D15 part one's limit of at most 5 ms
slower on the same carrier and probe. The warm phases are **5.715**
and **6.069 ms**; P3 remains within 2 ms of the original 6.581 ms. Misses show
CPU at **169.629 ms** and complete Ink at **343.322 ms**, with no boot-thread
compilation. The **100 ms application budget remains unmet**. The first Metal
launch's **669.576 ms** tail and every earlier failure remain in the record.
[Cohorts, predictions and retained files](../../../host/ink/MEASUREMENTS.md#chunk-4--retained-launches-with-no-displayed-frame).

## G4m: the four arms on the measurement Mac

Measured 2026-10-30 on the dedicated silver M5 Max, source
`42d5066572c148461b4b059306be1fd2225b45f0`, fetched from `origin/main`.
All four release arms, the probe, driver and wheel helper were rebuilt in
`~/projects/exact2-measure/target/`. The four bundles pass signature verification;
all 105 data hashes and GPUI's and egui's bundled copies match. The shared
probe SHA-256 is `40d6c4aaea87df9f25386259e3f0725b18905ca456b3b3cba57cc3c9a444d825`.

The unlocked Aqua session runs the unchanged G4 driver through
`launchctl asuser`, the user switch and `caffeinate -d -i`, with no GO file.
Five interleaved fling/rest rounds and five further cold-only rounds yield
**20 fling/rest samples and 40 successful launches**. All **104/104** five-second
quiet observations pass; maximum 1/5/15-minute loads are **11.24/9.54/8.19**,
with no compiler processes. Every selected launch has nominal thermal endpoints,
Low Power Mode off, successful frontmost/key-window checks and 2× scale.
The outer window is 800×800 points; content is 800×800 for native and
800×768 for the painted arms. Caches remain retained.

The load/compiler rule called this quiet, but the retained tasks plists show
about **5.6 busy cores**, **1.77 cores in PerfPowerServices**, **2.3 cores of
DEAD_TASKS churn** and Claude bursts to **1.9 cores**. The background limits
proposed below would reject it. No process was stopped by this lane.

The first egui fling attempt received a focus-loss notification during rest.
Its recorded focus state still names egui as active, frontmost and key; no
other application is identified. It is rejected, and its one permitted retake
passes. All **41 attempts** remain retained. The initial Ink qualification's
complete-screen capture was inspected before the series.

Cells give **G4m median (minimum–maximum)**, followed by G4's corresponding
value. Cold has ten samples per arm; the other rows have five. G4's native
snapshot and painted-drawable endpoints, cadence denominators, GPUI's unchanged
re-presentations and visual non-comparables still apply. Unlike G4's last three
cold launches, all G4m launches have retained quiet observations. These are
hook-on, power-on (c) rows. The hook created the system Metal device and a
command queue before `main`, including in native, which otherwise loads neither.
The native cold row is therefore not comparable to a future hook-off (a) cold row.

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Cold to first complete frame, ms; native snapshot / painted drawable | 277.67 (257.26–664.05)<br>G4: 354.14 (330.80–862.31) | 219.07 (198.91–515.50)<br>G4: 313.13 (301.71–484.67) | 249.78 (232.86–264.97)<br>G4: 352.13 (311.68–1019.34) | 234.43 (203.64–269.47)<br>G4: 337.45 (310.81–445.11) |
| Fling process CPU, ms/s | 156.91 (151.68–175.09)<br>G4: 131.91 (112.39–143.12) | 129.79 (120.63–135.25)<br>G4: 84.92 (82.05–89.05) | 110.90 (105.46–119.73)<br>G4: 86.41 (84.98–87.07) | 108.28 (99.29–109.32)<br>G4: 66.84 (64.93–71.54) |
| Fling process CPU per presented frame, ms | —<br>G4: — | 1.457 (1.352–1.518)<br>G4: 0.961 (0.922–1.005) | 3.341 (3.206–3.629)<br>G4: 2.615 (2.591–2.696) | 2.587 (2.382–2.616)<br>G4: 1.642 (1.592–1.733) |
| Fling main-thread CPU, ms/s | 106.06 (102.76–111.39)<br>G4: 84.51 (76.30–92.87) | 65.55 (59.68–66.71)<br>G4: 39.56 (38.97–41.52) | 55.62 (52.68–60.11)<br>G4: 43.83 (41.98–44.65) | 50.36 (46.79–51.13)<br>G4: 31.33 (30.28–33.69) |
| Rest process CPU, ms/s | 2.192 (2.055–2.245)<br>G4: 1.288 (1.108–1.440) | 5.073 (4.623–5.166)<br>G4: 5.631 (4.911–5.678) | 2.379 (2.042–2.406)<br>G4: 1.469 (1.241–1.544) | 0.644 (0.457–0.712)<br>G4: 0.359 (0.307–0.494) |
| Rest main-thread CPU, ms/s | 1.091 (1.040–1.148)<br>G4: 0.636 (0.536–0.692) | 3.191 (2.851–3.241)<br>G4: 3.495 (3.134–3.598) | 2.239 (1.952–2.278)<br>G4: 1.382 (1.158–1.466) | 0.523 (0.386–0.612)<br>G4: 0.288 (0.238–0.413) |
| Peak footprint during fling, MiB | 235.74 (198.41–240.30)<br>G4: 228.39 (211.35–232.45) | 413.75 (412.16–421.85)<br>G4: 406.42 (391.64–413.84) | 598.47 (592.45–606.92)<br>G4: 591.56 (587.67–593.42) | 512.95 (505.34–518.39)<br>G4: 507.81 (504.14–515.89) |
| End-of-fling footprint, MiB | 216.47 (189.25–222.39)<br>G4: 216.61 (198.06–220.39) | 391.83 (388.33–395.63)<br>G4: 389.38 (388.77–391.05) | 245.38 (244.45–248.42)<br>G4: 246.08 (242.61–247.64) | 185.17 (180.74–287.61)<br>G4: 186.49 (179.99–188.63) |
| End-of-rest footprint, MiB | 215.86 (188.56–221.72)<br>G4: 215.85 (197.42–219.58) | 209.64 (205.63–215.52)<br>G4: 208.55 (206.05–210.47) | 238.05 (237.16–241.30)<br>G4: 238.64 (235.56–240.38) | 176.99 (172.64–279.52)<br>G4: 178.28 (171.80–180.50) |

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Fling interval p50, ms | 8.33 (8.33–8.33)<br>G4: 8.33 (8.33–8.33) | 8.33 (8.33–8.33)<br>G4: 8.33 (8.33–8.33) | 8.33 (8.33–8.33)<br>G4: 8.33 (8.33–8.33) | 8.33 (8.33–8.33)<br>G4: 8.33 (8.33–8.33) |
| Fling interval p95, ms | 8.33 (8.33–8.33)<br>G4: 8.33 (8.33–8.33) | 8.33 (8.33–8.33)<br>G4: 8.33 (8.33–8.33) | 8.33 (8.33–8.33)<br>G4: 8.33 (8.33–8.33) | 8.33 (8.33–8.33)<br>G4: 8.33 (8.33–8.33) |
| Fling interval p99, ms | 16.67 (16.67–16.67)<br>G4: 8.33 (8.33–8.33) | 16.67 (8.33–16.67)<br>G4: 16.67 (12.50–16.67) | 16.67 (16.67–16.67)<br>G4: 8.33 (8.33–16.67) | 8.33 (8.33–8.33)<br>G4: 8.33 (8.33–8.33) |
| Late intervals/s | 1.900 (1.599–2.599)<br>G4: 0.700 (0.300–0.900) | 1.072 (0.526–1.581)<br>G4: 1.248 (0.996–1.499) | 1.843 (1.339–1.878)<br>G4: 0.749 (0.500–1.999) | 0.000 (0.000–0.804)<br>G4: 0.738 (0.000–0.998) |
| Gesture-window duration, s | 3.736 (3.731–4.001)<br>G4: 4.011 (4.003–4.085) | 3.795 (3.732–3.800)<br>G4: 4.007 (4.002–4.016) | 3.733 (3.728–3.864)<br>G4: 4.006 (4.001–4.011) | 3.799 (3.731–3.805)<br>G4: 4.009 (4.000–4.067) |
| Presented frames/s inside gesture windows | —<br>G4: — | 107.49 (106.97–108.70)<br>G4: 102.82 (101.35–103.05) | 88.25 (85.93–88.67)<br>G4: 81.38 (80.57–82.99) | 105.55 (105.07–105.89)<br>G4: 101.02 (98.60–102.45) |
| Presented frames/s over the full fling phase | —<br>G4: — | 89.09 (88.98–89.19)<br>G4: 88.59 (88.38–89.00) | 32.99 (32.89–33.19)<br>G4: 32.60 (32.29–33.20) | 41.79 (41.48–41.89)<br>G4: 40.90 (40.49–41.28) |
| Inputs received / posted per sample | 8/424 (8–8/424)<br>G4: 8/424 (8–8/424) | 422/424 (420–424/424)<br>G4: 410/424 (394–415/424) | 424/424 (422–424/424)<br>G4: 417/424 (405–423/424) | 421/424 (418–422/424)<br>G4: 410/424 (403–414/424) |
| Driver maxGapMs; maximum of four gestures | 16.664 (9.341–16.885)<br>G4: 24.270 (16.214–24.997) | 9.339 (9.328–16.551)<br>G4: 17.949 (16.712–24.829) | 9.341 (9.339–16.905)<br>G4: 16.704 (16.344–25.766) | 9.349 (9.342–9.355)<br>G4: 24.631 (16.665–25.281) |
| Driver empty callbacks per sample | 0 (0–0)<br>G4: 0 (0–0) | 0 (0–0)<br>G4: 0 (0–0) | 0 (0–0)<br>G4: 0 (0–0) | 0 (0–0)<br>G4: 0 (0–0) |

**The machines do not agree within the fling CPU spread:** every G4m process
and main-thread CPU range lies above G4's. All four p95 medians agree at
8.33 ms, but native's late-interval range is also above G4's. All four peak
footprint ranges overlap G4's. Every cold median is below G4's minimum.
Source, the GPU probe and simultaneous power sampling changed between series;
these observations do not isolate a hardware difference. The complete (a) tables
in “G4m, the instrument's share” below supply the CPU baseline, under the
background stated there; these original (c) cells remain unchanged.

Predictions were retained in `plan.txt` before the run, using the same probe
and driver metrics. A footprint prediction concerns the median, not overlapping
ranges.

| Arm | Predicted fling CPU, ms/s → result | Predicted p95, ms → result | Predicted peak footprint, MiB → result |
| --- | --- | --- | --- |
| Native | 112.39–143.12 → 156.91, **miss** | 8.33 → 8.33, met | 211.35–232.45 → 235.74, **miss** |
| GPUI | 82.05–89.05 → 129.79, **miss** | 8.33 → 8.33, met | 391.64–413.84 → 413.75, met |
| Ink | 84.98–87.07 → 110.90, **miss** | 8.33 → 8.33, met | 587.67–593.42 → 598.47, **miss** |
| egui | 64.93–71.54 → 108.28, **miss** | 8.33 → 8.33, met | 504.14–515.89 → 512.95, met |

### G4m power and retained evidence

`powermetrics --samplers cpu_power,gpu_power,tasks --show-process-energy
--show-process-gpu -i 1000` retains **514** plist samples. `power-summary.py`
joins accepted phases to the exact process PID and wall clock, excludes a second
of timestamp uncertainty at each edge, and reports the median and range of
five duration-weighted run means. System watts include the rest of the machine;
process energy impact is a proxy, not watts. GPU clock values follow the
sampler's MHz scale despite its `freq_hz` field name. No power saving was predicted.

| Metric | Native | GPUI | Ink | egui |
|---|---:|---:|---:|---:|
| Fling interior: system CPU, W | 13.208 (7.980–14.395) | 12.608 (7.893–13.971) | 10.928 (5.686–13.851) | 11.828 (7.006–14.124) |
| Fling interior: system GPU, W | 0.030 (0.029–0.064) | 0.215 (0.204–0.287) | 0.239 (0.220–0.262) | 0.077 (0.068–0.087) |
| Fling interior: process energy impact/s | 534.41 (420.20–624.66) | 106.79 (93.89–130.04) | 102.05 (83.69–109.90) | 94.81 (71.72–105.71) |
| Fling interior: system GPU clock, MHz | 338.0 (338.0–343.0) | 338.0 (338.0–371.9) | 338.5 (338.1–382.6) | 338.0 (282.1–358.2) |
| Fling interior: system GPU active, % | 4.47 (4.19–5.60) | 17.07 (15.68–17.98) | 12.49 (10.66–13.70) | 9.69 (8.25–10.49) |
| Rest interior: system CPU, W | 13.435 (4.911–14.406) | 11.479 (10.100–13.676) | 12.431 (4.917–12.809) | 10.368 (5.038–12.745) |
| Rest interior: system GPU, W | 0.000 (0.000–0.003) | 0.001 (0.000–0.009) | 0.000 (0.000–0.007) | 0.001 (0.000–0.015) |
| Rest interior: process energy impact/s | 0.84 (0.74–1.08) | 2.36 (2.14–2.46) | 1.07 (0.99–1.14) | 0.15 (0.09–0.37) |
| Rest interior: system GPU clock, MHz | 57.5 (0.0–113.2) | 55.8 (0.0–224.6) | 0.0 (0.0–192.4) | 56.1 (0.0–226.6) |
| Rest interior: system GPU active, % | 0.11 (0.00–0.61) | 0.14 (0.00–1.77) | 0.00 (0.00–1.26) | 0.13 (0.00–1.50) |

The sampler emits process-path and “Second underflow occured.” warnings. The
selected intervals have valid elapsed times and exact PID joins; these remain
reported power estimates with those warnings retained. Its first SIGINT stop
failed to exit within ten seconds. The recorded sudo PID was then terminated
through the privileged Aqua-session command, and aggregation completed. No
measurement slot was rerun for that cleanup failure.

Python 3.9's monotonic clock has a process-local epoch on this Mac. The original
quiet observations remain untouched; `clock-calibration.jsonl` records explicit
`CACurrentMediaTime`/wall-clock pairs for the power join, with **1.242 ms** offset
spread. G5m's monitor reads `CACurrentMediaTime` directly. Neither correction
changes the timed driver or the arms.

Raw files, build logs, predictions, source and bundle hashes, the exact wrappers,
all attempts, quiet observations, power and captures remain on the silver Mac
under `target/g0/run-42d5066572/`, with a copy in this development worktree at
`target/g0/silver-run-42d5066572/`. Earlier locked-session attempts under
`target/g0/refresh-ae835f960c/` remain excluded and retained. `g4m/summary.json`,
`tables.md`, `power-summary.json` and `audit.json` retain the derivation and sample
accounting. Reproduce the current tables (without their historical G4 cells) with:

```sh
bun bench/heavy-list/gpui/aggregate.mjs target/g0/silver-run-42d5066572/g4m
bun bench/heavy-list/gpui/aggregate.mjs target/g0/silver-run-42d5066572/g4m --json
python3 target/g0/silver-run-42d5066572/power-summary.py target/g0/silver-run-42d5066572/g4m
```

## G5m: static workloads on the measurement Mac

Measured 2026-10-30 on G4m's four rebuilt arms and linked GPU probe, source
`42d5066572`. Three interleaved rounds of fling, bar and caret yield **36/36**
accepted samples, **114/114** quiet observations and no rejected attempts.
Maximum 1/5/15-minute loads are **10.43/7.85/7.62**, with no compiler processes.
The retained tasks plists show about **5.8 busy cores**, **1.73 cores in
PerfPowerServices**, **2.3 cores of DEAD_TASKS churn** and Claude bursts to
**2.1 cores**. “Quiet” here means only the old load/compiler rule. G5m's bar
and caret CPU rows are configuration (c), and its GPU rows are power-on. Both
stand until a hook-off static series and a power-off static series replace them.
All launches retain focus, nominal thermal endpoints, Low Power Mode off,
G4m's dimensions and 2× scale. Static phases stay at row zero with zero wheel
input. Captures precede timing in the first static round.

All four bar capture pairs show different fill lengths. GPUI's and egui's
first caret pairs show the 80-device-pixel caret on and off, with focus held.
Native's and Ink's first pairs both catch the hidden state. Separate eight-frame
capture sequences after timing show **0/80/80/0/0/80/80/0** caret pixels in
both arms, with no focus loss. Egui's blink is therefore verified, resolving
G5's earlier open check. The supplemental captures contribute no timing sample.

The GPU hook accounts for every committed buffer in its commit phase: all
counts complete, with zero invalid timestamps. Times sum command-buffer spans,
including any overlap, at the clock the workload earns. They are not a union
of GPU occupancy or a shader-only cost. Native submits no buffers through this
hook; Core Animation's work in WindowServer remains outside it. Native's 120 Hz
row is display-link cadence, not a count of displayed content changes.

Medians (minimum–maximum), three runs per arm and workload:

bar

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Quiet samples | 3 | 3 | 3 | 3 |
| Process CPU, ms/s | 92.564 (90.519–95.920) | 146.946 (144.467–155.659) | 154.178 (150.781–166.870) | 165.564 (165.204–191.106) |
| Main-thread CPU, ms/s | 84.487 (82.399–87.438) | 94.704 (93.887–101.078) | 74.406 (71.240–79.708) | 81.734 (80.918–95.319) |
| Frames/s; native display link / painted drawables | 119.93 (119.74–119.96) | 119.76 (119.47–119.79) | 119.76 (119.08–119.86) | 119.59 (119.51–119.70) |
| Peak footprint, MiB | 128.11 (126.86–130.64) | 303.19 (301.70–306.75) | 356.34 (356.25–364.13) | 443.45 (441.70–450.17) |
| End footprint, MiB | 127.78 (126.56–130.20) | 302.81 (301.70–305.44) | 355.58 (353.05–364.13) | 442.94 (440.74–449.42) |
| GPU busy time, ms/s; process command buffers | — | 32.379 (32.332–32.706) | 196.550 (163.489–213.628) | 23.251 (23.217–23.361) |
| GPU ms/frame; native cadence / painted drawables | — | 0.2706 (0.2703–0.2731) | 1.6398 (1.3651–1.7939) | 0.1944 (0.1943–0.1952) |
| Completed command buffers per sample | 0 (0–0) | 1198 (1197–1199) | 2399 (2398–2399) | 8379 (8372–8386) |

caret

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Quiet samples | 3 | 3 | 3 | 3 |
| Process CPU, ms/s | 12.176 (11.619–12.217) | 17.188 (17.126–17.598) | 154.320 (149.241–159.467) | 15.623 (15.526–16.815) |
| Main-thread CPU, ms/s | 11.146 (10.522–11.296) | 13.956 (13.794–14.195) | 73.887 (72.356–76.607) | 13.479 (13.272–14.015) |
| Frames/s; native display link / painted drawables | 119.98 (119.86–120.05) | 2.00 (2.00–2.00) | 119.74 (119.67–119.97) | 2.00 (2.00–2.00) |
| Peak footprint, MiB | 128.84 (128.70–139.22) | 292.36 (292.20–302.14) | 356.03 (355.92–362.16) | 442.20 (440.30–444.02) |
| End footprint, MiB | 128.42 (128.42–138.92) | 291.99 (291.67–301.69) | 354.50 (353.03–355.06) | 441.88 (439.99–443.66) |
| GPU busy time, ms/s; process command buffers | — | 0.567 (0.566–0.567) | 212.438 (159.497–260.022) | 0.379 (0.377–0.381) |
| GPU ms/frame; native cadence / painted drawables | — | 0.2836 (0.2829–0.2837) | 1.7741 (1.3328–2.1673) | 0.1896 (0.1885–0.1908) |
| Completed command buffers per sample | 0 (0–0) | 20 (20–20) | 2399 (2399–2400) | 140 (140–140) |

fling

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Quiet samples | 3 | 3 | 3 | 3 |
| Process CPU, ms/s | 169.594 (154.607–173.340) | 132.453 (128.650–137.303) | 123.137 (114.198–123.463) | 104.650 (92.955–110.449) |
| Main-thread CPU, ms/s | 102.070 (100.402–106.527) | 66.719 (62.813–69.686) | 57.452 (55.786–60.338) | 49.654 (43.099–52.288) |
| Frames/s; native display link / painted drawables | 118.44 (118.28–118.49) | 89.07 (88.90–89.09) | 32.90 (32.69–33.19) | 41.79 (41.69–42.00) |
| Peak footprint, MiB | 235.92 (212.17–240.02) | 413.61 (413.56–417.61) | 597.19 (594.83–604.13) | 520.17 (503.41–520.47) |
| End footprint, MiB | 201.91 (197.63–211.19) | 392.38 (391.27–396.77) | 248.09 (246.41–249.78) | 185.39 (185.13–187.81) |
| GPU busy time, ms/s; process command buffers | — | 22.727 (22.617–23.254) | 76.373 (75.484–76.403) | 8.244 (8.202–8.277) |
| GPU ms/frame; native cadence / painted drawables | — | 0.2552 (0.2539–0.2616) | 2.3090 (2.3009–2.3225) | 0.1963 (0.1963–0.1985) |
| Completed command buffers per sample | 0 (0–0) | 895 (893–895) | 1236 (1230–1238) | 3038 (3031–3052) |

The fling control retains G4m's input and pause pattern; its GPU-per-frame row
includes uploads and every observed command buffer over the full phase, divided
by displayed drawables. Its CPU ranges overlap G4m's, although Ink's median
123.14 ms/s is above G4m's 119.73 maximum. This is observed inter-series spread,
not an optimization result.

### G5's expectation, checked line by line

| Expectation stated before any run | Result |
| --- | --- |
| Native bar main-thread CPU below 10 ms/s because Core Animation runs the animation | **Miss:** 84.487 (82.399–87.438) ms/s. The series does not attribute its stacks. |
| Ink and GPUI pay the frame's bookkeeping in bar | Consistent with 119.76 presented frames/s each and main-thread CPU 74.406 / 94.704 ms/s; the amount specifically spent on bookkeeping is not isolated. |
| Egui rebuilds visible rows each bar frame | Its existing `ui` path visits visible rows per repaint; 119.59 presentations/s and 81.734 main-thread ms/s are consistent. The probe does not count row rebuilds or attribute their CPU. |
| Every caret arm is near idle between two changes per second | **Falsified for Ink:** 119.74 presentations/s and 154.320 process ms/s. GPUI and egui present exactly 20 frames per ten seconds; their CPU remains 17.188 / 15.623 ms/s, above G4m rest. Native uses 12.176 ms/s; its off-process submissions are unmeasured. |
| Continuous caret submissions would falsify the idle expectation | **Falsified:** Ink completes 2,399–2,400 buffers per sample. GPUI completes 20 and egui 140 for their 20 presentations; neither continuously presents between blinks. |

**M8 is not earned by this series.** Ink's summed GPU spans per displayed
frame are **2.3090 ms fling, 1.6398 ms bar and 1.7741 ms caret**, or **27.7%,
19.7% and 21.3%** of an 8.33 ms refresh. These exceed 15% **before M3** and
include its compose/present path. Q3 requires the GPU share **after M3**;
these rows do not satisfy that precondition or isolate the cost a plain pipeline
would remove. Keep that decision open until the post-M3 measurement. No shader
or renderer design changes here.

### G5m power

533 power samples use G4m's PID/wall-clock join and edge exclusions.
Cells are medians (minimum–maximum) of three duration-weighted run means.
The process-path and underflow warnings recur; the same power-estimate limits
apply. Ink's caret system-GPU estimate remains close to its bar's, while the
other caret arms' estimates are small. These are whole-machine watts, not
per-process power or proof of a causal saving.

| Metric | Native | GPUI | Ink | egui |
|---|---:|---:|---:|---:|
| Fling interior: system CPU, W | 10.575 (10.443–15.841) | 11.593 (9.259–12.516) | 10.234 (8.595–10.997) | 10.421 (8.791–12.971) |
| Fling interior: system GPU, W | 0.029 (0.027–0.031) | 0.205 (0.204–0.207) | 0.231 (0.226–0.232) | 0.071 (0.069–0.080) |
| Fling interior: process energy impact/s | 525.67 (417.04–628.24) | 110.40 (84.12–118.43) | 97.68 (83.68–105.81) | 97.19 (70.75–124.58) |
| Fling interior: system GPU clock, MHz | 338.0 (338.0–338.0) | 338.0 (338.0–338.0) | 344.7 (339.7–354.2) | 338.0 (280.9–338.0) |
| Fling interior: system GPU active, % | 4.33 (4.18–4.48) | 16.70 (16.47–17.16) | 12.21 (11.32–12.97) | 9.68 (9.56–10.85) |
| Bar interior: system CPU, W | 10.618 (9.649–12.160) | 10.894 (10.267–13.660) | 11.089 (10.226–13.571) | 10.370 (9.316–12.146) |
| Bar interior: system GPU, W | 0.022 (0.021–0.029) | 0.276 (0.269–0.286) | 0.724 (0.715–0.767) | 0.177 (0.168–0.180) |
| Bar interior: process energy impact/s | 57.77 (57.47–58.60) | 103.10 (100.93–113.89) | 104.21 (100.43–107.02) | 91.90 (91.44–94.76) |
| Bar interior: system GPU clock, MHz | 338.0 (338.0–338.0) | 340.1 (338.0–340.7) | 338.0 (338.0–341.3) | 338.0 (338.0–338.0) |
| Bar interior: system GPU active, % | 6.79 (6.66–8.21) | 22.10 (21.05–22.23) | 32.63 (26.39–40.19) | 22.79 (22.00–23.14) |
| Caret interior: system CPU, W | 9.151 (7.863–13.019) | 13.259 (10.384–15.424) | 8.954 (7.117–11.841) | 10.733 (10.499–11.693) |
| Caret interior: system GPU, W | 0.010 (0.003–0.011) | 0.007 (0.007–0.015) | 0.718 (0.708–0.753) | 0.010 (0.008–0.017) |
| Caret interior: process energy impact/s | 5.52 (5.46–6.20) | 10.46 (10.41–10.51) | 96.08 (90.74–100.58) | 7.20 (7.18–8.09) |
| Caret interior: system GPU clock, MHz | 338.0 (338.0–338.0) | 338.0 (338.0–338.0) | 338.0 (338.0–338.0) | 338.0 (338.0–338.0) |
| Caret interior: system GPU active, % | 1.82 (0.65–1.94) | 0.96 (0.94–2.15) | 32.11 (26.16–38.22) | 1.80 (1.43–3.01) |

Raw phases, first-round captures, power and quiet observations are in
`target/g0/run-42d5066572/g5m/` on the silver Mac and
`target/g0/silver-run-42d5066572/g5m/` here. The separate blink sequences are
`caret-capture-native/` and `caret-capture-ink/` beside them. `capture-audit.json`,
`caret-capture-audit.json`, `audit.json` and `power-summary.json` retain the
pixel counts and joins. Reproduce the three workload tables with:

```sh
bun bench/heavy-list/gpui/aggregate.mjs target/g0/silver-run-42d5066572/g5m
bun bench/heavy-list/gpui/aggregate.mjs target/g0/silver-run-42d5066572/g5m --verify
python3 target/g0/silver-run-42d5066572/power-summary.py target/g0/silver-run-42d5066572/g5m
```

### Measurement-lane validation

All six repository checks pass in the development worktree after rebasing onto
`origin/main` `8c88ae6902`: all-target build, tests, Clippy plus fmt, staged caps,
boot and the workspace all-target check. Tests use `RUST_TEST_THREADS=2`:
**7,384 passed, zero failed, 83 ignored**, across 113 result summaries.
The Ibex failure in the brief was fixed upstream by `ae835f960c`; it does not
reproduce, and this lane makes no fix. Complete commands, exits and verbatim
output are in `target/g0/checks-final-8c88ae6902/`. The earlier full run
on the measured source base `42d5066572` also passes, with 7,380 tests passed
and 83 ignored, in `target/g0/checks-run-42d5066572/`.

Both G4m tables and all three G5m tables reproduce exactly from their raw files;
G5's committed `--verify` passes with zero exclusions. Both boot summaries also
reproduce exactly. Power rows reproduce from the retained plists and clock join.
No renderer, workload, committed probe or driver source changes in this lane.

## Stage 0: the numbers first

@ref LLP 1149.001 §2–4; `lane/1149-001-s0`, based on `b059d3d8a8`.
This stage changes instrumentation only. Retained evidence is under `target/s0/`;
S1.4's original files stay in place. Agent-driven runs require no focus. A posted
Mac fling waits for `target/s0/GO` newer than 2026-10-30T15:10:26Z.

Historical tables before the review-fix continuation retain their original
schema: zero-cause service turns could include a redraw's trailing pass,
and ownership gauges described the last target take. Those dispatch labels
are superseded by the corrected traces below; M1 uses the integrated rerun.
Current ownership gauges refresh at every pool mutation, while completion
partitions explicitly name their last observation's timestamp and serial.

### Predictions recorded before measurement

1. `exclusive.mjs`, on S1.4's image-disabled control and its full fling
   window: 3,579 service passes, 358 walks and 348 draws; exclusive elapsed
   rates round to 12.6, 5.7, 5.9 (draw plus encode), 1.2 (glyph), 1.0
   (refinement), 0.4 (layout) and 1.3 ms/s (wheel), as RFC §2 states.
   The script separates draw from encode and reports CPU from the probe's
   endpoints, without treating elapsed spans as CPU samples. Historical
   service causes are expected to be unavailable, not inferred from proximity.
2. On the same traces, display callbacks fall about 5 ms after the presented
   refresh grid; `exclusive.mjs` reports the distribution modulo the refresh
   period. GPU command and present intervals remain separate from CPU spans.
3. A fresh release carrier, traced through the phased live-agent wheel, is
   expected to retain fractional-shift refusals. No CPU or cadence equality
   with posted input is predicted: socket requests add service work and do
   not traverse AppKit's wheel delivery. The first run establishes its baseline.

### Retained S1.4 attribution

The exact ten-second control window reproduces every published count. The
probe's complete fling window is 10.003495 seconds and includes two additional
service, refinement and layout spans. The prediction of 3,579 passes over
that full window misses: it has 3,581. `--seconds=10` makes the hand table's
denominator explicit. All rates below are exclusive **elapsed** ms/s; counts
are over ten seconds. GPU timestamps are excluded from CPU-thread nesting,
even when their completion was observed on that thread.

| Phase | RFC hand rate | Script rate | Count |
|---|---:|---:|---:|
| carrier service | 12.6 | 12.717 | 3579 |
| paint walk | 5.7 | 5.598 | 358 |
| carrier draw | 5.9 together | 3.005 | 348 |
| scene encode | — | 2.925 | 348 |
| glyph raster | 1.2 | 1.300 | 481 |
| exact refine collections | 1.0 | 1.041 | 4775 |
| exact layout | 0.4 | 0.511 | 3362 |
| wheel input | 1.3 | 1.339 | 422 |

The draw/encode sum is 5.931 ms/s. The service, walk, glyph and layout rates
do not round to the hand table; those predictions miss. The script's precise
boundary and innermost-span rule replace the unpublished derivation. The
full-window control measures **85.573 process / 51.540 main CPU ms/s**; its
marked main-thread elapsed total is **29.034 ms/s**, leaving a cross-instrument
difference of **22.506**, not the RFC's ~15. The latter subtracted a G4 CPU
median from S1.4 elapsed spans. Neither difference identifies a CPU owner.

Normal images, over 10.002654 seconds: **96.753 process / 49.454 main CPU
ms/s**. Main-thread elapsed: service 13.339 (3,618), walk 5.467 (391),
draw 3.092 (356), finish 3.038 (384). This older trace has no scene-encode
or wheel-input marker, so its finish row cannot be split after the fact.
Its two decode threads total **14.258 elapsed ms/s**. Worker completion
waits are **45.112 / 59.781 elapsed ms/s** (control / normal), not CPU.

The present GPU interval averages **1.011 / 1.461 ms**. The generic GPU-command
stream also contains uploads and offscreen passes; its **0.536 / 0.742 ms**
mean is not a scene-only mean. The phase prediction is met: callback positions
have medians **5.072 / 5.098 ms** into the 8.333 ms presented-time grid.
All **3,579 / 3,618** service causes are unmarked in the old traces. None is
assigned a cause from the nearest event.

Both runs remain focus-rejected diagnostics; neither is a qualified cadence
series. Trace hashes match S1.5. Full per-thread tables, GPU distributions,
CPU endpoints and hashes are retained in `target/s0/{control,before}.{md,json}`;
the exact ten-second control is `control-ten.{md,json}`. Reproduce with:

```sh
bun bench/heavy-list/gpui/exclusive.mjs --verify
bun bench/heavy-list/gpui/exclusive.mjs target/s1-4/g3-control/ink-1.json.trace.json --seconds=10
bun bench/heavy-list/gpui/exclusive.mjs target/s1-4/g3-before/ink-1.json.trace.json
```

### Live-carrier predictions and instruments

The production bake refuses the live agent, as its trust boundary requires.
The agent measurements therefore use the optimized `host-dev` carrier. They
are diagnostics, not replacements for G4's release scoreboard. The first
agent drive completed 1,200 requests, but its full 20 MB `state` response
extended the fling endpoint to 41.510 seconds; that run is retained as
`target/s0/baseline-agent/` and excluded from ten-second comparisons. The
next launch timed out waiting for a complete frame. Both failures remain.

Before the counter runs, the predictions are:

- The phased 1,200-request wheel produces at least one fractional-shift
  refusal; at least half of these refusals meet A2's proposed eligibility.
  The live `state` counter delta is the instrument. A2 is observed only;
  the current reuse decision is unchanged.
- The viewport pool retains at least 16 targets, with at least 100 MiB idle
  at its byte high-water mark. A timestamped counter sample and `vmmap`
  distinguish logical texture storage from process physical footprint.
  Incomplete candidates and allocations blocked by them are counted apart.
- Dispatcher-only service passes remain nonzero after separating wheel,
  link, worker, timer, agent and draw causes. Combined causes remain combined
  in the trace; individual cause counters overlap and are not a partition.
- One enabled cumulative-counter increment costs less than 1 µs in the
  optimized `exact-ink` measurement test. Enabling all observations adds
  less than 5% to process CPU in paired unprofiled agent runs. The latter
  includes eligibility checks, pool scans, probe reads and compact state
  requests; it is not the increment's cost alone.
- The NucBox direct-D3D11 pool remains below G6b's pathological 1,201
  retained targets, and fractional-shift refusals are nonzero on the
  unchanged four-gesture Windows driver. No Mac/Windows CPU equality is
  predicted.

`target/s0/posted-profile.py` prepares the existing posted-fling driver and
`sample` capture. It refuses to run without a fresh GO. The 30-second GO
poll is retained in `target/s0/go-poll.jsonl`; it expires three hours after
the lane start. The live-agent profile needs neither focus nor posted input.

### Fresh agent trace and first counter observations

`target/s0/fresh-cpu-only/` is the first complete ten-second agent drive.
After three displayed-readiness timeouts, the probe can explicitly wait for
CPU scene completion (`G1_AGENT_CPU_ONLY=1`). This mode leaves displayed
startup unknown; it does not manufacture a presented timestamp. The carrier's
visibility and presentation policies are unchanged. One timeout retained a
complete scene, three submissions and no displayed callbacks, with AppKit
reporting the window visible and unoccluded (`counters-agent-2/ink.json.timeout`).

The fresh trace contains 1,200 phased requests over 10.068653 seconds; its
probe window is 10.145215 seconds. Process/main CPU is **882.383/549.722
ms/s**. Its continuous socket input repeats hit-testing paints and oversized
clip-mask work, as S1.4 observed. It is not the posted G4 trajectory and
cannot supply a replacement for G4's 44 ms/s main-thread budget.

| Fresh trace phase | Exclusive elapsed ms/s | Count |
|---|---:|---:|
| paint walk | 389.395 | 1284 |
| main resource wait | 235.672 | 4984 |
| path raster | 41.576 | 3871 |
| scene encode | 31.830 | 6107 |
| texture allocate | 20.514 | 3873 |
| carrier draw | 16.462 | 965 |
| carrier service | 15.453 | 2749 |
| agent request | 11.575 | 1200 |
| glyph raster | 2.328 | 724 |
| layout | 2.204 | 2985 |
| refinement | 1.230 | 3714 |

The GPU present interval averages 0.722 ms. The callback phase has p50
5.092 ms and p99 5.190 ms against the presented grid. Full thread tables
and hashes are `target/s0/fresh-cpu-only.{md,json}`; the reducer regenerates
them from `fresh-cpu-only/ink.json.trace.json` and its sibling endpoints.
Elapsed waits exceed CPU time here; subtracting marked elapsed from CPU
would give a negative number, not an unattributed CPU owner.

The subsequent `marked-on/` run adds the dispatch marks and counters.
Its 10.061795-second probe window contains:

| Observation | Phase delta |
|---|---:|
| paint attempts / submitted / displayed | 1145 / 1144 / 0 |
| whole-row recordings / replay decisions | 13833 / 11048 |
| fractional-shift refusals | 13781 |
| A2 eligible / glyph-zero fallback / scaled fallback | 13454 / 327 / 0 |
| new / stale / suspect / picture-invalid recordings | 51 / 77 / 1 / 25 |
| image-slot patches | 126 |
| viewport allocations / retirements | 23 / 18 |
| allocations blocked by incomplete matching targets | 4 |
| incomplete matching candidates inspected | 2603 |
| clip-mask targets built | 9076 |
| shaping-cache hits / misses | 58 / 135 |
| paragraph-measure cache hits / misses | 489 / 193 |

The fractional-refusal and ≥50% A2-eligibility predictions are met (97.63%
eligible). These classify the proposed policy; no row becomes reusable.
A fractional refusal can also have a stale, suspect or picture cause; those
counts overlap. A replay decision can be culled before drawing. Paint attempts
count completed painter snapshots, including refused presents.

The pool's process high water is **12 targets / 112.5 MiB**. At the process
footprint peak of **869.314 MiB**, the probe records **8 targets / 75 MiB**:
six reserved for the current frame, two idle (18.75 MiB), none held by
incomplete work. The ≥16-target prediction misses. The ≥100 MiB idle-at-pool-high-water
prediction is unresolved: the largest sampled idle value while all twelve
targets are retained is 75 MiB; the separate idle high-water mark is
103.125 MiB, without a retained coincident timestamp. The 18.75 MiB value
at the footprint peak answers a different question. The pool cannot account
for this footprint; these logical bytes must
not be equated with physical footprint. The separate profiled run retains
`marked-profile/vmmap.txt`: its 727.3 MiB snapshot precedes that run's
861.783 MiB probe peak, so it is not a map taken at the exact peak.

The first period gauge reset only on delivered link ticks, accumulating
passes while that link was stopped. That gauge is superseded: the counter
now advances nominal monitor periods on service turns, including at rest.
`exclusive.mjs` also bins passes on complete refresh intervals, anchored to
presented times when available and explicitly labelled nominal otherwise.
The other counters above are unaffected by that gauge correction.

### Observer cost and trace boundaries

The optimized increment test reports 1,224,083 ns disabled and 1,265,208 ns
enabled for 1,000,000 increments: **1.224/1.265 ns per loop iteration**. The
<1 µs prediction is met for this tight-loop instrument, not for the whole
observer (`counter-overhead-off.log`, `counter-overhead-on.log`).

The first paired live runs do **not** establish the <5% observer bound.
`marked-off/` has no fling submissions or display-link callbacks; `marked-on/`
has 1,144 submissions. Both fail the quiet rule: maximum load exceeds 43
and compiler processes are present. Their CPU values are retained, not
interpreted as an observer-cost difference. `marked-profile/` adds `sample`
and `vmmap`; its requests take 15.273221 seconds, so it is also excluded
from a ten-second performance comparison.

The original traced rest window included serialization of its fling and
end-of-rest traces. `G1_REST_TRACE=1` defers export until after the rest CPU
endpoint. With this boundary, the marked counter run measures **3.499 ms/s
process CPU** at rest, versus 84.609 ms/s in the earlier export-containing
window. These are different observations, not a renderer optimization.

Enabled observation retains glyph metadata on each kept row (two `f32`
coordinates, eight bytes per glyph, plus vector capacity). Windows allocates
one JSON event per recorded span and JSON counter samples, growing bounded
vectors until export, even when no output path is set. These allocations,
pool scans, clock reads and eligibility classification contribute to the
observed CPU and footprint. The increment microbenchmark measures none of
that work. Monitor queries now occur at attachment, movement and scale
changes, rather than on every wake.

### Observer-wakeup and GPU experiments: predictions

Before the next runs, removing only the probe's display callbacks is
predicted to reduce dispatcher-only service passes by at least half. Removing
its one-second memory timer next should remove ten probe-timer marks from
ten seconds of rest. The app's `producer` and `clock` tasks remain due at
250 and 1,000 ms even with `BENCH_LIVE=0`; the initial agent state records
both deadlines. Their work is not classified as redundant probe activity.

For the GPU experiment, the same carrier uses integral one-point wheel
steps, which avoid the continuous fractional clip-mask workload. Each arm
retains its own trace and binary/probe hashes. Replacing the final texture
sample with a clear is predicted to bring the mean GPU present interval
below 0.35 ms. Keeping a second Metal queue busy is predicted to bring the
sampled pass below 0.5 ms if low clock state explains the old ~1 ms. These
are hypotheses, not assumed causes. The existing two timestamp read sites
are compared too: the worker reads after completion; the main thread reads
again when it retires that command.

The first four GPU arms retain `gpu-{sample,clear,busy,busy-clear}-2/`.
Their mean present intervals are 0.413, 0.030, 0.874 and 0.030 ms. The clear
prediction is met; the busy-queue prediction misses. The second queue covers
45–46% of wall time, so it does not establish a continuously busy GPU. A
follow-up keeps four longer commands queued. Before that run: its interval
coverage should exceed 90%; if clock state alone explains the sampled pass,
that pass should still fall below 0.5 ms. A separate 30 Hz socket arm changes
only request frequency from 120 Hz; a higher mean than 0.413 ms is predicted
if the longer idle gap matters. These remain diagnostic experiments under
machine load, not scorecard replacements.

The longer queued workload covers 89.24% of the sampled arm and 94.89% of
its clear arm: the >90% coverage prediction misses for the former and is met
for the latter. Present means are 0.459 and 0.043 ms, meeting the <0.5 ms
prediction but not improving on the 120 Hz control's 0.413 ms. At 30 Hz,
with the second queue absent, the sampled pass rises to **1.078 ms**; the
frequency prediction is met. Before pairing that arm: clear should remain
below 0.35 ms and the continuously queued sample below 0.5 ms, on the same
30 Hz requests. This separates the old interval's order of magnitude from
the heavier 120 Hz socket workload.

### Windows live counters and observer cost

The NucBox mirror uses the direct-D3D11 G6e arm, its unchanged four-gesture
`windows-measure.py` driver, PresentMon, and the matching executable/PDB.
The retained wrapper is `target/s0/windows-run.py`; raw files are under
`target/s0/windows/{off,on,sampled,off2,on2,off3,on3}/` on both machines.
`normalize-windows.py` joins QPC endpoints, Unix-time counter samples and
PresentMon's displayed times, then invokes `exclusive.mjs`. It records each
counter endpoint's lag; counter and CPU denominators are not silently equated.
The initial on-run counter window is 10.089016 seconds; its CPU window is
10.096334 seconds, with 37.357/44.674 ms counter lags at the endpoints.

| Windows observation | Counter-window delta |
|---|---:|
| Paint attempts / submitted frames | 245 / 245 |
| Whole-row recordings / replay decisions | 1841 / 1038 |
| Fractional-shift refusals | 1832 |
| A2 eligible / zero-interval fallback / scaled fallback | 1796 / 36 / 0 |
| New / stale / suspect / picture-invalid recordings | 9 / 9 / 0 / 7 |
| Image-slot patches | 21 |
| Pool allocations / retirements | 56 / 17 |
| Allocations blocked / incomplete candidates inspected | 56 / 25338 |
| Clip-mask targets built | 3530 |
| Shaping-cache hits / misses | 8 / 7 |
| Paragraph-measure cache hits / misses | 122 / 15 |

PresentMon records 245 displayed fling frames and none at rest. Windows
has no equivalent presented callback in the live observer: `displayedFrames`
is null there; the normalized probe explicitly supplies the ETW count.
It never substitutes successful `Present` calls for displayed frames.

The process high water is **80 viewport targets / 182.075 MiB**; at the
sample retaining all eighty, fourteen are reserved and sixty-six are idle
(150.212 MiB), with none held by incomplete work. Elsewhere as many as
72 are incomplete; 83 allocations over the process lifetime were blocked
by incomplete matching targets. Both states occur. The working-set peak
is 156.371 MiB, less than the logical pool bytes: those storage counts are
not a physical-memory attribution. The <1,201-target and nonzero-fractional
predictions are met. A2 eligibility is 98.03% on these observed refusals.

| Observer pair | Disabled process CPU ms/s | Enabled process CPU ms/s |
|---|---:|---:|
| 1 | 82.044 | 94.403 |
| 2 | 111.789 | 104.046 |
| 3 | 86.955 | 96.313 |
| Median | 86.955 | 96.313 |

The median difference is **+10.76%**, missing the <5% prediction. Each run
passed the driver's quiet check before launch, used the same trajectory and
retained 245–248 displayed frames. Variation across three pairs prevents
an exact causal overhead estimate; it does not establish the desired bound.
These Windows observations include trace recording and gauge serialization.
Enabled observation also retains an eight-byte coordinate pair per captured glyph
on each kept row, grows the capture vectors, scans pool entries, and allocates
Windows span JSON and sample maps. These costs are included in process memory
and CPU; the increment microbenchmark does not measure them. Monitor queries
now run at attach, move and scale change, rather than every wake.
The Mac <5% agent comparison remains invalid because its disabled arm did
not submit fling frames and the machine was busy.

### Dispatch, service causes and rest

`final-control/` uses the corrected nominal-period counter. Over 10.059203
seconds it records 4,975 passes, including 1,170 draw passes. The other
combinations are dispatch 1,388, agent 1,112, link 1,040, link+agent 121,
link+worker+agent 23, worker 60, worker+agent 42, timer 16 and timer+agent 3.
Complete refresh periods average 4.120 passes, p95 6, maximum 10.
The agent's wheel is delivered through the socket, so it has an agent cause;
it does not manufacture a native wheel-event cause.

The new marks expose main-thread exclusive elapsed costs of 31.959 ms/s
in probe display ticks, 4.444 in probe usage reads, 5.180 in run-loop
dispatch, 0.992 in winit dispatch and 0.149 in winit window events.
The display-link thread has its own wake span. AppKit event delivery is
marked too, but this drive bypasses AppKit wheel delivery. These elapsed
values include scheduling delays on a busy machine. They cannot be used
as a CPU subtraction or applied retrospectively to S1.4's old trace.

| Mac observer arm | Fling dispatcher-only passes | Rest dispatcher-only / timer passes | Rest process CPU ms/s |
|---|---:|---:|---:|
| Full probe | 1388 | 65 / 40 | 2.182 |
| Probe display callbacks disabled | 808 | 63 / 40 | 1.975 |
| Probe display callbacks and memory timer disabled | 701 | 52 / 40 | 1.397 |

Disabling display callbacks removes all their marks and reduces
fling dispatcher-only passes by **41.79%**, missing the predicted ≥50%.
Removing the memory timer removes all ten rest probe-timer marks, meeting
that prediction. Each rest trace is 10.054 seconds; neither arm removes
an app timer. The latter trace still has 52 event-source wakes with no
carrier cause and 40 deadline wakes. Thus it names and counts the remaining
unknown source category; it does **not** claim to identify every AppKit
or Core Foundation source behind it. `quiet.jsonl` rejects these as quiet
performance runs: maximum one-minute loads are 121.30, 95.10 and 72.79,
with compilers present throughout. Counts are diagnostic observations;
the CPU differences are not accepted savings.

Windows' on-run records 609 passes over 10.096334 seconds: draw 245,
wheel 236, timer 50, dispatch 53, worker 10, worker+timer 10,
wheel+worker 4 and wheel+timer 1. The 605 complete refresh periods average
1.007 passes, p95 3, maximum 4. In 10.144496 seconds at rest, exactly fifty
run-loop wakes produce fifty timer passes and fifty dispatcher passes,
with no submitted or displayed frame. The 250 ms producer and 1,000 ms
clock tasks explain the 40+10 scheduled deliveries; the counter preserves
the second service pass rather than calling it another timer. This is the
live window, not the headless `perf` presenter.

The historical 3,579 passes had no cause tags. They cannot now be partitioned
into 2,250 redundant passes by inference. The observed extra passes include
probe callbacks, authored timers, image workers and dispatch-only turns.
Stage 1 must retain the timer and worker work and measure the remainder
with posted input before spending an 11 ms/s CPU budget.

### Per-thread CPU profiles and memory

`final-profile/` retains the ten-second 5 ms `sample` profile, the symbolized
copy, Mach thread CPU endpoints, trace and `vmmap`. Its probe window is
10.053222 seconds; process/main CPU is 588.038/365.217 ms/s. The endpoints
name the present worker (41.525), image workers (17.492 and 13.769), agent
socket (4.570) and CVDisplayLink (2.404). Unnamed dispatch workers total
137.631 ms/s; endpoint thread sums leave 5.426 ms/s in exited threads or
endpoint skew. The sample names Metal command/completion queues, ImageIO
JPEG decoding, drawable acquisition and completion waits. Many dispatch
worker stacks terminate at `start_wqthread` or `__workq_kernreturn`; their
CPU cannot all be assigned to the Metal driver from this sample. Blocked
stack samples are not CPU shares. This agent profile does not apportion
the old posted G4 run's 42 ms/s off-main-thread budget.

The footprint peak in that run is 967.611 MiB at +5.639 seconds. The
`vmmap` launched at +5.8 seconds sees 812.6 MiB, with a lifetime peak of
968.9 MiB: it is close in time but **not a map at the exact peak**. It
reports 272.1 MiB resident IOAccelerator graphics, 28.5 MiB IOSurface and
141.7 MiB resident malloc zones. The pool at the probe peak is 75 MiB,
including 18.75 MiB idle. The map and counters establish that the pool is
not the whole transient; they do not identify all of the difference.

Windows' `sampled/` retains xperf's ETL, module reports, per-thread stack
reports and matching PDB. Over 10.107471 seconds, process/main CPU is
102.028/66.473 ms/s. Thread CPU endpoints and stack reports identify:

| Windows thread | CPU ms/s | Sampled owner |
|---|---:|---|
| 40296 | 66.473 | main carrier |
| 165220 | 13.913 | AMD user/kernel driver |
| 443036 | 10.821 | AMD user/kernel driver |
| 122656 | 6.184 | image worker, WindowsCodecs |
| 532276 | 3.092 | image worker, WindowsCodecs |
| 127956 | 1.546 | insufficient samples (two hits) |

The 876 process samples include 668 main-thread samples. Exclusive module
hits include Heavy list 357, WindowsCodecs 79, AMD user driver 76 and AMD
kernel driver 19. These are sample counts, not elapsed spans. xperf's merge
extended this run's rest phase to 29.295 seconds, so the separate unprofiled
on-run supplies the ten-second rest result.

### The GPU present interval and callback phase

All these arms use the same retained `Final.app`, probe and integral one-point
phased socket wheel. `gpu-sample-2/` is 120 Hz; `gpu-sample-30hz/` changes only
the request rate. The clear hook is available only through the linked probe;
normal carriers still encode the same sampled quad. The second queue is
`target/s0/metal-busy-continuous.m`, with four queued compute commands and
retained GPU intervals. It posts no events and acquires no window.

| Request rate / present arm | Present intervals | Mean GPU ms | p99 GPU ms | Second-queue interval coverage |
|---|---:|---:|---:|---:|
| 120 Hz sample | 1185 | 0.413 | 1.401 | — |
| 120 Hz clear | 1175 | 0.030 | 0.124 | — |
| 120 Hz continuously queued sample | 1127 | 0.459 | 1.123 | 89.24% |
| 120 Hz continuously queued clear | 1142 | 0.043 | 0.487 | 94.89% |
| 30 Hz sample | 592 | 1.078 | 1.879 | — |
| 30 Hz clear | 132 | 0.058 | 0.926 | — |
| 30 Hz continuously queued sample | 302 | 0.654 | 1.496 | 99.62% |

The 30 Hz clear prediction is met; the <0.5 ms busy-sample prediction misses.
The old ~1 ms scale is reproducible, and removing the sampled draw removes
most of it. Added GPU work changes it, but does not establish clock state
as its sole cause. Submission counts and visibility vary, especially at
30 Hz, and compiler load disqualifies a scorecard comparison. The supported
cause is the work/dependencies of the draw that samples the compose target,
with load-sensitive GPU execution; this experiment does not separate shader
execution, resource dependencies and hardware clock changes inside it.
No CPU drawable-acquisition or completion wait is included in these GPU
timestamps. The clear keeps command submission, drawable presentation and
completion handling.

The worker reads `GPUStartTime`/`GPUEndTime` after command completion; the
main thread reads them again during retirement. Every pair retained at both
sites is identical, including all 592, 132 and 302 intervals in the 30 Hz
arms. In some 120 Hz arms one final command lacks a later retirement mark;
that is a missing second read, not a changed timestamp. Read timing does not
explain the old interval. `exclusive.mjs` prints both counts.

The historical callback phase p50 is 5.072/5.098 ms (control/normal) within
an 8.333 ms refresh period. The fresh 120 Hz sample and clear arms give
5.108/5.107 ms; this meets the approximately 5 ms prediction. An occluded
arm with only a few displayed times cannot supply a current refresh grid;
its callback-phase result is not used. The Windows carrier has no display
link; the reducer reports that absence instead of supplying a nominal
callback time.

The `sample` capture leaves unnamed dispatch CPU. Before a Time Profiler
follow-up on the agent drive: at least 90% of its weighted CPU samples are
predicted to have a resolved stack naming an owner (carrier, probe, image,
present worker or platform). That profile also covers the same ten-second
rest interval. It remains a diagnostic under machine load; it cannot
reconstruct G4's old thread distribution.

For the third lead, Windows' 3,530 clip-mask targets across 245 submitted
frames are 14.41 per frame. The sampled run has 54 inclusive path-coverage
samples among 668 main-thread samples (8.08%). Clip work remains, but the
G6b repaint churn is absent: this is 249 process-lifetime submissions,
not thousands. The new counters distinguish whole-row recording from
image-slot patching and make the remaining clip work measurable before a
cache change.

The final live schema also exposes `period_{wheel,link,worker,timer,agent,
window,dispatch,draw}` gauges and their process high waters. Periods are
nominal monitor intervals, advancing even while the link is stopped;
`exclusive.mjs` independently bins trace passes on the observed refresh grid
and prints combined-cause distributions. `period-counters/` reads the
schema through both live state and the probe: 6,659 passes, 1,198 paints,
1,196 submissions and 13,781 fractional refusals over 10.055500 seconds.
Its complete-period maximum is 11. Cause high waters are wheel 0, link 2,
worker 5, timer 1, agent 3, window 1, dispatch 8 and draw 2; they need not
occur in the same period. Pool completion partitions describe the last
`take` observation using its existing completion value, not an extra GPU
poll made by a state read. This run is loaded and supplies schema validation,
not a newer CPU score.

`cpu-profiler/` adds Time Profiler, attached before the agent phase and
aligned by the retained wall/monotonic clock pair. The XML export and
`time-profile.py` resolve IDs, frame references and Rust symbols, then cut
samples at the probe endpoints. All 6,338 fling samples are `Running`, with
1 ms weights: 629.465 sampled ms/s over 10.068870 seconds, versus 627.858
process CPU ms/s from the probe. The main thread has 3,908 samples
(388.127 ms/s), versus its endpoint 390.121 ms/s. Fifty-four samples have
an unresolved leaf: **99.15% are symbolized**. Excluding those 54 samples
and all 535 bare `start_wqthread` leaves leaves **90.71%**, meeting the
≥90% broad-owner prediction (`resolution-bound.json`). A resolved platform
entry symbol is not a recovered caller; the excluded entries remain unassigned
to a component.

Off the main thread, stack ownership identifies Metal driver work
(76.076 sampled ms/s), the present worker (34.661), image work (30.689),
the socket agent (3.476) and display link (1.688). Platform dispatch work
accounts for 87.895; 535 process samples end at `start_wqthread` without a
component caller. The named Metal stacks include `iokit_user_client_trap`
and command/completion handling. These results explain much of the new
agent run's off-main cost; they still do not assign G4's old 42 ms/s.

On the main thread, stack classification places 16.983 sampled ms/s in the
probe, 2.582 in winit dispatch and 11.024 in other AppKit/Core Foundation
or platform work. Five counter-observer samples represent 0.497 ms/s;
this excludes indirect allocation and eligibility costs. The remaining
main samples have paint, Ink, device, service, encoding, glyph, path or
layout owners. This is a CPU-sampled attribution of the **new run**, separate
from the elapsed-phase reducer. `main-attribution.json` retains the exact
classification and leaf examples; no old ~15 ms/s remainder is manufactured
by subtracting elapsed spans from a different run's CPU.

Time Profiler catches only eleven running samples in 10.046629 seconds at
rest: three probe, three presenter service, one winit, two other main-thread
platform, one Metal and one unresolved. The probe's rest CPU is 1.610 ms/s.
That sample count names observed work but is too small for a precise CPU
partition or for identifying every event-source wake. Raw rest wake marks
remain the count instrument.

The rest trace gives a narrower lead: forty of the fifty-two dispatcher-only
passes in `no-probe-timers/` occur within 1 ms after a timer pass. Before
tracing the requests themselves, forty app timer rearms are predicted, each
followed by that second dispatch. Opt-in probe interposition records
`CFRunLoopTimerSetNextFireDate` and `CFRunLoopWakeUp` while forwarding the
original calls and deadlines unchanged. This tests whether timer maintenance
explains those forty turns; the remaining twelve still require evidence.

Windows' final schema run, `windows/period/`, retains per-period high waters
of wheel 2, worker 3, timer 3, window 2, dispatch 2 and draw 2; agent and
link are zero in this carrier. Its counter window is 9.834747 seconds
(the end gauge precedes the CPU endpoint by 235.428 ms), so the 1,761
fractional refusals and 1,728 A2-eligible refusals are not presented as an
exact ten-second delta. PresentMon counts 248 displayed fling frames and
none at rest. A final disabled/enabled pair on this schema measures
93.172/94.800 process CPU ms/s (**+1.75%**), numerically meeting the <5%
prediction for that pair. It does not erase the earlier three-pair miss
or establish a tight overhead bound across runs.

The wake-request experiment meets the forty-rearm prediction. In
`wake-callers/`, 10.069326 seconds at rest contain forty deadline wakes,
forty timer arms from `winit::ApplicationDelegate::cleared`, and forty
additional dispatcher turns within 1 ms after those arms. Every arm sets
the next deadline about 249 ms ahead. The remaining four dispatcher turns
are two immediately after the probe changes phase and two following AppKit
wake requests. The latter callers are
`___NSCreateMainQueueHysteresisBlock_block_invoke` and
`-[NSApplication(NSQuietSafeQuit) _updateCanQuitQuietlyAndSafely]`.
These are observed caller symbols, not proposed dependencies on private APIs.
The trace has no dropped events. `exclusive.mjs --rest` prints the callers;
`wake-summary.py` retains the temporal join. The earlier fifty-two unknown
turns are not retroactively assigned this run's counts.

`wake-sources/` failed trace export because winit's extreme stopped-timer
date overflowed a metadata conversion to milliseconds. Its log is retained.
The observer now bounds only the recorded delay; it passes the original
absolute deadline unchanged. `wake-sources-2/` and `wake-callers/` both
complete, preserve all 1,200 requests and export without dropped events.

The final probe also marks its phase-transition callback. `rest-final/`
records 40 timer wakes and 45 dispatcher turns over 10.049548 seconds:
40 follow winit timer arms, two follow the marked probe transition, and
three follow AppKit wake requests. The additional observed AppKit caller
is `-[NSApplication _copyPublicPersistentUIInfo]`. One final GPU completion
crosses into this rest window; the earlier `wake-callers/` trace supplies
the zero-GPU rest observation. The measured 2.063 process / 1.691 main CPU
ms/s remains a loaded diagnostic, not M2 qualification.

### Stage 0 verdicts

**M1 — service budget.** The published control has 3,579 passes in exactly
ten seconds, costing 12.717 **elapsed** ms/s; it has no cause tags. In the
latest corrected agent trace (`review-fixes/dispatch-rebased/`), **1,838
dispatch-only passes over 10.056452 seconds cost 6.328 exclusive elapsed
ms/s**. Its 5,193 total passes include 1,016 draw-owned passes and 1,016
trailing carrier passes tagged `DRAW`, including combined causes. The
pre-rebase diagnostic has 1,184 dispatch-only passes; the integrated run
misses the predicted upper bound of 1,388. The old 2,902
count included redraw turns and is superseded. These are not the old
2,250 passes and not a CPU saving. The probe itself creates
turns; timer maintenance creates a second turn after due work; worker and
agent deliveries carry useful work. M1's **44 → ~33 main-thread CPU ms/s**
remains a conditional prediction for the quiet posted instrument. Stage 0
does not establish its 11 ms/s saving or authorize dropping timer work.

**M4a — pool policy.** Idle retained targets are real on both hosts. The
Windows high-water sample has 66 idle targets, while other samples find
72 incomplete; Mac's footprint peak has two idle and six frame-reserved
targets. That meets the policy's idle-target precondition. It does not
justify treating all retained targets as disposable or claiming the pool
explains the transient: logical pool bytes and physical memory differ,
and the Mac map was taken near, rather than exactly at, its peak. No pool
retirement or reuse policy changes in this lane.

**M6 — fractional row recordings.** The Mac agent run refuses 13,781 kept
rows for fractional shifts and records 13,833 rows whole; Windows refuses
1,832 and records 1,841 over its sampled counter window. Fractional
re-recording is therefore real and frequent in these trajectories, with
97.63%/98.03% classified A2-eligible. Stale, suspect and picture-invalid
records are reported separately and may overlap a fractional refusal.
Eligibility counters predict opportunity; they do not prove the proposed
pixel-placement policy or change today's `Ink::reusable` result.

**M7 — draw and encode.** The retained ten-second control separates 348
carrier draws at **3.005 elapsed ms/s** from 348 scene encodes at **2.925**.
The hand table's combined 5.9 is supported at that precision. The new
socket trajectory has many more walks, clip targets and submissions, so
its larger draw/encode rows cannot replace that budget. Windows' 54
inclusive path-coverage CPU samples and 3,530 clip targets establish that
clip work remains after G6e's repaint fix; neither number is a prediction
of what an instance-patch implementation will save.

**Unattributed work and M2.** The old ~15 ms/s was a cross-run CPU-minus-
elapsed difference, not an unnamed CPU measurement. New dispatch/probe
marks and CPU profiles provide actual owners, including the present,
decode and driver threads; 54 of 6,338 Time Profiler fling samples have
unresolved leaves, and dispatch-entry stacks still lack component callers.
Rest traces expose due app timers, their following maintenance turns,
probe activity and AppKit bookkeeping. M2's **≤0.4 process CPU ms/s**
remains a future quiet-run target after removing avoidable wakes while
preserving due work. Loaded Mac rest readings and Windows' coarse CPU
counter quantization cannot certify it. The GPU experiments identify the
sampled draw as the dominant part of the present interval, but do not
separate every internal GPU dependency or clock-state contribution.

### Reproduction and retained evidence

The raw root is this worktree's `target/s0/`; all S1.4 inputs remain at their
original paths. Copies of each Mac bundle preserve the executable and probe
used by its named run. `provenance.json` stores their SHA-256 values and
environment. Windows JSON stores executable hashes, QPC endpoints, focus
losses and PresentMon loss counts; `release-symbols/` on the mirror retains
the sampled build's executable/PDB. Source inputs are retained in
`source.bundle`, `counters.patch`, `current-source.tar.gz` and
`final-source.tar.gz`; the final commits and counter-source copy identify
the later period-gauge build. No retained result directory was removed.

```sh
bun bench/heavy-list/gpui/exclusive.mjs --verify
bun bench/heavy-list/gpui/exclusive.mjs target/s1-4/g3-control/ink-1.json.trace.json --seconds=10
bun bench/heavy-list/gpui/exclusive.mjs target/s1-4/g3-before/ink-1.json.trace.json
bun bench/heavy-list/gpui/exclusive.mjs target/s0/period-counters/ink.json.trace.json
bun bench/heavy-list/gpui/exclusive.mjs target/s0/wake-callers/ink.json.trace.json --rest
python3 bench/heavy-list/gpui/normalize-windows.py
python3 bench/heavy-list/gpui/time-profile.py
python3 bench/heavy-list/gpui/main-attribution.py
python3 bench/heavy-list/gpui/wake-summary.py
```

`--json` retains all reducer fields; `--rest` selects the rest endpoints.
The published socket driver sets `G1_REST_TRACE=1` and exports once, after
rest. Without it, the first save is `.trace.json` and the next is
`.trace-2.json`, with separate interval buffers.
The reducer rejects dropped events and keeps GPU command/present spans
out of CPU-thread nesting. Service-cause elapsed rows use the same
innermost-span accounting as the main table, not inclusive service time.
Callback phase uses observed presented-time anchors; absent callbacks are
reported as absent. Thread CPU endpoints cannot retain already-exited
threads, and counter samples on Windows can precede CPU endpoints.

Build the live Mac carrier with the command above under S1.1, omitting
`--release` and using `CARGO_TARGET_DIR="$PWD/target"`; copy its bundle
before the next build. Production trust intentionally refuses the live
agent. The retained driver runs it without activation:

```sh
python3 bench/heavy-list/gpui/drive.py fresh-run --binary 'target/s0/Observed.app/Contents/MacOS/Heavy list' --cpu-only --dispatch --counters
```

The driver supplies `EXACT_AGENT=1 EXACT_AGENT_LIVE=1 G1_TRACE=1`, sends
1,200 phased wheel requests, and reads compact state every twelve requests.
`--delta 1 --hz 30` selects the integral GPU experiment; `--clear` replaces
only its final sampled draw; `--busy --busy-binary metal-busy-continuous`
runs the retained second-queue source. `--profile --vmmap --vmmap-at 5.8`
selects `sample` and the map; `--time-profiler` selects the retained xctrace
workflow. `--no-probe-display --no-probe-timer --wake-trace` selects the
rest-source experiment. Use a new output name on every run. The driver
records machine load continuously; a loaded run is diagnostic evidence,
not a clean performance result.

The Python drivers and reducers named above, plus `windows-*.py`, are
published under `bench/heavy-list/gpui/`. Run them from the repository root;
the historical defaults read retained inputs under `target/s0/`. The Windows
drivers refuse existing output directories. Profile reduction requires
`rustfilt`; Windows collection requires the existing G6 driver, PresentMon
and xperf. The other build, export and journey helpers below remain retained
in `target/s0/`, rather than published; they are not independent number reducers.

On the NucBox mirror, `windows-run.py` and `windows-pairs.py` call the
existing G6e driver; `windows-period.py` and `windows-final-pair.py` read
the final schema. `export-sampled.py` and `export-threads.py` retain xperf
exports against matching PDBs. `native-build-period.py`,
`native-tests-period.py` and `journey.py` run the S2.8b suites in fresh
`native-period/` and `journeys/period-d3d11-dpi/` output directories. The
fixtures' `EXACT_INK_TEST_OUTPUT` override preserves earlier S2 raw files.

### Verification

`target/s0/checks-final/results.json` records all commands, environment,
exit codes and result lines before the final paint-attempt correction.
That round's six checks pass, including both clippy
and fmt, plus `cargo test -p exact-ink`, the Apple carrier tests and
`cargo test -p exact-linux --lib --tests --no-fail-fast` with
`RUST_TEST_THREADS=1`. All seven pinned Metal groups and the image suite
pass with `EXACT_INK_DEVICE=metal EXACT_REQUIRED=1`. The reducer's nested,
clipped, crossing, equal-start and empty-window checks pass. The final
probe builds with `-Wall -Wextra -Werror` and completes live traces.

The first parallel Linux run failed one long-poll test because the shared
test executor could not obtain seven worker slots within thirty seconds.
Its isolated rerun and full serial rerun passed; the final serial run
passes too. No assertion or executor policy changed. The first Windows
device-loss journey recovered Mica on retry two while the fixture expected
retry one. Its isolated retry passed, and every `native-period/` suite passes:
37 groups plus the 96→144→96→144→96 DPI journey, with required-device flags.
The initial failures remain in `checks/linux.log` and
`windows/native/d3d11-device_loss.log`.

Verbatim stdout/stderr for that round is retained in `MAC-TEST-OUTPUT.txt` and
`WINDOWS-TEST-OUTPUT.txt`; `TEST-RESULTS.txt` contains their unchanged result
lines. The Mac originals are `checks-final/*.log`; Windows originals are
`windows/native-period/*.log` and `windows/period-d3d11-dpi/test.log`.

A final counter review found that `paint_attempts` was incremented only
after successful painter completion. The final observer counts a
non-geometry finish attempt before error handling, including a failed paint
or a present refusal. Existing `snapshots` retain their completed-frame
meaning. The historical tables above retain the old definition; their
live runs had no reported paint errors. Before the failure-path check:
the injected `allocation refused` paint must increase attempts by exactly
one while still returning that error. The existing group-error test checks
this with counters enabled in a hand run; the ordinary gate leaves counters
disabled and does not exercise that conditional assertion. No rendering or error
policy changes.

The enabled failure-path assertion passes on both Metal and D3D11. The
current-code Windows rerun (`windows/native-attempt/`) passes 37 of 38
groups and the DPI journey. Its device-loss group fails: hardware painting
recovers, but desktop composition stays unavailable through all three
Mica retries. The earlier passing round does not make this final round
green. No recovery policy or assertion changed; further retries stop,
and `QUEUE.md` records the unresolved qualification.

Fresh captures after that correction use `Attempts.app` on the Mac and
the `build-attempt/` release on Windows. `attempts-final/` measures
10.078251 seconds, with 913 paint attempts, 912 submissions and 909
displayed callbacks; it observes 13,777 fractional refusals, 13,450
A2-eligible. Its pool high water remains twelve targets. Process/main CPU
is 705.693/453.397 ms/s, at a maximum one-minute load of 154.26 with
compilers present: diagnostic only. Windows' `windows/attempt/` measures
10.066911 seconds, with 248 attempts/submissions in its 10.058003-second
counter window and 248 displayed frames from PresentMon. It observes
1,820 fractional refusals, 1,782 eligible. The process-lifetime pool high
water is 104 targets; at the sampled byte maximum, 94 are idle, ten are
frame-reserved and none incomplete. Its ten-second rest has fifty timer
wakes, fifty following dispatch passes and no displayed frame. These
captures verify the final observer in live carriers; they do not replace
the earlier prediction arms or improve their qualification.

The current-source six-check round is `checks-attempt/`. Its initial root
test run fails two wall-clock assertions: the Apple upload-stream test
times out waiting five seconds for an outcome, and the SVG expansion-limit
test takes 8.074 seconds against five. Build, clippy, fmt, caps, boot,
workspace, Ink, serial Linux, carrier and all required Metal groups pass.
The first final release link fails with `No space left on device`; only
this worktree's disposable `target/debug/incremental/` cache is removed.
All measurement files, test logs and retained bundles remain intact.
The release retry succeeds and is retained as `PostedFinal.app`.
Both isolated root-test reruns pass (upload 0.83 s; SVG 3.96 s), but the
serial root rerun reproduces the upload timeout inside the asynchronous-core
sub-suite. This is the third round on that problem; retries stop without
changing its code or assertions. `QUEUE.md` records it. The root test
check remains failing, independently of the Windows device-loss failure.
The serial run finishes with only `exact-apple --lib` failing; SVG and the
other targets pass. Current-source stdout/stderr is retained verbatim in
`target/s0/MAC-TEST-OUTPUT-FINAL.txt` and
`target/s0/WINDOWS-TEST-OUTPUT-FINAL.txt`, with unchanged result lines in
`target/s0/TEST-RESULTS-FINAL.txt`. The originals are
`checks-attempt/*.log`, `windows/native-attempt/*.log` and
`windows/attempt-d3d11-dpi/test.log`; `checks-attempt/retry-results.json`
records both isolated runs and the serial root run. This qualification is
not green: five of the six checks pass, and Windows has the separate
device-loss failure. The enabled paint-error counter checks and all named
pinned Metal/D3D11/wgpu suites pass.

The GO wait expires at **2026-10-30T18:10:26Z**, three hours after the lane
start, with no `target/s0/GO`. `go-poll.jsonl` retains every bounded poll;
`go-expired.txt` records expiry. `PostedFinal.app` and `posted-profile.py`
are prepared, but no posted Mac fling or profile is run. The agent-driven
`sample` and Time Profiler captures are the delivered profiles. Stage 0
remains unmet: there is no quiet posted qualification, the historical
untagged CPU budgets cannot be reconstructed, and the final root and
Windows device-loss checks fail as detailed above. Work stops at this
boundary. `evidence.sha256`, `final-source-sha256.json` and
`final-source.patch` identify the retained evidence and final sources.

### Prediction ledger

| Prediction written before its run | Result on the named instrument |
|---|---|
| Full control window: 3579 service / 358 walks / 348 draws | 3581 / 358 / 348: service-count miss; the explicit ten-second cut gives 3579 / 358 / 348 |
| Hand-table elapsed rates round to 12.6 / 5.7 / 5.9 / 1.2 / 1.0 / 0.4 / 1.3 ms/s | Ten-second script: 12.717 / 5.598 / 5.931 / 1.300 / 1.041 / 0.511 / 1.339; service, walk, glyph and layout rounding miss |
| Callback phase approximately 5 ms | Historical p50 5.072 / 5.098; fresh 5.108 ms: met |
| Fresh release carrier exposes fractional refusals through the agent | Release trust refuses that agent; unexercised. The optimized development carrier supplies the live observation |
| At least one fractional refusal; at least 50% A2-eligible | Mac 13781 / 97.63%; Windows 1832 / 98.03%: met |
| Mac pool retains ≥16 targets | High water 12: miss |
| ≥100 MiB idle at the pool's byte high water | Largest coincident sample 75 MiB; separate idle high water 103.125 MiB: unresolved, not a measured coincidence |
| Dispatcher-only passes remain nonzero | Corrected integrated full-probe run: 1838 in `review-fixes/dispatch-rebased/`: nonzero prediction met; the review continuation's separate <1388 bound misses |
| Enabled increment <1 µs | 1.265 ns per iteration in the retained optimized increment test: met for that instrument |
| Full observer adds <5% CPU | Earlier Windows medians +10.76%: miss; final-schema pair +1.75%: met for one pair; Mac pair invalid; bound not established |
| Windows pool <1201 targets and fractional refusals >0 | 80 targets and 1832 refusals: met |
| Disabling probe display callbacks removes ≥50% of dispatcher-only passes | 41.79% in the pre-fix cause category: miss; not the corrected dispatch definition |
| Disabling probe memory timer removes ten rest marks | Ten → zero: met |
| Clear present mean <0.35 ms | 0.030 ms at 120 Hz; 0.058 at 30 Hz: met |
| Busy-queue sampled present mean <0.5 ms if clock state is the sole cause | 0.874 ms with the short queue: miss; 0.459 with longer commands at 120 Hz: met; 0.654 at 30 Hz: miss |
| Longer queue covers >90% of the interval | 89.24% sampled / 94.89% clear at 120 Hz: miss / met; 99.62% sampled at 30 Hz: met |
| 30 Hz sampled mean exceeds the 120 Hz 0.413 ms | 1.078 ms: met |
| ≥90% of Time Profiler weighted samples name a carrier, probe, image, present or platform owner | 99.15% have resolved leaf symbols; 90.71% after excluding unresolved leaves and bare dispatch entries: met at that broad level; dispatch component callers remain unknown |
| Forty timer rearms each precede a second rest dispatch | Forty winit arms and forty following turns in both request traces: met |
| A failed paint increases attempts exactly once and preserves its error | Enabled-counter `allocation refused` test passes its +1 assertion; error unchanged |
| Stage 1 M1: 44 → ~33 main CPU ms/s; M2: ≤0.4 rest process CPU ms/s | Future conditional targets, not achieved or certified by these Stage 0 diagnostics |

### Landing recheck after rebasing onto `fc321f962e`

The rebase preserves main's G5 section before Stage 0 and its RFC r9
amendments. `RESULTS.md` and `probe.m` conflicted; `QUEUE.md` and the RFC
merged cleanly. The probe keeps G5's workload and GPU-phase accounting
with Stage 0's thread CPU endpoints, peak sample and optional timers.
Its trace sidecar now names the selected workload, and timer cancellation
permits the memory timer to be disabled. An offscreen Metal test builds
with `-Wall -Wextra -Werror` and retains three GPU completions, the `bar`
endpoints and thread CPU with both probe timers disabled. Its source,
build log, output and JSON are `target/s0/landing/probe-check*`.

The six checks pass with `RUST_TEST_THREADS=2`, including both clippy and
fmt. The complete root run passes
`a_burst_of_photo_uploads_goes_three_at_a_time_and_leaves_other_streams_their_places`
and `the_suite_passes_on_the_asynchronous_core`. The earlier upload
timeout remains a recorded flake in `QUEUE.md`; no assertion, timeout or
executor policy changed. Commands, environment, exits and unchanged
result lines are in `target/s0/landing/checks/results.json`; the original
stdout/stderr is in that directory's named logs.

Ink, the full Linux lib-and-tests run, the Apple carrier, all seven pinned
Metal groups and image tests also pass. Metal uses
`EXACT_INK_DEVICE=metal EXACT_REQUIRED=1` and two test threads. Every Mac
command exits zero. `target/s0/landing/MAC-TEST-OUTPUT.txt` retains the
complete stdout/stderr; `MAC-RESULTS.txt` retains the unchanged result
lines beside each command and environment.

On the NucBox, the Shelf build and test executables build from the rebased
sources. Both D3D11 and wgpu pass the seven pinned groups and image tests,
and each Windows library run passes 39 tests. The first Ink library runs
from SSH fail `a_device_less_compositor_cannot_create_a_cpu_surface`:
`DCompositionCreateDevice2(None)` returns `E_ACCESSDENIED` without an
interactive desktop. The same executables run through the existing
interactive scheduled-task driver and each pass 44 tests. Those runs
create no window or focus request. Both enabled-counter refused-paint
checks pass. The failed SSH logs remain alongside the passing runs under
`target/s0/landing/windows/{headless,desktop-ink}/`; `native/build.json`
and `provenance.json` identify the executables and source hashes.

The Windows carrier and D3D11 source files are unchanged between
`e4ead5ae81` and the rebased tip: G6d's bounded Mica retry was already in
the earlier failing build. A passing rerun therefore cannot establish
that this rebase added its recovery fix. The source comparison is retained
in `target/s0/landing/windows-rebase-source.json`.

The continuation's GO window, **2026-10-30T18:14:31Z–21:14:31Z**, expires
without `target/s0/GO`. `target/s0/landing/go-poll.jsonl` retains the
thirty-second polls and `go-expired.txt` records expiry. No forced
device-loss, DPI, resize, minimize/restore, caret, sleep/wake, shared-window,
composition-window or startup/fallback journey runs in this recheck.
The prepared runner is `target/s0/landing/native-journeys.py`, copied to
the NucBox as `target/s0/landing-native-journeys.py`. Mica recovery remains
unverified, so there is no new failure to bisect and no recovery change.
The passing suites do not close that qualification. `QUEUE.md` retains it.

The exclusive reducer's verification passes. An additional historical
G3c aggregation fails because its older driver records lack
`emptyCallbacks`; main's unchanged reducer fails on the same inputs.
`QUEUE.md` records the missing-data handling problem. The diagnostic and
main-control logs are `target/s0/landing/aggregate-{diagnostic,origin-main}.log`.
No reducer policy changes in this recheck.

These are correctness checks, not new performance measurements. The
earlier Stage 0 measurement limits still apply. All raw evidence remains
retained; only this worktree's disposable incremental compilation caches
were removed as the shared disk filled. The paths are recorded in
`target/s0/landing/disposable-cache-removal.json`.

Predictions were written in `target/s0/landing/plan.json` before this
recheck. No new performance target was predicted or measured.

| Prediction | Result on the named instrument |
|---|---|
| Six checks with `RUST_TEST_THREADS=2`: every command exits 0; the upload timeout does not recur | Met: all seven invocations exit 0, including clippy and fmt; the full root run passes the upload and asynchronous-core tests |
| NucBox `shelf_device_loss`: exit 0 and recovered Mica pixels within the existing band, with no recovery policy change | Unexercised: the three-hour wait expires without the fresh `target/s0/GO` required for this display journey |
| Metal pinned/image, Ink, Linux and native Windows suites: every command exits 0 | Miss on the initial Windows Ink launches under SSH (`E_ACCESSDENIED`); both pass in the interactive desktop session. All 18 Mac commands and 22 completed Windows groups pass; display journeys remain unrun |

`target/s0/landing/TEST-RESULTS.txt` collects the unchanged result lines.
Complete output is retained in `MAC-TEST-OUTPUT.txt` and
`WINDOWS-TEST-OUTPUT.txt`; the latter includes
the failed SSH launches as well as the passing desktop runs. The rebase's
range comparison is `rebase-range-diff.txt`; `verified-source.bundle`
and `verified-source-sha256.json` identify the code that passed.

### Stage 0 review fixes: predictions before the runs

The two reviews of `e4ead5ae81` are retained in
`target/review-{sol,claude}-s0.md`. This continuation corrects observations
and their cost without changing rendering, reuse or scheduling policy.
Evidence goes in `target/s0/review-fixes/`; earlier raw files stay intact.

- The same 1,200-request Mac socket fling, with the full probe, will record
  more than zero and fewer than 1,388 dispatch-only passes after redraw
  turns carry `DRAW`. `exclusive.mjs` is the count instrument; the new
  elapsed rate is not a CPU saving or a quiet posted-input qualification.
- The optimized million-iteration counter test will cost less than 5 ns
  per disabled iteration and less than 1 µs enabled. This tests increments,
  not a bound on the complete observer.
- The NucBox forced-device-loss journey will exit zero and recover Mica
  within the existing pixel band. The other display journeys and native
  suites will exit zero in its interactive desktop session, without a GO
  file. Lock/unlock remains outside the existing journeys.
- The six checks, Ink, Linux, carrier and pinned Metal suites will exit
  zero with `RUST_TEST_THREADS=2`.

The pre-rebase corrected Mac trace has zero dropped events and all 1,200
wheel requests. Its count prediction is met: **1,184**, between zero and 1,388.
The full by-cause table is `review-fixes/dispatch/exclusive.{md,json}`;
`cause-check.json` independently counts 1,089 draw-owned passes and 1,089
trailing passes with `DRAW`. The probe window is 10.058512 seconds, the
request window 9.992964 seconds. The maximum one-minute load is 45.85,
with compilers present. This is attribution evidence, not a performance
comparison with the earlier loaded run. The integrated rerun below replaces
this count in M1's verdict; its quiet posted-input CPU prediction remains
unqualified.

The probe's offscreen check saves one `first-save` event, then one
`second-save` event in a separate file; neither save repeats the preceding
buffer, and each resets the dropped count. It retains G5's three GPU
completions and workload endpoints. Its source, strict compiler output and
raw JSON are `review-fixes/probe-check*`.

The wgpu Mac carrier completes another 1,200-request drive and reports
`submitted_frames`, `displayed_frames` and `displayedFrames` as null at
boot, fling start and fling end (`review-fixes/wgpu-state/`). Its backend
has neither observation hook; the probe's independent drawable observation
does not make these state fields supported. Direct Metal exports displayed
frames by name to both state and the probe.

The published Python reducers run successfully on the retained inputs:
Windows normalization, Time Profiler, main-thread attribution and wake
summary. Their stdout/stderr is in `review-fixes/*profile.log`,
`main-attribution.log`, `normalize-windows.log` and `wake-summary.log`.

The review fixes have these dispositions:

| Finding | Change and verification |
|---|---|
| Sol 1 / Claude 3: disabled cost | `counters.rs` initializes the flag at carrier/device startup and guards each compound observation once. `ink/rows.rs`, `paint/rows.rs` and `layer.rs` keep diagnostic work behind that guard; Windows cause writes are guarded. The disabled closure test and optimized increment runs pass. |
| Claude 1 / 5: service causes | Both carriers mark redraw turns `DRAW`; Mac consumes the mask with `mem::take`; Windows clears the modal draw's `TIMER`. The service classification test passes and the fresh Mac trace distinguishes both draw passes from dispatch. |
| Sol 2 / Claude 6: pool gauges | `begin()` and `trim()` refresh ownership gauges even after a partial mutation returns an error. Completion partitions retain their existing sample, with its serial and monotonic-derived timestamp named in the schema. The enabled pool retirement/resize/trim assertions pass. |
| Sol 3: unsupported observations | Presentation capabilities determine nullable submitted/displayed state. The Mac wgpu drive verifies nulls; direct Metal and D3D11 retain their supported observations. |
| Sol 4: accepted presents | D3D11 counts each accepted real present before fallible submission/activation bookkeeping, including the activation retry. Test-only and refused presents remain excluded. The native enabled-counter assertion checks both accepted paths and refusals. |
| Sol 5: trace clock | Windows span starts and sample timestamps derive from one wall/monotonic anchor. A timestamp-delta test checks elapsed monotonic time. |
| Claude 2: monitor queries | Windows queries at `resumed`, `Moved` and `ScaleFactorChanged`. The desktop DPI journey exercises the transition. |
| Claude 4: reproduction scripts | The ten Python drivers/reducers are published under `bench/heavy-list/gpui/`; retained helper dependencies are named in Reproduction. Four historical reductions run successfully. |
| Claude 7: enabled allocations | Observer cost above now names glyph vectors, JSON events and samples, including their retained memory. No full-observer bound is claimed. |
| Claude 8: classification precedence | The existing suspect-membership precedence remains: `rows_suspect_invalid` means an invalid row was also marked suspect; it does not establish that suspicion alone caused invalidity. No gate or classification policy changes. |
| Claude 9 / 10: probe exports | Each save clears interval/drop counts and retains a distinct sidecar; the offscreen two-save test passes. Displayed frames are exported by name. |
| Claude 11: failed-paint assertion | The earlier assertion is identified as an enabled-counter hand run; this continuation also runs that assertion explicitly on the NucBox. |

The new optimized million-increment runs retain 434,542 ns disabled and
1,116,875 ns enabled: **0.435 ns / 1.117 ns per iteration**. Both predictions
are met (<5 ns disabled, <1 µs enabled). Raw output is
`review-fixes/counter-{off,on}.log`. These results do not establish the cost
of service classification, glyph capture, pool observation or tracing.

The NucBox's interactive desktop passes forced device loss with
`S2_BAND device-loss mean=0 over32=0`: recovered and fresh Mica pixels match
in the existing band. The recorded CPU-to-ink recovery is 2804.306 ms;
this is a correctness journey, not a timing target. DPI, resize,
minimize/restore, caret and sleep/wake also pass. No GO file is required on
this dedicated machine. There is no new lane failure to bisect; this pass
does not explain the earlier intermittent compositor failures or establish
that counters caused them. Recovery policy is unchanged.

The rebase onto `03822d449a` preserves Stage 0b's asynchronous startup and
its probe readiness criteria. Its CPU startup path uses the same Metal
layer and displayed callback, so Stage 0 now counts an accepted CPU present
at command submission too; unavailable drawables remain excluded. Counter
initialization occurs when startup opens, and attachment declares both
observations supported. This adds observations only.

Before the integrated rerun: the same full-probe 1,200-request Mac drive
is again predicted to retain more than zero and fewer than 1,388 dispatch-only
passes, with all redraw trailing passes carrying `DRAW`. `drive.py --cache-dir`
selects a new retained cache directory so this run also exercises Stage 0b's
archive miss. The probe should record a CPU submission followed by ink, and
frame counter state should remain available. The six checks and affected
Mac host/pixel suites are predicted to exit zero after this rebase.

The integrated trace completes all 1,200 requests in 9.996654 seconds;
its probe window is 10.056452 seconds with zero dropped events. It counts
**1,838 dispatch-only passes**, so the repeated **<1,388 prediction misses**.
There are 1,016 draw-owned passes and 1,016 trailing carrier passes with
`DRAW`. `review-fixes/dispatch-rebased/exclusive.{md,json}` retains the
complete by-cause table; `cause-check.json` independently derives these
counts. `redraw-followup-check.json` also checks that each of the 1,016
draw services is followed by a carrier service carrying `DRAW`. M1 above
uses this result. Maximum one-minute load is 56.45 with
compilers present, so neither the changed dispatch count nor its
6.328 exclusive elapsed ms/s establishes a CPU improvement or regression.

The cache-miss trace records four CPU present spans before the fling,
followed by direct Metal. At the start snapshot, one ink paint attempt
and those four CPU submissions account for all five submitted frames;
submitted and displayed state remain available at boot, start and end.
`startup-check.json` retains the assertions. The dedicated cache directory,
`ObservedRebased.app`, trace and source provenance remain under
`review-fixes/`. The strict offscreen probe and reducer verification also
pass after the rebase.

Before the rebase, all 18 Mac commands pass, including the six checks,
Ink, Linux lib/tests, the carrier, the seven pinned Metal groups and image
tests. The NucBox passes all 41 desktop groups: D3D11 and wgpu host/pixel
suites, device-loss/DPI/resize/minimize/caret/sleep journeys, shared-window
and composition groups, ten startup/fallback cases and three explicit
counter assertions. Native Ink passes 48 tests on each backend, with no
`E_ACCESSDENIED` launch. `windows/native/results.json` and `windows/dpi/`
retain results and captures. The Windows carrier, D3D11 backend, counters, pool and row-observation
files are unchanged by this rebase (`windows-rebase-source.diff`). The
native suites qualify `cdd6da8030`; their binaries and source hash remain
those in `windows/provenance.json`. The required post-rebase six checks
run locally against the integrated source.

The unchanged output is retained in `review-fixes/checks/TEST-OUTPUT.txt`
and `review-fixes/WINDOWS-TEST-OUTPUT.txt`; their `RESULTS.txt` and
`WINDOWS-RESULTS.txt` companions retain each suite's exact result lines.

After rebasing onto `03822d449a`, all 18 Mac commands pass again with
`RUST_TEST_THREADS=2`: the six checks (including clippy and fmt), Ink,
Linux lib/tests, carrier, seven pinned Metal groups and image tests.
Commands, environment, exits and exact result lines are in
`review-fixes/checks-rebased/results.json`; complete unchanged output is
`checks-rebased/TEST-OUTPUT.txt`. The Linux library reports 1,637 passed,
zero failed and 22 ignored; Ink reports 46 passed, zero failed and one
ignored; the carrier reports four passed and zero failed. The NucBox's
41 passing groups remain the pre-rebase native qualification described
above. `review-fixes/TEST-RESULTS.txt` collects all named suite result lines.

| Continuation prediction | Result on its stated instrument |
|---|---|
| Pre-rebase full-probe socket drive: 0 < dispatch-only passes < 1,388 | 1,184: met |
| Integrated full-probe socket drive: the same bound | 1,838: miss; all 1,016 redraw trailing passes carry `DRAW` |
| Optimized disabled increment <5 ns | 0.435 ns per iteration: met |
| Optimized enabled increment <1 µs | 1.117 ns per iteration: met |
| Archive miss submits CPU then ink and keeps frame observations available | Four CPU submissions, then ink; five total before fling, with available state: met |
| NucBox Mica recovery and remaining desktop groups exit zero | All 41 pass; recovered/fresh Mica band mean 0, over32 0: met |
| Six checks and Mac host/pixel suites exit zero | Both 18-command runs pass; the final guard follow-up has timing failures below: overall prediction misses |

M1's **44 → ~33 main-thread CPU ms/s** remains a conditional Stage 1
prediction. It requires the quiet posted-input instrument; these loaded
socket traces establish attribution. The full-observer CPU bound also
remains unestablished. Earlier intermittent Mica failures remain retained;
this passing journey supplies no failing case to bisect and changes no
recovery policy.

The rebase comparison, source bundle and SHA-256 manifest are retained as
`review-fixes/rebase-range-diff.txt`, `verified-source.bundle` and
`verified-source-sha256.json`. All previously cited raw files remain intact.

Final review moves the CPU startup observation's result check behind its
enable load. The final source again passes build, clippy, fmt, caps, boot,
workspace and Ink, with two test threads. **The final root test check is
not green.** Its first follow-up fails
`exact-svg-file::tests::a_file_that_expands_past_a_limit_draws_nothing_and_names_it`
at 8.218781 seconds against its five-second limit. The isolated second
round reproduces it at 8.294572 seconds. The third full root round passes
SVG but fails the already-recorded Apple upload-stream test inside
`the_suite_passes_on_the_asynchronous_core`: `an outcome within five
seconds: Timeout`. That full command exits 101. Retries stop after three
rounds; no assertion, deadline, SVG implementation or executor policy
changes. `QUEUE.md` records both unresolved timing failures.

The two completed 18-command runs and the NucBox results above remain
valid for their named sources; they do not turn the final root check green.
`review-fixes/checks-final/results.json`, `svg-limit-isolated.log` and
`root-final-round3/{result.json,test.log}` retain the final checks and retry
sequence. `MAC-TEST-OUTPUT.txt` now collects all unchanged Mac output,
including both failures and the third run; `MAC-RESULTS.txt` and
`TEST-RESULTS.txt` include their exact result lines. The final gate cannot
be certified within the required three-round limit.

### Stage 0 landing 2: rebase and quiet-machine checks

Rebased thirteen lane commits, previously ending at `df4b1f29e2`, onto
`origin/main` `5fa5fcc2f3` (27 upstream commits). The RESULTS conflict keeps
main's Stage 0b, G4m and G5m records, then the lane's Stage 0 sections,
unchanged. The QUEUE conflict keeps both sides' follow-ups. The RFC merges
automatically: Stage 0's attribution stays in §2 and main's measurement
lines in §3 stay intact. No rendering or deadline changes are made here.

**Prediction before the checks:** all six checks exit zero, with both
Clippy and fmt included in lint. Commands run serially, with the pinned
Bun 1.4.2, `RUST_TEST_THREADS=2` and this worktree's own `target/`.
The quiet gate requires one-minute load below 30 and no other worktree's
Rust, Swift or Clang compiler. Polling is once a minute in batches of at
most ten minutes, up to ninety minutes. The wait, commands, exit codes
and unchanged output are retained under `target/s0/landing2/`.

The first gate passed at **15:50:19 PDT**, after **3 min 1 s**: load
20.70, with no compilers. Compilation resumed before caps; that first caps
passed, and build started at load 25.17 with no compilers and passed.
The first root-test attempt then started at load 76.36. It was interrupted
without a completed verdict, with no failures reported before interruption.
Its unchanged output and recorded process IDs remain in `test.log`,
`interrupted-test.json` and `stopped-processes.json`.

The remaining commands each waited for their own quiet observation. Caps
resumed at **16:24:21 PDT** after another **27 min 57 s**, at load 29.04
with no compilers, and passed. Tests waited another minute; Clippy two
minutes; fmt one minute. Total gate waiting was **34 min 59 s**. The
ninety-minute limit did not expire. Test/Clippy/fmt/boot/workspace started
at loads **19.79/26.17/21.96/19.96/19.96**, each with no compilers.
Every completed command had a quiet start; observations record later
compiler activity during some checks. `quiet.jsonl`, `checks-machine.jsonl`
and `qualified/machine.jsonl` retain the observations, including the
loaded interruption and the resumed wait. Early per-process inspection
stretched one ten-minute polling batch to 10 min 15 s; the resumed controller
then used one batched process query to keep the minute cadence.

**Prediction met: all six completed checks exit zero.** Lint includes both
Clippy and fmt. The root command emits 113 passing result summaries:
7,384 passes, zero failures and 83 ignored, including nested child reruns.
Both known timing cases pass, so no isolated binary rerun is needed.
Their deadlines are unchanged. The existing queued timing investigations remain;
this passing run does not explain the earlier failures.

| Check | Command | Exit | Verbatim completion output |
| --- | --- | ---: | --- |
| build | `cargo build --all-targets --keep-going` | 0 | ``    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 32s`` |
| test | `cargo test --tests --no-fail-fast` | 0 | All 113 result lines in `FINAL-RESULTS.txt` |
| lint | `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | ``    Finished `dev` profile [unoptimized + debuginfo] target(s) in 55.54s`` |
| lint | `cargo fmt --all -- --check` | 0 | No output |
| caps | `bun scripts/caps.mjs` | 0 | `All budgets within cap. 10 categories inspected.` |
| boot | `bun scripts/boot.mjs` | 0 | Full output in `FINAL-RESULTS.txt` |
| workspace | `cargo check --workspace --all-targets --keep-going` | 0 | ``    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6m 27s`` |

The two previously failing binaries' final result lines, verbatim:

```text
exact-apple --lib:
test result: ok. 564 passed; 0 failed; 18 ignored; 0 measured; 0 filtered out; finished in 15.60s
exact-svg-file --lib:
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.03s
```

`FINAL-RESULTS.txt` retains every check's completion lines;
`FINAL-OUTPUT.txt` retains their complete output, including warnings.
`final.json` records commands, environment, exits, durations and log hashes.
All paths in this subsection are under `target/s0/landing2/`; the completed
root tests, lint, repeated caps, boot and workspace logs are in `qualified/`.
The initial completed build remains `build.log`. `range-diff.txt` and
`merge-audit.txt` retain the rebase comparison: executable patches are
unchanged, and the RESULTS sections and RFC measurement lines are preserved.
No prior raw evidence was deleted. No benchmark series was rerun in this
mechanical landing pass, and Stage 0's outstanding qualification is unchanged.

Third rebase onto `43e0d93b85`: the `probe.m` and `window.rs` conflicts retain Stage 0's cause attribution and traces alongside Stage 0b's retries until a frame displays; all six checks pass, including Clippy and fmt (`target/s0/landing3/`).

## G4m, the instrument's share

**Predictions before measurement, 2026-10-30.** Five rounds per configuration,
interleaved by round on the silver Mac: (a) command-buffer hook off, power
sampling off; (b) hook on, power off; (c) hook on, power on as G4m. The
unchanged posted fling and probe CPU endpoints measure paired differences.
Predicted process CPU added by the hook, in ms/s: native −5 to +5, GPUI
+1 to +10, Ink +1 to +10, egui +5 to +20. Predicted power-sampler cost:
0 to +5 ms/s per arm. Presented frames/s should change by at most 5% for
each painted arm, and median peak footprint by at most 10% for every arm,
in (b) versus (a) and (c) versus (b). These are hypotheses, not gates.
[`hook-plan.txt`](hook-plan.txt) retains the pre-run predictions and denominators,
with the review correction to qualification and the instrument rule dated apart.

**No positive hook cost is resolved.** Every paired hook-on minus
hook-off CPU range crosses zero. Hook-off CPU is higher than G4 for every
arm, with disjoint ranges. Configuration (c) measured lower app CPU; the prediction of a small positive
power-sampler cost misses for all four arms. Most of (c)−(b) is on the main
thread, with cadence and received input unchanged: the same observed work took
fewer milliseconds. The hypothesis is that the sampler's steady runnable load
holds a cluster out of its lowest performance states between frames, so the
app's bursts run at a higher clock. This is not an isolated cause. The order
puts (c) after (b) in four rounds of five and is not fully counterbalanced;
time, cache and frequency-state drift remain confounds. No renderer saving is
claimed. The probe now records `ri_instructions` and `ri_cycles` from its
existing `rusage_info_v4` read at every phase endpoint. Equal instructions
with fewer milliseconds would distinguish clock from less executed work; cycles
also permit comparison across machines and energy states. Future series retain
`pmset -g` and `pmset -g batt` for power mode and source. The old series did not
record them, so this chunk cannot settle that hypothesis.

Measured on 2026-10-30, 15:58–16:22 PDT, on the dedicated silver M5 Max.
The checkout was refreshed to `origin/main` `f069d37722`; the probe change is
`c22ef19622`. Only the carrier and probe were rebuilt. Native, GPUI and egui
are copied from `42d5066572`, byte-identical before their code signatures;
re-signing includes the changed probe. The driver and wheel-helper binaries
are unchanged from original G4m. All 105 data hashes and the GPUI/egui copies pass. The shared
probe SHA-256 is
`cc37debaa120a87f8e841f42205f5af7c50d0eede7af1b15b406823de702162c`.

Five rounds rotate configurations **abc, bca, cab, abc, bca**; each round
rotates the four arms as the existing driver does. Every sample starts after
complete-scene readiness, then runs four 600-point flings over ten seconds
and ten seconds at rest. There are no cold-only launches. Every configuration
uses the same binaries, retained caches, 800×800-point outer window, 2× scale,
120 Hz display and disabled tracing/counters. Configuration (c) uses
`powermetrics --samplers cpu_power,gpu_power,tasks --show-process-energy
--show-process-gpu -i 1000`. The recorded sampler PID exits before (a) or (b).

**60/60 samples pass**, from 61 attempts. The first egui launch in (c) round
one fails startup readiness; its one permitted retry passes. Both remain
retained. There are no focus losses, thermal changes or Low Power Mode samples.
All **285/285** observations pass the old load/compiler rule, with maximum
1/5/15-minute loads **11.01/8.96/8.72** and no compilers. That rule did not
qualify this machine as quiet: the tasks plists retain the standing background
described below. Every sample posts 424 wheel events.
Enabled GPU counts all complete, with zero invalid timestamps; disabled runs
install no queue or buffer hook and record zero observed buffers.

**The background.** The five (c) round windows average **5.6–5.9 busy cores**
(`ALL_TASKS`), with **1.8–1.9 cores in PerfPowerServices**, roughly **2.4–2.7
cores/s of DEAD_TASKS churn**, about nineteen `fleet` processes and a `claude`
process with bursts above **1.6 cores**. The table below includes startup and
inter-arm gaps as well as the phases; its largest Claude burst is **2.06 cores**.
The original G4m and G5m plists show the same background. At 16:56 the
orchestrator confirmed PerfPowerServices at 196% CPU for 29 hours, `fleet serve`
and its plugin services, a Claude Code session with shell loops polling every
five seconds, three booted iOS simulators and DeviceHub. Load averages and a
compiler-name check cannot see that work.

| (c) round | Round samples | Busy cores, mean / max | PerfPowerServices, median cores | DEAD_TASKS, mean cores | fleet count, median | claude, max cores | Pass ≤2 busy / ≤0.25 PerfPowerServices |
|---|---:|---:|---:|---:|---:|---:|---:|
| power-1 | 111 | 5.58 / 11.98 | 1.89 | 2.42 | 20 | 1.43 | 0 / 0 |
| power-2 | 83 | 5.61 / 9.94 | 1.88 | 2.41 | 19 | 1.58 | 0 / 0 |
| power-3 | 83 | 5.79 / 9.77 | 1.87 | 2.46 | 19 | 2.06 | 0 / 0 |
| power-4 | 83 | 5.74 / 9.46 | 1.88 | 2.45 | 20 | 1.93 | 0 / 0 |
| power-5 | 83 | 5.87 / 11.78 | 1.87 | 2.58 | 20 | 2.06 | 0 / 0 |

Proposed qualification: at every five-second observation, **≤2 whole-machine
busy cores** and **≤0.25 core for the largest non-benchmark process**, in
addition to all load averages below 30 and no compiler. `run-hook.py` records
busy cores from deltas of the system's CPU ticks, including exited tasks, and
the five largest non-benchmark processes from `ps -Ao pid,pcpu,comm` in
`quiet.jsonl`. Process CPU is `ps`'s recent average; it is not the same interval
instrument as the whole-machine ticks or the plists. Only the exact prepared
app, driver and wheel-helper paths are excluded from the process ranking.
The old rule passed all 285 observations; **none of the 443 retained (c) round
samples passes either proposed bound** (PerfPowerServices alone fails the
process bound). The (a)/(b) rounds have no historical per-process sampler, so
no pass count can be reconstructed for them. These bounds reject this measured
background; they do not establish that a quieter machine has been observed.
Before the next series the owner must stop the fleet/Claude session and its
polling loops, the simulators and DeviceHub, and PerfPowerServices if it is
spinning. This lane kills none of them.

Medians (minimum–maximum), five samples per cell. Presented frames/s uses the
whole ten-second fling, including pauses; native has no presented-frame
instrument. GPUI includes unchanged re-presentations. The G4/G4m text,
scroll-distance, readiness and whole-application limits still apply.

| Arm; metric | G4 | G4m original (c) | (a) hook off, power off | (b) hook on, power off | (c) hook on, power on |
|---|---:|---:|---:|---:|---:|
| native; process CPU, ms/s | 131.91 (112.39–143.12) | 156.91 (151.68–175.09) | 174.87 (167.55–188.56) | 179.23 (169.59–181.80) | 165.00 (156.16–172.07) |
| native; main CPU, ms/s | 84.51 (76.30–92.87) | 106.06 (102.76–111.39) | 118.63 (114.85–120.12) | 119.31 (118.50–121.22) | 110.18 (104.14–113.63) |
| native; presented frames/s | — | — | — | — | — |
| native; peak footprint, MiB | 228.39 (211.35–232.45) | 235.74 (198.41–240.30) | 230.92 (209.39–239.33) | 233.77 (231.49–236.70) | 237.16 (229.63–238.89) |
| gpui; process CPU, ms/s | 84.92 (82.05–89.05) | 129.79 (120.63–135.25) | 156.13 (136.13–162.86) | 152.41 (139.05–163.43) | 132.29 (128.78–141.38) |
| gpui; main CPU, ms/s | 39.56 (38.97–41.52) | 65.55 (59.68–66.71) | 78.98 (68.79–83.15) | 78.95 (69.85–84.85) | 66.63 (63.16–71.01) |
| gpui; presented frames/s | 88.59 (88.38–89.00) | 89.09 (88.98–89.19) | 89.16 (88.87–89.29) | 89.17 (88.89–89.28) | 89.16 (89.07–89.29) |
| gpui; peak footprint, MiB | 406.42 (391.64–413.84) | 413.75 (412.16–421.85) | 418.17 (412.63–431.42) | 413.56 (404.77–421.25) | 407.45 (402.74–417.09) |
| ink; process CPU, ms/s | 86.41 (84.98–87.07) | 110.90 (105.46–119.73) | 128.00 (105.84–133.51) | 123.07 (116.70–136.40) | 117.29 (107.42–118.57) |
| ink; main CPU, ms/s | 43.83 (41.98–44.65) | 55.62 (52.68–60.11) | 63.95 (56.19–67.87) | 62.71 (60.57–68.31) | 57.58 (51.81–60.33) |
| ink; presented frames/s | 32.60 (32.29–33.20) | 32.99 (32.89–33.19) | 33.00 (32.99–33.20) | 33.00 (32.79–33.09) | 32.90 (32.69–32.90) |
| ink; peak footprint, MiB | 591.56 (587.67–593.42) | 598.47 (592.45–606.92) | 602.66 (592.97–604.69) | 604.42 (600.89–606.81) | 595.02 (592.34–601.25) |
| egui; process CPU, ms/s | 66.84 (64.93–71.54) | 108.28 (99.29–109.32) | 119.76 (105.58–123.56) | 116.97 (105.62–121.63) | 103.21 (100.23–114.13) |
| egui; main CPU, ms/s | 31.33 (30.28–33.69) | 50.36 (46.79–51.13) | 56.49 (49.97–58.43) | 54.89 (49.59–58.34) | 48.90 (45.30–52.01) |
| egui; presented frames/s | 40.90 (40.49–41.28) | 41.79 (41.48–41.89) | 41.70 (41.49–41.79) | 41.70 (41.59–41.80) | 41.79 (41.69–41.79) |
| egui; peak footprint, MiB | 507.81 (504.14–515.89) | 512.95 (505.34–518.39) | 514.19 (508.05–516.11) | 512.52 (509.25–520.52) | 514.08 (501.20–519.83) |

| Arm | Hook process CPU, ms/s | Hook main CPU, ms/s | Completed buffers in (b), ten seconds | Hook process CPU, µs/buffer | Power process CPU, ms/s | Power main CPU, ms/s |
|---|---:|---:|---:|---:|---:|---:|
| native | -2.140 (-11.726–11.682) | 1.919 (-0.964–4.227) | 0 (0–0) | — | -10.285 (-20.676–-4.590) | -9.137 (-15.021–-6.195) |
| gpui | 3.511 (-23.809–10.046) | 3.485 (-13.300–3.832) | 896 (893–897) | 39.326 (-265.805–112.381) | -11.023 (-34.656–-4.606) | -7.938 (-18.693–-2.509) |
| ink | 2.896 (-6.222–10.862) | 0.445 (-5.804–4.377) | 1236 (1234–1238) | 23.440 (-50.343–87.885) | -5.780 (-21.033–1.868) | -5.131 (-11.463–-0.479) |
| egui | -2.851 (-13.461–11.395) | -2.884 (-6.901–7.411) | 3031 (3024–3038) | -9.388 (-44.413–37.607) | -9.873 (-15.460–1.250) | -5.995 (-12.076–0.348) |

This series' (c) reproduces original G4m's (c) process CPU within ranges for
every arm; this is what permits comparing the two series. The (b)−(a) ranges
span about ±12 to ±24 ms/s. A hook cost up to about **10 ms/s**, about
**100 µs per GPUI buffer**, is compatible with these data. These descriptive
five-run ranges are not confidence intervals.

The second table uses **paired per-round differences**, (b)−(a) for the hook
and (c)−(b) for power, rather than subtracting the first table's medians.
The per-buffer estimate divides each paired process CPU-rate difference by
(b)'s completed buffers/s and converts milliseconds to microseconds. All
three estimates span zero: these five rounds cannot resolve a positive hook
cost per buffer. Native's per-buffer cost is unavailable because its Core
Animation GPU work is outside the process. The 8,379/2,399/1,198 counts in G5m
are its bar workload; the denominator here is the observed fling count.
Power deltas describe the app's CPU, excluding the sampler's own process CPU.
Native and GPUI have negative process deltas in all five pairs; Ink and egui
have one positive pair each. Negative deltas are not a claimed optimization.

Predictions use the paired medians specified before measurement. A median
inside its predicted band does not establish a cost whose range crosses zero.

| Arm | Predicted hook CPU, ms/s → result | Predicted power CPU, ms/s → result |
|---|---:|---:|
| Native | -5 to +5 → -2.140, **met** | +0 to +5 → -10.285, **miss** |
| GPUI | +1 to +10 → +3.511, **met** | +0 to +5 → -11.023, **miss** |
| Ink | +1 to +10 → +2.896, **met** | +0 to +5 → -5.780, **miss** |
| egui | +5 to +20 → -2.851, **miss** | +0 to +5 → -9.873, **miss** |

The cadence prediction was at most 5% change, and peak footprint at most 10%,
for each comparison's medians. **All meet** those bounds:

| Arm | Frames/s change, b−a; c−b | Peak footprint change, b−a; c−b |
|---|---:|---:|
| Native | — | +1.231%; +1.450% |
| GPUI | +0.010%; -0.012% | -1.102%; -1.477% |
| Ink | -0.001%; -0.307% | +0.293%; -1.556% |
| egui | +0.004%; +0.211% | -0.325%; +0.305% |

**Scoreboard CPU baseline: complete configuration (a), hook off and power off.**
Taken under that background; re-take when the machine is quiet. These tables
are the complete `aggregate.mjs` output for (a), including rest, per-frame CPU,
footprint and cadence. Their five readiness launches are not a new cold-only
series. GPU rows are unavailable with the hook off. The old probes did not
record instructions or cycles; no values are reconstructed for them.

GPU hook: off (gpuHookEnabled=false).

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Cold to first complete frame, ms; native snapshot / painted drawable | 294.89 (265.26–673.36) | 251.31 (215.31–398.57) | 250.41 (212.51–636.38) | 236.56 (202.63–636.67) |
| Fling process CPU, ms/s | 174.87 (167.55–188.56) | 156.13 (136.13–162.86) | 128.00 (105.84–133.51) | 119.76 (105.58–123.56) |
| Fling process CPU per presented frame, ms | — | 1.751 (1.525–1.833) | 3.880 (3.188–4.022) | 2.870 (2.526–2.963) |
| Fling main-thread CPU, ms/s | 118.63 (114.85–120.12) | 78.98 (68.79–83.15) | 63.95 (56.19–67.87) | 56.49 (49.97–58.43) |
| Rest process CPU, ms/s | 2.217 (1.928–2.337) | 5.374 (5.178–5.440) | 2.436 (2.401–2.517) | 0.593 (0.534–0.640) |
| Rest main-thread CPU, ms/s | 1.107 (0.965–1.181) | 3.442 (3.304–3.494) | 2.310 (2.256–2.409) | 0.490 (0.435–0.557) |
| Peak footprint during fling, MiB | 230.92 (209.39–239.33) | 418.17 (412.63–431.42) | 602.66 (592.97–604.69) | 514.19 (508.05–516.11) |
| End-of-fling footprint, MiB | 215.83 (185.92–220.44) | 391.81 (390.55–403.33) | 247.92 (245.78–249.17) | 189.09 (185.38–192.45) |
| End-of-rest footprint, MiB | 215.13 (185.22–219.75) | 209.13 (207.39–219.16) | 240.13 (238.52–241.09) | 180.95 (177.20–184.27) |

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Fling interval p50, ms | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) |
| Fling interval p95, ms | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) |
| Fling interval p99, ms | 16.67 (16.67–16.67) | 8.33 (8.33–16.67) | 16.67 (16.67–16.67) | 8.33 (8.33–8.33) |
| Late intervals/s | 3.300 (1.700–4.099) | 1.053 (0.790–1.580) | 1.608 (1.580–1.845) | 0.268 (0.000–0.536) |
| Gesture-window duration, s | 3.868 (3.792–3.945) | 3.799 (3.729–3.863) | 3.794 (3.732–3.798) | 3.799 (3.731–3.861) |
| Presented frames/s inside gesture windows | — | 107.39 (107.00–108.21) | 86.99 (86.92–88.96) | 104.47 (103.85–105.00) |
| Presented frames/s over the full fling phase | — | 89.16 (88.87–89.29) | 33.00 (32.99–33.20) | 41.70 (41.49–41.79) |
| Inputs received / posted per sample | 8/424 (8–8/424) | 421/424 (417–423/424) | 423/424 (422–424/424) | 419/424 (415–419/424) |
| Driver maxGapMs; maximum of four gestures | 16.500 (9.346–17.369) | 16.389 (9.341–17.038) | 9.343 (9.336–16.646) | 16.307 (9.341–16.704) |
| Driver empty callbacks per sample | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Fling GPU busy time, ms/s; process command buffers | — | — | — | — |
| Fling GPU ms/presented frame | — | — | — | — |
| Fling completed command buffers per sample | — | — | — | — |

**The G4→G4m rise remains unattributed; the queue hook is excluded as its
explanation.** Hook-off process/main CPU still exceeds G4's ranges for every
arm. The runs used two physical Macs of the same model (Mac17,6) and OS
(27.0.1, 26A434). Other differences in the record are the silver driver's
pacing (gesture windows 3.73–3.80 s against about 4.00 s, maxGapMs 9–17 against
16–25, received 421–424 against 410–417), the background above, the login
session, `launchctl asuser`, the user switch and `caffeinate`. The wheel
helper's source is identical between G4 (`994b003388`) and G4m (`42d5066572`);
its independently rebuilt binary is not established byte-identical. The pacing
difference belongs to the machine/session conditions, not a helper source change.
For Ink alone this comparison uses `c22ef19622` on `f069d37722`, rather than
original G4m's `42d5066572`: the intervening `exact-ink` commits are
`15dc7c4aee` (trace unshown retries), `4118630a2b` (keep retrying a retained
launch until shown) and `f069d37722` (record the thirty displayed launches).
The decisive test is the same (a) binaries interleaved on both Macs within one
day, with the background stopped. It is not run in this chunk.

Raw files, all attempts, quiet observations, five power plists, schedule,
commands, predictions, binaries and hashes remain on the silver Mac under
`target/g0/hook-c22ef19622/`. The development copy of the records is
`target/g4m-hook/silver/`; original G4m records are also copied under
`target/g4m-hook/historical/g4m/`. `source.bundle` retains the measured patch;
`prepare.py`, `run-hook.py`, `verify-code.py` and `analyze.py` retain its
preparation, execution and derivation. No cited prior evidence was replaced.
`analysis.json` includes every paired difference and prediction disposition.
The committed scripts beside `aggregate.mjs` are `analyze-hook.py`,
`hook-plan.txt`, `run-hook.py`, `verify-hook-code.py` and `report-hook.py`.
The report script emits tables and checks their presence in this record; it
never rewrites historical cells. Reproduce here from the retained raw copies:

```sh
python3 bench/heavy-list/gpui/analyze-hook.py target/g4m-hook/silver/series-run target/g4m-hook/historical/g4m --out target/g4m-hook-review/analysis
python3 bench/heavy-list/gpui/report-hook.py target/g4m-hook/silver/series-run target/g4m-hook-review/analysis --verify
bun bench/heavy-list/gpui/aggregate.mjs target/g4m-hook/historical/g4m --verify
```

On the silver Mac the same committed commands take
`target/g0/hook-c22ef19622/series-run` and `target/g0/run-42d5066572/g4m` as
inputs, with a fresh output directory. `analyze-hook.py` checks five samples per
arm/configuration, hook state, focus, thermal state, phase durations, old quiet
coverage and completed GPU counts. `report-hook.py` reads the retained power
plists and schedule for the background table. Its generated output and the
complete (a) resources/cadence tables reproduce from raw files.

The probe and driver default to hook-off for fling, bar, caret and cold-only.
The probe reads `G1_GPU_HOOK=0|1` once at load; only exactly `1` enables it.
The driver rejects an empty or invalid value before any launch, naming the
variable. Accepted results carry observed `gpuHookEnabled` and requested
`requestedGpuHookEnabled`; startup/focus rejections carry the requested value
without claiming to have observed the probe. The aggregator treats a missing
observed field as on for pre-switch probes, carries it into JSON and Markdown,
refuses mixed tables, checks completed GPU work only when on, and prints GPU
rows as `—` when off while checking zero observed buffers and empty class names.
The drawable hook remains enabled in every configuration.

**Scoreboard instrument rule:** CPU rows come from hook-off, power-off runs;
GPU rows from hook-on, power-off runs. G5m's bar/caret CPU rows remain (c),
and its GPU rows remain power-on, until hook-off and power-off static series
replace them respectively. Power is measured in its own run and never joined
to a CPU row. LLP 1149.001 §4 and the README state the same rule.

All six development-Mac checks pass on `c22ef19622`: all-target build,
`cargo test --tests --no-fail-fast`, Clippy plus fmt, staged caps, boot and
the workspace all-target check. With `RUST_TEST_THREADS=2`, the test command
reports **7,384 passed, zero failed, 83 ignored** across 113 result summaries.
Commands, exit codes and complete output are in `target/g4m-hook/checks/`;
`suite-results.txt` retains every result line verbatim. No check needed a retry.
The probe and driver compile with `-Wall -Wextra -Werror`. The probe's real
Metal check runs with the hook off, on and unset; invalid input exits 2.
Its enabled and disabled outputs are:

```text
probe: painter-specific completeness and one boot-expose wake passed
probe: GPU hook off; three buffers completed, commit-phase accounting passed
probe: painter-specific completeness and one boot-expose wake passed
probe: GPU hook on; three buffers completed, commit-phase accounting passed
```

The branch was rebased onto `origin/main` `43e0d93b85`. That update changes
pointer-focus handling, not Rust sources or this instrument. Post-rebase
build, Clippy, fmt, caps, boot and workspace check all pass; output is retained in
`target/g4m-hook/checks-rebased/`; the full test run above is on the measured
source. The measurement series was not repeated after the rebase.


### Both reviews folded: instrument verification, 2026-10-30

The local real-Metal self-test passes with `G1_GPU_HOOK=0`, `1` and unset;
unset reports off. Invalid and empty values exit 2 in the probe and exit 1 in
the driver before any launch, naming `G1_GPU_HOOK`. Probe and driver compile
with `-Wall -Wextra -Werror`. Verbatim successful outputs, with the off pair
also repeated for unset:

```text
probe: painter-specific completeness and one boot-expose wake passed
probe: GPU hook off; three buffers completed, commit-phase accounting passed
probe: painter-specific completeness and one boot-expose wake passed
probe: GPU hook on; three buffers completed, commit-phase accounting passed
```

Aggregation runs over all retained (a), (b), (c) and original G4m samples,
printing off/on/on/on respectively. Original G4m's two tables and G5m's three
tables reproduce exactly. A deliberately mixed table is refused with
`mixed GPU hook states; aggregate separate configurations`. A hook-off static
fixture derived from the G5m files exercises fling, bar and caret: it retains
CPU rows and prints every GPU row as `—`. That transformed fixture is a test,
not measured evidence. `report-hook.py --verify` confirms the complete (a)
tables, paired comparison, prediction dispositions, bounds and background table.
`verify-hook-code.py` confirms the retained native/GPUI/egui code identity.

**One silver-Mac instrument sample, no series.** The copied Ink bundle uses
the new probe and `measure.m`, with `G1_GPU_HOOK` unset and default fling settings.
The single sample is accepted, hook-off and power-off, with zero observed GPU
buffers. The before-run prediction in `hook-plan.txt` is met: positive instruction
and cycle deltas in both phases, and busy cores plus five non-benchmark process
records in every quiet observation. `aggregate.mjs --sample` prints:

GPU hook: off (gpuHookEnabled=false). Instrument check only; not a scoreboard table.

| Phase | Seconds | Process CPU, ms | Main CPU, ms | Instructions | Cycles |
|---|---:|---:|---:|---:|---:|
| fling | 10.001313 | 1347.523 | 664.744 | 5330508726 | 2957935869 |
| rest | 10.003690 | 24.209 | 22.827 | 39854363 | 45864163 |

All **six quiet observations fail** the proposed bounds: **4.23–5.90 busy
cores** and **1.08–2.81 cores** in the largest non-benchmark process. Power
metadata records **AC Power**, battery 100%, and `powermode 0`. No process was
stopped and no performance table uses this sample. The probe, driver and monitor source hashes used for this sample are retained
in `single-provenance.json`. The sample predates the integration of Stage 0 at
`942b409559`; that integration preserves the instruction/cycle fields alongside
Stage 0's counters and trace diagnostics. No second silver sample was run.
Raw sample, process logs, schedule, five-second observations and power mode/source
remain on the silver Mac at
`target/g0/hook-review-20261030/single/`, and here at
`target/g4m-hook-review/single/`. Instrument checks and generated fixtures remain
in `target/g4m-hook-review/`; nothing under a historical evidence path was removed.
Reproduce the sample's columns without a launch:

```sh
bun bench/heavy-list/gpui/aggregate.mjs target/g4m-hook-review/single/sample/ink-1.json --sample
```

**Integrated verification.** Rebased onto Stage 0 at `origin/main`
`942b409559`. Its record precedes this one; its probe counters, trace diagnostics
and endpoint samples remain alongside the hook switch and instruction/cycle
fields. The integrated probe and driver compile with `-Wall -Wextra -Werror`.
Off, on, unset, invalid and empty checks pass again; the driver rejects invalid
and empty settings before launch. Output is retained in
`target/g4m-hook-review/probe-integrated/verification.json`.
Both the complete hook report and the original G4m tables reproduce after
integration (`report-integrated-verify.log`, `original-g4m-integrated.verify.txt`).

| Check on the integrated source | Exit | Qualification |
|---|---:|---|
| `bun install --frozen-lockfile` | 0 | Pinned Bun 1.4.2 |
| `cargo build --all-targets --keep-going` | 0 | Worktree's own `target/` |
| `cargo test --tests --no-fail-fast` | 101 | One underlying assertion and its suite wrapper fail; quiet condition unmet |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | |
| `cargo fmt --all -- --check` | 0 | Includes a formatting-only correction to the inherited Linux focus test |
| `bun scripts/caps.mjs` | 0 | Staged; all ten categories within cap |
| `bun scripts/boot.mjs` | 0 | |
| `cargo check --workspace --all-targets --keep-going` | 0 | Pinned Bun on PATH |

The two completed pre-integration test runs exit zero. Other lanes compiled
during each, so neither qualifies as quiet. A third start was interrupted to
integrate Stage 0; the final integrated run is the third completed attempt,
with `RUST_TEST_THREADS=2`. Its preflight passes, but other lanes compile during
the run. No further retry is made. Across its 113 result summaries, including
the nested executor suite, it reports **7,382 passed, two failures and 83
ignored**. The two failures are the same underlying quit assertion and the
wrapper that checks its child suite. The assertion passes on the ordinary
core and fails on the asynchronous core. Those test sources are unchanged by
this branch and the Stage 0 integration; background compilation was observed,
but its causal role in the failure is not established. Verbatim excerpts:

```text
thread 'executor::core::process::real::tests::a_quit_ends_every_group' (356547277) panicked at host/apple/src/executor_process_real_tests.rs:306:5:
the quit waited out the grace
test result: FAILED. 168 passed; 1 failed; 15 ignored; 0 measured; 398 filtered out; finished in 5.36s
thread 'executor::core::async_tests::the_suite_passes_on_the_asynchronous_core' (356545811) panicked at host/apple/src/executor_async_tests.rs:415:5:
the executor's suite failed on the asynchronous core
test result: FAILED. 563 passed; 1 failed; 18 ignored; 0 measured; 0 filtered out; finished in 15.94s
error: 1 target failed:
    `-p exact-apple --lib`
```

Complete command output, exits, five-second quiet observations and all suite
result lines verbatim remain in `target/g4m-hook-review/checks-integrated/`
(`final-results.json`, `test-3.log`, `test-3-quiet.jsonl`, `suite-results.txt`).
Earlier attempts, the interrupted start and the initial formatting failure
remain in `target/g4m-hook-review/checks/`. The final source is not qualified
by a passing quiet test run. The quiet silver CPU baseline, power-off static
GPU series and interleaved two-Mac comparison also remain unrun, as required
by this chunk's one-sample limit.

## G5m, after M2b

### Attribution and prediction, before the change

The headless heavy-list caret drive on `5fa5fcc2f3` painted all 120 display
periods after its starting sample, while opacity changed only twice, at
750 and 1,250 ms. The presenter wanted another display frame after each paint. This is presenter
scheduling, not a different animation value: `Engine::advance_animations`
marks every sampled property of a live animation dirty, `Host::present`
accepts a repeated value once the node has presentation state, and
`Presenter::tick` marks the frame dirty while `Host::motion()` is true.
`needs_animation_frame` uses that same lifetime test. The carrier consequently
walks and encodes another frame on every display callback.

The baseline headless run uses direct Metal and a readback target: it does
not acquire or present a window drawable. G5m's retained window probe above
supplies the drawable evidence (119.74 presents/s). Neither the lane's starting main
commit nor the measured revision has an `EXACT_INK_COUNTERS` implementation.
Setting it supplies no counter evidence, so per-period instance re-emission
and encoding counters could not be collected. The initial trace and its reduced rows are retained in
`target/m2b/before-periods-2.jsonl` (complete), `before-periods.jsonl` (the
initial eight-period attempt) and `before-periods-summary.json`; its scratch
entry and build commands are under `target/m2b/trace/` and `trace-build*.log`.
The `scripts/agent.mjs` drive is in `target/m2b/before-agent-2.jsonl`;
`m2b-trace-before` retains the baseline executable.

Prediction: on the same three-round G5m driver and probe, Ink caret falls to
2 presents/s and below 5 process CPU ms/s. Near-zero GPU means at most 5 GPU
ms/s in the probe and 0.03 W median whole-machine GPU power in powermetrics.
Ink bar and the native arm remain within G5m's three-round CPU and frame-rate
ranges. The headless 120-period trace should paint only at the two half-second
steps. Each prediction is judged on its named instrument, including misses.

### Three quiet rounds on the silver Mac

2026-10-30, source `41b339e758`, rebased onto main `43e0d93b85`. All four
arms were rebuilt from this lane and ad-hoc signed with the same probe. The G5m static driver ran three rotating
rounds of fling, bar and caret: 36 accepted samples, no exclusions or focus
losses, and all 114 five-second observations quiet. Powermetrics collected
534 samples. The qualification capture shows the complete heavy list.

The sampler now supplies a deadline for steps, delays and phase edges, or
continuous demand; paused and finished plays supply none. The presenter
compares samples with its stored presentation before dirtying a kept row or
requesting a frame. The agent's seek stops at discrete animation deadlines.
The native lowerer and sampled values are unchanged.

| Prediction and instrument | Measured after, median (range) | Result |
|---|---|---|
| Headless caret: two paints in 120 display periods after its starting frame | 2 paints; 2 opacity changes; no continuous display demand | Met |
| Probe: caret 2 presents/s | 2.00 (2.00–2.00) | Met |
| Probe: caret process CPU below 5 ms/s | 27.148 (25.611–27.577), from G5m's 154.320 (149.241–159.467) | Miss |
| Probe: caret GPU work at most 5 ms/s | 4.349 (4.347–4.359), from 212.438 (159.497–260.022) | Met |
| Powermetrics: caret whole-machine GPU power at most 0.03 W | 0.019 (0.016–0.022), from 0.718 (0.708–0.753) | Met |
| Probe: bar CPU and cadence stay in G5m's ranges | 157.413 CPU ms/s; 119.75 presents/s | Medians are inside; the highest cadence sample, 119.940, exceeds the old 119.865 upper edge, so the literal range prediction missed |
| Probe: native CPU and cadence stay in G5m's ranges | Caret 12.255 CPU ms/s, 119.98 display ticks/s; bar 89.749 CPU ms/s, 119.96 ticks/s | Miss: caret CPU is above the old 12.217 upper edge; bar CPU is below its old 90.519 lower edge. Cadence medians are inside |

The caret's 40 completed command buffers per sample replace G5m's
2,399–2,400. Its remaining CPU is mostly the main thread (24.661 ms/s);
the under-5 target remains unmet. The continuous bar still presents at the
display rate; its CPU range is inside G5m's. Native CPU differs from G5m's
medians by +0.079 ms/s for caret and −2.816 ms/s for bar. The literal range
prediction is not met, even though the bar difference is a reduction.
Native cadence is the probe's display link, not drawable presentation; its
Core Animation work is not in the process command-buffer GPU rows.

The under-5 ms/s prediction used a workload with timers: the benchmark's
`every(250, tick)` and `every(1000, second)` fire five times per second,
each requiring a carrier service pass. The display link also starts and
stops twice per second. Counting these as seven wake causes against the
24.661 main-thread ms/s suggests about 3 ms per turn, but is not a trace
attribution: timer firings can share a wake. M2's rest trace must separate
service work, link start/stop and the probe. This workload is not at rest
in M2's sense; `QUEUE.md` names that remaining attribution.

The unchanged native path's literal range misses remain misses. Three samples
do not define a stable tolerance. For subsequent unchanged-path comparisons,
use ±5% of the baseline CPU median and ±1 frame/s, with the full spread retained.
The native deviations above fit that tolerance; this does not revise the
original prediction or establish an instrument defect.

Known scheduling limits: flat segments of `linear()` still demand every
display frame; continuous easing is never classified as stepped. A `jump-none`
iteration edge can wake without changing pixels, and `Property::D` restated
by an unrelated turn still dirties its row. Neither changes the opacity-caret
counts. Seconds-to-milliseconds rounding can turn a just-later reverse edge
into current-frame demand; the carrier samples on the next display period.

### Bar, after M2b

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Quiet samples | 3 | 3 | 3 | 3 |
| Process CPU, ms/s | 89.749 (88.419–90.048) | 146.498 (140.512–149.985) | 157.413 (152.553–160.141) | 163.823 (163.569–165.908) |
| Main-thread CPU, ms/s | 81.772 (80.648–81.991) | 93.614 (90.811–95.351) | 73.475 (72.507–76.914) | 81.107 (80.662–81.390) |
| Frames/s; native display link / painted drawables | 119.96 (119.88–119.98) | 119.70 (119.69–119.72) | 119.75 (119.66–119.94) | 119.90 (119.80–120.00) |
| Peak footprint, MiB | 126.53 (125.28–126.67) | 303.22 (301.20–303.58) | 356.08 (347.05–362.30) | 444.56 (441.17–449.39) |
| End footprint, MiB | 126.05 (124.89–126.17) | 302.80 (301.20–303.13) | 350.78 (347.05–354.67) | 444.31 (440.80–448.89) |
| GPU busy time, ms/s; process command buffers | — | 32.435 (32.426–32.479) | 191.163 (155.103–204.514) | 23.154 (23.018–23.212) |
| GPU ms/frame; native cadence / painted drawables | — | 0.2709 (0.2709–0.2713) | 1.5964 (1.2962–1.7051) | 0.1930 (0.1921–0.1936) |
| Completed command buffers per sample | 0 (0–0) | 1198 (1198–1200) | 2399 (2399–2402) | 8400 (8393–8407) |

### Caret, after M2b

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Quiet samples | 3 | 3 | 3 | 3 |
| Process CPU, ms/s | 12.255 (11.719–12.369) | 17.977 (17.939–18.224) | 27.148 (25.611–27.577) | 14.965 (14.827–15.316) |
| Main-thread CPU, ms/s | 11.271 (10.741–11.371) | 14.633 (14.502–14.832) | 24.661 (23.512–24.823) | 12.730 (12.704–12.855) |
| Frames/s; native display link / painted drawables | 119.98 (119.89–120.06) | 2.00 (2.00–2.00) | 2.00 (2.00–2.00) | 2.00 (2.00–2.00) |
| Peak footprint, MiB | 127.95 (127.80–137.75) | 292.64 (292.56–293.59) | 355.94 (355.78–356.61) | 443.25 (432.63–443.44) |
| End footprint, MiB | 127.72 (127.53–137.47) | 292.14 (292.14–293.09) | 352.88 (352.75–353.27) | 442.83 (432.44–443.30) |
| GPU busy time, ms/s; process command buffers | — | 0.563 (0.561–0.567) | 4.349 (4.347–4.359) | 0.377 (0.371–0.377) |
| GPU ms/frame; native cadence / painted drawables | — | 0.2816 (0.2808–0.2836) | 2.1756 (2.1744–2.1799) | 0.1885 (0.1857–0.1887) |
| Completed command buffers per sample | 0 (0–0) | 20 (20–20) | 40 (40–40) | 140 (140–140) |

| Whole-machine GPU power, W; median (range) | Native | GPUI | Ink | egui |
|---|---:|---:|---:|---:|
| Bar | 0.024 (0.022–0.027) | 0.284 (0.268–0.288) | 0.782 (0.768–0.784) | 0.182 (0.179–0.192) |
| Caret | 0.002 (0.001–0.002) | 0.015 (0.007–0.016) | 0.019 (0.016–0.022) | 0.005 (0.005–0.007) |

These are powermetrics estimates for the whole machine, not process watts.
The process GPU rows above sum the probe's completed command-buffer spans.

### Reproduction and retained evidence

In this lane's worktree, `target/m2b/silver-run-41b339e758/` retains the signed
bundles, build and qualification logs, provenance hashes, driver, aggregator
outputs, captures, all raw attempts, quiet observations and power samples.
The measurement Mac retains the same files at
`~/projects/exact2-measure/target/g0/m2b-41b339e758/`. G5m's before cohort is
also copied to `target/m2b/g5m-before/`. The preliminary cohort before the
rebase is retained at `target/m2b/silver-run-4eea9b7d90/` and the measurement
Mac's `target/g0/m2b-4eea9b7d90/`. No retained cohort was replaced.

```sh
bun bench/heavy-list/gpui/aggregate.mjs target/m2b/silver-run-41b339e758/g5m-after
python3 target/m2b/silver-run-41b339e758/power-summary.py target/m2b/silver-run-41b339e758/g5m-after
```

`target/m2b/after-periods.jsonl` repeats the baseline headless instrument:
two dirty frames after the starting sample, at 750 and 1,250 ms, and none
between. `after-agent.jsonl` and `after.png` retain the app drive, including
`clock settle` leaving the infinite caret at 750 ms. `after-on-agent.log` and
`after-on.png` show the caret again at 1,250 ms. The presenter tests cover
one frame per step, no intervening frames, paused and delayed plays, the final
sample, and continuous motion. The motion tests cover all four step positions,
reverse boundaries and lowered plays.

### Validation

At `41b339e758`, all six checks passed, including the full root test run and
workspace compilation. The full `exact-linux` library and test suites,
`exact-motion`, and the painted carrier's five Rust tests passed. The pinned
`paint`, `borders`, `tint`, `backdrop`, `gradients`, `visual` and `materials`
filters, and the integration `image` filter, each passed with
`EXACT_INK_DEVICE=metal EXACT_REQUIRED=1`. The kernel, runner and native
sampler protocol did not change. The Apple Rust tests ran in the root suite;
Swift XCTests were not run.

`target/m2b/validation.txt` contains the commands and their complete verbatim
output; `validation-summary.txt` contains the result lines. The final six
checks are under `target/m2b/final-validation/`; the additional suites are
under `target/m2b/integrated/`. Earlier root runs hit the compiler-growth and
SVG wall-clock assertions, and an earlier Linux run hit upload and image
waits. Their logs and isolated reruns remain under `target/m2b/`; the final
full root and Linux runs passed without changing those tests.

The retained binary and probe hashes match their provenance. Recomputed
process and power tables match the published tables; Python versions differ
in the power JSON's floating-point sums by at most 1.8e-13. The original JSON
and the recomputation are both retained. `target/m2b/integrated/reproduction.log`
records these checks.


### M2b review fixes and integrated verification

The review fixes are integrated at `d15f40e145`, on Stage 0's `942b409559`.
Android and the Canvas reader include sampled motion in their wake deadline
and advance the host clock before reading demand. Their production clock turns
are driven headlessly with a real presenter and no runner timer. Windows uses
current-frame demand at all four lifetime checks; its three after samples per
device each redraw twice per second. See
[M2b-Windows](../../../host/ink/MEASUREMENTS.md#m2b-windows--step-demand) for
the before counts, instruments and limits.

The filter compares against the kept entry before hover, focus and press
feedback. The painted hover regression passes. Agent seeks exceeding 4,096
discrete instants return the named limit error and can resume. The tighter
motion tests check immediately before each deadline and cover holds,
alternate repeats, retargeting, competing deadlines and a finite final change.
They exposed a reverse boundary losing an ULP when adding its iteration;
crossing the boundary after converting it to time fixes that case. Flat
`linear()` spans remain continuous demand deliberately.

All six integrated Mac checks pass, including Clippy and fmt, staged caps,
boot and workspace compilation. The root command emits 113 passing result
summaries: 7,390 passes and 83 ignored, including nested child reruns. The
explicit Linux command emits 2,123 passes and 38 ignored on the same counting
basis; motion has 213 passes and the painted carrier has five. Root tests
started at a quiet observation (load 26.9, no competing compiler); no test
or timing threshold was relaxed. All 46 Windows suite and journey commands
pass, as recorded in M2b-Windows.

`target/m2b-fix/` retains predictions, commands, individual logs,
`CHECKS.txt` (every result line), `FULL-OUTPUT.txt` (verbatim output), the
rebase range diff and native evidence under `nucbox/`. The first complete
Mac pass is retained separately in `pre-stage0/`. Both RESULTS records and
both lanes' QUEUE items survived the rebase. No retained raw file was deleted.
No silver-Mac series was run in this fix chunk. Android's physical carrier
was not run; the clock fragments used by both Android loops are covered by
the headless tests. The timer-workload CPU attribution remains queued.


## G4q, the quiet baselines

**Predictions recorded before any series, 2026-10-30.** Measure source
`d5429830d2` on the silver Mac, using the landed probe and driver. Native,
GPUI and egui retain their `42d5066572` application binaries; Ink and the
probe are rebuilt. The control binaries must match before their signatures.
CPU uses hook-off and power-off; GPU uses hook-on and power-off; power runs
separately. Every series requires sixty seconds of five-second observations
passing ≤2 busy cores and ≤0.25 core for the largest non-benchmark process,
plus the monitor's load/compiler conditions. Retain every failed observation
and every rejected attempt. No background process is stopped by this lane.

The fling prediction tests whether removing the recorded background returns
the silver Mac to G4's development-Mac ranges. Values below are predicted
medians, measured by the same ten-second process-CPU endpoints. They are
hypotheses, not acceptance bounds for selecting samples.

| Arm | Predicted quiet fling CPU, ms/s | Under-background (a), ms/s | G4, ms/s |
|---|---:|---:|---:|
| Native | 112.39–143.12 | 174.87 | 131.91 |
| GPUI | 82.05–89.05 | 156.13 | 84.92 |
| Ink | 84.98–87.07 | 128.00 | 86.41 |
| egui | 64.93–71.54 | 119.76 | 66.84 |

**Instructions:** Ink's median instructions over the full fling phase are
predicted within **2% of 5,330,508,726**, the single hook-off sample above,
using `ri_instructions` endpoint differences. Only Ink has that earlier
counter sample; no cross-series instruction prediction is testable for the
other arms. Report cycles and CPU beside instructions without predicting a
fixed clock. Stage 0's integrated counters are an instrument change, so a
miss cannot be attributed solely to background work.

**Cold:** ten cold-only launches per arm, retained application and driver
caches, with the hook off. The native snapshot and painted drawable retain
their different endpoints. Predicted p50 milliseconds are below G4m's
hook-on/power-on cold medians, with background and hook state both changed;
this does not isolate the hook's startup cost.

| Arm | Predicted hook-off cold p50, ms | G4m hook-on cold p50, ms |
|---|---:|---:|
| Native | 200–250 | 277.67 |
| GPUI | 180–220 | 219.07 |
| Ink | 180–230 | 249.78 |
| egui | 180–220 | 234.43 |

**Static:** five interleaved rounds each of bar and caret, read from the
probe's ten-second CPU endpoints. Predictions retain main's continuous Ink
caret rendering; M2b is excluded from the measured source. Cadence comes
from presented drawables for painted arms and the display link for native.

| Arm | Predicted bar process CPU, ms/s | Predicted caret process CPU, ms/s | Predicted bar / caret cadence, Hz |
|---|---:|---:|---:|
| Native | 60–95 | 6–15 | 118–121 / 118–121; display link only |
| GPUI | 100–160 | 10–22 | 118–121 / 1.9–2.1 |
| Ink | 100–170 | 100–170 | 118–121 / 118–121 |
| egui | 110–180 | 8–20 | 118–121 / 1.9–2.1 |

For the separate hook-on static runs, predicted Ink GPU time is **1.25–2.5
ms per presented frame** in both workloads, above 15% of an 8.33 ms period;
GPUI and egui are predicted below **0.417 ms/frame** (5%). The instrument
sums command-buffer spans at the clock the load earns, not device occupancy.
Native's GPU work remains unavailable. These are before-M3 rows and cannot
earn M8. One fling round and one bar round with power sampling report power
separately; no numerical power saving is predicted.

**Stage 0b:** the same boot-only probe and trace as chunk 4, hook off,
predict 30/30 complete Ink screens across ten retained Metal, ten interleaved
wgpu controls and ten archive misses. Predict retained Metal's phase p50
≤8 ms and within 2 ms of 6.581 ms; complete Ink p50 ≤wgpu +5 ms. Misses
predict zero boot-thread compilation, CPU first pixel p50 ≤300 ms and
complete Ink p50 ≤450 ms. No 100 ms application-budget pass is predicted.

**Preparation.** The silver worktree is detached at `d5429830d2`. Only Ink,
the probe and the driver were rebuilt; the wheel helper and the three control
applications were copied from `target/g0/run-42d5066572/`. All 105 data hashes
and the GPUI/egui bundled copies pass. All four bundles pass strict signature
verification. `verify-hook-code.py` confirms that native, GPUI and egui are
byte-identical to their originals before the code signatures. The probe and
driver compile with `-Wall -Wextra -Werror`.

| Prepared artifact | SHA-256 |
|---|---|
| Ink executable | `12d3baf93c7298942b9858c36a441c24eada8b19957894b67a71774f8ff56a15` |
| Shared probe | `2a83c0c93fa1ede456e7144a2bb33584e0deb8b3bb2970e53cce19722fba9ab2` |
| Driver | `7ef5c8053383c5f84fd3ba9cda9f728c03aa83368ca995bb97aabcfad45235e1` |

The signed bundles, commands, build logs, source bundle, predictions and hashes
are retained on silver under `target/g0/quiet-d5429830d2/` (source bundle:
`target/g0/quiet-source-d5429830d2.bundle`). This worktree retains the preparation
records and instrument binaries under
`target/g4q/silver/target/g0/quiet-d5429830d2/`. Predictions were committed as
`8ead227047` before the first gate began at **18:02:34 PDT**. The gate imports
the landed `QuietMonitor`; its wrapper samples every five seconds in bounded
one-minute loops, with a two-hour total deadline.
