# Heavy list: GPUI on the Mac

@ref LLP 1149 §4; G1.

**G1m remains incomplete: three clean interleaved pairs, not five.** Explicit
activation and key-window verification now precede each sample and gesture.
The continuation lost focus to Google Chrome during a fling, and its one
permitted retake lost focus to Ocho during startup. Neither qualifies. The
tables retain the three previously completed pairs as diagnostic samples;
they do not meet the requested five pairs or ten fresh launches per app.
The diagnostic attempts identified Ocho and Slack taking focus too. After
three runs of the corrected driver, work stopped at the lane's limit.

## Arms and provenance

- Native Exact: this checkout at `f41aaafec4`, `exact-heavylist`, built with
  `bun host/apple/build.mjs exact-heavylist-apple --release` and
  `EXACT_APP_DIR=$PWD/bench/heavy-list/exact-heavylist`.
- GPUI app and probe source: `a0f6fe8523` on `lane/gpui-arm`. The report and
  captures do not change either executable. G1m ran the existing instrumented
  release copies at report commit `11896dd84d`. The continuation rebuilds only
  `measure.m` and `probe.m`, replaces the probe in both instrumented copies,
  and ad-hoc signs them. Neither application's code was rebuilt. The table's
  retained samples used the earlier probe; the new probe produced no qualifying
  pair.
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
`ps` before launching, wait at most 20 minutes if necessary, and monitor the
same conditions during measurement. The desktop must be unlocked so each
window can become active and remain in front for the whole fling/rest sample;
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
`-retry`; a second loss stops the series. Activation does not prevent another
application from taking focus afterward.

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

**Diagnostic samples only: three pairs.** Values are the median (minimum–maximum)
of `series-g1m4/fling` round 1 and `series-g1m5/fling` rounds 1 and 2, under
`target/g1/`. Their order was native/GPUI, native/GPUI, GPUI/native. These six
processes passed the driver's focus and scroll checks, had nominal thermal
endpoints and Low Power Mode off, and had no compiler observed during their
measurements. All were fresh launches with warm system caches. No rejected run
or unpaired native run contributes to these tables.

| Metric | Native Apple host | GPUI 0.2.2 |
|---|---:|---:|
| Cold to complete-screen milestone, ms; **3 launches each, not 10** | 596.84 (441.92–621.55) | 338.49 (318.66–447.50) |
| Fling process CPU, ms/s | 121.41 (115.96–124.16) | 89.47 (88.60–93.55) |
| Fling main-thread CPU, ms/s | 81.77 (79.82–82.69) | 43.15 (43.14–45.15) |
| Peak footprint during fling, MiB | 224.16 (197.08–230.61) | 397.50 (396.16–398.16) |
| Lifetime peak footprint through end of rest, MiB | 226.38 (214.88–234.08) | 413.33 (403.80–417.89) |
| End-of-fling footprint, MiB | 216.86 (188.27–225.58) | 388.56 (387.58–388.97) |
| Rest process CPU, ms/s | 1.084 (1.076–1.113) | 5.113 (4.796–5.401) |
| Rest main-thread CPU, ms/s | 0.471 (0.467–0.491) | 3.301 (3.071–3.474) |
| End-of-rest footprint, MiB | 216.27 (187.78–224.97) | 208.52 (204.47–209.38) |

In these samples, GPUI used 31.94 ms/s less process CPU during the fling
(26.3%) and 38.62 ms/s less main-thread CPU (47.2%). Native Exact used
173.34 MiB less peak fling footprint and 186.95 MiB less lifetime peak footprint;
its rest process/main-thread CPU was lower by 4.029/2.831 ms/s. Native Exact's
end-of-fling footprint was 171.70 MiB lower. GPUI's end-of-rest footprint was
7.75 MiB lower, within the native runs' spread. GPUI's cold milestone was
258.34 ms earlier, but the milestones differ and this is not a startup win
measured with one instrument. These are whole-app observations from an
incomplete series, not a renderer ranking.

Equal input produced native final offsets of 8,454, 8,318 and 8,366 points;
GPUI ended at item 18, offset 255, every time. The driver posted 106 events
per gesture at an 8.33 ms nominal period in each selected run. The apps do not
establish equal scroll work from those inputs.

**The frame intervals use different instruments.** The native probe records
display-link cadence, not presented-frame timestamps. GPUI also records
`MTLDrawable.presentedTime`. Native cadence and GPUI presentation cannot
establish which app presents more smoothly; native presented-frame intervals
and late frames remain unmeasured.

