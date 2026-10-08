---
name: 20261008-x57-overflowing-centred-line
plan: 20261005-t3code-macos-parity
status: published
kind: framework-gap
blocks: [20261008-pr-list-title-clip]
upstream_url: https://github.com/ccheever/exact2/issues/291
reproduced_on: 0365ad1a4 (main)
---

# X57: macOS centres a line wider than its box, cutting its start (CSS start-aligns it)

## Summary

CSS Text 3 §7.1 (`text-align`): "If (after justification, if any) the inline contents of a line box
are too long to fit within it, then the contents are start-aligned: any content that doesn't fit
overflows the line box's end edge." The macOS host aligns an over-wide line by its `text-align`
anyway. Under `center` (a `button`'s UA value, LLP 1001 §1, which its text inherits; `right` takes the
same offset, not driven) the line starts left of the box: its start is clipped, and with `text-overflow: ellipsis` the "…" sits
early, with blank space after it. The web host (the browser) start-aligns, so one Contract source
shows two different strings.

## Why this issue arose

### The T3 Code behavior

The Pull Requests list row is a `<button>` with `text-left` (`PULL_REQUEST_ROW_CLASS`,
`apps/web/src/components/pullRequest/PullRequestListRow.tsx:28`); its title is `truncate`. In Chrome,
a title too long for the list column reads "Describe the vowel count…". Chrome 151 start-aligns the
same overflowing title under `text-align: center` too (a reset button, and a plain UA button).

### What exact2 does today

Repro, a one-file app (`bun scripts/exact.mjs new`, then this `app.contract` view):

```contract
button width=240 padding=6 display="flex" flex-direction="row" align-items="center" gap=8
  text "#159" flex-shrink=0
  text "Describe the vowel counter (edited)" min-width=0 white-space="nowrap" overflow="hidden" text-overflow="ellipsis" flex-shrink=1
  text "✓" flex-shrink=0
button … text-align="left"                       // the same row, start-aligned by the author
row … text-align="center"                         // the same row in a plain container
text "Describe the vowel counter (edited)" width=160 white-space="nowrap" overflow="hidden" text-align="center"
```

