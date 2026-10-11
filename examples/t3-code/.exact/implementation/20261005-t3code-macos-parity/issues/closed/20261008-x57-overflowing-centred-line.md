---
name: 20261008-x57-overflowing-centred-line
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: [20261008-pr-list-title-clip]
upstream_url: https://github.com/ccheever/exact2/issues/291
reproduced_on: 0365ad1a4 (main)
---

# X57: macOS centres a line wider than its box, cutting its start (CSS start-aligns it)

Moved to main `issues/20261009-apple-overflow-line-alignment.md` (2026-10-09); tracked there.

## Summary

CSS Text 3 §7.1 start-aligns a line too long for its box, whatever `text-align` says. The macOS host
aligns an over-wide line by its `text-align` anyway. Under `center` (a `button`'s UA value, which its
text inherits) the line starts left of the box: its start is clipped, and with `text-overflow:
ellipsis` the "…" sits early, with blank space after it. The web host start-aligns, so one Contract
source shows two different strings.

## Why it arose

### The T3 Code behavior

The Pull Requests list row is a `<button>` with `text-left` (`PULL_REQUEST_ROW_CLASS`,
`apps/web/src/components/pullRequest/PullRequestListRow.tsx:28`); its title is `truncate`. In Chrome,
a title too long for the list column reads "Describe the vowel count…". Chrome 151 start-aligns the
same overflowing title under `text-align: center` too (a reset button, and a plain UA button).

### Where the clone hits it

Every `button` in the clone centres its text unless the author says otherwise. Task
[pr-list-title-clip](../../tasks/closed/20261008-pr-list-title-clip.md): the Pull Requests row's title
lost its start once it was longer than the list column (after a rename, or a long title on its first
read). Elsewhere in the clone, buttons whose `text-overflow: ellipsis` text inherits the centre, found
by reading the sources: `diff.contract:140`, `r4-surfaces-files.contract:175`,
`r4-surfaces.contract:193`, `redacted-text.contract:27`, `usage-bars.contract:135`,
`usage-pooled.contract:363`; about 60 more `line-clamp=1` texts in buttons inherit it too (a single
word wider than the box is cut the same way; a clamped line is centred). Each loses its start only when
its text overflows.

## Clone workaround

The clone says `text-align="left"` where the reference says `text-left` (`PrRowButton`, the timeline
group toggle, `PrdCopy`) and on the base freshness mark and the filter submenu's value (the same
rendering as Chrome). Once main start-aligns overflowing lines, the reference-mirroring sites stay; the
freshness mark's and the submenu value's `text-align="left"` may go, and the other sites above are
correct without edits.

## Evidence and history

- Local draft (2026-10-08, pr-list-title-clip), reproduced in a one-file app on the feature branch's
  framework (main `1f19b2400`): macOS read "cribe the vowel count…" in the button and the centred row,
  "Describe the vowel count…" in the `text-align="left"` row and "ribe the vowel counter (ec" in the
  clipped line; the web started all four at "Describe".
- Evidence: [03-x57-one-file-app.png](https://raw.githubusercontent.com/ccheever/exact2/0d0833be4ef61ea1abf087307568f953cc7cdee7/pr-list-title-clip/03-x57-one-file-app.png)
  (macOS beside web, same Contract; [the app's view](https://raw.githubusercontent.com/ccheever/exact2/2bbdddc39babadda9b2c5e05dc0aba37cbb20162/pr-list-title-clip/x57-app.contract.txt))
  and [04-chrome-151-same-css.png](https://raw.githubusercontent.com/ccheever/exact2/d36af12402ccb5430fd2fd323214d57a81375cf6/pr-list-title-clip/04-chrome-151-same-css.png)
  (Chrome 151 headless, [the CSS](https://raw.githubusercontent.com/ccheever/exact2/7ca993368f4f02df386e74523846c09daf596f37/pr-list-title-clip/chrome-case.html.txt):
  centred, `text-left` and a UA button all start at "Describe").
- Filed as [#291](https://github.com/ccheever/exact2/issues/291) ([Bug] macOS: a centred line wider than
  its box is centred and clipped at its start (CSS start-aligns it)) on 2026-10-08, reproduced on main
  `0365ad1a4` with the same one-file app (`layout` reporting `text_align = center (inherited from #2)`).
  No duplicate found (#128 and #266 are other text differences).
- Accepted upstream on 2026-10-08 as a correctness fix ("Start-align overflowing lines consistently in
  all text paths").
