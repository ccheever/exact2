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

**Clock qualification (G4q.1):** this series has no retained cycle counters.
Its clock is unavailable; it is not a savings baseline. Later low-clock samples
do not retroactively establish this series' cluster or clock.

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

**Clock qualification (G4q.1):** this series has no retained cycle counters.
Its clock is unavailable; it is not a savings baseline. The later single Ink
counter sample is 2.195 GHz during fling, and G4q's Rust arms run below 2 GHz.
Those observations do not retroactively establish this series' cluster or clock.

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

**Clock qualification (G4q.1):** the retained fling and rest counters show low
clocks. These CPU rows are excluded regardless of quiet qualification. The
later diagnostic locates Rust main threads on the M5 Max's lower Performance
cluster; this machine has no cluster named Efficiency. Under the amended quiet
rule, four cohorts pass; see G4q.1. The original outcome below uses the original
bounds and remains the record of that run.

**Outcome:** every prescribed cohort and its one retry exceeded the landed
quiet bounds. All twenty are retained and excluded; no scoreboard row is
replaced. The gates used 71 min 18 s of the two-hour wait allowance. The
G4→G4m attribution, a quiet Ink GPU share and the 100 ms application obligation
remain open. The tables below report failed cohorts and partial phases, not
quiet baselines.

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
one-minute loops, with a two-hour cumulative wait allowance.

The initial gate passed after **1,777.842 s (29 min 38 s)**: 351 observations,
339 failing before twelve consecutive passes. Whole-machine CPU ranged from
**0.645 to 4.417 busy cores**; Fleet/Claude reached **2.641 cores** and
`PerfPowerServices` **2.008 cores**. The final twelve observations ranged from
**0.645 to 1.715 busy cores**, with the largest process at **0.166 core**
(`fleet`). No process was stopped by this lane. The full gate, including its
power-mode record and result, is retained in `gate-fling-1/` beneath both raw
roots above. Every series starts a fresh sixty-second gate; subsequent waits
count toward the same two-hour allowance.

**Reading the monitor.** The monitor excludes the four application executables, the driver and the wheel
helper. WindowServer remains a non-benchmark process under this rule. A crossing
does not establish whether its work came from the benchmark or other windows.
Whole-machine busy cores use five-second Mach CPU-tick differences; `ps` reports
each process's recent CPU average. Their time windows differ, so their values
cannot be subtracted to attribute the load.

### Fling attempt 1 — excluded

`fling-a-1` completed all twenty launches, five interleaved rounds, hook off
and power off. Its fresh gate took **60.749 s**. The gate and series have
**102 observations, 18 failing**: whole-machine CPU is **0.640–1.816 cores**,
but the largest non-benchmark process reaches **2.460 cores** (Fleet/Claude).
`launchd` and `airportd` also cross the 0.25-core bound. The entire series is
excluded from the scoreboard. These tables reproduce the retained attempt;
they do not establish a quiet baseline. The startup row has five launches
per arm and does not replace the separate ten-launch cold-only cohort.

GPU hook: off (gpuHookEnabled=false).

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Cold to first complete frame, ms; native snapshot / painted drawable | 265.72 (261.14–646.75) | 223.15 (219.98–411.36) | 239.13 (210.47–617.54) | 231.93 (223.03–706.58) |
| Fling process CPU, ms/s | 167.11 (151.68–178.86) | 161.49 (157.12–166.47) | 138.52 (130.85–144.70) | 121.95 (119.40–124.15) |
| Fling process CPU per presented frame, ms | — | 1.821 (1.770–1.868) | 4.148 (3.943–4.346) | 2.939 (2.864–2.978) |
| Fling main-thread CPU, ms/s | 117.42 (111.22–120.98) | 83.20 (78.33–86.43) | 69.62 (66.13–71.96) | 57.58 (55.10–58.78) |
| Fling process instructions, million/s | 1247.899 (1041.508–1498.699) | 495.333 (491.563–499.232) | 510.133 (509.159–513.096) | 368.799 (367.443–370.094) |
| Fling process cycles, million/s | 451.260 (387.775–506.569) | 259.053 (254.781–265.029) | 243.136 (241.973–257.866) | 204.568 (200.864–208.400) |
| Fling process effective GHz | 2.700 (2.549–2.832) | 1.604 (1.592–1.640) | 1.755 (1.672–1.971) | 1.678 (1.659–1.707) |
| Fling process instructions/cycle | 2.741 (2.627–2.959) | 1.911 (1.875–1.929) | 2.096 (1.982–2.120) | 1.808 (1.770–1.829) |
| Rest process CPU, ms/s | 2.707 (2.585–2.830) | 4.994 (4.967–5.080) | 3.096 (2.998–3.220) | 0.777 (0.752–0.841) |
| Rest main-thread CPU, ms/s | 1.340 (1.274–1.410) | 3.113 (3.066–3.198) | 2.932 (2.845–3.060) | 0.658 (0.618–0.691) |
| Rest process instructions, million/s | 2.412 (2.265–2.434) | 8.000 (7.974–8.049) | 3.906 (3.873–4.099) | 1.037 (1.007–1.093) |
| Rest process cycles, million/s | 4.034 (3.903–4.127) | 6.898 (6.785–7.083) | 4.587 (4.512–4.893) | 1.106 (0.997–1.117) |
| Rest process effective GHz | 1.467 (1.404–1.596) | 1.376 (1.361–1.412) | 1.506 (1.444–1.545) | 1.365 (1.326–1.456) |
| Rest process instructions/cycle | 0.590 (0.561–0.619) | 1.160 (1.134–1.186) | 0.857 (0.808–0.860) | 0.979 (0.917–1.010) |
| Peak footprint during fling, MiB | 231.06 (230.19–240.45) | 410.83 (405.44–420.20) | 600.55 (599.30–605.70) | 514.88 (504.00–521.27) |
| End-of-fling footprint, MiB | 217.31 (214.16–229.72) | 396.16 (386.69–398.80) | 249.50 (248.05–250.63) | 187.59 (187.09–191.97) |
| End-of-rest footprint, MiB | 216.67 (213.67–228.99) | 210.64 (203.55–212.97) | 242.24 (240.74–243.28) | 179.44 (178.92–183.83) |

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Fling interval p50, ms | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) |
| Fling interval p95, ms | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) |
| Fling interval p99, ms | 8.33 (8.33–16.67) | 16.67 (16.67–16.67) | 16.67 (8.33–16.67) | 8.33 (8.33–8.33) |
| Late intervals/s | 1.100 (0.600–1.600) | 1.877 (1.318–2.412) | 1.072 (0.538–1.295) | 0.000 (0.000–0.000) |
| Gesture-window duration, s | 3.797 (3.730–3.929) | 3.730 (3.729–3.794) | 3.723 (3.720–3.860) | 3.795 (3.731–3.862) |
| Presented frames/s inside gesture windows | — | 105.35 (104.54–106.23) | 89.53 (86.02–89.74) | 104.33 (103.45–104.97) |
| Presented frames/s over the full fling phase | — | 88.78 (88.68–89.09) | 33.39 (33.19–33.40) | 41.60 (41.49–41.69) |
| Inputs received / posted per sample | 8/424 (8–8/424) | 416/424 (413–418/424) | 422/424 (421–423/424) | 416/424 (415–418/424) |
| Driver maxGapMs; maximum of four gestures | 9.386 (9.311–16.663) | 12.448 (9.331–14.194) | 9.352 (9.342–16.692) | 9.345 (9.336–16.436) |
| Driver empty callbacks per sample | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Fling GPU busy time, ms/s; process command buffers | — | — | — | — |
| Fling GPU ms/presented frame | — | — | — | — |
| Fling completed command buffers per sample | — | — | — | — |

### Fling attempt 2 — excluded

`fling-a-2` passed its gate after **182.160 s**. Its qualifying
minute and series contain **115 observations, 22 failing**, with
**0.633–1.769 busy cores** and Fleet/Claude reaching **2.077 cores**. All five
native, GPUI and Ink samples complete; egui has four. Both attempts at egui's
third launch time out waiting for startup readiness; its second launch also
times out, then succeeds on retry. All three timeouts remain retained. The
series is excluded in full; no third fling series is attempted.

GPU hook: off (gpuHookEnabled=false).

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Cold to first complete frame, ms; native snapshot / painted drawable | 260.92 (238.62–261.28) | 227.66 (218.53–231.90) | 236.53 (227.45–248.59) | 233.02 (218.79–248.43) |
| Fling process CPU, ms/s | 170.95 (159.22–183.68) | 148.07 (143.95–163.69) | 141.91 (137.03–146.59) | 121.39 (113.92–121.82) |
| Fling process CPU per presented frame, ms | — | 1.666 (1.623–1.831) | 4.275 (4.092–4.391) | 2.941 (2.733–2.947) |
| Fling main-thread CPU, ms/s | 117.12 (109.50–119.25) | 76.39 (74.90–84.65) | 70.88 (67.81–73.62) | 56.26 (54.02–57.02) |
| Fling process instructions, million/s | 1357.071 (1211.326–1627.046) | 495.702 (493.899–497.706) | 506.061 (499.267–526.356) | 367.956 (366.883–370.093) |
| Fling process cycles, million/s | 464.988 (417.105–552.464) | 271.152 (262.454–279.096) | 245.413 (240.284–260.082) | 205.842 (204.871–207.257) |
| Fling process effective GHz | 2.720 (2.589–3.008) | 1.863 (1.612–1.903) | 1.711 (1.682–1.806) | 1.695 (1.684–1.819) |
| Fling process instructions/cycle | 2.904 (2.791–2.945) | 1.828 (1.783–1.888) | 2.078 (1.967–2.127) | 1.786 (1.782–1.796) |
| Rest process CPU, ms/s | 2.808 (2.706–3.454) | 4.942 (4.847–5.353) | 3.170 (2.822–3.231) | 0.752 (0.745–0.826) |
| Rest main-thread CPU, ms/s | 1.366 (1.314–1.938) | 3.116 (3.046–3.389) | 3.021 (2.703–3.087) | 0.605 (0.597–0.674) |
| Rest process instructions, million/s | 2.423 (2.331–4.859) | 8.014 (7.941–8.331) | 4.030 (3.859–4.066) | 0.993 (0.962–1.118) |
| Rest process cycles, million/s | 4.202 (4.054–5.976) | 6.801 (6.554–7.522) | 4.541 (3.909–4.760) | 1.098 (1.027–1.128) |
| Rest process effective GHz | 1.521 (1.433–1.730) | 1.376 (1.352–1.405) | 1.411 (1.385–1.547) | 1.403 (1.351–1.514) |
| Rest process instructions/cycle | 0.585 (0.552–0.813) | 1.173 (1.108–1.217) | 0.889 (0.847–0.988) | 0.919 (0.897–1.003) |
| Peak footprint during fling, MiB | 234.66 (229.94–245.56) | 411.61 (401.75–419.14) | 599.31 (597.22–604.80) | 513.31 (510.02–517.83) |
| End-of-fling footprint, MiB | 218.31 (208.35–223.02) | 390.67 (388.38–401.50) | 249.39 (245.17–250.49) | 189.06 (183.09–194.45) |
| End-of-rest footprint, MiB | 217.66 (207.72–222.28) | 207.55 (205.20–218.33) | 242.06 (237.95–243.19) | 180.90 (174.89–186.22) |

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Fling interval p50, ms | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) |
| Fling interval p95, ms | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) |
| Fling interval p99, ms | 16.67 (16.67–16.67) | 16.67 (16.67–16.67) | 16.67 (8.33–16.67) | 8.33 (8.33–8.33) |
| Late intervals/s | 1.600 (1.500–2.400) | 1.847 (1.318–2.149) | 1.071 (0.791–1.296) | 0.134 (0.000–0.268) |
| Gesture-window duration, s | 3.799 (3.730–3.858) | 3.790 (3.723–3.798) | 3.794 (3.732–3.866) | 3.732 (3.728–3.797) |
| Presented frames/s inside gesture windows | — | 106.11 (105.84–107.29) | 87.52 (86.39–89.77) | 104.03 (102.54–104.61) |
| Presented frames/s over the full fling phase | — | 88.87 (88.69–89.39) | 33.39 (33.19–33.48) | 41.39 (41.10–41.69) |
| Inputs received / posted per sample | 8/424 (8–8/424) | 418/424 (415–420/424) | 420/424 (420–423/424) | 414/424 (410–418/424) |
| Driver maxGapMs; maximum of four gestures | 16.627 (9.336–16.833) | 10.200 (9.330–16.658) | 16.628 (9.333–16.944) | 9.334 (9.329–16.826) |
| Driver empty callbacks per sample | 0 (0–0) | 0 (0–0) | 0 (0–0) | 0 (0–0) |

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Fling GPU busy time, ms/s; process command buffers | — | — | — | — |
| Fling GPU ms/presented frame | — | — | — | — |
| Fling completed command buffers per sample | — | — | — | — |

### Cold attempt 1 — excluded

`cold-a-1` retains ten cold-only launches per arm, hook off and power off.
Its fresh gate passed in **60.711 s**, then **3 of 37** gate/series observations
failed. Whole-machine CPU ranges **0.653–2.269 cores**; WindowServer reaches
**0.320 core**. The entire cohort is excluded from the cold scoreboard.

GPU hook: off (gpuHookEnabled=false).

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Cold to first complete frame, ms; native snapshot / painted drawable | 263.04 (243.46–282.00) | 215.54 (195.20–231.52) | 240.08 (207.61–273.29) | 242.52 (204.43–263.59) |

### Cold attempt 2 — excluded

`cold-a-2` retains ten cold-only launches per arm, hook off and power off,
after a **151.809 s** gate. **8 of 37** qualifying-gate/series
observations fail: **0.686–2.434 busy cores**, with the system `deleted`
process reaching **0.779 core**. The entire cohort is excluded; neither cold
attempt replaces G4m's row. There are **1** rejected launch attempts.

GPU hook: off (gpuHookEnabled=false).

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Cold to first complete frame, ms; native snapshot / painted drawable | 275.13 (228.94–290.05) | 220.70 (206.95–249.07) | 254.81 (227.92–282.20) | 235.08 (198.14–256.84) |

### Fling and cold predictions beside the excluded results

Attempt 1 / attempt 2 are shown in that order. These comparisons report the
misses without qualifying the quiet-machine hypotheses: neither attempt
passes the monitor. No quiet CPU baseline or causal explanation follows from
these rows. The instruction reference has only Ink; its other-arm comparisons
remain unavailable.

| Prediction and instrument | Predicted | Observed, attempts 1 / 2 | Result |
|---|---|---|---|
| Fling native, ms/s | 112.39–143.12 | 167.11 / 170.95 | Both miss; both excluded |
| Fling gpui, ms/s | 82.05–89.05 | 161.49 / 148.07 | Both miss; both excluded |
| Fling ink, ms/s | 84.98–87.07 | 138.52 / 141.91 | Both miss; both excluded |
| Fling egui, ms/s | 64.93–71.54 | 121.95 / 121.39 | Both miss; both excluded |
| Ink full-phase instructions | 5,330,508,726 ±2% | 5,103,010,355 / 5,062,312,607 | Both miss: −4.268% / −5.031%; both excluded |
| Cold native, ms; G4m 277.67 | 200–250, below G4m | 263.04 / 275.13; Δ -14.63 / -2.54 | Miss / Miss against band; both excluded |
| Cold gpui, ms; G4m 219.07 | 180–220, below G4m | 215.54 / 220.70; Δ -3.53 / +1.63 | Met / Miss against band; both excluded |
| Cold ink, ms; G4m 249.78 | 180–230, below G4m | 240.08 / 254.81; Δ -9.70 / +5.03 | Miss / Miss against band; both excluded |
| Cold egui, ms; G4m 234.43 | 180–220, below G4m | 242.52 / 235.08; Δ +8.09 / +0.65 | Miss / Miss against band; both excluded |

### Bar CPU attempt 1 — excluded

`bar-a-1` retains five rounds of all four arms, hook off and power off, after
its **202.523 s** gate. **45 of 63** qualifying-gate/series
observations fail: **0.610–2.643 busy cores**, with `biomesyncd` reaching
**0.859 core**. Every timed sample overlaps a failing observation, so the
landed static filter reports no quiet samples. The whole series is excluded.
The retained captures show all four heavy-list screens at row zero with live
updates off and a bar visible; Ink's two captures show the bar changing.
They establish displayed content, not quiet timing.

GPU hook: off (gpuHookEnabled=false).

bar

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Quiet samples | 0 | 0 | 0 | 0 |
| Process CPU, ms/s | — | — | — | — |
| Main-thread CPU, ms/s | — | — | — | — |
| Frames/s; native display link / painted drawables | — | — | — | — |
| Peak footprint, MiB | — | — | — | — |
| End footprint, MiB | — | — | — | — |
| GPU busy time, ms/s; process command buffers | — | — | — | — |
| GPU ms/frame; native cadence / painted drawables | — | — | — | — |
| Completed command buffers per sample | — | — | — | — |

### Bar CPU attempt 2 — excluded

`bar-a-2` retains five rounds of all four arms after a **156.920 s**
gate. **43 of 63** qualifying-gate/series observations fail, with
**0.718–2.074 busy cores** and WindowServer at **0.341 core**. The whole series
is excluded. The static filter excludes **20** timed samples;
its remaining per-arm counts are shown below. No bar CPU row replaces G5m.

GPU hook: off (gpuHookEnabled=false).

bar

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Quiet samples | 0 | 0 | 0 | 0 |
| Process CPU, ms/s | — | — | — | — |
| Main-thread CPU, ms/s | — | — | — | — |
| Frames/s; native display link / painted drawables | — | — | — | — |
| Peak footprint, MiB | — | — | — | — |
| End footprint, MiB | — | — | — | — |
| GPU busy time, ms/s; process command buffers | — | — | — | — |
| GPU ms/frame; native cadence / painted drawables | — | — | — | — |
| Completed command buffers per sample | — | — | — | — |

### Caret CPU attempt 1 — excluded

`caret-a-1` is main's before-M2b cohort, five rounds per arm, hook off and
power off. After a **157.012 s** gate, **16 of 63** qualifying-gate/
series observations fail. Whole-machine CPU is **0.383–1.884 cores**;
`corespotlightd` reaches **0.463 core**. The static filter retains two native
and two GPUI phases, and excludes the other sixteen. Their values below
reproduce that filter; the whole-series rule excludes all of them from the
scoreboard. There is no quiet Ink or egui CPU value in this attempt.

GPU hook: off (gpuHookEnabled=false).

caret

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Quiet samples | 2 | 2 | 0 | 0 |
| Process CPU, ms/s | 11.729 (11.457–12.002) | 18.157 (17.622–18.691) | — | — |
| Main-thread CPU, ms/s | 10.399 (10.145–10.653) | 14.540 (14.083–14.997) | — | — |
| Process instructions, million/s | 20.369 (20.249–20.489) | 35.089 (34.954–35.224) | — | — |
| Process cycles, million/s | 17.035 (17.010–17.060) | 26.489 (24.588–28.391) | — | — |
| Process effective GHz | 1.453 (1.421–1.485) | 1.457 (1.395–1.519) | — | — |
| Process instructions/cycle | 1.196 (1.187–1.205) | 1.332 (1.231–1.433) | — | — |
| Frames/s; native display link / painted drawables | 119.84 (119.78–119.89) | 2.00 (2.00–2.00) | — | — |
| Peak footprint, MiB | 128.95 (128.63–129.28) | 299.11 (292.97–305.25) | — | — |
| End footprint, MiB | 128.62 (128.28–128.95) | 298.56 (292.47–304.66) | — | — |
| GPU busy time, ms/s; process command buffers | — | — | — | — |
| GPU ms/frame; native cadence / painted drawables | — | — | — | — |
| Completed command buffers per sample | — | — | — | — |

### Caret CPU attempt 2 — excluded

`caret-a-2` uses the hook off and power sampling off, after a
**60.742 s** gate. **18 of 63** qualifying-gate/series observations
fail: **0.413–1.942 busy cores**, with `hybridsearchd` at
**0.942 core**. The whole series is excluded.
The static filter excludes **16** sample/launch records; the individually
passing phases below do not qualify a scoreboard row.
This is main's before-M2b carrier, pinned to `d5429830d2`.

GPU hook: off (gpuHookEnabled=false).

caret

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Quiet samples | 3 | 1 | 0 | 0 |
| Process CPU, ms/s | 10.527 (9.751–10.974) | 17.305 (17.305–17.305) | — | — |
| Main-thread CPU, ms/s | 9.282 (8.651–9.800) | 13.767 (13.767–13.767) | — | — |
| Process instructions, million/s | 20.227 (20.205–20.672) | 34.987 (34.987–34.987) | — | — |
| Process cycles, million/s | 15.224 (13.519–15.451) | 23.793 (23.793–23.793) | — | — |
| Process effective GHz | 1.408 (1.386–1.446) | 1.375 (1.375–1.375) | — | — |
| Process instructions/cycle | 1.338 (1.327–1.496) | 1.470 (1.470–1.470) | — | — |
| Frames/s; native display link / painted drawables | 119.88 (119.76–119.94) | 2.00 (2.00–2.00) | — | — |
| Peak footprint, MiB | 126.00 (125.88–127.22) | 292.95 (292.95–292.95) | — | — |
| End footprint, MiB | 125.59 (125.49–126.78) | 292.42 (292.42–292.42) | — | — |
| GPU busy time, ms/s; process command buffers | — | — | — | — |
| GPU ms/frame; native cadence / painted drawables | — | — | — | — |
| Completed command buffers per sample | — | — | — | — |

### Bar GPU attempt 1 — excluded

Configuration (b) qualifies GPU columns only. The CPU columns printed by the
aggregator include the hook's cost and are excluded from the CPU baseline.

`bar-b-1` uses the hook on and power sampling off, after a
**212.581 s** gate. **32 of 63** qualifying-gate/series observations
fail: **0.476–1.810 busy cores**, with `duetexpertd` at
**0.686 core**. The whole series is excluded.
The static filter excludes **20** sample/launch records; the individually
passing phases below do not qualify a scoreboard row.

GPU hook: on (gpuHookEnabled=true).

bar

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Quiet samples | 0 | 0 | 0 | 0 |
| Process CPU, ms/s | — | — | — | — |
| Main-thread CPU, ms/s | — | — | — | — |
| Frames/s; native display link / painted drawables | — | — | — | — |
| Peak footprint, MiB | — | — | — | — |
| End footprint, MiB | — | — | — | — |
| GPU busy time, ms/s; process command buffers | — | — | — | — |
| GPU ms/frame; native cadence / painted drawables | — | — | — | — |
| Completed command buffers per sample | — | — | — | — |

### Bar GPU attempt 2 — excluded

`bar-b-2` uses the hook on and power sampling off, after a
**60.765 s** gate. **26 of 63** qualifying-gate/series observations
fail: **0.465–1.785 busy cores**, with `WindowServer` at
**0.384 core**. The whole series is excluded.
The static filter excludes **20** sample/launch records; the individually
passing phases below do not qualify a scoreboard row.

GPU hook: on (gpuHookEnabled=true).

bar

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Quiet samples | 0 | 0 | 0 | 0 |
| Process CPU, ms/s | — | — | — | — |
| Main-thread CPU, ms/s | — | — | — | — |
| Frames/s; native display link / painted drawables | — | — | — | — |
| Peak footprint, MiB | — | — | — | — |
| End footprint, MiB | — | — | — | — |
| GPU busy time, ms/s; process command buffers | — | — | — | — |
| GPU ms/frame; native cadence / painted drawables | — | — | — | — |
| Completed command buffers per sample | — | — | — | — |

### Caret GPU attempt 1 — excluded

`caret-b-1` uses the hook on and power sampling off, after a
**60.771 s** gate. **17 of 64** qualifying-gate/series observations
fail: **0.532–1.430 busy cores**, with `corespotlightd` at
**0.559 core**. The whole series is excluded.
The static filter excludes **17** sample/launch records; the individually
passing phases below do not qualify a scoreboard row.
This is main's before-M2b carrier, pinned to `d5429830d2`.

GPU hook: on (gpuHookEnabled=true).

caret

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Quiet samples | 1 | 1 | 0 | 1 |
| Process CPU, ms/s | 10.986 (10.986–10.986) | 17.534 (17.534–17.534) | — | 15.395 (15.395–15.395) |
| Main-thread CPU, ms/s | 9.695 (9.695–9.695) | 13.936 (13.936–13.936) | — | 12.525 (12.525–12.525) |
| Process instructions, million/s | 20.951 (20.951–20.951) | 34.998 (34.998–34.998) | — | 27.750 (27.750–27.750) |
| Process cycles, million/s | 15.400 (15.400–15.400) | 23.636 (23.636–23.636) | — | 21.394 (21.394–21.394) |
| Process effective GHz | 1.402 (1.402–1.402) | 1.348 (1.348–1.348) | — | 1.390 (1.390–1.390) |
| Process instructions/cycle | 1.360 (1.360–1.360) | 1.481 (1.481–1.481) | — | 1.297 (1.297–1.297) |
| Frames/s; native display link / painted drawables | 119.95 (119.95–119.95) | 2.00 (2.00–2.00) | — | 2.00 (2.00–2.00) |
| Peak footprint, MiB | 128.33 (128.33–128.33) | 305.09 (305.09–305.09) | — | 438.78 (438.78–438.78) |
| End footprint, MiB | 128.00 (128.00–128.00) | 304.53 (304.53–304.53) | — | 438.41 (438.41–438.41) |
| GPU busy time, ms/s; process command buffers | — | 0.581 (0.581–0.581) | — | 0.387 (0.387–0.387) |
| GPU ms/frame; native cadence / painted drawables | — | 0.2907 (0.2907–0.2907) | — | 0.1933 (0.1933–0.1933) |
| Completed command buffers per sample | 0 (0–0) | 20 (20–20) | — | 140 (140–140) |

### Caret GPU attempt 2 — excluded

`caret-b-2` uses the hook on and power sampling off, after a
**86.078 s** gate. **19 of 64** qualifying-gate/series observations
fail: **0.443–2.910 busy cores**, with `fseventsd` at
**1.031 core**. The whole series is excluded.
The static filter excludes **13** sample/launch records; the individually
passing phases below do not qualify a scoreboard row.
This is main's before-M2b carrier, pinned to `d5429830d2`.

GPU hook: on (gpuHookEnabled=true).

caret

| Metric | Native Apple host | GPUI 0.2.2 | Ink, direct Metal | egui 0.37.0 |
|---|---:|---:|---:|---:|
| Quiet samples | 4 | 2 | 0 | 1 |
| Process CPU, ms/s | 10.388 (10.356–10.569) | 17.558 (16.754–18.362) | — | 15.048 (15.048–15.048) |
| Main-thread CPU, ms/s | 9.116 (9.066–9.349) | 13.974 (13.232–14.716) | — | 12.343 (12.343–12.343) |
| Process instructions, million/s | 20.234 (20.151–20.646) | 34.480 (33.983–34.976) | — | 27.659 (27.659–27.659) |
| Process cycles, million/s | 14.657 (14.347–15.187) | 24.636 (23.010–26.263) | — | 20.969 (20.969–20.969) |
| Process effective GHz | 1.411 (1.385–1.437) | 1.402 (1.373–1.430) | — | 1.393 (1.393–1.393) |
| Process instructions/cycle | 1.392 (1.329–1.414) | 1.404 (1.332–1.477) | — | 1.319 (1.319–1.319) |
| Frames/s; native display link / painted drawables | 119.91 (119.83–119.99) | 2.00 (2.00–2.00) | — | 2.00 (2.00–2.00) |
| Peak footprint, MiB | 129.02 (125.58–133.02) | 292.19 (292.16–292.22) | — | 440.88 (440.88–440.88) |
| End footprint, MiB | 128.63 (125.24–132.45) | 291.76 (291.74–291.78) | — | 440.36 (440.36–440.36) |
| GPU busy time, ms/s; process command buffers | — | 0.582 (0.579–0.586) | — | 0.378 (0.378–0.378) |
| GPU ms/frame; native cadence / painted drawables | — | 0.2912 (0.2894–0.2930) | — | 0.1893 (0.1893–0.1893) |
| Completed command buffers per sample | 0 (0–0) | 20 (20–20) | — | 140 (140–140) |

### Static predictions beside the excluded results

Attempt 1 / attempt 2 are shown in that order. Values are the individually
passing phases retained by the static filter, with the counts in the tables
above. They do not establish a quiet five-round result: every whole cohort is
excluded. A missing value is untested, not zero or a successful prediction.

| Prediction and instrument | Predicted | Reported result, attempts 1 / 2 | Result |
|---|---|---|---|
| Bar process CPU, native / GPUI / Ink / egui, ms/s | 60–95 / 100–160 / 100–170 / 110–180 | No individually passing phase for any arm | Untested; both cohorts excluded |
| Caret native process CPU, ms/s | 6–15 | 11.729 / 10.527 | Within band in partial phases; cohorts excluded |
| Caret GPUI process CPU, ms/s | 10–22 | 18.157 / 17.305 | Within band in partial phases; cohorts excluded |
| Caret Ink process CPU, ms/s | 100–170 | No individually passing phase | Untested; cohorts excluded |
| Caret egui process CPU, ms/s | 8–20 | No individually passing hook-off phase | Untested; cohorts excluded |
| Bar cadence, all arms, Hz | 118–121 | No individually passing phase | Untested; cohorts excluded |
| Caret native display-link cadence, Hz | 118–121 | 119.84 / 119.88, hook off | Within band in partial phases; cohorts excluded |
| Caret GPUI presented cadence, Hz | 1.9–2.1 | 2.00 / 2.00, hook off | Within band in partial phases; cohorts excluded |
| Caret Ink / egui presented cadence, Hz | 118–121 / 1.9–2.1 | No individually passing hook-off phase | Untested in the CPU cohorts; hook-on egui phases show 2.00 Hz |
| Ink GPU time, bar and caret, ms/frame | 1.25–2.5; above 15% of 8.33 ms | No individually passing Ink phase in either workload | Untested; no quiet M8 measurement |
| GPUI / egui bar GPU time, ms/frame | Below 0.417, or 5% of 8.33 ms | No individually passing phase | Untested; cohorts excluded |
| GPUI caret GPU time, ms/frame | Below 0.417 | 0.2907 / 0.2912 | Below the bound in partial phases; cohorts excluded |
| egui caret GPU time, ms/frame | Below 0.417 | 0.1933 / 0.1893 | Below the bound in partial phases; cohorts excluded |

**Scoreboard.** No G4q CPU or GPU row replaces the existing under-background
rows. The quiet-silver explanation of G4→G4m remains untested; the excluded
fling medians are above G4, but cannot settle the question. A qualified silver
retake is owed, followed by the interleaved two-Mac comparison unless that
retake meets G4's ranges. There is no quiet Ink GPU share to compare with the
15% trigger. These are before-M3 runs, so they do not earn M8 in any case.

### Separate power: fling attempt 1 — excluded

`fling-power-1` runs one interleaved round with the hook off and
`powermetrics` on, after a **60.750 s** gate. **1 of 31**
qualifying-gate/series observations fails: **0.461–1.558 busy cores**, with an
external `find` process at **0.421 core**. The whole series is excluded.

The join uses each application's PID and paired wall/CACurrentMediaTime
calibrations. It discards power intervals that could cross either phase edge,
including one second of plist timestamp uncertainty in each direction, then
takes duration-weighted means. This run's calibration spread is
**0.000093460 s**. Every reported arm/phase has at least
five interior samples. System watts are estimates; process energy impact is
a proxy, not watts. Each cell is one run's mean; its repeated min/max is not
a confidence interval. No numerical power saving was predicted.

The sampler exits after its finite sample count. Monitoring covers the gate,
the driver and its terminal observation; later idle power samples are retained
but excluded from the join. Power is reported separately from CPU timings.

| Metric | Native | GPUI | Ink | egui |
|---|---:|---:|---:|---:|
| Fling interior: system CPU, W | 1.727 (1.727–1.727) | 1.221 (1.221–1.221) | 1.575 (1.575–1.575) | 6.140 (6.140–6.140) |
| Fling interior: system GPU, W | 0.029 (0.029–0.029) | 0.172 (0.172–0.172) | 0.216 (0.216–0.216) | 0.054 (0.054–0.054) |
| Fling interior: process energy impact/s | 311.10 (311.10–311.10) | 57.49 (57.49–57.49) | 48.38 (48.38–48.38) | 69.08 (69.08–69.08) |
| Fling interior: system GPU clock, MHz | 338.0 (338.0–338.0) | 338.0 (338.0–338.0) | 338.0 (338.0–338.0) | 343.6 (343.6–343.6) |
| Fling interior: system GPU active, % | 4.80 (4.80–4.80) | 11.60 (11.60–11.60) | 11.90 (11.90–11.90) | 6.11 (6.11–6.11) |
| Rest interior: system CPU, W | 1.038 (1.038–1.038) | 0.987 (0.987–0.987) | 1.264 (1.264–1.264) | 1.240 (1.240–1.240) |
| Rest interior: system GPU, W | 0.002 (0.002–0.002) | 0.001 (0.001–0.001) | 0.007 (0.007–0.007) | 0.012 (0.012–0.012) |
| Rest interior: process energy impact/s | 0.39 (0.39–0.39) | 0.97 (0.97–0.97) | 0.60 (0.60–0.60) | 0.17 (0.17–0.17) |
| Rest interior: system GPU clock, MHz | 338.0 (338.0–338.0) | 338.0 (338.0–338.0) | 371.2 (371.2–371.2) | 386.2 (386.2–386.2) |
| Rest interior: system GPU active, % | 0.55 (0.55–0.55) | 0.36 (0.36–0.36) | 1.36 (1.36–1.36) | 1.95 (1.95–1.95) |

### Separate power: fling attempt 2 — excluded

`fling-power-2` runs one round with the hook off and power sampling on, after
a **60.728 s** gate. **4 of 31** qualifying-gate/series observations
fail: **0.461–1.667 busy cores**, with `duetexpertd` at
**0.843 cores**. The whole series is excluded.
The same interior PID join has **0.000408649 s** calibration spread.
Each cell is one run's duration-weighted mean, with the singleton min/max.

| Metric | Native | GPUI | Ink | egui |
|---|---:|---:|---:|---:|
| Fling interior: system CPU, W | 1.861 (1.861–1.861) | 1.562 (1.562–1.562) | 1.376 (1.376–1.376) | 1.258 (1.258–1.258) |
| Fling interior: system GPU, W | 0.029 (0.029–0.029) | 0.162 (0.162–0.162) | 0.213 (0.213–0.213) | 0.046 (0.046–0.046) |
| Fling interior: process energy impact/s | 309.74 (309.74–309.74) | 53.19 (53.19–53.19) | 64.17 (64.17–64.17) | 53.98 (53.98–53.98) |
| Fling interior: system GPU clock, MHz | 338.0 (338.0–338.0) | 346.1 (346.1–346.1) | 344.5 (344.5–344.5) | 357.7 (357.7–357.7) |
| Fling interior: system GPU active, % | 4.74 (4.74–4.74) | 10.77 (10.77–10.77) | 12.26 (12.26–12.26) | 5.59 (5.59–5.59) |
| Rest interior: system CPU, W | 1.034 (1.034–1.034) | 0.975 (0.975–0.975) | 1.119 (1.119–1.119) | 1.412 (1.412–1.412) |
| Rest interior: system GPU, W | 0.001 (0.001–0.001) | 0.004 (0.004–0.004) | 0.001 (0.001–0.001) | 0.001 (0.001–0.001) |
| Rest interior: process energy impact/s | 0.34 (0.34–0.34) | 1.23 (1.23–1.23) | 0.47 (0.47–0.47) | 0.09 (0.09–0.09) |
| Rest interior: system GPU clock, MHz | 338.0 (338.0–338.0) | 338.0 (338.0–338.0) | 338.0 (338.0–338.0) | 338.0 (338.0–338.0) |
| Rest interior: system GPU active, % | 0.50 (0.50–0.50) | 0.83 (0.83–0.83) | 0.41 (0.41–0.41) | 0.39 (0.39–0.39) |

### Separate power: bar attempt 1 — excluded

`bar-power-1` runs one round with the hook off and power sampling on, after
a **475.678 s** gate. **11 of 24** qualifying-gate/series observations
fail: **0.641–2.398 busy cores**, with `duetexpertd` at
**0.788 cores**. The whole series is excluded.
The same interior PID join has **0.000072241 s** calibration spread.
Each cell is one run's duration-weighted mean, with the singleton min/max.

| Metric | Native | GPUI | Ink | egui |
|---|---:|---:|---:|---:|
| Bar interior: system CPU, W | 2.009 (2.009–2.009) | 1.884 (1.884–1.884) | 2.332 (2.332–2.332) | 1.834 (1.834–1.834) |
| Bar interior: system GPU, W | 0.431 (0.431–0.431) | 0.700 (0.700–0.700) | 1.149 (1.149–1.149) | 0.661 (0.661–0.661) |
| Bar interior: process energy impact/s | 19.23 (19.23–19.23) | 58.70 (58.70–58.70) | 57.62 (57.62–57.62) | 48.96 (48.96–48.96) |
| Bar interior: system GPU clock, MHz | 342.7 (342.7–342.7) | 349.3 (349.3–349.3) | 371.1 (371.1–371.1) | 341.3 (341.3–341.3) |
| Bar interior: system GPU active, % | 32.34 (32.34–32.34) | 46.68 (46.68–46.68) | 49.44 (49.44–49.44) | 45.01 (45.01–45.01) |

### Separate power: bar attempt 2 — excluded

`bar-power-2` runs one round with the hook off and power sampling on, after
a **131.687 s** gate. **11 of 24** qualifying-gate/series observations
fail: **0.680–2.565 busy cores**, with `WindowServer` at
**0.439 cores**. The whole series is excluded.
The same interior PID join has **0.000084162 s** calibration spread.
Each cell is one run's duration-weighted mean, with the singleton min/max.

| Metric | Native | GPUI | Ink | egui |
|---|---:|---:|---:|---:|
| Bar interior: system CPU, W | 1.758 (1.758–1.758) | 1.662 (1.662–1.662) | 1.902 (1.902–1.902) | 2.147 (2.147–2.147) |
| Bar interior: system GPU, W | 0.530 (0.530–0.530) | 0.678 (0.678–0.678) | 1.151 (1.151–1.151) | 0.665 (0.665–0.665) |
| Bar interior: process energy impact/s | 19.88 (19.88–19.88) | 57.62 (57.62–57.62) | 54.72 (54.72–54.72) | 53.01 (53.01–53.01) |
| Bar interior: system GPU clock, MHz | 362.7 (362.7–362.7) | 338.2 (338.2–338.2) | 369.7 (369.7–369.7) | 361.2 (361.2–361.2) |
| Bar interior: system GPU active, % | 38.63 (38.63–38.63) | 46.48 (46.48–46.48) | 49.22 (49.22–49.22) | 48.86 (48.86–48.86) |

### Stage 0b boot cohorts

All four cohorts use the hook and power sampling off. Their complete
per-launch traces, gates and archive audit are recorded in
[MEASUREMENTS](../../../host/ink/MEASUREMENTS.md#stage-0b-g4q-quiet-boot-cohorts-2026-10-30).

**boot-retained-1: excluded.**

GPU hook: off (gpuHookEnabled=false).

| Metric | metal | wgpu |
|---|---:|---:|
| Launches | 10 | 10 |
| Displayed | 10 | 10 |
| Complete Ink screens | 10 | 10 |
| Device/layer phase, ms | 5.969 (5.599–7.855) | 6.290 (6.024–232.594) |
| First pixel, ms | 223.733 (182.126–253.811) | 263.722 (249.341–458.228) |
| First CPU pixel, ms | — | — |
| Complete CPU screen, ms | — | — |
| Complete Ink screen, ms | 223.733 (182.126–253.811) | 263.722 (249.341–458.228) |
| Worker compilation/serialization, ms | 0.000 (0.000–0.000) | 0.000 (0.000–0.000) |
| Active or key windows | 0 | 0 |

**boot-retained-2: excluded.**

GPU hook: off (gpuHookEnabled=false).

| Metric | metal | wgpu |
|---|---:|---:|
| Launches | 10 | 10 |
| Displayed | 10 | 10 |
| Complete Ink screens | 10 | 10 |
| Device/layer phase, ms | 6.540 (5.318–6.756) | 6.407 (6.119–7.022) |
| First pixel, ms | 224.022 (191.142–247.024) | 259.248 (237.942–283.799) |
| First CPU pixel, ms | — | — |
| Complete CPU screen, ms | — | — |
| Complete Ink screen, ms | 224.022 (191.142–247.024) | 259.248 (237.942–283.799) |
| Worker compilation/serialization, ms | 0.000 (0.000–0.000) | 0.000 (0.000–0.000) |
| Active or key windows | 0 | 0 |

**boot-miss-1: excluded.**

GPU hook: off (gpuHookEnabled=false).

| Metric | metal |
|---|---:|
| Launches | 10 |
| Displayed | 10 |
| Complete Ink screens | 10 |
| Device/layer phase, ms | 0.850 (0.731–1.442) |
| First pixel, ms | 198.387 (162.417–215.706) |
| First CPU pixel, ms | 198.387 (162.417–215.706) |
| Complete CPU screen, ms | 206.274 (179.084–224.039) |
| Complete Ink screen, ms | 360.441 (329.084–365.706) |
| Worker compilation/serialization, ms | 198.057 (192.600–205.989) |
| Active or key windows | 0 |

**boot-miss-2: excluded.**

GPU hook: off (gpuHookEnabled=false).

| Metric | metal |
|---|---:|
| Launches | 10 |
| Displayed | 10 |
| Complete Ink screens | 10 |
| Device/layer phase, ms | 0.844 (0.731–0.999) |
| First pixel, ms | 199.441 (181.382–216.351) |
| First CPU pixel, ms | 199.441 (181.382–216.351) |
| Complete CPU screen, ms | 209.589 (181.382–224.684) |
| Complete Ink screen, ms | 355.560 (339.715–366.351) |
| Worker compilation/serialization, ms | 196.369 (191.982–202.331) |
| Active or key windows | 0 |

### Boot predictions beside the results

Attempt 1 / attempt 2 are in that order. All sixty launches displayed a
complete Ink screen, with zero dropped trace events, zero boot-thread
compilation, nominal thermal endpoints and no active or key boot window.
The per-launch span sums and archive hashes reproduce. No failed cohort is
promoted by its passing launch endpoints or by a numeric prediction being met.

| Prediction and instrument | Observed result, attempts 1 / 2 | Disposition |
|---|---|---|
| 30/30 complete Ink screens per three-arm cohort; boot probe | 30/30 / 30/30, including all retries | Completion prediction met |
| Retained Metal phase p50 ≤8 ms; startup span union | 5.969 / 6.540 ms | Numerically met; retained cohorts excluded |
| Retained Metal phase p50 within 2 ms of 6.581 ms; same trace | -0.612 / -0.041 ms | Numerically met; retained cohorts excluded |
| Complete Metal screen p50 ≤wgpu +5 ms; per-launch complete-path sum | 223.733 vs 263.722; 224.022 vs 259.248 ms (-39.989 / -35.226) | Numerically met; no quiet D15 comparison |
| Zero boot-thread compilation in misses; trace thread IDs | 0 / 0 spans | Met |
| Miss CPU first pixel p50 ≤300 ms; presented timestamp | 198.387 / 199.441 ms | Numerically met; cohorts excluded / excluded |
| Miss complete Ink screen p50 ≤450 ms; per-painter completeness | 360.441 / 355.560 ms | Numerically met; cohorts excluded / excluded |

The 100 ms application obligation remains unmet. This carrier retake does not
qualify the Caltrain/Shelf application gate. The retained comparison cannot
replace chunk 4's result under the stronger whole-series quiet rule.

### Gate accounting and disposition

**No quiet baseline was established.** All twenty cohorts failed during
collection, including each allowed retry. Each began after a fresh passing
minute. Total gate waiting was **4278.288 s (71 min 18 s)**, below the
two-hour cumulative allowance; the lane stopped after its prescribed retries,
not because that allowance expired. The first gate began at **18:02:34 PDT**;
the final boot launch ended at **20:31:16 PDT**, followed by its terminal
observation. Collection and finite power-sampler completion time are not gate
waiting. No background process was killed and no go file was required.

The table includes the initial passing minute of each series and all subsequent
collection observations; earlier failed waiting observations are counted in the
gate column and retained in `quiet.jsonl`. The separate initial 1,777.842 s
gate is described above. Across the twenty series, **300/967** qualifying-gate
and collection observations fail; **43** exceed the busy-core bound and
**240** identify WindowServer as the largest process above 0.25 core. These
counts overlap. No series observation fails the compiler or load-average
condition. No sample is rescued by subtracting WindowServer or a process with
an ambiguous command name. In `boot-retained-2`, the landed monitor classifies
`(Heavy list)` as non-benchmark because that command does not match the retained
executable paths. Its two crossings remain failures. This is instrument
classification, not evidence that those cycles came from background work.

| Series | Gate wait, s | Gate failures / observations | Qualifying minute busy cores | Qualifying minute largest process, core | Series + gate failures / observations | Busy cores | Largest non-benchmark, cores |
|---|---:|---:|---:|---:|---:|---:|---|
| fling-a-1 | 60.749 | 0/12 | 0.690–1.183 | 0.145, sysmond | 18/102 | 0.640–1.816 | 2.460, claude (PID 20934) |
| fling-a-2 | 182.160 | 5/36 | 0.679–1.256 | 0.198, sysmond | 22/115 | 0.633–1.769 | 2.077, claude (PID 20934) |
| cold-a-1 | 60.711 | 0/12 | 0.684–1.280 | 0.177, fleet | 3/37 | 0.653–2.269 | 0.320, WindowServer (PID 446) |
| cold-a-2 | 151.809 | 2/30 | 0.686–1.109 | 0.162, fleet | 8/37 | 0.686–2.434 | 0.779, deleted (PID 792) |
| bar-a-1 | 202.523 | 3/40 | 0.610–1.215 | 0.214, sysmond | 45/63 | 0.610–2.643 | 0.859, biomesyncd (PID 30323) |
| bar-a-2 | 156.920 | 3/31 | 0.718–1.167 | 0.140, claude | 43/63 | 0.718–2.074 | 0.341, WindowServer (PID 446) |
| caret-a-1 | 157.012 | 3/31 | 0.383–1.006 | 0.178, fleet | 16/63 | 0.383–1.884 | 0.463, corespotlightd (PID 784) |
| caret-a-2 | 60.742 | 0/12 | 0.450–0.742 | 0.188, fleet | 18/63 | 0.413–1.942 | 0.942, hybridsearchd (PID 933) |
| bar-b-1 | 212.581 | 7/42 | 0.476–1.128 | 0.188, fleet | 32/63 | 0.476–1.810 | 0.686, duetexpertd (PID 806) |
| bar-b-2 | 60.765 | 0/12 | 0.465–0.674 | 0.159, fleet | 26/63 | 0.465–1.785 | 0.384, WindowServer (PID 446) |
| caret-b-1 | 60.771 | 0/12 | 0.532–0.664 | 0.162, fleet | 17/64 | 0.532–1.430 | 0.559, corespotlightd (PID 784) |
| caret-b-2 | 86.078 | 1/17 | 0.495–0.708 | 0.184, fleet | 19/64 | 0.443–2.910 | 1.031, fseventsd (PID 378) |
| fling-power-1 | 60.750 | 0/12 | 0.461–0.714 | 0.197, io.tailscale.ipn.macsys.network-extension | 1/31 | 0.461–1.558 | 0.421, find (PID 91615) |
| fling-power-2 | 60.728 | 0/12 | 0.461–0.708 | 0.182, WindowServer | 4/31 | 0.461–1.667 | 0.843, duetexpertd (PID 806) |
| bar-power-1 | 475.678 | 21/94 | 0.641–1.794 | 0.121, fleet | 11/24 | 0.641–2.398 | 0.788, duetexpertd (PID 806) |
| bar-power-2 | 131.687 | 4/26 | 0.680–1.421 | 0.179, fleet | 11/24 | 0.680–2.565 | 0.439, WindowServer (PID 446) |
| boot-retained-1 | 131.507 | 4/26 | 0.623–0.774 | 0.187, Microsoft Update Assistant | 1/16 | 0.623–1.966 | 0.283, ScreensharingAgent (PID 9846) |
| boot-retained-2 | 60.742 | 0/12 | 0.484–0.829 | 0.184, launchd | 3/16 | 0.484–2.144 | 0.514, (Heavy list) (PID 35938) |
| boot-miss-1 | 65.794 | 1/13 | 0.516–0.930 | 0.173, fleet | 1/14 | 0.516–2.419 | 0.268, WindowServer (PID 446) |
| boot-miss-2 | 60.740 | 0/12 | 0.635–0.988 | 0.212, (fleet) | 1/14 | 0.635–2.331 | 0.247, WindowServer (PID 446) |

The following table counts observations in which each process was the largest
non-benchmark process above 0.25 core. It covers the same qualifying-gate and
collection intervals, not the earlier failed waiting intervals.

| Largest process above the bound | Observations | Maximum cores |
|---|---:|---:|
| WindowServer | 240 | 0.458 |
| fseventsd | 6 | 1.031 |
| corespotlightd | 5 | 0.559 |
| claude | 5 | 2.460 |
| launchd | 5 | 0.409 |
| biomesyncd | 5 | 0.859 |
| duetexpertd | 5 | 0.978 |
| agy | 3 | 0.325 |
| (Heavy list) | 2 | 0.514 |
| mediaanalysisd | 2 | 0.473 |
| fleet | 2 | 0.582 |
| airportd | 2 | 0.369 |
| io.tailscale.ipn.macsys.network-extension | 1 | 0.793 |
| ScreensharingAgent | 1 | 0.283 |
| mds | 1 | 0.454 |
| mds_stores | 1 | 0.835 |
| deleted | 1 | 0.779 |
| UsageTrackingAgent | 1 | 0.435 |
| aned | 1 | 0.385 |
| frauddefensed | 1 | 0.464 |
| hybridsearchd | 1 | 0.942 |
| searchpartyd | 1 | 0.501 |
| usernoted | 1 | 0.257 |
| find | 1 | 0.421 |

Every series records `pmset -g` and `pmset -g batt`: AC power, charged battery,
and `powermode 0`. CPU, GPU and power configurations remain separate. None of
the power runs supplies a CPU baseline. No G4q row replaces an existing
scoreboard row; the prior silver rows remain marked under-background. The
G4→G4m rise is still unattributed. A qualifying silver retake is owed, then
the interleaved two-Mac comparison unless that retake meets G4's ranges.
There is no qualifying Ink static GPU share for M8's 15% trigger, and these
before-M3 measurements cannot earn M8. The 100 ms application obligation is
unchanged.

### G4q verification and reproduction

`aggregate.mjs --verify` reproduces all **twenty** new reports: twelve CPU/GPU
series, four separate-power series and four boot cohorts (`--boot --verify`).
The historical G4 and G5 reports still reproduce exactly. The cohort audit
checks uninterrupted five-second monitor coverage, the passing minute, the
terminal observation, whole-series exclusion, accepted slots, hook flags,
static dimensions, scale, zero wheel input and zero offset, monotone work
counters and completed GPU timestamps. The boot audit checks all sixty
complete scenes, passive windows, trace loss, archive hashes and per-launch
phase sums. Static screenshots are retained; the first bar captures for all
four arms and Ink's changed bar were inspected. Final binary hashes and strict
bundle signatures still match preparation.

The only source change is to the existing aggregator: cold-only cohorts,
separate five-round static workloads, boot tables and verification of the
separate power joins. No renderer design or instrument threshold changes.
The final report branch is rebased onto `origin/main` `1d7a1ac9b5`; the silver
measurement source remains `d5429830d2`. The intervening main changes,
including M2b, are not part of these before measurements. The original
prediction commit remains in `target/g4q/lane-before-rebase.bundle`; the
measured source remains in `target/g4q/measured-source.bundle`.

**Repository checks did not obtain a quiet qualification.** The initial full
run on the measured-source branch exits zero for all six checks, including
both lint commands. The test and workspace-check intervals overlap external
compilers. Those results are retained as passing executions, not a quiet gate
or verification of the rebased tree. The later preflight waits each stop after
three five-minute windows: `checks-2` has 0/177 passing observations;
`checks-rebased` has 0/170. In the latter, Cargo PID 4068 and rustc PID 86725
remain among the observed compilers; maximum 1/5/15-minute loads are
79.723 / 59.345 / 47.881. The final six-check run is therefore unrun. No
unrelated compiler was stopped to obtain a passing gate.

| Initial execution | Exit | Wall seconds | Qualification |
|---|---:|---:|---|
| `cargo build --all-targets --keep-going` | 0 | 1.874 | No failing observation during command; full run unqualified |
| `cargo test --tests --no-fail-fast` | 0 | 194.252 | External compiler overlap |
| `cargo clippy --all-targets --keep-going -- -D warnings` | 0 | 1.319 | No failing observation during command; full run unqualified |
| `cargo fmt --all -- --check` | 0 | 7.531 | No failing observation during command; full run unqualified |
| `bun scripts/caps.mjs` | 0 | 2.704 | No failing observation during command; full run unqualified |
| `bun scripts/boot.mjs` | 0 | 0.025 | No failing observation during command; full run unqualified |
| `cargo check --workspace --all-targets --keep-going` | 0 | 56.585 | External compiler overlap |

Complete verbatim stdout/stderr is retained in
`target/g4q/checks/FULL-OUTPUT.txt`, with the original individual logs beside
it. `suite-results.txt` preserves all 113 reported test-result lines; none
reports a failure. Some lines are from nested suites, so their sum is not a
unique-test count. `checks-2/` and `checks-rebased/` retain the blocked gates,
monitor fields and `results.json`. Report verification output is retained in
`target/g4q/final-verify/OUTPUT.txt`, with one log per series and the historical
G4/G5 logs. Final staged caps passes: `All budgets within cap. 10 categories inspected.`
Its full output is `target/g4q/final-caps.log`.

The raw mirror has **2,593 files, 483,712,856 bytes**, enumerated with SHA-256
in `target/g4q/raw-sha256.json` (manifest SHA-256
`58c8ff9824d6729a882446fb446e95ddfca37cd4c5ac533e446427d7011b26bf`).
It is retained at `target/g4q/silver/target/g0/quiet-d5429830d2/` here; the
original raw series and signed bundles remain at
`~/projects/exact2-measure/target/g0/quiet-d5429830d2/` on silver.
No cited file was removed. Each series retains its gate and observations,
`pmset` state, provenance, runner snapshot, accepted and rejected probe output,
and logs. Power series also retain the entire plist and paired clock
calibrations. The local raw roots include the regenerated power joins and
boot audits; the collection originals on silver need no reporting change.

From this branch, reproduce a CPU/GPU table and its comparison with the record:

```sh
bun bench/heavy-list/gpui/aggregate.mjs target/g4q/silver/target/g0/quiet-d5429830d2/fling-a-1 --verify
bun bench/heavy-list/gpui/aggregate.mjs target/g4q/silver/target/g0/quiet-d5429830d2/caret-b-2 --verify
python3 target/g4q/audit-series.py
```

Use each directory in the gate table. Boot cohorts use `--boot --verify` and
`python3 target/g4q/audit-boot.py boot-retained-1`; the other three names work
likewise. Power joins are regenerated from the retained plist before checking
their tables:

```sh
python3 target/g4q/power-summary.py target/g4q/silver/target/g0/quiet-d5429830d2/fling-power-1
bun bench/heavy-list/gpui/aggregate.mjs target/g4q/silver/target/g0/quiet-d5429830d2/fling-power-1 --verify
```

Repeat for the fling retry and the two bar rounds. These commands report
retained data and do not launch applications or power sampling. Recollection
requires the same Aqua-session recipe in `bench/heavy-list/README.md`, the
retained signed bundles and the runner snapshot from the desired cohort, with
a fresh output directory and the same quiet rule. The old output directories
must remain intact.

## G4q.1, the clock

**Predictions before measurement, 2026-10-30.** Retained G4q fling endpoints
give GPUI 1.59–1.90 GHz, Ink 1.67–1.97 and egui 1.66–1.82; native
2.55–3.01. These are per-sample cycle deltas divided by process CPU seconds,
not ratios of published medians. The old five-round hook comparison has no
cycles or instructions; neither quantity can be recovered for it.

On the same probe, predict Terminal and LaunchServices launches each give all
four arms **3.0–4.5 effective GHz** during fling and process CPU inside G4's
ranges: native **112.39–143.12**, GPUI **82.05–89.05**, Ink
**84.98–87.07**, egui **64.93–71.54 ms/s**. Predict each painted arm's
full-phase presented rate stays within **5%** of G4q attempt 1's median.
The constructor's interactive-QoS diagnostic predicts **3.0–4.5 GHz** too;
it is never a scoreboard configuration. One sample per arm/configuration,
with a second only to separate power-sampler effects or confirm a finding.
The original launch with powermetrics predicts Rust arms below **2.5 GHz**
and more than **50%** of their main-thread CPU on the lower performance
level. QoS, task role and cluster counters decide the explanation.

Raw inputs, reductions, launch sources and observations are retained under
`target/g4q-clock/` here and `target/g0/clock-2035d50319/` on silver.
The silver reports performance-level names **Super** and **Performance**;
no efficiency-core assignment is inferred from GHz alone.

### G4q.1 results and qualification

**The low clock is measured; a restoring launch recipe is not established.**
G4q's Rust fling samples run at 1.59–1.97 effective GHz. The new main-thread
counters put them mostly on the M5 Max's lower **Performance** level, not its
**Super** level. This machine has no efficiency-core level: `hw.perflevel0.name`
is `Super`, level 1 is `Performance`, matching Apple's
[M5 Max CPU description](https://www.apple.com/newsroom/2026/03/apple-debuts-m5-pro-and-m5-max-to-supercharge-the-most-demanding-pro-workflows/).
The brief's “efficiency-cluster clock” describes the suspected symptom, not
this chip's cluster name. Cycles alone do not identify a cluster.

LaunchServices changes the task role and effective QoS accounting, but the
Rust arms remain below 2 GHz. The constructor override also misses. Terminal
reaches the driver's preflight but has no Accessibility grant, so that remedy
has no sample. No tested remedy restores a ≥3 GHz clock for all four arms or
reproduces all four G4 CPU ranges. The hypothesis that SSH ancestry alone
explains the gap is not established. Task placement, burst-driven frequency
selection and other process policy remain candidates; this experiment does
not apportion their costs or the instrument's own overhead.

### Retained phase clocks

For each accepted sample, effective GHz is `Δri_cycles / ΔCPU seconds / 10⁹`;
IPC is `Δri_instructions / Δri_cycles`. CPU seconds use the probe's existing
Mach-timebase conversion. These are ratios within each sample, never ratios
of independently aggregated medians. XNU documents the accounting in
[recount](https://github.com/apple-oss-distributions/xnu/blob/main/doc/observability/recount.md).

| Cohort / sample | Native GHz / IPC | GPUI GHz / IPC | Ink GHz / IPC | egui GHz / IPC |
|---|---:|---:|---:|---:|
| fling-a-1, fling 1 | 2.700 / 2.765 | 1.592 / 1.884 | 1.971 / 1.982 | 1.678 / 1.809 |
| fling-a-1, fling 2 | 2.549 / 2.627 | 1.592 / 1.929 | 1.672 / 2.120 | 1.707 / 1.808 |
| fling-a-1, fling 3 | 2.557 / 2.741 | 1.640 / 1.929 | 1.820 / 2.026 | 1.700 / 1.770 |
| fling-a-1, fling 4 | 2.832 / 2.959 | 1.629 / 1.875 | 1.720 / 2.102 | 1.671 / 1.783 |
| fling-a-1, fling 5 | 2.806 / 2.661 | 1.604 / 1.911 | 1.755 / 2.096 | 1.659 / 1.829 |
| fling-a-1, rest 1 | 1.490 / 0.561 | 1.376 / 1.143 | 1.471 / 0.860 | 1.380 / 0.917 |
| fling-a-1, rest 2 | 1.458 / 0.590 | 1.376 / 1.167 | 1.545 / 0.857 | 1.326 / 1.010 |
| fling-a-1, rest 3 | 1.596 / 0.584 | 1.381 / 1.160 | 1.444 / 0.851 | 1.365 / 1.004 |
| fling-a-1, rest 4 | 1.467 / 0.619 | 1.412 / 1.134 | 1.506 / 0.858 | 1.456 / 0.938 |
| fling-a-1, rest 5 | 1.404 / 0.614 | 1.361 / 1.186 | 1.520 / 0.808 | 1.328 / 0.979 |
| fling-a-2, fling 1 | 2.771 / 2.877 | 1.884 / 1.828 | 1.806 / 2.127 | 1.684 / 1.796 |
| fling-a-2, fling 2 | 2.589 / 2.791 | 1.612 / 1.888 | 1.774 / 1.967 | 1.685 / 1.787 |
| fling-a-2, fling 3 | 2.620 / 2.904 | 1.863 / 1.803 | 1.711 / 2.106 | unavailable |
| fling-a-2, fling 4 | 2.720 / 2.919 | 1.903 / 1.783 | 1.682 / 2.035 | 1.704 / 1.782 |
| fling-a-2, fling 5 | 3.008 / 2.945 | 1.612 / 1.872 | 1.693 / 2.078 | 1.819 / 1.786 |
| fling-a-2, rest 1 | 1.449 / 0.585 | 1.385 / 1.173 | 1.406 / 0.861 | 1.514 / 0.897 |
| fling-a-2, rest 2 | 1.564 / 0.552 | 1.352 / 1.217 | 1.547 / 0.847 | 1.427 / 0.901 |
| fling-a-2, rest 3 | 1.730 / 0.813 | 1.376 / 1.178 | 1.385 / 0.988 | unavailable |
| fling-a-2, rest 4 | 1.433 / 0.601 | 1.405 / 1.108 | 1.411 / 0.889 | 1.351 / 1.003 |
| fling-a-2, rest 5 | 1.521 / 0.566 | 1.367 / 1.134 | 1.432 / 0.895 | 1.378 / 0.937 |

The missing egui round in `fling-a-2` was rejected, including its retry; it is
not zero. `retained.json` also contains the counter-bearing power samples,
with their separate instrument identity. They are not CPU baselines.

| Historical hook sample | Fling GHz / IPC | Rest GHz / IPC |
|---|---:|---:|
| Configurations (a), (b), (c), rounds 1–5, all four arms | unavailable | unavailable |
| Later hook-off Ink instrument sample | 2.195 / 1.802 | 1.895 / 0.869 |

Original G4m and all sixty hook-comparison samples lack cycle/instruction
endpoints. Their clocks cannot be recovered. The later Ink sample is
`target/g4m-hook-review/single/sample/ink-1.json`; its CPU is 134.735 ms/s
in fling and 2.420 at rest. It establishes a low clock only for that sample.
These historical series are marked clock-unqualified, not retroactively
assigned to a cluster. None is a savings baseline.

### Launches and thread placement

Measured 2026-10-30, 20:46–21:05 PDT. All four applications are byte-identical
before their code signatures to the retained G4q arms (`code-identity.json`).
Only the diagnostic probe and
launch adapter change; signed copies and both probe versions remain on silver.
Every row below is one accepted ten-second fling. `ssh-power` uses the original
SSH → privileged Aqua-session switch → user → caffeinate → Python → driver
chain alongside `powermetrics`. `ssh` repeats it without the sampler.
`services` launches each bundle with `open -n -W -a`; the GUI bootstrap domain's
`DYLD_INSERT_LIBRARIES` names that bundle's probe and is cleared after launch.
The driver uses the constructor's PID file to signal the application rather
than `open`. `qos` uses the SSH chain with `G1_CLOCK_QOS=1`: the constructor's
`setpriority(PRIO_DARWIN_PROCESS, 0, 0)` and
`pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0)` both return zero.
That override is diagnostic only. `-confirm` is a second sample with task-role
reads; the power-off SSH control uses that probe too.

| Launch / arm | Effective GHz | IPC | Process CPU, ms/s | Presented frames/s | Main on Performance | Requested QoS |
|---|---:|---:|---:|---:|---:|---:|
| ssh-power / native | 2.824 | 2.567 | 177.74 | — | 41.6% | 21 |
| ssh-power / gpui | 1.939 | 1.374 | 196.55 | 88.95 | 91.5% | 21 |
| ssh-power / ink | 1.913 | 1.788 | 158.10 | 33.38 | 88.8% | 21 |
| ssh-power / egui | 1.724 | 1.556 | 146.01 | 41.49 | 92.0% | 21 |
| ssh / native | 2.853 | 2.577 | 182.22 | — | 44.1% | 21 |
| ssh / gpui | 1.648 | 1.585 | 199.96 | 88.68 | 94.7% | 21 |
| ssh / ink | 1.871 | 1.758 | 158.63 | 33.39 | 93.0% | 21 |
| ssh / egui | 1.750 | 1.489 | 151.66 | 41.50 | 96.5% | 21 |
| services / native | 3.049 | 2.903 | 147.34 | — | 18.8% | 33 |
| services / gpui | 1.822 | 1.526 | 184.03 | 88.98 | 93.1% | 33 |
| services / ink | 1.997 | 1.846 | 147.21 | 33.20 | 90.0% | 33 |
| services / egui | 1.849 | 1.470 | 142.38 | 40.99 | 92.7% | 33 |
| services-confirm / native | 2.862 | 2.813 | 136.63 | — | 27.8% | 33 |
| services-confirm / gpui | 1.884 | 1.536 | 179.13 | 88.99 | 92.8% | 33 |
| services-confirm / ink | 1.967 | 1.893 | 146.92 | 33.49 | 89.4% | 33 |
| qos / native | 2.881 | 2.442 | 180.23 | — | 55.6% | 33 |
| qos / gpui | 1.666 | 1.617 | 192.78 | 89.06 | 93.0% | 33 |
| qos / ink | 1.870 | 1.882 | 149.50 | 33.39 | 89.1% | 33 |
| qos / egui | 1.707 | 1.533 | 148.48 | 41.40 | 94.1% | 33 |
| qos-confirm / native | 2.832 | 2.643 | 183.19 | — | 42.0% | 33 |
| qos-confirm / gpui | 1.723 | 1.591 | 189.22 | 89.19 | 92.1% | 33 |
| qos-confirm / ink | 1.915 | 1.814 | 152.67 | 33.29 | 91.5% | 33 |
| qos-confirm / egui | 1.727 | 1.552 | 146.22 | 41.58 | 93.8% | 33 |

“Main on Performance” is the main thread's lower-level CPU divided by its CPU
across both levels. It comes from XNU `thread_selfcounts` kind 4, called on the
main thread, using `hw.perflevel` ordering; it is not inferred from process
GHz or a system-wide frequency. The ABI is declared in
[XNU's resource header](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/resource_private.h).
Requested QoS 21 means default and 33 means user interactive. Presented rates
count the painted arms' presentation timestamps over the full phase. Native's
AppKit path provides no comparable callback, so its rate is unavailable.

The first `qos` native sample overlaps preparation of the second diagnostic
probe and is excluded from causal comparisons. Its confirmation avoids that
build. In the LaunchServices confirmation egui failed readiness twice; those
rejections remain and supply no row. The first LaunchServices native attempt
lost focus to another Heavy list PID; its accepted retry is shown. The first
three configurations checked other lane processes before their cohort; the
second probe's driver additionally checks before every arm. M10 started between
arms in the last confirmation; the guard waited 96.34 seconds, with nineteen
blocked observations, before launching egui.
All attempts, including refusals, remain in their original directories.

The SSH power-off control passes all its recorded amended quiet observations.
Other measured configurations exceed at least one whole-series bound; these
are diagnostic samples, not qualified scoreboard series. `pmset` reports AC
power and `powermode 0`. The extra per-level/QoS reads and `launchctl procinfo`
inspection have not been calibrated against the ordinary probe. In particular,
their rest CPU is not a renderer regression estimate.

The exact rest endpoints are retained too:

| Launch / arm | Effective GHz | IPC | Process CPU, ms/s | Presented frames/s | Main on Performance | Requested QoS |
|---|---:|---:|---:|---:|---:|---:|
| ssh-power / native | 1.527 | 0.621 | 2.71 | — | 93.2% | 21 |
| ssh-power / gpui | 2.034 | 0.752 | 5.62 | 0.00 | 90.2% | 21 |
| ssh-power / ink | 1.594 | 0.834 | 3.04 | 0.00 | 94.4% | 21 |
| ssh-power / egui | 1.512 | 0.875 | 0.86 | 0.00 | 100.0% | 21 |
| ssh / native | 1.461 | 0.601 | 3.00 | — | 98.7% | 21 |
| ssh / gpui | 1.392 | 1.184 | 5.03 | 0.00 | 98.5% | 21 |
| ssh / ink | 1.415 | 0.928 | 3.22 | 0.00 | 100.0% | 21 |
| ssh / egui | 1.432 | 0.957 | 0.91 | 0.00 | 89.0% | 21 |
| services / native | 1.440 | 0.611 | 2.95 | — | 100.0% | 33 |
| services / gpui | 1.424 | 1.203 | 4.77 | 0.00 | 100.0% | 33 |
| services / ink | 1.427 | 0.810 | 3.18 | 0.00 | 100.0% | 33 |
| services / egui | 1.410 | 0.966 | 0.94 | 0.00 | 100.0% | 33 |
| services-confirm / native | 1.618 | 0.554 | 2.92 | — | 100.0% | 33 |
| services-confirm / gpui | 2.215 | 0.753 | 4.90 | 0.00 | 100.0% | 33 |
| services-confirm / ink | 2.255 | 0.597 | 2.80 | 0.00 | 100.0% | 33 |
| qos / native | 1.919 | 0.531 | 2.56 | — | 100.0% | 33 |
| qos / gpui | 1.407 | 1.162 | 5.01 | 0.00 | 98.5% | 33 |
| qos / ink | 1.390 | 0.878 | 3.28 | 0.00 | 98.4% | 33 |
| qos / egui | 1.335 | 1.022 | 0.88 | 0.00 | 100.0% | 33 |
| qos-confirm / native | 1.426 | 0.678 | 2.77 | — | 98.0% | 33 |
| qos-confirm / gpui | 1.373 | 1.219 | 4.96 | 0.00 | 98.9% | 33 |
| qos-confirm / ink | 1.450 | 0.865 | 3.30 | 0.00 | 98.3% | 33 |
| qos-confirm / egui | 1.382 | 0.962 | 0.93 | 0.00 | 100.0% | 33 |

**QoS and role.** Phase deltas of `rusage_info_v4` give the following share of
process CPU. “Other” sums maintenance, background and utility. The first probe
has no task-role read; the second uses `TASK_CATEGORY_POLICY`, where 0 is
unspecified and 1 is foreground application.

| Launch / arm | Legacy + default | User initiated | User interactive | Other | Task role |
|---|---:|---:|---:|---:|---|
| ssh-power / native | 84.6% | 15.4% | 0.0% | 0.01% | unavailable |
| ssh-power / gpui | 59.5% | 40.5% | 0.0% | 0.04% | unavailable |
| ssh-power / ink | 78.2% | 21.8% | 0.0% | 0.04% | unavailable |
| ssh-power / egui | 70.3% | 29.6% | 0.0% | 0.05% | unavailable |
| ssh / native | 85.5% | 14.5% | 0.0% | 0.01% | 0 |
| ssh / gpui | 60.9% | 39.1% | 0.0% | 0.04% | 0 |
| ssh / ink | 78.2% | 21.7% | 0.0% | 0.03% | 0 |
| ssh / egui | 71.3% | 28.6% | 0.0% | 0.04% | 0 |
| services / native | 18.0% | 4.9% | 77.0% | 0.01% | unavailable |
| services / gpui | 1.2% | 22.6% | 76.2% | 0.02% | unavailable |
| services / ink | 23.2% | 4.7% | 72.1% | 0.03% | unavailable |
| services / egui | 13.8% | 8.5% | 77.7% | 0.05% | unavailable |
| services-confirm / native | 14.0% | 4.8% | 81.2% | 0.01% | 1 |
| services-confirm / gpui | 1.3% | 21.9% | 76.7% | 0.03% | 1 |
| services-confirm / ink | 24.9% | 4.9% | 70.2% | 0.04% | 1 |
| qos / native | 16.5% | 83.4% | 0.0% | 0.01% | unavailable |
| qos / gpui | 1.2% | 98.7% | 0.0% | 0.03% | unavailable |
| qos / ink | 22.3% | 77.7% | 0.0% | 0.04% | unavailable |
| qos / egui | 16.1% | 83.9% | 0.0% | 0.05% | unavailable |
| qos-confirm / native | 15.8% | 84.1% | 0.0% | 0.01% | 0 |
| qos-confirm / gpui | 1.2% | 98.8% | 0.0% | 0.03% | 0 |
| qos-confirm / ink | 24.8% | 75.1% | 0.0% | 0.04% | 0 |
| qos-confirm / egui | 16.1% | 83.9% | 0.0% | 0.05% | 0 |

`launchctl procinfo` and `ps -o pid,ppid,pri,nice,comm` are retained per launched
PID. Direct children are not launchd-managed and name `sshd-keygen-wrapper`
as responsible. LaunchServices children have parent 1, a GUI application job,
`spawn type = app` and `spawn role = ui`, and are responsible for themselves.
The main thread requests interactive QoS in both remedies, but only
LaunchServices produces substantial user-interactive CPU accounting. The
constructor override accounts most CPU as user initiated. That is consistent
with an inherited limit, but does not measure its value. `launchctl procinfo`
does not print a QoS clamp on this OS; `TASK_POLICY_STATE` returns
`KERN_INVALID_ARGUMENT` (2). The retained v2 raw fields are zero-initialized
when that call fails: those zeros do **not** mean an unclamped task. The final
probe emits null for unavailable policy fields.

### Power sampler's independent view

`sudo powermetrics --samplers tasks,cpu_power --show-process-qos
--show-process-amp --show-process-ipc -i 1000` records the original launch.
Only full one-second intervals inside the fling are joined to each PID, with a
one-second interior guard because plist timestamps have second precision.
The CA-to-wall offset comes from the paired quiet observations. Cluster
frequencies are active-time frequencies, with their active residency alongside;
they cover all system work, not just the benchmark.

| Arm | Interior samples | QoS CPU: default / initiated / interactive | Clusters: active GHz; residency |
|---|---:|---:|---|
| native | 7 | 84.2% / 15.8% / 0.0% | P0-Cluster 1.785; 84.3%; P1-Cluster 1.622; 88.3%; S-Cluster 2.812; 67.7% |
| gpui | 7 | 59.9% / 38.9% / 0.0% | P0-Cluster 1.483; 94.5%; P1-Cluster 1.844; 87.7%; S-Cluster 2.782; 66.5% |
| ink | 7 | 76.3% / 23.0% / 0.0% | P0-Cluster 1.354; 95.0%; P1-Cluster 1.701; 55.0%; S-Cluster 2.841; 64.6% |
| egui | 6 | 70.5% / 29.5% / 0.0% | P0-Cluster 1.344; 95.1%; P1-Cluster 1.627; 62.5%; S-Cluster 2.770; 75.6% |

The power sampler and the probe agree that the original launch accounts almost
all CPU in default/legacy and user-initiated tiers, including native. Native
is not observed as user interactive under that launch. The direct main-thread
counters establish Rust placement on the lower Performance level; the system
cluster table alone would not establish it. Power-off control clocks remain
low, so power sampling is not required for the symptom. One pair does not
estimate the sampler's CPU cost.

### Predictions against results

| Prediction, written before collection | Result |
|---|---|
| Original launch: each Rust arm <2.5 GHz and >50% main CPU on the lower level | Met: GPUI 1.939 GHz / 91.5%, Ink 1.913 / 88.8%, egui 1.724 / 92.0% |
| Terminal: all four 3.0–4.5 GHz; CPU within native 112.39–143.12, GPUI 82.05–89.05, Ink 84.98–87.07, egui 64.93–71.54 ms/s | Untested: Terminal lacks Accessibility; no arm launched |
| LaunchServices: all four 3.0–4.5 GHz | Miss: first native 3.049 meets; all Rust samples 1.822–1.997 miss, and native confirmation 2.862 misses |
| LaunchServices: all four CPU ranges above | Miss: first native 147.34, GPUI 184.03, Ink 147.21, egui 142.38; only native confirmation 136.63 reaches its range |
| Painted frames/s within 5% of G4q attempt 1 medians (88.78, 33.39, 41.60) | Met for accepted LaunchServices and override samples: GPUI 88.98–89.19, Ink 33.20–33.49, egui 40.99–41.58; Terminal untested, LaunchServices confirmation egui unavailable |
| Constructor override: all four 3.0–4.5 GHz | Miss in every accepted sample; see both rows per available arm above |

### Amended quiet bound

WindowServer is benchmark work because it composites the benchmark's frames;
it still counts toward whole-machine busy cores. A transient macOS daemon may
exceed 0.25 core in at most 10% of a series' observations, never exceeding 1.0
core. Other user workloads have no exception. Load <30, no compiler and ≤2 busy
cores still apply at every observation. This is the orchestrator's amendment,
applied after the G4q runs; their original qualification records are preserved.

The audit uses the twelve successful gate observations and the complete run,
including its trailing observation. “Process crossings” counts observations
with any remaining process over 0.25 core, including failures outside the daemon
allowance. “Other failures” lists those process failures separately from
busy/load/compiler failures. Completeness still requires every accepted arm.

| Cohort | Process crossings / observations | Maximum, cores | Other failures | Complete | Amended quiet |
|---|---:|---:|---:|---|---|
| bar-a-1 | 8/63 | 0.859 | 0 process; 7 busy/load/compiler | yes | fail |
| bar-a-2 | 0/63 | 0.000 | 0 process; 1 busy/load/compiler | yes | fail |
| bar-b-1 | 2/63 | 0.686 | 0 process; 0 busy/load/compiler | yes | pass |
| bar-b-2 | 1/63 | 0.252 | 1 process; 0 busy/load/compiler | yes | fail |
| bar-power-1 | 4/24 | 0.788 | 0 process; 9 busy/load/compiler | yes | fail |
| bar-power-2 | 6/24 | 0.336 | 0 process; 9 busy/load/compiler | yes | fail |
| boot-miss-1 | 0/14 | 0.000 | 0 process; 1 busy/load/compiler | yes | fail |
| boot-miss-2 | 0/14 | 0.000 | 0 process; 1 busy/load/compiler | yes | fail |
| boot-retained-1 | 1/16 | 0.283 | 0 process; 0 busy/load/compiler | yes | pass |
| boot-retained-2 | 2/16 | 0.514 | 2 process; 3 busy/load/compiler | yes | fail |
| caret-a-1 | 1/63 | 0.463 | 0 process; 0 busy/load/compiler | yes | pass |
| caret-a-2 | 7/63 | 0.942 | 0 process; 0 busy/load/compiler | yes | fail |
| caret-b-1 | 4/64 | 0.559 | 0 process; 0 busy/load/compiler | yes | pass |
| caret-b-2 | 10/64 | 1.031 | 9 process; 6 busy/load/compiler | yes | fail |
| cold-a-1 | 0/37 | 0.000 | 0 process; 2 busy/load/compiler | yes | fail |
| cold-a-2 | 3/37 | 0.779 | 0 process; 4 busy/load/compiler | yes | fail |
| fling-a-1 | 7/102 | 2.460 | 2 process; 0 busy/load/compiler | yes | fail |
| fling-a-2 | 6/115 | 2.077 | 5 process; 0 busy/load/compiler | no | fail |
| fling-power-1 | 1/31 | 0.421 | 1 process; 0 busy/load/compiler | yes | fail |
| fling-power-2 | 2/31 | 0.843 | 1 process; 0 busy/load/compiler | yes | fail |

Thus `bar-b-1`, `caret-a-1`, `caret-b-1` and `boot-retained-1` now pass the quiet
bound. G4q's CPU rows enter the record with their clocks (the Rust arms near
1.9 GHz on the Performance level, native near 2.8 on Super): the clock is
reported, not gated, and work is compared by cycles and instructions per second. GPU and boot quiet qualification is not completion
of M8 or M10's other conditions.

### Launch recipe and reproduction

There is no qualified restoring recipe to prescribe. The tested LaunchServices
recipe is retained in `measure.m` and `run-clock.py`, including exact per-app
environment, PID handling, stdout/stderr and GUI-domain cleanup. Run it from the
Aqua session with a fresh suffix, after the before-each-launch M10 guard:

```sh
cd ~/projects/exact2-measure
sudo -n launchctl asuser $(id -u) sudo -u ccheever \
  /usr/bin/caffeinate -u -d -i /usr/bin/python3 \
  target/g0/clock-2035d50319/run-clock.py services --suffix=-new
```

The Terminal AppleScript request timed out after 84 seconds without starting
the wrapper. Opening the retained executable `terminal.command` with
`open -a Terminal` did start it, but `terminal/driver.log` reports:

```text
this terminal lacks Accessibility permission for CGEvent.postToPid
```

No permission was changed. A Terminal rerun needs that grant, a fresh output
name in the command file, and a clean clock sample before any full series.
The constructor override is never part of a launch recommendation.

The development raw root is `target/g4q-clock/`; the original signed bundles,
raws and launch sources remain at `~/projects/exact2-measure/target/g0/clock-2035d50319/`
on silver. The first probe is retained as `probe-v1.m`, `probe-v1.dylib` and
`v1-*.app`; the second as `probe-v2.m`, `probe-v2.dylib` and the primary bundles.
The `qos/provenance.json` label incorrectly says v2 because the first v2 build
failed before that run; its binary hash and absent task-policy fields identify
v1. Both source versions and the failed-build correction are retained, rather
than rewriting that collection metadata. The final committed probe changes
only failed-policy serialization to null after those measurements.

From this checkout, the retained reductions reproduce all tables without
launching apps or sampling power:

```sh
python3 target/g4q-clock/reduce-retained.py
python3 target/g4q-clock/reduce-runs.py
python3 target/g4q-clock/reduce-power.py
bun bench/heavy-list/gpui/aggregate.mjs target/g4q-clock/ssh/series/fling/ink-1.json --sample
bun bench/heavy-list/gpui/aggregate.mjs target/g4q/silver/target/g0/quiet-d5429830d2/fling-a-1 --verify
```

`retained.json`, `runs.json`, `power-summary.json` and `quiet-amended.json`
retain the unrounded values, phase endpoints or input paths. The old G4q and
hook raw roots cited above remain intact. No cited raw file was deleted.

The collection mirror contains **1,204 files, 314,731,471 bytes**, enumerated in
`target/g4q-clock/raw-sha256.json`, whose SHA-256 is
`09deec28a5ed632f3df0610a33307e08c0966a382945d97432ec6ebb118a81fa`.
It includes both signed instrument versions. Reductions and verification logs
are retained separately in the same root and copied to silver's `report/`.

### Verification

The final probe builds with `-Wall -Wextra -Werror`; its existing offscreen
self-test passes in both hook modes. Seven quiet-rule boundary cases and all
twenty post-hoc cohort classifications pass. All twenty original G4q reports
reproduce, including the added per-sample clock aggregates. All four application
executables match G4q before their signatures. Full verbatim output is
`target/g4q-clock/verification-output.txt`; the probe output is:

```text
probe: painter-specific completeness and one boot-expose wake passed
probe: GPU hook off; three buffers completed, commit-phase accounting passed
probe: painter-specific completeness and one boot-expose wake passed
probe: GPU hook on; three buffers completed, commit-phase accounting passed
```

The six checks could not run under the repository's quiet condition. Three
five-minute waits expired with load averages above 30 or other compilers
present; `checks/results.json` records:

```json
{"unrun": "three five-minute load/compiler waits expired"}
```

The short structural checks were then run separately, without a quiet timing
claim. Caps and boot pass, and `cargo fmt --all -- --check` exits zero with no
output. Build, tests, clippy and workspace check remain unrun in this lane;
there is no output to report for them. The full gate observations are retained
in `checks/repository-quiet.jsonl`. No test failure was treated as a pass.

After rebasing onto `origin/main` `197877789a`, caps, boot and formatting pass
again; their verbatim logs are `rebase-caps.log`, `rebase-boot.log` and
`rebase-fmt.log`. The forced-QoS sample reports its counters, while ordinary
scoreboard aggregation refuses it (`clock-aggregation-test.log`). The rebase
changes no measured application or diagnostic input.
## Stage 1w — Windows startup-tail instrumentation (2026-10-30)

The S2 Shelf startup driver retains a 200-launch, same-executable A/B: 100 device-at-entry launches had
2 runner/layout intervals over 50 ms (138.547 and 77.718 ms); 100
device-after-layout launches had none. Their medians are 3.826 and 2.5525 ms.
This is Shelf's startup interval, not a new heavy-list first-screen or CPU
score. The approximate 10/100 baseline prediction missed.

The stack observer is not qualified: neither logged tail produced a dump,
while one 2.646 ms interval produced a late dump. The injected hold resolves
PDB symbols after snapshotting before symbol loading; three qualification
rounds leave ordinary-tail capture open. These data do not establish a
loader-lock cause, and no startup-policy fix is enabled. Full accounting,
commands, suite results and predictions: `host/ink/MEASUREMENTS.md`,
“Stage 1w”. Retained NucBox files are `target/stage-1w/series/`,
`series.log` and `series-exe-hash.txt`; copies live in this worktree's
`target/stage-1w-nucbox/`. G6e's scoreboard is unchanged.


Stage 1w fix chunk A corrects the observer's marks: logging previously included
one phase write while the watchdog included the other. The 2/100 logged tails
are not established as runner-to-layout work; two-sided Fisher p = 0.4975
against 0/100 supplies no support. A queued channel completion suppresses a
capture, never delays it. Ordinary journeys and the Python driver no longer
arm snapshots. The observer redesign and corrected A/B are queued; this chunk
reruns neither the series nor the scoreboard.

## M10, the first screen's phases

**Chunk 1 is historical diagnostic evidence.** Chunk 2 below repairs the merge,
uses the amended quiet rule and gives the current qualification status. Neither
chunk supplies a qualified replacement for the scoreboard.

**Predictions, before measurement, 2026-10-30.** This chunk instruments boot
and makes no startup cut. The silver Mac's other lane owns the session while
`run-plan.py` or `run-boot.py` runs; M10 waits at most ninety minutes from
02:55 UTC on October 31. No window or cohort runs during that ownership.

G4's cold row is a fresh process with application, filesystem and driver
caches retained. Here a cold launch is followed immediately by a second fresh
process of the same arm, called the warm repeat; no cache is purged in either.
The prepared cohort rotates egui, Ink and GPUI each round for ten pairs. The probe uses process creation
as zero, Core Animation's clock for spans and `MTLDrawable.presentedTime`
for complete screens. Constructor entry is **not main**. Ink has separate main
and host-entry marks; the other arms retain
constructor as a coarse boundary. Hook-off, power-off, trace-on is the primary
instrument; its lazy command-buffer observer does not create a Metal device.
GPUI's transaction path commits a buffer, waits until it is scheduled and calls
`drawable.present()`. For that path the probe joins the direct present request
to the preceding commit on the same thread; it retains both events and all
command commits. The ordinary path tags the buffer passed a drawable directly.
Binary facts and constructor/completion fault counters test the loader hypothesis.
The local OS emits no `DYLD_PRINT_STATISTICS` output for a runnable CLI control;
the separate App Launch diagnostic was prepared but did not run after the
primary cohort failed; the dyld breakdown remains missing. Primary cohorts set no `DYLD_*` variables.
No output is not a zero-duration loader. Completion counters are read
in the presented callback, with its observation time retained; the two-second
readiness report's counters cannot stand in for boot work.

| Prediction | Instrument and decision | Result |
| --- | --- | --- |
| Ink warm repeat complete screen 210 ms, range of the median estimate 180–240 ms | Process start to complete presented drawable | Not qualified. One retained-Metal repeat is 218.510 ms, inside the estimate range; no ten-pair median was obtained. |
| Ink warm: process to constructor 15 ms; remaining entry/AppKit/window 75 ms; plan/data/runner 30 ms; fonts 25 ms; layout and shaping 25 ms; painter/device/encode 15 ms; presentation and image completion 25 ms | Per-launch disjoint timeline, 210 ms estimate; nested shaping and device spans also reported separately | Diagnostic misses: constructor 6.618; plan/data/runner 16.445; fonts 2.750; initial layout 1.237 ms. Constructor to window creation is 74.303 ms, close to 75. Metal constructor is 5.672 ms and first paint 27.786 ms. First present commit to display is 32.398 ms. The remaining shares are not separated; 103.126 ms has no named main-thread interval. |
| Ink cold exceeds warm repeats by at most 25 ms at p50 | Ten within-pair differences, no cache purge | Not tested on ten retained pairs. The only Ink pair differs by +0.997 ms but changes from a CPU archive-miss screen to Metal. |
| At least half of the cold-minus-warm excess is before host entry, with increased page-ins rather than runner instructions | Constructor boundary, dyld statistics and fault counters; absent attribution stays unknown | Unresolved. No matched retained Ink cold/repeat cohort or dyld breakdown. Constructor page-ins are four in each Ink diagnostic; that does not attribute elapsed time. |
| Of the historical roughly 30 ms Ink/CPU difference, at least 15 ms is renderer preparation or waiting to present, and under 10 ms is extra shaping/layout | Hook-off device and paint spans; historical hook-on CPU samples are not a paired control | Unresolved: no matched CPU control. The Metal repeat includes 25.239 ms of glyph-raster spans, 23.038 ms exclusive, and only 1.237 ms of initial layout. Those cannot establish the historical painter difference. |
| GPUI/egui complete-screen p50 is 190–240 / 210–260 ms; Ink's positive gap to GPUI lies mainly after constructor | Same probe and rotating launch pairs | Cold diagnostic misses: GPUI 595.457 ms and egui 560.348 ms. Their constructor boundaries are already 259.494/325.555 ms. Egui repeats at 224.162 ms; GPUI has no repeat. No qualifying comparison. |
| Trace-on adds at most 5 ms to each arm's complete-screen p50 | Three trace-off pairs per arm after the primary cohort, same hook-off launch path; between-series drift remains a limit | Unrun. The coordinator stopped after the primary cohort failed; no observer-cost claim. |

The seven warm estimates are an additive hypothesis, not a sum of marginal
medians. The analysis assigns each launch's nested main-thread spans once,
then reports distributions; asynchronous decode and GPU work are not added
to that timeline. An event after readiness or an unshown drawable is never
substituted for the complete screen. Startup failures remain failed attempts.

**Check prediction:** the six checks, the Linux host tests, the painted-carrier
tests and the probe tests exit zero. Checks run serially with Bun 1.4.2, starting
at one-minute load below 30 with no Rust, Swift or Clang compiler. Artifact builds
on the development Mac supply no timing result.


### Outcome and retained attempts

**M10 is instrumented, not qualified. No startup cut or scoreboard replacement is
made.** The named silver timing and boot runners finished before M10 launched.
A later clock-attribution lane initially overlapped the qualification work.
Its existing guard recognizes `run-*.py`; the retained M10 coordinator uses that
name so the other lane waits before its next launch. The process query used by
M10's local handoff helper truncated `comm` because another column followed it;
its empty blocker list was wrong. `guard-observation.txt` records the actual
state: the clock driver remained alive at its guard, with no benchmark app or
power sampler running. The corrected helper is retained beside the original.

The sixty-second gate keeps the landed bounds: at most two whole-machine busy
cores, at most 0.25 core for the largest non-benchmark process, all load averages
below 30 and no compiler. Power-sampler absence is checked throughout the gate
and cohort in a separate sidecar. A passing minute does not qualify a loaded
cohort. Three attempts crossed the bounds; retries stop under the three-round
rule. The requested 10 cold and 10 repeat launches per arm, trace-off controls,
and separate App Launch diagnostics are incomplete.

| Attempt | Outcome |
| --- | --- |
| `qualification-1` | Refused before launching: another lane's `powermetrics` capture was active. |
| `qualification-2` | One Ink launch rejected at focus confirmation. The driver now uses G4's `NSRunningApplication` activation and five-second confirmation loop, retaining every reply. |
| `qualification-3` | Three complete diagnostics; excluded at 2.110 busy cores. |
| `qualification-4` | Three complete diagnostics; excluded at 2.003 busy cores and WindowServer 0.253 core. |
| `primary-1` | First egui launch failed readiness at 30 s; cohort also excluded at ScreenSharingAgent 0.279 core and WindowServer 0.268 core. |

The failed egui launch has a complete scene, a visible window, confirmed focus,
three drawable acquisitions and three present commits, but no positive presented
drawable timestamp. The timeout sidecar and lossless trace are retained. A commit
is not substituted for display. Whether this is an observer effect, an app race
or a presentation/session failure is unresolved; changing egui's source or its
readiness criterion is outside this trace chunk.

### Diagnostic phase tables

The six completed launches below are all retained, including the first launches
of the newly signed GPUI and egui bundles. Neither their large constructor gaps
nor the smaller repeats establish a cold-cache or loader-work attribution.
Ink's runner mark is the return from boot, an upper bound on first-commit
completion; the benchmark source's bind is a no-op over baked data. The first
text-shape mark is one source, not all boot shaping. GPUI and egui retain probe
window observations and constructor boundaries; their actual `main` and window
construction are not marked in source.

**Unqualified diagnostics only.** Cells are milliseconds from process creation: median (minimum–maximum). No cell is a qualifying cold row.

ink: cold n=2, repeat n=1.

| Milestone | Cold diagnostics | Immediate repeat diagnostics |
| --- | ---: | ---: |
| constructor | 6.331 (5.178–7.483) | 6.618 (6.618–6.618) |
| main | 7.166 (5.894–8.437) | 7.502 (7.502–7.502) |
| AppKit finished launching | 62.053 (58.296–65.811) | 57.829 (57.829–57.829) |
| window created | 84.456 (81.743–87.169) | 80.921 (80.921–80.921) |
| visible window observed | 137.553 (128.048–147.057) | 143.829 (143.829–143.829) |
| plan loaded | 88.838 (88.710–88.965) | 87.380 (87.380–87.380) |
| fonts ready | 92.618 (92.079–93.156) | 90.132 (90.132–90.132) |
| data source bound | 92.726 (92.176–93.276) | 90.221 (90.221–90.221) |
| first commit completed (runner returned) | 110.170 (108.960–111.379) | 106.130 (106.130–106.130) |
| first layout completed | 111.499 (110.290–112.709) | 107.459 (107.459–107.459) |
| first text shape start | 110.303 (109.094–111.512) | 106.256 (106.256–106.256) |
| first text shape end | 110.466 (109.248–111.684) | 106.418 (106.418–106.418) |
| layer attached | 113.654 (112.347–114.962) | 109.546 (109.546–109.546) |
| first paint record | 141.749 (130.824–152.673) | 152.798 (152.798–152.798) |
| first paint record completed | 163.399 (141.820–184.978) | 180.584 (180.584–180.584) |
| first present committed | 167.672 (145.241–190.103) | 186.113 (186.113–186.113) |
| first presented | 220.236 (219.508–220.964) | 218.510 (218.510–218.510) |
| complete screen | 220.236 (219.508–220.964) | 218.510 (218.510–218.510) |

The two cold diagnostics mix one archive-miss CPU screen and one retained-Metal screen. The repeat uses Metal; these are not a matched renderer comparison.

gpui: cold n=1, repeat n=0.

| Milestone | Cold diagnostics | Immediate repeat diagnostics |
| --- | ---: | ---: |
| constructor | 259.494 (259.494–259.494) | — |
| AppKit finished launching | 350.461 (350.461–350.461) | — |
| visible window observed | 570.304 (570.304–570.304) | — |
| first present committed | 542.913 (542.913–542.913) | — |
| first presented | 595.457 (595.457–595.457) | — |
| complete screen | 595.457 (595.457–595.457) | — |

egui: cold n=1, repeat n=1.

| Milestone | Cold diagnostics | Immediate repeat diagnostics |
| --- | ---: | ---: |
| constructor | 325.555 (325.555–325.555) | 7.314 (7.314–7.314) |
| AppKit finished launching | 417.864 (417.864–417.864) | 80.301 (80.301–80.301) |
| visible window observed | 517.236 (517.236–517.236) | 174.797 (174.797–174.797) |
| first present committed | 522.266 (522.266–522.266) | 196.337 (196.337–196.337) |
| first presented | 560.348 (560.348–560.348) | 224.162 (224.162–224.162) |
| complete screen | 560.348 (560.348–560.348) | 224.162 (224.162–224.162) |

### Binary facts and limits of the loader attribution

These are the signed measured copies, built from `f67f57c8a6`. Direct dependencies
come from `otool -L`; defined Objective-C class pointers come from Mach-O sections.
The runtime counts are sampled at the two-second readiness report, not the
constructor. They include classes from the loaded frameworks.

| Arm | Executable bytes | Direct dylibs/frameworks, including probe | Defined ObjC class pointers | Runtime loaded images / registered classes |
| --- | ---: | ---: | ---: | ---: |
| Ink | 37,774,720 | 18 | 5 | 1,149 / 74,735 |
| GPUI | 5,374,400 | 20 | 0 | 1,149 / 74,745 |
| egui | 7,971,088 | 17 | 0 | 1,149 / 74,732 |

Ink's `__TEXT` is 36,388,864 bytes, including 18,877,756 bytes of code and
17,017,160 bytes of constants. Its fixture JSON is 7,168,232 bytes and the
current baked plan 7,850,935 bytes. The release profiles already request
stripping and thin LTO. File size is not pages read, and class count is not
registration time. The large first egui constructor gap has two page-ins and
49,983,519 instructions; its repeat also has two page-ins and 49,857,164
instructions. This argues against treating that entire gap as extra executed
loader work, but does not identify the wait. The planned dyld diagnostic is
still needed. Constructor, page-in, instruction and completion counters for
every completed launch remain in `diagnostic-summary.json`.

### Savings hypotheses, not a funded budget

The basis below is the single unqualified Metal repeat. Inclusive glyph raster
is 25.239 ms across 321 calls; exclusive raster is 23.038 ms. Initial layout is
1.237 ms, and all named layout/shaping work before completion is about 1.8 ms.
The 103.126 ms without a named main-thread interval includes the 32.398 ms from
present commit to display. It must not be relabeled AppKit CPU or added to the
asynchronous image-decode and upload spans.
Layer attachment to the first paint spans another 43.252 ms. The next trace
must explain that interval before claiming it as removable work or waiting.

| Candidate | Phase and numerical basis | Next-cut prediction, ms | Design status |
| --- | --- | ---: | --- |
| Runner boot beside window creation | Plan/data/runner 16.445; window creation 21.294. Overlap cannot save more than the smaller span. | 12, conditional on at most 4.445 ms of dependency/handoff cost; ceiling 16.445 | Moving live runner ownership needs an M10/D11 amendment. Moving only immutable plan decode preserves ownership but exposes just 0.510 ms. |
| Visible-only layout and shaping at boot | The list is already virtualized; about 1.8 ms of named layout/shaping is available. | 0–1 | Implementation if visible output and D4's kept-row validity remain unchanged. No 25 ms layout saving is supported. |
| Shaping/layout cache across launches | Same roughly 1.8 ms ceiling, before cache validation and loading. | 0–1 | Amendment: persistent identity and invalidation are new design. |
| Glyph-mask cache across launches | 23.038 ms exclusive raster; allow 3 ms for validation/loading as an unmeasured engineering assumption. | 20 if that allowance holds | Amendment to cache lifetime/identity under D3/D4; platform font revision, size, scale, synthesis, subpixel position and rendering settings must be respected. |
| Remove one avoidable first-redraw turn | A 120 Hz frame is 8.333 ms. The trace does not prove a removable turn. | 0–8.333 | Implementation only if D7/D11 presentation and thread ownership remain unchanged. |
| Dylib count, stripping, ObjC registration, carrier size | Ink has fewer direct dependencies than GPUI; its sampled warm constructor boundary is 6.618 ms. Size reductions may affect later faults too, which are unattributed. | 0 funded; 6.618 is the whole observed pre-constructor ceiling, not a saving | Ordinary link/data changes are implementation; moving capabilities into new artifacts needs an amendment. |

**Recommended first candidate:** reduce first-screen glyph rasterization. A
persistent mask-cache experiment has the largest named addressable cost, but
requires the amendment above and a paired measurement of cache overhead. It is
not implemented here. The runner-overlap candidate comes next; a shaping/layout
cache has too little measured work to justify priority.

No combination is funded to beat GPUI on either Mac. The hypothetical
12 + 20 + 8.333 = 40.333 ms combination would arithmetically cover the old
silver gap of about 31 ms and black gap of about 39 ms, with almost no black
margin. Confidence is **low**: these cuts are unmeasured, their presentation
savings may overlap, the old rows have the GPU hook on, and no qualifying
hook-off baseline or black phase trace was obtained. It would also leave far
more than 100 ms in the sampled application launch. M10's application obligation
remains open.

### Verification and reproduction

The three release arms build, and the probe passes hook-off, hook-on, lazy-trace
and direct-present tests. The analysis checks nested-span conservation,
separation of background work, missing markers and rejection of an incomplete
cohort. The initial root build and root tests exit zero: the test log has 113
successful result lines and no failed result. Their preflight passed, but
external compiler activity during execution was not monitored, so these are
functional results without a full quiet qualification.

Clippy's preflight then waited 983 seconds across 29 observations, exceeding
three five-minute windows, with no quiet opening. One-minute load went from
254.7 to 693.2 and other lanes' compilers remained active. This lane's waiting
controller was stopped before rebasing. Clippy, the final format check, caps,
boot, workspace check, Linux tests and painted-carrier tests did not run; the
check prediction is unmet. The final rebased tree has no six-check pass. Exact
commands, exit records and complete verbatim output are retained in
`target/m10/checks/`; `verbatim-output.txt` collects the executed build/test logs,
including the initial stale-lockfile build failure. `clippy.gate.json` and
`stopped-wait.json` retain the blocked preflight and the owned processes stopped.

`target/m10/` retains the signed bundles, binary/fixture facts, source patch,
probe source, each driver revision, commands, build/check output and the local
mirror of every remote attempt under `silver/`. The remote originals remain at
`~/projects/exact2-measure/target/g0/m10-f67f57c8a6/`. No cited raw file was removed.
The bundle and run provenance record hashes; each run also records its driver
hash. `quiet.jsonl`, `power-observations.jsonl`, the gate, schedule, focus replies,
timeout sidecars, stderr and trace files retain exclusions as well as successes.
`measured-source.bundle` retains the two commits before rebase;
`final-provenance.json` names the rebased tree and confirms that its instrument
source matches the measured source and retained driver.

Reproduce the diagnostic tables without launching anything:

```sh
python3 target/m10/diagnostic-report.py
```

The normal aggregator deliberately refuses the incomplete, loaded primary run:

```sh
python3 bench/heavy-list/gpui/startup.py summarize target/m10/silver/primary-1
```

For a future qualifying collection, use the README's Aqua-session recipe, the
signed bundles and `run-m10.py`/`run-m10-plan.py` snapshots, with fresh output
names. The `run-` name makes the quiet wait visible to the other lane's existing
launch guard. The coordinator stops on any failed cohort before controls or
App Launch profiling. Keep that separation and retain every failed attempt.

### Chunk 2 — matched retained-cache cohorts

**Predictions recorded before collection, 2026-10-30.** The merged probe is
repaired in `8b6bdcca59`: page-ins join the sample dictionary that holds the
clock lane's cycles, instructions, thread placement, QoS and task-policy reads.
The startup trace, lazy device observer and direct-present join remain. GPUI
and egui now mark their actual `main`, data readiness and framework boundaries.

Scoreboard-cold means exactly G4/G4m's cold-only launch: a fresh process, with
**nothing evicted**. Application archives, filesystem pages, dyld closures and
OS/driver caches stay between launches. Warm is the immediate repeat of the
same signed arm, with all those caches retained again. These labels describe
launch order, not two cache-reset policies. Ten pairs per arm rotate the arm
order. All arms use the same hook-off, power-off, trace-on probe, the same
Aqua-session launch chain and 800×800-point outer window at 2×. No QoS override
is set. Both dyld-statistics variables are requested on every launch; absent
output will be unavailable, never zero. The pre-main boundary is measured at
`main`; constructor remains separately named.

The amended quiet rule counts WindowServer as benchmark work and admits at
most 10% transient macOS-daemon crossings of 0.25 core, none over 1.0 core;
whole-machine busy cores stay at most two, loads below 30, with no compiler.
User workloads get no crossing allowance. The runner checks for another lane
before each launch and preserves all attempts and five-second observations.

`target/m10-chunk2/predictions.json` was written at 2026-10-31T04:34:42.340152+00:00.
Times below are predicted medians in milliseconds from process creation,
with acceptance ranges in brackets. They use chunk 1's diagnostic Ink repeat,
egui repeat and G4m's GPUI row; GPUI's detailed split is a hypothesis.

### Chunk 2 outcome: no qualified table

The three collection attempts failed qualification. Stop at the three-round
limit: there is no fourth collection, no ten-pair median and no observer-cost
control. The repaired probe and all three release arms build. No startup
optimization is implemented. The values below remain diagnostics; they do not
replace the historical scoreboard or fund a savings claim.

| Arm | Qualified scoreboard-cold p50, n=10 | Qualified warm p50, n=10 | Qualified paired difference |
| --- | --- | --- | --- |
| Ink | unavailable | unavailable | unavailable |
| GPUI | unavailable | unavailable | unavailable |
| egui | unavailable | unavailable | unavailable |

All three attempts passed the strict sixty-second preflight. The ownership
checks found no other benchmark driver or arm. Background user work still
crossed the cohort bounds; an empty benchmark-owner list does not imply a
quiet machine. No other process was stopped.

| Retained directory | Completed launches | Cohort observations | Exclusion |
| --- | ---: | ---: | --- |
| `qualification-1` | 0 | 0 | Guard initially counted its own sibling `caffeinate` helper. The owned waiting driver was stopped; guard fixtures now distinguish its helper from foreign or orphan helpers. This is not a collection attempt. |
| `qualification-2` | 2 egui | 1 | 2.099 busy cores exceeds two. |
| `qualification-3` | 2 per arm | 3 | 2.139 peak busy cores; all three observations cross 0.25 non-benchmark core, including XProtectService at 0.602 and ScreensharingAgent at 0.271. The allowance is 10%, not 100%. |
| `primary-1` | 0 | 6 | One `agy` observation at 0.289 core, outside the system-daemon allowance and also 16.7% of observations. Egui's first launch times out after 30 seconds with no positive presented timestamp. |

The failed egui launch confirms focus, a visible window, scene completeness,
three drawable acquisitions and three present commits. None produced a positive
presentation timestamp. Its timeout, focus replies and lossless trace remain.
This repeats chunk 1's failure; the observer, app and presentation session are
not distinguished. A commit or readiness flag cannot substitute for display.

No cache was purged in these attempts. The first launches of the newly signed
copies were egui 606.821, Ink 711.734 and GPUI 443.042 ms. Their process-to-main
intervals were 268.655, 476.894 and 216.889 ms. XProtect activity appeared during
the Ink/GPUI qualification. That coincidence does not attribute those intervals
to XProtect or dyld. Egui's later first-of-pair launch is 213.161 ms. Keep all
these observations; do not select the smaller one as a cold median.

### Predictions beside excluded observations

No qualified result exists for any prediction below. All observed times are excluded individual launches, **not medians**. `†` marks an observation outside its predicted range; it is a diagnostic miss, not a qualified hypothesis test. Egui has two attempts, shown in collection order. Ink and GPUI each have one pair.

ink:

| Milestone | Warm prediction [range] → excluded observations | Scoreboard-cold prediction [range] → excluded observations |
| --- | ---: | ---: |
| constructor | 7 [4–12] → 6.095 | 9 [6–14] → 475.746† |
| main | 8 [5–13] → 6.892 | 10 [7–15] → 476.894† |
| host entry | 8 [5–14] → 6.899 | 10 [7–16] → 476.923† |
| AppKit finished launching | 58 [48–72] → 64.182 | 63 [53–77] → 543.661† |
| window created | 81 [70–96] → 87.374 | 86 [75–101] → 566.952† |
| plan loaded | 87 [76–102] → 93.150 | 92 [81–107] → 577.909† |
| fonts ready | 90 [79–105] → 96.077 | 95 [84–110] → 581.279† |
| data source bound | 90 [79–105] → 96.194 | 95 [84–110] → 581.847† |
| first commit completed (runner returned) | 106 [92–125] → 112.565 | 111 [97–130] → 599.575† |
| first layout completed | 108 [94–127] → 113.926 | 113 [99–132] → 601.536† |
| first text shape start | 106 [92–126] → 112.720 | 111 [97–131] → 599.911† |
| first text shape end | 107 [93–126] → 112.903 | 112 [98–131] → 600.408† |
| layer attached | 110 [96–130] → 116.173 | 115 [101–135] → 604.337† |
| visible window observed | 144 [125–165] → 149.786 | 149 [130–170] → 645.246† |
| first window update | 144 [120–175] → 117.675† | 149 [125–180] → 608.038† |
| first paint record | 153 [130–175] → 153.180 | 158 [135–180] → 649.572† |
| first paint record completed | 181 [155–205] → 182.586 | 186 [160–210] → 678.731† |
| first present committed | 186 [160–210] → 187.630 | 191 [165–215] → 683.682† |
| first presented | 219 [190–245] → 217.928 | 224 [195–250] → 711.734† |
| complete screen | 219 [190–245] → 217.928 | 224 [195–250] → 711.734† |

gpui:

| Milestone | Warm prediction [range] → excluded observations | Scoreboard-cold prediction [range] → excluded observations |
| --- | ---: | ---: |
| constructor | 7 [4–15] → 5.164 | 9 [6–17] → 213.849† |
| main | 8 [5–17] → 6.061 | 10 [7–19] → 216.889† |
| data ready | 20 [12–35] → 19.002 | 22 [14–37] → 243.104† |
| AppKit finished launching | 70 [50–95] → 75.969 | 75 [55–100] → 309.021† |
| window created | 150 [115–185] → 138.158 | 155 [120–190] → 377.488† |
| visible window observed | 175 [140–205] → 184.508 | 180 [145–210] → 418.865† |
| first window update | 175 [140–205] → 98.819† | 180 [145–210] → 336.848† |
| first present committed | 185 [155–215] → 150.543† | 190 [160–220] → 390.821† |
| first presented | 215 [185–245] → 211.562 | 220 [190–250] → 443.042† |
| complete screen | 215 [185–245] → 211.562 | 220 [190–250] → 443.042† |

egui:

| Milestone | Warm prediction [range] → excluded observations | Scoreboard-cold prediction [range] → excluded observations |
| --- | ---: | ---: |
| constructor | 7 [4–15] → 5.542, 6.108 | 9 [6–17] → 266.564†, 4.828† |
| main | 8 [5–17] → 6.217, 6.942 | 10 [7–19] → 268.655†, 5.489† |
| data ready | 20 [12–35] → 18.857, 19.453 | 22 [14–37] → 293.335†, 17.869 |
| AppKit finished launching | 80 [60–100] → 78.899, 70.764 | 85 [65–105] → 364.370†, 75.711 |
| app creation | 140 [110–170] → 113.897, 105.016† | 145 [115–175] → 504.397†, 111.984† |
| app fonts ready | 150 [120–180] → 114.894†, 106.033† | 155 [125–185] → 505.321†, 112.971† |
| visible window observed | 175 [150–200] → 167.638, 156.294 | 180 [155–205] → 565.116†, 177.891 |
| first window update | 175 [145–205] → 147.190, 137.461† | 180 [150–210] → 537.311†, 144.825† |
| first present committed | 196 [175–220] → 184.506, 179.636 | 201 [180–225] → 569.026†, 181.663 |
| first presented | 224 [200–250] → 217.907, 219.304 | 229 [205–255] → 606.821†, 213.161 |
| complete screen | 224 [200–250] → 217.907, 219.304 | 229 [205–255] → 606.821†, 213.161 |


For each arm, predicted paired cold minus warm was **5 ms [−10, 15]**:
**2 [−5, 7]** before main, **2 [−8, 8]** from main to AppKit's launch
notification, and **1 [−10, 10]** after that notification. The results below
are excluded pairs; `†` denotes a diagnostic miss of that interval.

| Arm / collection | Complete screen | Before main | Main → AppKit | AppKit → complete |
| --- | ---: | ---: | ---: | ---: |
| Ink / qualification-3 | +493.806† | +470.001† | +9.477† | +14.328† |
| GPUI / qualification-3 | +231.480† | +210.828† | +22.224† | −1.572 |
| egui / qualification-2 | +388.914† | +262.439† | +23.032† | +103.443† |
| egui / qualification-3 | −6.143 | −1.453 | +6.400 | −11.090† |

Ink's predicted first-paint difference of at most 8 ms is −0.247 ms in its
excluded pair. Effective launch GHz was predicted in **[1.6, 2.5]** for every
arm, based on G4q.1's fling placement. Every completed diagnostic misses: Ink
cold/warm **3.949/4.154**, GPUI **3.734/4.216**, egui's first pair
**3.862/4.228**, second pair **4.221/4.231**. Fling placement does not predict
startup placement. These ratios are process cycles divided by CPU time through
the complete-screen callback; they are not instantaneous clock samples. The
callback follows display by 0.247/0.300/0.231 ms in the last Ink/GPUI/egui
repeats. The raw and reducer also retain pre-constructor, pre-main and remaining
launch ratios, CPU time, instructions and IPC. No power sampler was running.

The predicted trace-off median within **5 ms** of trace-on is **unrun** for
all three arms: the coordinator stops after a failed primary cohort. Thus the
phase observer's effect is unbounded here. No qualified prediction is met.

### What the repeats locate

These are the last excluded repeats, one launch per arm; each row is an
observed elapsed interval, not an additive set of marginal medians.

| Phase, ms | Ink | GPUI | egui |
| --- | ---: | ---: | ---: |
| Process → main | 6.892 | 6.061 | 6.942 |
| Main → AppKit finished launching | 57.290 | 69.908 | 63.822 |
| AppKit finished launching → first present commit | 123.448 | 74.574 | 108.872 |
| First present commit → complete display | 30.298 | 61.019 | 39.668 |
| Process → complete display | 217.928 | 211.562 | 219.304 |

Ink's window is created at 87.374 ms; the layer is attached at 116.173,
the `set_visible(true)` call returns at **118.304**, and first paint starts
at **153.180**. Its **34.876 ms** show-return-to-paint interval is a scheduling
lead. The probe's “visible window observed” is a later inspection at 149.786,
not the time the window was ordered front. Treating the 33.613 ms between layer
attachment and that inspection as the cost of showing the window is wrong.
First paint ends at 182.586; the first present commit ends at 187.630.

The main-to-AppKit interval is not all AppKit initialization. The named
configuration/event-loop span occupies 29.660 ms; 27.630 ms remains before the
launch notification. Within the named span the carrier prepares its environment,
builds its configuration and winit event loop, installs the event waker and
constructs state. The separate App Launch diagnostic below names platform work;
it does not split this excluded 57.290 ms interval causally.

The first paint contains **23.010 ms exclusive glyph rasterization**. The
runner-through-first-commit span is **16.387 ms**; plan decode is **0.542**,
font setup **2.925**, and initial layout/shaping is small. The first text-shape
marker names one source; the exclusive reducer charges every nested span once
and conserves process-to-display time. Background work is reported separately.

| Arm, last repeat | Constructor / main / complete page-ins | Complete CPU, ms | Cycles, millions | Effective GHz |
| --- | ---: | ---: | ---: | ---: |
| Ink | 4 / 6 / 29 | 226.599 | 941.336 | 4.154 |
| GPUI | 1 / 1 / 1 | 229.546 | 967.686 | 4.216 |
| egui | 2 / 3 / 20 | 247.809 | 1048.408 | 4.231 |

Page-ins are process counters, not named dyld page-ins. Ink's cold/repeat
page-ins are identical at all three boundaries despite a 470 ms pre-main
difference; page-in counts alone do not explain elapsed launch time.

### Dyld and AppKit attribution: separate diagnostic

Both `DYLD_PRINT_STATISTICS` and `DYLD_PRINT_STATISTICS_DETAILS` were set for
every collection launch and confirmed by the probe; none emitted the requested
breakdown. `dyld_info` retains static linked-image, initializer and segment
facts, not elapsed launch statistics. Per-launch dyld share is therefore
**unavailable**, including for the excluded cohorts. The actual `main` marker
and page-in counters remain usable observations; pre-main is not synonymous
with dyld work, and dyld also runs after main.

An App Launch recording supplies a diagnostic alternative on macOS 27.0.1.
`app-launch-normal/` retains one five-second recording per arm, exported
`dyld-activity-interval`, `dyld-library-load`, `time-profile` and `process-info`
tables, commands and target logs. The dyld **Launch Executable** spans are
**6.310 ms Ink, 6.465 GPUI, 7.181 egui**. They are profiler intervals, not
process-to-main measurements or a share of the separately collected cold row.
Instruments changes launch and observation; these cannot qualify any prediction.
All three recordings return 54 with a warning that the requested sampling
period differs from the configured period; their dyld tables export with exit
zero. No CPU sample weights are summed across the two exported profile tables.

In Ink's recorded main-thread stacks, `NSApplication` initialization queries
menu-bar state; `finishLaunching` customizes the main menu, adds text-input
items and checks Writing Tools; event routing and the Apple open event lead
to the finish-launch notification. The dyld trace records WritingToolsUI's
`dlopen` at **6.721 ms**, AppIntents at **2.086 ms** and the accessibility base
library at **1.807 ms**, after the executable-launch interval. These nested
and sometimes concurrent intervals must not simply be added. They identify
real owners inside startup, not permission to remove platform behavior or a
causal partition of the excluded 57.290 ms phase. The relevant Apple guidance
is [Reducing your app's launch time](https://developer.apple.com/documentation/xcode/reducing-your-app-s-launch-time).

The earlier `app-launch/` passive diagnostic is also retained. Its boot-only
probe initializes `NSApplication` in its constructor to suppress focus, so its
43.828 ms dyld launch includes 38.445 ms in that probe initializer. GPUI then
panics because its expected application subclass is absent (`Ivar platform not
found on class NSApplication`). The normal-launch recordings remove that
diagnostic flag. Neither diagnostic path changes the cohort source or its
readiness criterion. No diagnostic is silently promoted into a scoreboard row.

### Savings budget and first cut

**Funded saving: zero.** Qualified medians and the trace-off control are missing.
The table records numeric hypotheses for the next implementation experiment,
using the last excluded Ink repeat as a provisional ceiling. None has been
implemented or measured. Each saving must be tested with the same hook-off
instrument, matched retained-cache cohorts and a qualified complete-screen
endpoint before it enters the budget.

| Candidate | Phase and provisional evidence | Predicted net saving, ms | Implementation or amendment |
| --- | --- | ---: | --- |
| Prepare the first frame as soon as the layer is ready, before ordering the window front; submit at the first valid opportunity | Show returned → paint began is 34.876 ms; paint itself is 29.406 ms. Preserve decode completeness, compose target and positive displayed endpoint. | 20; reserve 14.876 ms for dependencies and scheduling. This is a hypothesis, not proof that the whole gap is removable. | Initial scheduling/preparation can be an implementation under D7/D11 if main-thread ownership, hidden-window lifecycle, acquisition and retirement contracts stay intact. |
| Overlap runner boot with AppKit launch | Runner through first commit is 16.387 ms; main → AppKit is 57.290. Only the independent portion can overlap. | 12, allowing 4.387 ms for dependencies/handoff; ceiling 16.387 | M10/D11 amendment for new worker ownership or a transferable boot representation. Moving only immutable plan decode exposes 0.542 ms. |
| Parallel first-frame glyph rasterization | 23.010 ms exclusive raster work in first paint | 14; three equal workers would expose 15.340 ms before overhead, leaving 1.340 ms for scheduling/merge | D3/D11 amendment: worker boundary, font/rasterizer ownership and deterministic atlas insertion. |
| Persistent glyph-mask cache | Same 23.010 ms raster work, on cache hits | 20, allowing about 3 ms for lookup/read/validation | D3/D4 amendment: identity across font/OS/rasterizer changes, persistence, invalidation and cold-cache behavior. Alternative to parallel rasterization; do not add both savings. |
| Reduce AppKit launch work | 29.660 ms configuration/event-loop span plus 27.630 to notification; separate profile names application/menu initialization, text input, Writing Tools and event routing | 0 until a specific dispensable dependency is isolated | Ordinary redundant carrier setup is implementation. Removing native menus, input or accessibility behavior requires a design/parity decision; no private-API bypass is proposed. |
| Present before ordering front | First commit → display is 30.298 ms, but no hidden drawable has been shown to present earlier | 0 separately funded | May be initial scheduling under D7; changing hidden-surface acquisition/presentation policy needs a D7/D11 amendment. It overlaps the first candidate; never add a second presentation saving for the same frame. |
| Reduce carrier size or dylibs | Ink is 37,774,720 bytes with 18 direct dylibs; GPUI 5,374,432/20, egui 7,971,104/17. Warm Ink pre-main is only 6.892 ms; page-ins do not attribute the large first-signed-launch delay. | 0; the observed 6.892 ms is an entire warm pre-main ceiling, not a size saving | Ordinary link/data reduction is implementation. Splitting optional capabilities into new artifacts needs an amendment. |

**Recommended first cut:** initial paint scheduling. Test the 20 ms hypothesis
before adding persistence or changing runner ownership. On the excluded repeat
alone it would move Ink from 217.928 to 197.928 ms, below GPUI 211.562 and egui
219.304. That arithmetic is a test priority, not a cold-row claim.

The independent scheduling + runner-overlap + parallel-raster hypotheses sum
to **46 ms**, giving a hypothetical **171.928 ms** repeat. Substituting the
persistent-mask hypothesis for parallel rasterization gives **50 ms** and
**167.928 ms**. These assume no overlap of saved work, unchanged completeness
and no shifted display opportunity. Confidence is **low**. **No combination is
yet established to put Ink's scoreboard-cold row below either comparator**,
and both hypothetical totals remain above the application's 100 ms obligation.

### Chunk 2 verification and reproduction

The first commit repairs only the merged probe and its existing self-test.
`a91fd38b4d` adds the coarse comparator marks and aligned startup accounting.
The Linux host, both painted-carrier entry points and benchmark data/entry
marks compile in the release builds. GPUI and egui use their own manifests
with `--release --locked --target-dir target`; the standalone painted carrier
uses `cargo build --release -p exact-apple-painted --target-dir target`.
The application carrier uses the worktree's own target and:

```sh
EXACT_APP_DIR="$PWD/bench/heavy-list/exact-heavylist" \
EXACT_PAINTED_PROBE="$PWD/bench/heavy-list/gpui/probe.m" \
CARGO_TARGET_DIR="$PWD/target" \
bun host/apple/build.mjs exact-heavylist-apple --painted-benchmark --release
```

The probe compiles with these flags (the self-test replaces `-dynamiclib` with
`-DG1_PROBE_TEST` and an executable output):

```sh
xcrun clang -dynamiclib -fobjc-arc -O2 -Wall -Wextra -Werror \
  -Wno-unused-parameter -mmacosx-version-min=14.0 \
  -framework AppKit -framework QuartzCore -framework Metal \
  bench/heavy-list/gpui/probe.m -o target/m10-chunk2/probe.dylib \
  -install_name @executable_path/../Frameworks/probe.dylib
```

With `G1_CLOCK_DIAGNOSTIC=1`, off and unset each exit zero and print:

```text
probe: clock placement, QoS, task policy and startup page-ins coexist
probe: painter-specific completeness and one boot-expose wake passed
probe: GPU hook off; three buffers completed, commit-phase accounting passed
```

On exits zero and prints:

```text
probe: clock placement, QoS, task policy and startup page-ins coexist
probe: painter-specific completeness and one boot-expose wake passed
probe: GPU hook on; three buffers completed, commit-phase accounting passed
```

Invalid exits **2**, as expected, and prints:

```text
probe: clock placement, QoS, task policy and startup page-ins coexist
probe: painter-specific completeness and one boot-expose wake passed
probe: G1_GPU_HOOK must be 0 or 1
```

The trace-on variants of both valid hook settings also pass; before the final
GPU-hook line each prints:

```text
probe: lazy trace observes application queues without pre-creating a device or queue
probe: direct presentation preserves methods, joins preceding commit, excludes buffer path and resets reused drawables
```

The analyzer/guard verification prints:

```text
startup: syntax and seven amended quiet-rule boundary cases passed
startup: 6 retained diagnostic traces conserve process-to-display time; missing clock counters remain unavailable
startup: own caffeinate accepted; five foreign driver, arm and helper cases refused
```

Root and benchmark formatting pass. Caps pass with `EXACT_CAPS_WASM=0`;
this is the structural caps check, not a wasm budget measurement. Per the
chunk's explicit overloaded-Mac instruction, **no Cargo check, Cargo test,
Clippy, root all-target build, boot suite, Linux test suite or painted-carrier
test suite runs in chunk 2**. The orchestrator must run the six checks and
affected suites on the Mac mini. Release builds are compilation evidence,
not those suite results.

All raw files remain under `target/m10-chunk2/`, mirrored from
`~/projects/exact2-measure/target/g0/m10-chunk2-a91fd38b4d/` on the silver.
The measured source is `a91fd38b4d`; `measured-source.bundle`, `source.patch`,
probe source, signed-bundle hashes, each collection's exact driver hash,
pre-run predictions, build/check logs and remote machine facts are retained.
The remote source checkout is unchanged; only its `target/g0/` holds this lane's
artifacts. `final-provenance.json` records the final branch and rebase facts.
No cited file under either target directory was removed.

Reproduce the excluded tables and the separate profiler attribution without
launching an app:

```sh
python3 target/m10-chunk2/diagnostic-report.py
python3 target/m10-chunk2/launch-report.py
```

The normal aggregator refuses each of `qualification-2`, `qualification-3`
and `primary-1`; no failure is discarded to manufacture a passing cohort:

```sh
python3 bench/heavy-list/gpui/startup.py summarize \
  target/m10-chunk2/silver/primary-1
```

A future collection needs the `run-m10.py` and `run-hook.py` snapshots beside
one another, fresh output directories, the signed bundles, the README's Aqua
session launch chain and the same no-eviction definition. The unrun three-pair
trace-off control remains required before assigning savings from phase medians.

The rebase onto `8ff734b0cc` preserves Stage 1w's results and the
Windows watchdog/log timestamp pairing in `boot_phase`, while forwarding the
same static C-string labels into the Mac trace. The probe, comparator sources
and Mac window source match the measured revision byte for byte. Incoming main
also refactors the shared glyph code around a Windows-only rasterizer; the
measured binary predates that refactor. No post-rebase timing is claimed.
The post-conflict release build passes on that base in 6m 04s
(`checks/rebase-carrier-build.log`). A subsequent clean rebase onto
`3859f6bb74` adds upstream web/agent changes; no release build or timing is
repeated after it. Final formatting and structural caps pass on that tree
(`checks/final-rebase-fmt.log`, `final-rebase-caps.log`). Windows compilation
and the prohibited local suites remain for the orchestrator.

## G7, Windows at 120 Hz

**Predictions before collection, 2026-10-31.** Source `6d501215b7`,
`lane/1149-001-win120`. The NucBox now uses DisplayPort at 5120×1440,
120 Hz; every earlier Windows row in this file was collected at 60 Hz.
The three arms use G6's release profiles, five rotating fling/rest rounds
and ten rotating fresh-process launches, with application, OS and driver
caches retained. Each entire series must satisfy G6's five-second quiet
rule: no compiler and total CPU below 10%. No build overlaps measurement.
The historical comparison changes both refresh rate and source revision;
it does not isolate a refresh-only effect.

| Prediction | Instrument and criterion |
| --- | --- |
| D3D11 and egui reach 116–120 displays per gesture second; Ink/wgpu still has no app-owned present records. | PresentMon 2.6.0, G6's four one-second gesture windows, five rounds. |
| D3D11 and egui have at most 1 late interval/s and p99 at most 12.5 ms. | Display timestamps; late means strictly over 12.5 ms, 1.5 periods at 120 Hz. |
| Fling process/main CPU roughly doubles: D3D11 168/125, Ink/wgpu 243/224, egui 50/50 ms/s, each within 25% of that estimate. | G6's process and HWND-owner-thread CPU endpoints over ten seconds. Windows accounting remains quantized in 15.625 ms ticks. |
| Peak working set stays within 10% of 153/325/182 MiB for D3D11/Ink-wgpu/egui. | G6's lifetime peak working set at the end of rest. |
| Cold first screen stays within 10% of G6e/G6 medians: D3D11 161 ms DWM / 178 ms DXGI, Ink/wgpu 716 ms DWM, egui 249 ms DWM. | Ten fresh processes per arm; G6's DWM proxy and D3D11's DXGI endpoint. The other arms expose no DXGI startup endpoint. |
| Painter snapshots equal successful D3D11 frames. | Existing exit counters, including startup. |

The carrier's reservation uses DirectComposition's reported refresh rational
and rounds one period up to milliseconds: 9 ms at 120 Hz. A missing or invalid
rate falls back to 60 Hz, but a signalled waitable releases the reservation
earlier. Its lifecycle does not impose a 60 Hz cap. No carrier change is planned. `windows-summary.py --refresh-hz 120`
uses the correct 12.5 ms late threshold for displayed and posted intervals;
its default remains 60 Hz for reproducing G6. This analysis change predicts
no change to cadence, CPU, memory or startup observations.

The first two mode diagnostics each recorded 683 focus losses: Settings
retained foreground ownership after the DPI journeys, and minimizing it did
not grant the driver foreground permission. Both are excluded from the
scoreboard. Before the third diagnostic, the driver adopts the native field
journey's `AttachThreadInput` fallback and asserts foreground ownership before
its timed endpoints. Prediction: zero focus losses and 116–120 displays/s
on egui, with no change to the carrier or the startup endpoint. The first two
captures remain under `target/g7/mode-presentmon{,-2}/` on the NucBox.


The third focus diagnostic also fails, at the new foreground assertion
before any timed fling endpoint. The zero-focus-loss prediction is a miss.
The three-round repair limit ends this problem; the fallback is removed
from the committed driver and retained as `target/g7/failed-focus-driver.py`.
No five-round fling series is claimed. Fresh-process startup uses its
existing readiness endpoints and does not depend on the fling focus check.
The first diagnostic's 8.3374 ms median displayed interval establishes that
the compositor presents egui at 120 Hz; its 683 focus losses exclude its
117.88 displays/s, CPU and memory observations from the scoreboard.


### G7 results beside the 60 Hz rows

All 30 fresh-process launches pass: 34 five-second quiet observations,
maximum total CPU 8.5%, no compiler, no failed launch. All four cold-first-screen
predictions are **met**. Application, OS and driver caches are retained;
“cold” here means a fresh process, not evicted caches. Values are medians
[min–max], milliseconds, ten processes per arm.

| Endpoint | 60 Hz, G6e / G6 | 120 Hz, G7 | Prediction |
| --- | ---: | ---: | --- |
| D3D11 DWM proxy | 160.78 [151.81–171.37] | 161.18 [146.55–175.67] | 161 ±10%: met |
| D3D11 DXGI display | 177.66 [170.41–182.54] | 167.03 [153.72–181.89] | 178 ±10%: met |
| Ink/wgpu DWM proxy | 716.21 [706.02–736.88] | 723.48 [687.45–834.75] | 716 ±10%: met |
| egui DWM proxy | 248.76 [240.70–255.15] | 239.05 [229.51–263.04] | 249 ±10%: met |

Ink/wgpu and egui still expose no DXGI first-display endpoint. D3D11's ten
startup processes total **38 painter snapshots / 38 successful frames**;
the invariant holds for these launches, and remains untested for five flings.
A higher refresh rate shortens a display period, but source also changed;
these historical distributions do not isolate the cause of endpoint changes.

No five-round 120 Hz fling cohort passed focus qualification. The rows below
preserve the historical comparison and show the missing measurements instead
of filling them with unfocused diagnostic values. “Uncollected” applies to
both the five-round median and range; all fling CPU, memory, late-interval
and cadence predictions remain **untested**, not passing or numeric misses.

| Arm / refresh | Displays / gesture s | Late intervals / s | p99 interval, ms | Process / main CPU, ms/s | Peak working set, MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| D3D11, 60 Hz (G6e) | 58.15 | 0 | 16.85 | 84.08 / 62.31 | 153.18 |
| D3D11, 120 Hz (G7) | Uncollected | Uncollected | Uncollected | Uncollected | Uncollected |
| Ink/wgpu, 60 Hz (G6) | No app-owned presents | Unavailable | Unavailable | 121.41 / 112.02 | 324.75 |
| Ink/wgpu, 120 Hz (G7) | Uncollected | Uncollected | Uncollected | Uncollected | Uncollected |
| egui, 60 Hz (G6) | 57.86 | 0 | 16.88 | 24.91 / 24.91 | 181.74 |
| egui, 120 Hz (G7) | Uncollected | Uncollected | Uncollected | Uncollected | Uncollected |

The 60 Hz late threshold was 25 ms; G7's is 12.5 ms. The carrier is unchanged.
Its reservation reads the composition refresh rate, with 60 Hz only as an
invalid-rate fallback; it is not hard-coded to a 60 Hz reservation. This
collection does not establish D3D11's actual 120 Hz reservation cadence.
Foreground acquisition is the collection blocker, and QUEUE retains it.

### G7 reproduction and qualification

Measured source: `6d501215b72929ea24620614691dee7f86192735`.
The source archive and `target/g7/preparation.patch` reproduce the build:
the only build adjustment adds the root's existing `accesskit_windows`
patch to the standalone heavy-list workspace. No Rust or carrier source
changes. The 120 Hz summary change is analytical only; the retained synthetic
16.6667 ms interval is late at 120 Hz and on time at the default 60 Hz.
The original and updated default summaries agree on all 30 startup samples;
the local 120 Hz summary equals the NucBox summary.

The own mirror is
`C:\Users\Charlie Cheever\projects\exact2-1149-001-win120`.
Its `target/g7/` retains the exact build scripts, logs, binary hashes,
mode enumerations, driver snapshots, three diagnostic captures and CSVs,
startup JSON/quiet logs, native test outputs, DPI captures and UIA tree.
The first two diagnostics are excluded for 683 focus losses each; the third
fails foreground acquisition before the timed endpoints. They are not quiet
series and cannot supply the missing CPU or memory rows.

The pre-paint evidence is retained locally as `target/g7/pre-paint-evidence.zip`
(SHA-256 `c3b929d7c54b06d22b17c4e7375c3bb20fdc29db5207ff07d731004434fae901`)
and extracted at `target/g7/nucbox/`. The source archive remains separate;
no cited raw file is deleted. Reproduce the qualified table without an app:

```sh
python3 bench/heavy-list/egui/windows-summary.py \
  target/g7/nucbox/series-1 --refresh-hz 120
```

To recollect after foreground qualification, use the existing console-session
launch chain, `target/g7/series.ps1` with a new output name, and the same five
rotating fling rounds plus ten rotating startup rounds. The 800×800 outer
windows run at 100% scale. The G7 build uses G6's release profiles and
PresentMon 2.6.0; the native qualification uses separate `gpu-dev` executables.
[Stage 1w's desktop results](../../../host/ink/MEASUREMENTS.md#stage-1w-desktop-qualification-at-120-hz--results)
include both devices' five allowed journeys, RichEdit at both scales,
wgpu surfaces, composition and startup fixtures. Sleep/wake is deliberately
skipped, and UIA discovery fails once with its full rejected tree retained.


The last, isolated wgpu paint attempt passes **56 tests, zero failures,
one opt-in measurement ignored**, with `amdihk64.dll` loaded in both the
preflight Shelf arm and the pinned process. The prior crash does not recur;
no new application fault event is captured, and no AMD setting changes.
The complete final archive is `target/g7/final-evidence.zip`, SHA-256
`f7609d5a6b970047f5085a2f79e583d40dc358aeb5f0dc00c0450c1caf96289c`.
The summary's standalone reproduction checks are in
`target/g7/summary-check/verification.txt` and `target/g7/summary-reproduction.txt`.

Structural caps pass with `EXACT_CAPS_WASM=0` (the prohibited implicit Mac
wasm build is skipped), and `git diff --check` passes. The NucBox release
and native builds pass; no Mac six-check gate is claimed.

The clean rebase onto `c83a5c1d97` brings in kernel table layout/style/schema
changes. These rows and native results remain attached to measured source
`6d501215b7`; no post-rebase performance or native qualification is claimed.

## A7, measured and parked

A7 (`parked/a7-glyph-workers`, `15b1735142`) bought no cold-start saving.
Its first paint still issued 250 uploads: summed staging waits were 27.205 ms,
against 3.094 ms of pure swash work. First paint changed from 33.676 to
31.624 ms; qualified cold medians were 241.452 and 242.751 ms. One fling
sample used 3.2% more CPU and one bar sample 6.9% more; bar peak footprint
rose 6.109 MiB. The worker amendment is not adopted. The full report remains
on that branch and every raw file remains in this worktree's `target/a7/`.

## M10, cut 7: the atlas uploaded once

Predictions recorded before implementation and collection, 2026-10-31.
Baseline: main at `4ee50baec5`, plus the benchmark workspace's required
AccessKit patch in both builds. No M10 startup cuts or A7 workers are included.
One paired session on the dedicated silver M5 Max, retained caches, GPU hook
and power sampling off; first launches after signing excluded and retained.
Each launch is qualified separately under §4's quiet, focus, viewport and
thermal rules. Failed launches remain counted; maxima are report-only.

| Prediction | Instrument |
| --- | --- |
| First-paint uploads about 27 → 2 ms; first paint about 31 → 8 ms | Five traced launches per carrier; upload CPU spans and GPU copies reported separately |
| Complete-frame commit about 20 ms earlier | The same traces, process start to first complete-scene commit |
| Cold complete scene 15–20 ms earlier, about 220 → 200 ms without M10's other cuts | `measure.m` cold-only, ten launches per arm, four-arm before and after |
| Fling CPU unchanged within 5%, with upload work lower as about 70 new glyphs arrive | Five posted fling rounds per carrier, counters and cycles/instructions beside CPU |
| Rest, bar and caret CPU unchanged within 5%; warm atlas uploads zero | One sample per carrier and workload, same probe and counters; single samples cannot establish a distribution |
| Peak footprint changes by at most 2 MiB | The same probe's phase peaks |
| Glyph masks and rendered pixels remain identical | Atlas byte comparison, pinned and text suites on Metal and wgpu |

The rest sample is fling round 1's rest interval for each carrier. The driver
also records rest after rounds 2–5; those intervals remain in the raw files.

The six-check gate is assigned to the Mac mini. This lane runs the affected
suites, formatting and staged caps locally. Raw files for this cut are retained
under `target/m10-cut7/`; `target/a7/` is unchanged.

The glyph atlas retains the padded mask bytes in a CPU page and tracks their
dirty bounding rectangle. Alpha and color pages flush together before a pass
samples them. Metal encodes one blit command with one copy per dirty page;
wgpu uses one mapped staging buffer and one copy encoder. D3D11 keeps
`UpdateSubresource` in queue order, once per dirty page; its runtime owns the
copy, with no application staging ring to combine. Paths, images, shelf
placement, page pins and swash output are unchanged.

Metal has three lazy staging slots, covering the previous, submitted and
encoding frames. A cold fill allocates one slot large enough for every dirty
rectangle, with 256-byte row alignment. A busy slot retires behind its own
submission and receives fresh storage; no glyph insertion waits for staging.
Later small batches replace a slot larger than the greater of 64 KiB and the
batch's next power of two; idle trimming releases all three. CPU shadows cost
256 KiB per normal alpha or color page, bounded by the existing sixteen-page
ceiling per atlas; overflow shadows are released after submission. Intermediate
layer draws must flush before sampling: a later pass that discovers new glyphs
can issue another batch. The trace and counters report those batches too.

The single collection is `silver/session-3/`: 80 successful cold launches
from 82 attempts, ten traces, ten flings, and one bar and caret sample per
carrier. Rest uses fling round 1, as predicted. Two egui readiness failures
(exit 2, before round 7 and after round 5) are retained; each retry succeeds.
After cold round 4 and after fling rounds 3 and 5 fail their individual quiet
gates. All other successful timed launches qualify. Thus cold has 10 qualified launches
per before arm and 9 per after arm; traces have 5/5, fling 5/3, and each
static/rest pair 1/1. The session-wide quiet flag is false; it is not the
per-launch gate. Maxima below are report-only. Hook and power sampling are off.

Cold complete-screen milliseconds, median [minimum–maximum]; n is qualified /
all attempts, including the retained readiness failures:

| Arm | Before | n | After | n |
| --- | ---: | ---: | ---: | ---: |
| Native | 256.146 [227.947–285.701] | 10/10 | 253.071 [240.206–281.900] | 9/10 |
| GPUI | 211.635 [188.874–231.867] | 10/10 | 218.309 [202.823–229.436] | 9/10 |
| Ink | 253.124 [204.693–265.480] | 10/10 | 215.080 [191.030–231.964] | 9/10 |
| egui | 223.185 [197.780–248.128] | 10/11 | 214.707 [192.838–248.247] | 9/11 |

Ink's difference of qualified medians is **38.044 ms**. All nine matched
Ink pairs improve: median paired saving **32.862 ms**, range 12.506–48.511.
The candidate sits between GPUI and egui's after medians; this cut establishes
no win over egui and leaves the 100 ms application budget unmet.

Each of the five traces per carrier has 321 raster-miss spans. The before
carrier uploads 307 nonempty masks, compared with A7's older 250-upload arm.
The candidate uses **one batch, one 223,744-byte (218.5 KiB) staging buffer,
and two copies**, one per dirty alpha/color page, on every traced first frame.
The CPU shadows for those two normal pages total 512 KiB. Five ordinary
texture uploads remain in both arms.

| Trace measurement, ms unless counted | Before median | After median |
| --- | ---: | ---: |
| First paint walk | 31.183 | 4.953 |
| Following Ink finish | 0.169 | 0.250 |
| Process start to complete-scene commit | 197.734 | 169.972 |
| Pure swash spans | 2.555 | 2.023 |
| Staging waits contained in first paint/finish, all uploads | 27.170 | 0.686 |
| Full glyph-upload elapsed spans | 34.464 | 0.0525 |
| Full glyph staging waits | 31.106 | 0 |
| Summed glyph GPU copy commands | 5.037 | 0.0337 |

The before arm's 307 upload commands average **101.323 μs staging wait,
10.247 μs CPU staging/copy/encoding, and 16.408 μs GPU copy** per mask
(medians of each launch's per-mask means). There are 305 staging waits;
the first two slots are fresh. Full glyph-upload elapsed time averages
112.260 μs per mask. The one after batch costs **52.500 μs** on its owner:
29.833 μs staging allocation/copy, 11.083 μs blit encoding, and 33.708 μs on
the GPU for both copies. GPU time and CPU-side elapsed time overlap.

The reducer joins all 312 serial upload submission IDs to 312 ordered
worker spans, then selects the 307 glyph IDs. Every glyph command has a GPU
interval; every after batch does too. Some serial uploads finish after the
paint walk, which is why their full waits exceed the 27.170 ms contained
within the first paint/finish interval. The old encoding span combines staging
allocation, memory copy and blit encoding; main-thread command-buffer creation
is not separately timed. The new batch span starts after packing the dirty
CPU rectangles; that packing remains included in paint/finish. Its two GPU
copies share a command timestamp and cannot be timed separately.

Traced cold medians are 221.220 → 194.535 ms, below the untraced cohort's
253.124 → 215.080 ms. The phase cohort is diagnostic; its medians are not
added to the untraced cold row, and this collection does not isolate observer
cost. The earlier M10 observer-control obligation remains open.

Fling rows use qualified samples only (before n=5, after n=3), medians
[minimum–maximum]. Cadence and late intervals use the four posted gesture
windows, as in `aggregate.mjs`; CPU and work use the full ten seconds.

| Fling metric | Before | After |
| --- | ---: | ---: |
| Process CPU, ms/s | 143.501 [134.664–145.352] | 143.245 [139.002–144.637] |
| Main-thread CPU, ms/s | 72.700 [69.794–73.687] | 75.565 [74.575–76.069] |
| Cycles, million/s | 241.817 [238.365–248.351] | 240.659 [238.432–244.408] |
| Instructions, million/s | 523.456 [523.103–525.250] | 514.507 [514.015–517.103] |
| Effective GHz | 1.731 [1.671–1.770] | 1.706 [1.664–1.715] |
| Instructions/cycle | 2.163 [2.115–2.195] | 2.138 [2.116–2.156] |
| Peak footprint, MiB | 599.111 [597.548–599.876] | 596.829 [594.642–598.861] |
| Presented frames / gesture s | 89.221 [83.945–90.322] | 88.812 [88.437–89.847] |
| Late intervals / gesture s | 1.075 [0.999–1.608] | 0.528 [0.264–0.805] |
| p99 interval, ms | 16.667 [16.667–16.667] | 8.333 [8.333–8.333] |

Fling process CPU moves **−0.178%**, cycles −0.479%, and instructions −1.710%:
within the predicted unchanged band, with no substantial CPU saving established.
The candidate records 18 glyph batches and 33 page copies per fling (copy
range 33–34), uploading 570,022 bytes (570,022–578,243), about 16.9 KiB per
copy. The before hook-off counter set has no glyph-upload count; the brief's
estimate of about 70 newly discovered glyphs is not independently counted in
this series. Rest, bar and caret each record **zero** glyph batches, copies
and bytes after the cut.

Single-sample static/rest rows; clocks and work accompany elapsed CPU:

| Phase / arm | CPU, ms/s | Main, ms/s | Cycles, M/s | Instructions, M/s | GHz | IPC | Peak, MiB | Frames/s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rest / before | 2.831 | 2.694 | 4.289 | 4.079 | 1.515 | 0.951 | 247.564 | 0.000 |
| rest / after | 3.229 | 3.067 | 4.539 | 4.231 | 1.406 | 0.932 | 245.392 | 0.000 |
| bar / before | 177.266 | 91.656 | 249.218 | 433.593 | 1.406 | 1.740 | 358.735 | 119.672 |
| bar / after | 188.156 | 96.504 | 270.588 | 439.296 | 1.438 | 1.623 | 351.407 | 119.630 |
| caret / before | 30.766 | 27.755 | 43.723 | 93.766 | 1.421 | 2.145 | 355.939 | 2.000 |
| caret / after | 31.354 | 28.487 | 45.452 | 97.260 | 1.450 | 2.140 | 340.798 | 1.999 |

| Prediction | Result |
| --- | --- |
| First-paint upload spans about 27 → 2 ms | Contained staging waits 27.170 → 0.686 ms; all contained upload elapsed spans 30.402 → 2.125 ms. Glyph waits are zero and the batch itself is 0.0525 ms. |
| First paint about 31 → 8 ms | 31.183 → 4.953 ms, faster than estimated. |
| Complete-frame commit about 20 ms earlier | 27.762 ms earlier, faster than estimated. |
| Cold 15–20 ms earlier, about 220 → 200 ms | The 15–20 ms estimate underpredicts both the 38.044 ms median-row change and 32.862 ms paired saving. The absolute 200 ms estimate misses: after is 215.080 ms, from 253.124. |
| Fling CPU within 5% | −0.178%; met, with n=5/3 after quiet qualification. |
| Rest, bar and caret CPU within 5% | Rest +14.084% and bar +6.143%: **misses**. Caret +1.911%: met. These are single samples; the rest/bar changes remain unattributed despite zero glyph uploads. |
| Peak footprint changes by at most 2 MiB | Literal ±2 MiB prediction **missed toward lower footprint**: fling −2.281, rest −2.172, bar −7.328, caret −15.141 MiB. No measured peak increases. |
| Bit-for-bit mask parity; pinned and text suites | Met on Metal and wgpu; validation below. |

No performance tuning followed this initial collection. The review-fix
paired collection below resolves its rest/bar follow-up; the original
single-sample predictions remain misses.

Reproduction uses the retained signed bundles and
`target/m10-cut7/silver/{provenance.json,sources.json,source-before.patch,source-after.patch}`.
The main base is `4ee50baec5`; before is instrumented `b7c52c6d22`, after is
`75ac7d413b`. The trace additions are common to both. Native is `d16c84ee03`,
GPUI and egui `a91fd38b4d`, with the same probe in all five bundles. All
executable/library and instrument hashes match the silver provenance.
`target/m10-cut7/measured-source.bundle` retains the pre-rebase commits. The remote original
is `/Users/ccheever/projects/exact2-measure/target/g0/m10-cut7-4ee50baec5/`;
the local signed mirror and every raw attempt are in `target/m10-cut7/silver/`.

Preparation attempt 1 uses a LaunchAgent and fails Accessibility preflight
before any app starts. Attempt 2 is stopped before the gate because its
relative script name matches its own ownership filter. Both remain retained;
neither has a timed app sample. The sole timed collection, session 3, uses the
README's existing SSH grant and Aqua chain, with an absolute script path;
no permission settings change. It waits for the M10 fix collector to finish.

```sh
EXACT_APP_DIR="$PWD/bench/heavy-list/exact-heavylist" \
EXACT_PAINTED_PROBE="$PWD/bench/heavy-list/gpui/probe.m" \
CARGO_BUILD_JOBS=6 bun host/apple/build.mjs exact-heavylist-apple --painted-benchmark --release
# The standalone workspace builds into this checkout's target/.
python3 target/m10-cut7/silver/analyze-cut7.py target/m10-cut7/silver 3
bun bench/heavy-list/gpui/aggregate.mjs \
  target/m10-cut7/silver/session-3/fling/after/series/fling/ink-1.json --sample
```

The retained runner specifies the alternating order, first-launch exclusions,
per-launch quiet window and all commands.
`target/m10-cut7/silver/session-3/analysis.json` carries every qualification
reason and trace join; `target/m10-cut7/analysis-final.json` retains its printed
metrics. The committed `aggregate.mjs --sample` independently checks all
fourteen motion samples' phase deltas and hook state. Its instrument-only
output does not qualify the two excluded flings.

Validation: the new GPU readbacks compare serial and batched padded masks
byte for byte on Metal and wgpu, including an overflow page and a later
small dirty rectangle. A delayed-device fixture inserts 250 masks before any
upload, checks one copy per dirty page, preserves a geometry-only walk,
and checks warm zero-upload and pending-upload reuse. Pinned, text and image
suites pass before and after rebasing onto `fd89454bd3`; the atlas, device,
glyph and Ink adapter sources match measured `75ac7d413b` byte for byte.
The incoming table-layout changes are included in the post-rebase tests,
not in the retained performance bundles.

Test commands use `EXACT_REQUIRED=1 RUST_TEST_THREADS=2 CARGO_BUILD_JOBS=6` and
`EXACT_INK_DEVICE=metal` or `wgpu`. Full stdout and stderr are in
`target/m10-cut7/checks/`. Result lines below are verbatim; Linux-host lines
are the post-rebase run.

```text
cargo test -p exact-ink --lib --no-fail-fast
test result: ok. 50 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.95s

EXACT_INK_DEVICE=metal cargo test -p exact-linux --test pinned --no-fail-fast
test result: ok. 104 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 9.56s

EXACT_INK_DEVICE=metal cargo test -p exact-linux --lib text:: --no-fail-fast
test result: ok. 217 passed; 0 failed; 0 ignored; 0 measured; 1499 filtered out; finished in 2.88s

EXACT_INK_DEVICE=metal cargo test -p exact-linux --test it image --no-fail-fast
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 130 filtered out; finished in 2.42s

EXACT_INK_DEVICE=wgpu cargo test -p exact-linux --test pinned --no-fail-fast
test result: ok. 104 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 10.19s

EXACT_INK_DEVICE=wgpu cargo test -p exact-linux --lib text:: --no-fail-fast
test result: ok. 217 passed; 0 failed; 0 ignored; 0 measured; 1499 filtered out; finished in 2.76s

EXACT_INK_DEVICE=wgpu cargo test -p exact-linux --test it image --no-fail-fast
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 130 filtered out; finished in 2.35s

cargo check -p exact-ink --target x86_64-pc-windows-msvc --tests
warning: exact-ink@0.1.0: exact-ink: fxc is not on PATH; the Direct3D 11 device will refuse to open
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 33s
```

Windows compilation passes; D3D11 runtime parity is untested on this Mac,
whose missing `fxc` prevents opening that device. The two initial wgpu
compilation failures and their corrected third run remain in `ink.log`,
`ink-2.log` and `ink-3.log`. The six-check gate remains assigned to the Mac mini.

Formatting, staged structural caps and `git diff --check` pass. Caps uses
`EXACT_CAPS_WASM=0`; the implicit wasm build stays with the Mac mini gate.

### Cut 7 review fixes: predictions and accounting

Recorded before the runs on 2026-10-31, from `d008a448c7`.
The page-visit counter predicts 32 visits at `begin` and 96 across three
clean flush boundaries with sixteen pages of each glyph kind, then zero
after the dirty-page list. Both paths should upload zero bytes. Five paired,
interleaved rest and bar rounds on the silver, hook and power sampling off,
should have median paired CPU changes within ±5% and ranges crossing zero;
one caret pair should remain within ±5%. Warm glyph uploads should stay zero.
CPU rows retain cycles, instructions, effective GHz and IPC. Predictions,
commands and raw output are retained under `target/m10-cut7-review/`.

`Stats.bytes` now includes each retained CPU page shadow, including an
unflushed overflow shadow. Overflow GPU storage remains in `overflow_bytes`;
GPU staging, map metadata and allocator overhead are outside these logical
atlas-byte counters. The normal ceiling is 8 MiB GPU plus 8 MiB CPU across
both glyph atlases, sixteen pages each. D8 now names those shadows.
`Stats.uploaded_bytes` counts dirty-rectangle bytes at flush for batched
atlases, including old pixels in the bounding union, and padded entry bytes
at insertion for serial path/ramp atlases. It excludes device row alignment.
The kept-rows series (`host/linux/src/presenter/kept_rows_tests/measurement.rs`,
`INK_RESULT counters[2]`) sums that field: its pre-cut mask-byte values in
`host/ink/MEASUREMENTS.md` cannot be compared directly with post-cut rectangles.

The baseline counter measures **32 begin visits and 96 flush visits**, with
zero uploads. Sol's two scans per single-flush frame are confirmed; three
boundaries make four scans. The dirty-page list measures **zero visits** at
both sites and zero uploads. Pending geometry-only masks prevent eviction
at insertion; no protection scan runs at begin. Existing overflow retirement
and glyph-cache bookkeeping are unchanged. The diagnostic source delta is
`scans-before.patch`; its output is `checks/scans-before.log`.

The fixes are `b1b0e9c993`. Metal validates and retains every destination
texture before opening the blit encoder, allocates at least 64 KiB per staging
slot, and returns a recorded submit error before the atlas clears dirty state.
The ordered fake queue runs the adapter's intermediate group, opacity,
backdrop and later-mask pass boundaries; each new mask uploads before its draw.
Real Metal and wgpu readbacks cover a batch spanning two pages of one atlas
kind, including both new padded regions and earlier masks. A blocked Metal
worker holds four batches in one frame: the fourth replaces the busy slot
without waiting; completion drains retired buffers; larger sub-64-KiB batches
reuse all three slots; trim releases them. The sixteen-page counter fixture
reports **16,777,216 retained bytes**, including both 8 MiB halves.

| Review finding | Disposition |
| --- | --- |
| Sol 1; Claude 5, CPU shadows | Counted in `Stats.bytes`; D8 amended; normal ceiling and overflow release asserted. |
| Sol 2; Claude warm path | Dirty-page list; measured visits fall 128 → 0 for one begin and three flushes. |
| Sol 2; Claude rest/bar | Five qualified pairs each; CPU medians within ±5%, both paired ranges cross zero. Extra bar timer/dispatch work is counted below. |
| Sol 3; Claude 9, coverage gaps | Ordered adapter passes, same-kind two-page readbacks, four busy Metal batches, reuse and trim pass. |
| Claude 2, failed Metal submission | Return the recorded error; dirty rectangles remain pending. |
| Claude 4, upload-byte meaning | Field comment and kept-rows qualification above. |
| Claude 6, small staging allocations | 64 KiB floor, with slot-reuse assertions. |
| Claude 8, open encoder on error | Destination textures collected before opening the encoder. |
| Claude 3 and 7, shelf-wrap union and double copy | Queued with the 16.9 KiB/copy and 29.8 μs staging observations; no optimization claimed. |
| Claude 1; Sol 4, device parity | Vulkan passes at `b1b0e9c993`; the separate Windows lane passes D3D11/wgpu at its frozen `d008a448c7`. The follow-up changes have not run on Windows. |

### Validation after review

The local suites ran the sources in `b1b0e9c993`. The Linux adapter runs use
`CARGO_BUILD_JOBS=6` and `EXACT_REQUIRED=1`; the Metal/wgpu parity runs also
set `RUST_TEST_THREADS=2`.
The ink library's `--nocapture` output also retains the scan and memory
counter measurements. Logs are under `target/m10-cut7-review/checks/`.

```text
cargo test -p exact-ink --lib --no-fail-fast -- --nocapture
test result: ok. 52 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.28s

cargo test -p exact-linux --lib ink:: --no-fail-fast -- --nocapture
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 1691 filtered out; finished in 0.37s

EXACT_INK_DEVICE=metal cargo test -p exact-linux --test pinned --test it --lib --no-fail-fast
test result: ok. 1695 passed; 0 failed; 22 ignored; 0 measured; 0 filtered out; finished in 53.95s
test result: ok. 133 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.09s
test result: ok. 104 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 7.50s

EXACT_INK_DEVICE=wgpu cargo test -p exact-linux --test pinned --no-fail-fast
test result: ok. 104 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 7.74s

EXACT_INK_DEVICE=wgpu cargo test -p exact-linux --lib text:: --no-fail-fast
test result: ok. 217 passed; 0 failed; 0 ignored; 0 measured; 1500 filtered out; finished in 2.42s

EXACT_INK_DEVICE=wgpu cargo test -p exact-linux --test it image --no-fail-fast
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 130 filtered out; finished in 1.81s
```

Both release carriers build locally with the same probe and own `target/`.
The main carrier uses `fd89454bd3`'s runtime files, checked byte for byte
before the build; the after carrier uses `b1b0e9c993`. Their build logs,
signed bundles and source checks are retained with the measurements.
The six checks remain assigned to the Mac mini; this lane runs package
formatting, `git diff --check` and staged structural caps with
`EXACT_CAPS_WASM=0`.

### Vulkan and Windows parity after review

Claude 1's Vulkan gap is closed at `b1b0e9c993`. The tracked sources were
rsynced to `ccheever@100.85.211.16` (`expose-builder-pixel-a`), into its own
`~/projects/exact2-wt-1149-raster`, and every changed runtime file was checked
against the local SHA-256 manifest. The builder ran Rust 1.97.0 and Bun 1.4.2.
Mesa lavapipe 25.2.8 was extracted under that mirror's
`target/m10-cut7-review/lavapipe/`; `VK_DRIVER_FILES` names its ICD and absolute
library path. No system driver or other checkout was changed.

The commands use `CARGO_BUILD_JOBS=6`, `RUST_TEST_THREADS=2`,
`EXACT_REQUIRED=1`, `EXACT_INK_DEVICE=wgpu`, `WGPU_BACKEND=vulkan`,
`EXACT_PAINTER=ink` and this mirror's `--target-dir target`:

```text
cargo test -p exact-ink --lib --target-dir target --no-fail-fast
test result: ok. 31 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.52s

EXACT_PAINTER=ink cargo test -p exact-linux --test pinned --target-dir target --no-fail-fast
test result: ok. 104 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 30.18s
```

The first pinned build stopped before running tests because the fresh mirror
lacked Rolldown. `bun install --frozen-lockfile` supplied it; the second build
and run passed. Both attempts, exit statuses, driver scripts and provenance
are retained locally in `target/m10-cut7-review/vulkan/`, with the original
driver files on the builder. `vulkan-source-check.log`, `source-sha256.json`
and `lavapipe-setup.log` beside that directory record the source and driver
checks. Reproduction starts with the tracked-file rsync and frozen install,
then runs `run-vulkan.sh` with the pinned tools first on PATH. The script
sets the private ICD and runtime paths explicitly. These are headless Vulkan
pixel/readback checks; they do not qualify a native Linux window or input.

D3D11 was not repeated. `lane/1149-001-atlas-win` committed its record as
`2851b41a15`, in `host/ink/MEASUREMENTS.md`, “M10 cut 7 on Windows”. Its
frozen candidate is **`d008a448c7`**, before these review fixes. D3D11 and
wgpu pass the original batch readback, pinned, text, image and all five
desktop journeys each. The new two-page test and accounting changes at
`b1b0e9c993` have not run on Windows. This closes the original cut's D3D11
qualification gap, without claiming a Windows run of the follow-up.

Verbatim parity lines from that committed record:

```text
test atlas::upload::tests::d3d11_batches_identical_masks_and_padding_then_only_the_new_rectangle ... ok
d3d11-ink: test result: ok. 52 passed; 0 failed; 34 ignored; 0 measured; 0 filtered out; finished in 4.83s
d3d11-pinned: test result: ok. 103 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 27.17s
d3d11-text: test result: ok. 216 passed; 0 failed; 1 ignored; 0 measured; 1420 filtered out; finished in 3.84s
d3d11-image: test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 130 filtered out; finished in 2.66s
wgpu-ink: test result: ok. 52 passed; 0 failed; 34 ignored; 0 measured; 0 filtered out; finished in 5.19s
wgpu-pinned: test result: ok. 103 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 57.05s
wgpu-text: test result: ok. 216 passed; 0 failed; 1 ignored; 0 measured; 1420 filtered out; finished in 3.98s
wgpu-image: test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 130 filtered out; finished in 2.58s
```

The record and result lines are copied to
`target/m10-cut7-review/windows-measurements.md`; its source/provenance and
raw evidence remain with that lane in `target/m10-cut7-windows/`. Its two
startup collections fail the quiet rule and its third attempt stops at
preflight, so no Windows startup saving is imported here.

### Paired rest, bar and caret after review

The silver's M10 collector released the display before this collection.
The existing Aqua launch chain ran main `fd89454bd3` against `b1b0e9c993`,
with the same signed probe, `G1_GPU_HOOK=0`, `EXACT_INK_COUNTERS=1`, direct
Metal and no power sampler. Two first launches after signing are retained
and excluded. Five rounds alternate the before/after order; bar reverses
that round's fling order. Rest is each fling's rest phase. Caret runs once
per carrier. All accepted rows have an 800×800-point outer window,
800×768-point content, 2× scale, 120 Hz display, focus, normal thermal state
and no low-power mode.

All drivers succeed. Three launches in session 1 fail the per-launch quiet
rule: after flings 4 and 5, and before bar 4. The observations record
`mediaanalysisd`/`launchd`, `duetexpertd`, and `agy` bursts respectively.
Their samples remain retained. Session 2 repeats both members of those
three pairs, with unchanged binaries and settings; all six replacements
qualify. Selection takes the first qualified pair for each phase and round,
regardless of its CPU value. The session-wide quiet flag remains false;
qualification covers each selected launch and its preceding sixty seconds.
There are five qualified rest pairs, five bar pairs and one caret pair.

Process CPU milliseconds/second, before → after, with each paired change:

| Round | Rest | Bar |
| --- | ---: | ---: |
| 1 | 3.134 → 3.161 (+0.882%) | 185.267 → 185.730 (+0.250%) |
| 2 | 3.137 → 3.150 (+0.408%) | 187.893 → 199.361 (+6.104%) |
| 3 | 3.184 → 3.055 (−4.033%) | 188.068 → 192.385 (+2.295%) |
| 4, replacement | 3.181 → 3.044 (−4.315%) | 187.129 → 183.094 (−2.156%) |
| 5 | 3.107 → 3.090 (−0.541%; replacement) | 183.119 → 190.114 (+3.820%) |

Qualified arm medians; GHz and IPC are calculated per sample before taking
their medians, so ratios of the displayed median columns need not agree:

| Phase / arm | CPU, ms/s | Main, ms/s | Cycles, M/s | Instructions, M/s | GHz | IPC |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rest / before | 3.137 | 3.006 | 4.482 | 4.274 | 1.408 | 0.970 |
| rest / after | 3.090 | 2.926 | 4.441 | 4.241 | 1.437 | 0.957 |
| bar / before | 187.129 | 95.857 | 268.796 | 437.597 | 1.436 | 1.621 |
| bar / after | 190.114 | 98.268 | 269.929 | 442.109 | 1.426 | 1.676 |
| caret / before | 29.974 | 26.999 | 41.993 | 92.666 | 1.401 | 2.207 |
| caret / after | 31.330 | 28.229 | 43.822 | 96.868 | 1.399 | 2.210 |

| Prediction | Result |
| --- | --- |
| Clean begin plus three flushes: 128 page visits → 0; zero uploads | Met: 32 + 96 → 0 + 0; zero uploads in both counter runs. |
| Normal GPU pages plus CPU shadows: 8 + 8 MiB at sixteen pages per kind | Met: 16,777,216 bytes. Overflow shadows count until successful flush. |
| Two same-kind page copies/readbacks; four busy Metal batches; retirement and trim | Met on the local devices; the shared readback also passes on Vulkan. |
| Rest median paired CPU within ±5%, range crossing zero | Met: −0.541%, range −4.315% to +0.882%; median paired delta −0.017 ms/s. |
| Bar median paired CPU within ±5%, range crossing zero | Met: +2.295%, range −2.156% to +6.104%; median paired delta +4.317 ms/s. |
| One caret pair within ±5% | Met: +4.523%, +1.356 ms/s. One pair establishes no distribution. |
| Warm glyph batches, copies and bytes remain zero | Met in every selected candidate rest, bar and caret interval. Main's older counter set lacks these fields. |
| Local Metal/wgpu and Vulkan parity | Met; verbatim results above. Windows' original cut also passes on its separate lane. |

**Variance under the requested paired-range test:** both process-CPU ranges
cross zero, so neither earlier single-sample increase is reproduced as a
consistent regression on the revision with these fixes. The old +14.084%
rest and +6.143% bar predictions remain recorded misses. These are observed paired
ranges, not confidence intervals, and do not establish zero cost. In
particular, paired bar instructions rise 0.933% median, range +0.513% to
+15.728%, while paired effective GHz falls 1.112% median.

The largest bar pair has 40 → 2,460 timer service passes and
3,566 → 4,711 dispatch-only passes over ten seconds; another has
15 → 802 timer passes. Both retain 1,200 paint attempts and zero candidate
glyph uploads. The counted path is
`host/apple-painted/src/window.rs::new_events/about_to_wait → service`
(`ResumeTimeReached`, then `advance`, `pump`, commands and image polling).
This names the extra work accompanying the spread; the trigger for those
intermittent timer wakes is not isolated by these counters. Rest records
zero paint attempts and presentations in both carriers; caret presents
twenty frames in each sample.

Retained evidence is `target/m10-cut7-review/silver/`, mirrored from
`/Users/ccheever/projects/exact2-measure/target/g0/m10-cut7-review-b1b0e9c993/`.
`session-1/` and `session-2/` contain every attempt, ready record, driver log,
schedule, five-second quiet observation and power configuration.
`combined/selection.json` names the pairs; `combined/paired.json` contains
their phase deltas and summaries. Both signed carriers remain in the local
parent directory and on the silver. `provenance.json` records their complete
file hashes and the shared probe hash. `baseline-source-check.json` and
`source-sha256.json` bind the builds to their source revisions.

The retained `run-paired.py`, `run-repair.py` and `repair-plan.json` specify
launch order, ownership checks, quiet gates and replacement criteria.
Recompute from this worktree:

```sh
python3 target/m10-cut7-review/analyze-paired.py target/m10-cut7-review/silver 1
python3 target/m10-cut7-review/analyze-paired.py target/m10-cut7-review/silver 2
python3 target/m10-cut7-review/combine-paired.py target/m10-cut7-review/silver session-1 session-2
python3 target/m10-cut7-review/summarize-paired.py target/m10-cut7-review/silver/combined
bun bench/heavy-list/gpui/aggregate.mjs target/m10-cut7-review/silver/session-1/bar/after/series/ink-2.json --sample
```

The committed aggregator independently checks all 22 selected source samples'
phase deltas and hook state; `aggregate-selected.log` retains its output.
No trace or power sample enters these CPU rows, and no other startup cut is
included. The earlier cold-row and observer-control limitations still stand.
Cut 7's [Windows qualification](../../../host/ink/MEASUREMENTS.md#m10-cut-7-on-windows)
passes Ink, pinned, text, image and five desktop journeys on both D3D11 and
wgpu, including D3D11's atlas byte comparison. No startup saving is qualified:
two S2 before/after collections violate the quiet rule and the third attempt
expires in preflight. Windows upload spans remain unmeasured.
## M10, cut 1: prepare the first frame before showing the window

**Predictions, before the runs, 2026-10-30.** The cut starts the existing
paint/encode/submit path after layer attachment, before the existing show
call. It preserves the compose target, host-thread encoding, worker submission
and retry-until-displayed rule. The system's roughly 30 ms commit-to-display
interval is outside this cut.

| Prediction | Instrument |
| --- | --- |
| Warm complete screen 217.9 → about 198 ms, a 20 ms saving | Ten interleaved cold/warm pairs for Ink before/after, GPUI and egui, `startup.py`, hook off, trace on, power off; positive complete drawable timestamps |
| Scoreboard cold row falls by the same 20 ms (G4m's 250 → about 230 ms; the contemporaneous before row is the control) | `measure.m` cold-only, ten launches of each of native, GPUI, Ink and egui before and after, hook and power off |
| First paint duration unchanged (about 29.4 ms) | The same trace's first `paint walk` span |
| Fling, rest and static rows unchanged | Existing scoreboard instruments; full series follows landing, outside this startup cut |
| First-frame pixels equal; CPU archive misses still hand off; unshown submissions keep retrying | Headless first-frame comparison, Stage 0b transition/completeness and painted-carrier suites |

The quiet amendment excludes benchmark work from the aggregate busy-core
bound: at most 1.0 non-benchmark core. The arms, driver, wheel helper, probe
helpers, caffeinate and WindowServer are benchmark work. The per-process bound,
10% transient-daemon allowance, load bound and compiler refusal stay. M10's
earlier cohorts crossed 2.0–2.1 whole-machine cores on their own launches.

### Cut results: no qualified startup row

**The cut builds and its correctness checks pass. No startup saving is
qualified.** The silver Mac's non-benchmark work crosses the amended bound
in all three before-scoreboard attempts, the paired cohort and the after
scoreboard. Repetition of the before cohort stops at three attempts. No
process belonging to another lane or the owner was stopped. The ten-pair
tables and ten-launch four-arm rows remain owed; LLP 1149 §4.1 is unchanged.

| Cohort | Original launch records / requested | Maximum non-benchmark cores | Result |
| --- | ---: | ---: | --- |
| `scoreboard-before` | 4 / 40 | 1.096 | Excluded; `searchpartyd` contributes 0.832 core |
| `scoreboard-before-2` | 4 / 40 | 1.532 | Excluded; screen sharing and macOS services |
| `scoreboard-before-3` | 4 / 40 | 1.696 | Excluded; screen sharing, fleet polling and macOS services |
| `paired-1` | 3 / 80 | 1.870 | Excluded; XprotectService, screen sharing, metadata and policy services |
| `scoreboard-after` | 4 / 40 | 1.595 | Excluded; aggregate load and an egui readiness failure |

Every cohort passed its sixty-second gate before launching. The hard
aggregate limit, not the transient per-process allowance, stops these runs.
The raw five-second observations retain the benchmark partition and the
five largest other processes. Power sampling stays off.

These are **excluded individual cold-only launches, not medians or scoreboard
replacements**, from `measure.m` with the same hook-off configuration. Each
number has one original launch; `—` is a failed original.

| Cold complete screen, ms | Native | GPUI | Ink | egui |
| --- | ---: | ---: | ---: | ---: |
| Before, attempt 1 | 635.579 | 421.591 | 615.336 | 552.744 |
| Before, attempt 2 | 236.749 | 225.495 | 250.000 | 236.424 |
| Before, attempt 3 | 253.748 | 217.659 | 260.938 | 252.562 |
| After | 245.412 | 210.795 | 247.095 | — |

The driver's egui retry reports 214.907 ms, retained separately as
`scoreboard-after/egui-1-retry.json`; it does not replace the failed original.
This exposed a wrapper bug: the original file's existence was recorded as
acceptance even when its contents said `accepted: false`. The collector now
reads that field, and the reducer rejects a failed original even with the
older incorrect schedule metadata. The retained collection snapshot and its
raw schedule are unchanged. The cohort was already excluded by load.

| Prediction | Result |
| --- | --- |
| Warm 217.9 → about 198 ms, −20 ms | Not established: before warm is 212.297 ms, but the cohort stops before after warm. The first after cold is 785.612 ms, including 559.234 ms before the probe constructor; it is retained, not substituted or called warm. |
| Cold row improves by 20 ms | Not established. The excluded last-before/after Ink samples are 260.938 → 247.095 ms, −13.843 ms: short of the prediction, and insufficient for a saving. Ink also remains slower than GPUI in those individual launches. |
| First paint stays about 29.4 ms | Excluded paired-cohort cold spans are 28.753 ms before and 28.227 ms after, consistent with unchanged paint work; no qualified phase median. |
| Fling, rest and static rows unchanged | Unmeasured in this cut; their scoreboard rows remain unchanged. |
| First-frame pixels equal | Pass, every pixel on both painters at the same frozen clock, including the unshown retry. |
| CPU archive miss hands off; unshown submissions retry | Pass in the suites and the silver's live miss and retained functional launches. |

The three retained phase traces show where the cut acts:

| Process age, ms | Before cold | Before warm | After cold |
| --- | ---: | ---: | ---: |
| Probe constructor | 5.376 | 6.140 | 559.234 |
| Layer attached | 114.395 | 112.301 | 690.995 |
| First paint begins | 150.461 | 151.901 | 692.096 |
| First paint ends | 179.214 | 181.028 | 720.323 |
| Window show returns | 115.795 | 114.377 | 725.047 |
| First present commits | 184.512 | 185.799 | 726.189 |
| Complete screen displayed | 213.707 | 212.297 | 785.612 |

Layer-to-paint falls from 36.066 ms to 1.101 ms in these two cold traces.
The after paint finishes before the show call returns. Its first
`nextDrawable` runs at 725.239–726.056 ms, before the probe observes visibility
at 742.422 ms, and succeeds: `present encode` follows. That submission gets
a zero displayed timestamp. The retained row repaint and subsequent
submissions continue until the positive complete timestamp at 785.612 ms.
Thus the cut removes the visibility/tick wait before recording; it does not
make an unshown submission count as displayed.

No nil drawable was observed in these after traces. If acquisition returns
nil, the existing worker commits for retirement and requests a retry. The
scene can already be encoded into D7's compose target; the final copy into
the drawable needs that drawable. This revision does not park an uncommitted
presentation buffer or add an acquirer. Its retained rows and compose target
survive for the existing retry. Stage 0b's CPU handoff and positive-timestamp
rule are unchanged. The roughly 30 ms system presentation interval is outside
the cut; the after trace's 59.423 ms from first commit to complete includes
the unshown submission and cannot be treated as that interval's cost.

The same three traces retain clocks through the complete callback. These
excluded process-CPU observations are not phase medians:

| Launch | CPU, ms | Instructions, million | Cycles, million | Effective GHz | IPC |
| --- | ---: | ---: | ---: | ---: | ---: |
| Before cold | 235.721 | 2411.335 | 982.103 | 4.166 | 2.455 |
| Before warm | 225.966 | 2390.059 | 943.061 | 4.173 | 2.534 |
| After cold | 234.933 | 2189.279 | 895.720 | 3.813 | 2.444 |

The separate `boot-functional` launches use `G1_BOOT_ONLY=1`, a fresh
`EXACT_CACHE` for the miss and that same archive for the retained launch.
The miss displays complete CPU at 175.758 ms and complete Ink at 342.424 ms;
its 200.302 ms pipeline compile runs off the boot thread. The retained launch
submits no CPU frame and displays complete Ink at 176.559 ms. Both begin
with an unshown submission and recover. These passive, activation-suppressed
checks are functional evidence, not replacements for the foreground startup
cohort. Their raw environment, source hash, exit status and archive are kept.

### Cut and pixel parity

`host/apple-painted/src/window.rs` calls its existing draw path after the first
layout and layer attachment, immediately before the existing `set_visible(true)`
call. `prepare_draw` shares the pre-paint service sequence with the headless
test. The presenter and Ink need no new boot API: their existing compose target,
main-thread encoding, worker submission and positive-displayed-timestamp retry
already support this order. No pipeline or glyph work moves between threads.

The headless test compares every pixel of the early frame with the previous
service-then-display-tick sequence at the same frozen clock, separately on CPU
and Ink. Its 2× fixture includes text, digits, rounded geometry and a scrolling
column; it checks nonblank output and an identical repaint after an unshown
frame. This is a deterministic fixture comparison, not a capture of two live
first screens. Stage 0b's archive-miss transition and unshown-drawable tests
remain unchanged and pass.

The benchmark workspace also gains the root workspace's required
`accesskit_windows` patch so the release carrier can build. `startup.py`
preserves variant identity in interleaved Ink-before/Ink-after cohorts and
invokes the existing `measure.m` cold-only path for the scoreboard. Its reducer
accepts native's absent drawable field and preserves startup clocks when
available. `run-hook.py`, the README and cost-program §4 carry the quiet
amendment above.

### Cut verification

Both release bundles build in this worktree's own `target/`, with the probe
linked, and are preserved separately as `target/m10-c1/InkBefore.app` and
`InkAfter.app`. The affected suites run once with Metal required:

```sh
env CARGO_BUILD_JOBS=6 RUST_TEST_THREADS=2 EXACT_INK_DEVICE=metal EXACT_REQUIRED=1 \
  cargo test -p exact-linux -p exact-apple-painted -p exact-ink \
  --lib --tests --no-fail-fast
```

Exit 0. The painted carrier, its binary target and Ink print, verbatim:

```text
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 46 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.71s
```

Linux's library and integration targets (`authored_dialog`,
`authored_dialog_windows`, `base_scale`, `bold_text`, `contrast`, `it`,
`neutral`, `neutral_alert`, `pinned`, `reported`) print, verbatim:

```text
test result: ok. 1687 passed; 0 failed; 22 ignored; 0 measured; 0 filtered out; finished in 69.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.28s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.02s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
test result: ok. 133 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.42s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.74s
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.39s
test result: ok. 104 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 8.09s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

The relevant cases print:

```text
test window::tests::an_unshown_boot_keeps_display_demand_after_two_completed_drawables ... ok
test window::tests::the_first_frame_before_visibility_matches_the_display_tick_frame ... ok
test presenter::painter::apple_tests::archive_miss_starts_with_cpu_pixels_then_replaces_the_whole_frame_with_ink ... ok
```

The existing probe self-test, with `G1_GPU_HOOK=0 G1_TRACE=1`, prints:

```text
probe: painter-specific completeness and one boot-expose wake passed
probe: lazy trace observes application queues without pre-creating a device or queue
probe: direct presentation preserves methods, joins preceding commit, excludes buffer path and resets reused drawables
probe: GPU hook off; three buffers completed, commit-phase accounting passed
```

Raw build and suite output is under `target/m10-c1/checks/`, including
`affected-tests.log` and `probe-tests.log`. The six checks are deliberately
skipped on the loaded development Mac, as directed; the orchestrator runs
them on the Mac mini. No local display was used.

### Cut reproduction and retained evidence

The before source is `b7ff5fc19d` (the carrier code at `9e565537bb`); the
after carrier is `c04c363e98`. The later commits change only measurement
handling and records. Executable SHA-256 values are:

| Bundle | SHA-256 |
| --- | --- |
| Ink before | `019e240d745c07ea53762835b21a2a322817e46f4bc925bbbc0f1358d59aa4bc` |
| Ink after | `84a8502592a813e1f0d5378a0765e570c5d8f0eb13cb3443f29ece2c40587aaf` |

The native control comes from `d16c84ee03`, GPUI and egui from `a91fd38b4d`;
their binaries are reused with the current probe and signed again. Each
cohort's `provenance.json` records their executable paths and hashes, the
probe, driver and wheel helper. The silver's source checkout is unchanged.
Its artifact root is `~/projects/exact2-measure/target/g0/m10-c1/`. Raw files
are also retained locally at `target/m10-c1/silver/`; the two Ink bundles and
build/check output are under `target/m10-c1/`. Nothing cited was deleted.

The benchmark's 105 generated data/assets files pass `data.sha256` before
building. The release build uses the command in “Chunk 2 verification and
reproduction” above, with `CARGO_BUILD_JOBS=6` and this worktree's `target/`.
`prepare-controls.py` and its log retain the probe/measure compile and
control-bundle preparation. Neither memory nor file caches were evicted.

`run-m10.py` is the exact collection snapshot; `run-m10-final.py` has the
native-field/clock reducer repair and failed-original rejection. Each is
retained beside `run-hook.py`. Enter the silver's Aqua session using the
README's SSH chain and use fresh output directories. Point provenance's
`ink` entry at `ink-before` or `ink-after` for the corresponding scoreboard
run; the paired run names both explicitly:

```sh
python3 "$PWD/target/g0/m10-c1/run-m10-final.py" run \
  target/g0/m10-c1 target/g0/m10-c1/paired-repeat \
  --arms ink-before,ink-after,gpui,egui
python3 "$PWD/target/g0/m10-c1/run-m10-final.py" run \
  target/g0/m10-c1 target/g0/m10-c1/scoreboard-repeat --scoreboard
```

The wrappers refuse competing `measure`, arm or `target/g0/run-*.py`
processes, require the amended sixty-second quiet gate and retain failures.
They require no GO file on the silver. To reproduce the excluded individual
values, phase spans, clocks and load observations without opening windows:

```sh
python3 -B target/m10-c1/report.py
```

This writes `diagnostic-report.json` and never promotes an excluded sample.
The ordinary reducer refuses every collected cohort:

```sh
python3 -B bench/heavy-list/gpui/startup.py summarize \
  target/m10-c1/silver/paired-1
```

The raw functional checks are `boot-functional/{miss,retained}.json`, their
trace, log and `.run.json` environment/exit records, plus the retained archive.
The quiet-bound/partition checks, four-arm reducer check and rejected-original
regression check print their outcomes in `target/m10-c1/checks/`. Fling, rest,
static, ten-pair startup medians and ten-launch cold rows remain unmeasured;
there is no evidence here to replace LLP 1149 §4.1 or satisfy D15's 100 ms
application obligation. Q4/Q6 and Stage 0b's third part remain as assigned.

The cut is rebased onto `518d469861`, after M10 landed as `9a88c8bead`.
The measured carrier, presenter, Ink adapter, probe and `measure.m` sources
are byte-identical across that rebase. Incoming main changes the kernel
schema, so the final-tree carrier suite and archive transition are rebuilt
and run again, with Metal required. They exit zero and print:

```text
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
Metal archive transition: mean 0.023/255, 0.000% beyond 32
test presenter::painter::apple_tests::archive_miss_starts_with_cpu_pixels_then_replaces_the_whole_frame_with_ink ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1708 filtered out; finished in 0.54s
```

These are `checks/rebase-painted-tests.log` and
`checks/rebase-transition-tests.log`. No post-rebase timing is claimed; the
retained signed bundles are the measured pre-rebase builds. The six repository
checks remain for the orchestrator.

## M10, cut 1b: the prepared frame rides the window's first composite

**Outcome, 2026-10-31: the first-composite goal remains unmet.** Each of the three
first-presentation placements produced zero timestamps in its live checks.
The final implementation preserves early preparation, pixel parity and CPU
handoff, but still needs the existing retry path. Three fix rounds are
exhausted; this is not a first-composite win.

**Predictions before the runs, 2026-10-30.** Paint the first scene into the
compose target before showing the window. Only after the show call returns,
acquire its drawable on the existing worker, copy the scene and present on the
main thread in that turn's Core Animation transaction. Keep the existing
retry-until-displayed path and CPU archive-miss handoff.

| Prediction | Instrument |
| --- | --- |
| Warm complete screen 146–160 ms, approximately show return at 116 ms plus a 30 ms first composite; faster than the prior single GPUI/egui launches at 212/218 ms | `startup.py`, ten warm and ten scoreboard-cold launches of each before/after Ink, GPUI and egui; hook off, trace on, power off |
| Qualified Ink cold median at least 30 ms below GPUI | `measure.m` cold-only, ten launches per arm, native/GPUI/Ink/egui, before and after; hook and power off |
| No unshown first submission on the ordinary retained-archive path | The same traced cohorts, drawable sequence and positive presented timestamps; report any 59 ms first-composite interval instead of the predicted 30 ms |
| First-frame pixels identical; CPU archive miss hands off; dropped submissions retry | Prepared-frame pixel comparison, Stage 0b transition/completeness and painted-carrier suites, plus live silver launches |

After signing, each arm launches once outside the cohorts to pay its initial
code-signature validation. The cold definition remains a fresh process with
application and system caches retained. Qualification now applies the 0.25-core
per-process bound only outside `/System`, `/usr/libexec`, `/sbin` and system
framework XPC services. System daemons remain in the 1.0-core non-benchmark
aggregate. This replaces the transient-daemon percentage allowance: cut 1's
crossings were XprotectService (0.6 core), mediaanalysisd, hybridsearchd,
searchpartyd, suggestd, duetexpertd, launchd and the system screen-sharing agent
(0.29 core), including work triggered by the launches themselves. Load below
30, compiler refusal and retained five-second observations remain required.

### Cut 1b mechanism and API contract

[`nextDrawable`](https://developer.apple.com/documentation/quartzcore/cametallayer/nextdrawable())
returns an available texture from the layer's pool; it does not acknowledge
that the window is on screen. The call can succeed while the layer is still
awaiting its first window composite. Apple's
[`presentedTime`](https://developer.apple.com/documentation/metal/mtldrawable/presentedtime)
is zero for an unpresented or dropped frame. Thus cut 1's zero callback records
a discarded submission, not a displayed black frame. The trace actually places
its acquisition at 725.239 ms, just after show returned at 725.047 ms and before
visibility was observed at 742.422 ms. Acquisition before visibility alone does
not prove why the compositor discarded it.

[`presentsWithTransaction`](https://developer.apple.com/documentation/quartzcore/cametallayer/presentswithtransaction)
is false by default, so Metal content is asynchronous to the window's Core
Animation transaction. Setting it true associates `drawable.present()` with
the caller's current transaction. Apple requires committing the rendering
command, waiting until it is scheduled, then calling the drawable's present
method; command-buffer presentation does not provide that synchronization.
The final cut uses this existing live-resize sequence at the first display-link
callback after show, outside Winit's AppKit redraw. The worker then
acquires and copies, and returns the scheduled drawable to the main thread
for presentation in that callback's transaction. Subsequent
frames keep their existing worker presentation. The compose target, resource
retirement, archive-miss compilation and positive-timestamp retry remain.

The trace reducer joins Ink's copy encoding to the following commit on that
worker and retains the main-thread drawable-present request separately. The
probe's thread-local preceding-commit marker can name the earlier compose
pass on main; it remains the join for GPUI's same-thread transaction path.
Presentation-to-display intervals below start at the actual present request.

**First live attempt; prediction before the second, 2026-10-30.** Immediate
post-show presentation still drops. The retained passive launch shows at
133.227 ms, requests presentation on main at 137.649 ms and receives the zero
callback at 137.723 ms, before the first layer transaction completes at
152.681 ms. Its complete screen is 177.600 ms. The ordinary active diagnostic
in `paired/ink-after-1-cold.json` similarly shows at 145.080 ms, presents at
151.768 ms, drops at 151.829 ms and completes at 204.282 ms: 59.2 ms after
show, not the predicted 30. The cohort is excluded for load.

The second attempt keeps the same prepared scene until the first redraw
request, or the first display-link callback if that arrives sooner. It
acquires, copies and presents on main's current transaction then, without a
second paint. A resize invalidates the prepared scene. Prediction: the first
submission now gets a positive timestamp; the same active trace's schedule
predicts about 185 + 20 = **205 ms**, so the original 146–160 ms prediction
may still miss. The first-composite interval is measured from the actual
present request on the same retained trace instrument. No timing from this
excluded diagnostic replaces a scoreboard row.

**Second live attempt; prediction before the final fix round.** The retained
archive's first redraw acquires at 146.841 ms and presents at 147.834 ms, but
receives zero at 151.530 ms; a retry also drops, and the complete screen is
192.261 ms. The CPU miss still hands off, at 212.389/345.722 ms for CPU/Ink,
but drops three earlier submissions. These are `boot-functional-2/`.
Winit 0.30's `RedrawRequested` comes through its NSView `drawRect`; GPUI
0.2.2 supplies a `displayLayer` delegate. Presenting within that AppKit draw
is the remaining hypothesis, not a proved compositor diagnosis.

The final round holds the scene through redraw requests and presents only on
the first display-link callback. Intervening redraws are suppressed through
that turn; the existing retry loop resumes on the next display callback.
Prediction on the same passive trace instrument: the first submission is
displayed without a zero callback, at about **182–195 ms** (show at 130 ms,
first display callback at 156 ms, then 27–39 ms to display). CPU startup still
hands off. If this fails, three fix rounds are exhausted and no further
implementation change is attempted in this cut.

### Final live result

The final passive retained-archive launch paints at 99.513–126.813 ms, shows
at 133.196 ms, receives its first display callback at 162.993 ms, acquires at
163.072–163.851 ms and presents on main at 164.279 ms. The zero callback
arrives at 164.301 ms. A retry also drops; the complete screen displays at
207.990 ms. That is 74.793 ms after show and 43.710 ms after the first
request. These intervals include waiting and retries; they do not isolate
the system's first-composite cost. Moving the request outside `drawRect`
did not eliminate the drop. The trace does not establish its internal cause.

The final CPU archive miss displays CPU at 199.749 ms and Ink at 324.749 ms.
Both frames satisfy the complete-scene criterion. Wgpu completes at
368.287 ms. The three runs acquire no drawable on main and paint once before
the first direct-present request. The Metal no-zero assertions fail; wgpu
has no equivalent negative-callback record, so its successful completion
is not evidence that its first submission displayed. These passive runs
are functional diagnostics, with activation suppressed, not scoreboard rows.

### Qualification and cold rows

No cohort qualifies. The six final-version attempts (before, paired final and
after, each with one repeat) all cross the 1.0-core non-benchmark aggregate.
None crosses the amended 0.25-core user-process bound during collection;
compiler and power-sampler checks pass and load averages stay below 30.
The earlier `paired` attempt measures the superseded immediate-post-show
implementation and is retained separately. Original attempts are never
replaced by successful diagnostic retries.

| Cohort | Original launches / planned | Maximum non-benchmark cores | Largest contributors at that observation |
| --- | ---: | ---: | --- |
| `scoreboard-before` | 4/40 | 1.189 | secd 0.110, BiomeAgent 0.090, MenuBarAgent 0.085 |
| `scoreboard-before-2` | 4/40 | 1.847 | ScreensharingAgent 0.266, SafariPlatformSupport.Helper 0.225, Tailscale 0.098 |
| `paired` (superseded) | 3/120 | 1.452 | ScreensharingAgent 0.273, BiomeAgent 0.103, Tailscale 0.097 |
| `paired-final` | 3/120 | 1.521 | ScreensharingAgent 0.208, BiomeAgent 0.148, CursorUIViewService 0.098 |
| `paired-final-2` | 3/120 | 1.591 | ScreensharingAgent 0.281, BiomeAgent 0.160, Tailscale 0.095 |
| `scoreboard-after` | 4/40 | 2.172 | ScreensharingAgent 0.218, fleet 0.194, agy 0.192 |
| `scoreboard-after-2` | 4/40 | 1.790 | ScreensharingAgent 0.283, launchd 0.177, Tailscale 0.136 |

The last row also rejects egui's original launch: the scene is complete and
visible, but its one drawable never receives a positive complete-frame
timestamp before the timeout (`egui-1.json.timeout`, exit 2). `measure.m`'s
successful `egui-1-retry` is retained as a diagnostic and excluded. Three of
the four original launches in that round succeed.

**Qualified four-arm cold p50, ten launches per arm:**

| Arm | Before, ms | After, ms |
| --- | ---: | ---: |
| Native | unavailable | unavailable |
| GPUI | unavailable | unavailable |
| Ink | unavailable | unavailable |
| Egui | unavailable | unavailable |

The following are **excluded single launches**, not medians or replacement
scoreboard rows. They preserve what `measure.m cold-only` actually collected:

| Attempt | Native, ms | GPUI, ms | Ink, ms | Egui, ms |
| --- | ---: | ---: | ---: | ---: |
| Before 1 | 266.372 | 219.500 | 256.507 | 230.904 |
| Before 2 | 245.883 | 206.083 | 244.212 | 239.677 |
| After 1 | 250.189 | 234.273 | 277.069 | 235.358 |
| After 2 | 250.936 | 205.988 | 286.109 | rejected |

The traced final cohorts each stop before reaching the GPUI and egui arms.
They therefore supply neither ten-pair startup tables nor control medians.
LLP 1149 §4.1's cold table remains unchanged.

### Predictions and results

| Prediction written before its run | Result |
| --- | --- |
| Warm complete screen 146–160 ms | **Miss in the retained observations:** final warm samples are 230.178 and 234.121 ms. Both cohorts are excluded; a qualified warm median is unavailable. |
| Qualified Ink cold at least 30 ms below GPUI | **Not established:** no four-arm cohort qualifies. The excluded Ink launches are slower than GPUI; they cannot fund a scoreboard claim. |
| Complete screen about show return plus a 30 ms first composite | **Miss as an end-to-end estimate:** the first active immediate-post-show trace takes 59.201 ms after show; the final passive trace takes 74.793 ms. Dropped submissions and retries prevent isolating the system-only composite cost. |
| No unshown first submission on the retained-archive path | **Miss:** the final passive Metal launch has two zero callbacks; the two final active warm launches have two and three. |
| First-frame pixels identical; CPU miss hands off; dropped frames retry | **Pass:** exact prepared-texture comparisons pass for CPU, direct Metal and wgpu. The final live CPU/Ink complete frames arrive at 199.749/324.749 ms, and every Ink diagnostic eventually displays. |
| Second-round active estimate about 205 ms | **Not measured as specified:** that revision fails its passive no-drop check (192.261 ms with two zero callbacks) and receives no active cohort. |
| Final-round passive complete screen 182–195 ms, without a zero callback | **Miss:** 207.990 ms and two zero callbacks. |

No saving is qualified, no first-composite mechanism is established, and
D15's 100 ms application obligation remains unmet. Q4/Q6 and Stage 0b's
third part remain as assigned.

### Reproduction

Both Ink arms build in this worktree's own target directory:

```sh
env EXACT_APP_DIR="$PWD/bench/heavy-list/exact-heavylist" \
  EXACT_PAINTED_PROBE="$PWD/bench/heavy-list/gpui/probe.m" \
  CARGO_TARGET_DIR="$PWD/target" CARGO_BUILD_JOBS=6 \
  bun host/apple/build.mjs exact-heavylist-apple --painted-benchmark --release
```

On silver, the retained base is `target/g0/m10-c1b`. The scripts run in the
logged-in GUI session through `launchctl asuser`, with `caffeinate -u -d -i`;
no GO file is used. `startup.py` checks lane ownership, observes a sixty-second
quiet gate, retains five-second observations throughout, and refuses an
incomplete, loaded or rejected-original cohort. Its default is ten rounds.
The paired final runs begin with the after arm and rotate the six variants;
GPUI/egui before and after keys name the same signed controls.

```sh
python3 target/g0/m10-c1b/provenance-third.py target/g0/m10-c1b ink-after
python3 target/g0/m10-c1b/prewarm.py target/g0/m10-c1b ink-after
python3 target/g0/m10-c1b/startup.py run target/g0/m10-c1b <fresh-output> \
  --arms ink-after,ink-before,gpui-after,gpui-before,egui-after,egui-before
python3 target/g0/m10-c1b/startup.py run target/g0/m10-c1b <fresh-output> --scoreboard
```

The before scoreboard uses `provenance.py ... ink-before`, after the four
before/control signature prewarms. `provenance-third.py` selects `InkAfter3.app`.
The retained driver snapshots, launch settings, executable hashes, power
observations, focus checks and failures reproduce the run conditions.
Locally, `python3 -B target/m10-c1b/report.py` reproduces the diagnostic
values and qualification decisions in `report.json`; `startup.py summarize`
refuses all seven cohorts. No retained raw file or failed original is removed.

### Builds and verification

The before arm is `a48fc7e77d` with the carrier from `9a88c8bead`; its product
sources are the M10 tip's. The three after arms are `438bb9fc11`,
`af03bc1d95` and `534e6b3271`. Native, GPUI and egui reuse the signed controls
from cut 1: native `d16c84ee03`, GPUI/egui `a91fd38b4d`. Every newly signed arm
launches once as an unrecorded throwaway before collection. The 105 data files
match `data.sha256`. No display events are posted on the black Mac.

All artifacts are retained under `target/m10-c1b/`, with silver's raw output
under `silver/`; the remote root is
`ccheever@100.76.94.78:~/projects/exact2-measure/target/g0/m10-c1b/`.
`InkBefore.app`, `InkAfter.app`, `InkAfter2.app` and `InkAfter3.app` retain the
local signed binaries. Each remote cohort's `provenance.json` records the
executable and driver hashes. `after-source.patch` preserves the final
measured source against `9a88c8bead`. The `boot-functional*` directories retain
the raw JSON, trace, log, launch environment, exit status and pipeline archives.
`functional-third.py` checks all three arms and records both failed Metal
assertions in `boot-functional-3/validation.json`.

The branch is rebased onto `56f3fc2f62`. The carrier, presenter, Ink, probe and
startup-driver sources are byte-identical across that rebase
(`checks/completion-source.diff` is empty). Main changes kernel metrics, the schema and
Contract lowering, so the affected suites run again on the final tree with
Metal required. The timing artifacts remain the measured pre-rebase builds;
no post-rebase startup timing is claimed. The full six-check gate is left
for the Mac mini, as requested.

```sh
env CARGO_BUILD_JOBS=6 RUST_TEST_THREADS=2 EXACT_INK_DEVICE=metal EXACT_REQUIRED=1 \
  cargo test -p exact-linux -p exact-apple-painted -p exact-ink \
  --lib --tests --no-fail-fast
```

`checks/completion-affected-tests.log` exits zero. Its suite summaries, in output
order (including the nested Linux processes), are verbatim:

```text
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 46 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.53s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1709 filtered out; finished in 0.56s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1709 filtered out; finished in 0.52s
test result: ok. 170 passed; 0 failed; 15 ignored; 0 measured; 1525 filtered out; finished in 3.00s
test result: ok. 1688 passed; 0 failed; 22 ignored; 0 measured; 0 filtered out; finished in 72.18s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.66s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s
test result: ok. 133 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.12s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.30s
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.13s
test result: ok. 104 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 11.03s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
```

The first pre-rebase Linux run timed out in the asynchronous photo-upload
burst test. Its failure is retained in `checks/affected-tests.log`; the
repeat passes all 1,688 tests in `checks/linux-repeat.log`. The prepared-frame
test reads the compose texture without a second paint and compares every
pixel with the ordinary path, for CPU and Ink. It also passes with wgpu.
The archive-miss test preserves the CPU painter across preparation and
checks the subsequent whole-frame handoff. The painted-carrier suite retains
the unshown-frame retry test. The existing probe test, load-rule boundary and
partition checks, and cross-thread phase reducer checks also pass; their
verbatim output is in `checks/`.

The final-tree wgpu comparison (`checks/completion-wgpu-parity.log`) prints:

```text
test paint::backend::tests::a_prepared_first_frame_has_the_same_pixels_without_a_second_paint ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1709 filtered out; finished in 0.29s
```

`checks/record-verification.log` verifies the retained executable hashes,
CPU-to-Ink ordering, lane ownership, absent power sampler, all seven reducer
refusals and the failed no-drop prediction. The scoped format check passes.
The staged caps check (`checks/caps-completion.log`) exits zero:

```text
All budgets within cap. 10 categories inspected.
```

## M10, cut 1c: diagnose the dropped first present

**Predictions before the diagnosis, 2026-10-31.** One diagnostic round on
the silver Mac, with no product fix until a cause is established. The unchanged
cut 1b carrier and GPUI run with `G1_FIRST_PRESENT=1`, hook off and trace on.
The probe retains drawable sequence numbers, layer/texture dimensions,
main-thread window facts with their sampling times, explicit CA flush/commit
events and submission/callback ordering. CA exposes no public pending-flush
query; explicit flush observations are not proof that an implicit commit ran.

| Hypothesis, in test order | Predicted observation that settles it |
| --- | --- |
| (a) Size or scale invalidates the first drawable | Its texture disagrees with the layer at presentation, or a size/scale assignment intervenes before its zero callback; sizing from the window before acquisition removes the zero. |
| (b) A second submission replaces the first | Submission 2 precedes callback 1; holding subsequent submissions until callback 1 removes the zero. A zero while submission count is 1 rules this out for that launch. |
| (c) The layer has not joined a committed window tree | An explicit main-thread flush before acquisition changes the first callback from zero to positive, with dimensions and submission count held fixed. |
| (d) Presentation must share the show transaction | Showing and presenting inside one explicit transaction changes the first callback to positive where the later-transaction control drops. |

The carrier's small window fixture varies those conditions and retains every
callback, including zero. Expected unchanged-carrier result: at least one zero;
expected GPUI result: a positive first callback. No millisecond saving is
predicted before this instrument establishes a cause. Raw files and commands
are retained in `target/m10-c1c/`; no earlier evidence is removed.

### Cut 1c outcome: cause not established; no renderer fix

The single diagnosis round rules out a scale/size change and replacement by
another submission for the captured first Ink drawable. Neither an explicit
flush nor presenting inside the show transaction repairs the fixture's first
callback. GPUI also reports a zero first callback in this run, contradicting
the expected control. The lane stops at the brief's one-round boundary.

The Ink diagnostic uses cut 1b's retained `InkAfter3.app` with the new probe;
its product is unchanged. It is a passive `G1_BOOT_ONLY` launch. GPUI uses
the retained control with the same probe and the ordinary active launch path.
They are functional observations with different activation paths, not paired
performance samples. Instrument overhead is not measured. Main-thread window
facts are cached with their timestamps for worker reads; the probe never asks
AppKit for window state on the render worker. The CV callback records its
link/counter state; its corresponding main-thread wake records the full
surface and window state. GPUI's own link is not directly sampled; the probe's
CADisplayLink is. Each trace retains fewer than
the diagnostic's 4,096-row limit (Ink 709, GPUI 2,495), with zero ordinary
trace events dropped.

| Event, process age in ms | Ink | GPUI |
| --- | ---: | ---: |
| Show returned | 145.960 | 188.738 |
| First acquire | 171.938 | 238.580 |
| First drawable present request | 172.486 | 243.025 |
| First callback, timestamp zero | 172.505 | 243.101 |
| Submissions at that callback | 1 | 1 |
| First positive drawable sequence | 3 | 2 |
| First positive displayed timestamp | 216.378 | 289.752 |

At Ink's first acquire, first present and first zero callback, texture and
layer are **1600×1536**, `contentsScale` and backing scale are **2**, and view
bounds are **800×768 points**. No size or scale assignment follows acquisition
before that callback. The layer is the `WinitView`'s layer and is in the
window's layer tree. The window is visible; occlusion state **8194** includes
the visible bit. The first main-thread display wake records the same facts
at **171.302 ms**, before acquisition on the worker. The carrier link is
running. Two explicit CA flushes precede acquisition; no later submission
exists when the zero callback arrives **0.019 ms** after the request. The
successful third submission has the same size, scale and tree membership.
Its positive timestamp follows show return by **70.418 ms**, including the
failed submissions and retries; that is not the system's first-composite cost.

GPUI's first acquire also matches its layer at **1600×1536**, scale **2**.
Its `GPUIView` is the layer delegate; Ink's delegate is nil. GPUI is visible
but occlusion state **8192** lacks the visible bit at its first callback.
That callback arrives **0.076 ms** after the direct present request, with
one submission. GPUI's second drawable supplies its first positive timestamp.
The trace does not reproduce the brief's GPUI-first-drawable premise.

Pinned GPUI 0.2.2's `platform/mac/window.rs` creates its Metal layer through
`makeBackingLayer`. `view_did_change_backing_properties` assigns the window's
backing scale to `contentsScale` before updating the drawable size. Its
constructor calls `makeKeyAndOrderFront` when show and focus are requested.
`display_layer` enables transaction presentation around its frame callback;
`metal_renderer.rs` acquires, encodes, commits, waits until scheduled and
presents. In this trace the scale/size assignment precedes show, which precedes
acquire and present. Source copies are retained beside the diagnostic.

The small fixture uses a layer-backed `NSView`, the carrier's layer format
and dimensions, and a cleared Metal render target. It submits on main after
the rendering command is scheduled. It holds subsequent submissions until
the prior callback, except in the explicit replacement variation. These are
single functional trials, with no quiet gate or performance conclusion.

| Fixture variation | Callback sequence (`0` = unshown, `+` = positive) | Result |
| --- | --- | --- |
| Size from window backing scale, present just after show | `0, +` | Correct initial size does not remove the zero |
| Acquire at 1×, change layer to 2× before present | `0, +` | Injected mismatch also drops; mismatch is not needed to reproduce it |
| Submit again immediately | `0, 0, 0` | No positive callback; bounded driver termination |
| Explicit flush after show, before acquire | `0, +` | Flush does not remove the zero |
| Show and present inside one explicit CA transaction | `0, +` | Shared transaction does not remove the zero |
| First present on the first display callback | `0, 0, +` | Moving to the callback does not remove the zero |

The replacement fixture saved its failed result at its two-second bound,
but `NSApp.stop` did not finish the process; the driver terminated its
recorded PID at ten seconds (exit −15). The other five exit zero. This failed
attempt is retained, not replaced. The first zero in every variation occurs
while submission count is one, before the second submission can replace it.

| Prediction or proposed explanation | Result |
| --- | --- |
| Unchanged Ink has at least one zero callback | **Observed:** sequences 1 and 2 report zero; sequence 3 displays |
| GPUI's first callback is positive | **Miss:** sequence 1 reports zero; sequence 2 displays |
| (a) Initial size/scale change causes Ink's first drop | **Ruled out for this launch:** matching texture/layer dimensions and scale throughout |
| (b) Ink's next submission replaces its first | **Ruled out for this launch:** callback 1 arrives while submission count is 1 |
| (c) A main-thread flush before acquire removes the zero | **Miss in the fixture:** first callback still zero. Tree membership and a flush call do not acknowledge server-side composition; the internal cause remains unknown. |
| (d) Sharing the show transaction removes the zero | **Miss in the fixture:** first callback still zero |

There is no established cause to fix once. Early painting, the CPU handoff
and retry policy remain unchanged. No saving, no zero-free confirmation
cohort and no qualified cold row are claimed. The startup qualification
amendment in `startup.py`, `run-hook.py`, the README and RFC §4, the five-launch
confirmation cohorts, the ten-pair cohorts and the four-arm cold series are
not performed after this diagnosis stop. LLP 1149 §4.1's scoreboard stays.

| Four-arm cold p50, ten launches per arm | Before, ms | After, ms |
| --- | --- | --- |
| Native | unavailable | unavailable |
| GPUI | unavailable | unavailable |
| Ink | unavailable | unavailable |
| Egui | unavailable | unavailable |

### Cut 1c reproduction and observer failures

The rebase onto `origin/main` **`74a7f861f0`** completed without conflicts.
The diagnostic source is **`6f8ea91f86`**; product code is unchanged by this
cut. `target/m10-c1c/silver/` retains the measured signed bundle copies,
probe and fixture binaries, launch scripts, raw records and session logs.
The remote copy is
`ccheever@100.76.94.78:~/projects/exact2-measure/target/g0/m10-c1c/`.
All windows ran there through `sudo -n /bin/launchctl asuser 501`, followed
by a user switch and `caffeinate -u -d -i`. No GO file was needed and no
display events were posted on the development Mac.

The observer needed two corrections before usable captures: its contents
hook queried a partly initialized Metal layer, then its window scan mutated
the layer map while enumerating it. The final observer skips layer properties
until sizing and enumerates a snapshot. The first passive GPUI attempt also
failed because `G1_BOOT_ONLY` creates an `NSApplication` before GPUI installs
its subclass. The corrected GPUI run uses the active path. Final-observer
Ink on that active path timed out with `probe: no window`; the passive Ink
capture above succeeds. Those failures are under `diagnosis/`,
`diagnosis-observer-fixed/` and `diagnosis-final-observer/`; none supplies
a numeric startup sample. They limit comparisons between the captures.

Build the probe using G4's command, with `first-present.m` beside `probe.m`.
The existing probe self-test and the fixture build with:

```sh
xcrun clang -DG1_PROBE_TEST -fobjc-arc -O2 -Wall -Wextra -Werror \
  -Wno-unused-parameter -mmacosx-version-min=14.0 -framework AppKit \
  -framework QuartzCore -framework Metal bench/heavy-list/gpui/probe.m \
  -o target/m10-c1c/probe-test
G1_GPU_HOOK=0 G1_TRACE=1 target/m10-c1c/probe-test
G1_GPU_HOOK=1 target/m10-c1c/probe-test
xcrun clang -fobjc-arc -O2 -Wall -Wextra -Werror -Wno-unused-function \
  -mmacosx-version-min=14.0 -framework AppKit -framework QuartzCore \
  -framework Metal host/apple-painted/tests/first-present.m \
  -o target/m10-c1c/first-present
python3 -B target/m10-c1c/report.py
```

On silver, run `first-present <variation> <fresh-output.json>` for the six
variations in the retained `fixture.py`, with its ten-second process bound.
`diagnose.py` and `diagnose-ink-passive.py` retain the exact launch settings;
use fresh output directories. `report.py` checks the recorded first callbacks,
dimensions, scale and absence of intervening assignments, and writes
`report.json` and `raw-sha256.json`. Diagnostic values above are reproduced
from those files, not inferred from the source.

### Cut 1c verification

The affected suites run on the rebased branch with its own `target/`:

```sh
env CARGO_BUILD_JOBS=6 RUST_TEST_THREADS=2 EXACT_INK_DEVICE=metal EXACT_REQUIRED=1 \
  cargo test -p exact-linux -p exact-apple-painted -p exact-ink \
  --lib --tests --no-fail-fast
```

`checks/affected-tests.log` exits zero on its first run. Prepared-frame parity,
the CPU archive-miss handoff and the unshown-frame retry tests pass. The suite
summaries below are verbatim, including nested Linux test processes:

```text
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 46 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.70s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1716 filtered out; finished in 0.59s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1716 filtered out; finished in 0.53s
test result: ok. 170 passed; 0 failed; 15 ignored; 0 measured; 1532 filtered out; finished in 3.02s
test result: ok. 1695 passed; 0 failed; 22 ignored; 0 measured; 0 filtered out; finished in 75.55s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.78s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.48s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 133 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.38s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.91s
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.28s
test result: ok. 104 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 12.74s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
```

The probe tests also exit zero, with hook off plus trace on, then hook on:

```text
probe: painter-specific completeness and one boot-expose wake passed
probe: lazy trace observes application queues without pre-creating a device or queue
probe: direct presentation preserves methods, joins preceding commit, excludes buffer path and resets reused drawables
probe: GPU hook off; three buffers completed, commit-phase accounting passed
probe: painter-specific completeness and one boot-expose wake passed
probe: GPU hook on; three buffers completed, commit-phase accounting passed
```

The separate wgpu prepared-frame comparison also passes
(`checks/wgpu-parity.log`):

```text
test paint::backend::tests::a_prepared_first_frame_has_the_same_pixels_without_a_second_paint ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1716 filtered out; finished in 0.79s
```

The full six-check gate is left for the Mac mini as requested. No root build,
root test, lint, boot or workspace check is claimed here. No push is made.

The staged caps check (`checks/caps.log`) exits zero:

```text
All budgets within cap. 10 categories inspected.
```

## M10, cut 1d: present the prepared frame every tick until displayed

**Prediction before the run, 2026-10-31.** Keep cut 1's early paint and
cut 1b's compose-target copy. After its first presentation, copy the prepared
frame into a fresh drawable on every display-link tick until a positive
`presentedTime`, then return to ordinary demand. The old retry requested a
Winit redraw on the tick and waited for AppKit's redraw event and transaction.
Each dropped presentation spends a drawable and a copy; it need not repeat
the paint walk. CPU archive-miss frames follow the same cadence. D7, D11,
D15 and the bounded failure-wake backstop remain unchanged.

| Prediction | Instrument |
| --- | --- |
| Warm complete screen about 150 ms, range 140–170: 116 ms show return + 25 ms for two or three display periods + about 10 ms compositor work | `startup.py`, hook off, trace on, power off; five warm/cold launches per arm, then ten paired before/after launches per arm |
| Ink cold median at least 30 ms below GPUI; prior single-launch controls were GPUI 212 ms and egui 218 ms | `measure.m` cold-only, ten launches per arm before and after, native/GPUI/Ink/egui |
| One or two zero callbacks per launch, then a positive callback within 17 ms of show return | Drawable callbacks joined to the show-return marker in the traced launches; report callback observation separately from display time |
| First-frame pixels identical, CPU-first archive miss hands off, unshown frames retry | Prepared-frame parity, archive transition and painted-carrier suites, plus live silver launches |
| Fling, rest and static rows unchanged | Ordinary-demand behavior after the first positive callback; separate measured rows required for a performance claim |

The orchestrator's launch qualification replaces five amendments that produced
no qualified startup cohort: launching wakes XProtect and the system daemons
listed in cut 1b. Require ten quiet seconds **before each launch**, with no
user process above 0.25 core and non-benchmark total below 1.0 core. Record
during-launch maxima beside every row; they never exclude a launch. The first
launch of each freshly signed arm is a throwaway. Cold remains a fresh process
with application and system caches retained. Before uses M10 tip `9a88c8bead`'s
carrier. Artifacts for this cut are retained under `target/m10-c1d/`.

### Cut 1d mechanism and functional result

`window.rs` retains the prepared frame until the layer reports a positive
presentation timestamp. Each display-link event acquires a fresh drawable
and copies the compose target; the CPU path uploads the retained pixmap.
Redraw events leave that frame alone. A failed submission releases it to
chunk 4's existing retry path. After a positive callback, ordinary demand
resumes. The display-link event previously requested a Winit redraw and
waited for AppKit to deliver it in a later transaction. This cut removes
that wait; each rejected presentation still costs a drawable and a copy.

The measured source is `7945a07fe0`; the prediction was committed first as
`6e0f4946ee`. Silver uses the retained M10 carrier from `9a88c8bead` for
before, the new release binary for after, and identical GPUI and egui
binaries for both labels. `silver/*/provenance.json` retains executable
hashes and driver settings. The final freshly signed throwaways all exit
zero after a normal AppKit quit. Earlier throwaways encountered macOS's
crash-restoration alert before application launch completed. A main-thread
sample and the benchmark process's accessibility tree identify the alert;
pressing its “Don't Reopen” button and quitting normally clears it. Those
attempts remain under `silver/prewarm*` and `silver/restore-alert*`.

The passive functional launches are diagnostics, without the cohort's quiet
gate. Both Metal paths present once per observed tick, with no paint walk
between their first request and first positive callback:

| Path | Requests / ticks before positive callback | Zero callbacks | Request gaps, ms | Show return → positive callback, ms | First complete screen, ms |
| --- | ---: | ---: | --- | ---: | ---: |
| Fresh archive, CPU first | 6 / 6 | 2 | 6.234–8.360 | 69.762 | 223.095 |
| Retained archive, Metal first | 6 / 6 | 2 | 6.949–9.043 | 76.053 | 234.397 |

The CPU screen hands off to a complete Ink screen at 323.095 ms. The
retained-archive launch submits no CPU frame. Both acquire drawables off
main and present on main. The first prepared frame can lack images: the
retained-archive launch first displays at 201.064 ms and completes at
234.397 ms, after painting resumes. These values come from
`silver/boot-functional/`, reproduced by `cadence.py` and `functional.py`.

**The live wgpu startup fails.** It makes 3,576 presentation requests over
3,576 ticks and records 3,549 positive probe timestamps, but only one paint
walk and no carrier `presented handler` event. Its timeout sidecar still
reports an incomplete scene. The wgpu surface path does not register the
layer callback that updates `presented_painter`; retaining the prepared
frame therefore prevents painting from resuming. The offscreen pixel
comparison passes but does not exercise this carrier transition. This is
a regression and prevents qualification of the implementation. The one
renderer round ends here; no second cadence policy is tried.

### Cut 1d launch qualification

The five-round confirmation and ten-round paired cohorts both stop on an
original egui launch with `probe: first complete frame timed out`. All
recorded prelaunch gates pass, power sampling is absent, and neither monitor
reports an error. Their during-launch load is not the exclusion reason.
`startup.py summarize` rejects both with `AssertionError: incomplete cohort`.

| Traced cohort | Successful / required launches | Failed original | Maximum during-launch other / user cores |
| --- | ---: | --- | ---: |
| Five per arm, cold and warm | 5 / 30 | egui, round 1 warm | 1.632 / 0.228 |
| Ten paired before/after per arm, cold and warm | 28 / 120 | egui-before, round 3 cold | 2.206 / 0.896 |

No five-launch or ten-pair median is qualified. The retained individual Ink
warm observations are 217.119 and 210.604 ms before, and 288.987, 276.463
and 264.808 ms after (the first after value is from confirmation). All three
after observations miss the predicted 140–170 ms range. The two completed
warm pairs regress by 59.344 and 54.205 ms; they do not substitute for ten
pairs. Both controls' before/after labels use identical binaries.

All six traced after launches record two zero callbacks, then a positive
callback 80.547–95.235 ms after show return: the zero count matches, the
17 ms bound misses. No retry paint intervenes. In the first warm pair,
before has one zero callback, four presentation requests and three further
paint walks before its positive callback; after has two zeros, six requests
and no further paint walk. The respective request gaps are 6.975–8.270 ms
and 6.966–8.494 ms. The prior redraw cadence can already approach display
cadence once it starts; the new path's repeated copies do not establish a
complete-screen saving. GPUI and egui zero callbacks are not recorded by the
ordinary positive-timestamp trace, so their zero-count pattern is not
requalified here.

The first warm pair locates the loss. These are process ages from one pair,
not qualified phase medians; before is the M10 tip, so the comparison includes
cuts 1 and 1b as well as 1d.

| Phase | Before, ms | After, ms |
| --- | ---: | ---: |
| First paint starts | 158.734 | 116.012 |
| Show returns | 126.778 | 150.986 |
| First present requested | 191.099 | 200.204 |
| First drawable displayed | 217.119 | 243.129 |
| Complete screen displayed | 217.119 | 276.463 |

`report.py` reproduces the excluded diagnostics and each launch's load
maxima from `silver/confirm/` and `silver/paired/`. Their schedule, gate
observations, failure sidecars and original logs are retained. There are no
replacement samples.

### Cut 1d four-arm cold rows and prediction outcomes

Both `measure.m` cold-only cohorts also stop on egui's original readiness
failure: before at round 2 (6 successes of 40 required), after at round 3
(9 of 40). Every prelaunch gate passes; both monitors report no error and
power sampling stays absent. The summarizer rejects both as incomplete.
The table retains **individual successful observations, not qualified
medians**, in round order. Load columns are the largest observed maxima
among those successful rows; every original's maxima, including the failed
ones, remain beside it in `launch-rows.md` and the schedule JSON.

| Arm | Before cold observations, ms | After cold observations, ms | Before max other / user cores | After max other / user cores |
| --- | --- | --- | ---: | ---: |
| Native | 256.525 | 239.993, 259.797 | 1.817 / 0.154 | 1.197 / 0.213 |
| GPUI | 197.056, 204.165 | 203.051, 189.589 | 1.572 / 0.230 | 1.311 / 0.181 |
| Ink | 225.509, 209.686 | 268.315, 265.702, 254.946 | 1.267 / 0.217 | 1.570 / 0.226 |
| egui | 225.886; round 2 fails | 214.469, 220.172; round 3 fails | 1.308 / 0.194 | 1.455 / 0.179 |

| Prediction | Result |
| --- | --- |
| Warm complete screen about 150 ms, range 140–170 | **Miss in every retained after observation:** passive retained 234.397 ms; traced warm 264.808–288.987 ms. No qualified warm median. |
| Cold median at least 30 ms below GPUI | **Not established:** neither cold cohort qualifies. The excluded after Ink observations are 254.946–268.315 ms against GPUI's 189.589–203.051 ms, in the opposite direction. |
| One or two zero callbacks, positive within 17 ms of show return | **Count matches, time misses:** two zeros in all six traced after launches; positive callback at 80.547–95.235 ms after show. Passive CPU/Metal launches also have two zeros, at 69.762/76.053 ms. |
| Identical prepared pixels, CPU-first handoff and unshown-frame retries | **Tests pass; live Metal handoff passes. Live wgpu fails:** it never leaves the prepared-frame loop. |
| Fling, rest and static rows unchanged | **Unmeasured:** no performance claim is made after the startup failure. Existing rows remain historical. |

No qualified startup table or saving is established. LLP 1149 §4.1's cold
row is unchanged. The experimental source remains on the lane branch with
the wgpu regression recorded in `QUEUE.md`; it is not ready to land. D15's
100 ms obligation, Q4/Q6 and Stage 0b's third part are unchanged. No second
renderer round or replacement cohort is attempted.

### Cut 1d reproduction and retained files

The worktree's release bundle builds with its own `target/`; the build
script passes `--target-dir` to the benchmark workspace's Cargo invocation:

```sh
env EXACT_APP_DIR="$PWD/bench/heavy-list/exact-heavylist" \
  EXACT_PAINTED_PROBE="$PWD/bench/heavy-list/gpui/probe.m" \
  CARGO_TARGET_DIR="$PWD/target" CARGO_BUILD_JOBS=6 \
  bun host/apple/build.mjs exact-heavylist-apple --painted-benchmark --release
```

`target/m10-c1d/build-context.json` and `after-source.patch` record the
build inputs. The after executable's SHA-256 is
`48668f28af536b379b31b01e447a13a72a94653f5e748abc6aa6fa5b26314cfc`.
Silver's retained base is `~/projects/exact2-measure/target/g0/m10-c1d/`;
its complete contents, including signed bundles, are copied to local
`target/m10-c1d/silver/`. `collection-final.json` records every command,
driver PID and exit. The wrapper continues to the next independently
requested cohort after an earlier one fails; its own exit is not a pass.

`prepare.py` records the copied control bundles, probe replacement and
signing. `prewarm-quit.py` pays first-execution validation and quits normally.
`run-final.py` runs that prewarm, the passive checks, five-round confirmation,
ten-round pairing, then both cold scoreboards. It uses the existing GUI
session through `launchctl asuser 501` and `caffeinate -u -d -i`, without a
GO file. Reproduction needs fresh output directories; the retained cohort
directories must not be reused. `provenance.json` in each cohort freezes
which Ink executable its before or after label selected.

Reproduce the values without launching an app:

```sh
python3 -B target/m10-c1d/report.py
python3 -B target/m10-c1d/cadence.py
python3 -B target/m10-c1d/launch-rows.py
```

`report.json`, `cadence.json` and `launch-rows.md` retain the derived
values. `raw-sha256.json` hashes the retained files. `checks/summarize-*.log`
contains the unchanged summarizer's four refusals. All first-complete
timeouts, gate observations, per-launch load maxima and callback traces
remain in the corresponding `silver/` directories.

### Cut 1d verification

The affected suites run here with this worktree's `target/`, Metal required,
and two test threads. The full six checks are deferred to the Mac mini.

```sh
env CARGO_BUILD_JOBS=6 RUST_TEST_THREADS=2 EXACT_INK_DEVICE=metal EXACT_REQUIRED=1 \
  cargo test -p exact-linux -p exact-apple-painted -p exact-ink \
  --lib --tests --no-fail-fast
```

`target/m10-c1d/checks/affected-tests.log` exits zero. Its suite summaries,
including nested Linux processes, are verbatim:

```text
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.68s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 46 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.90s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1716 filtered out; finished in 0.65s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1716 filtered out; finished in 0.58s
test result: ok. 170 passed; 0 failed; 15 ignored; 0 measured; 1532 filtered out; finished in 3.21s
test result: ok. 1695 passed; 0 failed; 22 ignored; 0 measured; 0 filtered out; finished in 86.08s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.83s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.05s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
test result: ok. 133 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.17s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.03s
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.92s
test result: ok. 104 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 13.90s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s
```

Prepared-frame parity, the CPU archive-miss handoff and the unshown-frame
retry tests pass. The separate wgpu prepared-frame comparison also passes:

```text
test paint::backend::tests::a_prepared_first_frame_has_the_same_pixels_without_a_second_paint ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1716 filtered out; finished in 0.72s
```

The probe tests, hook off with trace on and then hook on, exit zero:

```text
probe: painter-specific completeness and one boot-expose wake passed
probe: lazy trace observes application queues without pre-creating a device or queue
probe: direct presentation preserves methods, joins preceding commit, excludes buffer path and resets reused drawables
probe: GPU hook off; three buffers completed, commit-phase accounting passed
probe: painter-specific completeness and one boot-expose wake passed
probe: GPU hook on; three buffers completed, commit-phase accounting passed
```

The qualification regression checks pass: the prelaunch total is strictly
below 1.0 core and the user-process bound includes 0.25; launch maxima above
both limits are report-only. Failed originals, failed gates and power sampling
still prevent qualification. Other workloads keep their existing rule.
`checks/qualification.py` and its log retain the checks. All 105 generated
assets match `data.sha256`; no cited artifact was deleted.

The staged caps check exits zero (`checks/caps-final.log`):

```text
All budgets within cap. 10 categories inspected.
```

The changed Rust files pass `rustfmt --check`; `git diff --check` is clean.
The other broad checks are left for the Mac mini as requested: no root
build, root test, root clippy/fmt, boot or workspace check is claimed here.
The release benchmark build and affected suites above run locally. No push
is made.


## M10, cut 2: show the window before the first paint

**Predictions before the run, 2026-10-31.** Order the window front as soon
as it exists, before runner boot and layout. Set its background from the
plan's literal root background, or AppKit's window background when no colour
is known yet. Paint after layout and submit through the ordinary worker path.
Remove cuts 1b/1d's retained-frame copy and present-every-tick loop; keep
chunk 4's retry until a positive displayed timestamp. D7, D11 and D15 stay.

| Prediction | Instrument |
| --- | --- |
| Warm complete screen about 172 ms, range 160–190: max(show at about 82 + 90, present + composite) | `startup.py`, hook off, trace on, power off; five cold/warm launches per arm, then ten paired before/after launches per arm |
| Cold complete screen about 180–195 ms, against prior single GPUI 190–204 and egui 214–226 ms | `measure.m` cold-only, ten launches per arm before and after, native/GPUI/Ink/egui |
| One zero callback or none before the first positive timestamp | The same traced launches, with callback observation separate from display time |
| First-frame pixels identical; CPU archive miss hands off; live Metal and wgpu continue drawing and report the displayed painter | First-frame parity, archive transition, affected suites and live launches on silver |
| Fling, rest and static rows unchanged | Existing scoreboard instruments; this startup cut does not remeasure those rows |

Before is M10 tip `9a88c8bead`'s retained carrier. Each freshly signed arm
gets a throwaway launch. Ten quiet seconds precede each measured launch;
during-launch load maxima remain report-only. A failed launch is retained
and the cohort continues without replacement. Each median uses completed,
qualified launches and reports n, with every failure beside it. All new
evidence is retained under `target/m10-c2/`; older raw files stay intact.

### Cut 2 mechanism

The benchmark's root declares `background-color="#ffffff"`. Before runner
boot, `Config::initial_window_background` reads a single root's opaque literal
from the plan. Dynamic, transparent or unresolved colours use AppKit's
`windowBackgroundColor`. The carrier sets both `NSWindow.backgroundColor`
and the `CAMetalLayer` background, orders the window front, then boots the
runner, lays out and paints. The ordinary worker presents that frame; chunk
4's unshown-frame retry remains the backstop.

The retained first-frame API and per-tick copy loop are removed from the
carrier, presenter and both device paths. The wgpu path keeps its ordinary surface
submission, with a layer subclass registering the same displayed-timestamp
callback as Metal. A positive timestamp updates the painter and displayed
counter; a zero timestamp requests recovery. The callback observes wgpu's
private drawable without changing its submission or retirement. D7's compose
target, D11's threads and retirement, and D15's gate are unchanged.

The unpainted window shows the page's white background while its scene is
prepared. That is the normal AppKit order—show the window, then draw its
content—and I do not judge the matching empty surface a platform tell. The
measurements below distinguish frame preparation from actual display; the
show call alone does not time the background's visibility on the glass.

### Cut 2 confirmation

The signed release is `8116a09a55`, rebased onto main
`4a165fd466`. Before is the retained M10 carrier
`9a88c8bead`; GPUI, egui and native retain their prior control executables.
The dedicated silver M5 Max runs the same 800×768-point content at 2×,
hook off, trace on and power sampling off. Cold means a fresh process with
caches retained; warm is its next launch, with another ten-second quiet gate.

All 30 confirmation attempts pass the prelaunch gate. Each row below uses
completed originals; egui's cold rounds 3 and 4 exit 2 before complete-frame
readiness, and remain in the record. The cohort continues after each failure.

| Arm | Cold p50, ms (n) | Cold range, ms | Warm p50, ms (n) | Warm range, ms |
| --- | ---: | ---: | ---: | ---: |
| ink-after | 207.789 (5) | 199.426–244.058 | 210.881 (5) | 203.993–214.254 |
| gpui | 206.661 (5) | 201.602–209.430 | 202.728 (5) | 195.786–213.367 |
| egui | 216.552 (3) | 203.176–225.241 | 209.458 (5) | 204.149–224.225 |

**The warm prediction misses:** 210.881 ms against about 172 ms and the
160–190 ms range. All ten Ink launches have exactly one zero callback before
the first positive timestamp. The warm median show call returns at 96.339 ms,
first present at 152.929 ms and complete display at 210.881 ms. Showing earlier
does not establish the predicted `show + 90 ms` bound. The separate cold-only
scoreboard below is the cold prediction's instrument.

One retained warm trace, `confirm/ink-after-1-warm.json.trace.json`, shows
why the earlier call alone is insufficient: show returns at 97.333 ms,
first present commits at 155.483 ms, its zero callback arrives at 159.002 ms,
and the next paint starts at 190.350 ms. That present commits at 192.292 ms;
complete display is 210.881 ms. The 31.348 ms from zero callback to retry
paint is observed elapsed time, not an attribution to the compositor.
The one implementation round ends without changing that scheduling.

### Cut 2 paired launches

All 120 scheduled attempts pass the prelaunch gate; 112 complete. The eight
failed originals are egui: before cold rounds 1 and 3; before warm rounds
4, 6, 8 and 10; after warm rounds 1 and 8. Each exits 2 before readiness.

| Arm | Cohort | Before p50, ms (n) | After p50, ms (n) | Paired after−before p50, ms (n) | Paired range, ms |
| --- | --- | ---: | ---: | ---: | ---: |
| ink | cold | 211.851 (10) | 213.020 (10) | -0.262 (10) | -17.351–+13.276 |
| ink | warm | 218.171 (10) | 213.798 (10) | +1.407 (10) | -14.757–+8.821 |
| gpui | cold | 203.478 (10) | 204.691 (10) | +1.445 (10) | -33.529–+28.541 |
| gpui | warm | 202.105 (10) | 199.514 (10) | -2.538 (10) | -9.853–+12.112 |
| egui | cold | 220.498 (8) | 213.177 (10) | -3.843 (8) | -22.181–+2.932 |
| egui | warm | 211.281 (6) | 210.747 (8) | -0.355 (5) | -15.103–+19.460 |

A paired difference uses only rounds in which both launches complete; it
is not the difference of the two medians. **No complete-screen saving is
funded:** Ink's paired ranges cross zero, while the warm after median
213.798 ms again misses about 172 ms and the 160–190 ms range. Each
control uses the same executable for its before and after labels.

The warm Ink order, medians of ten launches per variant, is:

| Process age, ms | Before (M10 tip) | Cut 2 |
| --- | ---: | ---: |
| window created | 90.152 | 87.607 |
| window show returned | 125.493 | 93.348 |
| first commit completed (runner returned) | 115.126 | 120.334 |
| first layout completed | 116.381 | 121.729 |
| first paint record | 156.463 | 125.734 |
| first present committed | 189.650 | 152.877 |
| complete screen | 218.171 | 213.798 |
| complete callback observed | 218.458 | 214.038 |

Before shows after layout, then paints on the event-loop redraw. Cut 2 shows
before runner boot and paints immediately after layout. Each of the forty
paired Ink launches has one zero callback before its first positive one.
The earlier show and first present do not establish a complete-screen saving.
The cut 2 warm medians leave about 60 ms from the show call to first present,
with the matching white surface while content is prepared, and about 120 ms
from show to complete display. The background's on-screen duration is not
measured separately.

### Cut 2 cold scoreboard and verdict

Both `measure.m cold-only` schedules complete all forty attempts and qualify
under the per-launch gate and successful-launch rule. The probe's GPU hook
and power sampling are off; tracing is off in these scoreboard cohorts.
Native uses its complete-screen snapshot criterion; the painted arms use the
first complete drawable's displayed timestamp. These remain whole-app rows,
with the existing differences in text, scrolling, caches and presentation.

| Arm | Before p50, ms (n) | Before range, ms | Cut 2 p50, ms (n) | Cut 2 range, ms |
| --- | ---: | ---: | ---: | ---: |
| Native | 253.973 (10) | 232.556–281.835 | 251.480 (10) | 238.258–262.092 |
| GPUI | 194.446 (10) | 185.076–207.128 | 200.396 (10) | 187.968–217.837 |
| Ink | 209.576 (10) | 204.773–223.966 | 207.710 (10) | 198.497–218.529 |
| egui | 207.425 (8) | 199.557–231.991 | 202.383 (6) | 195.441–223.930 |

Egui's before rounds 6 and 9 and after rounds 1, 2, 4 and 6 fail startup
readiness with exit 2. Their records and timeout diagnostics remain; there are no
replacement launches. All native, GPUI and Ink originals complete.

| Prediction written before the run | Result |
| --- | --- |
| Warm about 172 ms, range 160–190 | **Miss:** confirmation 210.881 ms (n=5); paired after 213.798 ms (n=10). The paired warm difference is +1.407 ms, range −14.757 to +8.821. |
| Cold about 180–195 ms | **Miss:** qualified cold-only Ink 207.710 ms (n=10), against GPUI 200.396 (n=10) and egui 202.383 (n=6). Before Ink is 209.576 ms; the traced cold paired difference is −0.262 ms, range −17.351 to +13.276. |
| One zero callback or none | **Pass:** exactly one in every one of the thirty traced cut 2 Ink launches; the twenty paired before launches also each have one. |
| First-frame parity; CPU-first archive miss hands off; live Metal and wgpu keep drawing and report display | **Pass:** exact parity tests, passive CPU-to-Ink handoff, and both release and development live checks below. |
| Fling, rest and static rows unchanged | Not remeasured in this startup cut. Their existing rows stand; this is not a new equality measurement. |

The warm and cold predictions miss, and the paired observations fund no
complete-screen saving. The mechanism works and the live wgpu freeze is
repaired; the first retry's scheduling remains a lead. The matching white
background is retained as the carrier default on this branch. This is no
D15 decision: the 100 ms application obligation and dated boot obligation
are unchanged, as are D7 and D11. No second implementation round was run.

All 230 attempted launches pass ten quiet seconds before launch, with no
power sampler or monitor error. During-launch maxima are report-only:

| Cohort | Maximum other busy cores | Maximum single user-process cores | Attempts / failed |
| --- | ---: | ---: | ---: |
| confirm | 3.764 | 0.365 | 30 / 2 |
| paired | 2.062 | 1.139 | 120 / 8 |
| scoreboard-before | 2.382 | 0.327 | 40 / 2 |
| scoreboard-after | 2.064 | 0.302 | 40 / 4 |

`target/m10-c2/launch-rows.md` lists every original, its complete-screen time
or failure, and both load maxima. All sixteen failures are egui readiness
failures. None of those maxima is used to remove a sample.

### Cut 2 functional checks

The passive probe uses a fresh application archive for the CPU-first miss,
then that retained archive, then wgpu. These are diagnostics without the
cohort's quiet gate, not startup medians. All three show before presenter
boot, acquire drawables off main and reach a positive displayed timestamp.

| Launch | First complete display, ms | First complete Ink display, ms | Zero callbacks before first display |
| --- | ---: | ---: | ---: |
| Metal, archive miss (CPU first) | 167.188 | 333.854 | 1 |
| Metal, retained archive | 190.822 | 190.822 | 1 |
| wgpu | 334.993 | 334.993 | 1 |

`silver/boot-functional/validation.json` is empty. The initial live socket
attempts are retained under `silver/live/`: both time out waiting for a
socket because the production bake explicitly ignores `EXACT_AGENT_SOCKET`
and `EXACT_AGENT_LIVE`. This tests the production boundary, not liveness.
The subsequent socket check uses a development bake of the same source;
the release check uses the existing probe and wheel helper.

Both follow-up live instruments pass with `BENCH_LIVE=1`:

| Device | Release displayed count | Development displayed count | Development carrier frames | Displayed painter / API |
| --- | ---: | ---: | ---: | --- |
| Metal | 12 → 113 | 7 → 55 | 8 → 56 | `ink` / `direct Metal` |
| wgpu | 12 → 113 | 2 → 40 | 3 → 41 | `ink` / `wgpu Metal` |

The release probe records 106 wheel events, continued paint walks after the
first positive callback, 113 positive callbacks and zero dropped trace events
for each device. The development socket reads both displayed painter flags
and counters before and after scrolling. Its receipt is development trust;
the timed bundle remains the production release. Both validation files are
empty, and all four launches exit zero. The two release window captures,
`silver/live-release/{metal,wgpu}.png`, were inspected: the live list, text
and photos draw after scrolling. The development captures are retained too.

### Cut 2 verification and reproduction

The affected suites run locally with this worktree's own `target/`, Metal
required, and two test threads:

```sh
env CARGO_BUILD_JOBS=6 RUST_TEST_THREADS=2 EXACT_INK_DEVICE=metal EXACT_REQUIRED=1 \
  cargo test -p exact-linux -p exact-apple-painted -p exact-ink \
  --lib --tests --no-fail-fast
```

`target/m10-c2/checks/affected.log` exits zero. Its suite summaries, including
nested test processes, are verbatim:

```text
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.94s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 46 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.13s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1716 filtered out; finished in 0.81s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1716 filtered out; finished in 0.62s
test result: ok. 170 passed; 0 failed; 15 ignored; 0 measured; 1532 filtered out; finished in 3.98s
test result: ok. 1695 passed; 0 failed; 22 ignored; 0 measured; 0 filtered out; finished in 83.48s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.90s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.07s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s
test result: ok. 133 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.35s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.89s
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.80s
test result: ok. 104 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 13.54s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s
```

The pinned `paint`, `borders`, `tint`, `backdrop`, `gradients`, `visual` and
`materials` cases pass; paint's existing frame-time measurement stays ignored.
The root-background, first-frame parity, archive-miss handoff and unshown-frame
retry tests pass. The separate wgpu parity command is:

```sh
env CARGO_BUILD_JOBS=6 RUST_TEST_THREADS=2 EXACT_INK_DEVICE=wgpu EXACT_REQUIRED=1 \
  cargo test -p exact-apple-painted --lib \
  the_first_frame_after_layout_matches_the_display_tick_frame -- --nocapture
```

`checks/wgpu-parity.log` exits zero:

```text
test window::tests::the_first_frame_after_layout_matches_the_display_tick_frame ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 1.17s
```

The unchanged probe's hook-off/trace-on and hook-on self-tests pass:

```text
probe: painter-specific completeness and one boot-expose wake passed
probe: lazy trace observes application queues without pre-creating a device or queue
probe: direct presentation preserves methods, joins preceding commit, excludes buffer path and resets reused drawables
probe: GPU hook off; three buffers completed, commit-phase accounting passed
probe: painter-specific completeness and one boot-expose wake passed
probe: GPU hook on; three buffers completed, commit-phase accounting passed
```

`checks/qualification.py` checks strict total load, the inclusive user-process
bound, successful n with retained failures, an empty arm, refusal of failed
old cohorts, failed gates and power sampling, and report-only launch maxima.
It passes. All 105 generated assets match `data.sha256`.

The signed release and development carrier build with:

```sh
EXACT_APP_DIR="$PWD/bench/heavy-list/exact-heavylist" \
EXACT_PAINTED_PROBE="$PWD/bench/heavy-list/gpui/probe.m" \
CARGO_TARGET_DIR="$PWD/target" CARGO_BUILD_JOBS=6 \
  bun host/apple/build.mjs exact-heavylist-apple --painted-benchmark --release
```

Omit `--release` for the development socket check. The build script supplies
`--target-dir target` to the benchmark workspace. Its lock was refreshed with
`cargo metadata --offline --format-version 1` after adding AppKit's colour
features; `build-benchmark.Cargo.lock` retains it. `checks/release-build-final.log`
and `checks/live-dev-build.log` record the successful builds. The final timed
release executable hash is in every cohort's `provenance.json`; signed copies
of all arms and the development carrier remain under `silver/`.

The probe and driver compile with G4's commands. `prepare.py` copies the retained
controls and M10 before carrier, installs the same probe and signs every bundle.
`run-final.py` retains the exact original collection: signature throwaways,
passive checks, the rejected production socket attempts, confirmation, paired
launches and both scoreboards. `live-release.py` and `live-dev.py` are the
successful follow-up checks. All run on silver through `launchctl asuser 501`
and `caffeinate -u -d -i`. No GO file or development-Mac display input was needed.
Use a fresh output base when repeating; preserve all recorded originals.

Retained evidence is `target/m10-c2/silver/`, with its remote copy at
`ccheever@100.76.94.78:~/projects/exact2-measure/target/g0/m10-c2/`.
`build-context.json` records main `4a165fd466`, source `8116a09a55`, Bun and
Rust versions; `after-source.patch` retains the source delta against the M10 tip.
Reproduce the tables and each launch's load maxima without opening a window:

```sh
python3 -B target/m10-c2/report.py
python3 -B target/m10-c2/launch-rows.py
```

`report.json`, `tables.md`, `launch-rows.md` and the four
`checks/summarize-*.log` files retain the reductions. `raw-sha256.json` hashes
the retained diagnostics; cohort provenance hashes the executables. No cited
raw file from an earlier cut was deleted.

The staged caps check passes (`checks/caps-final.log`):

```text
All budgets within cap. 10 categories inspected.
```

The changed Rust files pass `rustfmt --check`; `git diff --check` is clean.
The six broad gates are left for the Mac mini as requested: no root build,
root test, root clippy/fmt, boot or workspace check is claimed here. Caps,
the release/development builds and the affected suites above run locally.
No push is made.

## M10, cut 3: re-present the first frame on each tick

**Predictions before the run, 2026-10-31.** Keep cut 2's early show and
page background. Retain the first submitted frame; after its zero callback,
copy it to a fresh drawable on every display-link tick until the first
positive displayed timestamp. Do not repaint it. Keep the wgpu layer's
displayed-painter tracking and chunk 4's redraw retry as the backstop.

| Prediction | Instrument |
| --- | --- |
| Warm complete screen about 178 ms, range 170–190, approximately show + 85 ms | `startup.py`, hook off, trace on, power off; five cold/warm launches per arm, then ten paired before/after launches per arm |
| Cold complete screen 180–188 ms, at least 10 ms below GPUI and egui | `measure.m` cold-only, ten attempts per arm before and after, native/GPUI/Ink/egui |
| Two to ten zero callbacks, then a positive one | The traced confirmation and paired launches; callback observation and displayed time reported separately |
| One drawable and compose-target copy per retry tick, no paint | The same traces, joining tick, copy and callback records |
| First-frame pixels identical; CPU-first archive miss hands off; live Metal and wgpu continue drawing | Affected suites, passive startup and live carrier checks on silver |
| Fling, rest and static unchanged | Existing scoreboard rows stand; this startup cut does not remeasure them |

Before is cut 2's retained `8116a09a55` carrier. Every signed arm gets a
throwaway launch. Every measured launch needs ten quiet seconds; launch load
maxima are report-only. Failed originals remain without replacement, with
successful n beside each median. New raw files stay under `target/m10-c3/`.
The six broad checks go to the Mac mini; affected suites and caps run here.
One implementation round, no push.

The retained ten cut 2 warm traces locate the proposed earlier show:
AppKit finishes launching at median 65.517 ms; the call to create the window
begins at 66.519, returns at 87.607, and show returns at 93.348. The median
within-launch dispatch gap is 0.985 ms, window construction 21.083 ms, and
layer setup plus show 5.721 ms. `target/m10-c3/window-before.json` retains
the per-file reduction. The gap is the window's construction, not carrier
setup ahead of it: `resumed` creates the window first. The window and layer
already precede the device, runner, layout and display-link setup. No such
work can be moved behind show here; creating before AppKit's launch callback
would change winit's lifecycle ordering. This cut keeps that order and
predicts no additional saving from an earlier show.

**Paired result: a regression.** The qualified traced comparison makes Ink
slower in every pair: +27.319 ms cold (12.018–47.087; n=10) and +28.148 ms
warm (14.852–47.332; n=10). Warm complete screen is 241.785 ms, missing
178 ms and the 170–190 ms range. The first displayed frame is still marked
incomplete; reaching the probe’s image-readiness criterion costs another
three refresh intervals in the warm cohort. No saving is
funded. The one implementation round ends with this result.

Confirmation uses five launches in each cold/warm cohort. Both cohorts
qualify under cut 2’s per-launch rule; the two egui failures remain:

| Arm | Cold complete ms (range; n) | Warm complete ms (range; n) |
| --- | ---: | ---: |
| ink-after | 231.432 (221.537–234.927; n=5) | 247.133 (232.401–258.571; n=5) |
| gpui | 203.588 (194.885–214.864; n=5) | 207.640 (185.754–228.672; n=5) |
| egui | 212.096 (208.695–216.142; n=4) | 218.277 (212.682–224.624; n=4) |

The ten-round pairing retains all 120 attempts: 113 successes and seven
egui readiness failures. The controls use the same executable in both
positions. A paired difference uses only rounds with both originals:

| Arm/cohort | Before complete ms (range; n) | After complete ms (range; n) | Paired after − before ms (range; n) |
| --- | ---: | ---: | ---: |
| ink cold | 208.183 (199.662–229.444; n=10) | 236.078 (228.672–253.761; n=10) | 27.319 (12.018–47.087; n=10) |
| ink warm | 208.756 (205.846–224.701; n=10) | 241.785 (227.863–255.085; n=10) | 28.148 (14.852–47.332; n=10) |
| gpui cold | 207.916 (196.038–214.663; n=10) | 205.576 (196.130–214.981; n=10) | -3.888 (-16.416–14.464; n=10) |
| gpui warm | 201.040 (187.511–213.123; n=10) | 206.260 (196.352–215.376; n=10) | 9.802 (-11.126–15.768; n=10) |
| egui cold | 212.414 (204.198–240.890; n=9) | 209.329 (199.532–219.831; n=9) | 0.006 (-24.433–14.128; n=8) |
| egui warm | 213.511 (205.284–235.077; n=8) | 216.102 (203.686–236.229; n=7) | -5.621 (-15.473–21.412; n=6) |

**Warm timeline, ten launches per carrier.** These are medians from the
same paired cohort, in milliseconds from process start. Callback observation
and the drawable’s displayed timestamp are distinct endpoints.

| Milestone | Cut 2 before | Cut 3 after |
| --- | ---: | ---: |
| AppKit finished launching | 68.103 | 68.454 |
| Window created | 90.018 | 91.709 |
| Window show returned | 96.824 | 97.714 |
| Runner first commit returned | 122.672 | 124.676 |
| First layout completed | 123.935 | 126.062 |
| First paint record | 126.888 | 128.650 |
| First present committed | 152.390 | 157.016 |
| Zero callback observed | 156.167 | 160.665 |
| Display link started | 187.280 | 186.871 |
| First display-link callback | 197.799 | 197.640 |
| First retained-frame repeat | — | 198.276 |
| First present committed after zero | 187.557 | 198.641 |
| Next paint after zero | 185.671 | 221.988 |
| First positive displayed time | 208.756 | 216.785 |
| First positive callback observed | 209.052 | 217.086 |
| Complete screen | 208.756 | 241.785 |
| Complete callback observed | 209.040 | 242.065 |

All thirty after launches (confirmation plus pairing) have **one zero
callback, three repeats, then positive callbacks**. The two-to-ten-zero
prediction misses. Each repeat acquires a drawable and encodes a compose
copy. During those repeats there is no paint walk, scene encoding, text
shaping, texture allocation, image upload or image decode. Every launch has
one paint before the first positive callback and no repeat after it. All
thirty report zero main-thread drawable acquisitions.

The warm median within-launch zero-to-first-repeat interval is 36.686 ms
(27.800–50.768). The display link does not start until the event loop’s
`about_to_wait`; its first callback is late even though show remains early.
The first displayed time is show +117.342 ms (110.043–132.849), not +85.
Every warm after launch then takes 25.000 ms from first display to complete
screen; across cold and warm the range is 25.000–33.333 ms. The first
submission is marked incomplete. Its complete-marked successor waits for
painting to resume while the already submitted copies drain.
Cut 2’s redraw repaints and commits before its first display-link tick;
cut 3 suppresses that redraw and starts copying only on the later tick.
The traces establish these waits; they do not establish an 85 ms composite
floor for this carrier. The show order did not change: its measured warm
median moves 96.824 → 97.714 ms, with no earlier-show saving claimed.

**Four-arm cold row.** `measure.m` runs cold-only with the probe GPU hook,
trace and power sampler off. Each schedule has ten original attempts per
arm. Both qualify. Native retains its snapshot criterion; the painted arms
use displayed complete-screen timestamps. Processes are fresh; caches are
retained, as in cut 2.

| Arm | Before complete ms (range; n) | After complete ms (range; n) |
| --- | ---: | ---: |
| native | 250.616 (236.070–267.914; n=10) | 248.680 (234.345–267.701; n=10) |
| gpui | 199.870 (191.749–209.196; n=10) | 204.925 (190.439–211.687; n=10) |
| ink | 209.686 (200.681–215.664; n=10) | 244.322 (218.293–247.040; n=10) |
| egui | 202.160 (196.389–218.292; n=8) | 214.842 (201.854–224.597; n=10) |

Ink’s **244.322 ms** misses 180–188 ms. It is 39.397 ms slower than
GPUI and 29.480 ms slower than egui, missing the predicted ≥10 ms lead.
The before schedule retains two egui readiness failures (rounds 4 and 5);
all forty after attempts succeed. Together with confirmation and pairing,
the record has 230 attempts, 219 successes and eleven failed egui originals.
None is replaced or removed.

All launches pass ten quiet seconds before launch; all four cohorts retain
a power-absent, error-free monitor record. During-launch maxima below are
report-only. `target/m10-c3/launch-rows.md` gives every launch’s maxima and
status; `silver/*/schedule.json` and `quiet.jsonl` retain their observations.

| Cohort | Other busy cores, max | User-process cores, max | Whole-machine busy cores, max | Minimum quiet seconds |
| --- | ---: | ---: | ---: | ---: |
| confirm | 2.288 | 0.247 | 8.800 | 10.110 |
| paired | 2.137 | 0.674 | 8.722 | 10.108 |
| scoreboard-before | 2.090 | 0.306 | 8.129 | 10.118 |
| scoreboard-after | 1.858 | 0.283 | 8.455 | 10.107 |

**Predictions against results.**

| Prediction | Result |
| --- | --- |
| Warm ≈178 ms, 170–190; show +85 | **Miss:** paired warm 241.785 ms; first display is show +117.342 ms, then the complete screen takes another 25.000 ms |
| Cold 180–188 ms, ≥10 ms ahead of GPUI and egui | **Miss:** 244.322 ms; 39.397 ms behind GPUI and 29.480 ms behind egui |
| Two to ten zero callbacks | **Miss:** exactly one in every one of the thirty traced after launches |
| A drawable and compose copy per tick, no repaint, stopping after first positive | **Pass:** three ticks, three acquisitions and three copies per launch; no paint during them and no repeat after the positive callback |
| Same pixels, CPU-first handoff and continuing live Metal/wgpu | Pixel suites and CPU handoff pass; both live devices draw and their captures are reviewed. No separate capture of the first dropped frame is compared |
| Fling, rest and static unchanged | Not remeasured; the existing rows stand, with no new performance claim |
| No additional saving from moving show earlier | Show ordering stays unchanged; there is no carrier setup to move out of the measured window-construction call |

The passive startup checks pass for an archive miss, a retained archive and
wgpu, each with zero main-thread drawable calls. On the archive miss the
complete CPU screen displays at 185.387 ms and Ink at 318.720 ms. These are
functional observations, not qualified startup medians. Live release
Metal displayed frames advance 13 → 115 and wgpu 13 → 113; the development
socket drive advances carrier frames 6 → 55 and 1 → 39 respectively, and
both report painter `ink`. `boot-functional/validation.json`,
`live-release/validation.json` and `live-dev/validation.json` are empty.
The release captures and the development wgpu capture were inspected.
D7, D11 and D15, including the dated boot and 100 ms application obligations,
remain unchanged.

**Implementation.** `a98e9b83f2` adds a generation-scoped zero-callback bit
in `host/ink/src/device/apple.rs`. `host/apple-painted/src/window.rs`
retains the first submitted frame and repeats it on ticks after that bit is
set, stopping at the first positive callback. Paint and painter handoff wait
until then; resource service continues. An actual resize or an agent
operation that can change pixels releases the retained frame. Chunk 4's
redraw retry remains the backstop.

`host/linux/src/presenter/painter.rs` retains CPU pixels or a reference to
the painter's unchanged compose target. `presenter.rs` exports that handle;
`paint/backend.rs` and `ink.rs` carry the repeat operation to the existing
present worker. Each repeat acquires another drawable and copies that
source. The wgpu tracked layer still records display. Metal presentation
and resource retirement use their existing paths. The window's creation
and show order stay as in cut 2.

**Validation.** The local affected-suite command is:

```sh
EXACT_INK_DEVICE=metal EXACT_REQUIRED=1 RUST_TEST_THREADS=2 \
  cargo test -p exact-linux -p exact-apple-painted -p exact-ink \
  --lib --tests --no-fail-fast
```

It exits 0. `target/m10-c3/checks/affected.log` retains the full output;
`checks/suite-results.txt` retains the suite status lines verbatim. The
presentation-generation test also passes after the new zero-callback
assertions (`checks/callback.log`):

```text
test device::apple::tests::painter_identity_follows_display_and_rejects_a_lost_generations_callbacks ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 46 filtered out; finished in 0.00s
```

Both release and development heavy-list carriers build with the worktree's
own `target/`. `checks/release-build.log` and `checks/dev-build.log` retain
the commands' output. The changed Rust files pass `rustfmt --check`.
The six broad gates remain for the Mac mini as requested; they are not
claimed here. Caps runs locally. No retained file from an earlier cut is
deleted, and no push is made.

**Reproduction.** The signed control bundles and the before carrier are the
retained cut 2 arms; after is `a98e9b83f2`, rebased onto `47bdcd085f`.
`target/m10-c3/build-context.json` records tool versions and the build command;
`after-source.patch` is this cut’s six-file source delta. The prediction is
committed before the run in `22e11bd5f7`. No new benchmark hook is added.

Raw files are retained locally at `target/m10-c3/silver/` and on the silver at
`ccheever@100.76.94.78:~/projects/exact2-measure/target/g0/m10-c3/`.
`collection-final.json` retains each collection command, PID and exit;
`provenance.json` and each cohort’s copy hash the executables. Every signed
measurement arm receives a throwaway launch before collection.

To repeat, use a fresh artifact directory, copy the retained scripts and
bundles, record their hashes and run `run-final.py` in the silver’s GUI
session under `caffeinate`. It runs the five-per-arm confirmation, the ten
paired rounds, and both ten-per-arm four-arm cold-only schedules. Preserve
all originals. `live-dev.py` separately drives the development carrier’s
socket with both devices. Reproduce the published reductions without a
window:

```sh
python3 -B target/m10-c3/report.py
python3 -B target/m10-c3/launch-rows.py
```

`report.json`, `tables.md`, `launch-rows.md` and `checks/final-reduction.log`
retain the reductions. `raw-sha256.json` hashes the retained raw files;
`analysis-sha256.json` hashes the reduction scripts. The final staged caps
output is retained in `checks/caps-final.log`:

```text
All budgets within cap. 10 categories inspected.
```

## M10, cut 4: request the pictures earlier and re-present the latest frame

**Predictions before the run, 2026-10-31.** Correct cut 3's hold: a new
paint replaces the frame repeated after a zero callback; repetition ends
at the first positive callback. Request image metadata when the first
commit exposes the image nodes and request sized decodes at first layout.
The data source's `bind` precedes those nodes: it names neither the rows
nor their resolved content boxes. Keep the two image workers, admission,
slot patching, paint contract and resource retirement unchanged.

| Prediction | Instrument |
| --- | --- |
| Pictures decoded and uploaded by about 150 ms warm, from about 185 | Per-picture request, decode and upload spans on the existing probe, with thread IDs; separate before/after diagnostics |
| Complete frame presented by about 160 ms; complete screen 185–195 ms warm, from cut 2's 208.756 | `startup.py`, GPU hook off, trace on, power off; five cold/warm confirmation launches per arm, then ten paired rounds |
| Cold complete screen 190–200 ms, below GPUI's 204.925 and egui's 214.842 | `measure.m` cold-only, hook/trace/power off; ten original launches per arm, before and after |
| One zero callback or a few; each repeat uses the latest painted frame and stops after the first positive callback | The traced launches |
| First displayed pixels, CPU-first handoff and live Metal/wgpu remain correct | Affected suites and functional carrier drives on silver |
| Fling, rest and static unchanged | Existing rows stand; this cut does not remeasure them |

The retained cut 2 warm trace has five unique decodes, not twenty. Five
jobs need at least three rounds on two workers; twenty would need ten.
The before measurement carrier is cut 2 (`19f0758ca6`, rebased later as
`8116a09a55`); all failed originals remain with successful n. Each signed
arm gets a throwaway launch and each measurement ten quiet seconds first.
Raw evidence stays under `target/m10-c4/` and its silver copy. One round;
the six broad checks are deferred to the Mac mini. No push.

**Outcome.** The first positive display is complete in all thirty traced
Ink after launches. The cut 3 hold is corrected, but the timing targets miss.
Warm complete screen is 218.772 ms. Qualified four-arm cold is 224.299 ms,
against GPUI's 209.354 and egui's 218.137 ms. The paired ranges cross zero;
no consistent complete-screen saving is established in this one round.

| Prediction | Result |
| --- | --- |
| Pictures ready by about 150 ms warm | **Miss:** last decode ends at 154.513 ms; the final upload is issued at 196.685 ms |
| Complete frame presented by about 160 ms | **Miss:** complete-frame commit is 197.368 ms |
| Complete screen 185–195 ms warm | **Miss:** 218.772 ms, n=10 |
| Cold complete screen 190–200 ms, below GPUI and egui | **Miss:** 224.299 ms, n=10; both controls are faster |
| One zero callback or a few; repeats use the latest frame and end at first positive | **Met:** one zero and one or two repeats in all thirty Ink after traces; every first display is complete; none repeats after the positive callback |
| First displayed pixels, CPU-first handoff and live Metal/wgpu remain correct | **Pass:** identical RGBA first-screen content against each matching cut 2 backend; handoff and both live drives pass |
| Fling, rest and static unchanged | **Not remeasured:** existing cost rows stand; live drives establish operation only |

**Four-arm cold rows.** Both schedules qualify: `measure.m` cold-only,
GPU hook, trace and power sampling off, fresh processes and retained caches.
Native retains its snapshot criterion; painted arms use displayed timestamps.
Every cold original succeeds. All 230 requested attempts remain, with three
egui readiness failures in the traced cohorts and no replacements.

| Arm | Before, ms (n) | After, ms (n) |
| --- | ---: | ---: |
| Native | 269.169 (10) | 253.631 (10) |
| GPUI | 211.914 (10) | 209.354 (10) |
| Ink | 222.856 (10) | 224.299 (10) |
| egui | 223.831 (10) | 218.137 (10) |

The launch rule gates the ten seconds before launch. During-launch load
maxima are reported, not grounds for discarding a launch. No power sampler
overlaps any cohort, and every monitor finishes without error.

| Cohort | Minimum quiet seconds | Max non-benchmark busy cores | Max user-process cores | Max whole-machine busy cores |
| --- | ---: | ---: | ---: | ---: |
| confirm | 10.111 | 3.759 | 0.293 | 7.864 |
| paired | 10.109 | 2.419 | 0.485 | 9.267 |
| scoreboard-before | 10.105 | 1.909 | 0.327 | 7.573 |
| scoreboard-after | 10.111 | 1.757 | 0.242 | 7.965 |

Ranges, original filenames, qualification flags and failed-launch records
remain in `report.json` and `launch-rows.md`.

**Traced cohorts.** Confirmation and pairing qualify under cut 2's
per-launch rule: ten quiet seconds before each launch, GPU hook and power
sampling off, trace on. Failed originals remain without replacement.

| Confirmation | Cold, ms (n) | Warm, ms (n) |
| --- | ---: | ---: |
| Ink after | 218.230 (5) | 223.174 (5) |
| GPUI | 211.701 (5) | 216.540 (5) |
| egui | 223.018 (5) | 222.373 (4) |

| Paired cohort | Before, ms (n) | After, ms (n) | Median paired change, ms (paired n) | Paired range, ms |
| --- | ---: | ---: | ---: | ---: |
| ink cold | 225.943 (10) | 222.686 (10) | -1.123 (10) | -22.023 to +15.950 |
| ink warm | 226.276 (10) | 218.772 (10) | -4.727 (10) | -23.655 to +20.244 |
| gpui cold | 214.818 (10) | 210.949 (10) | -2.351 (10) | -15.734 to +32.067 |
| gpui warm | 212.843 (10) | 211.677 (10) | +0.854 (10) | -18.124 to +19.614 |
| egui cold | 222.927 (10) | 224.403 (10) | -1.756 (10) | -17.070 to +25.685 |
| egui warm | 229.206 (9) | 230.520 (9) | +1.741 (8) | -23.868 to +23.074 |

All Ink and GPUI originals succeed. egui fails confirmation round 5 warm,
paired after round 8 warm and paired before round 9 warm, each at readiness.
The two failed paired launches leave eight matched warm egui pairs.
Ink's paired ranges cross zero: this round does not establish a consistent
complete-screen saving. The contemporary cut 2 warm median is 226.276 ms;
the brief's 208.756 ms is the earlier cohort, not this before measurement.

**Warm timeline.** These are medians of ten original launches per carrier,
in process-relative milliseconds. Before is the retained cut 2 binary.
Its existing unlabelled decode/upload spans supply the aggregate image
endpoints; source-ID spans below use the separate diagnostic rebuild. Phase
timestamps and per-launch intervals are reduced separately.

| Event | Cut 2 | Cut 4 |
| --- | ---: | ---: |
| Window show returns | 105.035 | 101.002 |
| Data bind | 114.623 | 111.152 |
| First commit returns | 130.817 | 127.686 |
| First layout completes | 132.129 | 128.984 |
| First paint starts | 136.715 | 131.605 |
| Last visible-picture decode ends | 164.832 | 154.513 |
| First present commits | 165.073 | 159.089 |
| Last picture upload is issued | 206.077 | 196.685 |
| Complete frame commits | 206.565 | 197.368 |
| First display; complete screen | 226.276 | 218.772 |

Cut 4 requests sized decodes at 128.999 ms, a median 0.017 ms after
layout; the instrumented before arm takes 1.439 ms after layout. The last
decode ends at 154.513 ms, but the final upload is issued at 196.685 ms.
The median per-launch gap is **43.711 ms**, versus **1.557 ms** summed
picture-upload CPU work. Waiting for the paint that consumes the decoded
pictures dominates the upload step. The complete successor commits at
197.368 ms, missing about 160 ms. The first positive display follows show
by a median **118.928 ms**, not the assumed 85 ms; complete screen is
**218.772 ms**, missing 185–195 ms. Moving the request does not make uploads
happen as soon as the workers finish.

All thirty Ink after traces have exactly one zero callback, two paints before
the first positive callback and one or two repeats. Every first positive
display is complete. No repeat occurs after that callback, and no drawable
is acquired on the main thread. The repeats now carry the complete
successor; the incomplete first frame no longer blocks it.

**Per-picture spans.** The following warm traces are nearest their cohort's
median complete-screen time: diagnostic before
`before-picture-spans/ink-before-trace-1-warm.json` and after
`paired/ink-after-1-warm.json`. Each cell is start–end in process-relative
milliseconds. Decode thread IDs are in parentheses; every upload below is
on main thread 259. Thread IDs are local to each trace.

| Source ID | Before decode (thread) | After decode (thread) | Before upload | After upload |
| --- | ---: | ---: | ---: | ---: |
| 1 | 128.779–130.579 (62647) | 129.173–130.935 (49415) | 201.379–201.554 | 132.343–132.620 |
| 2 | 128.786–130.560 (61467) | 129.181–130.929 (62003) | 201.579–201.651 | 145.659–145.819 |
| 3 | 153.556–166.757 (61467) | 131.664–145.185 (62003) | 201.654–202.428 | 195.372–196.187 |
| 4 | 153.557–165.852 (62647) | 131.666–143.787 (49415) | 202.430–202.657 | 196.197–196.480 |
| 5 | 167.897–168.252 (62647) | 155.973–156.464 (62003) | 202.664–202.702 | 196.489–196.543 |

`picture-spans.json` retains every span and each cohort's medians, including
the ten-launch paired before timeline. `picture-examples.json` identifies
the displayed examples. Neither table treats CPU upload submission as
GPU completion. The first complete displayed frame remains the endpoint.

**Implementation and scope.** `35eea646e6` starts embedded-image metadata
work after the first commit, in `Host::boot_decoded`, and asks for sized
decodes immediately after the first layout. `Heavy::bind` has no image
nodes or resolved boxes. Selected assets retain their metadata/integrity
gate. The initial mounted set names ten sources; the first screen decodes
five, and the existing visibility pass cancels the other requests. Two
persistent image workers serve them; five jobs cannot fit one decode round.

The carrier permits ordinary redraws while awaiting its first positive
callback. Each new successful paint replaces the retained frame. A tick
services arrivals and paints any damage first; otherwise it repeats the
latest frame after a zero callback. The first positive callback ends the
repeat path. The paint walk, image-slot patching, CPU-first handoff,
compose target and retirement rules are unchanged.

`image/workers.rs` records the source ID on each decode span;
`image/bitmap.rs` carries that ID into `ink/images.rs`'s picture-upload
span. Both spans retain start, end and thread. The upload span measures
the CPU's texture creation, pixel upload and mip submission, not a
per-picture GPU-completion timestamp. GPU submission and display remain
separate events in the existing trace.

The picture-span before arm rebuilds cut 2's window scheduling on this
worktree's rebased dependencies, with the same source-ID instrumentation.
Its five cold and five warm launches are a diagnostic control; each cited
warm launch follows its cold partner, with no separate throwaway launch
for this extra arm. The paired and four-arm before arm is the original
retained cut 2 carrier, source
`19f0758ca6` (later rebased as `8116a09a55`); the diagnostic rebuild does
not replace it. `before-trace.patch`, `after-source.patch` and
`build-context.json` retain that distinction.

**Validation.** The affected-suite command exits 0:

```sh
EXACT_INK_DEVICE=metal EXACT_REQUIRED=1 RUST_TEST_THREADS=2 \
  cargo test -p exact-linux -p exact-apple-painted -p exact-ink \
  --lib --tests --no-fail-fast
```

`target/m10-c4/checks/affected.log` retains the full output, and
`checks/final-suite-results.txt` retains the suite status lines verbatim.
The new regression verifies that metadata can finish before layout without
starting a decode, that layout sizes the decode, and that symbols still
report their intrinsic size. Its final focused run reports:

```text
test image::control_tests::boot_metadata_waits_for_layout_size_before_decoding ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1717 filtered out; finished in 0.03s
```

Both release and development carriers build into this worktree's own
`target/`. The changed Rust files pass `rustfmt --check`. The full six-check
gate remains for the Mac mini; affected suites and caps run here. No push
is made, and no earlier cut's retained artifact is removed.

**Functional drives and pixels.** On the silver, an empty application
archive displays the complete CPU frame at 184.325 ms and the complete Ink
frame at 334.325 ms. Retained Metal and wgpu display complete screens;
all three boot checks pass, with no main-thread drawable acquisition.
These are functional observations, not qualified startup medians.

Release Metal advances its displayed-frame counter from 12 to 113 under
the posted wheel drive; wgpu advances from 12 to 111. The development
socket drive advances carrier frames from 2 to 40 on Metal and 3 to 39 on
wgpu. All functional validation files are empty, and all drives exit 0.

Untouched first-screen captures match cut 2 exactly for each backend:
**zero changed RGBA content pixels**, 1600×1536. The capture runs also record
that the complete screen was the first displayed screen. The initial
cross-backend wgpu-to-Metal comparison differs on 0.055542% of RGB pixels,
with no channel difference above 8; the matching cut 2 wgpu reference
removes that difference. Its pre-run zero-difference prediction is retained
in `first-screen-wgpu-prediction.txt`. All captures are retained and the
Metal first screen and both release captures were inspected.

`first-screen-compare.mjs` uses the repository's existing PNG reader and
comparator, removing only the 32-point native title bar from its content
comparison. `first-screen/comparison.json` retains the full-window and
content comparisons, including the cross-backend result. Reproduce it with
`bun target/m10-c4/first-screen-compare.mjs`. No renderer revision follows
the timing results; this is one implementation round.

**Reproduction.** The prediction is committed before the run in `2e15057801`.
The implementation is `35eea646e6`; `build-context.json` records the rebase,
tool versions, worktree and build command. All arms use the worktree's own
build output or the retained signed controls. The silver needs no go file.

Raw files are retained at `target/m10-c4/silver/` locally and at
`ccheever@100.76.94.78:~/projects/exact2-measure/target/g0/m10-c4/`.
`collection-final.json` records commands, PIDs and exits; each cohort's
`provenance.json` hashes its executables. Re-run the retained `run-final.py`
in a fresh artifact directory, in the silver's GUI session under
`caffeinate`, to collect the five-round confirmation, ten paired rounds and
two ten-round four-arm cold schedules. Each signed arm receives a throwaway
launch first. Failed originals are retained, never replaced.

Reproduce the tables and image spans without opening a window:

```sh
python3 -B target/m10-c4/report.py
python3 -B target/m10-c4/picture-spans.py
python3 -B target/m10-c4/launch-rows.py
```

`report.json`, `tables.md`, `picture-spans.json`, `picture-examples.json`
and `launch-rows.md` retain the reductions. `raw-sha256.json` hashes the
retained raw files; `analysis-sha256.json` hashes the reduction scripts.
The first-screen capture and development socket checks are separate
functional drives, not qualified startup samples. Fling, rest and static
costs are not remeasured; their existing rows stand. D7, D11 and D15,
including the dated boot and 100 ms application obligations, are unchanged.

The final staged caps output is retained in `checks/caps-final.log`.

## M10, cut 5: paint landed pictures on the display tick

**Predictions before the run, 2026-10-31.** Cut 4's retained warm trace
`target/m10-c4/silver/paired/ink-after-1-warm.json.trace.json` starts the
display link only after the initial AppKit redraw. Its first `winit dispatch`
is 95.880 ms after show; `display running` follows at 95.925 ms. The
first paint has already finished at show +55.181 ms and the last decode
at show +55.543 ms. No display tick can service that interval yet.

Start the link before the first paint. Drain picture arrivals before painting
on its tick, and permit one immediate successor during boot for arrivals
that land during a paint. The successor uses the ordinary image-slot path;
later arrivals remain damage for the next tick. The link's existing hidden
and idle rules remain. Cuts 1–4, CPU handoff, D7, D11 and D15 remain.

| Prediction | Instrument |
| --- | --- |
| Complete-frame commit about 165 ms warm, from 197.368 | Existing per-picture and presentation trace, `startup.py`; five confirmation and ten paired launches per arm and cache label |
| First complete display 185–195 ms warm, from 218.772 | Same traced launches; the show +80 ms floor is about 181 ms |
| Cold complete display 190–200 ms, below GPUI's 209.354 and egui's 218.137 | `measure.m` cold-only, ten launches per arm before and after, same order as cut 4; hook, trace and power off |
| One zero callback, with cut 4's one or two repeats; latest frame repeated until positive display | Traced launches |
| Zero changed first-screen content pixels; CPU-first handoff and live Metal/wgpu pass | Matching cut 4 backend captures, affected suites and functional drives |
| Fling, rest and bar unchanged | One fling/rest and one bar sample per arm; hook, trace and power off, CPU clock counters retained |

Before is cut 4's retained `35eea646e6` carrier. The lane rebased onto
`origin/main` before implementation. All originals, including failures,
remain under `target/m10-c5/` and its silver copy. Medians report successful
n and qualification. One implementation and measurement round; no push.
The six broad checks go to the Mac mini; affected suites and caps run here.

**Outcome.** The complete screen is the first displayed frame in all thirty
traced Ink after launches, but the next-tick boot goal is **unmet**. Warm
complete-frame commit is **209.712 ms** and complete display **232.957 ms**.
Starting the display link earlier moves callbacks earlier; winit still
delivers their queued events only at its first main-thread dispatch.

| Prediction | Result |
| --- | --- |
| Complete-frame commit about 165 ms warm | **Miss:** 209.712 ms, n=10 |
| Complete display 185–195 ms warm | **Miss:** 232.957 ms, n=10 |
| Cold complete display 190–200 ms, below GPUI and egui | **Miss:** 219.046 ms, n=10; slower than GPUI 209.626, faster than egui 223.863 |
| One zero callback and one or two repeats | **Partial:** one zero in all thirty after traces; two or three repeats, so the repeat-count prediction misses; none follows the first positive callback |
| Zero changed first-screen pixels; CPU-first handoff and live Metal/wgpu pass | **Met:** matching-backend first screens have identical RGBA pixels; CPU handoff and release/development drives pass |
| Fling, rest and bar unchanged | **Not established:** one after sample per arm/workload, without matched cut 4 cost samples; cadence and idle checks pass, but egui fling/rest fails quiet qualification |

**Four-arm cold rows.** Both schedules qualify: `measure.m` cold-only,
hook, trace and power off, ten original launches per arm in the same
order as cut 4. Native retains its snapshot criterion; painted arms use
displayed timestamps. All eighty cold originals succeed. The preceding
session is retained beside this session's cut 4 before and cut 5 after:

| Arm | Cut 4 published after, ms (n) | This session before, ms (n) | This session after, ms (n) |
| --- | ---: | ---: | ---: |
| native | 253.631 (10) | 255.337 (10) | 254.012 (10) |
| gpui | 209.354 (10) | 214.492 (10) | 209.626 (10) |
| ink | 224.299 (10) | 221.389 (10) | 219.046 (10) |
| egui | 218.137 (10) | 223.202 (10) | 223.863 (10) |

Ink's untraced median falls 2.343 ms. GPUI falls 4.866 ms and native
1.325 ms; egui rises 0.661 ms. These are differences of cohort medians,
not paired changes or evidence that the boot deadline was met. The traced
paired medians below move in the other direction for Ink. No consistent
startup saving is established.

Every launch passes ten quiet seconds. During-launch load maxima are
reported, never used to discard an original. All four cohorts finish with
power sampling absent and no monitor error. All **230 originals** remain,
including five egui readiness failures in the traced cohorts.

| Cohort | Minimum quiet seconds | Max non-benchmark busy cores | Max user-process cores | Max whole-machine busy cores |
| --- | ---: | ---: | ---: | ---: |
| confirm | 10.105 | 1.995 | 0.248 | 8.571 |
| paired | 10.106 | 2.739 | 1.097 | 8.673 |
| scoreboard-before | 10.111 | 1.947 | 0.235 | 8.296 |
| scoreboard-after | 10.111 | 2.192 | 0.363 | 8.923 |

`report.json` and `launch-rows.md` retain each original filename, range,
successful n, failure and load observation.

**Traced confirmation and pairing.** Both cohorts qualify under the
per-launch rule: ten quiet seconds before each launch; hook and power
sampling off, trace on. Fresh processes retain their caches; each warm
launch follows its cold partner. All Ink and GPUI originals succeed.
Confirmation loses egui round 4 cold. Pairing loses egui before round 1
cold and after round 1 warm, round 4 cold and round 9 cold. Each failed
original remains, with no replacement.

| Confirmation | Cold, ms (range; n) | Warm, ms (range; n) |
| --- | --- | --- |
| ink-after | 221.338 (209.600–230.700; n=5) | 221.704 (218.076–229.991; n=5) |
| gpui | 220.443 (210.787–230.296; n=5) | 207.317 (205.588–213.198; n=5) |
| egui | 218.958 (216.410–232.538; n=4) | 224.333 (221.382–247.724; n=5) |

| Paired cohort | Before, ms (range; n) | After, ms (range; n) | Paired after − before, ms (range; n) |
| --- | --- | --- | --- |
| ink cold | 225.594 (215.077–234.709; n=10) | 229.545 (212.194–240.289; n=10) | 1.317 (-16.932–19.208; n=10) |
| ink warm | 223.870 (210.758–230.502; n=10) | 232.957 (209.302–241.700; n=10) | 8.486 (-13.634–23.826; n=10) |
| gpui cold | 211.592 (206.882–224.687; n=10) | 214.024 (205.380–224.297; n=10) | -0.604 (-16.026–17.415; n=10) |
| gpui warm | 213.729 (204.576–227.658; n=10) | 211.143 (204.007–221.872; n=10) | -5.884 (-15.127–8.292; n=10) |
| egui cold | 221.527 (213.350–234.043; n=9) | 219.040 (213.678–229.421; n=8) | -8.912 (-14.402–14.342; n=7) |
| egui warm | 224.668 (213.281–243.037; n=10) | 225.904 (211.813–245.335; n=9) | 0.379 (-17.132–13.000; n=9) |

Ink's median paired changes are **+1.317 ms cold and +8.486 ms warm**.
Both ranges cross zero. This round establishes no complete-screen saving;
the warm after median is slower. The contemporary cut 4 warm median is
223.870 ms; the brief's 218.772 ms is from the preceding session.

**Warm timeline.** Medians of ten original launches per carrier, in
process-relative milliseconds. Intervals are reduced per launch; a
difference of phase medians is not the median interval.

| Event | Cut 4 this session | Cut 5 |
| --- | --- | --- |
| Window show returns | 97.837 | 104.217 |
| Data bind | 108.136 | 114.467 |
| First layout completes | 126.223 | 132.494 |
| Display link starts | 200.508 | 136.198 |
| First paint starts | 129.638 | 136.323 |
| First display-link callback | 212.470 | 147.707 |
| Last visible-picture decode ends | 153.518 | 160.629 |
| First present commits | 157.968 | 165.220 |
| First tick delivered | 212.489 | 208.956 |
| Last picture upload is issued | 200.317 | 209.144 |
| Complete frame commits | 200.912 | 209.712 |
| First display; complete screen | 223.870 | 232.957 |

**Wake before and after.** Cut 4 starts the display link in its first
`about_to_wait`, after AppKit's initial redraw transaction. Image-worker
`Event::Wake` notifications and the redraw transaction service image
arrivals; the complete paint precedes the first delivered display tick.
There is no running display link during the decode-to-paint gap.

Cut 5 starts the link before painting. It calls the ordinary paint path
directly from a delivered `Event::Tick` when an image slot owes paint,
and permits one immediate successor for arrivals during the first paint.
The first callback now occurs at 147.707 ms, but its main-thread delivery
is at 208.956 ms. The median per-launch callback-to-delivery interval is
**64.292 ms**. The first callback after the last decode is at 165.241 ms;
the next carrier draw is still at 208.956 ms. The desired next-tick upload
does not happen.

The last decode finishes a median **0.323 ms after the immediate successor
starts** (0.275–0.366 ms in the ten warm launches). That successor uploads
pictures 3 and 4. Picture 5 misses its arrival poll and waits for the
delivered tick. Last decode to final upload completion is **48.809 ms**,
against **1.389 ms** summed picture-upload CPU work. Earlier callbacks
alone do not remove the AppKit/winit dispatch delay. The trace locates
that interval; it does not attribute the work inside AppKit.

All thirty traced Ink after launches display a complete first frame:
**one zero callback, three paint walks and two or three repeats**.
No repeat follows the first positive callback, and no drawable acquisition
runs on the main thread. The one-zero prediction passes; the one-or-two
repeat prediction misses.

**Per-picture example.** `silver/paired/ink-after-5-warm.json` is nearest
the paired warm median. Cells are process-relative start–end milliseconds;
decode thread IDs are in parentheses. All uploads run on main thread 259.
These spans measure CPU upload submission, not GPU completion.

| Source ID | Decode (thread) | Upload |
| --- | --- | --- |
| 1 | 124.401–126.219 (13363) | 128.168–128.452 |
| 2 | 124.413–126.200 (58131) | 141.869–142.000 |
| 3 | 127.476–140.911 (13363) | 152.909–153.442 |
| 4 | 127.480–139.677 (58131) | 153.447–153.797 |
| 5 | 152.637–153.108 (13363) | 206.805–206.902 |

`picture-spans.json` retains every source span, and `wake-spans.json`
retains the callback, delivery, successor and idle counts for each trace.
The example is reproduced by selecting the warm after launch closest to
the median complete-screen time.

**Implementation.** The measured candidate is `94638ea2d8`. The presenter
keeps `image_paint_pending` when image reports land, including arrivals
accepted by the visibility sync after a paint. A successful paint consumes
that demand before the next visibility sync. The carrier paints this demand
in `Event::Tick`, without calling `request_redraw` first. The image worker's
existing wake and the presenter's existing dirty flag start a stopped link;
no new timer or periodic poll is added. A paint with no remaining demand
leaves the link's existing idle and hidden rules in force.

The carrier starts its link before the first paint, instead of waiting for
`about_to_wait`. During boot it polls arrivals after that paint and permits
one immediate successor. The successor replaces the retained frame through
cut 4's ordinary paint and submission path. Resource retirement, the CPU
startup device, the compose target and the two image workers are unchanged.

Winit 0.30's `platform_impl/macos/app_state.rs::cleared` delivers queued user events
before its redraws and `AboutToWait`; starting `CVDisplayLink` earlier
starts callbacks but does not make AppKit reach that dispatch earlier.

**Validation.** Release and development carriers build with this worktree's
own `target/`. The affected command exits 0:

```sh
EXACT_INK_DEVICE=metal EXACT_REQUIRED=1 RUST_TEST_THREADS=2 \
  cargo test -p exact-linux -p exact-apple-painted -p exact-ink \
  --lib --tests --no-fail-fast
```

It passes six carrier tests, 46 Ink tests and 1,697 Linux library tests,
plus every Linux integration binary, including 104 pinned tests. The
image-demand regression gates a server response until after the initial
paint, observes the arrival's demand, paints its pixels, then verifies
that neither demand nor an idle deadline remains. Its final focused output:

```text
test presenter::images::tests::a_landed_picture_owes_one_paint_and_no_idle_deadline ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1718 filtered out; finished in 0.24s
```

The named suite output is verbatim:

```text
paint: test result: ok. 56 passed; 0 failed; 1 ignored; 0 measured; 48 filtered out; finished in 3.87s
borders: test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 0.73s
tint: test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 0.49s
backdrop: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out; finished in 0.95s
gradients: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 0.51s
visual: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 1.95s
materials: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 1.26s
image: test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 130 filtered out; finished in 1.64s
```

`target/m10-c5/checks/affected.log` retains the full affected-suite output;
`suite-results.txt`, `named-commands.json` and the individual suite logs
retain the commands and status lines. Changed Rust files pass formatting,
and staged caps passes all ten categories. The first release build had an
error-conversion compile error, corrected before any candidate run; both
build logs remain. The broad six-check gate is deferred to the Mac mini
as requested. No push is made.

**Functional drives.** With an empty application archive, the complete CPU
frame displays at 181.594 ms and the complete Ink frame at 339.928 ms.
Retained Metal displays at 187.646 ms and wgpu at 238.019 ms. All three
checks pass, with no main-thread drawable acquisition. These are separate
functional observations, not qualified startup medians.

The release wheel drive advances displayed frames from 13 to 115 on Metal
and 13 to 114 on wgpu. `live-release/validation.json` is empty. Their subsequent
idle intervals, 1.121 seconds each, contain no display callback, display
wake, paint, present commit or first-frame repeat. Both release screenshots
were inspected. This establishes that the link stops after boot and input;
the ten-second, trace-off rest samples below supply the cost observation.

The development socket drives advance carrier frames from 3 to 39 on
Metal and 3 to 41 on wgpu; both keep the Ink painter and exit 0.
`live-dev/validation.json` is empty.

The first-screen comparison passes against cut 4 for both Metal and wgpu:
**zero changed RGBA content pixels**, 1600×1536. The full 1600×1600 windows
also match. Each after capture records that its complete screen was the
first displayed screen. Both after captures were inspected.
`first-screen/comparison.json` retains the exact comparisons;
`first-screen/validation.json` is empty.

**Fling, rest and bar samples.** One original fling/rest and one bar
sample per arm, hook, trace and power off, with the committed driver and
wheel helper. All eight launches complete without focus loss, thermal
change or Low Power Mode. Seven pass every five-second quiet observation.
The egui fling/rest run has one observation at **1.060 non-benchmark
cores**, above 1.0; both of its phase rows are excluded. Its largest user
processes are two Fleet processes at 0.215 and 0.208 core. No sample is
replaced. `costs/schedule.json` and each sample's `quiet.jsonl` retain the
gate, load observations and qualification.

These are individual observations, **n=1**, not medians or a replacement
cost scoreboard. Native cadence is its display link; painted cadence is
displayed drawables over the full phase. Native rest has no cadence row.
Effective GHz and IPC use the retained phase-endpoint counters.

| Arm | Phase | Quiet | CPU ms/s | Main ms/s | Mcycles/s | Minstructions/s | GHz | IPC | Frames/s |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| native | fling | yes | 172.220 | 119.483 | 493.334 | 1262.462 | 2.865 | 2.559 | 118.462 |
| native | rest | yes | 3.162 | 1.870 | 4.629 | 3.597 | 1.464 | 0.777 | — |
| gpui | fling | yes | 168.868 | 91.471 | 349.238 | 502.155 | 2.068 | 1.438 | 89.180 |
| gpui | rest | yes | 5.392 | 3.593 | 7.586 | 9.334 | 1.407 | 1.230 | 0.000 |
| ink | fling | yes | 146.319 | 72.753 | 296.189 | 528.330 | 2.024 | 1.784 | 33.387 |
| ink | rest | yes | 3.552 | 3.381 | 5.269 | 5.140 | 1.484 | 0.975 | 0.000 |
| egui | fling | **excluded** | 125.286 | 62.571 | 274.466 | 371.341 | 2.191 | 1.353 | 41.377 |
| egui | rest | **excluded** | 1.258 | 1.118 | 1.858 | 2.162 | 1.477 | 1.163 | 0.000 |
| native | bar | yes | 11.791 | 10.532 | 17.127 | 21.624 | 1.453 | 1.263 | 119.952 |
| gpui | bar | yes | 203.856 | 143.783 | 379.114 | 478.993 | 1.860 | 1.263 | 119.831 |
| ink | bar | yes | 206.719 | 109.766 | 364.835 | 414.774 | 1.765 | 1.137 | 119.524 |
| egui | bar | yes | 205.932 | 114.321 | 463.120 | 336.404 | 2.249 | 0.726 | 119.706 |

| Fling arm | p95, ms | Late intervals/s | Input received / posted | Peak, MiB | End of rest, MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| native | 8.333 | 1.400 | 8 / 424 | 232.611 | 217.689 |
| gpui | 8.333 | 1.342 | 420 / 424 | 414.501 | 210.736 |
| ink | 8.333 | 0.537 | 421 / 424 | 600.486 | 241.392 |
| egui (excluded) | 8.333 | 0.268 | 412 / 424 | 515.095 | 174.736 |

All four flings post 424 events. Late intervals use the committed
aggregator's >12.5 ms threshold and gesture-window denominator for the
painted arms; native uses the full phase. Input windows, driver gaps and
the individual `aggregate.mjs --sample` outputs are retained.

Ink's 33.387 fling frames/s, zero rest presents and 119.524 bar frames/s
are consistent with the existing cadence rows. Its 146.319 / 3.552 /
206.719 process CPU ms/s do **not** establish unchanged CPU cost. This
round has one after sample per workload and no matched cut 4 cost sample;
G4 lacks clock counters and G5m uses a different hook/power configuration.
No cross-series CPU saving or equivalence is claimed. The prediction that
all three cost rows stay unchanged is therefore **not established**, and
egui's fling/rest comparison is unavailable under the quiet rule.

The post-boot scheduling argument has direct evidence: consuming an image
arrival leaves no idle deadline in the regression test; the live traces
have no idle display-link callbacks, paints or presents; and the ten-second
Ink rest sample has zero displayed frames. The link stops when idle.

**Reproduction.** Predictions are committed in `7b4a216652`, before any
candidate run. The measured implementation is `94638ea2d8`, based on
`origin/main` `7ca04fac1c`; the before carrier is cut 4's retained
`35eea646e6`. `build-context.json` records the worktree and release command.
Both builds use this worktree's own `target/`. Native, GPUI and egui are
cut 4's retained signed controls. `instrument-diff.patch` is empty: the
committed probe, driver and wheel helper are unchanged. The two Ink probes
have the same SHA-256, recorded in `silver/probe-hashes.json`.

Raw files remain in `target/m10-c5/silver/` and
`ccheever@100.76.94.78:~/projects/exact2-measure/target/g0/m10-c5/`.
The silver is dedicated and requires no go file. `collection-final.json`
records each command, PID and exit; cohort provenance files hash the signed
executables. All freshly signed arms receive a throwaway launch before the
fixed schedule. No failed original is replaced. Earlier cuts' artifacts
remain untouched.

`run-final.py` records confirmation, pairing, the before and after cold
schedules, the cost collector and first-screen captures in that order.
The cost collector's first CPU-tick observation had a zero-length interval
and stopped before launching any sample. Its log and setup directory without samples
remain as `costs-session.log` and `costs-setup-failed/`. `costs-run.py` adds
the monitor's normal initial five-second interval; the original script
remains. `collection-supplement.json` records that collection and the
development socket drives after the first-screen captures. No measured
sample or renderer configuration is replaced.

To repeat collection, prepare a fresh artifact directory and run the
retained commands under `caffeinate` in the silver's GUI session, using
the corrected cost collector. The first-screen and development socket
checks are separate functional drives, not startup samples.
Reproduce the reductions without opening a window:

```sh
python3 -B target/m10-c5/report.py
python3 -B target/m10-c5/picture-spans.py
python3 -B target/m10-c5/wake-spans.py
python3 -B target/m10-c5/launch-rows.py
python3 -B target/m10-c5/cost-report.py
bun target/m10-c5/first-screen-compare.mjs
```

The corresponding JSON and Markdown files retain all reduced values and
original filenames. `raw-sha256.json` hashes the retained raw files;
`analysis-sha256.json` hashes the reducers. The cost reducer also retains
the committed `aggregate.mjs --sample` output for every original. Pixel
comparison uses the repository's PNG reader and removes only the native
title bar. D7, D11 and D15, including the dated device and 100 ms application
obligations, are unchanged. This is one implementation and measurement
round, with no second renderer configuration after the timing miss.

## M10, cut 6: the boot ticks reach the carrier when they fire

**Predictions, before the candidate run (2026-10-31).** On the silver Mac,
using cut 5's linked probe and startup driver, hook and power sampling off:
first delivered tick within one 8.33 ms period of its callback (about
156 ms); final upload by about 170 ms; complete-frame commit by about
175 ms; warm complete display 190–200 ms; cold complete display 195–205 ms,
below the contemporaneous GPUI and egui medians. Fling, rest and bar should
remain unchanged. Matching-backend first-screen pixels should remain
identical; CPU-first handoff, live Metal and live wgpu should pass.

The cut-5 diagnostic identifies a boundary before AppKit's first display
cycle. Two ordinary launches finish their first paint and immediate
successor at 170.003 and 167.961 ms, enter the run loop at 171.041 and
168.922 ms, and deliver their first winit tick at 208.345 and 214.251 ms.
The corresponding main-queue checkpoints arrive at 206.452 and 191.123 ms.
Winit's proxy source has a no-op callback; user events drain at its
last-priority `BeforeWaiting` observer. Its source runs, but neither that
source nor dispatch to the main queue preempts AppKit's entry observers.
The sampled stack names UpdateCycle, CATransaction's flush, native window
layout and SwiftUI layout within the window chrome. A run-loop observer
before those phases can consume an already pending boot tick. It cannot
preempt the first paint, so the one-period prediction is at risk.

The single candidate will consume pending ticks at that early boundary,
with a checked presenter borrow, until a complete frame is displayed.
Afterward the observer retires and ordinary winit delivery and idle stopping
remain. D7's off-main acquisition, D11's encode/submit division and
retirement, and D15's obligations remain. Diagnostic injection and sampling
are excluded from performance cohorts. Raw diagnosis is retained under
`target/m10-c6/diagnosis/`; all failed attempts remain there. The requested
six broad checks are deferred to the Mac mini; affected suites and caps run
in this worktree. No push.

**Outcome.** Cold complete display is **200.874 ms**, below GPUI's
209.308 and egui's 222.489 ms. The cold prediction is met. The renderer
commits the complete warm frame at 170.573 ms, but it displays at
212.754 ms: the warm display prediction misses. Early delivery removes
winit's long queue delay; it cannot preempt the first paint or force the
window's first composite. The 100 ms application obligation remains unmet.

| Prediction | Result |
| --- | --- |
| First delivered tick within 8.33 ms of its callback, about 156 ms | **Miss:** warm delivery 163.213 ms; per-launch callback-to-delivery median 20.038 ms, range 16.572–28.119. None of the thirty candidate launches meets one period. |
| Final upload by about 170 ms | **Met:** issued at 162.223 ms warm. |
| Complete-frame commit by about 175 ms | **Met:** 170.573 ms warm. |
| Warm complete display 190–200 ms | **Miss:** 212.754 ms. |
| Cold complete display 195–205 ms, below GPUI and egui | **Met:** 200.874 ms, against 209.308 and 222.489 ms; n=10 each. |
| Matching-backend pixels unchanged; CPU-first handoff and live Metal/wgpu pass | **Met:** zero changed full-window RGBA pixels on both backends; handoff, release wheel and development socket drives pass. |
| Fling, rest and bar unchanged | **Not established from n=1:** CPU is lower and idle remains stopped, but fling late intervals rise from 0.537 to 1.053/s. Qualified samples and clock counters are below. |

**Cause and change.** Winit 0.30.13 puts proxy events in a channel,
signals a run-loop source and wakes the loop. The source's callback does
no work. Winit drains the channel at its last-priority `BeforeWaiting`
observer, then handles redraws and `about_to_wait`. Signalling the source
does not invoke the carrier's tick.

In the two diagnostic launches, the first `BeforeWaiting` is 208.342 and
214.249 ms. No other run-loop mode is observed before tick delivery.
AppKit's first entry-observer interval is 14.161 and 19.067 ms; its later
`updateWindows` spans are 4.674 and 7.390 ms. The source can run before
winit delivers its queued tick: in the second launch it runs at 188.586 ms,
25.665 ms before delivery. This is not a source wake lost in another mode.
The sample names AppKit's UpdateCycle, transaction flush, native window
layout and SwiftUI chrome layout. It also contains earlier menu/Writing
Tools initialization and later probe saving; neither is charged to this
delivery interval. The sample does not provide an exclusive CPU breakdown
of every millisecond of the gap.

GPUI 0.2.2 coalesces its CVDisplayLink callbacks into a dispatch source on
the main queue and calls its frame callback there. It has no winit channel
to drain. Dispatch alone is not the earliest usable boundary for this
carrier: the diagnostic main-queue checkpoints arrive at 206.452 and
191.123 ms, after run-loop entry at 171.041 and 168.922 ms. The carrier can
paint at that entry without calling AppKit's `drawRect` or forcing a
transaction flush.

Source `54929835ce` installs a boot-only observer at priority −1, in common
modes, for entry, after-waiting, before-sources and before-waiting. It
consumes the pending tick on main. A checked mutable borrow defers a tick
when AppKit enters a nested loop inside a carrier callback. The existing
proxy still supplies the wake; its later queued event does nothing if the
tick was already consumed. A positive presented timestamp for a complete
frame retires the observer. Dropped frames and callbacks from an old layer
generation cannot retire it. A boot latch keeps later resize generations
on ordinary delivery. Frame readiness reaches the presentation callback
without requiring the probe. Drawable acquisition and final presentation
remain on the worker; CPU-first handoff and cuts 1–5 remain.

**Affected suites.** The release and development bundles build. With
`EXACT_INK_DEVICE=metal EXACT_REQUIRED=1 RUST_TEST_THREADS=2`,
`cargo test -p exact-linux -p exact-apple-painted -p exact-ink --lib --tests
--no-fail-fast` exits 0: six carrier tests, 46 Ink tests, 1,697 Linux
library tests and every Linux integration binary pass. The presentation
regression checks dropped complete frames, stale generations, a displayed
complete CPU frame and invalidation. Full output is retained in
`target/m10-c6/checks/affected.log`; `suite-results.txt` retains its status
lines, including the nested Linux processes. The named filters print:

```text
paint: test result: ok. 56 passed; 0 failed; 1 ignored; 0 measured; 48 filtered out; finished in 5.06s
borders: test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 99 filtered out; finished in 0.98s
tint: test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 103 filtered out; finished in 0.75s
backdrop: test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out; finished in 1.23s
gradients: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 0.75s
visual: test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 97 filtered out; finished in 2.39s
materials: test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 104 filtered out; finished in 1.61s
image: test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 130 filtered out; finished in 2.08s
```

Changed Rust files pass formatting. Staged caps passes all ten categories.
The six broad checks are deferred to the Mac mini as requested.

**Cold scoreboard.** The retained `measure.m` driver runs cold-only,
hook, trace and power off, ten original launches per arm before and after
in this session. Before is cut 5's `94638ea2d8` carrier; after is
`54929835ce`. Controls and the linked probe match cut 5's hashes. Both
schedules qualify. Before egui round 5 fails startup readiness; all forty
after originals succeed. No failure is replaced.

| Arm | Before, ms (n) | After, ms (n) |
| --- | ---: | ---: |
| Native | 255.063 (10) | 257.581 (10) |
| GPUI | 208.224 (10) | 209.308 (10) |
| egui | 219.400 (9) | 222.489 (10) |
| Ink | 221.986 (10) | 200.874 (10) |

The Ink row improves by 21.112 ms. Cold means a fresh process without
file-cache eviction, as in G4 and cut 5; it does not mean an empty Metal
system cache. The root scoreboard's other workloads retain their existing
cohorts.

**Five-run confirmation.** Every one of the thirty originals succeeds.

| Arm | Cold, ms (range; n) | Warm, ms (range; n) |
| --- | --- | --- |
| ink-after | 214.282 (205.017–226.387; n=5) | 213.078 (195.573–219.112; n=5) |
| gpui | 208.944 (208.529–216.683; n=5) | 210.508 (202.630–213.741; n=5) |
| egui | 227.444 (221.157–240.162; n=5) | 220.717 (213.622–227.863; n=5) |

**Ten-pair comparison.** All 120 original attempts remain. Three egui-after
attempts time out before the first complete frame: cold rounds 5 and 7, and
warm round 6. The successful sample counts and matched-pair counts are below.

| Arm/cohort | Before, ms (range; n) | After, ms (range; n) | Paired after − before, ms (range; n) |
| --- | --- | --- | --- |
| ink cold | 230.340 (216.102–246.372; n=10) | 217.607 (194.755–226.479; n=10) | -14.447 (-26.912–1.120; n=10) |
| ink warm | 228.469 (216.711–241.373; n=10) | 212.754 (196.368–225.433; n=10) | -18.499 (-33.161–0.839; n=10) |
| gpui cold | 209.653 (201.735–217.496; n=10) | 208.293 (195.271–224.344; n=10) | -6.894 (-10.865–22.609; n=10) |
| gpui warm | 213.216 (205.207–228.227; n=10) | 210.138 (199.144–222.274; n=10) | -3.924 (-21.865–8.120; n=10) |
| egui cold | 222.250 (208.823–247.199; n=10) | 224.601 (206.372–237.280; n=8) | 0.097 (-21.006–14.319; n=8) |
| egui warm | 224.523 (210.435–238.911; n=10) | 217.930 (205.569–266.593; n=9) | -10.190 (-33.342–53.843; n=9) |

**Warm phase trace.** Ten successful launches per Ink arm. Each column
contains medians of its own observations; differences of column medians are
not phase-duration medians.

| Event, process-relative ms unless labelled | Cut 5 | Cut 6 |
| --- | --- | --- |
| Window show returns | 99.201 | 102.890 |
| First layout completes | 128.262 | 130.261 |
| First paint starts | 132.534 | 132.963 |
| First display-link callback | 143.717 | 142.843 |
| Last picture decode ends | 160.929 | 160.422 |
| First tick delivered | 205.700 | 163.213 |
| Final picture upload is issued | 205.844 | 162.223 |
| Complete frame commits | 206.386 | 170.573 |
| Complete frame displays | 228.469 | 212.754 |
| Last decode to last upload end, per-launch interval | 43.913 | 1.890 |

The callback-to-delivery interval falls from 61.495 to 20.038 ms (per-launch
medians). Every candidate launch still exceeds one 8.33 ms period. Final
upload follows the first delivered tick in all ten cold launches and seven
warm launches; the immediate successor already uploads the last picture in the
other three warm launches. The warm median last-decode-to-upload interval
falls from 43.913 to 1.890 ms.

The warm per-launch complete-commit-to-display interval grows from 23.389 to
45.991 ms. Earlier commit does not force AppKit's first display cycle to
composite sooner. The first displayed frame is complete in all thirty
candidate launches, and every observer retires once, after that display, with
no later boot tick. Warm zero callbacks before positive display rise from a
median one to two. The paired Ink median improves by 14.447 ms cold and 18.499
ms warm, but both ranges cross zero; neither is an all-launch win.

**Qualification.** All 230 startup attempts are retained: 226 successes
and four egui failures. The prelaunch gate is unchanged. During-launch
load is reported, not used to discard a launch. All four schedules pass
their ten-second prelaunch gates and observe no power sampler. Successful
samples meet the focus, geometry, thermal and power-mode checks. Accepted traced
samples lose no trace events.

| Cohort | Minimum quiet seconds | Maximum non-benchmark cores during launch | Maximum user-process cores during launch | Maximum whole-machine cores during launch |
| --- | ---: | ---: | ---: | ---: |
| confirm | 10.111 | 1.356 | 0.285 | 6.750 |
| paired | 10.106 | 1.529 | 0.671 | 8.518 |
| scoreboard-before | 10.104 | 2.029 | 0.666 | 8.914 |
| scoreboard-after | 10.101 | 1.401 | 0.260 | 7.029 |

**Functional drives and pixels.** On an empty application archive, the
complete CPU frame displays at 195.595 ms and the complete Ink frame at
337.262 ms. Retained Metal displays at 182.238 ms and wgpu at 230.484 ms.
All three checks pass with zero main-thread drawable acquisition. These
are single functional observations, not qualified startup medians.

The release wheel drive advances displayed frames from 14 to 116 on Metal
and 13 to 114 on wgpu. Both observers retire once. Their subsequent idle
intervals, 1.132 and 1.142 seconds, contain zero display callbacks, tick
deliveries, paints, present commits or first-frame repeats. The development
socket drives advance carrier frames from 4 to 41 on each backend, keep
the Ink painter and exit 0. All four validation records report no failures.

Matching-backend first screens have **zero changed RGBA pixels**, both
for the 1600×1536 content and the full 1600×1600 windows. Every after capture
records that the complete screen was its first display. Both after first
screens and both live release captures were inspected. The predictions
for pixels, handoff and live drawing pass.

**Fling, rest and bar.** One original fling/rest and one bar sample per
arm; hook, trace and power off, CPU clock counters retained. All eight
samples pass the prelaunch gate, every five-second quiet observation,
focus, geometry, thermal and power-mode checks. Each phase lasts ten
seconds. There are no replacements. Native's display-link counts are its
existing cadence proxy; native rest has no comparable presented-frame
count.

| Arm | Phase | Qualified | CPU ms/s | Main ms/s | Mcycles/s | Minstructions/s | GHz | IPC | Frames/s |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| native | fling | True | 169.395 | 116.670 | 476.660 | 1300.196 | 2.814 | 2.728 | 118.252 |
| native | rest | True | 3.233 | 1.862 | 4.715 | 3.601 | 1.458 | 0.764 | — |
| gpui | fling | True | 163.403 | 84.367 | 321.988 | 500.704 | 1.971 | 1.555 | 88.955 |
| gpui | rest | True | 5.149 | 3.418 | 7.293 | 9.157 | 1.416 | 1.256 | 0.000 |
| ink | fling | True | 139.451 | 69.338 | 265.256 | 527.308 | 1.902 | 1.988 | 33.287 |
| ink | rest | True | 3.493 | 3.340 | 5.221 | 5.013 | 1.494 | 0.960 | 0.000 |
| egui | fling | True | 123.658 | 60.666 | 245.925 | 369.970 | 1.989 | 1.504 | 41.679 |
| egui | rest | True | 1.304 | 1.163 | 1.826 | 2.160 | 1.401 | 1.183 | 0.000 |
| native | bar | True | 12.110 | 10.851 | 17.089 | 23.087 | 1.411 | 1.351 | 119.953 |
| gpui | bar | True | 180.632 | 126.720 | 310.281 | 473.059 | 1.718 | 1.525 | 119.495 |
| ink | bar | True | 189.846 | 96.948 | 295.141 | 410.774 | 1.555 | 1.392 | 119.553 |
| egui | bar | True | 203.871 | 110.044 | 350.268 | 331.316 | 1.718 | 0.946 | 119.705 |

| Arm | Fling p95, ms | Late intervals/s | Fling peak, MiB | End of rest, MiB | Bar peak, MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| native | 8.333 | 1.699 | 231.611 | 216.923 | 131.407 |
| gpui | 8.333 | 1.877 | 421.970 | 215.673 | 301.892 |
| ink | 8.333 | 1.053 | 599.876 | 241.892 | 353.548 |
| egui | 8.333 | 0.268 | 515.767 | 181.954 | 439.157 |

Compared with cut 5's retained single Ink samples, fling/rest/bar CPU is
146.319/3.552/206.719 → 139.451/3.493/189.846 ms/s. Fling instructions are
528.330 → 527.308 million/s and bar instructions 414.774 → 410.774
million/s. Effective clocks and IPC also change; these points do not
establish a new CPU saving. Fling p95 stays 8.333 ms, but late intervals
rise from 0.537 to 1.053/s. Rest presents zero frames in both sessions,
and bar cadence is 119.524 → 119.553/s. The unchanged-cost prediction is
not established by these one-sample checks. The ordinary idle stop and
post-boot drawing behavior pass their separate drives.

**Reproduction and retained files.** Work stays in
`/Users/ccheever/projects/exact2-wt-1149-c1`, branch
`lane/1149-001-m10-c1`. The lane rebased onto `ab50634c87` before the fix;
`rebase.txt` and `rebase-code.diff` retain that boundary. Prediction commit
`50d2444b49` precedes the candidate run; implementation `54929835ce` is the
only measured renderer configuration. No push is made.

Local `target/m10-c6/` retains the release/development bundles, build
context, logs, tests and reducers. The silver's
`~/projects/exact2-measure/target/g0/m10-c6/` retains all five measurement
bundles, the development bundle, original drivers and every startup,
cost and functional run. Its raw records are mirrored locally under
`target/m10-c6/silver/`. `provenance.json`, `instrument-identity.json` and
`probe-hashes.json` identify the exact before/after executables and verify
that the controls and probe match cut 5. `collection-final.json` records
every collector command, PID and exit. The silver is dedicated; no go
file is required.

The diagnostic injection, binaries, traces, sample, run records and source
excerpts are retained under `target/m10-c6/diagnosis/` and the silver's
`target/g0/m10-c6-diagnosis/`. Two passive diagnostic Ink launches have
large startup perturbations; their sample is stack evidence only. An
early-initialization diagnostic GPUI launch aborts before its custom
application class is ready. All remain excluded. The two ordinary Ink
traces and ordinary GPUI trace supply the run-loop observations. Diagnostic
injection and sampling are absent from every performance cohort.

Build from the worktree's own target directory:

```sh
cargo metadata --offline --format-version 1 \
  --manifest-path bench/heavy-list/exact-heavylist/Cargo.toml > /dev/null
EXACT_APP_DIR="$PWD/bench/heavy-list/exact-heavylist" \
EXACT_PAINTED_PROBE="$PWD/bench/heavy-list/gpui/probe.m" \
CARGO_TARGET_DIR="$PWD/target" \
bun host/apple/build.mjs exact-heavylist-apple --painted-benchmark --release
```

The build script passes `--target-dir` to this worktree's `target/`.
Omitting `--release` builds the development socket fixture. Prepare a
fresh artifact directory with the retained `prepare.py`; run
`run-final.py` under `caffeinate` in the silver's GUI session. Its cost
collector includes cut 5's corrected initial CPU observation interval.
Use a new directory rather than overwriting any retained original.

Reproduce the reductions without opening a window:

```sh
python3 -B target/m10-c6/report.py
python3 -B target/m10-c6/picture-spans.py
python3 -B target/m10-c6/wake-spans.py
python3 -B target/m10-c6/boot-observer.py
python3 -B target/m10-c6/paired-intervals.py
python3 -B target/m10-c6/launch-rows.py
python3 -B target/m10-c6/cost-report.py
python3 -B target/m10-c6/record-tables.py
python3 -B target/m10-c6/diagnosis/timing.py
bun target/m10-c6/first-screen-compare.mjs
```

The JSON and table files retain every reduced value and its original
filename. `raw-sha256.json` hashes the retained records and
`analysis-sha256.json` hashes the reducers. `checks/verbatim-output.txt`
retains the full affected and named-suite stdout/stderr. All requested
collection is complete. The six broad checks remain deferred to the Mac
mini. One implementation and measurement round; no second renderer change
is made to pursue the missed warm targets. D7, D11 and D15, including the
dated device and 100 ms application obligations, remain unchanged.

## M10, confirmed

**Predictions, before measurement (2026-10-31).** This records-only
confirmation compares main's carrier at `bc9da74a4b` with cut 6 rebased at
`c7a9596585`. Both are release builds from this worktree. The native, GPUI
and egui controls are the retained cut 6 binaries. All run on the dedicated
silver Mac with the GPU queue hook, tracing and power sampling off.

| Prediction | Instrument and cohort | Result |
| --- | --- | --- |
| Ink after is 5–12 ms below GPUI and 15–25 ms below egui; Ink before is 8–15 ms above GPUI | `measure.m`, cold-only; ten launches of each control under each before/after label, ten of each Ink carrier, interleaved in one session | **Miss:** Ink after is 3.847 ms above GPUI and only 1.843 ms below egui; Ink before is 17.131 ms above GPUI. |
| Ink after and before have a paired range crossing zero for process/main CPU ms/s, cycles/s, frames/s, late intervals/s and peak footprint in fling, rest, bar and caret | G4's posted fling and rest; G5's bar and caret; five interleaved rounds of native, GPUI, egui and adjacent Ink before/after | **Partly met:** fling/rest/caret meet all named rows. Bar misses frame rate (+0.194/s), late rate (−0.200/s) and peak footprint (−3.188 MiB), with ranges excluding zero; CPU/main/cycles meet. |
| Ink rest submits zero frames | Presented timestamps in the same five fling/rest samples | **Met:** all ten before/after Ink rest originals have zero frames. |

Every freshly signed bundle gets a throwaway launch. Cold launches require
ten quiet seconds each; their during-launch load maxima are report-only.
Fling and static samples keep the amended five-second quiet rule and clock
columns. Failed originals remain, without replacement. Raw files, signed
bundles, commands and reductions remain in `target/m10-confirmed/`
and the silver's `target/g0/m10-confirmed/`. No renderer code changes.

**Cold confirmation: the lead does not repeat.** All 80 original slots
were attempted. Native, GPUI and both Ink carriers complete all ten launches
per label; egui completes 9/10 before and 7/10 after. Its four readiness
timeouts remain without replacement. Every launch passes the per-launch
quiet gate. During-launch maxima are report-only: 1.855 non-benchmark
cores, 0.521 for one user process and 8.375 whole-machine busy cores.

Medians in milliseconds (successful n). Session 1 is cut 6's retained
series; its before carrier is cut 5. Session 2 uses main `bc9da74a4b`
before and `c7a9596585` after. The controls are unchanged binaries.

| Arm | Session 1 before | Session 1 after | Session 2 before | Session 2 after |
| --- | ---: | ---: | ---: | ---: |
| Native | 255.063 (10) | 257.581 (10) | 250.780 (10) | 242.379 (10) |
| GPUI | 208.224 (10) | 209.308 (10) | 201.212 (10) | 197.920 (10) |
| egui | 219.400 (9) | 222.489 (10) | 207.824 (9) | 203.609 (7) |
| Ink | 221.986 (10) | 200.874 (10) | 218.343 (10) | 201.766 (10) |

The second session’s paired Ink change is **−19.836 ms [−44.145,
+16.012]**, n=10. Its range crosses zero. Ink’s after median is 0.892 ms
above session 1, while the controls’ after medians fall by 15.202 ms
(native), 11.388 ms (GPUI) and 18.880 ms (egui). The repeated ordering
required to promote the cold row is absent; the 100 ms obligation remains.

**After boot: paired results.** Values are medians [minimum, maximum].
Pairs are after minus before within the same round; these are observed
ranges, not confidence intervals. The prediction is that zero lies in the
range for CPU, main-thread CPU, cycles, frame rate, intervals over 12.5 ms
per second and peak footprint. Instructions and end footprint are reported
alongside them; no unchanged prediction was made for those two rows.

| Phase | Native n | GPUI n | egui n | Ink before n | Ink after n | Ink pairs |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| fling | 5 | 4 | 2 | 5 | 5 | 5 |
| rest | 5 | 4 | 2 | 5 | 5 | 5 |
| bar | 4 | 5 | 4 | 5 | 5 | 5 |
| caret | 5 | 4 | 5 | 5 | 5 | 5 |

Process work and clocks. Cycles and instructions are millions per second;
effective GHz is cycles divided by process CPU time, and IPC is instructions
per cycle. Each clock cell is reduced per original before taking its median.

| Phase | Arm | CPU ms/s | Main ms/s | Mcycles/s | Minstructions/s | GHz | IPC |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| fling | native | 166.250 [165.687, 169.659] | 114.407 [113.971, 116.606] | 482.978 [459.583, 496.954] | 1303.828 [1205.839, 1377.387] | 2.903 [2.764, 2.929] | 2.705 [2.624, 2.806] |
| fling | gpui | 157.858 [156.501, 160.251] | 82.086 [81.017, 82.710] | 300.296 [294.786, 307.260] | 499.504 [496.925, 500.515] | 1.897 [1.851, 1.963] | 1.661 [1.624, 1.696] |
| fling | egui | 120.209 [119.882, 120.535] | 59.148 [58.871, 59.426] | 243.700 [239.942, 247.458] | 369.788 [369.784, 369.791] | 2.027 [2.001, 2.053] | 1.518 [1.494, 1.541] |
| fling | ink-before | 136.928 [129.661, 141.043] | 67.525 [66.768, 70.036] | 257.058 [254.070, 262.443] | 499.183 [497.324, 510.067] | 1.882 [1.841, 1.959] | 1.944 [1.941, 1.973] |
| fling | ink-after | 138.472 [131.217, 140.331] | 67.283 [63.717, 67.333] | 257.661 [251.500, 262.854] | 524.651 [524.070, 527.836] | 1.874 [1.831, 1.917] | 2.036 [2.001, 2.084] |
| rest | native | 2.572 [2.561, 2.626] | 1.260 [1.213, 1.303] | 3.861 [3.697, 4.067] | 2.411 [2.300, 2.470] | 1.503 [1.408, 1.581] | 0.605 [0.593, 0.668] |
| rest | gpui | 4.622 [4.581, 4.719] | 2.851 [2.821, 2.905] | 6.550 [6.491, 6.645] | 7.997 [7.957, 8.001] | 1.413 [1.408, 1.424] | 1.221 [1.204, 1.226] |
| rest | egui | 0.731 [0.703, 0.759] | 0.611 [0.591, 0.631] | 1.058 [1.027, 1.090] | 0.986 [0.962, 1.010] | 1.449 [1.436, 1.462] | 0.932 [0.926, 0.937] |
| rest | ink-before | 3.042 [2.934, 3.313] | 2.932 [2.774, 3.137] | 4.459 [4.327, 4.840] | 4.004 [3.962, 4.183] | 1.466 [1.385, 1.485] | 0.889 [0.838, 0.967] |
| rest | ink-after | 3.017 [2.965, 3.146] | 2.885 [2.841, 2.991] | 4.503 [4.255, 4.620] | 3.897 [3.809, 4.089] | 1.469 [1.435, 1.494] | 0.877 [0.844, 0.908] |
| bar | native | 11.663 [11.182, 11.921] | 10.367 [9.923, 10.602] | 17.009 [16.260, 17.905] | 21.844 [21.314, 21.896] | 1.459 [1.452, 1.502] | 1.285 [1.222, 1.311] |
| bar | gpui | 181.949 [179.369, 185.312] | 127.142 [125.099, 130.063] | 298.325 [295.698, 308.996] | 471.327 [468.789, 474.573] | 1.651 [1.596, 1.698] | 1.580 [1.517, 1.604] |
| bar | egui | 203.704 [203.214, 206.208] | 109.516 [109.339, 111.316] | 352.632 [348.231, 362.585] | 329.532 [326.656, 332.622] | 1.733 [1.709, 1.758] | 0.931 [0.917, 0.944] |
| bar | ink-before | 192.552 [188.655, 205.877] | 97.611 [95.083, 111.985] | 299.105 [283.524, 318.657] | 411.363 [408.229, 486.670] | 1.548 [1.503, 1.630] | 1.418 [1.294, 1.527] |
| bar | ink-after | 192.494 [190.167, 201.455] | 96.611 [96.072, 107.389] | 294.855 [289.765, 311.329] | 408.737 [406.344, 471.898] | 1.524 [1.509, 1.626] | 1.402 [1.313, 1.537] |
| caret | native | 11.499 [10.532, 11.659] | 10.249 [9.256, 10.366] | 16.914 [15.587, 17.332] | 21.783 [20.082, 21.853] | 1.483 [1.466, 1.487] | 1.274 [1.236, 1.292] |
| caret | gpui | 18.455 [17.774, 18.889] | 14.865 [14.337, 15.180] | 26.657 [25.977, 27.689] | 36.046 [35.855, 36.168] | 1.453 [1.444, 1.466] | 1.354 [1.303, 1.380] |
| caret | egui | 16.317 [15.879, 16.586] | 13.426 [13.129, 13.737] | 23.397 [22.988, 25.056] | 28.815 [28.754, 28.918] | 1.458 [1.434, 1.511] | 1.232 [1.148, 1.257] |
| caret | ink-before | 26.708 [26.611, 27.821] | 23.647 [23.418, 24.670] | 38.951 [38.263, 41.028] | 64.901 [64.375, 65.085] | 1.458 [1.437, 1.475] | 1.668 [1.569, 1.696] |
| caret | ink-after | 26.278 [26.143, 26.821] | 23.183 [23.060, 23.638] | 38.569 [37.791, 39.426] | 64.362 [63.547, 65.764] | 1.455 [1.446, 1.496] | 1.668 [1.660, 1.687] |

Presentation and footprint. Native reports display-link callbacks, painted
arms report presented drawables. Fling late intervals and p95 use the four
gesture windows for painted arms and the whole phase for native. Full-phase
and gesture frame rates are separate. Native rest cadence is unavailable.
Static intervals over 12.5 ms include intended caret blink intervals; they
are not a missed-deadline count. Peak and end footprint are MiB.

| Phase | Arm | Phase frames/s | Gesture frames/s | Intervals >12.5 ms/s | p95 ms | Peak MiB | End MiB |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| fling | native | 118.592 [118.350, 118.774] | — | 1.400 [1.100, 1.699] | 8.333 [8.333, 8.333] | 236.017 [231.095, 237.517] | 214.204 [211.298, 218.376] |
| fling | gpui | 88.938 [88.878, 89.086] | 105.868 [105.411, 106.053] | 1.876 [1.579, 2.146] | 8.333 [8.333, 8.333] | 416.306 [403.048, 422.048] | 394.150 [388.298, 399.048] |
| fling | egui | 41.739 [41.693, 41.785] | 104.711 [104.207, 105.216] | 0.000 [0.000, 0.000] | 8.333 [8.333, 8.333] | 513.993 [513.814, 514.173] | 187.470 [186.939, 188.001] |
| fling | ink-before | 33.197 [33.085, 33.494] | 89.082 [88.785, 89.996] | 1.075 [1.073, 1.607] | 8.333 [8.333, 8.333] | 598.032 [596.861, 601.095] | 248.267 [246.267, 249.314] |
| fling | ink-after | 33.290 [32.996, 33.592] | 89.449 [87.509, 90.340] | 1.075 [0.805, 1.608] | 8.333 [8.333, 8.333] | 598.782 [596.423, 607.173] | 249.907 [247.907, 254.689] |
| rest | native | — | — | — | — | 214.204 [211.298, 218.376] | 213.501 [210.626, 217.564] |
| rest | gpui | 0.000 [0.000, 0.000] | — | 0.000 [0.000, 0.000] | — | 394.150 [388.298, 399.048] | 210.134 [205.157, 215.064] |
| rest | egui | 0.000 [0.000, 0.000] | — | 0.000 [0.000, 0.000] | — | 187.470 [186.939, 188.001] | 179.345 [178.814, 179.876] |
| rest | ink-before | 0.000 [0.000, 0.000] | — | 0.000 [0.000, 0.000] | — | 248.267 [246.267, 249.314] | 240.923 [238.954, 242.032] |
| rest | ink-after | 0.000 [0.000, 0.000] | — | 0.000 [0.000, 0.000] | — | 249.907 [247.907, 254.689] | 242.626 [240.579, 245.845] |
| bar | native | 119.992 [119.934, 120.075] | — | 0.000 [0.000, 0.000] | 8.333 [8.333, 8.333] | 126.798 [126.610, 128.579] | 126.376 [126.251, 128.314] |
| bar | gpui | 119.636 [119.601, 119.683] | — | 0.400 [0.300, 0.400] | 8.333 [8.333, 8.333] | 303.501 [301.517, 305.939] | 303.079 [301.173, 305.501] |
| bar | egui | 119.699 [119.494, 119.701] | — | 0.300 [0.300, 0.500] | 8.333 [8.333, 8.333] | 439.314 [438.954, 442.767] | 438.915 [438.611, 442.376] |
| bar | ink-before | 119.555 [119.426, 119.580] | — | 0.400 [0.400, 0.600] | 8.333 [8.333, 8.333] | 356.251 [355.689, 358.251] | 351.220 [350.892, 353.079] |
| bar | ink-after | 119.650 [119.587, 119.884] | — | 0.400 [0.100, 0.400] | 8.333 [8.333, 8.333] | 353.064 [352.876, 353.360] | 348.064 [347.657, 350.314] |
| caret | native | 119.960 [119.882, 120.034] | — | 0.000 [0.000, 0.100] | 8.333 [8.333, 8.333] | 128.970 [125.595, 130.345] | 128.517 [125.064, 129.985] |
| caret | gpui | 2.000 [1.999, 2.098] | — | 1.900 [1.900, 1.998] | 529.167 [525.000, 533.334] | 292.985 [292.454, 301.392] | 292.525 [291.970, 300.814] |
| caret | egui | 2.000 [1.999, 2.000] | — | 1.900 [1.899, 1.900] | 500.000 [500.000, 508.333] | 440.079 [432.579, 443.657] | 439.735 [432.157, 443.189] |
| caret | ink-before | 1.999 [1.999, 2.000] | — | 1.899 [1.899, 1.900] | 508.334 [500.000, 508.334] | 354.064 [344.439, 356.610] | 350.532 [341.095, 350.798] |
| caret | ink-after | 1.999 [1.999, 2.000] | — | 1.899 [1.899, 1.900] | 508.333 [500.000, 508.334] | 350.829 [341.439, 351.079] | 347.501 [338.110, 347.751] |

Paired changes. The “zero in range” column tests the prediction only for
the metrics named above. The bar late-rate range uses six decimals because
its upper endpoint is close to zero.

| Phase | Metric | Paired after − before [range] | Zero in range |
| --- | --- | ---: | --- |
| fling | cpu | 1.136 [-5.711, 8.430] | yes |
| fling | main | -1.281 [-3.050, 0.458] | yes |
| fling | cycles | -0.149 [-6.209, 6.672] | yes |
| fling | instructions | 24.934 [15.612, 28.720] | no |
| fling | fps | 0.105278 [-0.201066, 0.196451] | yes |
| fling | gestureFps | -0.411269 [-1.276423, 0.893876] | yes |
| fling | late | -0.000991 [-0.531341, 0.535067] | yes |
| fling | peak | 0.750 [-4.234, 10.313] | yes |
| fling | end | 1.141 [-1.000, 8.344] | yes |
| rest | cpu | -0.077 [-0.167, 0.147] | yes |
| rest | main | -0.092 [-0.146, 0.147] | yes |
| rest | cycles | -0.070 [-0.220, 0.176] | yes |
| rest | instructions | -0.153 [-0.278, 0.105] | yes |
| rest | fps | 0.000000 [0.000000, 0.000000] | yes |
| rest | gestureFps | — | unavailable |
| rest | late | 0.000000 [0.000000, 0.000000] | yes |
| rest | peak | 1.141 [-1.000, 8.344] | yes |
| rest | end | 1.125 [-1.000, 6.891] | yes |
| bar | cpu | 0.394 [-4.422, 1.513] | yes |
| bar | main | -0.012 [-4.595, 2.355] | yes |
| bar | cycles | 5.207 [-23.059, 12.224] | yes |
| bar | instructions | -3.153 [-14.772, 18.761] | yes |
| bar | fps | 0.194316 [0.095124, 0.306029] | no |
| bar | gestureFps | — | unavailable |
| bar | late | -0.199800 [-0.299939, -0.000016] | no |
| bar | peak | -3.188 [-5.062, -2.703] | no |
| bar | end | -2.828 [-3.563, -2.766] | no |
| caret | cpu | -0.565 [-1.471, 0.210] | yes |
| caret | main | -0.587 [-1.369, 0.060] | yes |
| caret | cycles | -1.160 [-1.824, 0.492] | yes |
| caret | instructions | -0.516 [-1.432, 1.390] | yes |
| caret | fps | -0.000281 [-0.001171, 0.000729] | yes |
| caret | gestureFps | — | unavailable |
| caret | late | -0.000267 [-0.001113, 0.000693] | yes |
| caret | peak | -3.047 [-6.094, 6.297] | yes |
| caret | end | -2.984 [-3.266, 6.391] | yes |

Fling, rest and caret meet every named unchanged prediction. Every Ink
rest original has zero presented frames, before and after. Fling
instructions rise by 24.934 million/s [15.612, 28.720]; that is a measured
work increase outside the predicted metrics. The fling late-rate range
crosses zero, so this result does not trigger the requested commit
attribution.

Bar meets the CPU, main-thread CPU and cycles predictions. Its frame-rate,
late-rate and peak-footprint predictions miss: frame rate rises, while the
late rate and peak fall. The late-rate upper endpoint is −0.000016/s;
round 2 has four long intervals on each carrier, and its rate difference
comes only from the phase durations. That endpoint is not evidence of fewer
long intervals in every pair. Peak falls by 3.188 MiB [2.703, 5.062].

All 155 original slots complete. Cold has four egui readiness failures.
Fling excludes egui rounds 1 and 4 for readiness timeouts, egui round 3
for 1.326 non-benchmark cores, and GPUI round 5 for a user process at
0.280 core. Bar excludes native round 1 and egui round 4 for background
load. Caret excludes GPUI round 5 for a user process at 0.297 core.
All originals remain without replacement. Every Ink cost original qualifies,
so each phase has five complete pairs. The incomplete control counts prevent
claiming five qualified samples for every arm.

**Builds, originals and reproduction.** Prediction commit `b6ada1ba71`
(08:39 PDT) precedes the collector launch (08:49 PDT). The branch rebases onto
`origin/main` at `15c20224e2`; the measured sources remain `bc9da74a4b`
and `c7a9596585`. The renderer and benchmark source diff is empty;
`rebase-final-context.json` lists the checked paths and other upstream changes. `target/m10-confirmed/measured-source.bundle`,
`measured-source.patch` and the rebase records preserve those boundaries.

The after carrier builds with the release painted-benchmark wrapper.
Main’s wrapper refuses its benchmark manifest’s missing `accesskit_windows`
patch before compiling. No source is changed to bypass that refusal:
`build-before.py` runs the equivalent `cargo rustc` release/link command,
with `--target-dir target`, at the fixed before revision, then restores this
branch. Both builds succeed. `checks/build-before.log` retains the refusal;
`checks/build-before-direct.log`, `checks/build-after.log`,
`before-build-command.json` and the two Cargo lock snapshots retain the
builds. The bundles use the same carrier assets and retained probe, then
are signed and receive a throwaway launch on the silver.

The measured signed bundles and originals remain on the silver under
`~/projects/exact2-measure/target/g0/m10-confirmed/`, mirrored in
`target/m10-confirmed/silver/`. `provenance.json` identifies both sources,
executables, libraries and hashes. `instrument-identity.json` verifies the
three control executables are unchanged from cut 6 and all five loaded
probes have identical bytes. `launch-context.json` records the GUI-session
command and PID; `run-confirmation.py` uses the retained `measure`,
`exact-wheel`, `startup.py` and quiet monitor without changing them.

Each workload’s `schedule.json` retains every original, command, exit,
gate and qualification. The cold schedule has 80 slots; `cold-launches.md`
shows each launch’s gate and report-only maxima. The cost schedules rotate
five distinct arms, with the two Ink carriers adjacent and their order
reversed on even rounds. There are 25 slots per workload. Quiet failures
exclude the original, including its rest phase; no replacement enters a
row. Each cold before/after label has its own control launches; the cost
series shares one control launch per round between the two Ink carriers.

Reproduce the tables without opening a window:

```sh
python3 -B target/m10-confirmed/session1.py
python3 -B target/m10-confirmed/launch-table.py
python3 -B target/m10-confirmed/reduce.py
python3 -B target/m10-confirmed/aggregate-check.py
python3 -B target/m10-confirmed/audit.py
```

`summary.json` retains every value and its original filename. The
`aggregate/` projections feed unchanged qualified originals to the
committed `aggregate.mjs`; `selection-exclusions.json` records which cost
originals the quiet rule excludes. The originals themselves remain in
`silver/`, failures included. `raw-sha256.json` and `analysis-sha256.json`
hash the records and reducers. Recollection needs a fresh artifact
directory and its own provenance paths; do not overwrite these originals.

**Verification.** Both release builds succeed, as recorded above. The
committed reducer reproduces the cold, fling/rest, bar and caret values and
sample counts; `aggregate-check.txt` retains its exact comparison output.
`audit.txt` confirms all 155 slots, quiet gates and adjacent Ink pairs,
all 1,759 power observations, the common probe bytes, all five executable
hashes and all five bundle signatures. The final caps output is retained in
`checks/caps-final.log`; `git diff --check` is clean. No renderer code or
test changes in this chunk. The six broad suites and blind code reviews
remain the separate runs assigned in the brief; this records-only chunk
does not claim their results. No push.

## M10, the cuts' reviews folded

**Predictions before the fixes and collection (2026-10-31).** The
retained confirmation carriers supply one traced fling each, hook and
power off, with phase counters, to test whether the picture-arrival tick
adds service work after boot. The proposed restriction to boot predicts
no such direct draws after retirement and an after-minus-main fling
instruction change within ±10 million/s in five qualified pairs, against
the confirmation's +24.934 [15.612, 28.720] million/s. CPU, cycles and
cadence are reported beside instructions; they are not proxies for work.

The fixed carrier predicts complete cold p50 in 195–215 ms and a paired
change of −15 to −25 ms against origin/main. Instrument: unchanged
`measure.m` cold-only and drawable probe, ten original launches per arm
and label, one silver session, hook/trace/power off. Five additional
traced launches per arm and label predict trace overhead below 10 ms
per arm; ordering against GPUI is an open measurement, not a promised
lead. The five traced slots are adjacent to the same arm and label's
untraced slots, alternating which runs first, before taking the remaining
five untraced slots. Each freshly signed arm gets a throwaway launch, each original
passes the ten-second prelaunch gate, launch maxima are report-only, and
failures remain without substitution.

Functional predictions: acquisition or CPU-blit refusal requests a fresh
paint despite a retained frame; an incomplete positive then a settled
zero callback retains the newer frame; stale callbacks cannot retire it;
a refused visible picture permits retirement. At most one retry is in
flight, retries stop within 120 delivered visible ticks, nested run-loop
entries consume no carrier ticks, and off-screen images have zero source
or decode requests before first paint. The affected Rust suites test
these conditions. Literal, light/dark and role backgrounds match the
resolved first screen; unresolved bindings remain hidden until drawable acquisition.

Raw files and commands are retained under `target/m10-review-fix/`. The
six repository checks are assigned to the mini after this chunk and are
not run here; staged caps is run as requested.

**Review fixes.** Frame identity now governs the retained source, dropped
callbacks and boot retirement. Only the current frame can change retry
state; a positive callback with no pending visible picture retires boot,
including a terminal image refusal; symbols have no picture request to
wait for. One repeat may await acquisition or
display. After 120 visible ticks without completion, retention and the
early observer stop; exposure, resize and input rearm ordinary recovery.
Acquisition and CPU-blit failures clear retention and request fresh paint.
The unavailable-Metal stub has the same CPU submission signature.

The observer counts run-loop entries and exits and consumes ticks only
at the outer boundary. Its owner retains the block through invalidation.
The early image path belongs only to the painted Mac carrier: before
layout it binds sources, and both post-layout syncs use the viewport
until paint supplies boxes. Ordinary hosts keep their mounted-image
policy. The root background is read from the presenter's decoded plan;
literals, roles and light/dark pairs resolve against the window's
appearance, while an unresolved authored value delays showing the window
until drawable acquisition. The standalone first-present window fixture
is now `bench/heavy-list/gpui/first-present-window.m`. The LLP records the
boot observer's input-ordering limit, and the queue's stray blank is gone.

**Local verification.** The eight named suites pass; their verbatim result
lines and commands are retained under `target/m10-review-fix/checks/`.
The current parity test is
`the_first_frame_after_layout_matches_the_display_tick_frame`. Ink's
51 tests pass (one ignored), the carrier's eight pass, and the nine
focused Apple lifecycle tests pass, including an actual nested Core
Foundation run loop and compilation of the Metal-unavailable stub. The
60-image offscreen test passes with every image below the viewport and
no source or decode request before paint. The plain-color fallback and
the two pre-paint image syncs are covered by the corrected tests.

The combined affected run is **not green**: the existing
`agent::tests::exit::an_orderly_exit_waits_for_a_request_with_effects`
fails in both full runs and passes alone. The earlier offscreen and
plain-color failures remain in those logs; the corrected tests pass in
the focused runs. No change to the exit test or its implementation is
included. The final Linux library run, excluding only that known exit
failure, reports 1,699 passed, one failed and 22 ignored: its asynchronous
core subprocess hits `a_quit_ends_every_group`'s grace-time bound. The
direct test in the same run passes. Both timing failures are already in
`QUEUE.md`'s “Wall-clock test bounds flake under load” item; neither is
silently excluded from a green claim. `caps.log` and
`caps-source-final.log` pass all ten categories; the six broad repository
checks remain assigned to the mini.

**Review finding dispositions.** Commit `4a0166e254`
fixes Sol 1–5 and Claude 1–3, 5–7, 9–12. It records Claude 8's boot input
ordering as a known limit in LLP 1149.001. Commit `07ea81f30c` releases
the one-repeat reservation when wgpu coalesces or refuses a submission,
completing the retry fix. Sol 6 and Claude 4 are evidence findings: the
scoreboard carries both earlier untraced sessions and explicitly records
that the second puts Ink above GPUI. The final fixed-tip rows and the
paired trace comparison below decide the new session's ordering; none
can erase that disagreement.

**Fling diagnosis before the fix.** The retained confirmation binaries
reproduce the increase without tracing: 526.571 million instructions/s
before and 551.490 after, a +24.919 million/s pair. The traced main
original qualifies; the traced cuts original exceeds the quiet limit
(non-benchmark load reaches 1.305 cores), so it is diagnostic evidence,
not a cost row. Its observer retires at about 204 ms, before the fling.
The trace contains 12 picture-arrival direct draws after retirement;
main routes these through redraw demand. The fixed carrier limits that
direct path to unshown boot. These traces rule out a persistent observer
for this workload; they do not establish how much of the instruction
increase the direct path explains. The first five-pair attempt tests that claim.
The second plan decode and earlier link start are boot work, not fling
work. `diagnose.py` reproduces the phase counters and trace observations
from `silver/diagnosis/` and `silver/diagnosis-traced/`.

**Initial review-build identity.** The baseline is `origin/main` as fetched before work,
`15c20224e2a16165bad87552a036d90d16fd4d91`; the first review candidate is
`07ea81f30c0f5d69775bd96891dafc8a6eea93a5`. Both release builds use
`MACOSX_DEPLOYMENT_TARGET=14.0`, the worktree's own `--target-dir target`,
and the same explicit probe link command. `before-build.json`,
`after-build.json`, their command files and build logs retain the results.
The provenance check rejected a freshly compiled probe before collection;
`probe-alignment.json` records replacement with the exact signed probe
already shared by all three controls. The initially built probes are
retained separately. All five arms load identical probe bytes.
The silver's A7 raster collector delayed this lane; the first waiting
functional coordinator was stopped before launching an app to avoid
mutual ownership waits. Its ownership log is retained.

**Bound-background follow-up.** The six light-appearance functional
launches all retire boot, and the bound window is shown after drawable
acquisition. The diagnostic nevertheless finds its window colour still
white and its layer colour unset at reveal, before a positive display
callback. The late reveal now copies the presenter's already resolved
canvas colour before ordering front. The pre-run prediction in
`bound-reveal-prediction.json` is that the first visible window and layer
samples both carry `[167, 43, 84, 255]`, with reveal still after acquisition.
This branch does not run for heavy-list's literal root. The eight carrier
tests pass after the three-line change (`bound-reveal-carrier.log`).

**First instruction fix: miss.** The five qualified pairs at
`07ea81f30c` give +24.755 [11.743, 28.869] million instructions/s, outside
the predicted ±10. CPU is +1.596 [−1.935, +6.437] ms/s and cycles are
+9.738 [−4.818, +32.591] million/s. Limiting direct picture draws to boot
did not remove the extra fling work. These originals remain a separate
diagnostic cohort, not replacements for any final original.

The same pairs' cumulative process counters locate the shift: before
fling, after minus main is −268.892 [−275.055, −162.438] million
instructions; during fling it is +248.877 [117.282, 289.993] million;
through fling's end it is −15.524 [−45.156, +21.102] million. The earlier
traced carriers each decode 40 pictures in total: main decodes 7 before
fling and 33 during it; the cuts decode 5 then 35. Two decodes moved into
fling. `phase-work.py` reproduces the cumulative counter comparison.

The next prediction, written in `image-handoff-prediction.json` before
changing the handoff, is 7 pre-fling and 33 fling decodes, and an
instruction delta within ±10 million/s in five qualified untraced pairs.
Boot retirement now resynchronizes the ordinary image policy after
restoring it, so mounted first loads need no input to resume. The
60-image test still makes zero off-screen requests before first paint;
after painting and retirement it now checks that ordinary first loads
resume. That extended test passes (`boot-handoff-round1.log`).

**Second-source functional checks, `e8202d4dc8`.** The archive-miss launch hands
off from a displayed CPU frame to Ink; the retained Metal and wgpu
launches display Ink. All three acquire zero drawables on main. Live
Metal and wgpu drives continue painting after boot, with displayed-frame
counters 12→112 and 10→110, respectively, no focus losses and no trace
drops. Their screenshots show the scrolled list. The six background
cases each retire the boot observer exactly once.

The silver's effective appearance is Aqua. The first visible diagnostic
sample records matching window and layer colours, and the final
screenshots show these cases:

| Root | What appears |
| --- | --- |
| Literal `#23394f` | The matching dark-blue surface, then its label. |
| `light-dark(#ffffff, #111111)` | White in Aqua, then its label. The dark selection is unit-tested. |
| `-exact-background` | The platform's white window surface, then its label. |
| Bound `#a72b54` | Hidden through initial acquisition; the first visible window and layer are magenta, then the label appears on that colour. |
| No background | The platform's white window surface. |
| Refused picture | The dark-blue surface and label with the picture absent; the refusal does not keep boot alive. |

The bound-window prediction is met: both first-visible RGBA values are
`[167, 43, 84, 255] / 255`, and the show marker follows acquisition by
23.831 ms. This diagnostic timing is not a cold scoreboard measurement.
`final/background-summary.py` checks the colours, reveal order and
retirement counts against the retained first-present facts and traces;
`final/silver/background/` retains the plans, screenshots and originals.

All eight named suites pass again at this source; commands and verbatim
outputs are in `checks/final/`. The 20 image-control tests pass, including
the post-paint handoff assertion, and all eight carrier tests pass
(`image-handoff-controls.log`, `carrier-handoff-final.log`). The earlier
full-suite timing failures and the deferred six checks remain as stated
above. The second attempt uses a fresh `final/` artifact directory; the
first five-pair miss and every earlier original remain in place.

**Second handoff prediction: decode split misses.** Both traced originals
at `e8202d4dc8` qualify, but main still decodes 7 pictures before fling and
33 during it, while Ink after decodes 5 then 35. The ordinary resync uses
painted visibility; it does not repeat the mounted first-load pass that
main performed before boxes existed. Both traced carriers route each
fling draw through redraw demand (349/349 on main, 347/347 after), so the
direct-draw prediction is met and does not explain the work difference.
`final/trace-summary.py` reproduces the phase counts and counters.

The third and last instruction-fix attempt defers that ordinary mounted
first-load pass until the first visible frame has displayed. It retains
real on-screen priority and returns to ordinary visibility on subsequent
syncs. The pre-run prediction is again 7 pre-fling / 33 fling decodes and
an instruction change within ±10 million/s in five qualified pairs, with
zero off-screen requests before first paint. The number, instrument and
round are retained in `mounted-handoff-prediction.json`.

The third attempt's 20 image-control tests pass
(`mounted-handoff-controls.log`). The cold trace comparison uses the
retained driver's executable-wrapper path because the driver removes
`G1_TRACE` from its child environment. The raw traced-minus-untraced
launch difference therefore includes that wrapper. In addition to that
full launch difference, the reducer reports the difference after the
probe constructor (`coldMs − preMainMs`), separating the launcher's
pre-constructor interval. Both use the same five adjacent pairs; neither
changes a raw timestamp or the headline cold rows.

The second attempt's five untraced pairs all qualify and also miss:
+23.616 [23.183, 25.113] million instructions/s. CPU is +0.918
[−1.841, +2.438] ms/s and cycles are +1.973 [−2.576, +6.913] million/s.
`final/summary.json` and `final/reduce.py` retain that comparison. The
third source is `c2bbac1010`; its eight named suites, 20 image-control
tests and eight carrier tests pass. Their latest outputs are in
`checks/landing/`, `mounted-handoff-controls.log` and
`carrier-landing.log`. Its measurements use `landing/`, preserving the
two prior attempts' originals and predictions.

**Third decode prediction: miss; source fixes stop.** Both traced
originals at `c2bbac1010` qualify. Main again decodes 7 pictures before
fling and 33 during it; after decodes 5 then 35. Its 348 fling draws all
follow redraw requests (main: 347/347). The mounted sync is only one
pass: later ordinary syncs restore visibility and can cancel offscreen
requests before decoding begins. The trace establishes that the pass
does not move those two decodes earlier; it does not identify individual
source cancellations. This is the third attempt, so no further source
fix is made in this chunk. The five untraced pairs below measure the
remaining work difference.

The final source repeats the archive-miss handoff, retained Metal, wgpu,
live updates and all six background cases successfully. The bound
window's first visible window and layer colours match; reveal follows
acquisition by 24.001 ms. Each background case retires its observer
once. `landing/{trace-summary,background-summary}.py` reproduces these
checks from `landing/silver/`; the live Metal and wgpu screenshots show
the scrolled list. The moved `first-present-window.m` fixture compiles
without diagnostics (`checks/first-present-window-build.{json,log}`);
that compile opens no window. Dark appearance is covered by unit tests, not a live
silver appearance switch; nesting is tested with a real nested Core
Foundation loop, not an AppKit modal window.

Another collector (`m10-cut7-4ee50baec5`) started after these diagnostics,
while this lane entered its initial cost quiet gate. This lane stopped
its own idle collectors before any cost sample, retaining the successful
gate and ownership observations under `landing/silver/fling-ownership-wait/`.
A second cut 7 coordinator then appeared after this lane’s first
qualified fling original. This lane again released its idle collector.
The resumed coordinator skips completed slots, whether accepted or
failed, and takes a fresh initial quiet gate; it keeps that original
unchanged. `resume-provenance.json` records the original and resumed
coordinator hashes and the retained slot. The first pair therefore
straddles the ownership interruption; no measured original is replaced.
The cold cohorts start only after this handoff. The primary cost result
keeps all five pairs; a separately reported sensitivity uses pairs 2–5,
whose collection is uninterrupted. This selection is written before
those four pairs finish, not chosen from their results.

**Landing source and reproduction.** The final source is
`c2bbac1010565b1bc606dad1b3fb7efe7aaa0741`, against main
`15c20224e2a16165bad87552a036d90d16fd4d91`. `build-landing.json` and
`build-landing.log` retain the release build, target directory and source;
later commits change only measurement records and the queue. The same three
control executables are reused. All five arms load probe SHA-256
`956fa19e69c1eb98619c1e476bf1f818b9f77f1c6fdcafd1cd1b09771b938dcc`.
The background diagnostic uses a separate copied bundle and probe; it
does not supply a scoreboard sample.

The retained silver session is
`/Users/ccheever/projects/exact2-measure/target/g0/m10-review-fix/landing/`,
mirrored locally at `target/m10-review-fix/landing/silver/`. Launches are
process-cold with application and system caches retained. The driver
commands, environment policy, original slots, quiet observations and
report-only launch maxima are in `run-review.py`, `provenance.json` and
each cohort's `schedule.json` and `quiet-*.jsonl`. Reproduce the final
tables from the repository root:

```sh
python3 target/m10-review-fix/landing/reduce.py
python3 target/m10-review-fix/landing/reduce-cold.py
python3 target/m10-review-fix/landing/phase-work.py
python3 target/m10-review-fix/landing/aggregate-check.py
python3 target/m10-review-fix/landing/aggregate-cold.py
python3 target/m10-review-fix/landing/launch-table.py cold
python3 target/m10-review-fix/landing/launch-table.py cold-traced
python3 target/m10-review-fix/landing/trace-summary.py
python3 target/m10-review-fix/landing/background-summary.py
python3 target/m10-review-fix/landing/audit.py
```

**Landing fling result: the instruction prediction misses.** All ten
originals qualify, giving five pairs at `c2bbac1010`. Ranges below are
the minimum and maximum paired differences, not confidence intervals.

| Metric | After − main median [range], n=5 |
| --- | ---: |
| Process CPU, ms/s | +0.016 [−8.697, +6.365] |
| Cycles, million/s | +8.018 [−7.705, +11.352] |
| Instructions, million/s | +26.352 [+10.743, +28.312] |
| Late intervals/s | −0.021159 [−0.246291, +0.804901] |

The predicted ±10 million instructions/s is missed in all five pairs.
The four uninterrupted pairs give +26.958 [+21.346, +28.312] million/s,
so the first pair's ownership gap does not explain the increase. CPU's
range crosses zero; cycles' range crosses zero in the primary five pairs
but not in the uninterrupted four (+9.347 [+1.990, +11.352] million/s).
All five after launches have zero rest frames. The third fix attempt
does not remove the fling regression, and the queue retains it.

| Cumulative process work | After − main, million instructions, n=5 |
| --- | ---: |
| Before fling begins | −283.961 [−291.771, −181.035] |
| During fling | +264.017 [+108.953, +284.770] |
| Through fling's end | −19.944 [−72.082, −3.988] |

These counters support work moving across the boot/fling boundary;
the final source still does less total work through fling's end. The
traces independently locate two decodes moved into fling, with no
post-boot observer and no direct picture-arrival draws. They do not
assign every extra instruction to those two decodes. `phase-work.py`
reproduces the cumulative comparison, and `reduce.py` reproduces both
the primary and uninterrupted-pair distributions.

The unchanged four-arm reducer refuses an Ink-only series because its
other arms are absent; that failed check is retained under
`landing/aggregate/before/fling/`. Each original instead passes its
`--sample` instrument check, and `aggregate-check.py` verifies the CPU
and instruction/cycle distributions against those outputs within their
printed precision. The cold cohorts use its full four-arm mode.

**Trace comparison, final five-round cohort.** All forty traced slots are
attempted; 39 qualify. The egui-before round 3 readiness failure remains.
The traced after medians are Native 250.120, GPUI 205.837, egui 205.812
and Ink 194.959 ms, n=5 each. Before Ink is 214.776 ms; its paired change
is −15.907 [−35.813, −8.328] ms, n=5. Ink after is below both controls in
this traced cohort. Its paired difference from GPUI is −7.536
[−22.737, +14.674] ms and from egui −17.873 [−21.717, −6.943] ms, n=5:
the median ordering is not a win in every GPUI pair.

Trace-associated differences use the adjacent matching arm, label and
round. The full column includes the executable wrapper. The second
column subtracts the probe's constructor-entry timestamp from each
launch before pairing; it leaves the measured app interval unchanged.
Ranges are the individual paired differences.

| Arm / label | Full trace-associated difference, ms | After constructor, ms |
| --- | ---: | ---: |
| native-before | 2.853 [-9.824, 248.101], n=5 | -1.836 [-13.878, 20.099], n=5 |
| native-after | 8.636 [0.724, 15.491], n=5 | 3.781 [-3.500, 10.652], n=5 |
| gpui-before | 9.028 [-6.924, 248.990], n=5 | 4.638 [-11.194, 24.371], n=5 |
| gpui-after | -1.868 [-23.086, 4.568], n=5 | -6.375 [-27.664, 1.383], n=5 |
| egui-before | 3.304 [-8.375, 30.112], n=3 | -1.350 [-12.048, 24.667], n=3 |
| egui-after | 0.788 [-8.211, 9.060], n=4 | -4.033 [-12.530, 4.285], n=4 |
| ink-before | -3.520 [-19.113, 13.298], n=5 | -8.920 [-24.950, 8.353], n=5 |
| ink-after | 6.742 [-0.266, 14.212], n=5 | 2.120 [-5.048, 9.573], n=5 |

All eight paired medians are below 10 ms. The <10 ms prediction **misses
as a per-launch bound**: Native-before and GPUI-before reach +248.101
and +248.990 ms. Their first traced launches spend 235.490 and 229.317 ms
before constructor entry; egui-before's first traced launch spends
187.808 ms there. Those intervals precede the probe's trace-enable read.
They belong to the launcher/loader interval, whose cause is not resolved
here. Every original remains in the row; none is removed as a warmup.
The signed app bundles had throwaway launches; the trace wrappers did
not have separate throwaways. The paired differences include launcher
overhead and run-to-run variation, so they do not isolate pure trace
collection cost.

The two untraced egui readiness failures in the matched rounds and the
one traced failure reduce the matched egui samples to n=3 before and n=4
after. `trace-cohort-summary.json` retains this completed trace comparison;
its untraced headline rows are provisional because rounds 6–10 were still
running. Both traced four-arm distributions, ranges and counts match
unchanged `aggregate.mjs` (`aggregate-cold.py cold-traced`).

**Collection pauses.** The untraced cohort stopped after 49 originals
when `mediaanalysisd` kept the machine above the existing quiet limit.
The next slot, egui-after round 7, exhausted its 300-second gate before
launching an app; it is not a measured failure. Its gate log and schedule
are retained in `cold/egui-after/quiet-7-before-gate-resume.jsonl` and
`cold/schedule-before-gate-resume.json`. `cold-resume-provenance.json`
hashes those files and both coordinator versions. The resume skips every
completed original, including the readiness failures.

A five-second `caffeinate -u` user-activity assertion predicted load at
or below one non-benchmark core within 120 seconds, sustained for the
existing resume check. The prediction holds: the first quiet observation
is 20.407 seconds later at 0.186 cores, followed by 61.389 seconds of
quiet observations. `quiet-wake-prediction.json`, `quiet-wake-result.json`
and `cold-resume-wait.jsonl` retain the prediction and result. No system
process was stopped and no sleep preference changed.

After two more originals, another collector started between Ink-before
and Ink-after round 7. This lane released its own idle collector;
`cold-owner-handoff.json` preserves all 51 completed originals. The
primary row retains all ten rounds. A sensitivity excluding round 7 was
declared before its Ink-after launch in `cold-handoff-sensitivity.json`.
This is one logical cohort with recorded pauses, not an uninterrupted
session or two independent sessions.

**Final cold rows, 2026-10-31.** Source `c2bbac1010` is compared with
main `15c20224e2`, using unchanged `measure.m` cold-only and the
first-complete-drawable probe on the silver. Hook and power are off;
the untraced cohort has no trace. Each of the five signed executables
has a successful throwaway launch (`throwaway.log`). The table gives
medians [minimum, maximum] and qualified n. All ten untraced and five
traced slots per arm and label were attempted once.

| Arm | Untraced before, ms | Untraced after, ms | Traced before, ms | Traced after, ms |
| --- | ---: | ---: | ---: | ---: |
| Native | 236.245 [217.617, 247.928], n=10 | 241.759 [230.728, 256.217], n=10 | 238.104 [226.410, 489.079], n=5 | 250.120 [244.087, 257.756], n=5 |
| GPUI | 196.795 [184.795, 208.887], n=10 | 198.280 [189.952, 207.706], n=10 | 208.096 [188.344, 446.075], n=5 | 205.837 [184.195, 209.678], n=5 |
| egui | 206.389 [200.263, 217.091], n=8 | 201.296 [195.753, 217.385], n=7 | 218.649 [203.464, 426.513], n=4 | 205.812 [203.365, 220.018], n=5 |
| Ink | 214.120 [204.642, 233.889], n=10 | 189.055 [179.885, 211.096], n=10 | 214.776 [203.287, 224.666], n=5 | 194.959 [186.941, 198.869], n=5 |

The 195–215 ms untraced p50 prediction **misses on the faster side**.
The median paired change is −21.531 [−43.335, +3.430] ms, n=10,
meeting the predicted −15 to −25 ms. The predeclared sensitivity without
round 7 is −20.814 ms, n=9, with the same range. Neither establishes a
saving in every pair.

Ink after is below both controls in this review cohort, with and without
tracing. Its untraced paired difference from GPUI is −10.450
[−23.617, +16.006] ms, n=10, and from egui −12.387
[−31.866, −1.253] ms, n=7. Without round 7, these are −10.833
[−23.617, +16.006], n=9, and −11.321 [−28.267, −1.253], n=6.
The ordering does not depend on enabling trace in this cohort. It does
not erase the earlier disagreement: cut 6's first untraced session put
Ink below GPUI, its traced cohorts put Ink 5–9 ms above, and the second
untraced confirmation put Ink 3.847 ms above. The two earlier sessions
gave a consistent Ink saving of about 17–20 ms; they did not repeat the
lead over GPUI. The current source has one review cohort, not two
independent confirmations. The 100 ms application obligation remains unmet.

**Qualification and retained evidence.** The untraced cohort qualifies
75/80 originals. Egui-before rounds 1 and 8 and egui-after rounds 4, 9
and 10 fail startup readiness. The traced cohort qualifies 39/40;
egui-before round 3 has the same failure. Every original remains, with
no replacement. All 120 launched originals pass the ten-second gate;
the shortest observed gate is 10.108 seconds untraced and 10.113 traced.
The failed, unstarted gate above is archived separately.

During-launch maxima are report-only and do not exclude a sample:

| Cohort | Non-benchmark busy cores | Largest user process, cores | Whole-machine busy cores |
| --- | ---: | ---: | ---: |
| Untraced | 3.710 | 0.307 | 7.770 |
| Traced | 1.750 | 0.287 | 8.554 |

`cold-launches.md` and `cold-traced-launches.md` under `landing/` give
every original's gate, maxima, result and failure reason. `reduce-cold.py`
reproduces the table, paired comparisons and qualification summary.
All four before/after cohort distributions match unchanged
`aggregate.mjs`, including ranges and counts. `audit.txt` verifies every
slot, the forty adjacent trace pairs, identical probes, executable
hashes and signatures, instrument hashes, and the unchanged originals
across resumes. Power sampling is absent in all 2,009 observations.
Each of the five traced Ink-after launches retires its observer once
after complete display, with zero trace drops and zero drawable calls
on main (`cold-retirement.json`). Raw and analysis SHA-256 manifests are
`raw-sha256.json` and `analysis-sha256.json`.

The review bugs are fixed and the cold rows are recorded. The chunk is
**not ready to land**: all three instruction-fix attempts miss, the final
fling increase remains queued, and the full affected run has the two
reported timing failures. The eight named suites and focused lifecycle
tests pass. The mini still owns the six-check gate; staged caps passes here.


## M10, landed: the six cuts over cut 7

**Predictions before collection, 2026-10-31.** The rebased source is
`cb7b01e228`, on `origin/main` `bc45470708`. The before carrier is that
main, including cut 7 and its review fixes; after adds the six startup
cuts and their review fixes. No renderer policy changes in this chunk.

The silver M5 Max runs one interleaved session through unchanged
`measure.m` cold-only: ten originals per arm and label, then five traced
originals per Ink carrier for the timeline. Each signed executable first
gets a throwaway launch. Every original requires ten quiet seconds;
during-launch maxima are report-only. Failed originals remain without
replacement, with successful n beside each median. These are fresh
processes with retained application and system caches, hook and power
off. The untraced row has tracing off. Native keeps its snapshot endpoint;
the other arms use the first complete displayed drawable.

| Prediction | Instrument |
| --- | --- |
| Ink before p50 210–215 ms; after 185 ms, estimated band 180–190 ms | Ten cold-only launches per carrier, same probe |
| After at least 10 ms below both GPUI and egui; before within 10 ms of both | Contemporaneous four-arm cold rows |
| First paint remains about 5 ms in both carriers; after complete commit at 150–160 ms and complete display at 180–190 ms | Five traced originals per carrier; process-age timeline and frame identity |
| After first positive display is complete in all five traces; boot observer retires once; no retries after retirement | Presented-frame identity and boot trace marks |
| Fling, rest, bar and caret process CPU and cycles within ±5% of before; fling instructions increase by no more than 30 million/s | One sample per carrier and workload with phase counters; rest is the fling's rest phase |
| No rest frames; bar within 2% of 120 frames/s; caret within 2% of 2 frames/s; no warm glyph uploads in rest/bar/caret | Same samples and glyph batch/copy/byte counters |
| Pixel parity and CPU-to-Ink transition survive the rebase | Affected suites, first-frame parity and Stage 0b transition |

The 185 ms estimate is 212.5 − 18.5 − 9 ms: the six cuts' historical
17–20 ms saving plus a hypothesis that the shorter first paint allows
about 9 ms more boot-tick work to overlap the window's first composite.
That last term is an additional overlap benefit, not a measured saving;
shared work could instead make the cuts non-additive. The trace will
locate first paint, complete commit and complete display separately.
The instruction allowance retains the earlier +26.352 million/s fling
finding, associated with two decodes moving out of boot. One cost pair
can detect a large change but cannot establish a stable absence of
regression. The 100 ms application obligation remains unmet.

**Rebase.** All 54 lane commits replay onto `bc45470708`. `RESULTS.md`
conflicts at the first appended cut-1 record: main's G7, cut-7 and review
records precede this lane's records. LLP 1149.001's M10 status retains
main's cut-7/A7 account before cut 1. LLP 1149 §4.1 retains cut 7 before
cut 2 and the later lane sessions. `QUEUE.md` conflicts when cut 2,
cut 3, cut 4, cut 5, cut 6, confirmation and review-fix commits replace
the same follow-up: both main atlas entries survive, followed by the
latest lane follow-up. `MEASUREMENTS.md` and the code have no conflicts.
Cut 7's batch upload seam and this lane's boot latch, generation checks,
retry reservations and complete-display tracking merge automatically.
The original branch bundle and per-commit conflict record are retained
under `target/m10-landed/`. The six broad checks stay with the Mac mini;
this chunk runs the affected suites and staged structural caps locally.

**Builds and instrument.** Both release carriers build in this worktree
with `--target-dir target` and `MACOSX_DEPLOYMENT_TARGET=14.0`: before
`bc454707081019b881e3c2340f71fa31e2cd529a`, after
`1502f94f541376c05312b5f2f40adfa4712648db` (the prediction commit;
its renderer is `cb7b01e228`). The controls retain native `d16c84ee03`
and GPUI/egui `a91fd38b4d`. Every arm loads the same probe bytes,
SHA-256 `956fa19e69c1eb98619c1e476bf1f818b9f77f1c6fdcafd1cd1b09771b938dcc`.
The driver and wheel poster are unchanged from the review cohort.
`provenance.json`, `build-{before,after}.json` and the build logs retain
sources, commands, hashes and deployment target; `repaired-hashes.txt`
verifies the measured bundles and their signatures on the silver.

The first throwaway attempt exposes a packaging error: the Ink template
places the probe in `Resources`, while the library's install name names
`Frameworks/probe.dylib`. Both Ink processes abort in dyld. The retained
`InkBeforeReady.app` and `InkAfterReady.app` add those identical bytes at
the required path and are re-signed; the original bundles and failures
remain. Both corrected throwaways exit successfully. Native and GPUI's
throwaways succeed; egui's times out waiting for a complete frame and is
retained without another throwaway. No measured original precedes this
repair. The packaging repair changes no renderer source.

**Affected suites.** All twelve invocations pass on the rebased source,
with `EXACT_REQUIRED=1`, two test threads and this worktree's `target/`.
The full Linux library and integration run has no timing failure in this
collection. The pinned, text, image and first-frame checks run on both
Metal and wgpu. The archive-miss transition test covers the CPU first
frame and its replacement by a complete Ink frame. Verbatim result
lines and commands are in `checks/result-lines.txt`; complete logs and
exit statuses are in `checks/`.

The broad six-check gate is assigned to the Mac mini and is not run in
this worktree. The local caps invocation is structural only
(`EXACT_CAPS_WASM=0`), as in cut 7's landing record.

**Combined cold session.** The uninterrupted `m10-landed` collection on
the silver M5 Max completes all 80 originals: 78 qualify. Egui-before
round 6 and egui-after round 8 time out waiting for readiness; both remain
without replacement. Every launched original passes ten quiet seconds
(minimum 10.112 s). The table gives medians [minimum, maximum], with
qualified n. Native's endpoint is its settled-layer snapshot; the painted
arms use the first complete drawable's displayed timestamp.

| Arm | Before ms [range], n | After ms [range], n |
| --- | ---: | ---: |
| Native | 239.284 [226.097, 259.262], n=10 | 239.689 [229.061, 252.924], n=10 |
| GPUI | 195.449 [184.662, 208.277], n=10 | 197.134 [187.733, 206.894], n=10 |
| egui | 204.924 [199.402, 225.221], n=9 | 203.295 [194.295, 214.803], n=9 |
| Ink | 187.214 [165.532, 202.727], n=10 | 159.679 [148.594, 188.900], n=10 |

Ink's median row falls 27.535 ms. The ten paired after-minus-before
changes have median **−29.634 ms [−52.353, +17.985]**; individual pairs
are not uniformly faster. After is **37.455 ms below GPUI** and
**43.616 ms below egui**, meeting the predicted margins of at least 10 ms.
Before is 8.235 ms below GPUI but 17.710 ms below egui, missing the
prediction that it would be within 10 ms of both.

Both absolute predictions **miss on the faster side**: before is
187.214 ms against 210–215 ms; after is 159.679 ms against 185 ms
(estimated band 180–190). The observed 27.535 ms difference between the
median rows is close to the predicted 27.5 ms, but this does not isolate
the hypothesized extra 9 ms of overlap. The first-paint and presentation
timeline below measures the combined path. The 100 ms obligation remains
unmet.

During-launch maxima are report-only: 1.669 non-benchmark busy cores,
0.328 core for the largest user process and 6.960 whole-machine busy
cores. `cold-launches.md` gives each original's gate, load, result and
failure. Both four-arm distributions match unchanged `aggregate.mjs`
exactly, including ranges and counts (`aggregate-check.txt`).

**Timeline.** Five traced originals per Ink carrier qualify, with no
replacement or trace drops. Values are medians [minimum, maximum], n=5;
ages start at process creation. First complete commit means the first
present commit whose captured scene is complete; that drawable may be
dropped. Complete display is the probe's first positive complete-drawable
timestamp. Medians of different events are not additive phase durations.

| Event | Before, ms [range] | After, ms [range] |
| --- | ---: | ---: |
| First paint begins, process age | 158.152 [154.051, 424.448] | 128.988 [112.478, 342.391] |
| First paint ends, process age | 163.437 [159.176, 429.767] | 132.846 [116.103, 346.112] |
| First paint work, duration | 5.285 [4.993, 5.583] | 3.721 [3.605, 3.857] |
| First complete commit, process age | 173.468 [169.966, 440.546] | 150.278 [133.797, 370.765] |
| First complete display, process age | 196.315 [188.224, 458.866] | 166.537 [150.097, 388.334] |

Before's 5.285 ms paint is near the predicted 5 ms; after's 3.721 ms is
faster than predicted. The
150–160 ms complete-commit prediction is met at 150.278 ms. The
180–190 ms display prediction misses on the faster side at 166.537 ms.
Each after launch first displays complete content, retires its boot
observer once, and has zero retries after retirement, meeting those
predictions. Retirement occurs 17.201–33.871 ms after complete display.
Both carriers make zero drawable acquisitions on main in every trace.

The first traced wrapper launch has 258.385 ms pre-main before and
207.074 ms after; later launches have 7.857–8.718 ms before and
8.220–11.342 ms after. All ten originals remain in the table. These
pre-constructor delays, and the separate placement of the trace cohort,
prevent a pure tracing-overhead claim. The first-paint work itself is
4.993–5.583 ms before and 3.605–3.857 ms after even in those launches.
The timeline locates the combined saving; it does not isolate an
additional 9 ms overlap term. During-launch maxima are 1.653 non-benchmark
busy cores, 0.188 user-process core and 7.688 whole-machine busy cores,
report-only. `timeline-launches.md` and `summary.json` retain every trace
join and qualification result.

**After boot.** All six original cost launches qualify: one fling/rest,
bar and caret sample per carrier, counters enabled, hook/trace/power off.
The cost collection starts with sixty quiet seconds and each launch passes
its ten-second gate; phase observations pass the existing sustained-load
rule. These are single pairs, not replacements for G4's CPU scoreboard
or a repeated no-regression test.

| Phase | Process CPU ms/s, before → after | Mcycles/s, before → after | Minstructions/s, before → after | CPU / cycles change |
| --- | ---: | ---: | ---: | ---: |
| fling | 136.065 → 136.173 | 244.343 → 242.610 | 508.660 → 540.755 | +0.079% / -0.709% |
| rest | 3.091 → 3.189 | 4.360 → 4.568 | 4.299 → 4.472 | +3.147% / +4.769% |
| bar | 182.297 → 186.226 | 260.839 → 265.321 | 438.282 → 439.603 | +2.155% / +1.718% |
| caret | 31.740 → 30.163 | 44.860 → 42.986 | 98.564 → 92.716 | -4.971% / -4.178% |

All four phases meet the ±5% CPU and cycle prediction. Fling instructions
rise **32.095 million/s (+6.310%)**, missing the ≤30 million/s allowance.
This keeps the instruction regression open; this chunk changes no source
to attempt a fourth fix. Cumulative instructions are 258.231 million lower
before fling but 63.499 million higher through fling's end in this pair
(`phase-boundary.json`). Thus this pair does not establish pure displacement
of boot work. The earlier five-pair diagnosis and three failed fix rounds
remain in the preceding record.

Both rest phases display zero frames. Bar runs at 119.784 → 119.878
frames/s and caret at 2.000 → 1.999, meeting their ±2% cadence predictions.
Rest, bar and caret each upload zero glyph batches, copies and bytes.
The phase-end counter differences are:

| Phase | Arm | Paints / submissions / displays | Service passes | Glyph batches / copies / bytes |
| --- | --- | ---: | ---: | ---: |
| fling | ink-before | 347 / 347 / 338 | 3718 | 18 / 34 / 578243 |
| fling | ink-after | 348 / 348 / 336 | 3693 | 18 / 35 / 575994 |
| rest | ink-before | 0 / 0 / 0 | 97 | 0 / 0 / 0 |
| rest | ink-after | 0 / 0 / 0 | 102 | 0 / 0 / 0 |
| bar | ink-before | 1200 / 1199 / 1198 | 6013 | 0 / 0 / 0 |
| bar | ink-after | 1200 / 1198 / 1199 | 6011 | 0 / 0 / 0 |
| caret | ink-before | 20 / 20 / 20 | 2553 | 0 / 0 / 0 |
| caret | ink-after | 20 / 20 / 20 | 2248 | 0 / 0 / 0 |

`summary.json` also retains main CPU, effective clock, IPC, peak/end
footprints, displayed cadence and the complete counter vector for each
sample. The committed reducer's `--sample` validation passes all six raw
records. Stable CPU/cycles and cadence in one pair do not close the
instruction finding or establish a stable absence of regression.

**Predictions beside results.**

| Prediction | Result | Verdict |
| --- | --- | --- |
| Before cold p50 210–215 ms | 187.214 ms, n=10 | Miss, faster |
| After cold p50 185 ms, band 180–190 | 159.679 ms, n=10 | Miss, faster |
| After ≥10 ms below GPUI and egui | 37.455 / 43.616 ms below | Met |
| Before within 10 ms of both controls | 8.235 / 17.710 ms below | Miss for egui |
| First paint about 5 ms in both | 5.285 / 3.721 ms | Before near 5; after misses faster |
| After complete commit at 150–160 ms | 150.278 ms, n=5 | Met |
| After complete display at 180–190 ms | 166.537 ms, n=5 | Miss, faster |
| After first positive display complete, one retirement, no retries after retirement | All five traces meet all three | Met |
| Fling/rest/bar/caret CPU and cycles within ±5% | All four pairs within ±5% | Met for these single pairs |
| Fling instruction increase ≤30 million/s | +32.095 million/s | Miss |
| Zero rest frames; bar 120/s and caret 2/s within ±2%; no warm glyph upload in rest/bar/caret | 0; 119.878/s; 1.999/s; zero uploads | Met |
| Pixel parity and CPU-to-Ink transition survive | All affected suites pass | Met |

**Reproduction and limits.** Raw originals, quiet observations, ownership
checks, throwaways, source/build receipts, signed bundles and instruments
remain at `target/m10-landed/` in this worktree and, for the collected
session, `target/g0/m10-landed/` on the silver. The local `silver/` directory
mirrors that session. `run-session.py` retains the exact schedule and
invocations; `launch-repaired.json` records the GUI-session launch. The
measure command is the unchanged `measure.m` binary with `cold-only`,
one original at a time, `G1_GPU_HOOK=0`, `EXACT_INK_DEVICE=metal` and the
selected arm. Traced wrappers enable `G1_TRACE=1`; cost launches enable
`EXACT_INK_COUNTERS=1` and select bar/caret with `BENCH_MOTION`.

From this worktree, reproduce the tables and audit with:

```sh
python3 target/m10-landed/reduce.py
python3 target/m10-landed/aggregate-check.py
python3 target/m10-landed/audit.py
```

`audit.txt` verifies all 80 cold, ten timeline and six cost originals,
adjacent before/after startup pairs, ten-second gates, matching instrument
hashes and Ink bundle signatures. Power sampling is absent in all 1,206
observations. `raw-sha256.json` hashes the retained session tree;
`analysis-sha256.json` hashes the reducers and their reports. Complete test
logs and verbatim result lines remain in `checks/`; staged structural caps
passes. The six broad checks, including the WASM size gate, remain the
Mac mini's work.

The combined carrier is below both controls in **1 of 1 qualified
sessions**. Across the four qualified untraced sessions of six-cut carrier
revisions, the count is **3 of 4**; the confirmation and cut 6's traced
cohorts remain contrary observations. This tip has not had a second
independent combined session. The 100 ms obligation remains unmet, and
the fling instruction increase remains queued. The branch is committed
for the mini's gate; it has not been pushed.

## G7b, Windows at 120 Hz with cut 7

**Predictions before collection, 2026-11-01.** Compare `5dc29e7bb2`
(before cut 7) with `5892f92d5a` (origin/main at preparation) in one
NucBox console session. Ten interleaved fresh processes per revision and
arm retain application, OS and driver caches. Five rotating fling/rest
rounds use the after revision; rest, bar and caret also get one separate
sample per arm. G6's whole-series quiet rule remains: every five-second
observation below 10% total CPU and no compiler. No build overlaps timing.
PresentMon 2.6.0 supplies displayed timestamps; late means over 12.5 ms.
Ink/wgpu's readback/softbuffer path has no app-owned displayed presents,
so its displayed cadence remains unavailable if PresentMon cannot observe it.

| Prediction | Instrument and criterion | Result |
| --- | --- | --- |
| D3D11 first paint saves 10–20 ms; DWM first complete screen falls from about 161 to 140–150 ms. | Existing Windows first-paint log, separately from the ten-launch DWM endpoint. DXGI display should fall by 10–20 ms in matched pairs. | First paint saves 1.23 ms paired; after DWM 192.98 ms and DXGI paired saving 2.28 ms: all miss. |
| Windows first paint replaces roughly 250–310 glyph uploads with two dirty-page copies. | Existing painter counters in separate diagnostics. D3D11 uses one `UpdateSubresource` and completion query per dirty page instead of per mask; wgpu uses one staging submission. Windows has no glyph-upload duration span, so first-paint time cannot establish upload time alone. | Two copies in every after diagnostic: met. Baseline upload count unavailable: 250–310 untested. |
| Ink/wgpu DWM first screen saves 10–25 ms, reaching 700–715 ms; egui remains within 10% of G7's 239.05 ms. | Ten launches per arm and revision, same DWM proxy as G7; no DXGI startup endpoint exists for these arms. | After wgpu 730.36 ms, paired saving 33.92 ms; egui 275.75 ms: all stated bands miss. |
| D3D11 and egui reach 116–120 displays/gesture s, at most 1 late interval/s and p99 at most 12.5 ms. | PresentMon, four gesture windows in each of five rounds. | Uncollected after two foreground failures; untested. |
| Fling process/main CPU is 168/125 ms/s on D3D11, 243/224 on Ink/wgpu and 50/50 on egui, within 25%. | Process and HWND-owner-thread CPU endpoints over ten seconds; accounting uses 15.625 ms ticks. | Uncollected; untested. |
| Peak working set stays within 10% of 153/325/182 MiB for D3D11/Ink-wgpu/egui. | Lifetime peak working set at the rest endpoint, beside sampled fling peak. | Uncollected; untested. |
| D3D11 snapshots equal successful frames. | Existing exit counters, including startup. | Cold before 39/39, after 40/40: met. Five flings uncollected. |
| Rest has zero displays and at most two CPU ticks in ten seconds; bar reaches 116–120 displays/s; caret displays 2.0/s within one frame per sample. | One ten-second static sample per arm, CPU endpoints and PresentMon. Ink/wgpu cadence may remain unavailable. | Uncollected; untested. |

No carrier change is planned. A foreground loss retains the foreground
window's title and process and permits one retry; a second loss stops
fling collection. Raw files belong under `target/g7b/` in this worktree
and the lane's own NucBox mirror; no earlier retained evidence is removed.

**Capture repair prediction, before collection.** The initial cold captures
close the application as soon as its carrier logs readiness. They produce
no PresentMon CSV and report lost ETW events. PresentMon 2.6.0 defers events
up to two seconds and resolves a live process's image name in its output
thread ([main thread](https://github.com/GameTechDev/PresentMon/blob/v2.6.0/PresentMon/MainThread.cpp),
[output thread](https://github.com/GameTechDev/PresentMon/blob/v2.6.0/PresentMon/OutputThread.cpp)).
Keeping each cold application alive until the existing four-second capture
finishes should produce nonempty D3D11 and egui CSVs with zero lost events;
Ink/wgpu should still have no app-owned presents. The carrier's already
logged DWM/DXGI endpoints stay unchanged. This changes the observer's
application lifetime, not the renderer or presentation policy.

**Final capture repair prediction, before collection.** Keeping the process
alive did not repair capture: the D3D11/wgpu/egui diagnostics lost
6,841/6,646/7,080 ETW events and produced no CSV. That prediction missed.
PresentMon's PID option filters the consumer before output; the name option
filters output only. Start the same capture immediately after `Popen`, with
that process's PID, and retain the process until capture ends. Predict at
least one displayed record for D3D11 and egui and zero lost ETW events in
all three diagnostics; wgpu remains unavailable. This may miss the earliest
startup events, so it cannot independently certify the first display unless
its timestamps establish that correspondence. The native DWM/DXGI endpoints
retain their definitions. This is the third and final capture configuration.

### G7b collection and first paint

The measured binaries are release builds of `5dc29e7bb260d00f9d6f98d0ff6fbed586e9ca4e`
and `5892f92d5a9de07f7be8cc170d8c48e434070281`, built in this lane's
NucBox mirror, with separate before/after output directories. No carrier or
renderer source changed. The desktop records 5120×1440 at **120 Hz**, 96 DPI;
Windows 11 build 26200 and Radeon driver 32.0.23033.1002 match G7. Windows
Search remained active after the builds. The first cold cohort completed all
60 launches but failed **60 of 109** quiet observations, maximum **16.4%**
CPU. It is excluded whole. A separate five-second process observation records
SearchIndexer consuming 8,375 CPU ms in 5.116 s; this locates background work,
not its share of every failed interval.

Five interleaved first-paint pairs per Ink device use the existing
`EXACT_AGENT_WINDOW_FRAMES=1` log and painter counters. All 20 exit normally;
all five quiet observations pass, maximum **9.7%** total CPU. These are
first-paint diagnostics, not the complete-screen endpoint. Medians
[min–max], milliseconds:

| Device | Before first paint | After first paint | Before minus after, paired |
| --- | ---: | ---: | ---: |
| D3D11 | 8.13 [7.95–8.27] | 6.81 [6.72–7.01] | 1.23 [1.15–1.38] |
| Ink/wgpu DX12 | 36.93 [36.15–37.73] | 16.88 [15.96–17.68] | 19.42 [19.25–21.09] |

D3D11's predicted 10–20 ms first-paint saving **misses**. The difference of
medians is 1.32 ms; 1.23 ms is the median of matched pairs. Every after
process records **one glyph batch, two page copies, 72,390 bytes and one
paint attempt**. The two-copy prediction is met. The baseline lacks the
per-glyph upload counter, so the predicted 250–310 Windows uploads is
**untested**; Mac's 307 is not a Windows count. D3D11 issues one
`UpdateSubresource` and completion query per page; these timings measure the
whole paint and cannot isolate the upload call's saving. No Windows upload
duration span was added.

### G7b cold carrier endpoints beside G6e/G6 and G7

The repeat (`series-2`) completes all **60 launches**, with **109 of 109**
quiet observations passing, maximum **9.5%** CPU and no compiler. Each arm
has ten before/after pairs, alternating the revision order and rotating the
arm order. These are **carrier endpoint results only**. The concurrent
startup PresentMon traces produce no CSV and lose events; the requested
PresentMon corroboration is unavailable. Cold launches use G7's endpoint
qualification, which does not certify foreground ownership or unoccluded
pixels. No failed-focus fling is used as a startup sample.

Medians [min–max], ms; ten processes per cell. Application, OS and driver
caches are retained. G7b adds a separate PresentMon process before each
launch; G7's startup driver did not start PresentMon. There is no observer-off
control here, so the difference from G7 is not an isolated renderer change.
The paired G7b columns share the same observer configuration.

| Endpoint | 60 Hz, G6e / G6 | 120 Hz, G7 | 120 Hz, G7b before | 120 Hz, G7b after |
| --- | ---: | ---: | ---: | ---: |
| D3D11 DWM proxy | 160.78 [151.81–171.37] | 161.18 [146.55–175.67] | 194.10 [178.46–200.83] | 192.98 [172.38–205.10] |
| D3D11 DXGI display | 177.66 [170.41–182.54] | 167.03 [153.72–181.89] | 199.96 [184.38–206.51] | 199.15 [178.38–209.76] |
| Ink/wgpu DWM proxy | 716.21 [706.02–736.88] | 723.48 [687.45–834.75] | 758.66 [719.69–791.44] | 730.36 [715.54–767.56] |
| egui DWM proxy | 248.76 [240.70–255.15] | 239.05 [229.51–263.04] | 278.20 [263.34–288.50] | 275.75 [264.68–280.11] |

The cold D3D11 snapshot invariant holds in every launch: **39/39** before
and **40/40** after. It remains untested over five flings.

Ink/wgpu and egui expose no DXGI startup endpoint. The independently logged
D3D11 DXGI median remains above the 100 ms application obligation.

| Prediction | Observed carrier endpoint | Verdict |
| --- | --- | --- |
| D3D11 DWM 140–150 ms | 192.98 ms; paired saving 2.37 [−26.64–18.39] ms | Miss, slower |
| D3D11 DXGI paired saving 10–20 ms | 2.28 [−25.38–18.18] ms | Miss, smaller saving |
| Ink/wgpu DWM 700–715 ms and paired saving 10–25 ms | 730.36 ms; saving 33.92 [−27.92–68.16] ms | Both miss: slower endpoint, larger paired median saving |
| Egui within 10% of G7's 239.05 ms | 275.75 ms, +15.35%; paired saving 2.09 [−12.69–17.25] ms | G7 band misses; the paired before/after medians differ by 0.88% |

### G7b PresentMon startup limit

The three capture configurations all fail. The original four-second capture
starts a second before the application and closes the application at native
readiness. The next keeps it alive through capture; the last starts a
PID-filtered capture immediately after process creation and keeps it alive.
The last two are diagnostics, not replacements selected into the cold cohort.

| Capture | D3D11 lost ETW events | Ink/wgpu lost ETW events | Egui lost ETW events | CSV |
| --- | ---: | ---: | ---: | --- |
| Original, quiet `series-2`, before / after totals over ten launches | 40,152 / 39,854 | 56,325 / 46,309 | 40,061 / 39,749 | None |
| Keep alive, one after process each | 6,841 | 6,646 | 7,080 | None |
| PID filter, one after process each | 8,140 | 7,595 | 8,298 | None |

Both repair predictions **miss**. All six diagnostic applications and
PresentMon processes exit zero. Capture repair stops after these three
configurations; no absent CSV is interpreted as zero displays. The failed
cold-capture additions are removed from the maintained driver and retained
as exact instrument archives and patches under `target/g7b/`. The driver
keeps the requested alternate-binary, static-motion and foreground-recording
support. The summary rejects failed-focus originals and reads a successful
retry's own CSV. The carrier is unchanged, so `host/ink/MEASUREMENTS.md`
needs no carrier amendment.

### G7b foreground and the missing 120 Hz rows

The first D3D11 fling fails foreground acquisition before its timed phase.
Its one retry fails the same way. Both raw observations identify HWND
**393270**, PID **22076**, process **ChatGPT.exe**, title **ChatGPT**. The
first QPC is **505490278762**, the retry **505584299146**, at 10 MHz. No wheel
input is posted. Collection stops without another focus workaround. No
five-round fling cohort or separate rest/bar/caret sample is collected.
The static cases require the same visible, foreground-qualified window;
they are not filled from an unfocused run.

The following 60 Hz values remain historical; the missing 120 Hz cells
have no median or range. Source and refresh both differ from G6e/G6.

| Arm / refresh | Displays / gesture s | Late intervals / s | p99, ms | Fling process / main CPU, ms/s | Peak working set, MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| D3D11, 60 Hz (G6e) | 58.15 | 0 | 16.85 | 84.08 / 62.31 | 153.18 |
| D3D11, 120 Hz (G7b) | Uncollected | Uncollected | Uncollected | Uncollected | Uncollected |
| Ink/wgpu, 60 Hz (G6) | Unavailable | Unavailable | Unavailable | 121.41 / 112.02 | 324.75 |
| Ink/wgpu, 120 Hz (G7b) | Uncollected | Uncollected | Uncollected | Uncollected | Uncollected |
| egui, 60 Hz (G6) | 57.86 | 0 | 16.88 | 24.91 / 24.91 | 181.74 |
| egui, 120 Hz (G7b) | Uncollected | Uncollected | Uncollected | Uncollected | Uncollected |

| Arm | 60 Hz rest process / main CPU, ms/s | 60 Hz rest displays | 60 Hz ending working set, MiB | 60 Hz static bar / caret | 120 Hz separate rest / bar / caret |
| --- | ---: | ---: | ---: | --- | --- |
| D3D11 | 0 / 0 | 0 | 153.16 | Bar unmeasured; caret 2.0 redraws/s only (M2b) | Uncollected |
| Ink/wgpu | 1.55 / 1.55 | Unavailable | 322.48 | Bar unmeasured; caret 2.0 redraws/s only (M2b) | Uncollected |
| egui | 0 / 0 | 0 | 170.00 | Unmeasured | Uncollected |

The 60 Hz rest values are medians of the post-fling phases; M2b's caret
counter counts redraws, not displayed frames, and its CPU cohort failed
quiet qualification. The 120 Hz cadence, late-interval, p99, CPU, memory,
fling snapshot and static predictions remain **untested**, not numerical
misses. This session establishes no carrier cadence limit and makes no
presentation-policy change. Late would mean over 12.5 ms at 120 Hz, compared
with G6's 25 ms.

### G7b reproduction and verification

All originals remain at `target/g7b/` in this worktree and
`C:\Users\Charlie Cheever\projects\exact2-1149-001-win120b\target\g7b\`
on the NucBox. The local `nucbox/` tree mirrors the observations: both
60-launch cold cohorts, all 20 first-paint processes, both failed foreground
attempts and all six capture-repair diagnostics. No earlier retained files
were removed. Only this lane's two completed scheduled tasks were removed.
No black-Mac input or focus operation was used.

`before-source.tar.gz` and `after-source.tar.gz`, `sources.json`, the build
script and four build logs pin the inputs and commands. The NucBox retains
the before/after Ink bundles and egui executables; their SHA-256 receipts
match every measured process. `prepare-data.log` records the shared data
check. Both standalone workspaces use this mirror's own `target/`.
`collect.py` and `cold-retry.py` retain the exact schedules. The latter waits
for six consecutive quiet observations in `waiting.jsonl` before the
all-observations cohort gate starts. Both runs keep the same capture
configuration and endpoint definitions. No sample is selected from the
loaded first cohort into the quiet second one.

`instrument.tar.gz` is the exact driver for both cold cohorts and the
foreground attempts. `instrument-repair.tar.gz` and `instrument-pid.tar.gz`,
their patches and the `pm-repair.ps1` / `pm-pid.ps1` invocations retain the
two failed capture changes. Reproducing those attempts requires their
archived drivers; the maintained driver no longer includes failed cold
capture. `first-paint.py` retains the one-frame diagnostic with counters and
quiet monitoring. `raw-sha256.json` hashes the local observation tree.

Reproduce the numeric tables and audit directly from the retained files:

```sh
python3 target/g7b/reduce.py series-1
python3 target/g7b/reduce.py series-2
python3 target/g7b/audit.py
```

The audit verifies all native endpoint values against their application
logs, binary hashes, the full interleaved schedules, all quiet observations,
first-paint counters, foreground failures and capture event losses.
`summary-validation-final.txt` records the unchanged G7 reduction, synthetic
0/120/2 displays/s and CPU endpoints, unavailable wgpu cadence, and rejection
of a failed-focus original while reading its retry's own CSV. Synthetic
values are reducer checks, not Windows measurements. Full check logs and
verbatim result lines are retained in `checks/` and `checks-rebased/`.

The branch is rebased onto `bf67258db5`. All four Windows release builds
pass. The six required checks pass after rebase, including both Clippy and
formatting; the root test run reports **7,470 passed, 0 failed, 83 ignored**
across 113 test binaries. Staged caps passes. Native Windows parity and
journey suites were not rerun; this lane changes measurement and records,
not the carrier. `checks-rebased/verbatim-results.txt` preserves the exact
result lines and commands, with complete output in the adjacent logs.
The branch is committed without a push.
