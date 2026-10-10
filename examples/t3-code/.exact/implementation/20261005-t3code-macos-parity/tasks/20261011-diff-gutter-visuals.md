---
name: 20261011-diff-gutter-visuals
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

# The diff gutter's selection colour and '+' position

## Outcome

[realinput-1010f-followups](20261010-realinput-1010f-followups.md) RF-3 (#413) gave the diff gutter its line drag. While
comparing it with T3 Code (`1e2ecbd975`) over CDP, the build saw two visual differences it did not change:

1. Selected lines (a click on a line number, or a drag across numbers) are amber in the clone and blue in the reference.
2. The reference draws the '+' (add a comment) to the right of the line number; the clone draws it over the change bar on
   the left.

Both are in the PR Code tab and the thread's Diff panel. Start after #413 merges (`diff-rows.contract` is the same file).

## Steps

1. On the live reference over CDP (`target/t3-audit/ref-app.sh`; the GitHub lane's #168 Code tab, and a thread's Diff
   panel in the plain lane), read the computed styles of a selected line (background, the number's colour, the border
   or bar) and the '+''s box (left, top, size, colour), in light and dark, inline and in the split view.
2. Make the clone's selection and '+' match: colours from the reference's tokens, the '+' placed where the reference
   places it. Keep #413's drag and click handling unchanged.
3. One agent drive per surface (PR Code tab, thread Diff) in light and dark, with a line selected and the '+' showing.

## Acceptance

| Row | How to verify | Before/after |
| --- | --- | --- |
| Selection colour and '+' position match the reference | computed styles from CDP against the clone's values; agent screenshots | before / after / reference images, light and dark |
| #413's drag and click still work | its tests; the same drive | test output |

## Next action

Start after realinput-1010f-followups (#413) merges.
