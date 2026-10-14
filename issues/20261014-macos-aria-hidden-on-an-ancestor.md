# macOS: aria-hidden on an ancestor drops the focus

**Status:** Open
**Systems:** host/apple macOS, focus, accessibility
**Author:** Daehyeon Mun
**Date:** 2026-10-14
**Severity:** P2
**Related:** issues/closed/20261010-aria-disabled-focusable-control.md; LLP 1115

On macOS, when an ancestor of the focused node becomes `aria-hidden="true"`, the host clears the focus (the window's first responder becomes nil). In Chrome, `aria-hidden` removes the subtree from the accessibility tree only; the focused element keeps the focus and Tab continues from it. Found while building an example app: a row of action buttons becomes `aria-hidden` after a menu closes, and the menu returns the focus to one of those buttons.

Cause: `refreshInputGates` in `host/apple/Sources/ExactKit/Mac/NavigationMac.swift:211-226` computes `hidden` with `node.accessibilityHiddenByProp` (line 214; `aria-hidden` lowers to `accessibilityElementsHidden`, read by `NodeViewMac.swift:220-222`) together with `isHiddenOrHasHiddenAncestor` and `accessibilityInert`, and it resigns the first responder when `hidden` is true (lines 221-225).

Reproduction: focus a button inside a `column`, then set `aria-hidden="true"` on the `column` from a state change. The focus becomes null on macOS. Chrome keeps the focus on the button.

Fix: keep `aria-hidden` for `setAccessibilityHidden` only. Resign the first responder only for gates that remove the node from input: hidden views, `inert`, route inert, and dialog blocking.

Acceptance: in the reproduction, the focus stays on the button after its ancestor becomes `aria-hidden`, and Tab goes on from it. A focused node inside a subtree that becomes `inert` or hidden still loses the focus.
