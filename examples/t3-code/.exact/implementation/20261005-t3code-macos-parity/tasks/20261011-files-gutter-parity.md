---
name: 20261011-files-gutter-parity
plan: 20261005-t3code-macos-parity
implementation: planned
verification: unverified
delivery: none
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: null
pr_url: null
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
4. Tests; one agent drive of the Files surface and the split view; before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| FG-1..FG-3 | reference comparison (CDP) first; tests; agent drive | before / after / reference images, light and dark |
| The '+' and drag under a real pointer | a real-input step for the next session | — |

## Next action

Start now.
