# macOS: hover does not follow the pointer during a mouse drag

**Status:** Open
**Systems:** host/apple macOS, pointer events
**Severity:** P3
**Author:** daehyeon-mun (T3 Code clone)
**Date:** 2026-10-10
**Related:** https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/.exact/implementation/20261005-t3code-macos-parity/issues/closed/20261010-x80-macos-hover-during-a-drag.md; issues/20261009-macos-subtree-hover.md (X62, #322: which nodes a free pointer hovers, a different gap); LLP 1056 §3 (the held pointer); main PR #327

## Summary

On the web, a Contract `hover` handler is `pointerenter`/`pointerleave` (`host/web-js/rt.js:911`,
`host/web/glue.js:573-575`). During a mouse drag with the button held, Chrome keeps hit-testing. The pressed element
hears its leave as the pointer moves off it, each element the pointer crosses hears an enter and a leave (`buttons`
1), and the element under the pointer is the hovered one when the button comes up. CSS `:hover` follows the same way.
The web host holds a pressed pointer node without a pointer capture. Its moves and its up are heard on the document
(`host/web-js/pointer.js:1-8`), so nothing pins the hover to it.

On macOS the hover stays on the node the button went down on, for the whole drag and after the release, until the
pointer moves again. The pointer reaches `hover` only through each `hover` node's `NSTrackingArea`
(`host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:318-341`). Its options are `[.mouseEnteredAndExited,
.mouseMoved, .activeAlways, .inVisibleRect]`, with no `.enabledDuringMouseDrag`. AppKit's `NSTrackingArea.h` says
such an area reports `mouseEntered` "as mouse is moved, and on mouseUp after a drag", and only with that flag "as
mouse is dragged". The drags go to the pressed view's `mouseDragged` (`:1394-1405`). That feeds the held pointer's
`pointermove`, the mouse chain, the press and the selection, but never the hover. The hit test that keeps the hover
under a resting pointer after a layout change (`hoverUnderPointer`, `Mac/MouseChainMac.swift:266`) does nothing
while a button is down or a pointer is held: `restingPointer` (`:250-260`) returns nil then. Inline runs' hover
uses the same tracking area (`TextInteraction.swift:25`).

## Why this arose

T3 Code's diff gutter (@pierre/diffs) starts a line selection on a press on a line number or on its "+". The lines
from the press to the pointer paint as it moves, and the "+" follows the pointer over the lines (`placeUtility`).
The clone's gutter (T3 PR #413, task realinput-1010f-followups RF-3, open) holds the pointer on the pressed node
and names the line under it with `elementFromPoint` on each `pointermove`. That part works on macOS: lines 3–5
paint. The "+" is placed by the lines' `hover`. In the real-input session it stayed on the pressed line 3 for the
whole drag. The clone has no workaround. The selection, which is what the drag is for, paints correctly.

## Reproduction

The app is made with `bun scripts/exact.mjs new <dir>` on main `efeb92cd221e0c9ec9168509c252370d9dbf7387`. Its
`app.ts` exports an empty `sources`. `contract build` compiles it with no warning. Column A's rows hear only
`hover`. Column B's rows also hear `pointerdown` and `pointerup`, so the pressed row holds the pointer, as the clone's
gutter does. The log is newest first: `+` an enter, `-` a leave, `v` a `pointerdown`, `^` a `pointerup`.

```text
component X80app
  state log = ""
  state n = 0
  state a1 = false
  state a2 = false
  state a3 = false
  state a4 = false
  state a5 = false
  state b1 = false
  state b2 = false
  state b3 = false
  state b4 = false
  state b5 = false
  action ev(name: string, over: bool)
    n = n + 1
    log = `${n}${name}${over ? "+" : "-"} ${log}`
    a1 = name == "A1" ? over : a1
    a2 = name == "A2" ? over : a2
    a3 = name == "A3" ? over : a3
    a4 = name == "A4" ? over : a4
    a5 = name == "A5" ? over : a5
    b1 = name == "B1" ? over : b1
    b2 = name == "B2" ? over : b2
    b3 = name == "B3" ? over : b3
    b4 = name == "B4" ? over : b4
    b5 = name == "B5" ? over : b5
  action pd(name: string)
    n = n + 1
    log = `${n}${name}v ${log}`
  action pu(name: string)
    n = n + 1
    log = `${n}${name}^ ${log}`
  view
    main testId="root" width="100%" height="100%" padding=24 box-sizing="border-box" background-color="#ffffff" color="#111111" display="flex" flex-direction="column" gap=16
      text `log: ${log}` font-size=13 testId="log"
      row gap=40 user-select="none"
        column width=200 gap=0
          text "A: hover only" font-size=13 margin-bottom=6
          box testId="a1" height=40 display="flex" align-items="center" padding-left=12 border-bottom-width=1 border-bottom-style="solid" border-bottom-color="#ffffff" hover=ev("A1") background-color=(a1 ? "#3b82f6" : "#e5e7eb") color=(a1 ? "#ffffff" : "#111111")
            text "A1" font-size=15
          box testId="a2" height=40 display="flex" align-items="center" padding-left=12 border-bottom-width=1 border-bottom-style="solid" border-bottom-color="#ffffff" hover=ev("A2") background-color=(a2 ? "#3b82f6" : "#e5e7eb") color=(a2 ? "#ffffff" : "#111111")
            text "A2" font-size=15
          box testId="a3" height=40 display="flex" align-items="center" padding-left=12 border-bottom-width=1 border-bottom-style="solid" border-bottom-color="#ffffff" hover=ev("A3") background-color=(a3 ? "#3b82f6" : "#e5e7eb") color=(a3 ? "#ffffff" : "#111111")
            text "A3" font-size=15
          box testId="a4" height=40 display="flex" align-items="center" padding-left=12 border-bottom-width=1 border-bottom-style="solid" border-bottom-color="#ffffff" hover=ev("A4") background-color=(a4 ? "#3b82f6" : "#e5e7eb") color=(a4 ? "#ffffff" : "#111111")
            text "A4" font-size=15
          box testId="a5" height=40 display="flex" align-items="center" padding-left=12 border-bottom-width=1 border-bottom-style="solid" border-bottom-color="#ffffff" hover=ev("A5") background-color=(a5 ? "#3b82f6" : "#e5e7eb") color=(a5 ? "#ffffff" : "#111111")
            text "A5" font-size=15
        column width=200 gap=0
          text "B: hover + pointerdown" font-size=13 margin-bottom=6
          box testId="b1" height=40 display="flex" align-items="center" padding-left=12 border-bottom-width=1 border-bottom-style="solid" border-bottom-color="#ffffff" hover=ev("B1") pointerdown=pd("B1") pointerup=pu("B1") background-color=(b1 ? "#3b82f6" : "#e5e7eb") color=(b1 ? "#ffffff" : "#111111")
            text "B1" font-size=15
          box testId="b2" height=40 display="flex" align-items="center" padding-left=12 border-bottom-width=1 border-bottom-style="solid" border-bottom-color="#ffffff" hover=ev("B2") pointerdown=pd("B2") pointerup=pu("B2") background-color=(b2 ? "#3b82f6" : "#e5e7eb") color=(b2 ? "#ffffff" : "#111111")
            text "B2" font-size=15
          box testId="b3" height=40 display="flex" align-items="center" padding-left=12 border-bottom-width=1 border-bottom-style="solid" border-bottom-color="#ffffff" hover=ev("B3") pointerdown=pd("B3") pointerup=pu("B3") background-color=(b3 ? "#3b82f6" : "#e5e7eb") color=(b3 ? "#ffffff" : "#111111")
            text "B3" font-size=15
          box testId="b4" height=40 display="flex" align-items="center" padding-left=12 border-bottom-width=1 border-bottom-style="solid" border-bottom-color="#ffffff" hover=ev("B4") pointerdown=pd("B4") pointerup=pu("B4") background-color=(b4 ? "#3b82f6" : "#e5e7eb") color=(b4 ? "#ffffff" : "#111111")
            text "B4" font-size=15
          box testId="b5" height=40 display="flex" align-items="center" padding-left=12 border-bottom-width=1 border-bottom-style="solid" border-bottom-color="#ffffff" hover=ev("B5") pointerdown=pd("B5") pointerup=pu("B5") background-color=(b5 ? "#3b82f6" : "#e5e7eb") color=(b5 ? "#ffffff" : "#111111")
            text "B5" font-size=15
```

The cursor drive launches the app with `bun exact.mjs mac --run` (a 900x700 window). It uses `cliclick`
(CGEvents posted at the HID tap, so they go through the window server) to click the empty page, rest on A1's middle,
then `dd` on A1, 31 `dm` steps of 4 pt down to A4's middle, and `du` over A4. It reads the window between steps
(`screencapture -l`, and the log through the accessibility tree). The agent drives are `bun exact.mjs agent <web|macos>
--size 520x420 "tap a1 hover" "tap a1 drag 0 120 [mouse] over 800 hold 200 during \"clock settle\" \"tree log\""
"tree log"` (`mouse` on the web; the macOS contact is the mouse already).

| Scenario | Platform | Revision | Actual | Expected | Evidence |
|---|---|---|---|---|---|
| A1 → A4 with the button held, the system cursor (`cliclick`) | macOS 26.6.2 (25G83), Apple Silicon | main `efeb92cd2` | Held over A4: `log: 0A1+`, and A1 is painted hovered. Released over A4: the same. The first free move after: `2A4+ 1A1- 0A1+` | `… 5A3- 6A4+` by the time the pointer is over A4, as on the web | image (middle, right), record |
| B1 → B4 (the pressed row holds the pointer), the system cursor | macOS | main `efeb92cd2` | Held over B4: `5B1v 4B1+`. B1 stays hovered until the first free move | B4 hovered, as on the web | record |
| The agent's drag after `tap a1 hover` | macOS | main `efeb92cd2` | `0A1+` during and after the drag | as on the web | record |
| The same drags, web | Exact web (JS target) in Chrome 155.0.8059.39, the agent's CDP mouse | main `efeb92cd2` | Held over A4: `6A4+ 5A3- 4A3+ 3A2- 2A2+ 1A1- 0A1+`. B: `7B4+ 6B3- 5B3+ 4B2- 3B2+ 2B1- 1B1v 0B1+` | (the reference) | image (left), record |
| CSS `:hover` on plain HTML rows, the same held drag | Chrome 155, Playwright's mouse | — | `:hover` is on r4 while the button is held and after the release. `pointerenter`/`pointerleave`: `r1+0 r1-1 r2+1 r2-1 r3+1 r3-1 r4+1` (the number is `buttons`) | — | record (probe source beside it) |
| Hand-built AppKit rows (`swiftc`), each an `NSView` with its own `NSTrackingArea`: the host's options; the same plus `.enabledDuringMouseDrag`; each also with the press passed to `super`. The same `cliclick` drag | macOS 26.6.2 | — | Every column: 31 `leftMouseDragged` reach the window, but no `mouseEntered` or `mouseExited` comes during the drag or at `mouseUp`. The first free move sends the leave and the enter. A CGEvent poster that sets click state, deltas and pressure gives the same | `NSTrackingArea.h`: with `.enabledDuringMouseDrag`, `mouseEntered` "as mouse is dragged"; without it, "on mouseUp after a drag"; a `mouseExited` pairs with its enter "whether the mouse is being moved or dragged" | record (oracle source beside it) |

![X80: hover during a held mouse drag, Exact web in Chrome and macOS](https://raw.githubusercontent.com/ccheever/exact2/931a320fd932afec2e89a9e4bd83b6369a0c6a22/fw-issues-20261010j/x80-hover-during-drag-web-macos.png)

Record (contract, commands, every drive's log, the Chrome probe, the AppKit oracle, the code read):
https://raw.githubusercontent.com/ccheever/exact2/931a320fd932afec2e89a9e4bd83b6369a0c6a22/fw-issues-20261010j/x80-record.txt
(sources at the same commit: `x80-app.contract.txt`, `x80-realdrag.py.txt`, `x80-chrome-css-hover-probe.mjs.txt`,
`x80-appkit-oracle.swift.txt`).

The oracle could not show the platform's own answer. With synthesized HID drags, AppKit sent no tracking-area
event during a drag in any configuration, and none at `mouseUp`, though the header says it does. So what
`.enabledDuringMouseDrag` does is read from the SDK header, not observed. With a hardware mouse, the host's areas
should, by the header, send the pressed row's leave as the pointer leaves it and the last row's enter at
`mouseUp`. Still, no row the pointer crosses hears an enter while the button is down.

Not run: a hardware mouse or trackpad, #327's head, an inline run's hover during a drag, iPadOS with a pointer, and
the Linux and Windows hosts.

## Constraints

- The web is the parity oracle (`CLAUDE.md`). Contract `hover` is DOM's `pointerenter`/`pointerleave` on the web
  hosts, and a mouse drag captures no pointer there. LLP 1115: author > platform > CSS default. The author wrote
  `hover=`, which on the web follows the pointer while a button is held.
- During a mouse drag, the hover follows the hit under the pointer. The node the button went down on leaves when
  the pointer leaves it, and each `hover` node the pointer crosses enters and leaves. At the release, the node under
  the pointer is the hovered one, with no further move.
- The held node keeps its drags, its `pointermove`s and its `pointerup` wherever the pointer goes (LLP 1056 §3,
  `pointerDragged`). The press's own tracking (`pressFollows`), text selection and the mouse chain's gestures stay as
  they are. Only the hover changes.
- How the host gets there is open. By its header, `.enabledDuringMouseDrag` on the hover tracking areas is AppKit's
  answer, but a synthesized drag does not exercise it, so no script or test can see it. A hit test from the pressed
  view's `mouseDragged`, as `hoverUnderPointer` hit-tests a resting pointer, works for synthesized and hardware drags
  alike.
- It is not X62 (`issues/20261009-macos-subtree-hover.md`, #322). That issue is about which nodes a free pointer
  hovers (a subtree, an overflowing child, overlapping nodes). This one is that nothing changes while a button is
  down. A fix to either can share one hit test.
- Main PR #327 (head `14493d253`) does not fix it (read from its diff, not run). It routes `mouseEntered`,
  `mouseMoved` and `mouseExited` through `Presenter.trackPointer` and adds a viewport tracking area
  (`PageScrollView`), both with the same four options and no `.enabledDuringMouseDrag`. `trackPointer` returns at
  once while `pointerHeld` or `pointerSource` is set, and `mouseDragged` never calls its new `hoverAt`.

## Acceptance criteria

- In the repro on macOS, under a real pointer (the real-input batch), a press on A1 dragged to A4 shows A4 hovered
  while the button is held. The log reads `0A1+ 1A1- 2A2+ 3A2- 4A3+ 5A3- 6A4+`, as on the web. The same holds for
  B1 → B4, with B1's `v` after its enter.
- Released over A4, A4 stays hovered with no further move, and moving off the rows sends its leave.
- The agent's macOS drag after `tap a1 hover` gives the web's log, so a script can check it.
- A macOS XCTest drags across two `hover` nodes and checks the enter and the leave.
- Hover with no button down, the resting pointer's follow after a layout change (#139's fix), inline-run hover, and
  the held node's `pointermove` and `pointerup` keep working.
