# 2026-11-05 — launch readiness and tab host

## Rough

The iOS Home first pixel waited behind draft recovery and live stream hydration,
so a cold launch could stay in the loading state. The Home tab controller was
also mounted under the route's Exact viewport. Moving its view to a window
surface without moving its controller parent made UIKit terminate the process
with `UIViewControllerHierarchyInconsistency`.

## Lean in

Home publishes the local catalog and focused status immediately, then prepares
draft and live data in the background. The tab controller now belongs to the
window root controller and its overlay is constrained to that root view, which
keeps the native tab bar outside the scroll viewport while preserving tab
selection events.

## Needed

The submitted TestFlight artifact must be checked after Apple processing and
internal tester installation. A connected environment still needs an end-to-end
pairing and chat-flow check on the release build.

## Checkpoints

- Focused catalog/readiness tests: 26 passed, 0 failed, 114 assertions.
- Native iOS simulator build and launch: passed on `t3-standard-pro-current`.
- Accessibility snapshot: the tab bar is a sibling of Exact's scroll area.
- Tab selection: Chats → Archived threads worked without a crash.
- Launch screenshot: `/tmp/t3-home-fixed.png`.
