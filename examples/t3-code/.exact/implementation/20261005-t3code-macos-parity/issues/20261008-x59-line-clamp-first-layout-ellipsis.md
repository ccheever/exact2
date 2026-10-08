---
name: 20261008-x59-line-clamp-first-layout-ellipsis
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: []
upstream_url: https://github.com/ccheever/exact2/issues/300
reproduced_on: febb2c5fb (main; a one-file app, window pixels); first seen in the clone on feat(example)/t3-code-visual-parity-followup and base 07dcef1ab
---

# X59: a `line-clamp` text mounted after launch paints its last line without the ellipsis until a restyle (macOS)

## Summary

On the macOS host, when a branch swaps a wrapping `text` for a `line-clamp=1` text with the same
runs, the clamped text's first layout shows the line that the wrapped text broke at, cut at that
word, with no ellipsis. Any later restyle (a `prefers-color-scheme` change) lays it out again with
the ellipsis at the box's edge. The expected result is CSS's: one line cut with `…` at the edge
from the first frame.

## Why this issue arose

### The T3 Code behavior
`ChatMarkdown.tsx` MarkdownTable: "Collapse table cells" sets `data-expanded="false"`, and
`index.css` gives every cell `white-space: nowrap; overflow: hidden; text-overflow: ellipsis`.

### What exact2 does today
Driven with the agent (2026-10-08, lane fixture thread, `screenshot … window`):

- Light, right after `tap table-expand-1` and `tap table-expand-5` (cells collapsed): the long value
  reads "…keeps going past the collapsed width, so": the expanded layout's first line, no `…`.
  Base `07dcef1ab` shows the same at its 22.5rem cap ("…past the collapsed").
- `prefer prefers-color-scheme dark` next: the same cell reads "…the collapsed width, s…".
- Evidence: `t3-code-evidence/visual-parity-followup/09-table-light-collapsed.png` (no ellipsis)
  and `10-table-dark-collapsed.png` (ellipsis), transcripts `ops-before.txt`, `ops-after.txt`.

A one-file app has not been tried (the task's session budget was spent); the excerpt to try:

```contract
component Root
  state collapsed = false
  action toggle
    collapsed = not collapsed
  view
    column width=300
      button press=toggle testId="t"
        text "toggle"
      when collapsed
        text "A long value that keeps going past the width so the line has to be cut with an ellipsis" width=0 min-width="100%" line-clamp=1 testId="v"
      else
        text "A long value that keeps going past the width so the line has to be cut with an ellipsis" width=0 min-width="100%" overflow-wrap="anywhere" testId="v"
```

`agent.mjs macos "tap t" "screenshot v.png window"`: expected one line ending in `…`.

### Where the clone hits it
`markdown.contract` TableCell: the collapsed branch's `line-clamp=1` text after "Collapse table
cells" (the light capture above).

## Why it must be resolved
A collapsed cell then reads as complete text cut at a word: nothing tells the reader that more of
the value is hidden until something else restyles the transcript.

## Upstream (filed 2026-10-08)

Upstream: https://github.com/ccheever/exact2/issues/300 (#300, [Bug] macOS: a `line-clamp` text mounted after launch
paints its last line without the ellipsis until a restyle). Reproduced on main `febb2c5fb` with a one-file app (macOS
26.6.2). The bug is wider than this draft said: it is not about replacing a wrapped text. **Any** `line-clamp` text
mounted after launch (a `when` arm that appears, or an `else` arm swapped for a clamped one; `line-clamp=1` and `=2`
alike) paints its last kept line cut at a word with no "…", while the layout is right (`layout` reports one line,
`300×20`). A clamped text present from the first frame, and a mounted `white-space="nowrap"` +
`text-overflow="ellipsis"` text, do show "…"; any restyle (`prefers-color-scheme`) repaints every row with it. It is on
the first-raster path, not in layout: a new paragraph's first pixels reuse its plain lines and skip the clamp's last
line (`TextRasterizer.ensure` → `TextRasterJob.render(lines:)`), and a restyle re-rasterizes through the clamp. The
agent's default `screenshot` hides it (capture declines the text raster and draws the node itself, with "…"): only `screenshot … window`, the window's own pixels, shows what a person sees.
The web shows "…" on every row at once. Evidence: [x59-collapse.png](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x59-collapse.png) (web vs macOS after the
toggle, and macOS after a restyle), transcript [x59-ops.txt](https://raw.githubusercontent.com/ccheever/exact2/649f5649dc51c4f75562485f5cabc5ef74baabd1/file-x59-x61/x59-ops.txt).

Next: `issue-close` once #300 lands: the collapsed table cell's first frame should show "…" in a `screenshot … window`
capture with no restyle. The clone has no workaround to remove.

## Fixed on main (2026-10-08)

[#300](https://github.com/ccheever/exact2/issues/300) was closed by main #305 (`9314e7a81`). Main adoption round 7
brings it in (round 7 waits for main fix of X67, the compiler's stack overflow in main's examples test); then
`issue-close`: the collapsed table cell's first frame shows "…" in a `screenshot … window` capture. Nothing to
remove.
