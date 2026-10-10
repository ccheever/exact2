---
name: 20261011-diff-gutter-visuals
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-diff-gutter-visuals
pr_url: https://github.com/ccheever/exact2/pull/416
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

## Reference (CDP)

T3 Code `1e2ecbd975` on lane `diff-gutter-visuals` (backend 16740, CDP 16741), paired with the GitHub lane's primary
server (16743, no GitHub write). `getComputedStyle` in the diffs' shadow roots while a Playwright drag over CDP was held,
light and dark (`emulateMedia`), in #168's Code tab (docs/usage.md, stacked and split) and the fixture thread's Diff panel
(Cargo.toml, one digit; Cargo.lock, four digits). The values are the same on both surfaces and in both layouts
([reference-cdp.txt](https://raw.githubusercontent.com/ccheever/exact2/4a31237ec5a671d4a53e43239a859a2ca5a1ee77/diff-gutter-visuals/reference-cdp.txt)):

| What | Light | Dark | Clone before |
| --- | --- | --- | --- |
| Selected code row (`[data-line]`) | `#eff7fd` | `#0f151a` | `#fef3c7` / `#3a3112` |
| Selected number cell (`[data-column-number]`) | `#f0f7fd` | `#10151a` | `#fde68a` / `#4a3d14` |
| Selected number | `#20649e` | `#72b6ff` | the change colour, else grey |
| Bar (`::before`, 4px at the cell's left; replaces the change bar on selected rows) | `#009fff` | `#009fff` | the change bar (none on context) |
| Comment card under a selected line (`[data-line-annotation]` / its gutter buffer) | row / cell colour, the bar runs through | same | not painted |
| "+" (`[data-utility-button]`) | 20×20 (1lh), radius 4, `#009fff`, 16×16 plus in `#fcfcfc` | plus `#0a0a0a` | 16×16 at the cell's left over the bar, `#1b4ed8` / `#346bf1`, white plus |
| "+" left | the number's right edge: cell left + width − (1ch + 2): 31.3 in a 41.1 gutter, 23.5 in 33.3, 47.0 in 56.8 | same | cell left + 4 |

Pierre's `renderSelection` marks the annotation that follows a selected line, so a comment card under a selected line is
painted as one (`StyledDiffCodeView` draws the bar through it). T3's `--diffs-modified-base` is `#009fff` in both schemes.

## Implementation

- `diff-rows.contract`: `diffSel(role)` holds the four colours. `DiffCell` paints a selected row, its number cell and its
  number with them and draws the selection's 4pt bar in place of the addition/deletion bar. The "+" is Pierre's utility
  button: `left=((gutter - 2) * size / 13 - 7.83 * size / 13)` (the number cell's width less its 1ch + 2pt inset, i.e.
  the number's right edge), `top=0`, 20×20, radius 4, `#009fff`, with Pierre's 16-unit plus path in
  `light-dark(#fcfcfc, #0a0a0a)` (it replaces `DiffGlyph`, so the plan is 51 KB smaller: 28,573,504 bytes against the
  base's 28,624,818, 631 fewer nodes). #413's handlers, `pin`/`pinned`/`dragging` and its mount rule are unchanged.
- What sits under a selected line: `pages-pr-code-rows.ts` (`annotate`) and `diff.ts` mark the conversation, pending and
  draft items, and the Diff panel's saved comment and draft, `selected` when the line they follow is selected. The Code
  tab's rows (`pages-pr-code.contract`) and the Diff panel's (`DiffRow`) paint them with the row's colour and the bar;
  `DiffDraftCard` and `PrdCodeDraft` take a `selected` prop for their own background (the Files surface passes false).
- Side effect, also the reference's: in a one-digit gutter the hidden "+" no longer covers the number's middle. Before, a
  press there (the agent's tap at the cell's centre, or a real click) landed on the "+" and opened a draft in the Diff
  panel instead of selecting (the before drive shows it).

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| Selection colour and "+" position match the reference | pass (CDP values, tests, one agent drive, light and dark, both surfaces): before amber rows and the "+" over the change bar at the cell's left; after the reference's blue row, cell, number and bar (through the comment cards), and the "+" at x 672.5–692 in the Code tab (reference 673–692) and cell left + 23.5 in the Diff panel (reference 23.5 for one digit) | [PR light](https://raw.githubusercontent.com/ccheever/exact2/c519fbb15bcc0fd9d2736abd18e9032587dc3c80/diff-gutter-visuals/pr-code-light.png), [PR dark](https://raw.githubusercontent.com/ccheever/exact2/d43b2d696a148f22dfd5925e10407ee4cffd4149/diff-gutter-visuals/pr-code-dark.png), [Diff light](https://raw.githubusercontent.com/ccheever/exact2/2f570430e967e8369c9513b8ab414b54bf648b19/diff-gutter-visuals/thread-diff-light.png), [Diff dark](https://raw.githubusercontent.com/ccheever/exact2/4cd7d931eb7db606269bcf8ef37bd0839584a43f/diff-gutter-visuals/thread-diff-dark.png), [reference-cdp.txt](https://raw.githubusercontent.com/ccheever/exact2/4a31237ec5a671d4a53e43239a859a2ca5a1ee77/diff-gutter-visuals/reference-cdp.txt), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/27340417a960740492d0771815fbb2b235094a3a/diff-gutter-visuals/drive-record.txt) |
| #413's drag and click still work | pass: its tests (`realinput-1010f-followups.test.ts`, `diff-line-drag.test.ts`, the gutter tests in `diff-review.test.ts` and `pages-pr-code.test.ts`) pass unchanged; in the after drive the number drag 3→added 5 paints the range with the "+" on added 5 while held and opens the draft at the release (Cancel closes it), and the Diff panel's drag 2→4 selects with no draft and the "+" pinned on 4 | [tests-before-after.txt](https://raw.githubusercontent.com/ccheever/exact2/0c813289d8c1fe4178db0ba9f5cc53902c347fb7/diff-gutter-visuals/tests-before-after.txt), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/27340417a960740492d0771815fbb2b235094a3a/diff-gutter-visuals/drive-record.txt) |

Split view was compared over CDP only (the same tokens and the same "+" offset on each side); the clone's split rows use the
same `DiffCell`, and the drive did not open the split layout.

## Tests

- `diff-gutter-visuals.test.ts` (new, 6): the selection colours are the CDP values; the row, the number cell, the number
  and the bar use them and the change bars yield on a selected row; the comment rows' tint and bar in both panels and the
  two drafts' `selected` backgrounds; the "+" geometry and colours; its left edge evaluated at the default size equals the
  reference's offsets (31.3 for a 41.1 gutter, 23.5 for 33.3); #413's handlers and mount rule unchanged.
- `pages-pr-code.test.ts` (1 new): a drag over lines 1–2 marks the conversations under deleted 2 and added 2, not the one
  under 3; the draft at the release; Cancel clears them.
- `diff-review.test.ts` (1 new): the draft under its range; a saved comment once its line is selected again, not when the
  selection stops above it.
- Before (the feature tip's sources with these files): 7 fail, 34 pass; after: all pass.

Checks on `fac65ca14` (the branch merged with `feat(example)/t3-code`, already up to date; all exit 0): `bun test
examples/t3-code --timeout 60000` 4444 pass / 1 skip / 0 fail (303 files); strict tsc; `contract build` of `app.contract`
(1397 lines; 109,609 nodes, 28,573,504 bytes); caps; the five checks (cargo build, cargo test 3679 pass / 0 fail / 34
ignored, clippy, fmt, caps, boot). No Rust or Swift changed, so `cargo test -p t3-code-macos --lib` and the AppKit binaries
were not run. The bundle for the live drive was built from `149170af0` (the code; the record only after it).

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Reference | T3 Code `1e2ecbd975`, lane `diff-gutter-visuals` (16740/16741), the GitHub lane on 16743 | the table above | [reference-cdp.txt](https://raw.githubusercontent.com/ccheever/exact2/4a31237ec5a671d4a53e43239a859a2ca5a1ee77/diff-gutter-visuals/reference-cdp.txt) |
| Before drive | feature tip `ca034e055`, built in worktree `t3-code-diff-gutter-visuals-before` (the shared evidence base `c03d7e908` predates #413's drag) | amber; the Diff panel's press opened a draft on the hidden "+". Three runs to settle the Diff panel's steps (the fixture thread's Changes in the clone hold `.agents/…/SKILL.md`, `.claude/…/SKILL.md` and `fixture.txt`, not Cargo.lock; the "+" read is after-only) | [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/27340417a960740492d0771815fbb2b235094a3a/diff-gutter-visuals/drive-record.txt) |
| After drive (one, no retry) | `149170af0`, bundle built from it | the images above | the images above |

## Not done / not verified

- Nothing of this record. No real-input steps: the rows are paint and position, which the agent's window screenshots
  show; #413's real-input steps (its record) still cover the drag under a real pointer.
- Observations for the coordinator, not findings of this record: (1) the agent's drag over the numbers also paints a
  native text selection over the code of the rows it crosses, in both builds (the reference's gutter drag selects no
  text); (2) the Files surface's line selection (`r4-surfaces-files.contract`) is still amber, and the reference's file
  preview paints its own selection (`fileSurfaceChrome.tsx`); (3) the fixture thread's Diff "Changes" reads +442k −1
  against `origin/main` in the reference (its own 0.0.45 backend) and +11 −0 in the clone (the embedded 0.0.46 server).

## Delivery

Draft PR [#416](https://github.com/ccheever/exact2/pull/416) into `feat(example)/t3-code`.

## Next action

The coordinator reviews the draft PR.
