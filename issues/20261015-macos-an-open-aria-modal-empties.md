# macOS: an open aria-modal empties the window's accessibility tree for VoiceOver

**Status:** Open
**Systems:** host/apple macOS, accessibility
**Author:** Daehyeon Mun
**Date:** 2026-10-15
**Severity:** P2
**Related:** issues/closed/20261009-macos-role-modal-accessibility.md; issues/20261013-macos-axpress-on-a-button-whose.md (possibly the same mechanism); LLP 1080.003 D3

While any `aria-modal` view is shown, another process (VoiceOver, Accessibility Inspector, an `AXUIElement` client) reads the window as one AXScrollArea with no children. AXChildren, AXChildrenInNavigationOrder and AXContents of the viewport answer kAXErrorAttributeUnsupported (-25205), and the AXParent of the focused control inside the modal cannot be read. VoiceOver says "empty, scroll area". In-process `tree --ax` shows the projection correctly, so the agent's tree does not reveal the failure. The modal's kind does not matter: an ignored row, a `role="dialog"` popover painted in the page, and an in-page `role="alertdialog"` all fail.

Cause: AppKit drops the reply to a remote request when an override serving it calls one of AppKit's accessibility getters. PageScrollView serves AXChildren and AXContents from `Presenter.macModalAccessibilityChildren` (`host/apple/Sources/ExactKit/Mac/PageScrollView.swift:29-37`), and NodeView serves AXParent through `modalAccessibilityParent` (`Mac/NodeViewMac.swift:370-376, 393`). Both evaluate `macAccessibilityModal`, which calls `isAccessibilityHidden()` on the modal, on each of its ancestors and on each candidate cover (`Accessibility.swift:559, 576, 578, 582`). AppKit receives the projected array (`_accessibilityValueForAttribute:clientError:` returns it with no client error), but `CopyAppKitUIElementAttributeValueNoCatch` returns nothing and the client gets -25205.

A plain AppKit app reproduces it: an NSScrollView whose `accessibilityChildren` and `accessibilityAttributeValue(.children)` return one deep subview is read correctly until the override calls `isAccessibilityHidden()` on any view; then the reply is -25205. Calling `isAccessibilityElement()` or `accessibilityRole()` there does no harm. (`NavigationMac.swift:215` already notes that this getter takes the legacy unsupported-attribute path.)

Reproduction: show a `column role="alertdialog" aria-modal=true aria-label="Confirm"` holding a button, then read the window's scroll area with `AXUIElementCopyAttributeValue(…, kAXChildrenAttribute, …)` from another process: -25205.

Fix: never call AppKit's accessibility getters while serving an accessibility request. Decide the modal from the host's own state (`isHiddenOrHasHiddenAncestor`, `accessibilityInert`, `accessibilityHiddenByProp`, or the hidden flag `refreshInputGates` writes), or compute it once in `syncAccessibility` and store it so that the AX overrides only read the stored value. When it changes, post `.layoutChanged` on the viewport with the modal in `uiElements`, so that VoiceOver moves into the dialog and reads its name, as it does for a web dialog.

Acceptance: with an `aria-modal` dialog, popover or plain view shown, an out-of-process read gives the viewport exactly the modal as its child, the focused control's AXParent chain reaches the window, and VoiceOver reads the modal's role, name and controls (slider values included). A test fails if the projection path calls `isAccessibilityHidden()` (a view subclass that counts the getter). Without a modal, the tree is unchanged.
