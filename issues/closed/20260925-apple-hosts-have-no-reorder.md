# Apple hosts do not implement reorder, so `reorderFor`/`reorderdrop` does nothing on iOS or macOS

**Status:** Closed
**Resolution:** Fixed in b58430d2. iOS picks a row up by its grip (pan or long press) and macOS by the mouse; the lifted row follows, siblings make room, a virtualized list autoscrolls at its edges and pins the lifted row, and the drop sends the same `reorderdrop` as the web and Linux hosts. Driven on Interaction Gallery (iOS and macOS) and the listbench copy (iOS, including an autoscrolled move); XCTests on both hosts. Closure audit 2026-09-30: archive the already-landed fix; its reproduction and verification evidence remain below.
**Systems:** Apple host (iOS and macOS), Runner (`runner/src/runner/event.rs` reorder events), Contract (`reorderFor`, `reorderdrop`; `contract/lower/src/tags.rs`), Scrolling (reorder inside `list virtualized=true`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-25
**Related:** the web and Linux implementations (`host/web/src/reorder_tests.rs`, `host/linux/src/presenter/arrange_tests.rs`, `host/linux/tests/it/arrange.rs`); `contract/cli/tests/it/reorder_collection.rs` and `reorder_binding.rs`; `apps/interaction-gallery` (Arrange)

A Contract app's drag-to-reorder works on the web and Linux hosts, and nothing on the Apple hosts drives it: no Swift source handles `reorderFor` or `reorderdrop`. The Expo PR 49975 list-demo port needed edit-mode reordering (SwiftUI `List.onMove`). On iOS it had to approximate it with a `pan` handler that jumps the row one place per 120 pt of drag, with no lifted row, no follow and no drop.

**Fix:** implement reorder on iOS and macOS with the web host's semantics: the same events, payloads and ordering, so the runner and Contract are unchanged.
- The platform recognizes the gesture: a long press or a handle drag on iOS, a mouse drag on macOS.
- The lifted row follows the pointer, siblings make room, and the drop reports the destination.
- Inside a virtualized list, the list autoscrolls near its edges and pins the lifted row, so it is not retired while dragged.
- Read the web and Linux hosts first; their tests are the specification.

**Done when:**
- Interaction Gallery's Arrange reorders on iOS and macOS with a real drag.
- The listbench demo reorders within a 10,000-row virtualized list, autoscrolling past the viewport.
- Agent-driven tests cover a drag on each host (LLP 1035.003 reproducible gestures).
- iOS and macOS XCTests pass.

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): Fixed in b58430d2. iOS picks a row up by its grip (pan or long press) and macOS by the mouse; the lifted row follows, siblings make room, a virtualized list autoscrolls at its edges and pins the lifted row, and the drop sends the same `reorderdrop` as the web and Linux hosts. Driven on Interaction Gallery (iOS and macOS) and the listbench copy (iOS, including an autoscrolled move); XCTests on both hosts.
