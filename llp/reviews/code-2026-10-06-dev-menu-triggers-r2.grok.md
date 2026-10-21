# Code review: the dev menu opens on a phone and over a presented controller, round 2 (042c8b48c), 2026-10-06 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `042c8b48c`, plain output.
- **Method:** one brief (sha256 `dc2171a2c3fe4dfc37d5214072b775fca7523de2b6b8b8fe7720d61916a1dbf4`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND.
- **Disposition:** no findings; the test gap it notes (reload exercised indirectly) is closed with astra's 2.

---

LAND

No findings. Round-1 dispositions hold: the fix commit matches what they said was taken, and the declined item is still true.

**Dispositions.** There is no `llp/reviews/code-2026-10-05-dev-menu-triggers*`. Against `llp/reviews/code-2026-10-06-dev-menu-triggers.*.md`:

1. Siblings are not simultaneous (`DevMenuIOS.swift:32-33`); the tap and the double tap require the press to fail (`:75`, `:87-88`). A hold reaches `.began` while the fingers are still down, so the taps are failed before lift and `menuTap` cannot run on the way up.
2. The press cancels touches (`:86`). `NodeView.touchesCancelled` clears `pressed` without `press` (`PointerIOS.swift:195-198`). Cancellation happens only after four touches have been recognized.
3. `present` (`:107-125`) is the only presenter for the sheet (`:159`), Save Trace (`:179`), the share sheet (`:186`), and Open Project (`:210`). Each try re-resolves `presenter` (`:134-137`), waits while that controller is presenting, dismissing, or still has a presented child, gives up after eight tries, and journals both the wait and the give-up.
4. One ticket (`:101`, `:108-111`). `reload` (`:217`) and toggle-close (`:143`) bump it, so a queued present does not land. Astra's baked-plan double-tap line is still lost: `note` runs, then `session.boot` replaces the runner (`:226`, `Session.swift:811-819`); `Bridge.log` only posts (`Bridge.swift:654-656`). Tap and hold do not boot, so those lines stay. Popover anchor is implemented (`:118-122`) and still untested, as the disposition said.

**Correctness.** Nothing here skips or caches a batch, a trait, a recycled view, or a fill. What changes on screen is the gesture and where the sheet is presented:

- A four-finger touch a pan or swipe would have taken exclusively now also reaches the menu (`:32-33`).
- A four-finger hold of 0.6s inside 40pt opens the menu and cancels the row press (`:82-88`).
- A sheet that is up is presented over, not refused (`:134-137`, `:147-159`). `toggle` treats a sheet whose `presentingViewController` is already nil as closed (`:143`).
- A dismiss still in flight waits; past eight tries (2s) it journals `dev menu: not shown…` and does not present (`:112-115`).

**Regressions.** `git diff --stat 743ca9652..HEAD` is `DevMenuIOS.swift`, `DevMenuIOSTests.swift`, and the two round-1 reviews. The idle-tick skip is untouched, so those holes stay as they were. One-finger pans, swipes, pull-to-reply (`maximumNumberOfTouches = 1`), and keyboard drags cannot satisfy `numberOfTouchesRequired == 4`. `delaysTouchesEnded` stays false (`:69`, `:74`, `:84`), so the press does not re-enter the delayed-event queue behind the nil-insertion crash. `shouldReceive` still requires a non-empty touch event (`:35-40`) and is the delegate for all three recognizers (`:89-90`). Agent taps are not `UITouch`s (`PointerIOS.swift:201-205`); an injected one-finger event can be delivered and still cannot recognize. On tvOS the press, like the taps, leaves `numberOfTouchesRequired` unset and `allowedPressTypes` empty (`:79-81`, `:92`), so the remote is not a trigger.

App recognizers do run beside the shortcut: `cancelsTouchesInView` cancels the view, not a pan that already began, and a context-menu long press defaults to 0.5s (`NodeViewIOS.swift:191-195`) so it can `.began` before this press at 0.6s. That is the simultaneous behavior the change asks for, and the menu still presents.

**Tests.** They fail a wrong sibling-simultaneous answer, a press that does not cancel, a hold that toggles on `.ended`, a present that does not wait out a dismissing child, and a ticket that does not drop the waiter (`DevMenuIOSTests.swift:39-48`, `:60-68`, `:96-111`). They do not fire real touches (a `UITouch` cannot be synthesized here). `require(toFail:)` has no public getter; exclusion is pinned only through the delegate. The “reload drops it” case bumps `pending` directly (`:107`) and never calls `reload()` or toggle-close. Popover `sourceView`, `isBeingPresented`, the eight-try give-up, and the double-tap action are unasserted. Those match the partial disposition, not a broken fix.

**Code.** 257 lines, same `nonisolated(unsafe)` statics and comment weight as the rest of the file. No dead path.

A phone can still eat a four-finger tap with no journal line: the tap’s movement slop is not public, so a quick tap that drifts fails on its own and the 0.6s press never starts; a finger that lands in the keyboard window or on the home-indicator grab never reaches this window; VoiceOver takes four-finger taps; a system gate that requires this recognizer to fail is not overridden by `shouldRecognizeSimultaneouslyWith`. `isMultipleTouchEnabled == false` does not do it. `EXACT_DEV_MENU=0` installs nothing (`:61`).
