# Code review: PageFactsSoonMacTests' load flake (test timing, not ordering), round 1 of 2, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `ecc3a2fa91`.
- **Method:** one brief (sha256 `ead292e89fa2c5ca6598a91a37892b1d888a1f5883d57f8a73cf10b47e13cc8f`), blind to the other reviewer; 2 rounds (the lead's ask). The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

BLOCKER: none found.

- **SHOULD — [QUEUE.md:10](QUEUE.md:10):** The coverage gap is real, but its explanation overstates the agent’s guarantee. This test calls `agentInstance.tap` directly, bypassing `Agent.handle`’s `drainAll()` and input completion. The macOS tap body does not unconditionally settle the session. AppKit’s click/tracking path can service queued work, so the final counter still cannot establish application order. Describe that limitation without claiming guaranteed settlement. Also, an `apply` hook may miss unchanged notices: `landFill` retires those without calling `apply`.

- **NIT — [PageFactsSoonMacTests.swift:37](host/apple/tests/ExactKitTests/PageFactsSoonMacTests.swift:37):** “The session’s own data activation … done first” is a sentence fragment, and the parenthetical recounts the investigation. Suggested wording: “Wait for first-draw activation to finish so system notices raised by the click take the synchronous path and cannot inflate `noticesInFlight`.”

The diagnosis is consistent with the code: `PageFacts.observe` hears key-window changes; `tellPageSoon` queues three batches during preparation; owner publication precedes later synchronous work, and `apply` joins those publications. Three outstanding batches therefore need not belong to the test’s earlier notice.

The new condition closes the startup gap: `dataGeneration` prevents premature success before the first draw, and `activationInFlight` covers remaining preparation/native activation. A missing draw produces a timeout assertion failure. The deadline bounds polling, though it cannot interrupt a blocking callback. The existing T4 coverage weakness remains; this change neither creates nor resolves it.

Static review only; XCTest was not rerun in this read-only checkout without build artifacts. No files edited.
