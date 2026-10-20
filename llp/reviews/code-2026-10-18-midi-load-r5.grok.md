# Code review: MIDIMacTests at load: handoff-aware, then gated timing checks, round 5, 2026-10-18 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `067dc3c86`.
- **Method:** one brief (sha256 `d2fa9a7a71dc6ac4ab463911ccace6f2230dc8c68a20c180b10577ffd8d99b13`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; a should-fix (the boot waits) and a nit (load unavailable; the lone tick's message).
- **Disposition:** taken in the round-5 commit.

Nothing is blocking. With the gate open, each regression you listed still fails the assertion that covers it.

* A coalesced hold, about 90–100 ms late, misses the send ceiling of 20 ms at `MIDIMacTests.swift:162`.
* A coalesced batch drain (~140 ms) and a second hold (~150 ms) miss the 100 ms ceilings on a timed batch (`:431`) and on the lone tick (`:529`).
* A Start held with the ticks for 50 ms misses `start.at - fa.at < 0.010 + startLag` when `startLag` is at most 20 ms, so the allowance is at most 30 ms (`:444`). If that lag or the load leaves Start untimed, the test skips (`:456`).
* Ticks drained one by one fail the ungated count at `:415` (48 is not under 30). On a quiet machine they also fail the 40 ms gap between two timed batches (`:436`).

The load gate is sound on this host. `getloadavg` is already used the same way in `TableFeedPerfMacTests`, and sample 0 is the 1-minute average. `activeProcessorCount` is the logical-CPU count that average is measured against. Twice that (36 on this machine) sits below the load where a qualified send was still 35 ms late (52–125). A closed gate skips; it does not turn the assertion green.

## Findings

1. **SHOULD-FIX** — `host/apple/tests/ExactKitTests/MIDIMacTests.swift:249` (also `:269`, `:294`)

   **Scenario:** The held note-off is scheduled at `now + 2000` (`:244`, `:266`, `:287`). The new wait is 2 s from just after that capture, then another 0.3 s (`:250`) or 0.1 s (`:270`, `:295`). That original timer therefore fires inside the window. A boot that never hands the note-off early still produces `[0x80]` in the first test, and the expected status sequence in the other two. Those two then only check arrival order (`:273`, `:298`), which a note-off 2 s late still satisfies. The old fixed 300 ms window rejected that delivery.

   The same tail is the only look at a message that should not come. It starts once the expected count is already in, and it lasts 100 ms in the second and third tests (300 ms in the first). A note-on left on that `now + 2000` timer, or any extra that shows up more than a tail later, is never read. Load already spread messages by more than 300 ms, which is longer than those tails.

   A missing message is still a failure: the wait runs until the expected count or 2 s, then `XCTAssertEqual` requires the exact statuses.

   **Fix:** Schedule the held messages several seconds past any patience window (for example +8 s). Wait up to 2 s for the note-off the boot should have handed; it can arrive there only if it was handed early. Then wait until the scheduled instant has passed before asserting the note-on is absent.

2. **NIT** — `host/apple/tests/ExactKitTests/MIDIMacTests.swift:107` and `:531`

   **Scenario:** If `getloadavg` returns less than 1, `machineBusy()` reports not busy and load 0, so the gate stays open and a later skip prints load 0. Leaving the gate open is the right direction: a coalesced build is still measured. The lone-tick skip (`:531`) always blames a 20 ms main-queue lag and never prints the load, including when every attempt was dropped by the load gate alone.

   **Fix:** On a failed `getloadavg`, keep running the timing check and say the average was unreadable instead of load 0. In the lone-tick skip, print the 1-minute load next to the lag, as the send and batch skips already do.
