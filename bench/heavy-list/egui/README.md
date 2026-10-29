# Heavy list: egui on the Mac

@ref LLP 1149 §4.1; G4a. This is benchmark apparatus, a separate Cargo workspace.
G4b supplies the four-arm series; there are no performance results here.

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

The unchanged G1 probe selects its drawable hook only for `G1_ARM=gpui`.
Use that alias for G4a's functional check, with an absolute `G1_OUTPUT`; it
will label the output `gpui`. This is not an egui series result. Its `.ready`
sidecar names the window for `screencapture -x -o -l <window> <file>`.
`G1_COLD_ONLY=1` makes SIGUSR2 after readiness write the cold diagnostic and
exit. Without it, three SIGUSR2 signals advance through fling, rest and exit;
advancing them without input or a timed dwell only exercises probe reporting.
Do not interpret those phase counters as measurements.

## G4b handoff

Add the egui executable to `gpui/measure.m`'s arm-to-executable mapping and
four-arm rotation, retaining the same launch environment, focus checks,
wheel helper, cold-only mode and per-arm filenames. The ink lane's shared
probe must recognize `egui` as a painted arm, preserve that name in output,
and report its logical top item/within-item offset from `g1_ready` rather
than looking for an `NSScrollView`. Apply the painted-arm movement check to
egui too. Keep drawable presentation separate from native display-link cadence.
Use `../gpui/g4-egui-top.png` for the top-of-list parity inspection before
interpreting the four-arm series. Neither driver nor probe is changed by G4a.
