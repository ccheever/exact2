---
name: 20261010-x78-macos-link-url-accessibility
plan: 20261005-t3code-macos-parity
status: moved-to-main
kind: framework-gap
blocks: []
upstream_url: null
reproduced_on: null
---

# X78: On macOS a link node has no accessibility URL, and an inline link run's element is invalid outside the app, so a table cell's links are lost to VoiceOver

Moved to main `issues/20261010-macos-link-url-accessibility.md` (2026-10-10), where it is tracked. Main PR
[#412](https://github.com/ccheever/exact2/pull/412) filed it (merged `efeb92cd2`).

## Summary

Read from another process through `AXUIElement` (the API VoiceOver uses), the macOS host has two link faults. A
`link` node is an `AXLink` with no `AXURL`: `updateRoleAccessibility` (`NodeViewMac.swift`) sets the role and
never the URL. An inline link run's element answers `kAXErrorInvalidUIElement` (-25202) for every attribute:
`textAccessibilityChildren()` (`TextInteraction.swift`) makes new `InlineAccessibility` elements on every call,
and nothing keeps them. Chrome, WebKit, `NSTextView` and SwiftUI expose both kinds of link as `AXLink` with
`AXURL`. Inside the app the run is fine, so the agent's `tree --ax` (in-process) shows `link "site"` with no
finding, and the clone's module reads the run's URL.

## Why it arose

`EXACT2-GAPS.md`'s "Text context menu" entry S3, from task `20261010-shell-context-menu` (T3 PR #407, merged
`6bac646cc`). Main PR #410 checked S3 ("a native module cannot read a node's `href`") and did not file it. While
checking, the tree read from another process showed both faults (`fw-issues-20261010g/s3-record.txt`). In the clone, a
Markdown table cell draws its links as inline runs in one text node (`TableCell` → `ChatRuns`,
`markdown.contract`), so on macOS those links are invalid elements outside the app. The clone's `link` nodes
(provider docs, licenses, check details) are links with no URL.

## Clone workaround

None. The clone keeps the difference. The shell menu's Copy Link over a table cell's link still works, because the
module reads the run's `accessibilityURL()` in-process. Over a `link` node there is still no Copy Link (S3). When
main fixes the host, read the clone's chat table links and a `link` node from another process: each should be an
`AXLink` with `AXURL`, with no clone change.

## Evidence and history

- 2026-10-10, main PR #412 reproduced it on main `474999b9f` in a one-file app: a `link href` node and a
  paragraph "Visit the site today" whose run "site" has an `href`. On macOS 26.6.2, read from another process, the
  `link` node was an `AXLink` whose `AXURL` was unsupported (-25205). The paragraph's one child answered -25202 for
  every attribute on three asks, and a hit test over "site" returned the same invalid element. The agent's
  `tree --ax` showed `link "site"` with no finding. In Chrome 155 the Exact web build had `link … url=…` for both
  (CDP), and headed Chrome on macOS gave `AXLink` with `AXURL`. A `swiftc` oracle gave `AXLink` with `AXURL` for an
  `NSTextView` link run, a SwiftUI `Link` and a `WKWebView` `<a href>` (inline too). Its `NSView` with an
  `NSAccessibilityElement` link child made fresh per call gave -25202, and the same child kept by the view gave
  `AXLink` with `AXURL`. VoiceOver was not turned on; what it means for VoiceOver is reasoned from the AX API. Main
  PR #327 does not touch accessibility (read from its diff). Image:
  [x78-link-url-ax-web-oracle-macos.png](https://raw.githubusercontent.com/ccheever/exact2/fa666be7faeaf4560af70be1d83601bfe7b98ec4/fw-issues-20261010h/x78-link-url-ax-web-oracle-macos.png);
  record: [x78-record.txt](https://raw.githubusercontent.com/ccheever/exact2/fa666be7faeaf4560af70be1d83601bfe7b98ec4/fw-issues-20261010h/x78-record.txt).
