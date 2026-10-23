# Code review: PageFactsSoonMacTests' load flake (test timing, not ordering), round 2 of 2, 2026-10-23 (grok)

- **Family:** xAI. Grok via `grok -p`, reasoning effort xhigh, Edit and Write denied, a throwaway worktree at `00980f8a8c`.
- **Method:** one brief (sha256 `9f8ed5a803612bb398c6288df36d672393c212318f73b45ba6f60167e2633676`), blind to the other reviewer; 2 rounds (the lead's ask). The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix and the opening narration before it.

**none**

The two-commit range is the wait in `PageFactsSoonMacTests.swift` and the `QUEUE.md` bullet. Both match the sources, and the wait is bounded and fails closed.

The loop at `PageFactsSoonMacTests.swift:41` keeps going only while activation is unfinished and `Date() < calm` (10s). The comma is a second required condition, so a timeout ends the loop. `XCTAssertEqual` / `XCTAssertFalse` at lines 44–45 then fail the test if `dataGeneration != generation` or `activationInFlight` is still set. Those flags are written on the main queue (`SessionActivation.swift`: `dataPreparing`, then `dataGeneration = drawnGeneration` only in `activateData`) and read on the main thread between run-loop turns.

The comment matches the click path. A non-key window’s click posts `didBecomeKey` (`PageFacts.observe` → `tellPageSoon`). Once activation has finished, `dataPreparing` is false, so `tellPageSoon` takes `tellPage()` and does not increment `noticesInFlight` (`SessionNotices.swift:55–57`).

The queue entry matches the code. The test asserts `noticesInFlight == 0` after `agentInstance.tap` (line 60). `publish` hops to `landFill` on the main queue (`Session.swift:620–624`). `apply`’s T4 join is the `landFill()` before the batch (`Session.swift:1068–1072`). `landFill` drops `noticesInFlight` and skips `apply` when the batch changes nothing (`Session.swift:653–665`). The tap is an AppKit click: `NSApp.sendEvent` of `leftMouseDown` with the mouse-up already queued (`AgentMac.swift:777–797`), and `NSButton.mouseDown` runs the tracking loop before `sendAction` (`NativeButtonsMac.swift:141–174`). The entry states that measurement and that mechanism, and it asks for a `landFill` hook as well as an `apply` hook, with the press made without that click.
