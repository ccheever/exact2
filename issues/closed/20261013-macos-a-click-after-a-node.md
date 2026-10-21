# macOS: a click after a node leaves the tree under a held press crashes in NodeView.releaseHold

**Status:** Closed
**Resolution:** fixed by #519
**Systems:** host/apple macOS
**Severity:** P1
**Author:** Daehyeon Mun (via the T3 Code clone)
**Date:** 2026-10-13
**Related:** c14017277 ("macOS: a held pointer outlives the child it was pressed on"); LLP 1005 ("`pointerdown` and `pointerup`", macOS)

## Summary

The macOS host crashes (EXC_BAD_ACCESS, SIGSEGV) on a mouse-up. The crash happens when, while the pointer is held,
the view the button went down on leaves the window with its parent, and that parent is then freed. Four crashes of
the T3 Code clone have the same faulting thread:

```text
libobjc.A.dylib  objc_msgSend
AppKit           -[NSView _setSuperview:] + 168
AppKit           -[NSView removeFromSuperview] + 204
ExactKit         NodeView.releaseHold()                      MouseChainMac.swift:383 (main: :384)
ExactKit         $defer #2 in NodeView.pointerReleased(_:)   MouseChainMac.swift:134
ExactKit         NodeView.mouseUp(with:)                     NodeViewMac.swift:1406
AppKit           _routeMouseUpEvent / -[NSWindow sendEvent:] / -[NSApplication run]
```

## Where

`-[NSView _setSuperview:]` + 168 is the return from `[self->_superview isHiddenOrHasHiddenAncestor]`. The ivar is
the same one it then passes to `_removeSubview:`. So the view that `releaseHold` removes points to a superview that
has been freed.

`NodeView.removeFromSuperview()` (`host/apple/Sources/ExactKit/Mac/MouseChainMac.swift:347`) keeps the hold's source
view, transparent, in its parent while the pointer is held (c14017277). It refuses every removal it is asked for. That
includes the one AppKit makes when the parent itself is freed: `-[NSView _finalize]` calls
`removeFromSuperviewWithoutNeedingDisplay` on each subview, and that call reaches the override. The parent is freed,
but the kept view's `_superview` still points to it. At the mouse-up, `releaseHold` calls `super.removeFromSuperview()`
on that view, and AppKit sends a message to the freed parent.

This oracle shows AppKit's side with no Exact code. A child whose `removeFromSuperview` returns early stays a subview
of a parent that is being freed:

```swift
class C: NSView { override func removeFromSuperview() { print(Thread.callStackSymbols[2]) } }   // refuses
// parent.addSubview(c); window.contentView!.addSubview(parent); parent.removeFromSuperview(); parent = nil
//   prints: -[NSView removeFromSuperviewWithoutNeedingDisplay] + 44   (called from -[NSView _finalize] + 684)
// c.superview  → the freed parent: reading it crashes (exit 139)
```

A hold can outlive its button in two ways. A native menu can take the up: `MenuHost.track` runs `NSMenu.popUp` for a
right-click's context menu, and the menu eats the `rightMouseUp`. Or AppKit can send the up to no view, because the
view the button went down on has left the window. In both cases `pointerHeld` and `pointerSource` stay set until the
next mouse-up anywhere. If the held node stays in the tree (an ancestor that hears pointer events), the hold stays
alive while a batch removes and frees the row or panel the press landed on. The next click's mouse-up then runs
`releaseHold` on that view.

## Triggers seen in the T3 Code clone (realinput-1011c)

1. Right-click a sidebar row, then pick "Archive thread" from its native context menu (the row leaves the tree). Then
   click another button: Settings in the sidebar footer. A click on the same Settings button before the archive did
   not crash (G4-14, `T3 Code (Exact)-2026-10-12-225830.ips`).
2. Right-click a thread, choose Delete, then click Confirm in the dialog. The clone crashed on Confirm's mouse-up, and
   the delete was never sent (LSF-6, `-2026-10-13-035103.ips`).
3. While a work draft has the file viewer open in the right panel, click Settings in the sidebar. This crashed twice:
   B13 R11-X79 (`-2026-10-13-051210.ips`) and, after a relaunch into the same state, B14 R11-X51
   (`-2026-10-13-051845.ips`).

All four crashes are on the T3 clone build `1f813789b`, whose `MouseChainMac.swift` is main's apart from a two-line
hunk above this code. The test plan in the T3 Code clone carries the input logs and the crash summaries
(`target/t3-ui-parity/realinput-1011c/`).

## Reproduction

Main `48755525c`, macOS 26.6.2 (25G83), Apple Silicon. A scratch app made with `bun scripts/exact.mjs new <dir>`
(`app.ts` exports an empty `sources`). The layout hears the pointer, as the clone's does with its right panel open,
and its `pointerdown` swaps the panel the Settings view is in:

```text
component Holdcrash
  state flipped = false
  …
  action held
    downs = downs + 1
    flipped = not flipped
  view
    column testId="layout" pointerdown=held pointerup=lifted …
      when flipped
        column testId="panel-b" …
          column press=open testId="settings-b" role="button" …
      else
        column testId="panel-a" …
          column press=open testId="settings" role="button" …
```

`bun exact.mjs agent macos --size 420x260 "tap settings down" "tap up" "tap settings-b down" "tap up"`. AppKit sends
the first up to no view, because the view pressed has left the window, so the hold stays. The second click's up then
ends with "the app exited (killed by SIGSEGV)", with the faulting thread above (`Holdcrash-2026-10-13-210322.ips`).
Two AppKit XCTests crash at the second click's up in the same frames (xctest exit 139).
`GesturePrecedenceMacTests.testARowFreedUnderAHoldTakesThePressedViewWithIt` has a menu take the right-click's up.
`testAPanelReplacedUnderAClickTakesThePressedViewWithIt` takes the steps above: the first click has no up, and the
second click is a down and an up on the new button.

## Current scope

Fix the crash. A hold must never keep a view in a parent that is going away, and the hold must be dropped safely,
as a hand-built AppKit app never crashes. Press and hold behaviour does not change for nodes that stay: the source is
still kept in the window, transparent, while the button is held (fix/syntax6's board drag). Also, when the button
comes up after a release has cleared the held node, the hold's source is let go, and hover is no longer frozen until
the next press. Out of this scope: delivering an up that a menu took, or that came to no view, when it happens rather
than at the next click's up. That changes the hold for a node that stays (QUEUE.md).
