# Extra Heavy feed benchmark — macOS

The iOS benchmark (`../SPEC.md`: rows, data, clocks, freeze, scenarios, Decisions) run on macOS:
the same 3,000-row, 19-kind feed from the same `../data/` (seed 49975), a SwiftUI app ported to macOS, and the
exact2 app `../exact-xheavy/`, **unchanged**, built for macOS. Everything not said here is the iOS SPEC's.
The web pair is `../web/` (its README).

| path | what |
|---|---|
| `swiftui/` | the SwiftUI app ported to macOS (`App.swift`, `Kinds.swift`, `Media.swift`, `Nested.swift`, `Shader.metal`); `build.sh` (needs the Metal Toolchain) → `build/XHeavy.app`, `dev.exact.xheavy.swiftui` |
| `build-exact.sh` | `../exact-xheavy` built for macOS (`host/apple/build.mjs exact-xheavy-apple --bundle`) → `dist/<label>.app`, `dev.exact.xheavy.exact2`. The series below were built at origin/main **558a5bd8** (2026-09-28) and later commits named with each |
| `push.sh` | stages an app, the probe and the runner scripts on the bench Mac (`BENCH_STAGE`, default `~/xhm`; over ssh with `BENCH_HOST` or host arguments) |
| `probe/` | `probe.m` → `probe-mac.dylib` (`build.sh`): the heavy-list iOS probe ported to AppKit |
| `run/run.sh`, `run/series.sh`, `run/series-m2.sh` | the runner (one probe run) and the interleaved 3-round series (`series-m2.sh`: order alternated per round, provenance, retries), run on the bench Mac from the stage |
| `../summarize.py` | the iOS summary, whose ladder threshold scales with the display (110/120 × maxFps: 55 at 60 Hz) |
| `check-m2.py`, `abtable.py`, `byspeed.py`, `kinds.py` | is every result a whole run (run it first); an A/B as medians with every round; fling by speed; the per-kind table |
| `run/trace.sh`, `run/trace2.sh`, `run/tp/` | Time Profiler of one run (main-thread aggregates; every thread, read by `tpx.py`) |

The original harness's results, shots and repros are not in the repository; this record quotes their summaries.
The machines were "bones" (a Mac mini M4 on a 60 Hz 1x display) and "silver" (a MacBook Pro M5 Max, 120 Hz).

## Machines and their limits

| | bones | silver |
|---|---|---|
| machine | Mac mini M4 (Mac16,10), 10 cores, 16 GB | MacBook Pro M5 Max (Charlie's laptop), on AC |
| OS / Xcode | macOS 27.0 (26A428), Xcode 27.0 | macOS 26.6.2, Xcode 27 (`DEVELOPER_DIR` set; CLT is the active dir) |
| display | one external 1920 × 1080 at **60 Hz, 1x** | built-in Liquid Retina XDR 3456 × 2234 (1728 × 1117 pt, **2x**), ProMotion, the probe's display link measured **120 Hz** (8.33 ms) |
| disk | 33 GB free: the bench uses ~50 MB in `~/xhm` | 3 TB free; everything in `~/xhm` |
| state | console user logged in, **screen locked**, display sleeps after 10 min | same (locked), `caffeinate` kept by the coordinator (PID 21351) |

- **Build elsewhere.** Neither machine builds: both apps are built on the M5 mini (macOS 27 SDK, deployment
  target 14.0, arm64, ad-hoc signed, no hardened runtime so dyld honours `DYLD_INSERT_LIBRARIES`) and copied.
- **60 Hz, 1x on bones.** Its fps and late frames are against a 16.7 ms budget; never compare them with the iPad's
  120 Hz. Silver is the 120 Hz number and the more important one (exact2's iOS losses showed only at the 120 Hz budget).
- **Locked screen.** Both machines sit at the lock screen, and we cannot unlock them. With the display awake
  (the runner asserts user activity with `caffeinate -u -t 2`, the probe holds a display-sleep assertion), windows
  still composite and each app's window reports itself visible, key and active (`window` in every JSON), the display
  link ticks at the panel's rate, and the window server returns the app's own window pixels. A sleeping display
  ticks no display link at all (found on bones: the first runs hung until the display was woken).
- **No screen recording over ssh.** `screencapture` fails ("could not create image from display"). A process may
  capture **its own** windows without the permission: the probe's `window` sampler uses
  `CGWindowListCreateImage(…, kCGWindowListOptionIncludingWindow, own window number)` (looked up with `dlsym`:
  the macOS 15+ SDK marks it unavailable; the function is still exported on 26 and 27). This is the compositor's
  real output: Metal (shader, maps), AVPlayerLayer video and WKWebView included.
- **No Xcode license on bones** (`xcrun` tools and `/usr/bin/python3` refuse; accepting needs sudo). The runner is
  bash; `xctrace` works when called directly (`/Applications/Xcode.app/Contents/Developer/usr/bin/xctrace`); the
  CLT's `/Library/Developer/CommandLineTools/usr/bin/python3` works.
- **Background load.** bones' `TGOnDeviceInferenceProviderService` (Apple Intelligence) used ~25 % of a core the
  whole time (load average ≈ 2 of 10 cores); nothing else. Silver: load ≈ 1.9 at rest.
- Launch goes through Launch Services (`open -n -F --env …`), so the app runs in the GUI session whatever the ssh
  session; the runner reads the new PID once by the app's unique path under `~/xhm`, and kills only that PID if the
  probe has not exited by itself (`BENCH_EXIT=1`).

## The window

Both apps get the same **window frame, 1366 × 940 pt** at the top-left of the screen's visible area (1366 is the
landscape iPad's width; 940 fits bones' 1080 p screen), set by the probe as soon as the window exists (both apps
also ask for it: SwiftUI `.defaultSize`, exact2 `EXACT_WINDOW_WIDTH/HEIGHT`). The list's viewport (in each JSON as
`viewport`):

