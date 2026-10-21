# Code review: a real touch aims at what a grouped list draws, round 2 (3603b7703), 2026-10-04 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `3603b7703`.
- **Method:** one brief (sha256 `2f6205610bc5681baa010475c31a2e4fd3b8edc0660409bd899dbd15ff36370e`), shared with grok. Round 2, blind to the other round-2 review; it read the round-1 artifacts. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH FIXES.
- **Disposition (r3):** 1 taken: the dispatch log's landing records, for a touch in a grouped list's cell, `projected: {row, part}` (part `cell`, `switch` or `detail`); `aim` replies the same for its hit, and `realTap` refuses a touch whose row or part differs, so a list update between aim and touch-down that moves another row's control under the finger fails instead of passing on the list's node. Covered by `GroupedListHost.part` assertions in the unit test; DEFERRED: a race test (a list update between aim and dispatch), which needs the runner and a timed batch. 2 taken: the grok round-1 disposition now says an outside-port point is refused with the port error or "covers its middle".

---

1. **Should-fix — Delivery verification still loses the projected control’s identity.** [TouchIOS.swift:157](/tmp/x15p-review/host/apple/Sources/ExactKit/IOS/TouchIOS.swift:157)

   The new guards validate the control at aim time, but `aim.hit` and `TouchLog.landing` both reduce every projected cell/accessory to the enclosing list’s node ID. If an asynchronous list update moves or removes the requested control between the final aim and touch-down, a neighboring row or switch can receive the touch and still pass `realTap`’s landing check. Session generation does not change for an ordinary list update.

   **Fix:** Carry the projected row/control identity through both aim and the actual touch’s landing record, and compare it before returning `delivery: platform`. Cover a list update between aim and dispatch; checking the UIKit class alone cannot distinguish two switches.

2. **Nit — The disposition overstates which refusal message wins.** [code-2026-10-04-grouped-platform-tap.grok.md:7](/tmp/x15p-review/llp/reviews/code-2026-10-04-grouped-platform-tap.grok.md:7)

   It says an outside-port point receives the port error, but `obscured()` runs first. A point outside the collection that hits a sibling or non-`NodeView` ancestor receives “covers its middle” instead. It correctly refuses the touch.

   **Fix:** Describe this as an outside-port refusal without guaranteeing the port-specific message.

Both original aim-time failure paths are fixed. Partially visible cells and valid viewport-coordinate overrides remain accepted when the chosen point hits the requested view or a descendant. No static Swift/tvOS compilation issue found. The resolver test is valid; the documented aim-level regression coverage remains deferred. I did not run builds or tests.

Verdict: LAND WITH FIXES