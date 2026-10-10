---
name: 20261011-diff-gutter-selection-followups
plan: 20261005-t3code-macos-parity
implementation: implemented
verification: verified
delivery: draft-pr
repository: https://github.com/ccheever/exact2
base_branch: feat(example)/t3-code
branch: feat(example)/t3-code-diff-gutter-selection-followups
pr_url: https://github.com/ccheever/exact2/pull/417
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

## Reference (CDP)

T3 Code `1e2ecbd975` on lane `diff-gutter-selection-followups` (backend 16780, CDP 16781), paired with the GitHub lane's primary
server (16783, no GitHub write). Playwright mouse drags over CDP; `window.getSelection()` and the diffs' shadow root's
`getSelection()` read while held and after; `getComputedStyle` in the shadow roots, light and dark
([reference-cdp.txt](https://raw.githubusercontent.com/ccheever/exact2/4a8c1873117cfa1813e864fbf66f8b0f726188bb/diff-gutter-selection-followups/reference-cdp.txt)):

| What | Reference | Clone before |
| --- | --- | --- |
| Number cell `[data-column-number]` and its content | `user-select: none`; the code row `[data-line]` `auto` | the number a selectable `text` inside the gutter cell |
| Drag over the numbers (Diff panel Cargo.lock 4727→4730; #168 docs/usage.md 2→4) | lines selected, no text: held `None`, released an empty caret | lines selected and a text selection over the numbers and their code |
| Drag in the code (line 2→4 / 4727→4730) | the code's text only (`"\n## Install\n"`, `"\n[[package]]\nname = …"`) | the code and the numbers it crosses |
| Press on a number while code text is selected | the text selection ends, the line is selected | the same |
| Files surface, selected line (fixture.txt 1–2) | number cell `#d5e6ff` / `#254566`, number `#20649e` / `#72b6ff` (the diff's), code row untinted, no bar, the comment card under it untinted | the whole row amber `#fef3c7` / `#3a3112`, the number grey |

## Implementation

- GS-1 (`diff-rows.contract`): the number left the gutter cell. `DiffNumber` draws it over the cell's gutter, after every cell's
  code in the row (the deletions side at `left="0%"`, a split row's additions side at `50%`; the two cells split the row
  evenly), with `pointer-events="none" user-select="none" aria-hidden=true` and the cell's width, inset, font and colours
  unchanged. The cause: the number was a selectable paragraph, so a press on it started the host's text selection
  (`NodeViewMac.swift` `mouseDown` → `selection.begin`) and the drag extended it to the code; #413 had replaced the cell's
  `press` (which made its text a control's label) with `pointerdown`. Now a press on a number reaches the gutter cell (its
  `pointerdown`, #413's drag unchanged) and starts no text selection; a drag in the code selects the code and not the numbers.
  The gutter cell keeps the line's height with `min-height="20px"` (the number had given it). Why after the code: a copy
  (⌘C) across a virtualized list's rows reads the runner's list text, which numbers every text of a row
  (`runner/src/instance/text.rs` `collect`), while the Mac host places the selection by the row's selectable texts only
  (`TextSelectionMac.swift` `position`); an unselectable number before the code would shift what ⌘C takes (read, not run;
  see Observations). `user-select="none"` on the cell with the number inside it would have done that.
- GS-2 (`r4-surfaces-files.contract` `R4CodeRow`): no row tint; the number's cell `light-dark(#d5e6ff, #254566)` and the number
  `diffSel("number")` (#416's `light-dark(#20649e, #72b6ff)`, the same value) when selected, and the cell stretches to its
  line's height (`align-self="stretch"`), as the grid's number cell does under a wrapped line. The comment card under a
  selected line was already untinted (`selected=false`).
- Plan: 109,609 nodes (unchanged), 28,573,305 bytes (the base 28,573,504). No Rust or Swift changed.

## Acceptance results

| Row | Result | Proof |
| --- | --- | --- |
| GS-1 | pass (CDP, tests, one agent drive): a drag over the numbers selects lines and no text, in the thread's Diff panel and #168's Code tab; a drag in the code selects the code, no longer the numbers it crosses; a number press still ends a code selection. Every other pixel of the two surfaces is unchanged (the before/after shots with nothing selected are identical) | [Diff gutter drag](https://raw.githubusercontent.com/ccheever/exact2/38a6ec285ca5693a09f8adc38256109a7141b94e/diff-gutter-selection-followups/gs1-diff-gutter-drag.png), [Diff code drag](https://raw.githubusercontent.com/ccheever/exact2/1343df9fc4b323cfd131a8b9913c87cd4f01bc5c/diff-gutter-selection-followups/gs1-diff-code-drag.png), [PR gutter drag](https://raw.githubusercontent.com/ccheever/exact2/fadb613c27f207644e2d951415a4aabbbaf42175/diff-gutter-selection-followups/gs1-pr-gutter-drag.png), [PR code drag](https://raw.githubusercontent.com/ccheever/exact2/947229f9b56df41e9726fe4479acb8e45f50f8d5/diff-gutter-selection-followups/gs1-pr-code-drag.png), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/84be59fa77efa9ed44eb9735c1c719e715e6d937/diff-gutter-selection-followups/drive-record.txt), [reference-cdp.txt](https://raw.githubusercontent.com/ccheever/exact2/4a8c1873117cfa1813e864fbf66f8b0f726188bb/diff-gutter-selection-followups/reference-cdp.txt) |
| GS-2 | pass (CDP, tests, the same drive, light and dark): the selected line's number cell and number in the reference's blue, the code row untinted | [Files light](https://raw.githubusercontent.com/ccheever/exact2/7baac2debc0d767fc18cc330cfffc6eecc075af5/diff-gutter-selection-followups/gs2-files-light.png), [Files dark](https://raw.githubusercontent.com/ccheever/exact2/4e6108f28b7e4e79ffd96dcbecdc4f38c1287fdd/diff-gutter-selection-followups/gs2-files-dark.png), [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/84be59fa77efa9ed44eb9735c1c719e715e6d937/diff-gutter-selection-followups/drive-record.txt) |

No real-input rows: the agent's drags are mouse events delivered through `NSApplication.sendEvent` (main #186), so they take
the host's `mouseDown` / `mouseDragged` path a hand's drag takes, which is where the text selection began; #413's real-input
steps (its record) still cover the gutter drag under a real pointer.

## Tests

- `diff-gutter-selection-followups.test.ts` (new, 7): the number is `DiffNumber`'s (no pointer, `user-select: none`), the cell
  keeps one paragraph (the code); the number's geometry and colours equal the old ones; every row draws its numbers after all
  of its code (stacked and split, 0% and 50%); #413's handlers unchanged; the Files row's CDP colours, no row tint, no bar,
  the untinted draft; no amber left in any `.contract`.
- `diff-gutter-visuals.test.ts`: its number-colour assertion reads `DiffNumber` (the colour is unchanged).
- Before (the feature tip's sources with the new test): 1 pass, 6 fail; after: all pass ([tests-before-after.txt](https://raw.githubusercontent.com/ccheever/exact2/017a0a30479f75542d4467d071f81f4230c47cb7/diff-gutter-selection-followups/tests-before-after.txt)).

Checks on `694fd0d33` (the branch merged with `feat(example)/t3-code`, already up to date; all exit 0): `bun test
examples/t3-code --timeout 60000` 4451 pass / 1 skip / 0 fail (304 files); strict tsc; `contract build` of `app.contract`
(1397 lines; 109,609 nodes, 28,573,305 bytes); caps; the five checks (cargo build, cargo test 3679 pass / 0 fail / 34
ignored, clippy, fmt, caps, boot). No Rust or Swift changed, so `cargo test -p t3-code-macos --lib` and the AppKit binaries
were not run. The bundle for the live drive was built from `c160a4d6f` (the code; the record only after it).

## Attempts and evidence

| Attempt | Revision | Outcome | Evidence |
| --- | --- | --- | --- |
| Reference | T3 Code `1e2ecbd975`, lane 16780/16781, the GitHub lane on 16783 | the table above | [reference-cdp.txt](https://raw.githubusercontent.com/ccheever/exact2/4a8c1873117cfa1813e864fbf66f8b0f726188bb/diff-gutter-selection-followups/reference-cdp.txt) |
| Before drive | feature tip `327336447`, built in worktree `t3-code-diff-gutter-selection-followups-before` (the shared evidence base `c03d7e908` predates #413, whose gutter `pointerdown` made the drag select text) | the text selection over the numbers and code; amber Files row. Two runs: the first stopped at the Files step (a provider-update toast covered the tree row); the drive then points Codex and Claude at missing binaries in both lanes, as the reference finds neither, and closes the fresh storage's Nightly notice | [drive-record.txt](https://raw.githubusercontent.com/ccheever/exact2/84be59fa77efa9ed44eb9735c1c719e715e6d937/diff-gutter-selection-followups/drive-record.txt) |
| After drive (one, no retry) | `c160a4d6f`, bundle built from it | the images above | the images above |

## Observations (outside this record's rows)

1. The Files surface's "+" is still the old 16×16 `#1b4ed8` button at the cell's left (x 2–18 of a one-digit gutter), so a
   press on the middle of number 1 opens a draft instead of selecting (both builds' drives show it); the reference's is
   Pierre's utility button at the number's right edge (cell left + 23.5), 20×20 `#009fff` with the plus in the surface colour
   (`#ffffff` / `#111111`). #416 moved the diff's, not the Files surface's.
2. The reference's Files surface selects lines by a drag over the numbers too and opens the comment draft at the end of any
   line selection (`FilePreviewPanel` `onLineSelectionEnd` → `beginComment`); the clone's selects by press and Shift-press
   and opens a draft only from the "+".
3. Framework, read and not run: on macOS a copy across a virtualized list's rows takes the runner's `list_text`, which counts
   every text of a row and ignores `user-select`, while the host counts selectable texts only. So a copy across diff rows
   still carries the line numbers (as before this change; the reference copies the code only) with a blank line between
   paragraphs, and an unselectable text before a selectable one in a row would shift the copied text. Not filed here.

## Not done / not verified

- Nothing of this record's rows.

## Delivery

Draft PR [#417](https://github.com/ccheever/exact2/pull/417) into `feat(example)/t3-code`.

## Next action

Coordinator review of the draft PR.
