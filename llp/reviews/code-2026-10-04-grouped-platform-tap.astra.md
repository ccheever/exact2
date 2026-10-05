# Code review: a real touch aims at what a grouped list draws (a20c20dd6), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `a20c20dd6`.
- **Method:** one brief (sha256 `d18d483ee75699a738492915d2e9103cbf5788ac67d4c646730cc01096f41ce0`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r2):** 1 taken: a projected aim's point must lie in the collection view's port and the hit must be the aimed view or a descendant (no ancestor), else refused. 2 taken: a toggle's or detail button's control whose accessory is not shown is refused ("its switch is not shown"), never the row in its place. 3 taken in part: the unit test now covers the missing-accessory refusal and the port refusal's text, and checks the port; DEFERRED: aim-level assertions (clipping, occlusion, keyboard, modal) in XCTest, which needs a live session and a foreground key window — the smoke leg drives `aim` end to end on the simulator.

---

1. **Should-fix — A clipped row can pass aim while the touch misses the list.** [TouchIOS.swift:169](/tmp/x15p-review/host/apple/Sources/ExactKit/IOS/TouchIOS.swift:169)

   `shown()` only requires the cell to intersect the collection’s bounds. If its middle lies outside the port but inside the viewport, hit-testing can return a containing `NodeView`. `obscured()` accepts that ancestor, and `aim.hit` becomes its ID. `realTap` then validates a touch on the containing node, despite missing the requested row. Host activation correctly refuses this through its explicit port check.

   **Fix:** For projected targets, validate the resolved point against the collection’s port and require the hit to be the target or its descendant. Preserve valid coordinate overrides on the visible portion. Add a regression exercising aim with a partially clipped cell and an otherwise uncovered containing node.

2. **Should-fix — Missing accessories must not silently become row taps.** [GroupedListIOS.swift:144](/tmp/x15p-review/host/apple/Sources/ExactKit/IOS/GroupedListIOS.swift:144)

   When accessory resolution fails, `?? cell` changes the requested operation. A detail-button tap can activate the row’s separate action; a switch tap can do nothing. Both can pass the landing check because the cell and accessory resolve to the same enclosing list node.

   **Fix:** Refuse an unavailable accessory explicitly. Reserve cell resolution for row targets; keep any platform-specific alternative explicit. Test a missing accessory on a pressable row and assert refusal without triggering the row action.

3. **Nit — The unit test verifies resolution, not aiming.** [GroupedListIOSTests.swift:154](/tmp/x15p-review/host/apple/tests/ExactKitTests/GroupedListIOSTests.swift:154)

   It checks `shown()` identities but never checks aim coordinates, covering refusals, or `hit`. The smoke’s state readbacks are useful end-to-end coverage for the happy paths, but neither test catches finding 1.

   **Fix:** Add aim-level assertions for clipping and occlusion, including keyboard and modal placement; rename the existing test if it remains solely a resolver test.

No additional static issue found in checkmark-row mapping, custom rows and their controls, keyboard/modal coordinate conversion, or the tvOS guards. Disabled controls can receive a real touch without changing state; that differs from activation refusal but fits the dispatch-only landing contract.

The fixture’s continuations appear compatible with the formatter; I did not execute the round-trip or tests. The LLP note accurately describes the happy path, but its refusal guarantee needs finding 1 fixed.

Verdict: LAND WITH FIXES