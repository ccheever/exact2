# Heavy list: GPUI on the Mac

@ref LLP 1149 §4; G1.

**G1 remains unqualified.** Both release apps build, and the functional checks
below pass. The five-round comparison was not run: other lanes kept building
on this shared Mac and the compiler preflight refused to launch the series.
The functional previews reported thermal state `fair`. No
performance conclusion is established; the timing tables remain unmeasured.

## Arms and provenance

- Native Exact: this checkout at `f41aaafec4`, `exact-heavylist`, built with
  `bun host/apple/build.mjs exact-heavylist-apple --release` and
  `EXACT_APP_DIR=$PWD/bench/heavy-list/exact-heavylist`.
- GPUI app and probe source: `a0f6fe8523` on `lane/gpui-arm`. The report and
  captures do not change either executable.
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

## Whole app

| Arm | Cold to complete screen | Fling process/main CPU | Peak footprint | Rest process/main CPU |
|---|---:|---:|---:|---:|
| Native Apple host | unavailable | unavailable | unavailable | unavailable |
| GPUI 0.2.2 | unavailable | unavailable | unavailable | unavailable |

| Arm | Fling interval p50/p95/p99 | Late frames/s | End-of-fling footprint |
|---|---:|---:|---:|
| Native Apple host | unavailable | unavailable | unavailable |
| GPUI 0.2.2 | unavailable | unavailable | unavailable |

There are **zero qualifying rounds**. The remaining requirement is five
interleaved rounds with both windows active, recorded stable thermals and no
builds on the machine. The driver rejects a lost-focus, non-scrolling or
thermal-transition run.
The functional previews are excluded from these tables.

## Renderer only

| Comparison | Status |
|---|---|
| GPUI renderer versus Ink headless paint, same heavy-list screen | Follow-up in the Ink lane; not measured by G1 |

## Method

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
with one-second samples retained as well as each phase’s aggregate. Both windows
are brought to the front; the driver rejects a run that loses focus or changes
thermal state. A stable `fair` state is recorded, not treated as nominal.
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

All required commands passed on this worktree:

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
xcrun clang -fobjc-arc -O2 -Wall -Wextra -Werror -framework Foundation \
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

The instrumented release copies and helper are already built in this worktree.
After reserving a quiet machine, the remaining series can be run directly:

```sh
target/g1/measure "$PWD/target/g1/series" \
  "$PWD/target/g1/Native.app/Contents/MacOS/Heavy list" \
  "$PWD/target/g1/GPUI.app/Contents/MacOS/gpui-heavy-list" \
  "$PWD/target/g1/exact-wheel"
```
