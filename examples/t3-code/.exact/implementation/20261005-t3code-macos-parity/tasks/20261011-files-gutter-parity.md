---
name: 20261011-files-gutter-parity
plan: 20261005-t3code-macos-parity
implementation: done
verification: unverified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-files-gutter-parity
pr_url: https://github.com/ccheever/exact2/pull/427
verified_commit: null
---

# The Files surface's gutter, and split view's empty side

## Outcome

[diff-gutter-selection-followups](closed/20261011-diff-gutter-selection-followups.md) (#417, "Observations") saw three
differences from T3 Code (`1e2ecbd975`) outside its rows:

| Id | Clone | Reference |
| --- | --- | --- |
| FG-1 | The Files surface's '+' is the old 16×16 `#1b4ed8` button at the cell's left (x 2–18 of a one-digit gutter), so a press on the middle of number 1 opens a draft instead of selecting. | Pierre's utility button at the number's right edge (cell left + 23.5), 20×20 `#009fff`, the plus in the surface colour (`#ffffff` / `#111111`). #416 made the diff's '+' this way. |
| FG-2 | Lines are selected by press and Shift-press only; a draft opens only from the '+'. | A drag over the numbers selects lines too, and the comment draft opens at the end of any line selection (`FilePreviewPanel` `onLineSelectionEnd` → `beginComment`). |
| FG-3 | In split view, the empty side of an added run is flat `light-dark(#e4e4e4, #222222)`. | It is hatched with diagonal stripes ([image](https://raw.githubusercontent.com/ccheever/exact2/23d03a0b3cd9f69b8da1cdb3822b4b919d9dd476/diff-gutter-selection-followups/r2/gs1-split-open.png)). |
| FG-4 | In the thread's Diff panel a press on the gutter (a line number or the '+') leaves an open skill chip's details open: the gutter's `pointerdown` keeps the press from the window root's outside press (X71, main `issues/20261010-pointer-events-reach-ancestors.md`); handing the press on through `FlowRuns`/the root grew the plan by 777 KB in #413, so it was declared. | An outside press anywhere, the gutter included, closes the chip's details (Base UI's outside press). |

## Steps

1. Confirm each row on the live reference over CDP (`target/t3-audit/ref-app.sh`): the Files surface's '+' box and
   colours, a drag over its numbers and when the draft opens; the split view's empty side (its CSS: the stripe angle,
   width and colours, light and dark).
2. FG-1, FG-2: reuse #413's drag (`diff-line-drag.ts`, `placeUtilityFromSelection`) and #416's utility button for the
   Files surface's rows (`r4-surfaces-files.contract` `R4CodeRow`), so the Files surface and the diff share them. Keep
   #417's GS-2 colours and no text selection during a gutter drag.
3. FG-3: draw the stripes the way Contract allows (a repeating linear gradient if the kernel takes one; otherwise the
   closest supported pattern), with the reference's colours. If Contract cannot draw stripes, record it in
   `EXACT2-GAPS.md` and leave the flat fill.
4. FG-4: close the chip's details from the gutter's own press (the press action the gutter already runs, or the state
   the details read), not by handing the press on to the root, so the plan grows by kilobytes, not the 777 KB of #413's
   try. Report the plan size before and after.
5. Tests; one agent drive of the Files surface, the split view and the Diff panel's chip; before / after / reference
   images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| FG-1..FG-4 | reference comparison (CDP) first; tests; agent drive | before / after / reference images, light and dark |
| The '+' and drag under a real pointer | a real-input step for the next session | — |

## Stopped, then reopened

2026-10-11: stopped by the user before it started ("마지막 세션 1회 후 종료"), then reopened the same day: the user asked
to fix the Known differences that can be fixed now (STATUS "Known differences" rows 1–3 and 6). FG-4 is that list's row 6
(the skill chip's outside press from the gutter).

## Reference (CDP)

T3 Code `1e2ecbd975` on lane `files-gutter-parity` (backend 17100, CDP 17101), the fixture thread's right panel, Playwright
mouse events, light and dark ([reference-cdp.txt](https://raw.githubusercontent.com/ccheever/exact2/ab4ee1ecefb42654c7ff49db0e7dba167912e435/files-gutter-parity/reference-cdp.txt)):

| Row | Reference | Clone before |
| --- | --- | --- |
| FG-1 | Files "+" at cell left + 23.47 (one digit), 20×20, radius 4, `#009fff`, its 16pt plus `#ffffff` / `#111111` | 16×16 at the cell's left (x 2–18), `#1b4ed8` / `#346bf1`, white plus |
| FG-2 | a click on a number: the line selected and the draft open (focused) at the release; a drag 1 → 2: both lines selected while held, the "+" on line 2, the draft at the release; a "+" press-release: its line and the draft | press and Shift-press select; a draft only from the "+"; a drag selects nothing |
| FG-3 | `[data-gutter-buffer]` flat `#f8f8f8` / `#131313`; `[data-content-buffer]` `repeating-linear-gradient(-45deg, transparent 0 4.242px, #e6e6e6 / #1d1d1d 4.242px 5.656px)`, 8px tiles at `5px 0`, one element over the run, over `#fcfcfc` / `#0a0a0a` | the whole cell flat `#e4e4e4` / `#222222` |
| FG-4 | not reachable here (the lane's `$` menu says "No skills found"); Base UI's Popover closes on any outside press | a press on the Diff gutter leaves the details open (the before drive) |

## Implementation

- FG-1, FG-2 (`r4-surfaces-files.contract` `R4CodeRow`, `diff-file-comments.ts`): the Files row takes the diff gutter's
  gestures (#413's `diff-line-drag.ts`: `pressLine`, `pressGutter`, `dragTo`, `releaseDrag`). The gutter cell (a box with
  `pointerdown`/`pointermove`/`pointerup`, and `press` for a keyboard or accessibility press, which also keeps the click
  from the preview's begin-edit) and the "+" hold the pointer and name the line under it by the row's id
  (`fl:<line>:<path>`, `elementFromPoint`); the ops ride the Files comments' queued send (`surface-files-comment-drag`,
  `-drag-shift`, `-gutter`, `-to`, `-end`). The release opens the draft on what it ended on (FilePreviewPanel's
  `onLineSelectionEnd` → `beginComment`), so a click on a number opens one. The "+" is Pierre's utility button (#416's):
  `left=(gutter - 9.83)` (the number's right edge less 1ch + 2pt), 20×20, radius 4, `#009fff`, the plus in
  `light-dark(#ffffff, #111111)`; while a selection is held it sits on its bottom line only (`pin`/`pinned`,
  placeUtilityFromSelection). The number is drawn over the cell after the code with `pointer-events="none"
  user-select="none"` (#417's GS-1), so a press on it is the cell's and a drag selects no text; GS-2's colours are kept. The
  old press-only `line`/`line-shift` ops are gone. The release and the press `blur()`: a pressable node takes the focus on
  macOS and autofocus waits while one holds it, so without it the draft opened unfocused (after drive, run 1).
- FG-3 (`diff-rows.contract` `DiffCell`): the empty side's gutter part `light-dark(#f8f8f8, #131313)`, the row the code
  surface, and over the code part an SVG `pattern` of two polygons in `light-dark(#e6e6e6, #1d1d1d)`: 45° stripes rising to
  the right, the reference's 25 % share and phase. The kernel refuses `repeating-linear-gradient` and the rows are drawn one
  at a time, so the period is 10pt (it divides the 20pt line; an 8pt tile would break at every row's edge, wrapped rows
  included): declared in `EXACT2-GAPS.md` ("Split view's hatched empty side"). Its framework gap is filed on main by
  [#430](https://github.com/ccheever/exact2/pull/430) as `issues/20261011-repeating-hatch-across-list-rows.md` (see
  "Review repair").
- FG-4 (`composer-chip-popover.ts` `closeChipFromPress`, `diff-review.ts`, `pages-pr-code.ts`, `diff-file-comments.ts`):
  the gutters' own press closes the details. A press that starts a drag already reaches the data module on the drags'
  queued send; it now also reads the native editor's newest chip press (`editorChip`) and closes it when open
  (`editorChipClose`, as the window's `chip-close`), and the window's `chip` resource follows `t3.chip`. A primary press that
  starts no drag (a draft open, an empty side) reports itself as `diffreview|press` (`DiffCell`, routed with the drags in
  `app.contract` and `pages-pr-code.contract`) or `surface-files-comment-press` (`R4CodeRow`). Nothing is handed to the
  window root, and the Diff panel, the pull request surface's Code tab and the Files preview all close the details.
- Plan (`contract build` of `app.contract`): base 109,609 nodes, 28,585,446 bytes; FG-1..FG-3 +23 nodes, +19,001 bytes;
  FG-4 +38,250 bytes and no node (measured as the build with and without FG-4's Contract lines: the `press` reports in
  `DiffCell` and `R4CodeRow` and the two routes; its data side costs no plan). #413's try was +777 KB. Final (with the
  release's `blur()`): 109,632 nodes, 28,642,531 bytes, +57,085 against the base. No Rust or Swift changed.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| FG-1 | pass (CDP, tests, agent drive, light and dark): the "+" at x 764.5–784.5 on line 1, `#009fff` with its plus `#ffffff` / `#111111` (the after shots' pixels), as the reference's; before the old 16pt `#1b4ed8` button over the number | [image](https://raw.githubusercontent.com/ccheever/exact2/60285caebac8eb7eca2e347e71fee15363ec59d3/files-gutter-parity/fg1-files-plus.png), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/d0301facecddac7fdd75ea9ff9afdd05dcd30f4f/files-gutter-parity/drive-record.txt) |
| FG-2 | pass (CDP, tests, agent drive, light and dark): a click on number 1 selects it and opens the focused draft; a drag 1 → 2 paints both lines with the "+" on line 2 while held and opens the focused draft "L1 to L2" at the release; before the drag did nothing | [click](https://raw.githubusercontent.com/ccheever/exact2/5e380ecfed62c652c434451068ab5d7b7bfd80a1/files-gutter-parity/fg2-files-click.png), [drag light](https://raw.githubusercontent.com/ccheever/exact2/781d99c7deaef2313ad7b85aa15cc2b07438ebd4/files-gutter-parity/fg2-files-drag-light.png), [drag dark](https://raw.githubusercontent.com/ccheever/exact2/dfc7421e6cc8dd962531e304912c7284cc550e0d/files-gutter-parity/fg2-files-drag-dark.png), [tests-before-after.txt](https://raw.githubusercontent.com/ccheever/exact2/93b3f9c5eb3a6d9578d3a697851d565db38937bd/files-gutter-parity/tests-before-after.txt) |
| FG-3 | pass with a declared stripe period (CDP, tests, agent drive, light and dark): the gutter part `#f8f8f8` / `#131313`, the code part `#fcfcfc` / `#0a0a0a` with `#e6e6e6` / `#1d1d1d` stripes rising to the right, unbroken over wrapped rows; 10pt apart where the reference's are 8 (EXACT2-GAPS; framework gap main [#430](https://github.com/ccheever/exact2/pull/430)) | [image](https://raw.githubusercontent.com/ccheever/exact2/a71fa4553e417d8430c6480d4c122d45c6f20339/files-gutter-parity/fg3-split.png) |
| FG-4 | pass (tests, agent drive; the reference's chip is not reachable in this lane): with the details open, a press on number 2 of the Diff panel's gutter closes them (before: they stay open); a press on the Files gutter closes them too | [Diff gutter](https://raw.githubusercontent.com/ccheever/exact2/13fe828b21dbc433f48c3010cd21ca3c93bc0bf0/files-gutter-parity/fg4-chip-diff-gutter.png), [Files gutter](https://raw.githubusercontent.com/ccheever/exact2/c1042b03812f7bfa8119144e679147073daa15f2/files-gutter-parity/fg4-chip-files-gutter.png) |
| The "+" and drag under a real pointer | open: the real-input steps below | — |

## Tests

- `files-gutter-parity.test.ts` (new, 18): FG-1's geometry and colours against the CDP values (the "+" left evaluated for a
  one-digit gutter: 23.5); FG-2's gestures through `fileComment` (a click, a drag down and up, other files' and non-line
  ids ignored, the "+" press-release and one in flight, a keyboard press, an open draft holding the gutter, the release's
  `blur()`); FG-3's colours and the pattern's period, share and phase; FG-4's `closeChipFromPress` and its calls from the
  Files gutter, `diffReview` (number, Shift, "+", `press`; not `to`/`end`) and `prCodeLocal`, and the routes.
- Updated: `diff-file-comments.test.ts` (the range by a drag), `diff-gutter-selection-followups.test.ts` (GS-2 reads the
  new cell and number), `realinput-1010f-followups.test.ts` (the `press` route).
- `usage-pooled.test.ts`: the Files number and "+" join the list of nodes that take the pointer (the Usage page covers the
  right panel, so its popover is never on screen with them).
- Before (the feature tip's sources with the new file): 1 pass, 16 fail; after: 18 pass.

Checks on `7274e5772` (the branch merged with `feat(example)/t3-code`, already up to date; all exit 0): `bun test
examples/t3-code --timeout 60000` 4471 pass / 1 skip / 0 fail (306 files); strict tsc; `contract build` of `app.contract`
(1398 lines; 109,632 nodes, 28,642,531 bytes); caps; the five checks (cargo build, cargo test 3679 pass / 0 fail / 34
ignored, clippy, fmt, caps, boot). No Rust or Swift changed, so `cargo test -p t3-code-macos --lib` and the AppKit binaries
were not run. The first full Bun run, on `7cbe0429b`, failed one test (`usage-pooled.test.ts`'s list of pointer takers,
which the Files gutter now joins); `7274e5772` adds them. The bundle for the live drive was built from `31f17a3c6` (the
code; the record and that test only after it).

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Reference | T3 Code `1e2ecbd975`, lane 17100/17101 | the table above | [reference-cdp.txt](https://raw.githubusercontent.com/ccheever/exact2/ab4ee1ecefb42654c7ff49db0e7dba167912e435/files-gutter-parity/reference-cdp.txt) |
| Before drive | feature tip `eb9752918` in `t3-code-evidence-base` | the before images; a first try on server port 17102 stopped at the thread tap: the development build takes lane ports 16000–16999 only, so the drives use 16102 | [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/d0301facecddac7fdd75ea9ff9afdd05dcd30f4f/files-gutter-parity/drive-record.txt) |
| After drive, run 1 | `43e001290` | every row as the reference except the Files draft opening without the focus | [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/d0301facecddac7fdd75ea9ff9afdd05dcd30f4f/files-gutter-parity/drive-record.txt) |
| After drive, run 2 (the one retry) | `31f17a3c6` (the release's `blur()`) | the after images; the draft focused | the images above |

## Review repair (independent review of #427)

The review found one should-fix: FG-3 declared a difference (a 10pt stripe period where the reference's is 8) with no
Exact issue number, which the rule of 2026-10-07 requires; the review agreed the limit is real. Repair, with no code or
UI change:

- Duplicates searched: main `issues/` and `issues/closed/` (`gradient`, `repeating`, `stripe`, `background-attachment`),
  the GitHub issues (`gradient`, `repeating`, `stripe OR hatch OR pattern`) and open PRs (`gradient`): none. Main's
  `QUEUE.md` names repeating gradients and length stops in one line ("The document window, what the studio pass left"),
  which decides nothing.
- Reproduced on main `2b5e4a7dc` with four one-file contracts and `bun scripts/exact.mjs contract build`:
  `repeating-linear-gradient(-45deg, transparent 0 4.243px, #e6e6e6 4.243px 5.657px)` exits 1 ("repeating-linear-gradient()
  is not implemented; a gradient paints once"), the same stripes with length stops exit 1 ("a stop's position is a
  percentage; lengths are not implemented"), `background-attachment="local"` on a virtualized list exits 1 (expected
  "scroll" or "fixed"), and an 8px SVG pattern per 20px row builds (and breaks at every row edge). Chrome 155 paints one
  repeating gradient on the scroller with `background-attachment: local` unbroken across rows 20 and 40px tall
  ([image](https://raw.githubusercontent.com/ccheever/exact2/22a6f95318afd6806d86b42fa1cfd231f1eb93dd/fw-issues-20261011m/hatch-chrome.png),
  [record](https://raw.githubusercontent.com/ccheever/exact2/fc72646d868267355a042910f991fd2c718231ee/fw-issues-20261011m/hatch-record.txt)).
- Filed on main as `issues/20261011-repeating-hatch-across-list-rows.md` by draft PR
  [#430](https://github.com/ccheever/exact2/pull/430) (branch `issues/t3-clone-gap-repeating-hatch`, the user's
  2026-10-10 process for a new framework gap); `EXACT2-GAPS.md` and FG-3's rows link it. The plan has no X record for it
  yet: the coordinator merges #430, adds the mapping row to the plan's `issues/README.md` and tells Charlie.
- Checks: main `bun scripts/issue.mjs check` and caps pass; this branch's change is Markdown only (the five checks once,
  below).

## Real-input batch steps

The clone's packaged or development build (a lane home and port), the fixture thread "Timeline verification", a real mouse:

1. Right panel › Files › `fixture.txt`. Hover line 1's code: a blue 20pt "+" at the number's right edge. Click the middle
   of number 1: line 1's number cell turns blue and the comment draft opens under it with the caret in it (type a letter:
   it lands in the draft). Cancel.
2. Press number 1 and drag down to number 2: both lines turn blue as the pointer moves, the "+" sits on line 2, the code
   shows no text selection; release: the draft "L1 to L2" opens, focused. Cancel.
3. Hover line 2, press its "+" and release: line 2 is selected and its draft opens. Cancel.
4. Type `$frontend-design now` in the thread's composer and click the chip: its details open. With the Diff surface open
   (Changes, `.agents/skills/verify/SKILL.md` expanded), click number 2: the details close. Open them again and click the
   Diff's "+" on a hovered line, then (Files surface) a Files number: each closes them.
5. Diff › Split diff view: the empty left side of `SKILL.md` is hatched with stripes rising to the right, unbroken across
   the wrapped lines; switch the system appearance to dark and back: the stripes follow.

## Not done / not verified

- Under a real pointer: the steps above (the agent's drags are mouse events through `NSApplication.sendEvent`, the host's
  `mouseDown`/`mouseDragged` path a hand takes).
- FG-3's framework gap is filed by main PR [#430](https://github.com/ccheever/exact2/pull/430), a draft until the
  coordinator merges it (the issue file lands on main only then).
- The reference's skill chip details (FG-4) could not be opened in this lane (no provider, so no skill in its `$` menu);
  the row is checked against the reference's source (Base UI Popover's outside press).

## Delivery

Draft PR [#427](https://github.com/ccheever/exact2/pull/427) into `feat(example)/t3-code`.

## Next action

Coordinator review of the draft PR; merge main PR #430 (FG-3's framework gap) and map it in the plan's issues; the
real-input steps go to the final session.
