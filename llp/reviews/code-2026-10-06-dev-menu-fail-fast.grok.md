# Code review: the dev menu's four-finger gestures fail at once on a one-finger touch (3ce950fcf), 2026-10-06 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `3ce950fcf`, plain output.
- **Method:** one brief (sha256 `028eb82bba7c6827c4fcddcd3e72c9abd910b91b03e5e05cb29db2bb8cb19109`), shared with astra. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND.
- **Disposition:** 1 taken in part: the stale moment and a fourth finger landing 60 ms after the first are tested; the subclasses are not driven with synthesized events.

---

LAND

No earlier `llp/reviews/code-2026-10-05-dev-menu-fail-fast*` round exists. The dev-menu trigger dispositions still hold: siblings stay exclusive (`DevMenuIOS.swift:101-103`, `:144`, `:156-157`), the hold still cancels (`:155`), `delaysTouchesEnded` stays false (`:138`, `:143`, `:153`), and `shouldReceive` still requires a non-empty touch event (`:104-109`).

1. **MINOR** — `testOneFingerFailsTheFourFingerGesturesAtOnce` never lets a stale timer fire, and nothing drives `FourFingerTap` / `FourFingerHold`. `DevMenuIOSTests.swift:89-92` calls `reset()`, begins again, and asserts `.possible` on that same turn. Dropping `attempt == this` (`DevMenuIOS.swift:39-42`) still passes. Both `began`s in the four-finger case (`DevMenuIOSTests.swift:84-88`) run before the run loop, so a landing window of 0 still passes. The type check (`:95-101`) passes if the subclasses stop calling the gate. Fix: spin once so the previous attempt's block has fired and the new one has not; begin the fourth finger only after ~90 ms with `down` still under 4; send `touchesBegan` / `touchesMoved` through the subclasses with an event whose `allTouches` count is the count under test.

Checked, not findings:

Correctness. A one-finger drag past 8 pt fails inside `touchesMoved` (`DevMenuIOS.swift:45-51`); a stationary touch fails from the main-queue block 0.1 s after the first `began` (`:35-42`). Both write `.failed` only from `.possible`. That transition is legal for a subclass (the setter is the protected one), including from a timer: UIKit keeps `.failed` until the sequence ends, does not send the action, and does not cancel touches. `reset` drops `peak` and bumps `attempt` (`:53`, `:68`, `:80`), so a block from the previous attempt cannot fail the next one. Four fingers within 0.1 s of the first set `peak` to 4 (`:31-37`); the block then no-ops, and later movement is ignored (`:46-47`), which is what the 40 pt hold needs. `peak` is a max, and `reset` does not run during a double tap's pause (state stays `.possible`), so the second tap is not failed. `down` is `event.allTouches`, not `numberOfTouches` (`:31-32`, `:62`). On tvOS the gate never sees the remote: `allowedPressTypes` is empty (`:161`), `numberOfTouchesRequired` stays unset (`:132-150`), and `pressesBegan` is not overridden.

Regressions. No batch, trait, recycle, or fill path changed. Idle-tick skips stay as reviewed: `changesNothing` / `applyUnlessEmpty` (`Session.swift:946-970`) and `requestProjectionSync` (`PresenterIOS.swift:24-40`).

Code. 333 and 170 lines. Comments match the file. No dead path. `rules/RULES.md` is untouched.
