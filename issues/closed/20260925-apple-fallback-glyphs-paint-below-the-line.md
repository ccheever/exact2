# Apple hosts paint fallback glyphs with vertical offsets upside down, so a line ending in Arabic is clipped

**Status:** Closed
**Resolution:** Fixed in 83e363c3 Closure audit 2026-09-30: archive the already-landed fix; its reproduction and verification evidence remain below.
**Systems:** Apple host text (`TextEngine.draw`, `TextRasterJob`, `RegionRaster`; LLP 1008 §3)
**Severity:** P1
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-25
**Related:** the heavy list benchmark (`~/bench/heavybench/`, outside the repo; `exact-textfix/README.md` gap 8)

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed in 83e363c3

## What happens

In the heavy list on iOS, m478 ("notes مرحبا، سنلتقي في المحطة غداً.") loses the bottom of its line (`~/bench/heavybench/shots/exact-500.png`). In a two-line paragraph (m475, `textfix-before-arabic.png`) the words "في المحطة غداً" sit visibly lower than "مرحبا، سنلتقي" on the same line. SwiftUI draws them on one baseline (`swiftui-index478.png`).

## Why

Measurement was right. The line box includes the fallback face (`.SFArabic-Regular`, ascent 16.02 / descent 3.86 at 16 pt, all glyph ink inside it), and the frame was 20 pt. Drawing was wrong. Every Apple painter flipped y for its y-down context with a text matrix of scale(1, -1). CoreText places a glyph's vertical offset (`CTRunGetPositions` y) in text space, so that matrix inverted it. SF Arabic's cursive attachment offsets glyphs by up to 3.3 pt, and those glyphs landed about 4 pt below where CoreText measured them. That is below the line box and below the ink frame the raster sized from `CTLineGetBoundsWithOptions`, so the bitmap cut them off. Measured on macOS: at baseline 20 the ink bottom was 28 px under the text-matrix flip and 24 px (baseline + descent) under a CTM flip. Latin, emoji and a decomposed accent have no y offsets and are byte-identical both ways at integral positions.

## Fix

83e363c3: one line painter, `TextLinePaint.draw` (`Text.swift`), flips at each baseline in the CTM and draws with an identity text matrix. `TextEngine.draw` (iOS and macOS draw fallback), `TextRasterJob.render` (the iOS and macOS raster workers) and `RegionRaster` (the macOS reader) all use it. The viewport-ink test oracle in `TextGeometryTests` flips the same way. An emoji bitmap at a fractional y resamples differently between the two flips (same extent).

## Evidence

- The heavy list on an iPhone 17 Pro simulator: m478's line is whole and on one baseline, matching `swiftui-index478.png` (`textfix-after-478.png`, `textfix-after-arabic.png`).
- `TextPaintTests.testFallbackGlyphOffsetsPaintInsideTheMeasuredLineBox`: the painted ink ends where CoreText's glyph bounds say, inside the measured line box.
- The macOS Swift tests (400), the iOS XCTests, `smoke.mjs ios` and `macos` pass. Messages and Markdown on iOS are pixel-identical before and after.
