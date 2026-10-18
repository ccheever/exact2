# macOS: a focused button that becomes disabled keeps the focus and swallows Tab

**Status:** Closed
**Resolution:** fixed by refreshInputGates resigning the focus of a control that becomes disabled and keeping its place as Presenter.tabStart, which Tab and Shift-Tab go on from; FocusFixupMacTests
**Systems:** host/apple macOS, focus, keyboard
**Author:** Daehyeon Mun
**Date:** 2026-10-14
**Severity:** P2
**Related:** issues/closed/20260901-native-disabled-controls-active.md; issues/closed/20261010-macos-tab-from-a-focused-box.md; LLP 1115.000 D7.2

On macOS, when the focused native button becomes `disabled`, it stays the window's first responder, and Tab and Shift+Tab do nothing. In Chrome, the disabled button loses the focus (the focus goes to the body), and the next Tab continues from the button's position in the order. Found while building an example app: a button that starts an operation disables itself while the operation runs.

Cause: `NativeButtonsMac.swift` sets `button.isEnabled = written.enabled` (`host/apple/Sources/ExactKit/Mac/NativeButtonsMac.swift:238`) and does not resign the first responder. Then `keyDown` returns at its first guard, `guard acceptsFirstResponder, let owner else { return }` (line 85), because `acceptsFirstResponder` is false for a disabled button (lines 60-64). The Tab branch (lines 87-90) never runs.

Reproduction: a button whose `press` sets a state that makes its own `disabled` true. With Keyboard Navigation on, Tab to it and press Space. The focus stays on the button, and Tab does not move it.

Fix: when a focused button becomes disabled, give up the first responder, and keep its position so the next Tab or Shift+Tab goes on from it (`tab(from:back:in:)`, `KeyViewLoopMac.swift:133`). At minimum, let `keyDown` handle Tab before the `acceptsFirstResponder` guard.

Acceptance: in the reproduction, the button is no longer focused after it becomes disabled, and the next Tab moves the focus to the next stop after it, as in Chrome.
