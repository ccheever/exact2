---
name: 20261011-x81-macos-list-copy-unselectable-text
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: null
reproduced_on: null
---

# X81: On macOS a copy across a virtualized list's rows includes `user-select: none` text, so a copy across diff rows carries the line numbers

Moved to main `issues/20261011-macos-list-copy-unselectable-text.md` (2026-10-11), where it is tracked. Main PR
[#418](https://github.com/ccheever/exact2/pull/418) filed it (merged `e2c6fcc43`).

## Summary

On macOS, ⌘C on a selection inside a `list virtualized=true` copies the runner's `list_text` (`TextSelectionMac.swift`
`selectedText` → `onListText` → `Runner::list_text`). Its row projection (`runner/src/instance/text.rs` `collect`)
counts every text of a row and never reads `user-select`, so the unselectable texts between the endpoints are copied.
The host numbers a row's paragraphs over its selectable texts only, and the runner numbers them over every text. So an
unselectable text before a selectable one also shifts the endpoints. Rows that put the number first copy `L1`…`L3` and
lose the last row's code, though the painted selection is right. Plain rows are copied by the host and leave the label
out. On the web (the JS target), Chrome's own copy leaves `user-select: none` text out of list rows too.

## Why it arose

Task `20261011-diff-gutter-selection-followups` (GS-1, T3 PR #417, open at filing). The reference's diff
(@pierre/diffs) marks its line-number column `user-select: none` (`[data-column-number]`), so a copy across diff lines
holds only the code. The clone's diff rows (`diff-rows.contract`) are a virtualized list. #417 made the gutter's
number `DiffNumber`, with `user-select="none"`, and placed it after the code in each row, so the endpoints still land
on the code. Its observation 3 read from the source that a copy across the rows on macOS still carries the line numbers,
and did not file it.

## Clone workaround

None. On macOS a copy across the diff's rows (the thread's Diff panel and the pull request Code tab) includes the line
numbers, where the reference copies the code only. With #417 these are the numbers between the endpoints; before it,
the number was selectable text. When main fixes the list copy, re-drive a copy across diff rows on macOS: it should
hold the code only, with no clone change.

## Evidence and history

- 2026-10-11, main PR #418 reproduced it on main `c001a862b` in a one-file app. There are three groups of five rows,
  each a label `L<n>` with `user-select="none"` and a code text: a virtualized list with the label first (V), one with
  the label after the code (W, as #417), and plain rows (P). Each macOS scenario reproduced on its first drive.
  - macOS 26.6.2, with the agent's drag from row 1's code to the end of row 3's and ⌘C, the pasteboard read with
    `pbpaste`. V gave `L1\n\nvcode 1 body\n\nL2\n\nvcode 2 body\n\nL3`, and W gave
    `wcode 1 body\n\nL1\n\nwcode 2 body\n\nL2\n\nwcode 3 body`. P gave the code only. ⌘A then ⌘C in V copied every
    label. The pasteboard held no item before and was left empty.
  - The Exact web (JS target) in Chrome 155, with Playwright's mouse and the browser's own copy (`execCommand('copy')`
    and Meta+C), copied the code only in V, W and P.
  - The wasm web target, which copies a list through the same runner call (`list-selection.js`), included the labels
    too.

  Open main PR #327 does not change it: it touches none of `runner/`, `TextSelectionMac.swift`, `Bridge.swift` or
  `list-selection.js` (read from its diff).
  Image:
  [x81-list-copy-user-select-web-macos.png](https://raw.githubusercontent.com/ccheever/exact2/ef0069c611f4bc0837fbb439e8dc57d3bf4d17d2/fw-issues-20261011k/x81-list-copy-user-select-web-macos.png);
  record: [x81-record.txt](https://raw.githubusercontent.com/ccheever/exact2/ef0069c611f4bc0837fbb439e8dc57d3bf4d17d2/fw-issues-20261011k/x81-record.txt).
