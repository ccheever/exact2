# macOS: a press on a `scroll`'s empty area reaches no node, so neither it nor an ancestor hears `pointerdown`

**Status:** Open
**Systems:** host/apple macOS, scrolling, pointer events
**Severity:** P2
**Author:** daehyeon-mun (GitHub report); Codex (filesystem transfer)
**Date:** 2026-10-09
**Related:** https://github.com/ccheever/exact2/issues/317

## Current scope

Deliver empty-scroll-port presses to the scroll node/nearest listening ancestor. Preserve held pointer ownership and scrolling; check empty virtualized lists and horizontal/page scrollers with real and agent input.

Transferred at exact2 `5e8da7027` on 2026-10-09. This preserves reported evidence; this triage has not reproduced or fixed the runtime behavior. The current scope and Charlie's decisions below supersede conflicting proposals/acceptance in the original report. This file is the live issue after the GitHub copy is closed.

## Original report

### Request and background

A `scroll` node's content is often shorter than its port, which leaves an empty area below or beside it. On the web a press there targets the scroll element itself, so its `pointerdown` runs, or the nearest ancestor's when the scroll has none. On macOS no node hears that press. Neither the `scroll` node nor any ancestor runs `pointerdown` (or `pointerup`). A press on the content inside the same port is delivered as expected.

The consumer is T3 Code's Usage page. Like Base UI's `useDismiss`, it closes a pinned popover on a press outside it. The page's root column hears `pointerdown` and hit-tests the popover. On macOS a press on the page below the cards (`tap usage-scroll clicks 1 at 600 700`, below the 604-pt content of a 788-pt port) did nothing, and the journal held no `pointerdown`. The clone now puts a full-height column (`min-height="100%"`) under the scroll content, so that every press lands on a node.

### Current and expected behavior

- **Current (macOS):** a platform click on a port's empty area runs nothing:
  - on `port-a`, whose ancestor `outer` hears `pointerdown`, `outer` stays at 1;
  - on `port-b`, which hears `pointerdown` itself, `port` stays at 1.

  Clicks on the content run the expected handler on both hosts.
- **Current (web):** the same clicks bring both counters to 2.
- **Expected:** on macOS a press on a `scroll` node's empty area is that node's press. `pointerdown`/`pointerup` go to the innermost node, from the `scroll` node up, that hears them, as in Chrome.

Hypothesis, from reading the source:
- A node whose overflow scrolls moves its children into a `ChainingScrollView`. Its `documentView` is a `FlippedView` (`host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:1045`, `CollectionMac.swift:10`): a plain `NSView`, not a `NodeView`.
- `pointerPressed(_:)` (`MouseChainMac.swift:104`) walks up from the pressed `NodeView` to the innermost node that wants the pointer. It is called only from a `NodeView`'s own mouse-down methods (`NodeViewMac.swift:1348, 1448`, `MouseEventsMac.swift:30`) and from native buttons (`NativeButtonsMac.swift:73`).
- A press that hits the document view or the clip view never reaches that walk.

### Reproduction and evidence

App made with `bun scripts/exact.mjs new <dir>`, with no data sources:

```text
component X65app
  state outer = 0
  state port = 0
  action downOuter(e: PointerEvent)
    outer = outer + 1
  action downPort(e: PointerEvent)
    port = port + 1
  view
    row width="100%" height="100%" gap=16 padding=16 box-sizing="border-box" background-color="#ffffff" color="#111111"
      // A: only an ancestor of the scroll hears pointerdown (the clone's Usage page)
      column testId="outer" width=200 height=240 pointerdown=downOuter border-width=1 border-style="solid" border-color="#999999"
        text `outer ${outer}` testId="count-a"
        scroll testId="port-a" flex=1 min-height=0 width="100%" background-color="#eef2ff"
          box testId="content-a" height=60 width="100%" background-color="#cccccc"
      // B: the scroll itself hears pointerdown
      column width=200 height=240 border-width=1 border-style="solid" border-color="#999999"
        text `port ${port}` testId="count-b"
        scroll testId="port-b" flex=1 min-height=0 width="100%" pointerdown=downPort background-color="#eef2ff"
          box testId="content-b" height=60 width="100%" background-color="#cccccc"
```

Ops (each tap is a platform click; `at 100 150` is inside each port, below its 60-pt content): `"tap content-a clicks 1" "tap port-a clicks 1 at 100 150" "tap content-b clicks 1" "tap port-b clicks 1 at 100 150" "clock settle" tree`.

| Scenario | Setup / reset / exact commands | Platform / OS / device | Framework revision | Actual result | Expected result | Evidence |
|---|---|---|---|---|---|---|
| Presses on content and on the empty port, macOS | `bun exact.mjs mac`; `bun exact.mjs agent macos --size 480x300 <ops>` | macOS 26.6.2, Apple Silicon | main `b896050d7` | `outer 1`, `port 1`: only the two presses on the content counted | `outer 2`, `port 2` | [record](https://raw.githubusercontent.com/ccheever/exact2/547469bb7a0e7238708e45073656ae7b3eda899a/file-x48-x68/x65-record.txt), [screenshots](https://raw.githubusercontent.com/ccheever/exact2/b4a9eefaa39b1bc5fe2a510c6920541c866a62ff/file-x48-x68/x65-macos-vs-web.png) |
| Same, web | `bun exact.mjs agent web --size 480x300 <ops>` | Chrome 154 (the agent's) | same | `outer 2`, `port 2` | (the reference) | same |

![After the same four presses: macOS shows outer 1 and port 1; the web shows outer 2 and port 2](https://raw.githubusercontent.com/ccheever/exact2/b4a9eefaa39b1bc5fe2a510c6920541c866a62ff/file-x48-x68/x65-macos-vs-web.png)

The press path above is unchanged since `e200397ec`: the only later change to these files, `84e140ee5` (native buttons), rewrote a key-target lookup in `CollectionMac.swift`.

### Acceptance criteria

- On macOS the repro ends at `outer 2` and `port 2`, with the agent's click and with a real click.
- `pointerup` and a held pointer's `pointermove`s follow the same node. A `press` on the scroll node or an ancestor fires as on the web.
- Scrolling (wheel, trackpad, scroller drag) and a list's virtualized rows behave as before. A press on the empty area of a virtualized `list` is checked too.

### Constraints and related work

- Workaround: put a full-height column (`min-height="100%"`) under the scroll content, so the empty area is covered by a node.
- Not the same as #281, which is the opposite failure: a click inside an open popover also presses the page control under it, so too many nodes hear the press. #277 is about scroll restoration and `scroll-padding`, not hit testing.
- Not tested: a right-click or `contextmenu` on the empty area, iOS, a horizontal scroller, and a page-level scroll (`PageScrollView`).
