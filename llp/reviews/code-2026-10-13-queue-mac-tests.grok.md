# Review: QUEUE's pre-existing Mac Swift host test failures, 2026-10-13 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, the landing worktree, read-only by instruction.
- **Method:** one brief (sha256 `a7435b31297f42511f437e54783b3bae872523df5f4ec3ba52d2113a1c0c5ea8`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** NOT READY.
- **Disposition:** The MAJOR taken: the hatch time is the outer call's, 176 ms on 4387e37b5 and 131 on ae01efb63, and 'the same' now names the failing tests. Not re-reviewed.

---

**[MAJOR]** The hatch time is the outer call, and the two runs do not match — `QUEUE.md:830`

`HatchDiagnosticsTests.testACallsTimeIsItsOwn` fails on `XCTAssertLessThan(outer, 20)` (`host/apple/tests/ExactKitTests/HatchDiagnosticsTests.swift:82`). The printed value is the outer `navigation` call. The nested `element dot` check (`inner >= 20`) did not fail. On ae01efb63 that outer time is 131.4 ms; on 4387e37b5 it is 176.2 ms. The entry calls 131 ms the nested call and treats the two runs as the same measurement.

The other three failures match on both logs, including the quoted values: selection 0.345102 against 0.27451 (`hdrm(1)` on the drawn color), press wash `[1.0, 0.18]` against `[0.0, 0.16]`, and labels 20, 21, and 22. `HeaderMacTests` passed on both, including the new feed-first test only present on ae01efb63. `BackSessionMacTests` passed on 4387e37b5 (2.702 s) and ae01efb63 (3.182 s). The earlier log’s crash is the Swift fatal error `Negative value is not representable` (`xctest` SIGTRAP). `FirstResponder` is inside `#if os(iOS) || os(tvOS)`, so that move is not on the Mac test path. These four failures are already on 4387e37b5, before the `HeaderRule` edit. macOS 27.0 and M5 Pro match this machine.

NOT READY
