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