| | bones (legacy scrollers: a mouse is attached) | silver (overlay scrollers) |
|---|---|---|
| SwiftUI | 1349 × 867 pt (the `List`'s always-visible 17 pt scroller takes the right edge) | 1366 × 867.5 |
| exact2 | 1366 × 872 | 1366 × 871.5 |

The content column is C = 600 in both. The top bar sits in each app's content (exact2's window has a full-size
content view with a transparent title bar; SwiftUI's hosting window too).

## The macOS probe (`probe/probe.m`)

The iOS probe's scenarios, segments and JSON keys, so `summarize.py` / `agg.py` read both. What changed:
- **Scroll view.** The `NSScrollView` in the window with the tallest document (> 20,000 pt), driven by
  `-[NSClipView scrollToPoint:]` + `-reflectScrolledClipView:` each display-link frame. SwiftUI's `List` is
  `SwiftUI.ListCoreScrollView` over an `NSTableView` (`SwiftUIOutlineListView`); exact2's is its `ChainingScrollView`
  over a flipped document view. Inner lists are found by the iOS shape rule among the feed's descendant scroll
  views (SwiftUI `HostingScrollView`, exact2 `ChainingScrollView`).
- **Content height is each list's own.** SwiftUI's `List` on macOS reports an *estimated* document height
  (≈ 180,000–260,000 pt for a feed that is ≈ 1.4 M pt laid out; it changes as rows are measured); exact2's is
  1.39–1.41 M pt. Fling starts at 1/3 of it and jump targets are fractions of it, so the same scenario covers
  different rows in the two apps; travelled points and speeds are the same.
- **Frames.** `-[NSView displayLinkWithTarget:selector:]` (macOS 14) on the window's content view, preferred
  rate = the screen's `maximumFramesPerSecond`. `dts` = display-link timestamps, `expected` = target − timestamp.
  As on iOS, this times the main thread's frame callbacks, not the compositor's presents.
- **Busy, CPU, main CPU, memory** unchanged (run-loop observers, `getrusage`, `thread_info`, `phys_footprint`).
- **Blank frames.** `BENCH_RENDER=window` (above) for `fling-b`, `jump`, `coldstart`, `innerfling-b`. The iOS
  `layer` sampler (`renderInContext`) was tried first and **misses exact2's text on macOS** (header names and thread
  text were absent from the sample, present on screen), so it would count exact2 frames blank that are not; it is
  kept as `BENCH_RENDER=layer` but not used. The window capture costs ~17–18 ms per sample on bones (the iOS
  layer sampler cost ~4 ms), so sampled runs are perturbed more than on iOS, equally for both apps.
- **Coldstart.** The display link needs a view, so coldstart polls every 2 ms until the window exists and then
  samples every frame; `ms` = process start → first display-link frame with the list present and blank-free.
- **`shot`** (new): a full-scale window capture to `BENCH_OUT` (the parity shots).

## macOS equivalences (the SwiftUI port)

