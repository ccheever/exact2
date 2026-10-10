# macOS: a centred line wider than its box is centred and clipped at its start (CSS start-aligns it)

**Status:** Open
**Systems:** host/apple, text geometry, raster
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/291

## Current scope

Apply direction-aware CSS overflow start alignment before truncation across geometry, paint, raster, selection and hit testing. Cover long/short centered/right lines, clamp and RTL; preserve #305 and coordinate #316.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

CSS Text 3 §7.1 (`text-align`): "If (after justification, if any) the inline contents of a line box are too long to fit within it, then the contents are start-aligned: any content that doesn't fit overflows the line box's end edge."

The macOS host aligns an over-wide line by its `text-align` anyway. Under `center` (a `button`'s value, which its text inherits, LLP 1001 §1) the line starts left of its box, so its start is clipped. With `text-overflow: ellipsis` the "…" is then drawn early, with blank space after it. The web host start-aligns the line, so one Contract source shows two different strings. A truncated label loses exactly the part a reader needs, and an author finds out only on macOS.

### Current and expected behavior

- **Current (macOS):** in a `button` (centred by default) or a `row text-align="center"`, a `nowrap` + `ellipsis` title wider than its box reads "cribe the vowel count…". A `width=160` `nowrap` + `overflow="hidden"` centred text with no ellipsis reads "ribe the vowel counter (ec", with both ends cut. `layout` reports `text_align = center (inherited from #2)`. The same row with `text-align="left"` reads "Describe the vowel count…".
- **Current (web):** all four start at "Describe".
- **Expected:** on macOS an over-wide line is laid out start-aligned (direction-aware) whatever `text-align` says, before the ellipsis is made, for paint, raster, selection and hit-testing alike. A line that fits is aligned as now.

Hypothesis, from reading the source: each line is placed at `CTLineGetPenOffsetForFlush(line, flush, width)` of the line as broken, and for an over-wide line that offset is negative under flush 0.5 or 1. The sites are:

- `ParagraphGeometry.swift:129`;
- `TextRaster.swift:169`;
- `RegionRaster.swift:278`, `RegionTextGeometry.swift:40` and `TextFlow.swift:251`.

The ellipsis is made afterwards at the box width and drawn at that offset. iOS's raster passes the same flush (`IOS/TextRasterIOS.swift`), so iOS and tvOS are likely affected too (not driven).

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`:

```text
component X57App
  view
    main testId="root" width="100%" height="100%" box-sizing="border-box" padding=24 gap=12 background-color="light-dark(#ffffff, #111111)" font-size=14 line-height="20px"
      button testId="row-button" width=240 padding=6 border-width=1 border-style="solid" border-color="#cccccc" display="flex" flex-direction="row" align-items="center" gap=8
        text "#159" flex-shrink=0 color="#777777"
        text "Describe the vowel counter (edited)" testId="title-button" min-width=0 white-space="nowrap" overflow="hidden" text-overflow="ellipsis" flex-shrink=1
        text "✓" flex-shrink=0
      button testId="row-left" width=240 padding=6 border-width=1 border-style="solid" border-color="#cccccc" display="flex" flex-direction="row" align-items="center" gap=8 text-align="left"
        text "#159" flex-shrink=0 color="#777777"
        text "Describe the vowel counter (edited)" testId="title-left" min-width=0 white-space="nowrap" overflow="hidden" text-overflow="ellipsis" flex-shrink=1
        text "✓" flex-shrink=0
      row testId="row-box" width=240 padding=6 border-width=1 border-style="solid" border-color="#cccccc" align-items="center" gap=8 text-align="center"
        text "#159" flex-shrink=0 color="#777777"
        text "Describe the vowel counter (edited)" testId="title-box" min-width=0 white-space="nowrap" overflow="hidden" text-overflow="ellipsis" flex-shrink=1
        text "✓" flex-shrink=0
      text "Describe the vowel counter (edited)" testId="title-clip" width=160 white-space="nowrap" overflow="hidden" text-align="center"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Four over-wide lines, macOS | `bun exact.mjs mac`; `bun exact.mjs agent macos --size 420x320 "screenshot x57-macos.png" "layout title-button"` | macOS 26.6.2, Apple Silicon | main `0365ad1a4` | row-button "cribe the vowel count…" with a gap before ✓; row-left "Describe the vowel count…"; row-box "cribe the vowel count…"; title-clip "ribe the vowel counter (ec"; `text_align = center (inherited from #2)` | all four start at "Describe" | screenshot, `layout` |
| Same, web | `bun exact.mjs agent web --size 420x320 "screenshot x57-web.png"` | Chrome 154 (the agent's) | same | all four start at "Describe" (the clip line ends "vowel coun") | (the reference) | screenshot |

The text-placement files above are unchanged between `0365ad1a4` and main `e200397ec`.

### Acceptance criteria

- On macOS the repro's four lines start at "Describe", as on the web. The ellipsis sits at the end edge.
- `layout` places the centred title's glyphs from the box's start edge. Selection and hit-testing agree with the paint.
- A centred or right-aligned line that fits is unchanged. An RTL over-wide line starts at the right edge.
- iOS and tvOS behave the same (they share the flush offset).

### Constraints and related work

- Workaround: `text-align="left"` (or `start`) on every truncating label inside a `button` or a centred container, including where the web needs none.
- Not tested: iOS, tvOS, RTL text, `line-clamp` lines.
- Related: #128 / #266 (other text differences from Chrome on macOS).

## Discussion at transfer

### ccheever — 2026-10-08T08:07:24Z

**Decision: Start-align overflowing lines consistently in all text paths.**

Keep open for a correctness fix.

The CSS overflow rule is clear. Apply it to geometry, paint, raster, selection and hit-testing, including shared Apple code.

Exercise long/short centered and right-aligned lines, ellipsis, clamp and RTL. Rebase against #305 so this does not undo the first-paint ellipsis fix.
