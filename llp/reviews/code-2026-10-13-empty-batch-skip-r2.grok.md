# Code review: applyUnlessEmpty lands published slices, then skips an empty batch round 2, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `7cb606468800f559bd0390c037919a4a6a7f308c36ffe0c97d40f81380705070`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** READY.
- **Disposition:** Both MINORs taken in the landing commit: the test now checks the report applies after the skip (it fails without the hold), and the QUEUE entry names the result bundle's assertions.

---

No BLOCKER or MAJOR. Two MINORs. The round-1 ordering fix itself holds.

**MINOR** — `host/apple/tests/ExactKitTests/IdleTickTests.swift:83`

`appliedBatches == held + 2` stays green if the `holdingReports` bracket around `landFill` is removed and `drainReports` is left on the skip path. The slice's own outermost `apply` then asks the colours report (one apply for the slice, one for the report), and the skip afterwards writes the scroll batch's `timerDue` over the report. That is the loss round 1 fixed. The second call never reads `timerDue`, and on a session with no scheme `colors_changed` returns before any paint motion, so `changesNothing` stays true and the scroll is still skipped. The first half of the test does lock the landing and the skip (`before + 1`, `timerDue == due`). Record `timerDue` at the start of the colours `apply` and require it to already be the scroll deadline.

**MINOR** — `QUEUE.md:870`

The parenthetical says the result bundle records a missing temporary JPEG in `PickerColorTests`. The bundle's two `PickerColorTests` failures are `XCTAssertTrue failed - gainmap-iso.heic: still HDR as JPEG` and a bare `XCTAssertTrue failed` on `testOrientationIsAppliedNotCopied` (`Picker.isHDR`). Both run after `convert` has written the JPEG. The other six are colour-storage assertions (`RGBA8` vs `RGBAh`, `rec2100-hlg` vs `display-p3`, `pq.heic: stored as SDR`, `gainmap-iso.jpg` as an `isHDR` label in `HDRImageTests`). The eight names, the iPhone 17 Pro simulator on iOS 27.0, commit `48755525c`, "cause unconfirmed", and "xcodebuild's log names no assertion" match `/tmp/x2p2-iosbase.log` and `Test-Exact-2026.10.13_04-35-03--0700.xcresult` (8 failed, 1170 passed).

The rest of the change checks out.

`applyUnlessEmpty` (`Session.swift:977`) lands a published slice under `holdingReports`, returns after clearing that flag if the session was destroyed, then either `apply`s the later batch or takes the skip. A later batch whose frames differ fails `changesNothing` and goes through `apply`, so its clock and frames win. A later batch that still changes nothing keeps its deadline on the skip (`timerDue`, `scheduleClock`) and only then calls `drainReports` (`Session.swift:991`). `apply`'s outermost block uses the same drain (`Session.swift:1068`). While `applying` is set, a re-entrant call goes straight to `apply`. While `landing` is set, an inner `drainReports` sees `holdingReports` and leaves the reports for the outer drain. `drainFill` / `drainAll` are unchanged. The only callers are `onScroll`, `onScrollend`, and the clock timer; the agent advances with `apply`, and async fills are off under the agent. `publishTickForTests` and `oweColorsForTests` are `#if DEBUG`. `SessionClockTimer.swift` is the same timer, and `Session.swift` ends at line 1491.

READY
