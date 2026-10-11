# A hatch cannot run at a fixed period across a list's rows: `repeating-linear-gradient()`, length stops and `background-attachment: local` are refused

**Status:** Open
**Systems:** kernel gradient, Contract background-image and background-attachment, every host
**Severity:** P3
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-11
**Related:** LLP 1066 D2 (the grammar it refuses: `repeating-*`, length stop positions) and D7 (`local` refused); LLP 1077 ("Still refused: `repeating-*`"); `QUEUE.md` "The document window, what the studio pass left" (repeating gradients and length stops refused everywhere); https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/EXACT2-GAPS.md ("Split view's hatched empty side"); T3 PR #427

## Summary

CSS hatches a region with a repeating gradient: 45° stripes 2px wide every 8px are
`repeating-linear-gradient(-45deg, transparent 0 4.243px, #e6e6e6 4.243px 5.657px)`. Contract refuses that value
(`kernel/src/gradient.rs:573` `REFUSED`), refuses the same stripes written as a plain `linear-gradient()` with length
stop positions (`gradient.rs:928`: "a stop's position is a percentage; lengths are not implemented"), and refuses
`background-attachment: local` (schema bit 171 admits `scroll` and `fixed`).

So an author cannot paint one hatch at a fixed period under a run of rows of a virtualized list. Each row can draw its
own stripes (an SVG `pattern` builds and paints), but each row's pattern starts at its own top. Unless the period
divides every row's height, the stripes break at every row edge; with rows that wrap to a different number of lines,
no row knows its offset in the run, so no per-row phase fixes it. In Chrome the same rows are hatched by one pattern on
the scroller with `background-attachment: local` (or by one element over the run when the rows are not virtualized):
the stripes run unbroken across rows of any height and move with them as the list scrolls.

## Why this arose

The T3 Code desktop app (`1e2ecbd975`, with @pierre/diffs 1.3.0-beta.10) hatches the empty side of an added or deleted
run in its split diff view: one `[data-content-buffer]` element over the run, `repeating-linear-gradient(-45deg,
transparent 0 4.242px, <buffer> 4.242px 5.656px)` in 8px tiles (`#e6e6e6` / `#1d1d1d` over `#fcfcfc` / `#0a0a0a`). The
clone draws its diff as a virtualized list with one item per line (a new file's split view is one run of the whole
file, so a run cannot be one item). It draws the stripes as an SVG pattern per row at a 10pt period, which divides the
20pt line, so the stripes stay unbroken across rows and wrapped lines, where the reference's period is 8 (T3 PR #427,
task files-gutter-parity FG-3; declared in the clone's `EXACT2-GAPS.md`).

## Reproduction

Main `2b5e4a7dc01c2328a3407ab3b8744a4a6fbbe6d6`, `bun scripts/exact.mjs contract build <file>` on each one-file
contract (all four and their output are in the record below):

```text
// A: CSS's own hatch, 45° stripes 2px wide every 8px (5.657px along the gradient line).
component Hatch
  view
    column width=320 height=200 background-color="#fcfcfc" background-image="repeating-linear-gradient(-45deg, transparent 0 4.243px, #e6e6e6 4.243px 5.657px)"
```

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| A: `repeating-linear-gradient()` as above | `contract build`, macOS 26.6.2 | main `2b5e4a7dc` | exit 1: `[lower-attr-value] … repeating-linear-gradient() is not implemented; a gradient paints once` | builds; every host paints Chrome's stripes | record |
| B: the same stripes as `linear-gradient(-45deg, transparent 0px 4.243px, #e6e6e6 4.243px 5.657px)` | `contract build` | main `2b5e4a7dc` | exit 1: `a stop's position is a percentage; lengths are not implemented` | builds and paints as Chrome does (no hatch: past the last stop the end colour fills the box) | record |
| C: a `list virtualized=true` with `background-attachment="local"` | `contract build` | main `2b5e4a7dc` | exit 1: `` `background-attachment="local"` is not a valid `background-attachment`: expected one of "scroll", "fixed" `` | builds; the list's background scrolls with its rows | record |
| D: today's way, an 8px SVG `pattern` in each 20px row of a virtualized list | `contract build` | main `2b5e4a7dc` | builds (8 nodes); each row's stripes start at the row's top, so they break at every row edge | (the limit this issue is about) | record |
| The oracle: A as each 20px row's own background, and as one background on the scroller with `background-attachment: local`, scrolled 13px, one row two lines tall | Chrome 155.0.8059.39, headless, 2x | — | per row: broken at every row edge; on the scroller: unbroken across every row, and it moves with them | (the reference) | image |

![Chrome: a per-row 8px hatch breaks at every 20px row edge; one hatch on the scroller with background-attachment: local runs across them](https://raw.githubusercontent.com/ccheever/exact2/22a6f95318afd6806d86b42fa1cfd231f1eb93dd/fw-issues-20261011m/hatch-chrome.png)

Record (the four contracts, the commands and output, and the Chrome page):
https://raw.githubusercontent.com/ccheever/exact2/fc72646d868267355a042910f991fd2c718231ee/fw-issues-20261011m/hatch-record.txt

Not run: painting on any host (each value fails the build first); Linux, Windows, iOS.

## Constraints

- LLP 1066 D2 refuses rather than approximates: "A host that paints something else than Chrome is a parity bug that
  nothing reports." So a repeating gradient, once admitted, is held to Chrome's pixels on every host, as LLP 1066's
  twelve-case page holds the others (LLP 1055.000 notes Core Graphics and Skia antialias a hard-edged repeating
  gradient differently).
- LLP 1066 D7 refused `local` "since it is the same as `scroll` for a box whose own content does not scroll under its
  background". That holds for a box that does not scroll; on a scroller (a `scroll`, a `list`) `local` is what CSS
  writes for a background that moves with the content.
- `background-attachment: fixed` with a repeating gradient would join the rows too, but its stripes stay still while
  the rows scroll under them, where the browser's (and the app's reference's) move with the rows.
- A hypothesis, not a requirement: a per-row form would do as well if a row could place its background in its list's
  content coordinates, but CSS has no such property; `local` on the scroller is CSS's own way.
- No open or closed issue on main covers repeating gradients, length stops or `local`, and neither do the GitHub
  issues (searched `gradient`, `repeating`, `stripe`, `hatch`, `background-attachment`).

## Acceptance criteria

- A builds, and on the web, macOS and iOS its stripes match Chrome's within LLP 1066's band (a repeating case and a
  length-stop case on the gradients parity page).
- B builds and paints as Chrome paints it.
- C builds, and a hatch on a virtualized list with `background-attachment: local` runs unbroken across rows 20px and
  40px tall and moves with them as the list scrolls, on the web and macOS, as in the image's right half.
- What stays refused (`url()`, `image-set()`, `cross-fade()`) keeps its message.
