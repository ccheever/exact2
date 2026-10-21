# Code review: the dev menu's four-finger gestures fail at once on a one-finger touch (3ce950fcf), 2026-10-06 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `3ce950fcf`.
- **Method:** one brief (sha256 `028eb82bba7c6827c4fcddcd3e72c9abd910b91b03e5e05cb29db2bb8cb19109`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition:** 1 taken: the test resets while the last attempt's moment is pending, checks the new attempt survives it and fails at its own, and that a four-finger attempt's peak does not carry into the next. 2 not taken: a real staggered four-finger tap, double tap and hold cannot be driven here (exact2's touch runner makes one-finger touches); the one-finger path was measured with real touches (all three fail 69 ms after touch-down).

---

LAND WITH CHANGES

1. **MINOR — The reset/stale-timer test cannot catch either regression.** [DevMenuIOSTests.swift:87](host/apple/tests/ExactKitTests/DevMenuIOSTests.swift:87) drains the old timer before resetting, then immediately asserts `.possible`. Removing `reset()`’s contents or the timer’s attempt check would still pass. **Fix:** reset while a timer is pending; start another attempt; assert it survives the old deadline and fails at its own. Separately verify that a four-finger attempt followed by a one-finger attempt clears `peak`.

2. **MINOR — Successful gestures and subclass wiring remain untested.** [DevMenuIOSTests.swift:70](host/apple/tests/ExactKitTests/DevMenuIOSTests.swift:70) supplies synthetic counts to a stub; [line 100](host/apple/tests/ExactKitTests/DevMenuIOSTests.swift:100) checks only subclass identity. Deleting the subclasses’ gate calls would leave these tests green. **Fix:** exercise the installed recognizers with real touches: staggered four-finger tap, double tap with a pause exceeding 0.1 s, hold with movement after all four land, and a subsequent one-finger drag. Assert actions and failure timing.

No material production defect established. The guarded `.possible → .failed` transition follows [UIKit’s state machine](https://developer.apple.com/documentation/uikit/about-the-gesture-recognizer-state-machine); the dispatched block runs on main and checks the attempt. Peak retention preserves the double-tap pause. tvOS’s existing direct-touch-only filtering is unchanged.

No prior fail-fast review files were present. The idle-tick dispositions remain intact; this diff changes no batch skipping or projection paths. Both files satisfy the size limit. Static review only; UIKit tests were not run in this read-only checkout.