# Extra Heavy feed — UIKit app (iOS)

The Extra Heavy feed (`../../SPEC.md`) in hand-written UIKit, bundle `dev.exact.xheavy.uikit`, added 2026-09-29 as
the fourth competitor. Written as a performance-minded iOS engineer writes UIKit; same content and work as the
SwiftUI app (the reference), per-kind equivalences in the SPEC's Decisions, "UIKit".

| file | what |
|---|---|
| `App.swift` | data model (the SwiftUI app's), env (`BENCH_LIVE/FREEZE/START_INDEX/KINDS/SCENARIO`), clocks, app state (drafts, expansion, inner offsets), app + scene delegate |
| `Feed.swift` | top bar, the `UICollectionView` (compositional layout, estimated 470, diffable data source, prefetching), the 1 Hz tick, start index |
| `Cells.swift` | `FeedCell` (header, separator, manual layout + self-sizing, visibility) and photo, thumbs, canvas, svg, video, map, typeface, intl, motion, glass |
| `Text.swift` | markdown, code, thread, web view |
| `Live.swift` | the live card (CA keyframe rings and playhead) |
| `Shader.swift`, `Shader.metal` | the shader row's `MTKView` and fragment shader |
| `Nested.swift` | carousel, filmstrip, inbox (nested collection views) |
| `Media.swift` | image loader (ImageIO downsample off main, cache, prefetch, cancel), video, animated images, Lottie cache, web HTML |
| `build.sh` | `build/XHeavyUIKit.app` (iphoneos, unsigned); `SIM=1` adds `build-sim/` (ad-hoc signed); `SIMONLY=1` skips the device build |
| `metal.sh` | compiles `Shader.metal` to `obj/default-<sdk>.metallib` with the Metal Toolchain (this Mac's, or `BENCH_METAL_HOST`'s over ssh). If the metallib is missing the app compiles `Shader.metal` from its bundle at runtime (not the benchmark build) |

Build notes: no Xcode project; `swiftc -O -wmo` over the eight files, `actool` for the SVG asset catalog (as the
SwiftUI app), the vendored Lottie static library (copied from `../swiftui/obj/lottie-<sdk>` or built from
`../../vendor/lottie-ios`), `UIAppFonts`, a `UIApplicationSceneManifest` (the iOS 27 simulator traps a UIKit app
without scene lifecycle). Device: `../../device/XHeavyUIKit.app` = `build/` signed with the probe by
`../../../heavy-list/probe/resign.sh … dev.exact.xheavy.uikit`.

Runs: as the other apps (`series.sh` with `APPS="swiftui expo exact2 uikit"`, `summarize.py` includes `uikit`).

## Parity (simulator, `BENCH_FREEZE=1`, `../shoot-sim.sh` at the top and start rows 3, 7, 12, 16, 20, 500)

Every kind matches the SwiftUI app by eye: geometry, fonts, colours, the shader's duotone, canvas, SVGs, maps,
glass, the nested lists. Small differences, accepted:
- Wrapped text can break a word differently (UILabel vs SwiftUI `Text`), e.g. one Markdown paragraph wraps
  "build | notes" where SwiftUI wraps before "build".
- Intl: UIKit's line boxes for fallback scripts are a little taller; the Arabic block's wrapped lines are
  right-aligned (SwiftUI's are not; its README records that gap).
- The inbox's content height is 63,112 pt (items exactly 55/71 pt as the SPEC gives) vs SwiftUI's 65,852 and exact2's 63,024.
- Separator: a 0.5 pt layer (SwiftUI: the system 1 px hairline).

Bug found and fixed while checking parity: `MTKTextureLoader.newTexture(cgImage:)` on the ImageIO thumbnail gave
wrong channels (grey-blue duotone); the texture is now made from an explicit RGBA8 sRGB bitmap.

## Smoke (simulator, 60 Hz)
fling 0: 60 fps to ±3k, 48–56 at 6k–24k (sim, loaded Mac); innerfling 0: strip and inbox 60 fps at every speed;
innerfling 4: no blank band in either inner list; innerkeep 0: all four marks kept; jump p50 79 ms; rest 0: CPU 37 ms/s.

## iPad smoke (M1 iPad Pro, 120 Hz, 2026-09-29 04:11, one run)
fling 0: whole run 3.63 ms busy/frame, CPU 434 ms/s, main 304 ms/s; fps 92–115 by speed; **peak 433 MB**
(SwiftUI smoke: 4.19 ms/frame, CPU 546, main 414, peak 303 MB). iPad parity shots (`../shoot.sh`, top, 3, 7, 500)
match SwiftUI's. The memory is the first thing to trace: every map/web/shader/video cell keeps its
MKMapView/WKWebView/MTKView/AVPlayer across reuse, and the 300-entry image cache holds full-width photos.

## Sections (added by the parent session, 2026-09-29)
The feed's rows are in header-less sections of 500 (`BENCH_CHUNK` overrides; 0 = one section). With one section each
self-sized cell makes the compositional layout re-solve every estimated row after it; the heavy bench's UIKit app
traced it (`resolveForInvalidatedPreferredAttributes`). Simulator fling 0, 19 kinds: busy 3.67 → 3.12 ms/frame, the
12k pt/s segment 55.8 → 60 fps. Invisible on screen.

## Known issue (parent session, 2026-09-29): the motion row drops frames
In the full-feed series (exact2 dca795520, 2026-09-29) the UIKit filmstrip `innerfling`
reads a flat ~80 fps at every speed on both devices (SwiftUI and exact2: 120). Isolation on the iPhone
(`BENCH_KINDS=filmstrip,inbox,<kind>`): with `shader`, `photo`, `live`, `video` or
`carousel` rows beside the strip it holds 120 fps; with `motion` rows it drops to ~105 (p95 20.6 ms), and in the real
feed (motion + live + shader + video in view) to 80. A waiting-threads Time Profiler trace
shows the main thread idle in `mach_msg` (not blocked;
`nextDrawable` waits are ~20 ms/s), so the loss is frame pacing, not main-thread work. The GIF/WebP
(`CGAnimateImageAtURLWithBlock`) and `LottieAnimationView` tiles are the suspects; not fixed yet. It penalises this
app's filmstrip `innerfling` and some feed frames, i.e. it favours the other stacks.