- `bun exact.mjs agent macos --size 420x320 "screenshot x57-macos.png"`: the button row and the
  centred row read "cribe the vowel count…" with a gap before the check; the `text-align="left"` row
  reads "Describe the vowel count…"; the clipped line without an ellipsis reads "ribe the vowel
  counter (ec" (both ends cut).
- `bun exact.mjs agent web --size 420x320 "screenshot x57-web.png"`: all four start at "Describe".
- `layout <title>` on macOS: `text_align: center`, `source: inherited`, `from:` the button.

Where (read, not changed): `ParagraphGeometry.swift` `Paragraph.origin(_:align:width:)` and
`TextRaster.swift` `TextRasterJob.render` place each line at
`CTLineGetPenOffsetForFlush(line, flush, width)` of the line as broken. For an over-wide line that
offset is negative under flush 0.5 or 1. The ellipsis is made afterwards (`Paragraph.ellipsized`,
the raster job's `ellipsis` pass) at the box width, and it is drawn at that negative offset. The same
flush offset is taken in `RegionRaster.swift`, `RegionTextGeometry.swift`, `RegionWorkerLayout.swift`
and `TextFlow.swift`, and `IOS/TextRasterIOS.swift` passes the same flush, so iOS and tvOS are likely
affected too (not driven).

Evidence: [03-x57-one-file-app.png](https://raw.githubusercontent.com/ccheever/exact2/0d0833be4ef61ea1abf087307568f953cc7cdee7/pr-list-title-clip/03-x57-one-file-app.png) (macOS beside web, same
Contract; [the app's view](https://raw.githubusercontent.com/ccheever/exact2/2bbdddc39babadda9b2c5e05dc0aba37cbb20162/pr-list-title-clip/x57-app.contract.txt)) and
[04-chrome-151-same-css.png](https://raw.githubusercontent.com/ccheever/exact2/d36af12402ccb5430fd2fd323214d57a81375cf6/pr-list-title-clip/04-chrome-151-same-css.png) (Chrome 151 headless,
[the CSS](https://raw.githubusercontent.com/ccheever/exact2/7ca993368f4f02df386e74523846c09daf596f37/pr-list-title-clip/chrome-case.html.txt): centred, `text-left` and a UA button all start at "Describe").

### Where the clone hits it

Every `button` in the clone centres its text unless the author says otherwise. Task
`20261008-pr-list-title-clip`: the Pull Requests row's title lost its start once it was longer than
the list column (after a rename, or a long title on its first read). The clone now says `text-left`
where the reference does (`PrRowButton`, the timeline group toggle, `PrdCopy`) and on the base freshness
mark and the filter submenu's value (the same rendering as Chrome). Elsewhere in the clone, buttons
whose `text-overflow: ellipsis` text inherits the centre, found by reading the sources:
`diff.contract:140`, `r4-surfaces-files.contract:175`, `r4-surfaces.contract:193`,
`redacted-text.contract:27`, `usage-bars.contract:135`, `usage-pooled.contract:363`; about 60 more
`line-clamp=1` texts in buttons inherit it too (a single word wider than the box is cut the same way;
a clamped line is centred). Each loses its start only when its text overflows.

## Why it must be resolved

The web is the standard (`CLAUDE.md`): a kernel or host that disagrees with a bare `<div>` brings
back the disagreeing-defaults bug class. Here the start of a label, the part a reader needs, is the
part that disappears, and the author only finds out on macOS. A clone that matches Chrome has to
repeat `text-align: left` on every truncating label inside a button, including where the reference
needs none.

## Requested support

When a line is wider than its box, lay it out start-aligned (direction-aware), whatever `text-align`
says, before the ellipsis is made: paint, raster, selection and hit-testing alike.

## Acceptance for the fix

The repro's four lines start at "Describe" on macOS, as on the web; `layout` of the centred title
reports its glyphs from the box's start edge; a centred line that fits is unchanged.

## App adoption after resolution

Nothing to remove: the clone's `text-align="left"` lines match the reference's `text-left`. The
freshness mark's and the submenu value's may go; the other sites above are then correct without edits.

## Status and next action

Local draft (2026-10-08, pr-list-title-clip). Reproduced on a one-file app on the feature branch's
framework (main `1f19b2400`); main `c81074f19` keeps both offsets unchanged (read, not run). Not
searched upstream, not published: publication needs the user's approval (`issue-open`).

## Upstream (filed 2026-10-08)

Upstream: https://github.com/ccheever/exact2/issues/291 (#291, [Bug] macOS: a centred line wider than its box is centred and clipped at its start (CSS start-aligns it)). Reproduced on main `0365ad1a4` (the text-placement files are unchanged on main `e200397ec`) with the one-file app above, the greeting resource left out. macOS reads "cribe the vowel count…" in the button and the centred row, "Describe the vowel count…" in the `text-align="left"` row and "ribe the vowel counter (ec" in the clipped line, with `layout` reporting `text_align = center (inherited from #2)`. The web starts all four at "Describe". Searched: text-align center, ellipsis, truncated, start-aligned (issues and PRs): no duplicate (#128 and #266 are other text differences).

Next: issue-close once #291 lands; the freshness mark's and the submenu value's `text-align="left"` may then go.

## Decided upstream (2026-10-08): waits for main fix of #291

[Charlie on #291](https://github.com/ccheever/exact2/issues/291#issuecomment-6055582400): "Start-align overflowing lines consistently in all text paths."
- Waits for main fix of [#291](https://github.com/ccheever/exact2/issues/291), then an adoption round. The clone's `text-align="left"` sites stay (they mirror the reference's `text-left`).
- [#327](https://github.com/ccheever/exact2/pull/327) audit (open on main, 2026-10-08): attempt withdrawn (RTL ellipsis geometry still failed); #291 stays open.