| Arm and instrument | Interval p50, ms | p95, ms | p99, ms | Late intervals/s |
|---|---:|---:|---:|---:|
| Native, display-link timestamp | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 0.10 (0.00–0.30) |
| GPUI, display-link timestamp | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–16.67) | 0.30 (0.10–1.30) |
| Native, presented frame | unmeasured | unmeasured | unmeasured | unmeasured |
| GPUI, presented frame within gesture windows | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 8.33 (8.33–8.33) | 0.44 (0.22–0.44) |

Each run's quantiles use sorted intervals at index `floor(p × count)`, capped
at the final index; the table takes medians and ranges of those per-run values.
Late means strictly greater than 12.5 ms on this 120 Hz display. Display-link
late counts are divided by the whole ten-second fling sample. GPUI presentation
intervals join timestamps only within each individual gesture window; late
counts are divided by those windows' combined durations, 4.534–4.541 seconds.
Each run supplies 481–485 such intervals. Native display-link lateness was
0.20 intervals/s lower at the median; interval quantiles tied at this precision.
Neither observation establishes a presented-frame winner.

The requested series is unfinished. The three attempted series are retained
separately so the exclusions can be checked:

| Raw directory under `target/g1/` | Start, 2026-10-28 PDT | Completed clean pairs | Why the driver stopped |
|---|---|---:|---|
| `series-g1m4/fling` | 20:17:27 | 1 | GPUI round 2 lost focus for 198 fling callbacks and at the end; Cargo/rustc appeared during that round |
| `series-g1m5/fling` | 20:19:16 | 2 | GPUI round 3 lost focus during rest; its fling had zero inactive callbacks; no compiler observed |
| `series-g1m6/fling` | 20:23:58 | 0 | Native round 1 was inactive at both endpoints and for 1,002 fling callbacks; no compiler observed |

Every observed load average was below 13.3 across those attempts. The final attempt
held the display awake with `caffeinate -d -i -u -t 600`, released when the driver
exited; it did not cure lost focus. The desktop was unlocked before the retries;
cmux was frontmost after the second failure, but those probes did not identify
the application at the moment focus was lost.

The corrected driver's continuation, `series-g1m7`, began at 20:42:42 PDT on
2026-10-28 with `caffeinate` holding the display awake. All load averages stayed
below 10 and no compiler was observed. No sample from this continuation enters
the tables:

| Sample | Result | Foreground at the loss |
|---|---|---|
| `series-g1m7/fling/gpui-4.json` | Discarded during the fling | Google Chrome, `com.google.Chrome`, PID 60411 |
| `series-g1m7/fling/gpui-4-retry.json` | Discarded during cold startup; the permitted retake | Ocho, `dev.ocho.desktop`, PID 53948 |

The preceding `focus-validation` diagnostic deliberately activated Finder during
a native cold launch. The driver named Finder, rejected that launch and accepted
its one fresh retake. GPUI's launch and retake then lost focus to Ocho. A second
diagnostic, `focus-handoff`, started with GPUI to exclude native-to-GPUI handoff
as a necessary cause; both launches lost focus to Slack (`com.tinyspeck.slackmacgap`,
PID 36534). The logs establish the foreground applications, not whether a person
or automation activated them. No process belonging to another session was stopped.
The five additional cold pairs have not run. Finishing needs two clean fling/rest
pairs and five further cold-only pairs, about two uninterrupted minutes with
the benchmark windows in front and no compilation.

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

For the focus fix, both Objective-C files compiled with
`-O2 -Wall -Wextra -Werror`; the driver now links AppKit. The instrumented copies
were re-signed and launched. The deliberate Finder interruption verified rejection,
application identification and one successful fresh retake. Subsequent Ocho,
Slack and Chrome interruptions verified that a second loss stops the run.
`git diff --check` and `bun scripts/caps.mjs` passed:
“All budgets within cap. 10 categories inspected.” No workspace test suite was rerun.
The six selected launches were checked against their raw JSON and wheel logs:
800×800-point windows, 2× scale, 120 Hz, ten-second phases, monotonic CPU
counters and four identical 106-event gestures each.

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
already built in this worktree. To continue the retained three pairs, use fresh
directories for the remaining two fling/rest pairs and five cold-only pairs.
Each launch checks focus, and each gesture activates and verifies the target
again. Check and monitor the quiet rule above separately:

```sh
target/g1/measure "$PWD/target/g1/series-complete/fling" \
  "$PWD/target/g1/Native.app/Contents/MacOS/Heavy list" \
  "$PWD/target/g1/GPUI.app/Contents/MacOS/gpui-heavy-list" \
  "$PWD/target/g1/exact-wheel" 2 4
target/g1/measure "$PWD/target/g1/series-complete/cold" \
  "$PWD/target/g1/Native.app/Contents/MacOS/Heavy list" \
  "$PWD/target/g1/GPUI.app/Contents/MacOS/gpui-heavy-list" \
  "$PWD/target/g1/exact-wheel" 5 6 cold-only
```
