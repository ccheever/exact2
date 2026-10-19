# Code review: MIDIMacTests at load: handoff-aware, then gated timing checks, round 5, 2026-10-18 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `067dc3c86`.
- **Method:** one brief (sha256 `d2fa9a7a71dc6ac4ab463911ccace6f2230dc8c68a20c180b10577ffd8d99b13`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking; two should-fix (the boot waits reach the held messages' own 2 s; a batched Start can ride the last tick's drain) and two nits (no load reading taken as quiet; the lone tick's skip text).
- **Disposition:** all taken in the round-5 commit.

Nothing blocking. Read-only review completed; no builds or tests run.

1. **SHOULD-FIX — Boot waits reach the original send deadlines.** [MIDIMacTests.swift:249](/tmp/rv-ios-15/host/apple/tests/ExactKitTests/MIDIMacTests.swift:249), also lines 269 and 294. The messages are scheduled at `now + 2000`, and the new waits allow two seconds plus a grace period. If boot correctly drops held note-ons but leaves note-offs scheduled, those offs can arrive at their original deadlines and all three tests pass. **Fix:** schedule held messages comfortably beyond the observation window, such as `now + 10000`, keeping the bounded waits and exact-array assertions.

2. **SHOULD-FIX — A batched Start can still pass a qualified measurement.** [MIDIMacTests.swift:443](/tmp/rv-ios-15/host/apple/tests/ExactKitTests/MIDIMacTests.swift:443). Sender jitter can leave the final tick in its own batch, whose deadline is only about 0.83 ms before Start. A permitted 6–10 ms main-thread delay then lets an incorrectly batched Start ride that pending drain and satisfy `10 ms + startLag`. The batch count and spacing checks also pass. **Fix:** ensure the preceding ticks have drained before sending an isolated Start, with no subsequent message flushing it during the measurement.

3. **NIT — Unavailable load is reported as an idle machine.** [MIDIMacTests.swift:107](/tmp/rv-ios-15/host/apple/tests/ExactKitTests/MIDIMacTests.swift:107). If `getloadavg` fails, `(false, 0)` qualifies timing without the required load evidence and reports a fabricated zero. **Fix:** preserve “unavailable” and visibly skip timing qualification. The API itself is available on this macOS-only path; `activeProcessorCount` supplies active logical cores. Twice that count is a reasonable overload heuristic, not a guarantee against brief scheduling stalls.

4. **NIT — The lone-tick skip misreports load-only rejection.** [MIDIMacTests.swift:531](/tmp/rv-ios-15/host/apple/tests/ExactKitTests/MIDIMacTests.swift:531). When all attempts fail only the new load gate, the message incorrectly says main lag exceeded 20 ms and omits the load. This gated test also lacks the timing-count print added elsewhere. **Fix:** report the measured load, actual rejection reason, and qualified attempt count.

For qualified measurements, the fixed ceilings still reject the stated coalesced hold/drain delays and a second hold. Always draining ticks individually still trips the count/lower-bound checks. The boot assertions continue catching persistent omissions and extras observed within their windows; finding 1 is specifically a late scheduled delivery masquerading as successful boot settlement.
