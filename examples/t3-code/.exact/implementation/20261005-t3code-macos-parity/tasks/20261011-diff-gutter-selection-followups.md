---
name: 20261011-diff-gutter-selection-followups
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

# The gutter drag's text selection, and the Files surface's line selection colour

## Outcome

[diff-gutter-visuals](closed/20261011-diff-gutter-visuals.md) (#416) saw two differences outside its rows while
comparing with T3 Code (`1e2ecbd975`):

| Id | Clone | Reference |
| --- | --- | --- |
| GS-1 | A drag over the diff gutter's line numbers (PR Code tab and thread Diff panel) also paints a native text selection over the code, in both the before and after builds. | The gutter drag selects lines and no text. |
| GS-2 | The Files surface's line selection is still amber (`r4-surfaces-files.contract`). | Its selected lines use the same blue as the diff (#416's `diffSel()` colours). |

## Steps

1. GS-1: confirm on the reference over CDP that a gutter drag leaves `window.getSelection()` empty. In the clone, find
   why the drag selects text (the gutter's `pointerdown` does not prevent the default text selection, or the code
   column's `user-select` lets the drag start a selection) and stop it for drags that start on the gutter only. A drag
   inside the code must still select text.
2. GS-2: read the Files surface's selected-line colours on the reference (CDP computed styles, light and dark) and use
   them, sharing #416's colours where they are the same.
3. Tests; one agent drive of each surface; before / after / reference images.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| GS-1 | agent drive: a gutter drag leaves no text selection; a drag in the code still selects text | before / after / reference images |
| GS-2 | CDP computed styles against the clone's; agent screenshots, light and dark | before / after / reference images |

## Next action

Start now.
