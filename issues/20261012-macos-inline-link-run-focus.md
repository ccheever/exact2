# macOS: an inline link run takes no keyboard focus, so Tab and the ContextMenu key skip it

**Status:** Open
**Systems:** host/apple macOS, focus, keyboard
**Severity:** P2
**Author:** Daehyeon Mun (via the T3 Code clone)
**Date:** 2026-10-12
**Related:** T3 PR #474 (reply-links-reference-behavior); https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/EXACT2-GAPS.md (X77's note); `issues/20261010-macos-inline-run-contextmenu.md` (X77, a run's `contextmenu`); `issues/20261010-macos-tab-from-a-focused-box.md` (X79, the same key-view loop); LLP 1088 D7.3; LLP 1115.000 D7.2

## Summary

An inline run with an `href` (a `text` with `href` inside a `text`) is a link. The web target draws it as an
`<a href>`, which Chrome makes a Tab stop. Tab and Shift+Tab reach it in tree order, it holds the focus, the
ContextMenu key runs its `contextmenu` there, and Enter follows it. On macOS the run never takes the keyboard focus.
Tab and Shift+Tab pass over it even with Keyboard Navigation on, so a keyboard user cannot reach the link. The
ContextMenu key cannot reach it either. When the agent asks to focus the run, the paragraph that holds it gets the
focus.

The host's key-view loop lists views. `syncKeyViewLoop` (`host/apple/Sources/ExactKit/Mac/KeyViewLoopMac.swift:68-81`)
walks `NodeView`s and their subviews, and `tabbable` (`:156-163`) decides each one. A run is drawn inside its
paragraph's view (`InlineText.swift`) and is no view, so it is never listed and never becomes first responder. The
ContextMenu key's walk (`Mac/MouseEventsMac.swift:24-75`) starts from the window's first responder and also visits
views only (X77). With no focused run, the ContextMenu key has nothing to start from. X77 asks for a run's
`contextmenu` under the pointer. This issue asks for the keyboard path to a run: its focus, Tab order and keys.

## Why this arose

The T3 Code clone draws a Markdown table cell's runs inline in one text node, so a long cell ends in one ellipsis at
the column's edge. A cell's links are therefore inline runs. In the T3 Code desktop app (Electron) they are anchors:
Tab reaches each one, and the ContextMenu key on it opens the shell's menu (measured over CDP with the main
process's menu read back). In the clone neither works. T3 PR #474 (task reply-links-reference-behavior, RL-6) made
the ContextMenu key open that menu on a focused link node, and a cell's link is out of its reach. The clone records
this as a note under X77 in its `EXACT2-GAPS.md`.

## Reproduction

Main `85a116a9c80f0b9775f4ea593a80d815bc8d822b`, macOS 26.6.2 (25G83), Apple Silicon; the web target in Google Chrome
155.0.8059.39 on the same Mac. The app is the one in `issues/20261012-macos-control-click-contextmenu.md` (a `link`, a
`button` "Options", a paragraph holding the run `text "the notes" href="#notes" press=… contextmenu=… testId="run"`,
then a `button` "After"; each node's `focus` writes the log). It is made with `bun scripts/exact.mjs new <dir>` and
built with `bun exact.mjs mac`.

macOS: `bun exact.mjs agent macos --size 520x300 "prefer keyboard-navigation true" "type options key Tab" tree "type
after key Shift+Tab" tree "type run key ContextMenu" tree`. Web: `bun exact.mjs agent web --size 520x300 "type
options key Tab" tree "type run key Tab" tree "type after key Shift+Tab" tree "type run key ContextMenu" tree`. The
tree marks the node that holds the focus (`[focused]`). Keyboard Navigation is turned on for the macOS drive through
the agent's `prefer`. With it off, the machine's mode (LLP 1115.000 D7.2), Tab reaches no link or button at all, which
is the platform's rule and not this issue. No real input was used.

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| Tab from "Options" | macOS, Keyboard Navigation on | main `85a116a9c` | `After` takes the focus (`focus after`): the run is skipped | the run takes the focus, as on the web | image (right), record |
| Shift+Tab from "After" | macOS | main `85a116a9c` | `Options` takes the focus (`focus button`) | the run | record |
| `type run key ContextMenu`: the agent focuses the run, then sends the key | macOS | main `85a116a9c` | the paragraph `para` holds the focus; nothing runs (`menus=0`) | the run holds the focus and its `contextmenu` runs | record |
| The same steps, web | Chrome 155 on macOS | main `85a116a9c` | Tab from "Options" → the run (`[focused]`); Tab from the run → `After`; Shift+Tab from `After` → the run; the ContextMenu key on the run → `contextmenu run · menus=1` | (the reference) | image (left), record |

![An inline link run's focus, Exact web in Chrome and macOS](https://raw.githubusercontent.com/ccheever/exact2/2d571542682f4d32c3383bf13b6bf627b26f232c/fw-issues-20261012-x85/inline-link-focus-web-macos.png)

Record (the contract, the commands and both hosts' output, the code read):
https://raw.githubusercontent.com/ccheever/exact2/60ae554222a9ad462600ffe84be0285849bcbcd6/fw-issues-20261012-x85/x85-inline-link-focus-record.txt

Not run: real hardware input (the agent's key events were used); Enter or Space on a focused run; VoiceOver; a run
with `press` and no `href`; a run that wraps across lines; iOS with a hardware keyboard, Linux and Windows.

## Constraints

- The web is the parity oracle (`CLAUDE.md`). LLP 1115: author > platform > CSS default. The author wrote an `href`
  on the run, which makes it a link.
- Whether links are Tab stops follows the platform's Keyboard Navigation mode (LLP 1115.000 D7.2), as a `link` node
  does today. With it on, a run with an `href` is a stop in tree order between the nodes around it. With it off, it is
  no stop, like any link, but it can still hold the focus a click or `focus()` gives it.
- A focused run shows the platform's focus ring around its glyphs (all its line fragments when it wraps). Keys go to
  it as to a focused node: the ContextMenu key raises its `contextmenu` at its middle (the X77 dispatch), and Enter
  presses it or follows its `href`.
- The paragraph's text selection and its own place in the loop (a clicked paragraph is where Tab starts) are
  unchanged.
- No open or closed issue on main covers it, and neither do the GitHub issues (searched `inline run`, `inline link`,
  `link run` with `Tab` and `focus`). X77 (`issues/20261010-macos-inline-run-contextmenu.md`) is the pointer side of
  the same run and lists "the ContextMenu key on a focused link run" as not run. X79 is the same loop's Tab from a box
  that is no stop.

## Acceptance criteria

- In the repro on macOS with Keyboard Navigation on, Tab from "Options" focuses the run, Tab from it focuses
  "After", and Shift+Tab from "After" focuses the run, as on the web.
- With the run focused, the ContextMenu key runs its `contextmenu` once (`menus=1`).
- With Keyboard Navigation off, Tab still skips links, the run included.
- A host test (`host/apple` XCTest) covers the run in the key-view loop and a key on the focused run.
