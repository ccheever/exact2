# macOS: Tab from a window that is its own first responder focuses nothing

**Status:** Closed
**Resolution:** fixed: with the window as its own first responder, the key route's tabFromStart sends Tab to the loop's first stop and Shift-Tab to its last, in both Keyboard Navigation modes (KeyboardNavigationMacTests, InitialFocusMacTests testTabFromTheWindowItself…)
**Systems:** host/apple macOS, keyboard
**Severity:** P2
**Author:** Daehyeon Mun
**Date:** 2026-10-16
**Related:** LLP 1115.000 D7.2

On macOS, when the window itself is the first responder (nothing in it is focused: after `blur()`, or after a menu a click opened closes), Shift-Tab focuses nothing in either Keyboard Navigation mode, and with Keyboard Navigation on Tab focuses nothing too; no focus ring appears. Tab from a focused control moves through the loop normally. In AppKit, Tab from a window that is its own first responder goes to the window's `initialFirstResponder` and on to its first valid key view, and Shift-Tab to the last (`NSWindow.selectNextKeyView(_:)`, `selectPreviousKeyView(_:)`). Found in the T3 Code clone's real-input session: with Full Keyboard Access on, six Tabs after a pointer-opened menu closed focused nothing.

Cause: the app window's `initialFirstResponder` is the session's `ExactView` (`host/apple/Sources/ExactMac/main.swift`), which is no key view and has no `nextKeyView`; the loop the presenter builds starts at the viewport inside it (`host/apple/Sources/ExactKit/Mac/KeyViewLoopMac.swift`, `syncKeyViewLoop`). So AppKit's search from the window finds nothing. The host's own route for a window holding the focus (`tabFromStart`) only handled the place a disabled control left.

Fix: in the key route (`Presenter.routeKey` → `tabFromStart`), when the window holds the focus and the session hears the window's keys, Tab selects the key view following the viewport (the loop's first stop) and Shift-Tab the loop's last, in both Keyboard Navigation modes (the loop already follows the setting).

Acceptance: with the window as first responder, Tab focuses the first stop (Keyboard Navigation off: the first text field, or a box that takes keys of its own; on: the first control) and Shift-Tab the last, with its focus ring; XCTests drive both modes.
