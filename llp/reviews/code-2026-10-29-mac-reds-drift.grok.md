# Code review: four Mac reds as test drift (main red), round 1 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `5c151ea331`.
- **Method:** one brief (sha256 `0842e466aa915f75b8dbd8323e2bf3eab739e83f8589034679b4efd049a34141`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

**Blocking**

none

**Non-blocking**

none

`#1141` added Show Frame Health, Frame Health Details…, and Share Frame Health… to the Develop menu with an empty key equivalent (`DevMenuMac.swift:295`). `MacShortcutTests` still checks that a shipped bar has no Develop menu and does not take ⌘R, ⌘O, ⌘D, or ⌥⌘T, and that a development bar has those four chords plus the three new titles with no chord (`NavigationRulesTests.swift:938`).

`#1144` took ordinary live task timers off `timerDue` / `advance`. On a live clock their execution time is infinite, so `timer_due_ms()` does not name them and `advance` does not fire them. One `fire_native_timer` runs the action once and, for a repeat, schedules the next beat with `beat_after` (the same phase rule `advance` used). `StoreRefreshTests` reads that row: the short task is still the native due time 10s after boot, and the refresh batch’s earliest native due is the long task 20s after the refresh (`StoreRefreshTests.swift:100`, `StoreRefreshTests.swift:172`).

The LLP 1072.000 window is unchanged. `holdTick` still arms `after(1, tick)`, posts that commit to the owner, and returns while `advanceInFlight` is set and row `a` is still on screen (`AsyncAdvanceTests.swift:96`). `deliverNativeTimer` with `asyncFills` publishes and does not apply (`NativeTaskTimers.swift:98`). The removed-row and changed-row tests then deliver the same inputs into that window. A one-shot `Timer.fire()` invalidates the timer, so the run loop cannot deliver it again.

The delayed metronome still observes the clock when the owner runs it. `deliverTask` blocks the owner before `deliverNativeTimer`, sleeps 60ms, then lets the owner run (`AsyncAdvanceTests.swift:168`). `fire_native_timer` samples the live clock inside the action, so `seen >= posted + 55`. The next native due stays on the original 40ms phase and within one interval of `seen` (`AsyncAdvanceTests.swift:177`). That is the coalesce: one delivery, next beat after the run, not a catch-up. `beats == before + 1` is one callback’s result; the phase bounds are what would fail if the reschedule walked missed beats.

`LiveClockMacTests` is the same split. `fireNativeTimer(row, now: due + 2500)` must leave `ticks == 1` (`LiveClockMacTests.swift:36`). A catch-up inside that call would not. Taking the agent clock and `advance(now: due + 5500)` must land on 4 (`LiveClockMacTests.swift:39`). That is three seek-clock beats only if the wall-clock fire scheduled `due + 3000`. A non-coalesced next due of `due + 1000` would tick through to 6 under the takeover. The old test called `advance` for the wall-clock half; after `#1144` that call fires nothing.

`deliverTask` cannot race a real Foundation fire in these methods. The wait is `Thread.sleep` on the main thread, so the main run loop never runs the overdue timer. `Timer.fire()` queues one delivery; `deliverNativeTimer` takes it before the `resume()` hop can run. Nothing in these tests returns to the run loop before the asserts (`Owner.sync` waits on a semaphore). `tearDown` calls `taskTimers.cancel()` before XCTest spins the loop again. The repeating metronome would double-deliver only if something pumped the run loop while its timer was still overdue and `pending` had been cleared. These methods do not.

Both shared classes build for iOS. `AsyncAdvanceTests` is under `#if os(iOS) || os(macOS)`. `timerForTests` is `#if DEBUG` (`NativeTaskTimers.swift:82`), and both lanes compile ExactKit in Debug: Mac `swift build --build-tests`, iOS `xcodebuild build-for-testing` with no Release configuration (`build.mjs:1445`, `build.mjs:1477`). `StoreRefreshTests` does not call `timerForTests`.
