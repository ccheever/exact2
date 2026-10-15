# macOS: VoiceOver's navigation order is AppKit's geometric sort, not the tree order the author wrote

**Status:** Closed
**Resolution:** fixed: a node, the viewport and a scroll node's scroll view answer AXChildrenInNavigationOrder with their AXChildren, the tree order (AccessibilityTreeMacTests.testNavigationOrderIsTheTreeOrder)
**Systems:** host/apple macOS, accessibility
**Author:** Daehyeon Mun
**Date:** 2026-10-15
**Severity:** P3
**Related:** LLP 1115.000

On macOS, a node's `AXChildrenInNavigationOrder` (the order VoiceOver's VO-Right follows) is AppKit's default, a geometric sort of the children's frames. It is not the tree order, which `AXChildren` already gives. On the web, VoiceOver reads DOM order. So an element that sits inside or overlaps another one's box is read in an order the author did not write. For example, a button laid over the top-left of a textarea is read before the textarea even though it comes after it in the tree. Found in an example app: chips listed right after a textarea are read partly before it.

Cause: the host overrides `accessibilityChildren` (`host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:363`, `PageScrollView.swift:29`) but not `accessibilityChildrenInNavigationOrder`. AppKit then sorts the children by frame: it puts a child whose centre is above, or on the same row and left of, another child's centre first. Measured with `AXUIElementCopyAttributeValue(…, "AXChildrenInNavigationOrder")`: AXChildren was [textarea, button A, button B] and navigation order was [button A, textarea, button B].

Reproduction: `textarea` (70 pt tall), then a sibling `button` positioned absolutely at the textarea's top-left. From another process, read the parent's `AXChildren` and `AXChildrenInNavigationOrder`. The button comes first in navigation order.

What to change: answer `accessibilityChildrenInNavigationOrder` with the unignored `accessibilityChildren` (tree order) on `NodeView` and `PageScrollView`. Alternatively, decide under LLP 1115 that geometric order is the platform's answer and record that.

Acceptance: in the reproduction, `AXChildrenInNavigationOrder` is [textarea, button], as `AXChildren` is, and VoiceOver's VO-Right reads the textarea first.
