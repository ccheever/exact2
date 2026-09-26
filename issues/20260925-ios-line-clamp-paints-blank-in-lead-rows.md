# iOS: a `line-clamp` paragraph in a row the list builds during a scroll paints blank

**Status:** Fixed in f10253d1
**Systems:** Apple host (iOS `NodeView.draw(_:)`, text rasters, LLP 1044.000 §6), Scrolling (collections, LLP 1010 §6.5)
**Severity:** P1
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-25
**Related:** the heavy list benchmark (`~/bench/heavybench/`, outside the repo; `SPEC.md`, and `exact-textfix/README.md` gap 2); 0f19ab33 (the draw's visibility gate), 0e047888 (`NodeLayer.display`), 9e06c4ea (the row pool)

## What happens

A virtualized list's rows mounted during a later scroll can show nothing where a clamped `text` should be. The agent tree has the text and its frame. Removing `line-clamp` makes it paint; removing `text-overflow` does not help. In the heavy list, `"tap messages wheel 0 79300" "clock settle" "tap messages wheel 0 800" "clock settle"` leaves m487's link-card title and description (`line-clamp=2`) blank (`~/bench/heavybench/shots/textfix-before-clamp.png`).

## Why

A clamped paragraph cannot raster (`canRasterText` excludes `line_clamp`), so `NodeLayer.display` sends it to `draw(_:)`, and that bitmap is its only paint. Since 0f19ab33, `draw(_:)` paints a paragraph's text only when `textIsVisible`. The gate is meant for paragraphs whose raster is on its way. The list builds its lead rows below the scrollport. Their first display runs there, invisible, and keeps an empty bitmap. Nothing redisplays the row when it scrolls in, because `refreshVisibleText` only visits paragraphs that can raster. Neither the pool's reuse (9e06c4ea) nor the budgeted fill is the cause, but both make off-screen first draws common.

## Fix

f10253d1: a paragraph that cannot raster draws its text visible or not (`!canRasterText || Capture.capturing || textIsVisible`), as every paragraph did before 0f19ab33. A paragraph that can raster keeps the gate.

## Evidence

- The heavy list, same steps, on an iPhone 17 Pro simulator (iOS 27): m487's title and description paint and match `swiftui-index478.png` (`~/bench/heavybench/shots/textfix-after-clamp.png`, `textfix-after-478.png`).
- `TextPaintIOSTests.testAClampedParagraphInAReusedRowBuiltBelowTheScrollportPaints`: a row retires, the pool lends its clamped paragraph's view to the next row built at y 1000 below a 300-pt scrollport, and that view's `draw(_:)` paints text. Without the fix: 0 ink pixels, a failure; with it: pass.
- Messages and Markdown on iOS: agent screenshots before and after are pixel-identical. `smoke.mjs ios` and `macos` pass.
