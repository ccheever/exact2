# Code review: James's display-link scrolling, round 2 (d9f97e21a..576613352), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `576613352`.
- **Method:** one brief (sha256 `c488ed29f58d0d61b1dfdd5af88bb84f9aed08d2efebb7de91281767d498340a`), shared with grok. Round 2, blind to the other review. The authors are not reviewers.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition (r3):** 1 taken: `SvgFilterLive.animate` hops to the main queue when called off it (a nested filter updated inside its outer one's encode), so the clock is only touched on main, in order. No Thread Sanitizer run. 2 taken: a flight's source is the leaver's presentation geometry while a press ease runs on it or an ancestor (`Presenter.shownRect`), its model's otherwise, since a presentation lags a model set in the same batch; `testAFlightStartsFromThePressedBoxAsShown`. 3 taken in part: the smooth correction's test reads its full-rate vote off the shared clock; no new pump idle-path tests (the pump's owed-work condition is James's and unchanged since round 1).

---

DO NOT LAND

1. **MATERIAL — Nested SVG filters still access FrameClock off-main.** Removing the deinit call fixes only one path. [SvgFilterLive.swift:197](host/apple/Sources/ExactKit/SvgFilterLive.swift:197) runs `encode` on its worker, which calls `scene.apply`. That can update or remove a nested live filter through [SvgScene.swift:274](host/apple/Sources/ExactKit/SvgScene.swift:274), reaching `FrameClock.want/drop` from the worker. Toggling an inner animation inside a text-free outer blur therefore races the shared dictionary’s main-thread dispatch. **Fix:** confine nested filters’ clock registration and removal to main; test changing/removing nested filters under Thread Sanitizer.

2. **MATERIAL — Shared-element flights capture the press target instead of the displayed box.** [PressFeedback.swift:83](host/apple/Sources/ExactKit/PressFeedback.swift:83) now puts the target into the model immediately, but [FlightsIOS.swift:81](host/apple/Sources/ExactKit/IOS/FlightsIOS.swift:81) still captures `UIView.convert` geometry. Release a settled `press-scale: .5` button whose handler starts a flight: a 200-point button still displays at 100 points, while the flight captures 200. Previously the model retained the displayed press factor. **Fix:** capture presentation geometry, including pressed ancestors, with a model fallback. Assert that flight progress zero matches the displayed source during release.

3. **MINOR — Tests leave the scheduling fixes weakly protected.** [FrameClockIOSTests.swift:42](host/apple/tests/ExactKitTests/FrameClockIOSTests.swift:42) tests synthetic owners, so reverting a migrated user to `.default` still passes. [ScrollPumpIOSTests.swift:49](host/apple/tests/ExactKitTests/ScrollPumpIOSTests.swift:49) explicitly scrolls to wake work. **Fix:** assert actual users’ rate votes and test text invalidation and landed-fill work after the pump becomes idle, without another scroll.

The other round-1 fixes hold on inspection; the earlier idle-tick repair paths remain intact. Changed sources meet the 1,500-line limit; `git diff --check` passes. UIKit tests were not run in this read-only checkout.