# macOS: a paragraph that shrinks below the text-raster size keeps painting its old lines

**Status:** Open
**Systems:** host/apple macOS, text raster
**Severity:** P1
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/316

## Current scope

Remove obsolete raster/overflow-layer pixels on switching to direct small-text paint. Check actual window pixels in both threshold directions, clamp and shadows. Preserve replacement behavior for paragraphs that remain rastered and #305 ellipsis; coordinate #291.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

On macOS, the host draws a large paragraph from a text raster (a surface a worker paints) and a small one directly in `draw(_:)`. "Small" means `textIsSmall`: fewer than `TextRasterizer.minPixels` (16,384) device pixels (`host/apple/Sources/ExactKit/Mac/TextRasterMac.swift:30, 210-213`).

When an update shrinks a rastered paragraph below that size, the window keeps showing the old raster: the old lines at their old geometry. The view tree, `layout` and the agent's capture-path `screenshot` all have the new text. Only the window's own pixels are stale. A `line-clamp` box clips the old lines; an unclamped box lets them overflow onto the next box. The text stays wrong until something remounts the node or makes the paragraph large again.

The consumer is T3 Code's provider list. A row's status is `line-clamp=2` text. After Reconnect it changes from "Not authenticated · Sign in with ChatGPT to use Codex." (two lines) to "Authenticated · ChatGPT" (one short line), and the row kept painting "Not authenticated · Sign…" until Providers was reopened. The status reads as the opposite of the truth. The clone now keys these texts by their value (`each s in [status] key=s`), so each new status is a new node.

### Current and expected behavior

- **Current (macOS):** after one toggle, the two rows whose paragraph shrinks from 200×40 to 200×20 pt (16,000 device px at scale 2) still show their old raster in the window:
  - `clamped-2`: "Not authenticated · Sign in", the old first line, clipped by the clamp box;
  - `plain`: both old lines, the second overflowing onto the next box.

  The tree says "Disabled" for every row. `layout clamped-2` is `space viewport 25,71 200×20`, and the capture-path screenshot draws "Disabled".
- **Rows that update correctly:**
  - a row that is small both before and after (`clamped-1`, `nowrap`);
  - a keyed row, which is a new node;
  - a wide row that stays above the threshold (`wide`, 420×20 pt = 33,600 px).
- **Back to the long text:** the second toggle repaints every row correctly.
- **Current (web):** every row shows "Disabled" after the toggle.
- **Expected:** the window shows the new text from the first frame after the update, whichever path (raster or `draw(_:)`) the paragraph's new size selects.

Hypothesis, from reading the source (not instrumented):
- `canRasterText` refuses a `textIsSmall` paragraph (`TextRasterMac.swift:189-203`), so the view moves from `updateLayer` (raster) to `draw(_:)`.
- `textRasterGeometryChanged()` (`:230-237`) keeps the accepted surface up "at its original dimensions" until new pixels replace it. `presentTextRaster()` moves it into `textRasterOverflowLayer` when the frame no longer matches.
- Nothing ever replaces it, because the rasterizer only serves paragraphs for which `rastersText` is true (`:269`). The overflow layer stays up over the directly drawn text.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`, with no data sources:

```text
component X64app
  state short = false
  action toggle
    short = not short
  derive status = short ? "Disabled" : "Not authenticated · Sign in with ChatGPT to use Codex."
  view
    column testId="root" width="100%" height="100%" box-sizing="border-box" padding=24 gap=12 background-color="light-dark(#ffffff, #111111)" color="light-dark(#111111, #eeeeee)" font-size=14 line-height="20px"
      button press=toggle testId="t" width=120 padding=6 border-width=1 border-style="solid" border-color="#cccccc"
        text "toggle"
      column width=200 border-width=1 border-style="solid" border-color="#cccccc"
        text status line-clamp=2 testId="clamped-2"
      column width=200 border-width=1 border-style="solid" border-color="#cccccc"
        text status line-clamp=1 testId="clamped-1"
      column width=200 border-width=1 border-style="solid" border-color="#cccccc"
        text status testId="plain"
      column width=200 border-width=1 border-style="solid" border-color="#cccccc"
        text status white-space="nowrap" overflow="hidden" text-overflow="ellipsis" testId="nowrap"
      column width=200 border-width=1 border-style="solid" border-color="#cccccc"
        each s in [status] key=s
          text s line-clamp=2 testId="keyed"
      column width=420 border-width=1 border-style="solid" border-color="#cccccc"
        text (short ? "Disabled" : "Not authenticated · Sign in with ChatGPT to use Codex. Not authenticated · Sign in with ChatGPT.") testId="wide"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Shrink, macOS window | `bun exact.mjs mac`; `bun exact.mjs agent macos --size 480x420 "screenshot before.png window" "tap t" "clock settle" "screenshot after.png window" "screenshot after-capture.png" tree "layout clamped-2" "tap t" "clock settle" "screenshot back.png window"` | macOS 26.6.2, Apple Silicon, scale 2 | main `b896050d7` | window: `clamped-2` "Not authenticated · Sign in", `plain` both old lines; tree, `layout` and capture: "Disabled"; `back.png` correct | "Disabled" in every row of the window | [record](https://raw.githubusercontent.com/ccheever/exact2/7281d50a9829dd6af92c7d376d97b6f3218cf9df/file-x48-x68/x64-record.txt), [window, capture and web side by side](https://raw.githubusercontent.com/ccheever/exact2/dfef5b5e99f7158a118e18f00a2280c4a7cd78ea/file-x48-x68/x64-macos-vs-web.png), [second toggle](https://raw.githubusercontent.com/ccheever/exact2/2c23290feda795ac018ace20a37fb8fb6a3e9920/file-x48-x68/x64-back.png) |
| Same, web | `bun exact.mjs agent web --size 480x420 "tap t" "clock settle" "screenshot web.png" tree` | Chrome 154 (the agent's) | same | every row "Disabled" | (the reference) | same images and record |

![After one toggle: the macOS window keeps old lines in rows 1 and 3; the capture path and the web show Disabled](https://raw.githubusercontent.com/ccheever/exact2/dfef5b5e99f7158a118e18f00a2280c4a7cd78ea/file-x48-x68/x64-macos-vs-web.png)

`TextRasterMac.swift` was last changed in `477804f9b`. The same repro was also seen on main `475043d20`, which already contains #305.

### Acceptance criteria

- After the first toggle in the repro, the macOS window (`screenshot … window`) shows "Disabled" in every row, with nothing drawn outside each box. The capture path and the web agree.
- A paragraph that crosses the threshold in either direction (raster → `draw` and `draw` → raster) shows its new text on the next frame. That holds with and without `line-clamp`, and with a `text-shadow`, which is drawn through the overflow layer.
- No regression of #305's first-paint ellipsis, or of the "old pixels stay until the new ones replace them" behavior for a paragraph that stays rastered.

### Constraints and related work

- Workaround: key the text by its value (`each s in [status] key=s`), so a changed string is a new node. Any clamped or wrapped label whose content can shrink is exposed until it is keyed.
- Not the same as #300, now fixed by #305: there the clamped text had the right string and missed its ellipsis on first paint, and this repro still fails after #305. Not #291 either, which is the alignment of an over-wide line.
- All three touch `TextRasterMac.swift`, so a fix here should be checked against #291 and #305.
- Not tested: iOS (`TextRasterIOS.swift` has its own raster path), and a real resize of the window.
