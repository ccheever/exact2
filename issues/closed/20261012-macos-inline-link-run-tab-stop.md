# macOS: an inline link run is never a Tab stop, even with Keyboard navigation on

**Status:** Closed
**Resolution:** Inline responders follow Keyboard Navigation on the page and in dialogs, keep disabled links focusable, preserve a focused link's Return ahead of the default button, and activate press-only runs on Space; InlineFocusMacTests covers agent key delivery in painted dialogs and AppKit sheets, Enter/Space for href-only, press-only and combined runs, and wrapped focus rings.
**Systems:** host/apple macOS, focus, text
**Severity:** P3
**Author:** Daehyeon Mun (via the T3 Code clone)
**Date:** 2026-10-12
**Related:** T3 PR #474 (reply-links-reference-behavior); https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/EXACT2-GAPS.md (X77's note); LLP 1115.000 D7.2 (Tab follows the Mac's Keyboard navigation setting); `issues/closed/20261010-macos-inline-run-contextmenu.md` (X77); `issues/20261012-macos-voiceover-show-menu.md` (X84, main PR #483); main PR #327

## Summary

On macOS a text run with an `href` inside a paragraph (an inline link run) is never a Tab stop. A `link` node is a
Tab stop when the Mac's Keyboard navigation setting is on (System Settings ▸ Keyboard; LLP 1115.000 D7.2), as every
control is then. The run in the same app is passed over, forward and backward. The key-view loop is built from views
(`KeyViewLoopMac.swift:68` `syncKeyViewLoop` walks `NodeView`s; `FocusMac.swift:16` `acceptsFirstResponder` and
`:29` `canBecomeKeyView` are a `NodeView`'s). A run has no view: it is an `NSAccessibilityElement` inside its
paragraph's view (`TextInteraction.swift:177` `InlineAccessibility`), so it can be neither a responder nor a key view.
Neither Tab nor anything aimed at the focused node (the ContextMenu key, Enter) can reach it.

On the web (the JS target) the run is an `<a href>`, and Chrome on macOS Tabs to it as it does to the `link` node.

## The platform question (for Charlie, LLP 1115)

Which answer is the Mac's, and is the difference a gap?

| | Keyboard navigation off (the Mac's default) | Keyboard navigation on |
|---|---|---|
| Chrome on macOS, an `<a href>` (the Exact web build, measured; the T3 Code reference is Electron) | a Tab stop | a Tab stop |
| Safari, an `<a href>` (not run: its Settings ▸ Advanced text) | not a stop unless "Press Tab to highlight each item on a webpage" is on; ⌥-Tab reaches it | the same setting decides |
| A hand-built `NSTextView` link (measured with the setting off) | not a stop: the text view is one stop and its link is not | not run |
| Exact's `link` node (a `NodeView`; measured) | not a stop, as a button is not (D7.2) | a Tab stop |
| Exact's inline link run (measured) | not a stop | **not a stop** |

- With Keyboard navigation **on**, Exact's own rule ("On, Tab reaches every control",
  `docs/contract-grammar.md`) reaches a `link` node but not a link run. That looks like a gap whatever the answer for
  "off". A run inside a paragraph is how Markdown and any inline prose write a link, and a table cell lays its links
  out that way.
- With Keyboard navigation **off**, Exact matches Safari and AppKit (no link is a stop) and differs from Chrome (every
  link is a stop). Under LLP 1115 the platform's answer wins where it differs from the browser's. An app whose
  reference is an Electron app (as the T3 Code clone's is) differs there for every link, node or run. This issue does
  not ask to change that. It asks whether a link run should follow the `link` node's rule.

## Why this arose

The T3 Code clone draws a reply's Markdown table cell as inline runs of one text node (`TableCell` → `ChatRuns`). Its
paragraph, list and heading links are word-by-word `link` nodes (X77). T3 PR #474 (reply-links-reference-behavior)
made the ContextMenu key open the shell's menu at a focused reply link. It cannot reach a table cell's link, because
that link never takes the focus. In the reference (Electron) the cell's anchor is a Tab stop.

## Reproduction

Main `a9ceacf05efad43d73645fcea761afd517c94e85`, macOS 26.6.2 (25G83), agent mode only (no real input). This Mac's
Keyboard navigation setting is off; the macOS drive sets its own with `prefer keyboard-navigation`. A one-file app
made with `bun scripts/exact.mjs new <dir>` (`app.ts` exports an empty `sources`):

```text
component App
  state presses = 0
  action noop
    presses = presses + 1
  view
    main testId="root" width="100%" height="100%" padding=16 display="flex" flex-direction="column" align-items="flex-start" gap=12
      button press=noop testId="first"
        text "First"
      link href="https://example.com/node" press=noop testId="node-link"
        text "node link"
      text testId="para"
        text "Read "
        text "the run" href="https://example.com/run" press=noop testId="run-link"
        text " here."
      button press=noop testId="last"
        text "Last"
```

View ids: #2 First, #4 the `link` node, #8 the run, #10 Last. `type <target> key Tab` focuses the target and sends
Tab; `state.focus.logical` is the focused view.

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| M1: `prefer keyboard-navigation false`, `type first key Tab` | macOS, agent | main `a9ceacf05` | the focus stays on #2: no stop after it (the link node, the run and Last are passed over) | the Mac's default (D7.2): as is | record M |
| M2: `prefer keyboard-navigation true`, Tab from First, then from the link node, then Shift+Tab from Last | macOS, agent | main `a9ceacf05` | #2 → #4 → #10; Shift+Tab #10 → #4. The run #8 is passed over both ways | #2 → #4 → #8 → #10; Shift+Tab #10 → #8 (the question above) | record M |
| W: the same Tabs | Exact web (JS target), Chrome 155, agent | main `a9ceacf05` | #2 → #4 (`<a>`) → #8 (`<a>`) → #10; Shift+Tab #10 → #8 | (Chrome's answer) | record W |
| O: an `NSButton`, a read-only `NSTextView` whose "the run" has `NSLinkAttributeName`, an `NSButton`; Tab through the window | AppKit, Keyboard navigation off | — | the buttons are no key views; the text view is one stop; its link is no stop | (AppKit's answer, off) | record O |

Not run: Safari; the `NSTextView` with Keyboard navigation on (this Mac's setting); Firefox and WebKit; iOS with a
hardware keyboard; Linux; a run that wraps across lines; a focus ring on a run.

## Constraints

- No fix is proposed here. The site is the key-view loop (`KeyViewLoopMac.swift` `syncKeyViewLoop` and `tabbable`,
  `FocusMac.swift` `canBecomeKeyView`), which holds views. A run's stop would need a responder that stands for the run
  inside its paragraph's view (its focus, focus ring, `focus`/`blur`, Enter to press, the ContextMenu key at its
  rectangle) without breaking the paragraph's text selection. Alternatively, the decision could be that runs stay out.
- Whatever is decided for "off" should hold for a `link` node and a link run alike, so that an author's choice between
  the two does not change who can reach the link.
- No open or closed issue on main covers it. `issues/20261009-macos-controls-default-tab-order.md` is about dates and
  selects, `issues/closed/20261010-macos-tab-from-a-focused-box.md` about a focused box that is no stop, and X77's
  issue about a run's `contextmenu`. The GitHub issues do not cover it either. Main PR #327 (head `14493d253`) changes
  `FocusMac.swift` for dates and selects only.

## Acceptance criteria

- The decision is written down: with Keyboard navigation on, an inline link run is or is not a Tab stop; with it off,
  links (node and run) follow the Mac or Chrome.
- If runs are to be stops: in the repro with `prefer keyboard-navigation true`, Tab goes #2 → #4 → #8 → #10 and
  Shift+Tab #10 → #8. `state.focus.logical` names the run. Enter on the focused run presses it, and the ContextMenu
  key raises the run's `contextmenu` at the run (with X77). A host test covers forward and backward order, a run that
  wraps, and a paragraph with two runs.
- The web's order is unchanged.

2026-10-12: X77's fix (#491, which closes that issue) makes a secondary click reach a run's `contextmenu` and `contextPopover` on macOS. The ContextMenu key at a focused run is left to this decision. A run with `press` and no `href` is the same question: the web build gives every pressable `tabindex=0` (`host/web-js/src/emit.rs`), so Chrome can focus such a run (not driven), while macOS gives it no focus.
