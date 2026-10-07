# Code review r2: an idle timer tick only moves the clock (557c19407..73afe0df8), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `73afe0df8`.
- **Method:** one brief (sha256 `6a97ff22afe741ddd7213ecae32d559ffb79670692b9507c964c721cf868958c`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r3):** finding 1 is taken: `requestControlsSync` becomes `requestProjectionSync` and runs the tab-bar, control and grouped-list steps an empty batch ran (grouped lists prepared, then remounted, which writes the collection's frame and refreshes switches). A list's or tablist's appearance change requests it too. Finding 2 is taken in part: a grouped-list test of an out-of-batch frame and checked state, coalesced; the earlier timer's invalidation, a successor deadline and `clockDue == nil`. DEFERRED: GPU-completion and sampler-seq tests (no deferred artifact or sampler in the test host).

---

Static review of `557c19407` through `73afe0df8` (HEAD). No files modified or tests run.

1. **Should-fix — The replacement refresh misses other native control projections.**  
   [PresenterIOS.swift:28](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:28), [PresenterIOS.swift:989](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:989).

   `requestControlsSync` refreshes only `ControlHost`, but the skipped presenter pass also refreshed `SegmentHost` and `GroupedListHost`.

   A concrete appearance case is a grouped list containing a switch with a `light-dark()` accent. Change only that subtree’s appearance. The new callback refreshes the source node’s control, but the visible cell uses a **separate** switch, created at [GroupedListIOS.swift:398](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:398). Its resolved tint is refreshed by the grouped list’s `mount` path, at [GroupedListIOS.swift:434](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:434). The visible switch therefore remains stale until another applied batch. Projected tab bars likewise retain the resolved tint written at [SegmentsIOS.swift:256](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/SegmentsIOS.swift:256).

   Geometry has the same gap: replaying a background tablist or grouped-list frame after the second fullscreen dismissal updates its `NodeView`, but the projected view’s frame is written separately at [SegmentsIOS.swift:234](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/SegmentsIOS.swift:234) and [GroupedListIOS.swift:328](/tmp/x11-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:328). With unchanged viewport dimensions, the new callback does not repair those frames.

   **Fix:** Route appearance and geometry invalidations to the affected native projections as well. Add narrow grouped-list refresh/mount and segment refresh paths, or make their colors and geometry update directly from traits/layout. Cover descendant-only appearance changes and dismissal replay without an unrelated batch.

2. **Nit — Round-1 test coverage is only partially completed.**  
   [IdleTickTests.swift:10](/tmp/x11-review/host/apple/tests/ExactKitTests/IdleTickTests.swift:10), [controls.rs:48](/tmp/x11-review/host/apple/tests/it/controls.rs:48).

   The real future deadline, timer-validity assertion, decode test, and independent face/option cases are improvements. However, the tests still do not exercise deferred GPU completion, skipped-tick sequence attribution, trait-triggered synchronization, geometry replay, or control invalidation through fill/feedback and snapshot shortcuts. The nil case checks that the stored timer disappears, not that the previously armed timer was invalidated.

   **Fix:** Add focused regressions for those paths. Retain the armed timer and assert invalidation plus `clockDue == nil`; test a successor deadline; feed a sequenced batch through `applyTick` and inspect the next sampler record. Include option removal and the projection cases in finding 1.

Round-1 disposition:

| Finding | Assessment |
|---|---|
| Deferred GPU surface work | **Resolved.** Both load callbacks call `drainSurfaceWorkNow`, including failure. |
| Sampler sequence on skipped ticks | **Resolved.** [Session.swift:914](/tmp/x11-review/host/apple/Sources/ExactKit/Session.swift:914) records the range with zero presentation cost. |
| Subtree appearance and sizing traits | **Resolved for `ControlHost`; incomplete across projected controls**, as finding 1 describes. |
| Geometry replay outside `apply` | **Resolved for `ControlHost`; incomplete across projected views**, as finding 1 describes. |
| Tests | **Partially resolved**, as finding 2 describes. |

The requested lifecycle audit found no additional definite correctness regression:

- Queued control synchronization can survive teardown or restart, but retains no old node IDs. Reset clears the indexed tree; a later callback sees either an empty tree or the replacement tree.
- Main-queue scheduling keeps ordinary callbacks outside synchronous `apply`; the explicit guard also prevents synchronization while `applying`.
- Requests coalesce into one queued callback. An intervening full apply does not consume that callback, so a redundant synchronization remains possible.
- Every node already ran the appearance callback. This change adds a constant-time kind check; only control nodes request synchronization.
- The surface drain clears its pending queue before dispatching execution asynchronously. Destruction and generation checks protect execution/completion; the added entry point does not synchronously recurse into surface fulfillment.
- `clockDue` remains privately writable. Its new internal getter is safe and introduces no new mutation path.

Verdict: LAND WITH FIXES
