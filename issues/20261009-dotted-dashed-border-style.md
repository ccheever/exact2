# `border-style`: `dotted` and `dashed`

**Status:** Open
**Systems:** kernel schema, Contract, GUI hosts
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/287

## Current scope

Implement approved dotted/dashed borders from schema.json, compared with Chrome for mixed sides, widths and rounded corners. Preserve layout widths; other styles are not admitted.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

Dotted and dashed borders are common in desktop UI:

- a dotted underline marks an abbreviation or a "+N more" toggle;
- a dashed outline marks a drop zone or an empty slot.

CSS's `border-style` takes `dotted` and `dashed` (and `double`, `groove`, `ridge`, `outset`). Contract's border rows accept `none | hidden | solid | inset` only. LLP 1001 says "Other line styles are refused until a consumer needs their painting" (`llp/1001-kernel-v1.spec.md:100-103`). This issue is that consumer request for `dotted` and `dashed`.

### Current and expected behavior

- **Current:** `border-bottom-style="dotted"` fails `contract build` with `lower-attr-value`: "`border-bottom-style=\"dotted\"` is not a valid `border-bottom-style`: expected one of \"none\", \"hidden\", \"solid\", \"inset\"". `dashed` is refused the same way. `contract vocab border-style` lists `enum none|hidden|solid|inset`.
- **Expected:** `dotted` and `dashed` on `border-style` and `border-<side>-style`, painted as Chrome paints them (dot and dash length from the border width, corners as Chrome joins them) on every host, with Chrome as the oracle.

### Reproduction and evidence

In an app made by `bun scripts/exact.mjs new <dir>`:

```text
component Dotted
  view
    main testId="root"
      text "+3" border-bottom-width=1 border-bottom-style="dotted" border-bottom-color="#888888" testId="more"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| `dotted` | `bun exact.mjs contract build dotted.contract --json` | compiler (any host) | main `0365ad1a4` | `lower-attr-value` (expected one of none, hidden, solid, inset) | `[]`, and a 1 px dotted underline as in Chrome | compiler JSON |
| `dashed` | the same file with `dashed` | compiler | same | `lower-attr-value` | `[]` | compiler JSON |
| Vocabulary | `bun exact.mjs contract vocab border-style` | any | same | `enum none\|hidden\|solid\|inset, default none` | includes `dotted`, `dashed` | CLI output |

`kernel/tables/schema.json`'s border-style rows are unchanged between `0365ad1a4` and main `e200397ec`.

### Acceptance criteria

- `border-style`, `border-<side>-style` and the shorthand accept `dotted` and `dashed`. A bound value is checked against the same set.
- Native hosts paint them as Chrome does for 1, 2 and 4 px widths, on straight and rounded sides. The web emits CSS.
- `border_widths()` and layout treat them as visible styles (they keep their widths), as `solid` does.

### Constraints and related work

- Workaround: a `solid` border in the same colour, or an SVG `line` with `stroke-dasharray` beside the text. The SVG keeps the look, but the line is laid out outside the text box.
- `double`, `groove`, `ridge` and `outset` are not requested here.
- Related: LLP 1001 §Border semantics; LLP 1077 (CSS visual properties native hosts draw).

## Discussion at transfer

### ccheever — 2026-10-08T08:07:32Z

**Decision: Admit dotted and dashed borders for the named consumer.**

Keep open with the bounded scope below.

LLP 1001 explicitly waits for a consumer; this request supplies one. Keep the scope to these two CSS styles.

Use schema.json as the authority and Chrome fixtures for side widths, mixed sides and rounded corners. Preserve border widths in layout; do not add the other unrequested border styles.

## Implementation (2026-10-11)

Contract and the schema admit `dotted` and `dashed` on border longhands,
four-sided `border-style`, and `border`/`border-<side>` shorthands. The kernel
keeps their widths in layout; both web targets emit the authored CSS for Chrome
to paint. AppKit, UIKit and Linux paint patterned sides with separate straight
side strokes or a rounded centreline and compare twelve 1× cases against the
web screenshot. AppKit and Linux pass the 1× raster band declared in LLP 1001.
The driven Mac app's 2× screenshot, reduced to the Chrome fixture size,
differs by 1.19/255 mean RGB and 0.98% of pixels beyond 48 over the full page;
the Linux app's 1× screenshot differs by 0.60/255 and 0.65% respectively.

Review fix (2026-10-11): Linux now clips patterned ink to the rounded border
ring as well as the side join; a bottom-only dotted border test catches ink
outside the outline and inside the padding. Linux and Apple parity checks now
require ink and gaps on the 1 px dotted underline itself. A blank or solid
underline no longer passes by being averaged over its 100×70 box.

Review fix round 3 (2026-10-11): the schema gives column rules a separate
`none | hidden | solid` enum, so a bound value cannot switch to a pattern that
native column-rule painters omit. Linux and Apple reverse all four thin dotted
side paths to match Chrome's 1 px phase. Their parity checks now require ink
and gaps on each edge of the 1 px dotted and dashed boxes, and compare the
1 px dotted phase against Chrome on each edge. The driven Mac app's current
2× screenshot, reduced to the Chrome fixture size, differs by 0.59/255 mean
RGB and 0.65% of pixels beyond 48; `dot1` differs by 0.043/255 mean.

Review fix round 5 (2026-10-12): Linux round dots no longer shorten their
spacing by 0.01 px per dot; the 2,000 px side regression test checks the final
dot's position. Reproduced after rebasing onto `63ad4b82e`: restoring the old
formula makes `round_dots_keep_their_period_along_a_long_side` fail with the
last dot ending at 1995.4999 px instead of 1998 px.

## What remains

Drive the border fixture in the iOS app, then close this issue. All four UIKit
border parity tests passed on an iOS 27.0 simulator on 2026-10-11, including
the 1 px dotted underline. The earlier iOS app bundle built, but its fixture
drive still never opens the agent socket; iOS 26.4 and 26.5 did the same before
the painter. The app-level iOS screenshot remains unmeasured. The round-3
UIKit test bundle built on 2026-10-12, but `test-without-building` first lost
its simulator destination; a targeted retry on iOS 26.5 stalled in its first
test, and an iOS 27.0 simulator could not boot (`launchd_sim` did not respond).
The new thin-edge guards have not run on UIKit.