Rule (Charlie's): each kind uses its natural macOS equivalent with content and work equal to exact2's; exact2 may
do anything SwiftUI's app does, nothing that makes the benchmark invalid. The port is the iOS app with UIKit views
swapped for their AppKit twins; everything SwiftUI has on both platforms is unchanged.

| kind | iOS SwiftUI | macOS SwiftUI | exact2 (macOS host, unchanged app) |
|---|---|---|---|
| list | `List(.plain)` + `ForEach` | same (AppKit `NSTableView` underneath); `.listRowSeparator(.visible)` added (macOS plain lists hide separators by default) | `list virtualized=true` in `ChainingScrollView` |
| images | ImageIO thumbnail → `UIImage` → `Image(uiImage:)` | same decode → `NSImage` → `Image(nsImage:)` | host ImageIO decode |
| photo, thumbs (shimmer), fonts, markdown, code, intl, typeface, live, carousel, glass material | SwiftUI views | **unchanged** (`TimelineView`, `LinearGradient`, `.ultraThinMaterial`, `AttributedString`, `Font.custom`; fonts via `ATSApplicationFontsPath` instead of `UIAppFonts`) | as on iOS |
| shader | `.layerEffect` + `[[stitchable]]` Metal in `TimelineView(.animation)` | same, `default.metallib` compiled for macOS 14 | GPU module (WGSL, Metal) |
| canvas | SwiftUI `Canvas` | same | Canvas 2D (Rust) |
| svg | asset-catalog vector SVGs, `Image("…")` | same (`actool --platform macosx`) | inline SVG components |
| video | `AVQueuePlayer` + `AVPlayerLooper` in a `UIView` whose layer is `AVPlayerLayer` | same player in an `NSView` whose backing layer is an `AVPlayerLayer` (`NSViewRepresentable`), play in `viewDidMoveToWindow` | `video` (AVPlayerLayer) |
| map | MapKit for SwiftUI `Map(initialPosition:interactionModes: [])` + `Marker` | same (MapKit for SwiftUI exists on macOS 14; `MKMapView` underneath) | `native-map` module (`MKMapView`, reused across rows) |
| GIF / WebP | `UIImageView` fed by `CGAnimateImageAtURLWithBlock` | `NSImageView` fed by the same ImageIO call (`animates = false`: AppKit's own GIF animation not used, so GIF and WebP take one path, as on iOS) | `image` (animated natively) |
| Lottie | `lottie-ios` `LottieView` | same library compiled for macOS | hand-ported SVG + `@keyframes` (declared iOS difference, unchanged) |
| web view | `WKWebView`, `scrollView.isScrollEnabled = false`, `isOpaque = false`, `loadHTMLString(_, baseURL: nil)` | `WKWebView` subclass that hands `scrollWheel` to the next responder (AppKit's WKWebView has no `scrollView`), `drawsBackground = false`, same load | `iframe` (WKWebView) |
| text input | `TextField` | `TextField` + `.textFieldStyle(.plain)` (iOS's default field has no bezel; AppKit's does) | `input` |
| filmstrip / inbox | `ScrollView` + `LazyHStack` / `LazyVStack`; offset kept with `ScrollPosition` (iOS 18) | same; `#available(macOS 15)` (both machines run 26+, so offsets are kept) | nested virtualized lists (LLP 1070) |
| window | — | `.defaultSize(1366, 940)`, `.defaultPosition(.topLeading)` (the probe then sets the frame) | `EXACT_WINDOW_WIDTH/HEIGHT` |

Parity (`shots/bones/pair-*.png`, frozen): the rows match row for row (same wrapping, fonts, sizes, maps with the
same tiles and marker). Differences: SwiftUI's `List` shows its legacy scroller on bones and indents row separators
by 8 pt; exact2's separator is full width. **exact2 draws no rating capsule on the glass row** (a macOS host bug,
below), so exact2's glass rows are one material view lighter than SwiftUI's.

## exact2 on macOS: no Contract change

The app builds and runs on macOS as written for iOS; nothing in `../exact-xheavy/` changed (`app.json` already declares
`host.macos.minimumOS 14.0`). All 19 kinds render (the map module has a macOS path).

## exact2 bugs found

1. **macOS host: a clipping `border-radius` larger than half the box is not reduced (CSS reduces it), so the box
   draws nothing.** The glass row's capsule (`border-radius=100` on a ≈ 56 × 28 box) is invisible, text included; a
   wide box draws a lens. Repro: `repro-capsule/` (six variants; only `border-radius=14` renders). Responsible code
   (read, not edited): `host/apple/Sources/ExactKit/Mac/NodeViewMac.swift` ~1036 (`layer.cornerRadius` = the authored
   value when clipping) and ~559 (the material view's corner radius); iOS reduces first
   (`IOS/BoxLayerIOS.swift:136`, `cornerRadii(in:)`), and `BorderPaint.reduced` exists.
2. **macOS: `renderInContext` of an exact2 window omits much of its text** (header names, thread text), while other
   AppKit/SwiftUI content renders. Not a user-visible bug, but any layer-based capture of an exact2 macOS window
   (tests, snapshots) sees missing text. (Found by the probe; not reduced to a repro.)

## Series (2026-09-28/29): 3 rounds, interleaved, origin/main 558a5bd8

Scenarios as the iOS series (`run/series.sh`): fling-t, fling-live (`BENCH_LIVE=1`), rest, ladder-t, jump and
coldstart (window sampler, every frame), fling-b (window sampler every 4th frame), innerfling-t/-b, innerkeep;
BENCH_DELAY 15 s; the 19-kind feed. Medians over the 3 rounds (`summarize.py`; "blanks" sums fling-b's sampled
frames with a blank band over the 3 rounds; "ladder" is each round's first speed under 110/120 of the refresh rate).
Per kind: the feed filtered to one kind (`BENCH_KINDS=<kind>`, 2,250 rows), fling-t + ladder-t, 3 rounds.

### silver — 120 Hz (the budget that matters)

| app | fps | live fps | late/s | worst ms | busy/f | cpu ms/s | main ms/s | peak MB | end MB | blanks | ladder <110 | jump p50 | cold ms | rest cpu | rest main |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| swiftui | 109.0 | 107.9 | 7.2 | 58 | 3.97 | 569 | 351 | 1468 | 1437 | 0 | 12000,12000,-3000 | 34 | 737 | 413 | 367 |
| exact2 | 104.8 | 104.9 | 10.0 | 50 | 8.12 | 1161 | 713 | 1506 | 1480 | 40 | 6000,6000,12000 | 81 | 238 | 77 | 72 |

| fling fps by speed | 1000 | -1000 | 3000 | -3000 | 6000 | -6000 | 12000 | -12000 | 24000 | -24000 |
|---|---|---|---|---|---|---|---|---|---|---|
| swiftui | 116.2 | 117.0 | 110.2 | 111.5 | 109.2 | 109.5 | 107.5 | 110.5 | 98.5 | 105.0 |
| exact2 | 118.0 | 117.5 | 116.5 | 113.8 | 109.2 | 108.8 | 101.4 | 102.8 | 76.5 | 74.4 |

| innerfling filmstrip fps | 3000 | -3000 | 6000 | -6000 | 12000 | -12000 | 24000 | -24000 | 48000 | -48000 | 96000 | -96000 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| swiftui | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 |
| exact2 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 |

| innerfling inbox fps | 3000 | -3000 | 6000 | -6000 | 12000 | -12000 | 24000 | -24000 | 48000 | -48000 | 96000 | -96000 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| swiftui | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 119.5 | 118.3 | 118.5 |
| exact2 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 120.0 | 111.5 | 120.0 | 99.0 | 101.0 | 97.0 | 98.0 |

| app | inner busy/f | inner cpu ms/s | inner main ms/s | inner blanks (innerfling 4) | innerkeep kept | innerkeep errors pt |
|---|---|---|---|---|---|---|
| swiftui | 3.41 | 447 | 405 | 12 | 12/12 | - |
| exact2 | 5.35 | 625 | 595 | 38 | 7/12 | -34,-26,-8,-9,-18 |

### silver per kind

| kind | app | fps | late/s | fps ±24k | busy ms/f | main ms/s | cpu ms/s | peak MB | ladder break (3 rounds) |
|---|---|---|---|---|---|---|---|---|---|
| canvas | swiftui | 116.9 | 2.3 | 114.6 | 2.97 | 337 | 353 | 401 | 96000,96000,6000 |
| canvas | exact2 | 120.0 | 0.0 | 120.0 | 4.54 | 424 | 639 | 579 | 96000,96000,96000 |
| carousel | swiftui | 108.4 | 9.0 | 119.0 | 3.69 | 392 | 400 | 112 | 3000,3000,3000 |
| carousel | exact2 | 92.1 | 19.6 | 51.3 | 10.79 | 884 | 1058 | 523 | 3000,6000,-3000 |
| live | swiftui | 106.8 | 10.8 | 113.0 | 4.71 | 494 | 499 | 114 | 3000,3000,3000 |
| live | exact2 | 65.6 | 28.7 | 36.2 | 15.23 | 945 | 1020 | 463 | 3000,3000,3000 |
| map | swiftui | 76.9 | 17.6 | 12.8 | 10.87 | 639 | 2542 | 6483 | 3000,3000,3000 |
| map | exact2 | 73.7 | 19.3 | 11.9 | 12.66 | 803 | 3104 | 5342 | 3000,3000,3000 |
| shader | swiftui | 108.4 | 10.5 | 107.8 | 4.20 | 434 | 497 | 1085 | 6000,6000,6000 |
| shader | exact2 | 120.0 | 0.0 | 120.0 | 4.32 | 367 | 589 | 745 | -,-,- |
| svg | swiftui | 117.4 | 2.1 | 115.8 | 2.01 | 235 | 241 | 226 | -,-,- |
| svg | exact2 | 120.0 | 0.0 | 120.0 | 4.17 | 342 | 517 | 540 | -,-,- |
| video | swiftui | 113.3 | 5.2 | 102.5 | 2.91 | 277 | 757 | 120 | 3000,24000,24000 |
| video | exact2 | 114.7 | 5.2 | 90.6 | 5.27 | 487 | 931 | 522 | 24000,24000,24000 |
| webview | swiftui | 70.7 | 13.5 | 7.7 | 7.68 | 427 | 563 | 106 | 3000,3000,3000 |
| webview | exact2 | 119.3 | 0.7 | 116.3 | 4.61 | 398 | 718 | 524 | 48000,48000,48000 |

### bones — 60 Hz, 1x

| app | fps | live fps | late/s | worst ms | busy/f | cpu ms/s | main ms/s | peak MB | end MB | blanks | ladder <110 | jump p50 | cold ms | rest cpu | rest main |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| swiftui | 55.7 | 55.2 | 3.7 | 100 | 6.84 | 562 | 349 | 941 | 918 | 0 | 6000,6000,12000 | 32 | 583 | 402 | 328 |
| exact2 | 56.7 | 56.7 | 2.3 | 69 | 9.95 | 892 | 533 | 1328 | 1327 | 18 | 24000,24000,24000 | 39 | 273 | 201 | 118 |

| fling fps by speed | 1000 | -1000 | 3000 | -3000 | 6000 | -6000 | 12000 | -12000 | 24000 | -24000 |
|---|---|---|---|---|---|---|---|---|---|---|
| swiftui | 58.5 | 57.5 | 57.0 | 57.1 | 55.1 | 57.5 | 53.5 | 57.1 | 50.6 | 52.6 |
| exact2 | 60.0 | 60.0 | 60.0 | 60.0 | 59.3 | 59.8 | 59.0 | 58.5 | 41.9 | 43.2 |

| innerfling filmstrip fps | 3000 | -3000 | 6000 | -6000 | 12000 | -12000 | 24000 | -24000 | 48000 | -48000 | 96000 | -96000 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| swiftui | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 |
| exact2 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 |

| innerfling inbox fps | 3000 | -3000 | 6000 | -6000 | 12000 | -12000 | 24000 | -24000 | 48000 | -48000 | 96000 | -96000 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| swiftui | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 59.5 | 59.5 | 59.5 |
| exact2 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 60.0 | 59.9 | 55.5 | 59.5 | 53.1 |

| app | inner busy/f | inner cpu ms/s | inner main ms/s | inner blanks (innerfling 4) | innerkeep kept | innerkeep errors pt |
|---|---|---|---|---|---|---|
| swiftui | 5.96 | 407 | 353 | 0 | 12/12 | - |
| exact2 | 7.84 | 499 | 466 | 23 | 6/12 | -24,-8,-16,-16,-24,-16 |

### bones per kind

| kind | app | fps | late/s | fps ±24k | busy ms/f | main ms/s | cpu ms/s | peak MB | ladder break (3 rounds) |
|---|---|---|---|---|---|---|---|---|---|
| canvas | swiftui | 59.6 | 0.4 | 59.0 | 4.87 | 286 | 300 | 134 | -,-,- |
| canvas | exact2 | 60.0 | 0.0 | 60.0 | 5.50 | 324 | 384 | 158 | -,-,- |
| carousel | swiftui | 59.8 | 0.2 | 59.8 | 6.21 | 354 | 359 | 89 | 96000,96000,96000 |
| carousel | exact2 | 59.6 | 0.3 | 57.8 | 9.74 | 579 | 611 | 162 | 24000,48000,48000 |
| live | swiftui | 59.2 | 0.8 | 57.5 | 6.75 | 400 | 402 | 91 | 48000,48000,48000 |
| live | exact2 | 40.1 | 14.4 | 18.6 | 23.83 | 953 | 974 | 162 | 3000,3000,3000 |
| map | swiftui | 32.6 | 13.4 | 10.7 | 22.35 | 633 | 2763 | 6164 | 3000,3000,3000 |
| map | exact2 | 43.5 | 6.1 | 12.3 | 20.24 | 805 | 3599 | 4420 | 3000,3000,3000 |
| shader | swiftui | 59.6 | 0.4 | 58.8 | 6.11 | 348 | 396 | 400 | -,-,- |
| shader | exact2 | 60.0 | 0.0 | 60.0 | 6.34 | 312 | 514 | 528 | -,-,- |
| svg | swiftui | 59.8 | 0.2 | 59.0 | 3.83 | 230 | 234 | 102 | -,-,- |
| svg | exact2 | 60.0 | 0.0 | 60.0 | 5.31 | 295 | 339 | 138 | -,-,- |
| video | swiftui | 56.0 | 3.4 | 53.6 | 5.20 | 250 | 810 | 165 | 3000,3000,3000 |
| video | exact2 | 55.4 | 3.1 | 32.6 | 8.89 | 447 | 821 | 193 | 24000,24000,24000 |
| webview | swiftui | 39.2 | 4.0 | 4.4 | 15.71 | 435 | 532 | 83 | 6000,6000,6000 |
| webview | exact2 | 44.1 | 5.2 | 8.7 | 16.44 | 576 | 736 | 143 | 12000,12000,12000 |

### Web pair, bones, Chrome 154, 60 Hz (exact2 web vs Expo web; SwiftUI has no web)

| app | fps | live fps | late/s | worst ms | busy/f | cpu ms/s | main ms/s | peak MB | end MB | blanks | ladder <110 | jump p50 | cold ms | rest cpu | rest main |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| exact2 | 57.8 | nan | 0.1 | 35 | nan | 545 | 351 | 888 | 888 | 0 | None,None,None | 17 | 187 | 364 | 158 |
| expo | 57.0 | nan | 1.0 | 50 | nan | 583 | 312 | 964 | 962 | 9 | 48000,48000,48000 | 30 | 212 | 546 | 214 |

| fling fps by speed | 1000 | -1000 | 3000 | -3000 | 6000 | -6000 | 12000 | -12000 | 24000 | -24000 |
|---|---|---|---|---|---|---|---|---|---|---|
| exact2 | 58.7 | 59.0 | 58.8 | 59.5 | 58.2 | 59.0 | 57.8 | 58.7 | 58.7 | 58.8 |
| expo | 58.9 | 57.8 | 58.6 | 58.5 | 58.2 | 57.4 | 58.1 | 57.2 | 55.5 | 55.9 |


- "cpu" = the whole Chrome tree's CPU (`ps`), "main" = the page's renderer process; busy/frame is not measurable
  from a page. "peak MB" = renderer + GPU process RSS. Chrome's own per-page measurement
  (`measureUserAgentSpecificMemory`, cross-origin isolated): **exact2 82 MB vs Expo 153 MB**; JS heap 9 vs 51 MB.
- Blanks on the web are a DOM check (`elementFromPoint` down the column): missing rows, not unpainted ones.
- The 1 Hz live tick is off in both (exact2's web build reads no `BENCH_*` environment), so there is no fling-live
  and no per-kind web breakdown.
- Substitutions (Expo web): react-native-maps → OpenStreetMap tiles + a pin (exact2's web `native-map` module, from
  exact2's `apps/map-demo`, draws the same OSM tiles); react-native-webview → `<iframe srcdoc>`; Skia → CanvasKit
  (`LoadSkiaWeb`), the canvas title in bundled Inter (no system fonts in CanvasKit); lottie-react-native →
  `@lottiefiles/dotlottie-react`; `LayoutAnimation` is a no-op on react-native-web (the thread's height change does
  not animate). LegendList 3.4.0 web bug: a horizontal list given `initialScrollOffset={0}` with `dataKey` stays at
  opacity 0; the filmstrip passes no offset when none is saved.
- exact2 web app changes (a separate copy then; now folded into `../exact-xheavy/`, inert on Apple): a `web/` crate, the GPU crate's web deps (wasm-bindgen
  0.2.127, exact2's pin), the feed parsed on first use and the shader photo decoded inline (the browser build has
  no threads: the thread spawn trapped with "unreachable"), and the web `native-map` module. All 19 kinds render;
  the shader runs on WebGPU; the glass capsule **does** render on the web (the bug below is macOS-only).

## Where exact2 loses, and why

On **silver (120 Hz)** exact2 loses the whole-feed fling (104.8 vs 109.0 fps, 10.0 vs 7.2 late frames/s), the fast
speeds (±24k: 75 vs 98–105 fps), the ladder (breaks at 6k–12k vs 12k), main-thread cost (busy 8.1 vs 4.0 ms/frame,
main 713 vs 351 ms/s, CPU 1161 vs 569 ms/s), blanks (40 vs 0 sampled frames), jump (81 vs 34 ms) and the inbox's
fast inner fling (97–101 vs 118–120 fps); memory is a tie (1.5 GB each, most of it map tiles). It wins cold start
(238 vs 737 ms) and rest (77 vs 413 ms/s CPU). Per kind it loses **live** (65.6 vs 106.8 fps, the biggest loss),
**carousel** (92.1 vs 108.4, ±24k 51 vs 119), map (73.7 vs 76.9) and video at ±24k; it wins shader (120 vs 108),
webview (119.3 vs 70.7), canvas, svg. On **bones (60 Hz)** the whole-feed fps is a slight win (56.7 vs 55.7, fewer late
frames) but the same shape of losses: ±24k (42 vs 51), busy 9.95 vs 6.84 ms/frame, CPU 892 vs 562, peak memory
1,328 vs 941 MB, blanks 18 vs 0, and per kind live (40.1 vs 59.2) and carousel's ladder.

**Trace (Time Profiler, main thread, `results/traces/`).** In exact2's fling (bones and silver alike) the largest
blocks are Core Animation's commit **drawing layer contents on the main thread**: `CA::Layer::display_if_needed` is
36 % of main-thread samples on silver (`-[NSViewBackingLayer display]` 20 %, `NodeView.draw(_:)` 7 %, the rest
AppKit's backing-store setup), then row realization (`CollectionHost.flush`/`fillSlice` 22–27 %, the runner's
`collection_feedback` 6 %), `Presenter.refreshVisibleText` 7 %, AppKit bookkeeping iOS does not pay (tracking-area
rebuilds under `NSScrollView` 4 %, key-view loop 2 %, accessibility 2–3 %) and new `MKMapView`s (2 %). In the
`live`-only feed, layer display is **56 %** of main-thread time and `NodeView.draw` 16.5 % (`roundedPath` 4 %,
colour and `NSBezierPath fill`), plus 6 % applying per-frame batches (`Frames.tick` → `ExactSession.apply`).

Why: the macOS host paints any node with a background, border or gradient through `draw(_:)`
(`NodeViewMac.swift:942`, `wantsUpdateLayer` is false when `hasBoxPaint`), so every such box — the live row's 48
waveform bars and three ring tracks, cards, pills, carousel cards, thumbnails' cells — gets a CPU backing store
painted on the main thread at commit, and repainted whenever an animation or a new row touches it. The iOS host
says those boxes with layer properties and keeps no bitmap (`IOS/BoxLayerIOS.swift`, whose header names this macOS
split). SwiftUI on macOS pays layout (`NSHostingView.layout`, 27 %) but little drawing. Porting `BoxLayerIOS` to
the macOS host is the change the traces point at.

**Inner offsets.** exact2 kept the filmstrip's marks exactly (as on iOS) but the inbox's came back 8–34 pt short in
5–6 of 6 inbox checks per machine (iPad: 12/12 kept). The inbox's offset already moves in the 0.8 s after the probe
sets it (`markRead` 5,448 for a 5,432 mark): the list re-anchors as estimated rows are measured, and the restore
returns to the anchor, not the pixel offset. Not reduced to a repro; macOS-only as far as measured.

**Fairness notes.** exact2 draws no glass capsule on macOS (bug 1): one material view less per glass row than
SwiftUI. exact2's web view kind at ±24k holds 116 fps on silver where SwiftUI's drops to 8: per-kind runs sample no
blanks, so whether exact2's iframes are fully painted at that speed is not measured.

## The macOS host fix lane (2026-09-29, branch perf/macos-boxes, off dd09a300)

Before (`exact2base`, dd09a300) / after (`exact2mbF`, fd664f5e) / SwiftUI, silver 120 Hz, 3 rounds interleaved
(`run/series-mb.sh`, `results/mb-silver-r2`; bones 60 Hz in `results/mb-bones-r1` with an earlier build; traces in
`results/traces/bones-mb*`). Commits: 63f1d023 radius reduced; a9d10ee8 boxes as layers; 4e5feb1e accessibility
by touched nodes; 9e0ceaaa lazy key-view loop; 2e88c891 + 116dec960 + 5ec1f9a42 the paint decision kept and cheap;
22a58bd03 navigation gates only for gating ops; fd664f5e accessibility props written only when changed.

### The window sampler's blank bands were partly an artefact (2026-09-29)

The `window` sampler drew the window server's picture into a quarter-scale bitmap with the default
interpolation, which picks pixels: a 1 px border lying between sampled columns vanished, and a pale box bounded
only by it (exact2's web view card, `#EEFBFB` under the bars, luminance 247 against the page's 255) read as a
uniform, "blank" band. Which columns are sampled depends on each app's viewport (SwiftUI's is 1349 wide on bones,
exact2's 1366), so the count was geometry luck: exact2 37–42 against SwiftUI 0. Full-scale pictures taken right
after each flagged frame showed no missing content except web views still loading. The sampler now averages
(`kCGInterpolationHigh`), as the iOS probe's quarter-scale layer render fades a hairline instead of dropping it.
With it, both apps show blank bands, almost all web view rows not yet loaded: bones, fling 4, two runs each,
SwiftUI 8 and 11, exact2 (60fcfd769) 8 and 4. Earlier `blanks` columns (series before this date) used the old
sampler and are not comparable.

## 2026-09-30, the mac lane: what changed in the harness, and two things about the evidence

- **Every silver series (09-28/29) ran occluded.** All 252 result files of silver-r1, mb-silver-r1/r2 and
  next-silver-r1/r2 record `window: visible 0, key false, appActive false` (macOS 26's lock screen occludes app
  windows; bones on 27 and the M1 report visible/key/active). Both apps were occluded alike, so those tables compare
  two windows nobody could see: exact2 stops its GPU canvases on occlusion, MapKit/video/SwiftUI throttle theirs.
  The "Machines and their limits" line above saying each window reports visible, key and active holds for bones
  only. `check-m2.py` refuses such a result (cold starts excepted: exact2's first frame on bones lands before AppKit
  delivers the occlusion state).
- **The M1 MacBook Pro 14 is not a measurement machine.** Its CPU speed swings 3-5x within minutes
  (other jobs' bursts, load average up to 218; battery 1 % "Service Recommended", not charging): exact2 starved
  there reads 53-55 fps at 1000 pt/s, healthy 118. `run/mstate/speed.py` reads the state; `ms-run.sh` records it.
- **The machine-state gate.** `run.sh` waits for the machine's floor (`machine.state` in the stage, bones MIN1=760
  MIN4=730 = 90 % of idle) before each launch and writes `<result>.state` (before/after, load); `check-m2.py`
  refuses a result below the floor before or after (`--ungated` for older series, `--loaded` for a deliberately
  loaded machine, `STATE_GATE=off`). `run/mstate/spinners.sh start N | stop` makes the loaded state.
- **A run that was not a run.** The lock screen turns a display woken by a 2 s user-activity assertion off again
  12 s later, whatever display-sleep assertion the probe holds; the app then ticks no display link and the result
  is a 2 s `partial` that summarize.py would read as a fling. `run.sh` holds `caffeinate -u` for the run and exits 3
  on a partial; `series-m2.sh` retries once; `check-m2.py` refuses partial, short and missing results.
- **A/B protocol.** `series-m2.sh` alternates the app order each round (A B, B A, A B; `ORDER=fixed` keeps it) and
  writes the order per round to series.log and provenance.txt; `apps/<app>.env` passes an app's own switches
  (one build measured two ways) and is copied into provenance.txt; `abtable.py` prints medians with every round.
- **Map work counted.** `PROBE=probe-mac-mapwork.dylib BENCH_MAPCOUNT=1` counts MKMapViews made and places given
  (`mapWork` per segment): the macOS host made 31 to SwiftUI's 22 over one fling before the heavy-leaf hold.
- **Traces of every thread.** `run/trace2.sh <app> <label> [scenario]` → `tmp/tp-<label>.json.gz` (`tpdump.py`, with
  each sample's core); `run/tp/tpx.py` reads it: `segs` places the probe's segments in the trace, `threads`,
  `top`, `topin`, `callee`, `caller`, `cores`, and `buckets` with `rules-exact2-main.txt`, `rules-swiftui-main.txt`,
  `rules-offmain.txt` (the probe's own rule is last: its tick is the outer frame of the whole scroll callback).
- **The reference series** (bones, origin/main 9fdf7e56, 3 rounds): `results/main9fdf-bones-r1` (summary.md,
  byspeed.md); the ranked trace in `results/traces/main9fdf/`.
