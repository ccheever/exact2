# iOS node views all draw, so every view carries a bitmap and a collection's spacers ask for gigabytes

**Status:** Closed
**Resolution:** Fixed in 0e047888 Closure audit 2026-09-30: archive the already-landed fix; its reproduction and verification evidence remain below.
**Systems:** Apple host (iOS `NodeView`), Scrolling (the collection's spacers, LLP 1010 §6.5)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-25
**Related:** LLP 1050.000 (found while benchmarking the fill policy against Expo PR 49975); the macOS host's `wantsUpdateLayer` split (`host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:1046–1073`); the benchmark harness `~/bench/listbench/` on Charlie's Mac (outside the repo; `diag.m` is the probe below)

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed in 0e047888

## What happens

`NodeView` on iOS overrides `draw(_:)` for every node (`host/apple/Sources/ExactKit/IOS/NodeViewIOS.swift:1204`). It paints background, rounded path, borders and images there. `applyStyle` and the prop update end in `setNeedsDisplay()` (`:1088`, `:1144`).

UIKit gives any view whose class overrides `draw(_:)` a backing store the size of its bounds times the screen scale. It does so even when the view paints nothing. The macOS host avoids that with `wantsUpdateLayer`: empty containers "carry geometry and children, with no bitmap". iOS has no equivalent.

Two consequences, measured 2026-09-25 on the iPhone 17 simulator (iOS 27, M5 mini). The workload is a 10,000-row `list virtualized=true` scrolled to one third, and a probe injected into the running app walked its layers.

1. **The collection's spacers are node views, so UIKit asks for their backing stores.** The views above and below the mounted window measured 402 × 432,880 pt and 402 × 865,678 pt. At 3× the larger one would need about 12 GB. CoreAnimation refuses: the simulator log repeatedly shows `Ignoring bogus layer size (402, 1299046)`. That is about the whole content height, presumably a spacer while nothing was mounted above the window (inference).
2. **Every painted box is a bitmap.** 30 of the 35 views under the list had a `contents` bitmap. Each row's wrapper and card carry one about 402 × 150 pt at 3×. At the same scroll position:

   | | exact2 | Expo (SwiftUI `List`) |
   |---|---|---|
   | CoreAnimation memory | 24 MB | 1.9 MB |
   | Total footprint | 57 MB | 89 MB |

   Expo's 64 views had none. Its cells paint through layer properties.

A side effect showed up as a benchmark artifact. `CALayer.render(in:)`, and so any snapshot of the window, re-runs `draw(_:)` for every such view: the whole window took 15.3 ms for exact2 against 2.4 ms for Expo. On-screen scrolling is not affected, because CoreAnimation composites the existing bitmaps (120 fps at 24,000 pt/s on an iPhone 17 Pro Max in the same benchmark). Memory is affected, and so is every capture path that renders layers.

## Fix

Follow the macOS split on iOS:
- A node with no box paint has no bitmap.
- The collection's spacers never draw.
- A box whose paint CoreAnimation can express uses layer properties: `backgroundColor`, `cornerRadius` with `maskedCorners`, and `borderWidth`/`borderColor`. This covers uniform borders, and different radii per corner via a mask.
- Only what CoreAnimation cannot express keeps drawing: non-uniform borders, images, capture of a nested canvas.

One way is for `NodeView` to implement `display(_ layer:)`. UIKit then does not allocate its own backing store, and paint happens only when there is some. Keep the web's box semantics exact: the uniform border follows the curve, as the current `draw(_:)` comment requires.

## Done when

- In the listbench probe, the collection's spacers and every unpainted container have `contents == nil`.
- The `bogus layer size` log is gone.
- A row box with a background and radius has no bitmap.
- CoreAnimation memory at the same scroll position falls to within a few MB of the row count's real paint.
- Agent screenshots, the web parity corpus and the iOS XCTests are unchanged.

## Fixed

0e047888 makes `NodeLayer.display` decide (`host/apple/Sources/ExactKit/IOS/BoxLayerIOS.swift`). A box whose paint Core Animation can express goes on the layer: `backgroundColor`, one `cornerRadius` over `maskedCorners` (after CSS's radius reduction), and a uniform border. The border is a sublayer under the children, or the layer's own when they are clipped, scrolled or painted through a surface. Everything else still runs `draw(_:)` unchanged: borders or colours that differ by side, radii that differ by corner, images, paragraphs without a raster, and capture pictures. A node with nothing to draw has `contents == nil`.

Measured with the listbench copy and the probe on this Mac's iPhone 17 simulator, at a third of the list, before and after on the same build otherwise:

| | before | after |
|---|---|---|
| CoreAnimation (`footprint`) | 24 MB | 9.8 MB |
| Total footprint | 81 MB | 67 MB |
| Node views under the list with `contents` | 181 of 219 | 0 of 219 |
| Layers with `contents` | 214 | 33 (the text rasters) |
| `Ignoring bogus layer size` logs | 3 | 0 |
| Whole-window `renderInContext` | 22.1 ms | 1.0 ms |

Both spacers (402 × 432,880 pt and 402 × 865,678 pt) and every row box now have no bitmap.

The iOS XCTests (8, including `BoxLayerIOSTests`), the macOS Swift tests (386), the five checks and `smoke.mjs ios` pass. Agent screenshots of Caltrain, Interaction Gallery, Messages and Messages stress match except for antialiasing on curved edges. One difference is deliberate: an `overflow: hidden` rounded box now clips its children round, as the web does (Interaction Gallery's photo cards). Before, it clipped them square.

