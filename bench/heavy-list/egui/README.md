# Heavy list: egui on the Mac

@ref LLP 1149 §4.1; G4a. This is benchmark apparatus, a separate Cargo workspace.
G4b uses the shared driver and probe; results belong in `../gpui/RESULTS.md`.

## Implementation

`egui` and `eframe` are pinned to **0.37.0**, `wgpu` to **30.0.1**, the latest
non-yanked stable releases in the crates.io registry on 2026-10-29
([egui](https://index.crates.io/eg/ui/egui),
[eframe](https://index.crates.io/ef/ra/eframe),
[wgpu](https://index.crates.io/wg/pu/wgpu)). `Cargo.lock` fixes the published
dependency graph. There are no patches or git dependencies.

`main.rs` reads GPUI's shared message model and the same 10,000-message JSON.
`ScrollArea::show_viewport` lays out visible rows, using `heights.rs` to locate
them in a prefix-sum tree. Unseen rows have a 330-point estimate; a visible row
replaces that estimate with its measured height. Resizing invalidates heights.
`row.rs` draws SPEC's text, quote, photo grids, link card and wrapping reaction
chips. Counts are keyed by message id and emoji. The list has GPUI's additional
16-point outer inset as well as SPEC's row padding.

`picture.rs` runs four decoding workers and caches textures by filename and
displayed pixel dimensions. `build.rs` compiles **the same**
`../gpui/src/thumbnail.m`: ImageIO downsamples and center-crops to aspect-fill
at the tile's pixel size. The returned BGRA buffer is converted to RGBA for
egui. A completion wakes the UI; there is no image polling timer. Before
eframe uploads textures and acquires its drawable, the app calls the shared
`g1_scene_complete`, which finds the optional `g1_ready` export through `dlsym`.
It marks a frame complete only when every image requested by that frame is loaded.

The backend is restricted to Metal, with wgpu's default vsynced presentation
and two-frame queue. The display supplies the native scale and refresh rate;
on the comparison Mac these are 2× and 120 Hz. No application frame timer runs.
Wheel and momentum phases come through winit and egui-winit's AppKit momentum
monitor. Egui kinetic decay, rubber-banding, programmatic scroll animation and
edge fades are disabled. Phase-bearing trackpad events bypass egui's discrete
mouse-wheel smoothing. The app requests repaints only after image completion
or a reaction click; eframe also responds to window, input and font events.
The live label is static and `BENCH_LIVE=1` is refused.

## Parity and limits

- Egui owns immediate-mode layout, virtualization, shaping, glyph caching,
  tessellation and wgpu presentation. There is no Exact kernel, runner or data
  source. These are whole-process arms, not isolated renderer measurements.
- SF Pro, SF Pro Italic and SF Mono are loaded from
  `/System/Library/Fonts/SFNS{,Italic,Mono}.ttf` when available. SF Pro uses
  `wght=400/600` for regular/semibold. Egui's system-font provider supplies
  fallback faces. A missing primary file is logged and leaves egui's bundled
  Ubuntu Light/Hack defaults. Font bytes are not bundled. Egui's HarfRust
  shaping and glyph rasterization differ from Core Text; metrics, fallback,
  emoji and script rendering need visual qualification in G4b. The top capture
  has different line breaks and smaller emoji than the retained GPUI capture.
- Inline code is SF Mono at SPEC's 15 points, rather than GPUI's Menlo at 16.
  Paragraphs remain one wrapping rich-text flow. Reaction digits use equal
  width cells in the same semibold font, since this text API exposes no
  OpenType `tnum` feature selector.
- Estimated variable heights, retention and scrollbar presentation differ
  from GPUI and AppKit. Equal wheel deltas do not establish equal offsets or
  equal layout work. Egui renders only rows intersecting the viewport; GPUI
  also premeasures an 800-point margin. Textures remain cached for the process
  lifetime. No native row accessibility labels are exposed by this painter.
- The top capture's photos appear more saturated than the retained GPUI
  capture despite using the same ImageIO decoder and data. Color presentation
  is not qualified as equivalent; G4b must inspect this alongside text parity.
- JSON parsing, font loading, workers and caches count toward the app. Native
  Exact's first rows are baked into its plan. The painted arms' drawable
  milestone differs from the native probe's settled layer-snapshot milestone.

## Build and bundle

From the repository root, using this worktree's `target/` only. Data must already
match `bench/heavy-list/data.sha256`; `../prepare.sh` prepares it when absent.

```sh
cargo build --release --locked --manifest-path bench/heavy-list/egui/Cargo.toml --target-dir target
mkdir -p target/g4/Egui.app/Contents/{MacOS,Resources,Frameworks}
cp target/release/egui-heavy-list target/g4/Egui.app/Contents/MacOS/
cp bench/heavy-list/egui/Info.plist target/g4/Egui.app/Contents/
cp -R bench/heavy-list/data target/g4/Egui.app/Contents/Resources/
xcrun clang -dynamiclib -fobjc-arc -O2 -Wall -Wextra -Werror -Wno-unused-parameter \
  -mmacosx-version-min=14.0 -framework AppKit -framework QuartzCore -framework Metal \
  bench/heavy-list/gpui/probe.m -o target/g4/Egui.app/Contents/Frameworks/probe.dylib \
  -install_name @executable_path/../Frameworks/probe.dylib
python3 bench/heavy-list/probe/linkprobe.py target/g4/Egui.app/Contents/MacOS/egui-heavy-list \
  @executable_path/../Frameworks/probe.dylib
codesign --force --deep --sign - target/g4/Egui.app
cargo clippy --manifest-path bench/heavy-list/egui/Cargo.toml --target-dir target -- -D warnings
cargo fmt --manifest-path bench/heavy-list/egui/Cargo.toml -- --check
git add -A
bun scripts/caps.mjs
```

The bundle's executable is `egui-heavy-list`, type `APPL`, minimum macOS 14.0.
It reads `BENCH_DATA` or `../Resources/data` relative to the executable,
`BENCH_START_INDEX` (0–9999), and `BENCH_LIVE`. The measurement driver's
`EXACT_WINDOW_WIDTH/HEIGHT` select the outer window, default 800×800 points;
the content height reserves 32 points for the title bar, as GPUI does.

The shared probe accepts `G1_ARM=egui`, preserves that name and records the
logical top item and within-item offset. `G1_OUTPUT` is an absolute JSON path.
Its `.ready` sidecar supplies the window number and complete-frame timing;
`G1_COLD_ONLY=1` makes SIGUSR2 after readiness write the diagnostic and exit.

## G4b readiness and instrument

`G4_EGUI_BIN` adds this executable to `gpui/measure.m` alongside
`G3_PAINTED_BIN` for Ink. Four arms rotate through five fling/rest rounds,
then five cold-only rounds; omitting `G4_EGUI_BIN` preserves the three-arm
commands. Egui receives `EXACT_WINDOW_HEIGHT=800`, as GPUI does: the verified
content is 800×768 and the outer window 800×800 at 2×. Ink receives 768 because
its window API adds the title bar outside that height.

`G4_DIAGNOSTICS=1` logs each frame, image request, decode, completion, texture
upload and worker repaint call to stderr, timed from main. It is off for the
series. The probe's `.ready` adds `windowAt`, `firstComplete`, `coldMs` and
`completeAfterWindowMs`; these timestamps distinguish presentation from the
sidecar's two-second observation delay.

G4a's retained `target/g4/functional.json` has **both** `firstDrawableMs` and
`coldMs` equal to 14,668.34 ms. No earlier placeholder drawable was recorded;
that file cannot establish a lost final-image wake. On the rebased source,
the initial instrumented launch completed 103.40 ms after window attachment.
Three following launches completed in **82.80, 36.10 and 41.29 ms** after
attachment (**303.76, 352.24 and 369.45 ms** after process start). All workers
sent their completion and called `request_repaint`; subsequent frames uploaded
the results and marked the scene complete. Evidence is retained under
`target/g4b/readiness-{before,1,2,3}.json`, their `.ready` files and logs.

The original delay did not reproduce. Its cause remains unestablished; neither
a lost wake nor first-use driver work is proven. No scheduling workaround or
polling timer is added. These brief launches posted no input and are diagnostic
runs, excluded from the cold-start series.

G4b preparation at `994b003388`, rebased onto `0312b493fc`, built all four
release arms into `target/g4b/`. The signed bundle copies carry identical
probe bytes; `provenance.json` records their executable and probe hashes,
and `plan.txt` records the commands and quiet rule. The original three-arm
aggregation output matched byte for byte on controlled samples; the same
exercise produced all four columns with egui enabled. These are instrument
checks, not benchmark results.

All six repository checks passed, including clippy and fmt, as did the egui
workspace's clippy (`-D warnings`) and fmt. No check needed a retry. The root
build/test/check commands used this worktree's `target/`; tests used
`RUST_TEST_THREADS=2`. Exact commands, exit codes and logs are retained in
`target/g4b/checks/`. Timed samples remain pending the display window.
