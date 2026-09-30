# Apple hosts never paint an inline run's `background-color`

**Status:** Closed
**Resolution:** Fixed in 15e47aa9 Closure audit 2026-09-30: archive the already-landed fix; its reproduction and verification evidence remain below.
**Systems:** Apple host text (`Run`, the batch's inline rows, `TextLinePaint`; LLP 1044.000 §6 S1)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-25
**Related:** the heavy list benchmark (`~/bench/heavybench/`, outside the repo; `exact-textfix/README.md` gap 1)

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed in 15e47aa9

## What happens

The Contract accepts `background-color` on an inline `text`, and the web paints it. The heavy list's code runs have `background-color="#F2F2F7"`. On iOS and macOS they show only their monospaced face (`~/bench/heavybench/shots/textfix-before-clamp.png`, "list_viewport").

## Why

The runner sends the inline run's whole style row, `background_color` included. The Apple batch reader kept only the glyph and colour keys (`BatchReader.inlineStyle`), and `Run` had no background field, so neither the draw fallback nor the raster could paint one.

## Fix

15e47aa9, per CSS: an inline box's background covers each of its line fragments.
- `Run.background`, light and dark as `text_color` is (a paired value marks the run as a scheme colour). It is paint: it is in `TextPaint` and stripped from `Spec.geometry`, so it never changes measurement or the metric cache.
- `TextEngine.attributed` carries it as an `InlineBackground` attribute, which holds the colour and the content area of the run's own font. The main thread's draw and the raster workers read the same copied source.
- `TextLinePaint` fills, per line and before that line's glyphs, each background's glyph advance across and its font's ascent + descent down. Glyph runs split by fallback or bidi join where they touch. The raster's frame includes the fills.
- A container run's background covers its descendants when they have none of their own (`paragraphSpec`).
- macOS and iOS share all of it.

Not in scope: an inline box's padding, border and radius (not authored, and not in the heavy list).

## Evidence

- The heavy list on an iPhone 17 Pro simulator: code runs paint #F2F2F7 behind their glyphs across the run's advance (`textfix-after-clamp.png`, `textfix-after-arabic.png`).
- `TextPaintTests`: a run that wraps has one background per line fragment, starting at the run's offset, with the font's content area. Painted, the fragment's corner is #F2F2F7 and the plain run has none. Background is paint, not metrics. A batch row carries the light and dark background.
- The macOS Swift tests (400), the iOS XCTests, `smoke.mjs ios` and `macos` pass. Messages and Markdown on iOS are pixel-identical before and after.
