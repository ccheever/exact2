# A timer cannot wait for a frame, a UI update or the end of a scroll

**Status:** Open
**Systems:** Contract timers (`every`, `after`), Apple host clock (`SessionClock.swift`, `SessionClockTimer.swift`, `ExactCore/Owner.swift`), web host timers (`host/web-js/rt.js`)
**Severity:** P3
**Author:** Claude (Opus 5.5), from Lexy's iPhone Duo scroll traces
**Date:** 2026-10-27

## Symptom

A timer's action runs at its deadline, wherever that falls in a frame. On Apple, a deadline more than about 133 ms away arms a main-thread `Timer` in the common run-loop modes (`SessionClockTimer.schedule`), so it also fires while a scroll view tracks or decelerates. `fireClock` then advances the runner synchronously: the main thread waits in `Owner.sync` (a late frame's `runtime` phase) and applies the batch. Only deadlines within about 133 ms advance on the display link (LLP 1043.000 D8 and ruling 5). The web host likewise advances a distant timer inside `setTimeout` (`drive()`); only frame tasks run in `requestAnimationFrame` (`paint()`).

A timer's batch therefore gets whatever is left of the frame it lands in. A batch that would fit in a frame can still miss it because it started late, and an app has no way to say that its timer can wait.

Evidence from Lexy on exact2 `3c7eaa6bab` (iPhone Duo, iOS 27.2, production build, 120 Hz), from `startTrace`/`stopTrace`: in one fling on Status the drag missed no frames, and the glide missed 2 frames in a single event, an app batch whose main-thread turn took 18 ms (10.5 ms waiting on the runtime, 5.4 ms applying), about 10 s after launch. Whether that batch was Lexy's 5 s `tick` or a launch read is not known. On the simulator (Time Profiler, 62 s), Lexy's `tick` costs 1 to 3 ms of main-thread time and under 1 ms on `exact.owner`.

## Direction

Precise timing stays the default, since an app may need a precise timer. A timer that may wait says what it waits for, as an explicit choice on `every` and `after`. Points worth expressing, each with its platform mapping:

1. **The start of the next frame.** The deadline marks the action due, and the action runs at the start of the next frame with the whole frame's budget: `setTimeout`, then `requestAnimationFrame` on the web; one display-link callback on Apple.
2. **A frame the platform is already drawing.** With a UI update in progress, the action runs in an early phase of it, and waits another frame when the time left before the frame's deadline is less than recent batches cost; with none in progress, it runs at once. On Apple this is a passive `UIUpdateLink` and `UIUpdateInfo.completionDeadlineTime`.
3. **The end of a scroll.** The action waits while a scroll view tracks or decelerates, as a UIKit `Timer` scheduled in the default run-loop mode waits during tracking. The web's signal is `scrollend`.
4. **Idle.** The action runs when nothing else is pending: `requestIdleCallback` or `scheduler.postTask` with `background` priority on the web.

Each needs a cap so that a timer is never held indefinitely, and the runner's ordered catch-up must still hold. Two forms exist to build on: `every(frame, action)` fires once per presented frame (LLP 1073), and `task … when` arms a timer only while a condition holds (LLP 1092 D7). None of these points makes a batch longer than a frame fit in one; that needs batches that do not block the main thread, which LLP 1072 began for list rows.

## Platform facts (checked 2026-10-27)

Web:

- `requestAnimationFrame` is the only frame-aligned callback in every browser, and it gives no deadline.
- `scheduler.yield()` ships in Chromium and in Firefox from 142, and `scheduler.postTask` in Chromium and Firefox; Safari has neither ([postTask](https://caniuse.com/mdn-api_scheduler_posttask), [yield](https://caniuse.com/mdn-api_scheduler_yield)). Chrome recommends `scheduler.yield()` for breaking up long tasks ([Chrome for Developers](https://developer.chrome.com/blog/use-scheduler-yield)).
- `requestPostAnimationFrame` never shipped; WebKit closed it as WONTFIX ([bug 249798](https://bugs.webkit.org/show_bug.cgi?id=249798)).

Apple ([`CADisplayLink`](https://developer.apple.com/documentation/quartzcore/cadisplaylink), [`UIUpdateLink`](https://developer.apple.com/documentation/uikit/uiupdatelink), [`UIUpdateActionPhase`](https://developer.apple.com/documentation/uikit/uiupdateactionphase), [`UIUpdateInfo`](https://developer.apple.com/documentation/uikit/uiupdateinfo)):

- `CADisplayLink` calls back once per frame. `timestamp` is when the last frame was shown and `targetTimestamp` when the next will be, so `targetTimestamp` minus `CACurrentMediaTime()` is the time left. `preferredFrameRateRange` asks for a rate, which the system may cap for Low Power Mode, thermal state or accessibility settings. The run-loop mode it is added in decides whether it fires during tracking. On macOS 14 and later it comes from a view, window or screen. `CAMetalDisplayLink` serves Metal rendering.
- `UIUpdateLink` (iOS 18, tvOS 18, visionOS 2; not macOS; main thread only) runs actions in a chosen phase of each UI update: before or after event dispatch, before or after display-link dispatch, before or after Core Animation's commit, plus low-latency phases. With `requiresContinuousUpdates` false, the default, it only observes: its actions run while UIKit produces an update for another reason, such as a gesture or a layer change, and it causes no frames itself. `UIUpdateInfo` gives `modelTime`, `completionDeadlineTime` (when the app must finish submitting changes to the render server) and `estimatedPresentationTime`.
- Exact supports iOS 17 (`host/apple/Package.swift`), so iOS 17 and macOS need the display link instead of `UIUpdateLink`.

## Measuring

A headless simulator takes no real touches, and Instruments' Animation Hitches template refuses a simulator, so judge a change with `startTrace`/`stopTrace` on a device: flings across a timer's deadline, before and after. Exact's own `com.exact.host` signposts did not appear under xctrace's `os_signpost` instrument on the simulator; UIKit's `UpdateSequence` signposts did.
