# [Bug] macOS: a window minimized with ⌘M still reads visible

**Status:** Open
**Systems:** host/apple macOS, page facts
**Severity:** P3
**Author:** Daehyeon Mun (via the T3 Code clone)
**Date:** 2026-10-13
**Related:** LLP 1069.000 D2 (`exactPage().visibilityState`); T3 Code clone realinput-1011c G2-08 (https://github.com/ccheever/exact2/blob/feat(example)/t3-code/examples/t3-code/STATUS.md)

## Summary

On macOS `exactPage().visibilityState` stays `visible` after the app's only window is minimized (⌘M, Window ▸
Minimize, the yellow button), and no `visibilitychange` reaches the page. In Chrome, and in an Electron app, the same
minimize makes `document.visibilityState` `hidden` and fires `visibilitychange`; restoring it from the Dock makes it
`visible` again.

`PageFacts.hidden` (`host/apple/Sources/ExactKit/PageFacts.swift:55-60`) is true when the app is hidden (⌘H) or when
every window in `NSApp.windows` that `isVisible` lacks `.visible` occlusion. A minimized window is not `isVisible`
to AppKit, so it is filtered out; with no window left the list is empty and the answer is "not hidden". An app whose
windows are all minimized therefore reads visible. `PageFacts.observe` listens for hide, unhide, occlusion, key and
activation changes, not for `NSWindow.didMiniaturizeNotification` or `didDeminiaturizeNotification`, so even a
correct reading would not be told to the page at the moment of the minimize or the restore.

## Why this arose

The T3 Code clone reports the window's activity to its server from `exactPage()`'s `visibilityState`, as the
reference (Electron) does from `document.visibilityState`. In realinput-1011c (G2-08) the reference went to "not
visible, background" within a second of ⌘M; the clone stayed "visible, active" and kept its foreground lease until the
window was restored. ⌘H, unhide, restore and occlusion by another window matched.

## Reproduction

Main `c5d3b3f8e5e466278488bf6f822b81de3b14138c`, macOS 26.6.2 (25G83), Apple Silicon. Any app with one window,
launched outside the agent (or under the agent with `EXACT_DEVICE=real`, which reads the machine's facts instead of
the drive's): minimize the window, then read `exactPage().visibilityState` (or the agent's `state`, `page`
`visibility-state`).

| Scenario | Platform | Actual | Expected | Evidence |
|---|---|---|---|---|
| ⌘M on the only window, then a Dock click to restore | macOS, the clone, real input | `visible` throughout | `hidden` after ⌘M, `visible` after the restore | G2-08 record, "clone" |
| The same on the reference | Electron (Chromium), real input | `hidden` within ~0.8 s of ⌘M; `visible` after the restore | (the oracle) | G2-08 record, "ref" |
| ⌘H, unhide, restore, occlusion | macOS, the clone | match the reference | — | G2-08 record |

Record: https://raw.githubusercontent.com/ccheever/exact2/ce58c483fa3c5994a24469f0da7a8444bc19b15a/realinput-1011c/b3-window-activity/G2-08-states.txt

## Constraints

- A window behind another app's stays visible, as now; a fully occluded window stays hidden, as now (LLP 1069.000 D2).
- One minimized window beside another that is on screen and not occluded leaves the app visible: the page reads the
  whole app, as `NSApp.isHidden` does.
- A window that is closed or ordered out, neither on screen nor in the Dock, still counts for nothing.
- Under the agent the drive's `prefer visibility-state` still stands in (LLP 1069.007 D8).

## Acceptance criteria

- With every window minimized, `PageFacts.hidden` is true, and `visibilitychange` reaches the page at the minimize.
- Restoring a window is told to the page at the restore, and it then reads as its occlusion says.
- An AppKit XCTest minimizes and restores a window and checks both.
