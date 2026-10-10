# Text selection: its rectangles, and clearing or setting it on a text (rest of #132)

**Status:** Open
**Systems:** Contract, host/apple, web text selection
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/274

## Current scope

Add selection rectangles and web-named removeAllRanges/setBaseAndExtent semantics, or a documented setSelectionRange extension. Share #272 geometry; no new clearSelection name without a decision. Geometry remains an action/event snapshot.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

The rest of #132, split out because #132 closed when its first part landed (#171: on macOS a press on a `button` keeps the reader's text selection, as in Chrome; confirmed on main below). Two parts remain, on the web and macOS:

- **The selection's geometry.** `Selection` carries `text`, `start` and `end` only, so an app cannot place a control (a "quote this" button, a comment marker) at the selection's last line.
- **Clearing or setting a selection on a `text`.** There is no `clearSelection()`, and `setSelectionRange` on a `text` compiles but is refused at run time, so an app cannot clear the highlight after acting on it, or highlight a passage it opens.

### Current and expected behavior

Current, on main `0365ad1a4` (`TextSelectionMac.swift`, the web host and the compiler are unchanged on `e200397ec`):

- Reading `s.rect.y` or `length(s.rects)` in a `selectionchange` action: `type-unknown-field` "`Selection` has no field `rect`; available fields: `text`, `start`, `end`" (same for `rects`).
- `clearSelection()` in an action: `type-unknown-command` "`clearSelection` is not a host command".
- `setSelectionRange("p1", 0, 5)` on a `text`: compiles; at run time the journal says `setSelectionRange "p1" refused: not a text field`, on the web and macOS.
- `selectionchange` on a container (a `column`): `lower-attr-tag`. An app can combine the per-`text` parts itself, so this stays optional.

Expected (the web's Selection API, by its names):

- Each part of a selection has its rectangles: `Range.getClientRects()` (one per line box) and `getBoundingClientRect()`, in viewport space as `frame()` reports. Proposal: `Selection` gains `rects: list<Geometry>` (and optionally `rect`).
- `Selection.removeAllRanges()` and `Selection.setBaseAndExtent()`. Proposal: a host command `clearSelection()`, and `setSelectionRange` (or `selectText`) accepted on a `text` by its `id`, firing `selectionchange` as the web does.
- iOS and Linux have no selection on a `text` today (grammar, "Text selection"); they stay out unless that changes.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`; `app.ts` answers no sources:

```contract
component SelectionRange
  state p1 = "-"
  state p2 = "-"
  state events = 0
  state cited = "-"
  action heard(which: string, s: Selection)
    events = events + 1
    if which == "p1"
      p1 = `${s.start}-${s.end}:${s.text}`
    else
      p2 = `${s.start}-${s.end}:${s.text}`
  action selectFirst
    cited = `${p1} | ${p2}`
    setSelectionRange("p1", 0, 5)
  view
    column testId="root" padding=24 gap=16 width="100%"
      text font-size=18 selectionchange=heard("p1") id="p1" testId="p1"
        text "Alpha beta "
        text "gamma delta" font-weight=700
        text " epsilon 😀 zeta."
      text font-size=18 selectionchange=heard("p2") testId="p2"
        text "Second paragraph "
        text "code()" font-family="monospace"
        text " ends here."
      text `p1=${p1}` testId="out1"
      text `p2=${p2}` testId="out2"
      text `events=${events}` testId="out3"
      text `cited=${cited}` testId="out4"
      button press=selectFirst testId="sel"
        text "Cite"
```

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Press keeps the selection (fixed by #171) | `bun exact.mjs agent <host> --size 600x400 --json "tap p1 drag 60 44 from 60 14 mouse" "tap sel" logs state` | web (Chrome 154); macOS 26.6.2 after `bun exact.mjs mac` | `0365ad1a4` | both hosts: `cited` = `7-39:eta gamma delta epsilon 😀 zeta. \| 0-13:Second paragr`, `events` 6 | same (passes) | agent `state` |
| Set a range on `text` | the same drive's `logs` | web; macOS | `0365ad1a4` | `setSelectionRange "p1" refused: not a text field` on both | "Alpha" selected, `selectionchange` with `0-5:Alpha` | agent `logs` |
| Geometry | read `s.rect.y` (or `length(s.rects)`) in `heard`; `bun exact.mjs contract build <file> --json` | compiler | `0365ad1a4` | `type-unknown-field` (quoted above) | compiles; rectangles of the part | compiler output |
| Clear | `clearSelection()` in `selectFirst`; `contract build --json` | compiler | `0365ad1a4` | `type-unknown-command` | compiles; clears and fires `""` at 0, 0 | compiler output |

### Acceptance criteria

- After the drag above, the last rectangle of `p2`'s part has its bottom at the selected line's bottom on the web and macOS (within 1 pt of Chrome).
- `clearSelection()` removes the highlight and fires `selectionchange` with `""` at `0, 0` on each paragraph that had a part, on both hosts.
- `setSelectionRange("p1", 0, 5)` (or the chosen form) highlights "Alpha" and fires `0-5:Alpha`, on both hosts.
- An AppKit test and a web conformance case cover the three.

### Constraints and related work

- Related: #132 (closed by #171, the button press only); #272 (the frame of an inline run, rest of #133; geometry of a run, not of a selection); #127 (layout facts; distinct).
- Not tested: iOS and Linux (no selection on `text`), double and triple click, keyboard selection, a real mouse on macOS (the agent's drag is a platform event).
- Workarounds today: place the control at the pointer's release point, and leave the highlight in place.

## Discussion at transfer

### ccheever — 2026-10-08T08:07:30Z

**Decision: Add selection rectangles and web-named selection commands.**

Keep open with the bounded scope below.

A quote/comment control needs Range client rectangles and programmatic selection. Geometry remains an event/action snapshot.

Use Selection.removeAllRanges/setBaseAndExtent semantics or an explicitly documented extension of setSelectionRange. Share native rectangle work with #272; avoid inventing clearSelection where the web already names the operation.
