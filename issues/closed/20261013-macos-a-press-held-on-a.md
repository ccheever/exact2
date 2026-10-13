# macOS: a press held on a scroll view's empty port aborts the app when the scroll view leaves with its freed parent

**Status:** Closed
**Resolution:** fixed by #523
**Systems:** host/apple macOS
**Severity:** P1
**Author:** Daehyeon Mun (via the T3 Code clone, X87)
**Date:** 2026-10-13
**Related:** 3998e650e ("an empty scroll port passes the pointer on", issues/closed/20261009-macos-empty-scroll-pointer.md); #519 (issues/closed/20261013-macos-a-click-after-a-node.md)

## Summary

The macOS host aborts (SIGABRT) when a press is held on the empty part of a `scroll` or `list` port and a batch then
removes and frees the node whose scroll view it is:

```text
*** Terminating app due to uncaught exception 'NSInternalInconsistencyException', reason: '<ExactKit.ChainingScrollView
0x...> has reached dealloc but still has a super view. Super views strongly reference their children, so this is being
over-released, or has been over-released in the past.'
AppKit           -[NSView dealloc]
AppKit           -[NSScrollView dealloc]
libobjc.A.dylib  AutoreleasePoolPage::releaseUntil(objc_object**)
```

## Where

3998e650e made an empty scroll port pass the pointer to its node (`ChainingScrollView.pointerOwner`). So that the
drags and the up still arrive when a `pointerdown` removes the scroll view, it gave `ChainingScrollView` a
`removeFromSuperview` override (`host/apple/Sources/ExactKit/Mac/ChainingScrollView.swift:16`) that refuses every
removal while its mouse is held and removes the view at the up instead. That includes the removal AppKit makes when it
frees the parent: `-[NSView _finalize]` calls `removeFromSuperviewWithoutNeedingDisplay` on each subview, and that call
reaches the override. The parent `NodeView` is freed, releases the scroll view, and the scroll view reaches `dealloc`
still holding a superview, which AppKit raises.

This is the pattern #519 fixed in `NodeView.removeFromSuperview` (keep the view only while it is in the window, where
the up can reach it). `ChainingScrollView`'s is the only other `removeFromSuperview` override in `host/apple`.

Two shapes reach it:

1. No node hears the pointer: nothing holds the scroll node, so the batch that removes it frees it, and the app aborts
   in that batch's autorelease pool pop.
2. An ancestor hears the pointer: the hold keeps the scroll node (`Presenter.pointerSource`) after its panel is freed,
   and the app aborts when the hold ends at the next click's up and the node is freed.

## Trigger seen in the T3 Code clone

Round 12's agent drive on main `46f300937` (T3 clone gap X87): `tap transcript down` (the timeline scroller's empty
area below the reply), then ⌘1 and ⌘2 (`thread.jump`). The app aborts at ⌘1's batch, before the up. The same with
`MouseChainMac.swift` from before #519, and on the merge commit with no clone change, so neither #519 nor the clone
([readouts](https://raw.githubusercontent.com/ccheever/exact2/d3c50deffc9d03407c6d3282e4e48f4bf9275513/adopt-main-fixes-r12/readouts.txt),
RT-1 session 2).

## Reproduction

`ScrollRoutingTests.testAnEmptyScrollPortFreedUnderAPressGoesWithItsNode`: a press on the empty port of a scroll node
inside a panel, a batch that replaces the panel and destroys both nodes, then a click on the new panel (AppKit sends the
first up to no view). Both shapes above raise the exception on main `c016978a6`. A scratch app driven in agent mode
(`tap transcript down at 60 150`, then `type field x`, whose `input` action swaps the panel) exits with SIGABRT at the
`type` (shape 1), or, with the scroll node in a column that hears `pointerdown`, at the next click's up (shape 2).

## Current scope

Fix the crash: the scroll view keeps itself in its parent under a held press only while it is in the window, as
`NodeView` does since #519. X65's behaviour does not change for a scroll view that stays in the window: a `pointerdown`
that removes it still leaves it there, transparent, until the up
(`testEmptyScrollPortKeepsTheMouseTargetWhenPointerDownHidesOverflow`).
