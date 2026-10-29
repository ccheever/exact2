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
