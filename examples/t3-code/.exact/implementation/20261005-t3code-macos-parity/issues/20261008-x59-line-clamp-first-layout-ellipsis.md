---
name: 20261008-x59-line-clamp-first-layout-ellipsis
plan: 20261005-t3code-macos-parity
status: draft
kind: framework-gap (unconfirmed)
blocks: []
upstream_url: null
reproduced_on: feat(example)/t3-code-visual-parity-followup (07dcef1ab's framework) and base 07dcef1ab
---

# X59: a `line-clamp=1` text that replaces a wrapped one shows its first wrapped line without the ellipsis (macOS, unconfirmed)

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
