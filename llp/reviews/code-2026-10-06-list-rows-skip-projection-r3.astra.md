# Code review: a batch of list rows alone skips the route projection, round 3 (4d019ae0c), 2026-10-06 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `4d019ae0c`.
- **Method:** one brief (sha256 `b2b1f1b613d37488d44f12d3c596e31aafdab1c1e91b6d3f58537eb2e8a989d5`), shared with grok. Round 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition:** 1 taken: an `id` naming any route's `navigationScroll` is projected, so a create or props introducing one counts, and a `children` op placing anything the projection reads (a named scroller, a tablist, the Back control, a route) or a node created with projected props counts. 2 taken: retiring a presented navigation (UIKit's own dismissal included) sets `nativeMoved`, so the next batch syncs whatever it holds. 3 not taken: no mounted recovery tests; the navigation suites (pop, transitions, sheets, tabs) run with a session and pass. Three rounds reached; landed with these fixes.

---

DO NOT LAND

1. **MATERIAL — Newly named scrollers still bypass projection.** [ListRowsIOS.swift:39](host/apple/Sources/ExactKit/IOS/ListRowsIOS.swift:39) protects existing `chrome.named` entries, but `projected` excludes only Back names. With a route declaring `navigationScroll="feed"`, creating a `scroll id="feed"` inside an existing virtualized row—or renaming an existing scroller to `feed`—passes the classifier. The route hook never receives the new scroller, although its identity changes the projection signature ([NavigationBarIOS.swift:433](host/apple/Sources/ExactKit/IOS/NavigationBarIOS.swift:433)). Hook-configured scrolling remains stale. **Fix:** reject creates/props introducing any referenced scroll name, including same-batch placements; test creation and renaming. Round 2’s disposition remains incomplete.

2. **MATERIAL — Native modal dismissal can lose reconciliation.** [ModalIOS.swift:479](host/apple/Sources/ExactKit/IOS/ModalIOS.swift:479) retires the presentation before dispatching Back. [NavigationIOS.swift:514](host/apple/Sources/ExactKit/IOS/NavigationIOS.swift:514) records no sync debt, and `modalDidDismiss` requires `pendingSync`. Concrete case: dismiss a settled modal whose Back handler edits only a list row while retaining the declared route. With unchanged viewport geometry and no navigation transition, that batch skips synchronization; the modal stays absent. Previously the batch retried the declared presentation. **Fix:** record debt when UIKit dismisses a modal, and reconcile after retirement even when Back produces only row changes.

3. **MINOR — Deferred recovery remains untested.** The gate test uses a sessionless presenter and checks attempted-call counts ([ListRowsIOSTests.swift:84](host/apple/tests/ExactKitTests/ListRowsIOSTests.swift:84)). Removing `syncOwed` still passes it. **Fix:** add mounted tests for first-draw/refused-presentation recovery and native dismissal followed solely by row updates; include tvOS coverage.

The operation-switch, header/tab exclusions, existing-tablist tint protection, and early-return debt fixes hold. Both visibility guards and the idle-tick refresh paths look sound. Changed sources meet the line cap. UIKit tests were not run; the Swift probe was blocked by sandboxed module-cache writes.